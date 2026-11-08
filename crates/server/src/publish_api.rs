//! Review, commit and restore routes (S04-W4, W5).
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `POST /projects/{id}/tasks/{task}/check` | member | Run the engine commit check on the task's worktree now |
//! | `POST .../tasks/{task}/commit` | `ApproveProposal` | Commit the listed proposals, bound to their hashes, after the check |
//! | `POST .../tasks/{task}/restore` | `ApproveProposal` | Undo one factory commit, keeping later edits |
//! | `GET .../tasks/{task}/publications` | member | Commits, pushes, pull requests and restores of the task |
//!
//! Each effect has an idempotency key. Repeating a request with the same key returns the first
//! result, and a request whose reply was lost is reconciled by reading the branch.

use std::collections::BTreeSet;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use factory_accounts::{ApiError, ApiJson, Permission, Principal, Project, RetryClass};
use factory_tasks::{Actor, Task, TaskMode};
use factory_workspace::{GitRepo, Oid, RestoreLabel, TaskWorktree};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::{active_project_for, project_for};
use crate::agent::runner::{event, ws_task};
use crate::agent_api::{ensure_worktree, load_task, runtime};
use crate::api_error::Fail;
use crate::focus::store::{self as fstore, TaskKeys};
use crate::intake_api::{accepted_origins, origin_guard};
use crate::publish::{self, Checked, check_summary, trailer_line};
use crate::state::ServerState;
use crate::workspace_api::open_workspace;

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route("/projects/{id}/tasks/{task}/check", post(check))
        .route("/projects/{id}/tasks/{task}/commit", post(commit))
        .route("/projects/{id}/tasks/{task}/restore", post(restore))
        .route("/projects/{id}/tasks/{task}/publications", get(publications))
        .layer(middleware::from_fn_with_state(accepted, origin_guard))
        .with_state(state.clone())
}

