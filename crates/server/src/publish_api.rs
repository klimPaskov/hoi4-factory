//! Review, commit and restore routes (S04-W4, W5).
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `POST /projects/{id}/tasks/{task}/check` | member | Run the engine commit check on the task's worktree now |
//! | `GET /projects/{id}/import-report` | member | The engine baseline and preservation report of the newest import or commit |
//! | `POST .../tasks/{task}/commit` | `ApproveProposal` | Commit the listed proposals, bound to their hashes, after the check |
//! | `POST .../tasks/{task}/restore` | `ApproveProposal` | Undo one factory commit, keeping later edits |
//! | `GET .../tasks/{task}/publications` | member | Commits, pushes, pull requests and restores of the task |
//! | `POST .../tasks/{task}/publication-grants` | `Publish` | A C-08 grant for `push_owned_branch` and `open_pull_request` on this task's branch |
//! | `POST .../tasks/{task}/push` | `Publish`, a grant | Push the task branch to the linked GitHub repository |
//! | `POST .../tasks/{task}/pull-request` | `Publish`, a grant | Open a pull request from the task branch into the default branch, never merge |
//! | `GET .../tasks/{task}/download` | member | The task branch's committed files as a ZIP |
//! | `POST .../tasks/{task}/sync` | `CreateTask` | Take in commits pushed to the task branch from outside the factory, and list what changed |
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
        .route("/projects/{id}/import-report", get(import_report))
        .route("/projects/{id}/tasks/{task}/commit", post(commit))
        .route("/projects/{id}/tasks/{task}/restore", post(restore))
        .route("/projects/{id}/tasks/{task}/publications", get(publications))
        .route("/projects/{id}/tasks/{task}/publication-grants", post(publication_grant))
        .route("/projects/{id}/tasks/{task}/push", post(push))
        .route("/projects/{id}/tasks/{task}/pull-request", post(pull_request))
        .route("/projects/{id}/tasks/{task}/download", get(download))
        .route("/projects/{id}/tasks/{task}/sync", post(sync))
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

// ---- GitHub publication (C-10) ----

fn github_fail(e: factory_intake::IntakeError) -> Fail {
    use factory_intake::IntakeError as E;
    let (status, retry) = match &e {
        E::RepositoryNotAllowed | E::RepositoryNotInInstallation | E::AccessRevoked => (StatusCode::FORBIDDEN, RetryClass::RequiresAuthorization),
        E::GithubRateLimited { .. } | E::GithubUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, RetryClass::ReconcileFirst),
        E::InvalidRequest(_) => (StatusCode::UNPROCESSABLE_ENTITY, RetryClass::Terminal),
        _ => (StatusCode::BAD_GATEWAY, RetryClass::ReconcileFirst),
    };
    Fail::from(ApiError::new(status, e.code(), e.to_string(), retry))
}

