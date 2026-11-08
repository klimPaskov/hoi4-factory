//! Focus editor routes (S04-W1, W2, W4).
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `GET /projects/{id}/tasks/{task}/focus/trees/{tree}` | member | The tree's projection from `hoi4.focus_inspect` in the task's worktree |
//! | `POST .../focus/trees/{tree}/operations` | `CreateTask`, edit task | Apply focus operations as one C-02 proposal, with the person as actor |
//! | `GET /projects/{id}/tasks/{task}/proposals` | member | The task's change proposals, oldest first |
//! | `GET .../proposals/{proposal}` | member | One proposal: summary, diffs, hashes and checks |
//!
//! An editor session is an ordinary edit task, so a person's gestures, the agent's edits and a
//! later commit share one worktree, one event stream and one proposal list.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use factory_accounts::{ApiError, ApiJson, Permission, Principal, RetryClass};
use factory_tasks::{Task, TaskMode, TaskState};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::access::{active_project_for, project_for};
use crate::agent_api::{ensure_worktree, load_task, runtime};
use crate::api_error::Fail;
use crate::focus::apply::{apply_operations, inspect_tree};
use crate::focus::ops::{FocusOp, MAX_OPS};
use crate::focus::session::{FocusError, localisation_path};
use crate::focus::store::{self, ActorKind};
use crate::intake_api::{accepted_origins, origin_guard};
use crate::state::ServerState;

pub const LIMIT_FOCUS_OPERATION: &str = "focus_operation";

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route("/projects/{id}/tasks/{task}/focus/trees/{tree}", get(projection))
        .route("/projects/{id}/tasks/{task}/focus/trees/{tree}/operations", post(operations))
        .route("/projects/{id}/tasks/{task}/proposals", get(proposals))
        .route("/projects/{id}/tasks/{task}/proposals/{proposal}", get(proposal))
        .layer(middleware::from_fn_with_state(accepted, origin_guard))
        .with_state(state.clone())
}