fn internal(e: impl std::fmt::Display) -> Fail {
    tracing::error!(error = %e, "publication failure");
    ApiError::internal().into()
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 200 && key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

/// The task worktree and the primary repository, opened at its current HEAD.
async fn open(state: &ServerState, project: &Project, task: &Task) -> Result<(GitRepo, TaskWorktree), Fail> {
    let (dir, _) = ensure_worktree(state, project, task.id).await?;
    let workspace = open_workspace(state, project)?;
    let id = ws_task(task.id);
    tokio::task::spawn_blocking(move || {
        let repo = workspace.git().map_err(internal)?.ok_or_else(|| internal("no repository"))?;
        let worktree = repo.open_task_worktree(&id, &dir, None).map_err(internal)?;
        Ok((repo, worktree))
    })
    .await
    .map_err(internal)?
}

async fn run_check(state: &ServerState, project: &Project, task: &Task, keys: &TaskKeys) -> Result<(TaskWorktree, Checked), Fail> {
    let rt = runtime(state)?;
    let (repo, worktree) = open(state, project, task).await?;
    let head = worktree.expected_head().clone();
    let checked = publish::check_worktree(rt, keys, &repo, worktree.work_dir(), &head).await.map_err(internal)?;
    Ok((worktree, checked))
}

async fn check(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let keys = TaskKeys::of(&task, principal.account_id());
    let (_, checked) = run_check(&state, &project, &task, &keys).await?;
    let summary = check_summary(&checked);
    // The latest check travels with every proposal still waiting for review.
    let mut tx = rt.tx(principal.account_id()).await?;
    for p in fstore::list_proposals(&mut tx, task.id).await? {
        if p["state"] == "applied"
            && let Some(pid) = p["id"].as_str().and_then(|s| s.parse::<Uuid>().ok())
        {
            fstore::set_check(&mut tx, pid, &summary, &checked.delta["summary"]).await?;
        }
    }
    tx.commit().await?;
    Ok(Json(json!({ "check": summary, "finding_delta": checked.delta })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalRef {
    id: Uuid,
    proposal_hash: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitBody {
    proposals: Vec<ProposalRef>,
    message: String,
    idempotency_key: String,
    /// The person read the check and accepts an `incomplete` verdict.
    #[serde(default)]
    accept_incomplete: bool,
}

async fn commit(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<CommitBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::ApproveProposal).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if task.mode != TaskMode::Edit {
        return Err(ApiError::conflict("read_only_task", "A read-only task has nothing to commit.").into());
    }
    let message = body.message.trim().to_owned();
    if message.is_empty() || message.chars().count() > 2000 || message.contains(publish::TRAILER) {
        return Err(ApiError::invalid_request("The commit message is 1 to 2000 characters.").with_id("field", "message").into());
    }
    if !valid_key(&body.idempotency_key) || body.proposals.is_empty() || body.proposals.len() > 200 {
        return Err(ApiError::invalid_request("Name 1 to 200 proposals and an idempotency key.").into());
    }
    let keys = TaskKeys::of(&task, principal.account_id());

    // A repeated request returns the first result. One whose reply was lost is reconciled.
    let mut tx = rt.tx(principal.account_id()).await?;
    let existing = publish::find_publication(&mut tx, task.id, "commit", &body.idempotency_key).await?;
    tx.commit().await?;
    if let Some(p) = existing {
        return reconcile_commit(&state, &project, &task, p).await;
    }

    // Approval binds to each proposal's hash and to its unreviewed state.
    let mut tx = rt.tx(principal.account_id()).await?;
    let mut proposals = Vec::new();
    for r in &body.proposals {
        let p = fstore::get_proposal(&mut tx, task.id, r.id).await?.ok_or_else(|| Fail::from(ApiError::not_found()).with_id("proposal", r.id))?;
        if p["proposalHash"] != json!(r.proposal_hash) {
            return Err(Fail::from(ApiError::conflict("proposal_changed", "A proposal is not the one you reviewed.")).with_id("proposal", r.id));
        }
        if p["state"] != "applied" {
            return Err(Fail::from(ApiError::conflict("proposal_not_open", "A proposal was already committed or discarded.")).with_id("proposal", r.id));
        }
        proposals.push(p);
    }
    tx.commit().await?;
    let paths: Vec<String> = proposals
        .iter()
        .flat_map(|p| p["changedPaths"].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // The commit check. New findings block a commit that holds agent work; a person decides on
    // their own work with the same report in front of them.
    let (worktree, checked) = run_check(&state, &project, &task, &keys).await?;
    let summary = check_summary(&checked);
    let has_agent_work = proposals.iter().any(|p| p["actorKind"] != "person");
    let actor = if has_agent_work { factory_engine_check::CommitActor::Agent } else { factory_engine_check::CommitActor::Person };
    match factory_engine_check::commit_gate(&checked.report, actor) {
        factory_engine_check::CommitDecision::Blocked { findings } => {
            return Err(Fail::from(ApiError::conflict("commit_blocked", "The commit check found new problems in the agent's changes. Nothing was committed."))
                .with_details(json!({ "check": summary, "findings": findings })));
        }
        factory_engine_check::CommitDecision::Allowed { verdict } => {
            if verdict == factory_engine_check::Verdict::Incomplete && !body.accept_incomplete {
                return Err(Fail::from(ApiError::new(
                    StatusCode::CONFLICT,
                    "check_incomplete",
                    "The check could not decide everything. Incomplete is not a pass: confirm that you accept it.",
                    RetryClass::RefreshPreconditions,
                ))
                .with_details(json!({ "check": summary })));
            }
        }
    }

    let head = worktree.expected_head().clone();
    let intent = json!({
        "head": head.as_str(),
        "branch": worktree.branch(),
        "paths": paths,
        "proposals": proposals.iter().map(|p| json!({"id": p["id"], "proposal_hash": p["proposalHash"]})).collect::<Vec<_>>(),
        "message": message,
        "check": summary,
    });
    let mut tx = rt.tx(principal.account_id()).await?;
    let pub_id = publish::insert_intent(&mut tx, &keys, "commit", &body.idempotency_key, None, &intent).await?;
    tx.commit().await?;

    let author = publish::author_for(&principal.account);
    let full_message = format!("{message}\n\n{}", trailer_line(pub_id));
    let label = RestoreLabel::new(format!("before-{}", &pub_id.simple().to_string()[..12])).map_err(internal)?;
    let paths2 = paths.clone();
    let mut wt = worktree;
    let outcome = tokio::task::spawn_blocking(move || {
        let point = wt.create_restore_point(&label).map_err(|e| e.to_string())?;
        let oid = wt.commit_paths(&paths2, &full_message, Some(&author)).map_err(|e| e.to_string())?;
        Ok::<_, String>((point, oid))
    })
    .await
    .map_err(internal)?;
    let (point, oid) = match outcome {
        Ok(v) => v,
        Err(e) => {
            let mut tx = rt.tx(principal.account_id()).await?;
            publish::settle(&mut tx, pub_id, "failed", &json!({ "error": e })).await?;
            tx.commit().await?;
            return Err(Fail::from(ApiError::conflict("commit_failed", "The commit did not happen. The worktree is unchanged.")).with_details(json!({ "reason": e })));
        }
    };
    let result = json!({
        "commit": oid.as_str(),
        "parent": head.as_str(),
        "restore_point": {"ref": point.ref_name, "oid": point.oid.as_str()},
        "paths": paths,
        "check": summary,
    });
    let mut tx = rt.tx(principal.account_id()).await?;
    publish::settle(&mut tx, pub_id, "succeeded", &result).await?;
    let ids: Vec<Uuid> = body.proposals.iter().map(|p| p.id).collect();
    fstore::set_state(&mut tx, task.id, &ids, "applied", "committed").await?;
    for pid in &ids {
        fstore::set_check(&mut tx, *pid, &summary, &checked.delta["summary"]).await?;
    }
    factory_tasks::append_event(
        &mut tx,
        task.id,
        event(None, Actor::user(principal.account_id()), "committed", "Changes committed", json!({ "publication_id": pub_id, "commit": oid.as_str(), "paths": result["paths"], "check_verdict": summary["verdict"] }))
            .with_snapshot(oid.to_string()),
    )
    .await
    .map_err(internal)?;
    tx.commit().await?;
    rt.notify(task.id);
    Ok((StatusCode::CREATED, Json(json!({ "publication_id": pub_id, "result": result }))))
}

/// A commit request seen before. A settled one returns its result. One left as an intent
/// (the reply was lost or the server stopped) is decided by reading the branch for its trailer.
async fn reconcile_commit(state: &ServerState, project: &Project, task: &Task, p: publish::Publication) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(state)?;
    if p.state == "succeeded" {
        return Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "result": p.result, "replayed": true }))));
    }
    if p.state == "failed" {
        return Err(Fail::from(ApiError::conflict("commit_failed", "This commit request failed earlier. Use a new idempotency key to try again.")).with_details(json!({ "result": p.result })));
    }
    let (_, worktree) = open(state, project, task).await?;
    let head = worktree.expected_head().clone();
    let line = trailer_line(p.id);
    let found = {
        let repo = worktree.repo().clone();
        tokio::task::spawn_blocking(move || repo.find_commit_with_line(&head, &line)).await.map_err(internal)?.map_err(internal)?
    };
    let mut tx = rt.tx(task.owner_account_id).await?;
    let outcome = match found {
        Some(oid) => {
            let result = json!({ "commit": oid.as_str(), "parent": p.intent["head"], "paths": p.intent["paths"], "reconciled": true });
            publish::settle(&mut tx, p.id, "succeeded", &result).await?;
            let ids: Vec<Uuid> = p.intent["proposals"].as_array().into_iter().flatten().filter_map(|x| x["id"].as_str()?.parse().ok()).collect();
            fstore::set_state(&mut tx, task.id, &ids, "applied", "committed").await?;
            Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "result": result, "replayed": true }))))
        }
        None => {
            publish::settle(&mut tx, p.id, "failed", &json!({ "reconciled": true, "reason": "no commit carries this request" })).await?;
            Err(Fail::from(ApiError::conflict("commit_failed", "That commit never happened. Use a new idempotency key to try again.")))
        }
    };
    tx.commit().await?;
    outcome
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreBody {
    publication_id: Uuid,
    idempotency_key: String,
}

