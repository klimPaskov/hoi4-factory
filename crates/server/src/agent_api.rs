//! Agent task routes (S03): tasks, attempts, pause, interrupt, revocation and the C-03 event
//! stream; and the account's model keys, usage and model profiles.
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `GET /model-profiles` | signed in | Profiles a task can use, their capabilities, and who can pay |
//! | `GET /account/api-keys`, `POST`, `DELETE .../{id}` | signed in | The caller's own model keys; the secret is never returned |
//! | `POST /account/api-keys/{id}/probe` | signed in | Connection and model listing for a saved key (no inference) |
//! | `GET /account/usage` | signed in | This month's operator allowance and own-key usage |
//! | `GET /projects/{id}/tasks`, `POST` | member; create needs `CreateTask` | List and create agent tasks |
//! | `GET /projects/{id}/tasks/{task}` | member | Task, attempts, grants and unresolved effects |
//! | `POST .../tasks/{task}/attempts` | `CreateTask`, verified email | Admit an attempt and start the agent |
//! | `POST .../tasks/{task}/pause`, `.../resume` | `CreateTask` | Durable pause (expected version) |
//! | `POST .../attempts/{attempt}/interrupt` | `CreateTask` | Request cancellation; the attempt reports how it stopped |
//! | `POST .../grants/{grant}/revoke` | grantor or owner | Revoke the attempt's grant (new epoch) |
//! | `GET .../tasks/{task}/events?after=N` | member | C-03 events after a cursor |
//! | `GET .../tasks/{task}/events/stream?after=N` | member | The same as server-sent events, resumable by `Last-Event-ID` |
//!
//! Mutating requests carry the task's expected version where they change it, so two people
//! acting at once get one success and one `409 version_conflict`.

use std::convert::Infallible;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use factory_accounts::{AccountId, ApiError, ApiJson, Permission, Principal, Project, RetryClass};
use factory_agent::probe::{ProbeMode, probe};
use factory_agent::profile::ModelProfile;
use factory_broker::{
    Funding as MeterFunding, GrantId, KeyProvider, Meter, NewKey, SecretString, UserKeyId,
    get_grant, month_start, revoke_grant,
};
use factory_tasks::{
    Actor, Attempt, AttemptId, AttemptState, NewTask, RunEvent, Task, TaskId, TaskMode, TaskState,
};
use futures_util::stream::{self, Stream};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::access::{active_project_for, project_for};
use crate::agent::runner::{self, AdmitRequest, ReplayBase, event, task_error, ws_task};
use crate::agent::{AgentRuntime, Control, profiles, recovery, workspace_tools as ws};
use crate::api_error::Fail;
use crate::intake_api::{accepted_origins, origin_guard};
use crate::state::ServerState;
use crate::workspace_api::open_workspace;

const MAX_OBJECTIVE_CHARS: usize = 4000;
const MAX_BUDGET_USD: f64 = 5.0;
const DEFAULT_BUDGET_MICROS: i64 = 200_000;
const DEFAULT_MAX_TURNS: i32 = 20;

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route("/model-profiles", get(model_profiles))
        .route("/account/api-keys", get(list_keys).post(add_key))
        .route("/account/api-keys/{key}", delete(delete_key))
        .route("/account/api-keys/{key}/probe", post(probe_key))
        .route("/account/usage", get(usage))
        .route("/projects/{id}/tasks", get(list_tasks).post(create_task))
        .route("/projects/{id}/tasks/{task}", get(task_detail))
        .route("/projects/{id}/tasks/{task}/attempts", post(start_attempt))
        .route("/projects/{id}/tasks/{task}/pause", post(pause))
        .route("/projects/{id}/tasks/{task}/resume", post(resume))
        .route("/projects/{id}/tasks/{task}/attempts/{attempt}/interrupt", post(interrupt))
        .route("/projects/{id}/tasks/{task}/grants/{grant}/revoke", post(revoke))
        .route("/projects/{id}/tasks/{task}/events", get(events))
        .route("/projects/{id}/tasks/{task}/events/stream", get(event_stream))
        .layer(middleware::from_fn_with_state(accepted, origin_guard))
        .with_state(state.clone())
}

fn runtime(state: &ServerState) -> Result<&AgentRuntime, Fail> {
    state.agent.as_ref().ok_or_else(|| {
        Fail::from(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent_unavailable",
            "The agent is not available on this server.",
            RetryClass::Terminal,
        ))
    })
}

fn conflict(code: &'static str, message: &str) -> Fail {
    Fail::from(ApiError::conflict(code, message))
}