fn valid_tree_id(tree: &str) -> bool {
    !tree.is_empty() && tree.len() <= 256 && tree.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// The HTTP answer for a focus error. Every refusal says whether anything was written.
pub fn focus_fail(e: FocusError) -> Fail {
    let (status, code, retry, message): (StatusCode, &'static str, RetryClass, String) = match &e {
        FocusError::ToolsUnavailable { message } => (StatusCode::SERVICE_UNAVAILABLE, "tools_unavailable", RetryClass::SafeSameInputs, format!("HOI4 Agent Tools are unavailable: {message}")),
        FocusError::TreeNotFound { tree_id } => (StatusCode::NOT_FOUND, "tree_not_found", RetryClass::Terminal, format!("No focus tree {tree_id} in this mod.")),
        FocusError::NotInMod { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "not_in_mod", RetryClass::Terminal, "This tree comes from the game or a dependency, not this mod.".into()),
        FocusError::StaleBase { .. } => (StatusCode::CONFLICT, "stale_base", RetryClass::RefreshPreconditions, "The tree changed since you looked at it. Nothing was written. Reload it and try again.".into()),
        FocusError::OperationRefused { error } => (StatusCode::UNPROCESSABLE_ENTITY, "operation_refused", RetryClass::Terminal, format!("{} Nothing was written.", error.message)),
        FocusError::ToolRefused { message, .. } => (StatusCode::UNPROCESSABLE_ENTITY, "tool_refused", RetryClass::Terminal, format!("The focus tool refused the change: {message} Nothing was written.")),
        FocusError::UnknownEffect { message } => (StatusCode::INTERNAL_SERVER_ERROR, "unknown_effect", RetryClass::ReconcileFirst, format!("The change may have been written: {message}")),
        FocusError::UnexpectedWrite { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "unexpected_write", RetryClass::ReconcileFirst, "The focus tool wrote outside the tree's file. Review the worktree.".into()),
        FocusError::Internal { message } => {
            tracing::error!(error = %message, "focus editor failure");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal", RetryClass::SafeSameInputs, "Something went wrong on the server.".into())
        }
    };
    Fail::from(ApiError::new(status, code, message, retry)).with_details(serde_json::to_value(&e).unwrap_or(Value::Null))
}

/// Whether the task may write the paths a focus edit of `relative_path` and `tree_id` touches.
pub fn edit_allowed(task: &Task, relative_path: &str, tree_id: &str) -> Result<(), Fail> {
    if task.mode != TaskMode::Edit {
        return Err(ApiError::conflict("read_only_task", "This task is read-only. Start an edit task to change the tree.").into());
    }
    if matches!(task.state, TaskState::Completed | TaskState::Cancelled) {
        return Err(ApiError::conflict("task_closed", "This task is closed.").into());
    }
    let covered = |path: &str| {
        task.allowed_paths
            .iter()
            .any(|a| path == a || path.starts_with(&format!("{}/", a.trim_end_matches('/'))))
    };
    for path in [relative_path.to_owned(), localisation_path(tree_id)] {
        if !covered(&path) {
            return Err(Fail::from(ApiError::forbidden(format!("The task may not write {path}."))).with_id("path", path));
        }
    }
    Ok(())
}

async fn projection(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task, tree)): Path<(String, String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if !valid_tree_id(&tree) {
        return Err(ApiError::not_found().into());
    }
    let (root, _) = ensure_worktree(&state, &project, task.id).await?;
    let projection = inspect_tree(rt, &task, &root, &tree).await.map_err(focus_fail)?;
    Ok(Json(projection))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationsBody {
    expected_projection_hash: String,
    operations: Vec<FocusOp>,
}

async fn operations(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task, tree)): Path<(String, String, String)>,
    ApiJson(body): ApiJson<OperationsBody>,
) -> Result<(StatusCode, Json<Value>), Fail> {
    let rt = runtime(&state)?;
    let project = active_project_for(&state, &principal, &id, Permission::CreateTask).await?;
    rt.limit(LIMIT_FOCUS_OPERATION, principal.account_id())?;
    let task = load_task(rt, &principal, &project, &task).await?;
    if !valid_tree_id(&tree) {
        return Err(ApiError::not_found().into());
    }
    if body.operations.is_empty() || body.operations.len() > MAX_OPS {
        return Err(ApiError::invalid_request(format!("Send 1 to {MAX_OPS} operations.")).into());
    }
    if body.expected_projection_hash.len() != 64 || !body.expected_projection_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::invalid_request("expected_projection_hash is the projection's SHA-256.").into());
    }
    let (root, _) = ensure_worktree(&state, &project, task.id).await?;
    let relative_path = {
        let mut tx = rt.tx(principal.account_id()).await?;
        let latest = store::latest_projection(&mut tx, task.id, &tree).await?;
        tx.commit().await?;
        match latest {
            Some((_, b)) => b.relative_path,
            None => return Err(focus_fail(FocusError::StaleBase { current: Default::default() })),
        }
    };
    edit_allowed(&task, &relative_path, &tree)?;
    let applied = apply_operations(
        rt,
        &task,
        &root,
        ActorKind::Person(principal.account_id()),
        &tree,
        Some(&body.expected_projection_hash),
        body.operations,
    )
    .await
    .map_err(focus_fail)?;
    Ok((StatusCode::CREATED, Json(json!({ "proposal": applied.proposal, "projection": applied.projection }))))
}

async fn proposals(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let list = store::list_proposals(&mut tx, task.id).await?;
    tx.commit().await?;
    Ok(Json(json!({ "proposals": list })))
}

async fn proposal(
    principal: Principal,
    State(state): State<ServerState>,
    Path((id, task, proposal)): Path<(String, String, String)>,
) -> Result<Json<Value>, Fail> {
    let rt = runtime(&state)?;
    let project = project_for(&state, &principal, &id, Permission::ReadProject).await?;
    let task = load_task(rt, &principal, &project, &task).await?;
    let pid: uuid::Uuid = proposal.parse().map_err(|_| Fail::from(ApiError::not_found()))?;
    let mut tx = rt.tx(principal.account_id()).await?;
    let found = store::get_proposal(&mut tx, task.id, pid).await?;
    tx.commit().await?;
    found.map(Json).ok_or_else(|| ApiError::not_found().into())
}