/// The project's active GitHub link and its allowlist.
async fn link_for(state: &ServerState, principal: &Principal, project: &Project) -> Result<(factory_intake::RepositoryLink, factory_intake::RepositoryAllowlist), Fail> {
    let store = state.intake.service.store();
    let links = store.links_for_project(principal.account_id().0, project.id.0).await.map_err(github_fail)?;
    let link = links
        .into_iter()
        .find(|l| l.state == factory_intake::LinkState::Active)
        .ok_or_else(|| Fail::from(ApiError::conflict("no_repository_link", "This project is not linked to a GitHub repository.")))?;
    let allowlist = store.allowlist(principal.account_id().0, project.id.0).await.map_err(github_fail)?;
    Ok((link, allowlist))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantBody {
    operation_classes: Vec<String>,
}

/// A publication grant: separate from the task's editing grant, held by a person who may
/// publish, limited to push and pull request on this task's branch, valid for two hours.
async fn publication_grant(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<GrantBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::Publish).await?;
    let role = project.role.ok_or_else(ApiError::not_found)?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let mut classes = Vec::new();
    for c in &body.operation_classes {
        match factory_broker::OperationClass::parse(c) {
            Some(class @ (factory_broker::OperationClass::PushOwnedBranch | factory_broker::OperationClass::OpenPullRequest)) => classes.push(class),
            _ => return Err(ApiError::invalid_request("A publication grant names push_owned_branch and open_pull_request only.").with_id("field", "operation_classes").into()),
        }
    }
    let (link, _) = link_for(&state, &principal, &project).await?;
    let branch = factory_workspace::task_branch_name(&ws_task(task.id));
    let now = chrono::Utc::now();
    let mut tx = rt.tx(principal.account_id()).await?;
    let grant = factory_broker::issue_grant(
        &mut tx,
        factory_broker::NewGrant {
            project_id: task.project_id,
            owner_account_id: task.owner_account_id,
            grantor_account_id: principal.account_id(),
            task_id: task.id.0,
            operation_classes: classes,
            allowed_roots: vec![branch.clone()],
            provider_destinations: vec![format!("github:{}", link.repository_full_name)],
            max_cost_micros: 0,
            max_turns: 0,
            max_changed_files: 0,
            expires_at: now + chrono::Duration::hours(2),
            policy_version: crate::agent::runner::POLICY_VERSION.to_owned(),
            proposal_sha256: None,
        },
        role,
        now,
    )
    .await
    .map_err(|e| Fail::from(ApiError::forbidden(e.to_string())))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({ "grant_id": grant.id.0, "epoch": grant.revocation_epoch, "branch": branch, "repository": link.repository_full_name, "expires_at": grant.expires_at }))))
}

