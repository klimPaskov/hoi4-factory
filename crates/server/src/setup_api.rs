//! Setup routes: scan, plan, apply, status and recovery of a project's mod, through the
//! `factory-setup-adapter` (C-09). Setup keeps the authority; the routes carry the caller's
//! identity and nothing else into it. No host path goes in or comes out: the adapter resolves
//! the workspace from the project and redacts its answers to logical locators.
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `POST /projects/{id}/setup/scan` | member (`create` mode: owner, maintainer) | Open an onboarding session and scan the mod read-only |
//! | `GET /projects/{id}/setup/plan` | member | The prepared plan, or 501 `capability_unavailable` while no plan builder exists |
//! | `POST /projects/{id}/setup/apply` | owner, maintainer | Apply the plan the caller names, once it is approved |
//! | `GET /projects/{id}/setup/status` | member | Scan, plan, approval, transactions and receipts |
//! | `GET /projects/{id}/setup/recovery` | member | What recovery the native journal permits, if a transaction was interrupted |
//! | `POST /projects/{id}/setup/recover` | owner, maintainer | Take that one permitted recovery action |
//!
//! A project not visible to the caller is the same `404 not_found` as a missing one (the adapter
//! checks roles itself; these routes check membership first so a public project of someone else
//! is not distinguishable either).
//!
//! # Sessions
//!
//! The adapter keeps the Setup gates of an onboarding session in memory. `scan` opens a new
//! session (closing the caller's previous one for the project) and the server remembers it per
//! account and project. `apply` needs it; after a restart the answer is `409
//! setup_session_missing` and the fix is to scan again.
//!
//! # Approval
//!
//! `apply` applies only an approved plan. Without approval it is refused (`plan_not_approved`,
//! nothing changes). A body that carries `plan_sha256`, the hash shown with the plan, approves
//! exactly that plan first: if the stored plan differs, nothing is approved and nothing applied.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use factory_accounts::{ApiError, ApiJson, Principal, Project, RetryClass};
use factory_setup_adapter::{InspectOptions, OpContext, PlanView, RecoveryVariant, SetupError};
use factory_workspace::{AccountId as WsAccount, ProjectId as WsProject};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::{IMPORT_PERMISSION, READ_PERMISSION, active_project_for, project_for};
use crate::api_error::Fail;
use crate::intake_api::{accepted_origins, origin_guard};
use crate::state::{ActiveSetup, ServerState};
use crate::workspace_api::open_workspace;

/// Longest brief of a new mod.
const MAX_DESCRIPTION_CHARS: usize = 4000;
/// Findings listed as facts of a scan.
const MAX_FACT_FINDINGS: usize = 10;

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route("/projects/{id}/setup/scan", post(scan))
        .route("/projects/{id}/setup/plan", get(plan))
        .route("/projects/{id}/setup/apply", post(apply))
        .route("/projects/{id}/setup/status", get(status))
        .route("/projects/{id}/setup/recovery", get(recovery))
        .route("/projects/{id}/setup/recover", post(recover))
        .layer(middleware::from_fn_with_state(accepted, origin_guard))
        .with_state(state.clone())
}

fn field_error(field: &'static str, message: &str) -> Fail {
    Fail::from(ApiError::invalid_request(message).with_id("field", field))
}

fn registry_key(
    principal: &Principal,
    project: &Project,
) -> (factory_accounts::AccountId, factory_accounts::ProjectId) {
    (principal.account_id(), project.id)
}

fn active(state: &ServerState, principal: &Principal, project: &Project) -> Option<ActiveSetup> {
    state
        .setup
        .sessions
        .lock()
        .ok()?
        .get(&registry_key(principal, project))
        .cloned()
}

// ---------------------------------------------------------------------------------------------
// Scan

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanBody {
    /// `create` plans a new mod, `existing` inspects what was brought in.
    mode: String,
    #[serde(default)]
    description: Option<String>,
}

/// Plain facts about a scan, for the person to confirm or correct. A scan that stopped or hit a
/// limit says so: incomplete coverage is never presented as complete.
fn scan_facts(scan: &Value) -> (Vec<Value>, bool) {
    let flag = |key: &str| scan.get(key).and_then(Value::as_bool).unwrap_or(false);
    let number = |pointer: &str| {
        scan.pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .to_string()
    };
    let (partial, cancelled) = (flag("partial"), flag("cancelled"));
    let incomplete = partial || cancelled;
    let coverage = if cancelled {
        "cancelled".to_owned()
    } else if partial {
        let limits: Vec<&str> = scan
            .get("limits_hit")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        format!("partial ({})", limits.join(", "))
    } else {
        "complete".to_owned()
    };
    let conflicts = scan
        .get("conflicts")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut facts = vec![
        json!({"label": "Coverage", "value": coverage}),
        json!({"label": "Files scanned", "value": number("/files_scanned")}),
        json!({"label": "Folders scanned", "value": number("/directories_scanned")}),
        json!({"label": "Findings that need review", "value": number("/summary/needs_review")}),
        json!({"label": "Findings that block", "value": number("/summary/blocking")}),
        json!({"label": "Warnings", "value": number("/summary/warnings")}),
        json!({"label": "Conflicts", "value": conflicts.to_string()}),
    ];
    let mut labels = std::collections::BTreeSet::new();
    let findings = scan
        .get("findings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for finding in findings.iter().take(MAX_FACT_FINDINGS) {
        let text = |key: &str| finding.get(key).and_then(Value::as_str).unwrap_or("");
        let mut label = format!("{}: {}", text("category"), text("key"));
        if !labels.insert(label.clone()) {
            label = format!("{label} ({})", text("id"));
            labels.insert(label.clone());
        }
        let value = match finding.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        facts.push(json!({"label": label, "value": value.chars().take(200).collect::<String>()}));
    }
    (facts, incomplete)
}