fn parse<T: FromStr>(raw: &str) -> Result<T, Fail> {
    T::from_str(raw).map_err(|_| Fail::from(ApiError::not_found()))
}

// ---- model profiles, keys, usage ----

async fn model_profiles(principal: Principal, State(state): State<ServerState>) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let _ = principal;
    let profiles: Vec<Value> = rt
        .inner
        .operator_keys
        .profiles()
        .into_iter()
        .map(|p| {
            let (caps, evidence) = profiles::capability_summary(&p.starting_capabilities());
            json!({
                "id": p.id,
                "label": profiles::label(&p),
                "provider": profiles::provider_name(p.provider),
                "model": p.model,
                "default": p.id == profiles::DEFAULT_PROFILE,
                "operator_allowance": rt.inner.operator_keys.for_provider(p.provider).is_some(),
                "capabilities": caps,
                "capability_evidence": evidence,
            })
        })
        .collect();
    Ok(Json(json!({ "profiles": profiles, "user_keys_available": rt.keys_available() })))
}

fn key_json(k: &factory_broker::KeyRecord) -> Value {
    json!({
        "id": k.id,
        "provider": k.provider.as_str(),
        "label": k.label,
        "base_url": k.base_url,
        "key_hint": k.key_hint,
        "created_at": k.created_at,
        "last_used_at": k.last_used_at,
    })
}

fn keys_unavailable() -> Fail {
    Fail::from(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "keys_unavailable",
        "This server cannot save model keys (no key encryption is configured). The free allowance still works.",
        RetryClass::Terminal,
    ))
}

async fn list_keys(principal: Principal, State(state): State<ServerState>) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let Some(keys) = rt.inner.keys.as_ref() else {
        return Ok(Json(json!({ "available": false, "keys": [] })));
    };
    let mut tx = rt.tx(principal.account_id()).await?;
    let list = keys.list(&mut tx, principal.account_id()).await.map_err(key_error)?;
    tx.commit().await?;
    let active: Vec<Value> = list.iter().filter(|k| k.revoked_at.is_none()).map(key_json).collect();
    Ok(Json(json!({ "available": true, "keys": active })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddKey {
    provider: String,
    label: String,
    api_key: String,
    #[serde(default)]
    base_url: Option<String>,
}

fn key_error(err: factory_broker::KeyError) -> Fail {
    match err {
        factory_broker::KeyError::NotFound => ApiError::not_found().into(),
        factory_broker::KeyError::Invalid(why) => ApiError::invalid_request(why).into(),
        other => {
            tracing::error!(error = %other, "key store failure");
            ApiError::internal().into()
        }
    }
}

fn destination_blocked(detail: &str) -> Fail {
    Fail::from(ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "destination_blocked",
        format!("That server address is not allowed: {detail}"),
        RetryClass::Terminal,
    ))
}

/// Checks a custom endpoint's URL and every address its name resolves to (RT-11, EXT-14).
async fn check_endpoint(rt: &AgentRuntime, raw: &str) -> Result<String, Fail> {
    let url = rt.policy().check_url(raw).map_err(|r| destination_blocked(&r.to_string()))?;
    if let Some(host) = url.host_str()
        && url.host().is_some_and(|h| matches!(h, url::Host::Domain(_)))
    {
        factory_agent::destination::PolicyResolver::new(rt.policy().clone())
            .resolve_host(host)
            .await
            .map_err(|r| destination_blocked(&r.to_string()))?;
    }
    Ok(raw.trim_end_matches('/').to_owned())
}

async fn add_key(
    principal: Principal,
    State(state): State<ServerState>,
    ApiJson(body): ApiJson<AddKey>,
) -> Result<Response, Fail> {
    let rt = runtime(&state)?;
    let Some(keys) = rt.inner.keys.as_ref() else {
        return Err(keys_unavailable());
    };
    let provider = KeyProvider::parse(&body.provider)
        .ok_or_else(|| Fail::from(ApiError::invalid_request("Unknown provider.").with_id("field", "provider")))?;
    let label = body.label.trim();
    if label.is_empty() || label.chars().count() > 100 {
        return Err(ApiError::invalid_request("Give the key a name of up to 100 characters.").with_id("field", "label").into());
    }
    let secret = body.api_key.trim();
    if secret.is_empty() || secret.len() > 512 || secret.chars().any(char::is_whitespace) {
        return Err(ApiError::invalid_request("Paste the whole API key.").with_id("field", "api_key").into());
    }
    let base_url = match (provider, body.base_url.as_deref().map(str::trim).filter(|s| !s.is_empty())) {
        (KeyProvider::OpenaiCompatible, Some(raw)) => Some(check_endpoint(rt, raw).await?),
        (KeyProvider::OpenaiCompatible, None) => {
            return Err(ApiError::invalid_request("A self-hosted endpoint needs its server address.").with_id("field", "base_url").into());
        }
        (_, _) => None,
    };
    let mut tx = rt.tx(principal.account_id()).await?;
    let record = keys
        .save(
            &mut tx,
            principal.account_id(),
            NewKey { provider, label: label.to_owned(), base_url, plaintext: SecretString::new(secret) },
        )
        .await
        .map_err(key_error)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(key_json(&record))).into_response())
}