/// Checks the grant for `class` immediately before the effect and records the intent, in one
/// transaction (C-08: check before each effect; C-10: intent before the call).
async fn grant_and_intent(
    rt: &crate::agent::AgentRuntime,
    principal: &Principal,
    keys: &TaskKeys,
    grant: Uuid,
    epoch: i64,
    class: factory_broker::OperationClass,
    kind: &str,
    key: &str,
    intent: &Value,
) -> Result<Uuid, Fail> {
    let mut tx = rt.tx(principal.account_id()).await?;
    let snapshot = factory_broker::check_for_effect(&mut tx, factory_broker::GrantId(grant), epoch, class, chrono::Utc::now())
        .await
        .map_err(|e| Fail::from(ApiError::new(StatusCode::FORBIDDEN, "grant_refused", e.to_string(), RetryClass::RequiresAuthorization)))?;
    if snapshot.grant.task_id != keys.id.0 {
        return Err(Fail::from(ApiError::new(StatusCode::FORBIDDEN, "grant_refused", "The grant is for another task.", RetryClass::RequiresAuthorization)));
    }
    let id = publish::insert_intent(&mut tx, keys, kind, key, Some(grant), intent).await?;
    tx.commit().await?;
    Ok(id)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PushBody {
    grant_id: Uuid,
    epoch: i64,
    idempotency_key: String,
}

async fn push(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<PushBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::Publish).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if !valid_key(&body.idempotency_key) {
        return Err(ApiError::invalid_request("Give an idempotency key.").into());
    }
    let keys = TaskKeys::of(&task, principal.account_id());
    let (link, allowlist) = link_for(&state, &principal, &project).await?;
    let installation = factory_intake::InstallationRef::new(link.installation_ref.clone()).map_err(github_fail)?;
    let target = factory_intake::PublishTarget {
        installation: &installation,
        repository: &link.repository_full_name,
        allowlist: &allowlist,
        default_branch: link.default_branch.as_deref(),
    };
    let github = state.intake.service.github();
    let (_, worktree) = open(&state, &project, &task).await?;
    let branch = worktree.branch().to_owned();
    let local = worktree.expected_head().to_string();

    // A request seen before: return its result, or reconcile a lost reply by reading the remote.
    let mut tx = rt.tx(principal.account_id()).await?;
    let existing = publish::find_publication(&mut tx, task.id, "push", &body.idempotency_key).await?;
    tx.commit().await?;
    if let Some(p) = existing {
        if p.state == "succeeded" {
            return Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "result": p.result, "replayed": true }))));
        }
        let wanted = p.intent["commit"].as_str().unwrap_or_default().to_owned();
        let remote = github.remote_branch(&target, &branch).await.map_err(github_fail)?;
        let mut tx = rt.tx(principal.account_id()).await?;
        let response = if remote.as_deref() == Some(wanted.as_str()) {
            let result = json!({ "branch": branch, "remote_commit": wanted, "repository": link.repository_full_name, "reconciled": true });
            publish::settle(&mut tx, p.id, "succeeded", &result).await?;
            Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "result": result, "replayed": true }))))
        } else {
            publish::settle(&mut tx, p.id, "failed", &json!({ "reconciled": true, "remote_commit": remote })).await?;
            Err(Fail::from(ApiError::conflict("push_not_applied", "That push never reached GitHub. Use a new idempotency key to push again.")))
        };
        tx.commit().await?;
        return response;
    }

    let intent = json!({ "branch": branch, "commit": local, "repository": link.repository_full_name });
    let pub_id = grant_and_intent(rt, &principal, &keys, body.grant_id, body.epoch, factory_broker::OperationClass::PushOwnedBranch, "push", &body.idempotency_key, &intent).await?;
    let pushed = github.push_branch(&target, worktree.work_dir(), &local, &branch).await;
    let mut tx = rt.tx(principal.account_id()).await?;
    let response = match pushed {
        Ok(remote) if remote == local => {
            let result = json!({ "branch": branch, "remote_commit": remote, "repository": link.repository_full_name });
            publish::settle(&mut tx, pub_id, "succeeded", &result).await?;
            factory_tasks::append_event(&mut tx, task.id, event(None, Actor::user(principal.account_id()), "pushed", "Branch pushed to GitHub", json!({ "publication_id": pub_id, "result": result })))
                .await
                .map_err(internal)?;
            Ok((StatusCode::CREATED, Json(json!({ "publication_id": pub_id, "result": result }))))
        }
        Ok(remote) => {
            publish::settle(&mut tx, pub_id, "unknown", &json!({ "remote_commit": remote, "expected": local })).await?;
            Err(Fail::from(ApiError::new(StatusCode::CONFLICT, "push_unconfirmed", "GitHub shows another commit on the branch. Reconcile before pushing again.", RetryClass::ReconcileFirst)))
        }
        Err(e) => {
            // The push may or may not have landed: leave the intent for reconciliation.
            publish::settle(&mut tx, pub_id, "unknown", &json!({ "error": e.code() })).await?;
            Err(github_fail(e))
        }
    };
    tx.commit().await?;
    rt.notify(task.id);
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PullBody {
    grant_id: Uuid,
    epoch: i64,
    idempotency_key: String,
    title: String,
    #[serde(default)]
    body: String,
}