async fn scan(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ScanBody>,
) -> Result<Json<Value>, Fail> {
    let mode = match body.mode.as_str() {
        "create" | "existing" => body.mode.clone(),
        _ => return Err(field_error("mode", "mode must be create or existing.")),
    };
    let description = body
        .description
        .map(|d| d.trim().to_owned())
        .filter(|d| !d.is_empty());
    if description
        .as_ref()
        .is_some_and(|d| d.chars().count() > MAX_DESCRIPTION_CHARS)
    {
        return Err(field_error("description", "The description is too long."));
    }
    let project = if mode == "create" {
        active_project_for(&state, &principal, &id, IMPORT_PERMISSION).await?
    } else {
        project_for(&state, &principal, &id, READ_PERMISSION).await?
    };

    let has_workspace = open_workspace(&state, &project).is_ok();
    if !has_workspace {
        if mode == "existing" {
            return Err(Fail::from(
                ApiError::conflict(
                    "no_source",
                    "This project has no mod yet. Upload or import one first, or plan a new mod.",
                )
                .with_id("project_id", project.id),
            ));
        }
        // A new mod starts from an empty workspace.
        let storage = state.storage.clone();
        let (owner, pid) = (project.owner_account_id.to_string(), project.id.to_string());
        tokio::task::spawn_blocking(move || {
            let account = WsAccount::new(owner)?;
            let project = WsProject::new(pid)?;
            storage.create_workspace(&account, &project).map(|_| ())
        })
        .await
        .map_err(|_| ApiError::internal())?
        .map_err(|err| {
            tracing::error!(error = %err, "could not create a workspace for a new mod");
            Fail::from(ApiError::internal())
        })?;
    }

    let adapter = &state.setup.adapter;
    let account = principal.account_id();
    let previous = state
        .setup
        .sessions
        .lock()
        .ok()
        .and_then(|mut s| s.remove(&registry_key(&principal, &project)));
    if let Some(previous) = previous {
        // A session that is already gone is fine.
        let _ = adapter
            .close_session(account, project.id, previous.session)
            .await;
    }
    let session = adapter.open_session(account, project.id, None).await?;
    let outcome = adapter
        .inspect_project(
            account,
            project.id,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await?;
    if let Ok(mut sessions) = state.setup.sessions.lock() {
        sessions.insert(
            registry_key(&principal, &project),
            ActiveSetup {
                session: session.session_id,
                mode,
                description,
            },
        );
    }
    let scan_json = serde_json::to_value(&outcome.scan).unwrap_or(Value::Null);
    let (facts, incomplete) = scan_facts(&scan_json);
    Ok(Json(json!({
        "scan_id": outcome.scan_id,
        "facts": facts,
        "incomplete": incomplete,
        "receipt_id": outcome.receipt_id,
        "workspace_unchanged": outcome.workspace_unchanged,
    })))
}

// ---------------------------------------------------------------------------------------------
// Plan

/// The browser's view of a stored plan: every operation as create, change, preserve or conflict.
fn client_plan(view: &PlanView) -> Value {
    let plan = &view.plan;
    let operations = plan
        .pointer("/operations")
        .or_else(|| plan.pointer("/plan/operations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let unresolved: std::collections::BTreeSet<&str> = plan
        .pointer("/conflicts")
        .or_else(|| plan.pointer("/plan/conflicts"))
        .and_then(Value::as_array)
        .map(|conflicts| {
            conflicts
                .iter()
                .filter(|c| c.get("selected").is_none_or(Value::is_null))
                .filter_map(|c| c.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let operations: Vec<Value> = operations
        .iter()
        .map(|op| {
            let destination = op.get("destination").and_then(Value::as_str).unwrap_or("");
            let action = op.get("action").and_then(Value::as_str).unwrap_or("");
            let kind = if unresolved.contains(destination) {
                "conflict"
            } else {
                match action {
                    "create" => "create",
                    "skip" => "preserve",
                    _ => "change",
                }
            };
            let component = op.get("component_id").and_then(Value::as_str).unwrap_or("");
            json!({
                "path": destination,
                "action": kind,
                "note": if component.is_empty() { action.to_owned() } else { format!("{component} ({action})") },
            })
        })
        .collect();
    let components: Vec<Value> = plan
        .pointer("/components")
        .or_else(|| plan.pointer("/plan/components"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|c| {
                    let name = c.get("name").or_else(|| c.get("id"))?.as_str()?;
                    let version = c.get("version").and_then(Value::as_str).unwrap_or("");
                    Some(json!({"name": name, "version": version}))
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "plan_id": view.plan_id,
        "plan_sha256": view.plan_sha256,
        "state": view.state,
        "operations": operations,
        "components": components,
    })
}

async fn plan(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let adapter = &state.setup.adapter;
    let session = active(&state, &principal, &project).map(|a| a.session);
    let current = adapter
        .current_plan(principal.account_id(), project.id, session)
        .await?;
    if let Some(view) = current {
        return Ok(Json(json!({ "plan": client_plan(&view) })));
    }
    // No plan exists. Say why one cannot be built, and never invent one.
    let capabilities = adapter.capabilities();
    if let Some(plan) = capabilities.get("planSetup")
        && !plan.available
    {
        return Err(SetupError::CapabilityUnavailable {
            capability: "planSetup",
            reason: plan
                .reason
                .clone()
                .unwrap_or_else(|| "planning is not available on this server".into()),
        }
        .into());
    }
    Err(Fail::from(
        ApiError::new(
            StatusCode::CONFLICT,
            "plan_not_built",
            "No plan has been built for this project yet. Scan it, review the findings and build a plan first.",
            RetryClass::RefreshPreconditions,
        )
        .with_id("project_id", project.id),
    ))
}

// ---------------------------------------------------------------------------------------------
// Apply

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyBody {
    plan_id: Uuid,
    /// The hash shown with the plan. Present means: approve exactly this plan, then apply it.
    #[serde(default)]
    plan_sha256: Option<String>,
}

fn session_missing(project: &Project) -> Fail {
    Fail::from(
        ApiError::new(
            StatusCode::CONFLICT,
            "setup_session_missing",
            "This setup session is no longer open. Scan the project again, then review the plan.",
            RetryClass::RefreshPreconditions,
        )
        .with_id("project_id", project.id),
    )
}

async fn apply(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ApplyBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let adapter = &state.setup.adapter;
    let account = principal.account_id();
    let session = active(&state, &principal, &project)
        .ok_or_else(|| session_missing(&project))?
        .session;
    if let Some(reviewed) = body.plan_sha256.as_deref() {
        adapter
            .approve_setup_plan(
                account,
                project.id,
                session,
                body.plan_id,
                reviewed,
                OpContext::default(),
            )
            .await?;
    }
    match adapter
        .apply_setup_plan(
            account,
            project.id,
            session,
            body.plan_id,
            OpContext::default(),
        )
        .await
    {
        Ok(outcome) => {
            let mut out = serde_json::to_value(&outcome).unwrap_or(Value::Null);
            if let Some(map) = out.as_object_mut() {
                map.insert("status".into(), json!("applied"));
            }
            Ok(Json(out))
        }
        // The project was changed and the transaction did not finish. That is a result the
        // person must see, with the one recovery the journal permits, not a failed request.
        Err(SetupError::ApplyInterrupted {
            journal_id,
            variant,
            ..
        }) => Ok(Json(json!({
            "status": if variant == RecoveryVariant::RollbackRequiredPartialApply {
                "needs_rollback"
            } else {
                "interrupted"
            },
            "journal_id": journal_id,
            "variant": variant,
            "explanation": variant.explanation(),
        }))),
        Err(other) => Err(other.into()),
    }
}

// ---------------------------------------------------------------------------------------------
// Status and recovery

async fn status(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = active(&state, &principal, &project).map(|a| a.session);
    let status = state
        .setup
        .adapter
        .setup_status(principal.account_id(), project.id, session)
        .await?;
    Ok(Json(json!({
        "status": status,
        "session_open": session.is_some(),
    })))
}

async fn recovery(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let view = state
        .setup
        .adapter
        .inspect_recovery(principal.account_id(), project.id)
        .await?;
    Ok(Json(json!({ "recovery": view })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoverBody {
    journal_id: Uuid,
    /// The variant the person was shown. If the journal now permits something else the call is
    /// refused as stale.
    variant: String,
}

async fn recover(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<RecoverBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let variant = RecoveryVariant::parse(&body.variant)
        .ok_or_else(|| field_error("variant", "Unknown recovery variant."))?;
    let outcome = state
        .setup
        .adapter
        .recover_setup(
            principal.account_id(),
            project.id,
            body.journal_id,
            variant,
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({ "recovery": outcome })))
}