async fn delete_key(
    principal: Principal,
    State(state): State<ServerState>,
    Path(key): Path<String>,
) -> Result<StatusCode, Fail> {
    let rt = runtime(&state)?;
    let Some(keys) = rt.inner.keys.as_ref() else {
        return Err(keys_unavailable());
    };
    let id = UserKeyId(parse(&key)?);
    let mut tx = rt.tx(principal.account_id()).await?;
    keys.delete(&mut tx, principal.account_id(), id).await.map_err(key_error)?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The profile a saved key is probed with.
fn profile_for_key(record: &factory_broker::KeyRecord) -> ModelProfile {
    match record.provider {
        KeyProvider::Anthropic => ModelProfile::anthropic_haiku(),
        KeyProvider::Openai => ModelProfile::openai_nano(),
        KeyProvider::OpenaiCompatible => ModelProfile::openai_compatible(record.base_url.as_deref().unwrap_or(""), "listing"),
    }
}

async fn probe_key(
    principal: Principal,
    State(state): State<ServerState>,
    Path(key): Path<String>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let Some(keys) = rt.inner.keys.as_ref() else {
        return Err(keys_unavailable());
    };
    let id = UserKeyId(parse(&key)?);
    let mut tx = rt.tx(principal.account_id()).await?;
    let record = keys
        .list(&mut tx, principal.account_id())
        .await
        .map_err(key_error)?
        .into_iter()
        .find(|k| k.id == id && k.revoked_at.is_none())
        .ok_or_else(ApiError::not_found)?;
    let secret = keys.open_for_owner(&mut tx, principal.account_id(), id).await.map_err(key_error)?;
    tx.commit().await?;
    let profile = profile_for_key(&record);
    // Listing only: connection, model list and the destination policy. Never inference.
    let report = probe(
        &profile,
        factory_agent::openai::ApiKey::new(secret.expose().to_owned()),
        rt.policy(),
        ProbeMode::Listing,
    )
    .await;
    let (caps, evidence) = profiles::capability_summary(&report.capabilities);
    Ok(Json(json!({
        "profile_id": report.profile_id,
        "provider": record.provider.as_str(),
        "connection": report.connection,
        "models": report.models,
        "capabilities": caps,
        "capability_evidence": evidence,
        "refusal": report.refusal,
        "error": report.error,
        "notes": report.notes,
        "mode": "listing",
        "probed_at": report.probed_at,
    })))
}

async fn usage(principal: Principal, State(state): State<ServerState>) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let now = rt.inner.clock.now();
    let month = month_start(now);
    let mut tx = rt.tx(principal.account_id()).await?;
    let summary = Meter::new()
        .account_month(&mut tx, principal.account_id(), month, rt.allowance_micros())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "usage query failed");
            Fail::from(ApiError::internal())
        })?;
    tx.commit().await?;
    Ok(Json(json!({
        "month": month.format("%Y-%m").to_string(),
        "verified": principal.account.email_verified,
        "allowance_micros": summary.allowance_micros,
        "operator_spent_micros": summary.operator_spent_micros,
        "remaining_allowance_micros": summary.remaining_allowance_micros,
        "unknown_count": summary.operator_unknown_count,
        "user_key_spent_micros": summary.user_key_spent_micros,
    })))
}

// ---- tasks ----

async fn task_json(rt: &AgentRuntime, account: AccountId, task: &Task) -> Value {
    let usage = match rt.tx(account).await {
        Ok(mut tx) => Meter::new().task_usage(&mut tx, task.id.0).await.ok(),
        Err(_) => None,
    };
    json!({
        "id": task.id,
        "project_id": task.project_id,
        "objective": task.objective,
        "mode": task.mode,
        "allowed_paths": task.allowed_paths,
        "state": task.state,
        "paused": task.paused,
        "pause_reason": task.pause_reason,
        "budget_micros": task.budget_micros,
        "max_turns": task.max_turns,
        "model": task.model_choice,
        "version": task.version,
        "last_seq": task.last_seq,
        "spent_micros": usage.as_ref().map(|u| u.spent_micros).unwrap_or(0),
        "spend_unknown": usage.as_ref().map(|u| u.unknown_count > 0).unwrap_or(false),
        "created_at": task.created_at,
        "updated_at": task.updated_at,
    })
}