async fn restore(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<RestoreBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::ApproveProposal).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if !valid_key(&body.idempotency_key) {
        return Err(ApiError::invalid_request("Give an idempotency key.").into());
    }
    let keys = TaskKeys::of(&task, principal.account_id());
    let mut tx = rt.tx(principal.account_id()).await?;
    if let Some(p) = publish::find_publication(&mut tx, task.id, "restore", &body.idempotency_key).await? {
        tx.commit().await?;
        return Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "state": p.state, "result": p.result, "replayed": true }))));
    }
    let target = publish::get_publication(&mut tx, task.id, body.publication_id).await?;
    tx.commit().await?;
    let Some((kind, target)) = target else { return Err(ApiError::not_found().into()) };
    if kind != "commit" || target.state != "succeeded" {
        return Err(ApiError::conflict("not_restorable", "Only a completed commit can be restored.").into());
    }
    let result = target.result.unwrap_or_default();
    let commit = Oid::new(result["commit"].as_str().unwrap_or_default()).map_err(internal)?;
    let parent = Oid::new(result["parent"].as_str().unwrap_or_default()).map_err(internal)?;
    let paths: Vec<String> = result["paths"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();

    let (_, worktree) = open(&state, &project, &task).await?;
    let head = worktree.expected_head().clone();
    let intent = json!({ "head": head.as_str(), "restores": body.publication_id, "commit": commit.as_str(), "parent": parent.as_str(), "paths": paths });
    let mut tx = rt.tx(principal.account_id()).await?;
    let pub_id = publish::insert_intent(&mut tx, &keys, "restore", &body.idempotency_key, None, &intent).await?;
    tx.commit().await?;

    let author = publish::author_for(&principal.account);
    let message = format!("Restore the files of {}\n\n{}", &commit.as_str()[..12], trailer_line(pub_id));
    let mut wt = worktree;
    let outcome = tokio::task::spawn_blocking(move || {
        let repo = wt.repo().clone();
        let plan = publish::restore_files(&repo, wt.work_dir(), &commit, &parent, &paths)?;
        if plan.restore.is_empty() {
            return Ok((plan, None));
        }
        let oid = wt.commit_paths(&plan.restore, &message, Some(&author)).map_err(|e| e.to_string())?;
        Ok::<_, String>((plan, Some(oid)))
    })
    .await
    .map_err(internal)?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let response = match outcome {
        Ok((plan, Some(oid))) => {
            let result = json!({ "commit": oid.as_str(), "parent": head.as_str(), "restored": plan.restore, "kept": plan.kept });
            publish::settle(&mut tx, pub_id, "succeeded", &result).await?;
            factory_tasks::append_event(
                &mut tx,
                task.id,
                event(None, Actor::user(principal.account_id()), "restored", "Files restored", json!({ "publication_id": pub_id, "result": result })).with_snapshot(oid.to_string()),
            )
            .await
            .map_err(internal)?;
            Ok((StatusCode::CREATED, Json(json!({ "publication_id": pub_id, "result": result }))))
        }
        Ok((plan, None)) => {
            let result = json!({ "restored": [], "kept": plan.kept });
            publish::settle(&mut tx, pub_id, "failed", &result).await?;
            Err(Fail::from(ApiError::conflict("nothing_to_restore", "Every file of that commit changed since, so all were kept.")).with_details(result))
        }
        Err(e) => {
            publish::settle(&mut tx, pub_id, "failed", &json!({ "error": e })).await?;
            Err(internal(e))
        }
    };
    tx.commit().await?;
    rt.notify(task.id);
    response
}

async fn publications(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let rows = sqlx::query("SELECT id, kind, idempotency_key, state, intent, result, created_at, settled_at FROM publications WHERE task_id = $1 ORDER BY created_at, id")
        .bind(task.id)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    use sqlx::Row;
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "kind": r.get::<String, _>("kind"),
                "idempotencyKey": r.get::<String, _>("idempotency_key"),
                "state": r.get::<String, _>("state"),
                "intent": r.get::<Value, _>("intent"),
                "result": r.get::<Option<Value>, _>("result"),
                "createdAt": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                "settledAt": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("settled_at"),
            })
        })
        .collect();
    Ok(Json(json!({ "publications": list })))
}