async fn pull_request(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<PullBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::Publish).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let title = body.title.trim().to_owned();
    if !valid_key(&body.idempotency_key) || title.is_empty() || title.chars().count() > 200 || body.body.chars().count() > 20_000 {
        return Err(ApiError::invalid_request("A title of 1 to 200 characters, a body of up to 20,000 and an idempotency key.").into());
    }
    let keys = TaskKeys::of(&task, principal.account_id());
    let (link, allowlist) = link_for(&state, &principal, &project).await?;
    let installation = factory_intake::InstallationRef::new(link.installation_ref.clone()).map_err(github_fail)?;
    let target = factory_intake::PublishTarget {
        installation: &installation,
        repository: &link.repository_full_name,
        allowlist: &allowlist,
        default_branch: link.default_branch.as_deref(),
    };
    let github = state.intake.service.github();
    let branch = factory_workspace::task_branch_name(&ws_task(task.id));
    let as_result = |pr: &factory_intake::PullRequest| {
        json!({ "number": pr.number, "url": pr.html_url, "state": pr.state, "head": pr.head.branch, "head_commit": pr.head.sha, "base": pr.base.branch, "repository": link.repository_full_name })
    };

    let mut tx = rt.tx(principal.account_id()).await?;
    let existing = publish::find_publication(&mut tx, task.id, "pull_request", &body.idempotency_key).await?;
    tx.commit().await?;
    if let Some(p) = &existing
        && p.state == "succeeded"
    {
        return Ok((StatusCode::OK, Json(json!({ "publication_id": p.id, "result": p.result, "replayed": true }))));
    }
    // Before any create, and to reconcile a lost reply: a pull request from this branch may
    // already exist. Then it is the result, and nothing new is opened.
    let found = github.find_pulls(&target, &branch).await.map_err(github_fail)?;
    if let Some(pr) = found.iter().find(|p| p.state == "open").or(found.first()) {
        let result = as_result(pr);
        let mut tx = rt.tx(principal.account_id()).await?;
        let pub_id = match &existing {
            Some(p) => p.id,
            None => {
                let snapshot_intent = json!({ "branch": branch, "title": title, "existing": true });
                tx.commit().await?;
                let id = grant_and_intent(rt, &principal, &keys, body.grant_id, body.epoch, factory_broker::OperationClass::OpenPullRequest, "pull_request", &body.idempotency_key, &snapshot_intent).await?;
                tx = rt.tx(principal.account_id()).await?;
                id
            }
        };
        publish::settle(&mut tx, pub_id, "succeeded", &json!({ "pull_request": result, "reconciled": true })).await?;
        tx.commit().await?;
        return Ok((StatusCode::OK, Json(json!({ "publication_id": pub_id, "result": { "pull_request": result, "reconciled": true }, "replayed": true }))));
    }
    if github.remote_branch(&target, &branch).await.map_err(github_fail)?.is_none() {
        return Err(ApiError::conflict("branch_not_pushed", "Push the task branch before opening a pull request.").into());
    }
    let pub_id = match &existing {
        Some(p) => p.id,
        None => {
            let intent = json!({ "branch": branch, "title": title, "base": link.default_branch });
            grant_and_intent(rt, &principal, &keys, body.grant_id, body.epoch, factory_broker::OperationClass::OpenPullRequest, "pull_request", &body.idempotency_key, &intent).await?
        }
    };
    let opened = github.open_pull(&target, &branch, &title, &body.body).await;
    let mut tx = rt.tx(principal.account_id()).await?;
    let response = match opened {
        Ok(pr) => {
            let result = json!({ "pull_request": as_result(&pr) });
            publish::settle(&mut tx, pub_id, "succeeded", &result).await?;
            factory_tasks::append_event(&mut tx, task.id, event(None, Actor::user(principal.account_id()), "pull_request_opened", "Pull request opened", json!({ "publication_id": pub_id, "result": result })))
                .await
                .map_err(internal)?;
            Ok((StatusCode::CREATED, Json(json!({ "publication_id": pub_id, "result": result }))))
        }
        Err(e) => {
            publish::settle(&mut tx, pub_id, "unknown", &json!({ "error": e.code() })).await?;
            Err(github_fail(e))
        }
    };
    tx.commit().await?;
    rt.notify(task.id);
    response
}

// ---- working outside the factory (gate 6) ----

/// The committed state of the task branch as a ZIP, to keep editing the mod elsewhere.
async fn download(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<axum::response::Response, Fail> {
    use axum::response::IntoResponse;
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let (_, worktree) = open(&state, &project, &task).await?;
    let head = worktree.expected_head().clone();
    let repo = worktree.repo().clone();
    let bytes = tokio::task::spawn_blocking(move || repo.archive_zip(&head)).await.map_err(internal)?.map_err(internal)?;
    let name = format!("hoi4factory-{}-{}.zip", &task.id.to_string()[..8], &worktree.expected_head().as_str()[..12]);
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/zip".to_owned()),
            (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        bytes,
    )
        .into_response())
}