async fn attempt_json(conn: &mut sqlx::PgConnection, attempt: &Attempt) -> Value {
    let grant = get_grant(conn, GrantId(attempt.grant_id)).await.ok().flatten();
    let manifest: Option<String> = sqlx::query_scalar("SELECT manifest_sha256 FROM context_bundles WHERE id = $1")
        .bind(attempt.bundle_id.0)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten();
    json!({
        "id": attempt.id,
        "attempt_no": attempt.attempt_no,
        "state": attempt.state,
        "grant": grant.map(|g| json!({
            "id": g.id,
            "revocation_epoch": g.revocation_epoch,
            "revoked": g.revoked_at.is_some(),
            "operation_classes": g.operation_classes.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "allowed_roots": g.allowed_roots,
            "provider_destinations": g.provider_destinations,
            "max_cost_micros": g.max_cost_micros,
            "expires_at": g.expires_at,
        })),
        "bundle": { "id": attempt.bundle_id, "manifest_sha256": manifest },
        "fencing_generation": attempt.fencing_generation,
        "funding": attempt.funding,
        "error_code": attempt.error_code,
        "outcome": attempt.outcome,
        "admitted_at": attempt.admitted_at,
        "started_at": attempt.started_at,
        "finished_at": attempt.finished_at,
        "version": attempt.version,
    })
}

async fn list_tasks(
    principal: Principal,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let tasks = factory_tasks::list_tasks(&mut tx, project.id).await.map_err(task_error)?;
    tx.commit().await?;
    let mut out = Vec::new();
    for t in &tasks {
        out.push(task_json(rt, principal.account_id(), t).await);
    }
    Ok(Json(json!({ "tasks": out })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelChoice {
    profile_id: String,
    #[serde(default)]
    key_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTask {
    objective: String,
    mode: String,
    #[serde(default)]
    allowed_paths: Vec<String>,
    #[serde(default)]
    budget_usd: Option<f64>,
    #[serde(default)]
    model: Option<ModelChoice>,
}

/// The profile and funding a model choice names, for `account`.
async fn resolve_choice(
    rt: &AgentRuntime,
    account: AccountId,
    choice: &Value,
) -> Result<(ModelProfile, MeterFunding), Fail> {
    let profile_id = choice["profile_id"].as_str().unwrap_or(profiles::DEFAULT_PROFILE);
    let profile = rt.inner.operator_keys.find(profile_id)
        .ok_or_else(|| Fail::from(ApiError::invalid_request("Unknown model profile.").with_id("field", "model")))?;
    match choice["key_id"].as_str() {
        Some(raw) => {
            let Some(keys) = rt.inner.keys.as_ref() else {
                return Err(keys_unavailable());
            };
            let id = UserKeyId(uuid::Uuid::parse_str(raw).map_err(|_| Fail::from(ApiError::invalid_request("Unknown key.")))?);
            let mut tx = rt.tx(account).await?;
            let found = keys
                .list(&mut tx, account)
                .await
                .map_err(key_error)?
                .into_iter()
                .find(|k| k.id == id && k.revoked_at.is_none());
            tx.commit().await?;
            let record = found.ok_or_else(|| {
                Fail::from(ApiError::new(
                    StatusCode::CONFLICT,
                    "provider_key_unavailable",
                    "That saved key is not yours or was deleted.",
                    RetryClass::RefreshPreconditions,
                ))
            })?;
            if record.provider != profiles::key_provider(profile.provider) {
                return Err(ApiError::invalid_request("That key is for a different provider.").with_id("field", "model").into());
            }
            Ok((profile, MeterFunding::UserKey { key_id: id }))
        }
        None => {
            if rt.inner.operator_keys.for_provider(profile.provider).is_none() {
                return Err(Fail::from(ApiError::new(
                    StatusCode::CONFLICT,
                    "provider_unavailable",
                    "The free allowance does not cover this model on this server. Use your own key.",
                    RetryClass::Terminal,
                )));
            }
            Ok((profile, MeterFunding::OperatorAllowance))
        }
    }
}

async fn create_task(
    principal: Principal,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<CreateTask>,
) -> Result<Response, Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let objective = body.objective.trim().to_owned();
    if objective.is_empty() || objective.chars().count() > MAX_OBJECTIVE_CHARS {
        return Err(ApiError::invalid_request("Describe the task in up to 4000 characters.").with_id("field", "objective").into());
    }
    let mode = match body.mode.as_str() {
        "read_only" => TaskMode::ReadOnly,
        "edit" => TaskMode::Edit,
        _ => return Err(ApiError::invalid_request("Mode is read_only or edit.").with_id("field", "mode").into()),
    };
    let allowed: Vec<String> = body.allowed_paths.iter().map(|p| p.trim().trim_end_matches('/').to_owned()).filter(|p| !p.is_empty()).collect();
    if mode == TaskMode::Edit && allowed.is_empty() {
        return Err(ApiError::invalid_request("An edit task needs at least one allowed path.").with_id("field", "allowed_paths").into());
    }
    let allowed = if mode == TaskMode::Edit { allowed } else { Vec::new() };
    for p in &allowed {
        factory_tasks::tasks::validate_allowed_path(p).map_err(|e| Fail::from(ApiError::invalid_request(e.to_string()).with_id("field", "allowed_paths")))?;
        factory_workspace::validate_relative(p).map_err(|e| Fail::from(ApiError::invalid_request(format!("{p}: {e}")).with_id("field", "allowed_paths")))?;
    }
    let budget = match body.budget_usd {
        None => DEFAULT_BUDGET_MICROS,
        Some(usd) if usd.is_finite() && usd > 0.0 && usd <= MAX_BUDGET_USD => (usd * 1_000_000.0).round() as i64,
        Some(_) => return Err(ApiError::invalid_request("The budget is between $0.01 and $5.").with_id("field", "budget_usd").into()),
    };
    let choice = match &body.model {
        Some(m) => json!({ "profile_id": m.profile_id, "key_id": m.key_id }),
        None => json!({ "profile_id": profiles::DEFAULT_PROFILE, "key_id": null }),
    };
    let (profile, funding) = resolve_choice(rt, principal.account_id(), &choice).await?;
    let model_choice = json!({
        "profile_id": profile.id,
        "funding": match funding { MeterFunding::OperatorAllowance => "operator_allowance", MeterFunding::UserKey { .. } => "user_key" },
        "key_id": runner::funding_key(&funding),
    });
    let mut tx = rt.tx(principal.account_id()).await?;
    let task = factory_tasks::create_task(
        &mut tx,
        NewTask {
            project_id: project.id,
            owner_account_id: project.owner_account_id,
            created_by: principal.account_id(),
            objective,
            requirement_ids: vec![],
            mode,
            allowed_paths: allowed,
            budget_micros: budget,
            max_turns: DEFAULT_MAX_TURNS,
            model_choice,
        },
    )
    .await
    .map_err(task_error)?;
    let task = factory_tasks::transition_task(
        &mut tx,
        task.id,
        task.version,
        TaskState::Ready,
        event(None, Actor::user(principal.account_id()), "task_ready", "Task created and ready", json!({})),
    )
    .await
    .map_err(task_error)?;
    tx.commit().await?;
    rt.notify(task.id);
    let body = task_json(rt, principal.account_id(), &task).await;
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

async fn load_task(rt: &AgentRuntime, principal: &Principal, project: &Project, raw: &str) -> Result<Task, Fail> {
    let id = TaskId(parse(raw)?);
    let mut tx = rt.tx(principal.account_id()).await?;
    let task = factory_tasks::get_task(&mut tx, id).await.map_err(task_error)?;
    tx.commit().await?;
    if task.project_id != project.id {
        return Err(ApiError::not_found().into());
    }
    Ok(task)
}

async fn task_detail(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    // Task, attempts and effects from one snapshot, so the pair is always consistent.
    let mut tx = rt.snapshot(principal.account_id()).await?;
    let task = factory_tasks::get_task(&mut tx, task.id).await.map_err(task_error)?;
    let attempts = factory_tasks::list_attempts(&mut tx, task.id).await.map_err(task_error)?;
    let mut list = Vec::new();
    for a in &attempts {
        list.push(attempt_json(&mut tx, a).await);
    }
    let pending = factory_tasks::pending_for_task(&mut tx, task.id).await.map_err(task_error)?;
    tx.commit().await?;
    let pending: Vec<Value> = pending
        .iter()
        .map(|(i, r)| json!({ "operation_id": i.operation_id, "tool": i.replay.tool, "status": r.status }))
        .collect();
    Ok(Json(json!({
        "task": task_json(rt, principal.account_id(), &task).await,
        "attempts": list,
        "pending_effects": pending,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    expected_version: i64,
    #[serde(default)]
    reason: Option<String>,
}

/// Opens (or reopens) the task's own worktree on its owned branch.
async fn ensure_worktree(state: &ServerState, project: &Project, task: TaskId) -> Result<(PathBuf, String), Fail> {
    let workspace = open_workspace(state, project)?;
    tokio::task::spawn_blocking(move || {
        let not_ready = || {
            Fail::from(ApiError::conflict(
                "workspace_not_ready",
                "The project's mod has no Git baseline yet. Finish bringing the mod in first.",
            ))
        };
        let repo = workspace.git().map_err(|_| not_ready())?.ok_or_else(not_ready)?;
        let id = ws_task(task);
        let dir = workspace.task_dir(&id);
        let worktree = if dir.exists() {
            repo.open_task_worktree(&id, &dir, None)
        } else {
            let head = repo.head().map_err(|_| not_ready())?.ok_or_else(not_ready)?;
            std::fs::create_dir_all(workspace.tasks_dir()).map_err(|_| Fail::from(ApiError::internal()))?;
            repo.add_task_worktree(&id, &dir, &head)
        }
        .map_err(|e| {
            tracing::error!(error = %e, "could not open the task worktree");
            Fail::from(ApiError::internal())
        })?;
        Ok((worktree.work_dir().to_path_buf(), worktree.expected_head().to_string()))
    })
    .await
    .map_err(|_| Fail::from(ApiError::internal()))?
}

/// The files of the worktree as C-06 locators: paths, hashes and sizes, no content. Files under
/// the allowed paths come first; `descriptor.mod` comes with its content.
fn locators(root: &std::path::Path, allowed: &[String]) -> Vec<factory_tasks::context::SourceLocator> {
    let mut out = Vec::new();
    let Ok(listing) = ws::list_files(root, "", true) else {
        return out;
    };
    let mut files: Vec<(String, u64)> = listing["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["kind"] == "file")
        .filter_map(|e| Some((e["path"].as_str()?.to_owned(), e["bytes"].as_u64()?)))
        .collect();
    let in_scope = |p: &str| allowed.iter().any(|a| p == a || p.starts_with(&format!("{a}/")));
    files.sort_by_key(|(p, _)| (!in_scope(p), p.clone()));
    for (path, bytes) in files.into_iter().take(runner::MAX_BUNDLE_FILES) {
        let Ok(hash) = ws::observe(root, &path) else { continue };
        let descriptor = path == "descriptor.mod" && bytes < 16 * 1024;
        let content = if descriptor { std::fs::read_to_string(root.join(&path)).ok() } else { None };
        let priority = if in_scope(&path) { 10 } else { 1 };
        out.push(factory_tasks::context::SourceLocator {
            reason: if descriptor { "mod descriptor".into() } else if priority == 10 { "under an allowed path".into() } else { "file in the mod".into() },
            required: descriptor && content.is_some(),
            region: None,
            sha256: hash,
            bytes,
            priority,
            content,
            path,
        });
    }
    out
}

async fn start_attempt(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<Expected>,
) -> Result<Response, Fail> {
    let rt = runtime(&state)?;
    principal.require_verified()?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let role = project.role.ok_or_else(ApiError::not_found)?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if task.version != body.expected_version {
        return Err(task_error(factory_tasks::TaskError::VersionConflict { current: task.version }).into());
    }
    if task.paused {
        return Err(task_error(factory_tasks::TaskError::Paused).into());
    }
    let (profile, funding) = resolve_choice(rt, principal.account_id(), &task.model_choice).await?;
    if let Err(missing) = profile.starting_capabilities().require(profiles::REQUIRED) {
        return Err(Fail::from(ApiError::new(
            StatusCode::CONFLICT,
            "capability_unsatisfied",
            format!("This model profile has no evidence for {:?}.", missing.missing),
            RetryClass::Terminal,
        )));
    }
    let (worktree, head) = ensure_worktree(&state, &project, task.id).await?;
    let allowed = task.allowed_paths.clone();
    let root = worktree.clone();
    let locs = tokio::task::spawn_blocking(move || locators(&root, &allowed))
        .await
        .map_err(|_| Fail::from(ApiError::internal()))?;
    let replay = ReplayBase {
        workspace_id: format!("workspace:{}", project.id),
        root_identity: format!("workspace:{}/tasks/{}", project.id, task.id),
        source_snapshot: head,
    };
    let tool_state = recovery::tool_state_dir(rt, &task);
    let (task, attempt) = runner::admit(
        rt,
        AdmitRequest {
            account: principal.account_id(),
            role,
            verified: principal.account.email_verified,
            expected_version: body.expected_version,
            task,
            profile,
            funding,
            worktree,
            tool_state,
            replay,
            locators: locs,
        },
    )
    .await?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let attempt = attempt_json(&mut tx, &attempt).await;
    tx.commit().await?;
    let task = task_json(rt, principal.account_id(), &task).await;
    Ok((StatusCode::CREATED, Json(json!({ "task": task, "attempt": attempt }))).into_response())
}

async fn active_attempt(rt: &AgentRuntime, account: AccountId, task: TaskId) -> Result<Option<Attempt>, Fail> {
    let mut tx = rt.tx(account).await?;
    let attempts = factory_tasks::list_attempts(&mut tx, task).await.map_err(task_error)?;
    tx.commit().await?;
    Ok(attempts.into_iter().find(|a| !a.state.is_terminal()))
}

async fn pause(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<Expected>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let reason = body.reason.as_deref().map(|r| r.chars().take(500).collect::<String>());
    let mut tx = rt.tx(principal.account_id()).await?;
    let task = factory_tasks::set_paused(&mut tx, task.id, body.expected_version, true, reason.as_deref(), Actor::user(principal.account_id()))
        .await
        .map_err(task_error)?;
    tx.commit().await?;
    if let Some(a) = active_attempt(rt, principal.account_id(), task.id).await? {
        rt.signal(a.id, Control::Pause);
    }
    rt.notify(task.id);
    Ok(Json(task_json(rt, principal.account_id(), &task).await))
}

async fn resume(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    ApiJson(body): ApiJson<Expected>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if task.version != body.expected_version {
        return Err(task_error(factory_tasks::TaskError::VersionConflict { current: task.version }).into());
    }
    // A suspended attempt continues from its saved conversation once its lease is retaken; a
    // busy workspace is refused before the pause is cleared.
    let active = active_attempt(rt, principal.account_id(), task.id).await?;
    let resumable = match &active {
        Some(a) if !rt.is_running(a.id) && matches!(a.state, AttemptState::Running | AttemptState::Admitted) => {
            Some(recovery::prepare_resume(rt, principal.account_id(), &task, a).await?)
        }
        _ => None,
    };
    let mut tx = rt.tx(principal.account_id()).await?;
    let task = factory_tasks::set_paused(&mut tx, task.id, body.expected_version, false, None, Actor::user(principal.account_id()))
        .await
        .map_err(task_error)?;
    tx.commit().await?;
    match (active, resumable) {
        (_, Some(r)) => recovery::start_resumed(rt, r),
        (Some(a), None) if rt.is_running(a.id) => {
            rt.signal(a.id, Control::Run);
        }
        _ => {}
    }
    rt.notify(task.id);
    Ok(Json(task_json(rt, principal.account_id(), &task).await))
}

async fn interrupt(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task, attempt)): Path<(String, String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let attempt_id = AttemptId(parse(&attempt)?);
    let mut tx = rt.tx(principal.account_id()).await?;
    let current = factory_tasks::get_attempt(&mut tx, attempt_id).await.map_err(task_error)?;
    if current.task_id != task.id {
        return Err(ApiError::not_found().into());
    }
    let actor = Actor::user(principal.account_id());
    let updated = match current.state {
        AttemptState::Running | AttemptState::Admitted => {
            let to = if current.state == AttemptState::Admitted { AttemptState::Cancelled } else { AttemptState::InterruptRequested };
            factory_tasks::transition_attempt(&mut tx, attempt_id, current.version, to, None, None, actor)
                .await
                .map_err(task_error)?
        }
        AttemptState::InterruptRequested => current,
        _ => return Err(conflict("attempt_not_active", "This attempt is not running.")),
    };
    tx.commit().await?;
    // Effects are already refused at the database (the attempt is no longer running); now stop
    // the turn in flight. Without a live runner (a suspended attempt) the stop is immediate.
    let updated = if rt.signal(attempt_id, Control::Interrupt) || updated.state.is_terminal() {
        updated
    } else {
        let mut tx = rt.tx(principal.account_id()).await?;
        let a = factory_tasks::transition_attempt(&mut tx, attempt_id, updated.version, AttemptState::Cancelled, Some(json!({ "cancel": { "stop": "not_running" } })), Some("cancelled"), actor)
            .await
            .map_err(task_error)?;
        let t = factory_tasks::get_task(&mut tx, task.id).await.map_err(task_error)?;
        if t.state == TaskState::Running {
            factory_tasks::transition_task(&mut tx, task.id, t.version, TaskState::Ready, event(Some(attempt_id), actor, "task_ready", "The attempt was cancelled", json!({})))
                .await
                .map_err(task_error)?;
        }
        tx.commit().await?;
        a
    };
    rt.notify(task.id);
    let mut tx = rt.tx(principal.account_id()).await?;
    let body = attempt_json(&mut tx, &updated).await;
    tx.commit().await?;
    Ok(Json(body))
}

async fn revoke(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task, grant)): Path<(String, String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let grant_id = GrantId(parse(&grant)?);
    let now = rt.inner.clock.now();
    let mut tx = rt.tx(principal.account_id()).await?;
    let existing = get_grant(&mut tx, grant_id).await.map_err(|_| Fail::from(ApiError::internal()))?.ok_or_else(ApiError::not_found)?;
    if existing.task_id != task.id.0 {
        return Err(ApiError::not_found().into());
    }
    let revoked = revoke_grant(&mut tx, grant_id, principal.account_id(), now).await.map_err(|e| match e {
        factory_broker::GrantError::NotPermitted => Fail::from(ApiError::forbidden("Only the person who started the attempt or the project owner can revoke it.")),
        factory_broker::GrantError::NotFound => ApiError::not_found().into(),
        other => {
            tracing::error!(error = %other, "revoke failed");
            ApiError::internal().into()
        }
    })?;
    factory_tasks::append_event(
        &mut tx,
        task.id,
        event(None, Actor::user(principal.account_id()), "grant_revoked", "The attempt's permissions were revoked", json!({ "grant_id": grant_id, "revocation_epoch": revoked.revocation_epoch })),
    )
    .await
    .map_err(task_error)?;
    tx.commit().await?;
    rt.notify(task.id);
    Ok(Json(json!({ "id": revoked.id, "revocation_epoch": revoked.revocation_epoch, "revoked": revoked.revoked_at.is_some() })))
}

// ---- events ----

fn event_json(e: &RunEvent) -> Value {
    json!({
        "seq": e.seq,
        "event_id": e.event_id,
        "attempt_id": e.attempt_id,
        "type": e.event_type,
        "actor": e.actor,
        "summary": e.payload.get("summary").and_then(Value::as_str).unwrap_or(&e.event_type),
        "payload": e.payload,
        "payload_sha256": e.payload_sha256,
        "created_at": e.created_at,
    })
}

#[derive(Deserialize)]
struct After {
    #[serde(default)]
    after: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
}

async fn events(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    Query(q): Query<After>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let mut tx = rt.tx(principal.account_id()).await?;
    let list = factory_tasks::events_after(&mut tx, task.id, q.after.unwrap_or(0).max(0), limit).await.map_err(task_error)?;
    let current = factory_tasks::get_task(&mut tx, task.id).await.map_err(task_error)?;
    tx.commit().await?;
    Ok(Json(json!({ "events": list.iter().map(event_json).collect::<Vec<_>>(), "last_seq": current.last_seq })))
}

async fn event_stream(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
    Query(q): Query<After>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Fail> {
    let rt = runtime(&state)?.clone();
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(&rt, &principal, &project, &task).await?;
    let resume_from = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());
    let cursor = resume_from.or(q.after).unwrap_or(0).max(0);
    let account = principal.account_id();
    let receiver = rt.subscribe();
    // The database is the authority: every message is read back by cursor, so a missed wake-up
    // can delay an event but never skip or repeat one.
    let stream = stream::unfold(
        (rt, account, task.id, cursor, receiver, Vec::<RunEvent>::new()),
        |(rt, account, task, mut cursor, mut receiver, mut queue)| async move {
            loop {
                if let Some(e) = (!queue.is_empty()).then(|| queue.remove(0)) {
                    cursor = e.seq;
                    let message = Event::default()
                        .id(e.seq.to_string())
                        .event("run_event")
                        .data(event_json(&e).to_string());
                    return Some((Ok(message), (rt, account, task, cursor, receiver, queue)));
                }
                if let Ok(mut tx) = rt.tx(account).await
                    && let Ok(list) = factory_tasks::events_after(&mut tx, task, cursor, 100).await
                {
                    let _ = tx.commit().await;
                    if !list.is_empty() {
                        queue = list;
                        continue;
                    }
                }
                let wait = async {
                    loop {
                        match receiver.recv().await {
                            Ok(t) if t == task => break,
                            Ok(_) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                            Err(_) => std::future::pending::<()>().await,
                        }
                    }
                };
                let _ = tokio::time::timeout(Duration::from_secs(15), wait).await;
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keepalive")))
}