/// Takes in work pushed to the task branch on GitHub from outside the factory. A remote branch
/// that descends from the factory's branch is fast-forwarded and its changed files are listed;
/// one that diverged is reported and nothing moves.
async fn sync(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let (link, allowlist) = link_for(&state, &principal, &project).await?;
    let installation = factory_intake::InstallationRef::new(link.installation_ref.clone()).map_err(github_fail)?;
    let target = factory_intake::PublishTarget {
        installation: &installation,
        repository: &link.repository_full_name,
        allowlist: &allowlist,
        default_branch: link.default_branch.as_deref(),
    };
    let (_, worktree) = open(&state, &project, &task).await?;
    let branch = worktree.branch().to_owned();
    let local = worktree.expected_head().clone();
    let fetched = state.intake.service.github().fetch_branch(&target, worktree.work_dir(), &branch).await.map_err(github_fail)?;
    let Some(remote) = fetched else {
        return Ok(Json(json!({ "external_changes": false, "remote": null, "local": local.as_str() })));
    };
    let remote = Oid::new(remote).map_err(internal)?;
    if remote == local {
        return Ok(Json(json!({ "external_changes": false, "remote": remote.as_str(), "local": local.as_str() })));
    }
    let mut wt = worktree;
    let (l2, r2) = (local.clone(), remote.clone());
    let outcome = tokio::task::spawn_blocking(move || {
        let repo = wt.repo().clone();
        if repo.is_ancestor(&r2, &l2).map_err(|e| e.to_string())? {
            return Ok::<_, String>(("behind", Vec::new(), l2));
        }
        if !repo.is_ancestor(&l2, &r2).map_err(|e| e.to_string())? {
            return Ok(("diverged", Vec::new(), l2));
        }
        let changed = repo.changed_between(&l2, &r2).map_err(|e| e.to_string())?;
        let now = wt.fast_forward(&r2).map_err(|e| e.to_string())?;
        Ok(("fast_forward", changed, now))
    })
    .await
    .map_err(internal)?
    .map_err(|e| Fail::from(ApiError::conflict("sync_failed", "The outside changes could not be taken in. The worktree is unchanged.")).with_details(json!({ "reason": e })))?;
    match outcome {
        ("diverged", _, _) => Err(Fail::from(ApiError::conflict("diverged", "The branch on GitHub and the factory's branch both have new commits. Nothing was changed."))
            .with_details(json!({ "remote": remote.as_str(), "local": local.as_str() }))),
        ("behind", _, _) => Ok(Json(json!({ "external_changes": false, "remote": remote.as_str(), "local": local.as_str(), "remote_is_behind": true }))),
        (_, changed, now) => {
            let mut tx = rt.tx(principal.account_id()).await?;
            factory_tasks::append_event(
                &mut tx,
                task.id,
                event(None, Actor::user(principal.account_id()), "external_changes", "Changes made outside the factory were taken in", json!({ "from": local.as_str(), "to": now.as_str(), "changed_paths": changed }))
                    .with_snapshot(now.to_string()),
            )
            .await
            .map_err(internal)?;
            tx.commit().await?;
            rt.notify(task.id);
            Ok(Json(json!({ "external_changes": true, "from": local.as_str(), "to": now.as_str(), "changed_paths": changed })))
        }
    }
}

/// The import preservation report and baseline (gate 8, P4). `pending` while the background
/// check of a fresh import is still running.
async fn import_report(
    principal: Principal,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let report = publish::latest_report(&mut tx, project.id.0).await?;
    tx.commit().await?;
    Ok(Json(match report {
        Some(r) => json!({ "state": "recorded", "report": r }),
        None => json!({ "state": "pending", "report": null }),
    }))
}
