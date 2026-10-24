//! Setup routes: the whole onboarding flow of a project's mod, through the
//! `factory-setup-adapter` (C-09). Setup keeps the authority; the routes carry the caller's
//! identity and nothing else into it. No host path goes in or comes out: the adapter resolves
//! the workspace from the project and redacts its answers to logical locators.
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `POST /projects/{id}/setup/scan` | member (`create` mode: owner, maintainer) | Open an onboarding session and scan the mod read-only; lists the excerpts the model could see |
//! | `POST /projects/{id}/setup/evidence` | member that may create tasks | Approve excerpts, by reference, for the model (none is a decision too) |
//! | `POST /projects/{id}/setup/analyze` | same | Ask the server's model for the values; returns them for review |
//! | `POST /projects/{id}/setup/confirm` | same | Confirm exactly the values the person reviewed (edits included) |
//! | `POST /projects/{id}/setup/plan` | same | Build the plan from the confirmed values (nothing is written) |
//! | `GET /projects/{id}/setup/plan` | member | The latest plan, or 409 `plan_not_built` |
//! | `POST /projects/{id}/setup/conflicts` | same as plan | Decide conflicts; the plan gets a new hash and loses its approval |
//! | `POST /projects/{id}/setup/approve` | owner, maintainer | Approve the plan with exactly this hash |
//! | `POST /projects/{id}/setup/apply` | owner, maintainer | Apply the plan the caller names, once it is approved |
//! | `GET /projects/{id}/setup/status` | member | Where the session stands, to restore the browser after a reload |
//! | `GET /projects/{id}/setup/recovery` | member | What recovery the native journal permits, if a transaction was interrupted |
//! | `POST /projects/{id}/setup/recover` | owner, maintainer | Take that one permitted recovery action |
//!
//! A project not visible to the caller is the same `404 not_found` as a missing one (the adapter
//! checks roles itself; these routes check membership first so a public project of someone else
//! is not distinguishable either).
//!
//! # Sessions
//!
//! The adapter keeps the Setup gates of an onboarding session in the facade, in memory, and the
//! session row (with its mode and brief) in PostgreSQL. `scan` opens a new session and discards
//! the caller's previous one for the project. Every later route needs the live session:
//!
//! * no session was ever opened: `409 setup_session_missing`;
//! * the database has an open session this process no longer holds (the server restarted, and
//!   the facade cannot reopen a session with its approved evidence, analyses and plans):
//!   `409 setup_session_expired`. The fix is to scan again; the scan route reuses the brief of a
//!   new mod that the lost session had.
//!
//! # Gates
//!
//! No plan without a confirmed analysis made in this session; no analysis without a decision on
//! the evidence; an approval is bound to the plan hash and the root identity and is withdrawn by
//! any conflict decision; `apply` applies only an approved plan and consumes the approval; after
//! a partial apply only rollback is offered. All of these are Setup's own gates, run unchanged.

use std::path::Path as FsPath;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use factory_accounts::{ApiError, ApiJson, Principal, Project, RetryClass};
use factory_setup_adapter::{
    InspectOptions, OnboardingSessionId, OpContext, PlanView, RecoveryVariant, ReviewDefaults,
    ReviewValues, SetupError, SetupMode,
};
use factory_workspace::{AccountId as WsAccount, ProjectId as WsProject};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::{IMPORT_PERMISSION, READ_PERMISSION, active_project_for, project_for};
use crate::api_error::Fail;
use crate::intake_api::{accepted_origins, origin_guard};
use crate::state::ServerState;
use crate::workspace_api::open_workspace;

/// Longest brief of a new mod.
const MAX_DESCRIPTION_CHARS: usize = 4000;
/// Findings listed as facts of a scan.
const MAX_FACT_FINDINGS: usize = 10;

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route("/projects/{id}/setup/scan", post(scan))
        .route("/projects/{id}/setup/evidence", post(evidence))
        .route("/projects/{id}/setup/analyze", post(analyze))
        .route("/projects/{id}/setup/confirm", post(confirm))
        .route("/projects/{id}/setup/plan", get(plan).post(build_plan))
        .route("/projects/{id}/setup/conflicts", post(conflicts))
        .route("/projects/{id}/setup/approve", post(approve))
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

fn session_missing(project: &Project) -> Fail {
    Fail::from(
        ApiError::new(
            StatusCode::CONFLICT,
            "setup_session_missing",
            "No setup session is open for this project. Scan the mod to start one.",
            RetryClass::RefreshPreconditions,
        )
        .with_id("project_id", project.id),
    )
}

fn session_expired(project: &Project) -> Fail {
    Fail::from(
        ApiError::new(
            StatusCode::CONFLICT,
            "setup_session_expired",
            "This setup session ended (the server restarted or the session timed out). Scan the mod again to continue.",
            RetryClass::RefreshPreconditions,
        )
        .with_id("project_id", project.id),
    )
}

/// The caller's live onboarding session on the project, or the reason there is none.
async fn live_session(
    state: &ServerState,
    principal: &Principal,
    project: &Project,
) -> Result<OnboardingSessionId, Fail> {
    match state
        .setup
        .adapter
        .open_session_state(principal.account_id(), project.id)
        .await?
    {
        None => Err(session_missing(project)),
        Some((id, true, _)) => Ok(id),
        Some((_, false, _)) => Err(session_expired(project)),
    }
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

/// Whether the source directory holds anything besides Git's own directory.
fn source_has_files(source: &FsPath) -> bool {
    std::fs::read_dir(source)
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.file_name() != std::ffi::OsStr::new(".git"))
        })
        .unwrap_or(false)
}

async fn scan(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ScanBody>,
) -> Result<Json<Value>, Fail> {
    let mode = SetupMode::parse(&body.mode)
        .ok_or_else(|| field_error("mode", "mode must be create or existing."))?;
    let mut description = body
        .description
        .map(|d| d.trim().to_owned())
        .filter(|d| !d.is_empty());
    if description
        .as_ref()
        .is_some_and(|d| d.chars().count() > MAX_DESCRIPTION_CHARS)
    {
        return Err(field_error("description", "The description is too long."));
    }
    let project = if mode == SetupMode::Create {
        active_project_for(&state, &principal, &id, IMPORT_PERMISSION).await?
    } else {
        project_for(&state, &principal, &id, READ_PERMISSION).await?
    };

    let workspace = open_workspace(&state, &project).ok();
    if workspace.is_none() && mode == SetupMode::Existing {
        return Err(Fail::from(
            ApiError::conflict(
                "no_source",
                "This project has no mod yet. Upload or import one first, or plan a new mod.",
            )
            .with_id("project_id", project.id),
        ));
    }
    if let (Some(workspace), SetupMode::Create) = (&workspace, mode) {
        let source = workspace.source_dir();
        let occupied = tokio::task::spawn_blocking(move || source_has_files(&source))
            .await
            .map_err(|_| ApiError::internal())?;
        if occupied {
            return Err(Fail::from(
                ApiError::conflict(
                    "workspace_not_empty",
                    "This project already holds a mod. Prepare it as an existing mod instead of creating a new one.",
                )
                .with_id("project_id", project.id),
            ));
        }
    }
    if workspace.is_none() {
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
    // The previous session is over, live or lost. A lost one tells us what the person was doing:
    // a new mod's brief is not asked for twice.
    if let Some((previous, _live, kept)) = adapter.open_session_state(account, project.id).await? {
        if mode == SetupMode::Create
            && description.is_none()
            && kept.get("mode").and_then(Value::as_str) == Some("create")
        {
            description = kept
                .get("brief")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .filter(|b| !b.trim().is_empty());
        }
        adapter
            .discard_session(account, project.id, previous)
            .await?;
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
    adapter
        .set_intent(account, project.id, session.session_id, mode, description)
        .await?;
    let evidence = adapter
        .evidence_candidates(account, project.id, session.session_id)
        .await?;
    let scan_json = serde_json::to_value(&outcome.scan).unwrap_or(Value::Null);
    let (facts, incomplete) = scan_facts(&scan_json);
    Ok(Json(json!({
        "scan_id": outcome.scan_id,
        "session_id": session.session_id,
        "mode": mode,
        "facts": facts,
        "incomplete": incomplete,
        "receipt_id": outcome.receipt_id,
        "workspace_unchanged": outcome.workspace_unchanged,
        "evidence": evidence,
    })))
}

// ---------------------------------------------------------------------------------------------
// Evidence, analysis and confirmation

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceBody {
    scan_id: Uuid,
    references: Vec<String>,
}

async fn evidence(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<EvidenceBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let approved = state
        .setup
        .adapter
        .approve_evidence_refs(
            principal.account_id(),
            project.id,
            session,
            body.scan_id,
            &body.references,
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({ "approved": approved })))
}

/// `1.19.3` is `1.19.*`: a mod targets a release line, not a patch.
fn supported_game_version(version: Option<&str>) -> String {
    let Some(version) = version else {
        return String::new();
    };
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        format!("{}.{}.*", parts[0], parts[1])
    } else {
        String::new()
    }
}

fn review_defaults(state: &ServerState, principal: &Principal) -> ReviewDefaults {
    let summary = state.game_reference.status().summary();
    ReviewDefaults {
        author: principal.account.display_name.clone(),
        supported_game_version: supported_game_version(summary.version.as_deref()),
    }
}

async fn analyze(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let draft = state
        .setup
        .adapter
        .analyze_session(
            principal.account_id(),
            project.id,
            session,
            &review_defaults(&state, &principal),
            OpContext::default(),
        )
        .await?;
    Ok(Json(serde_json::to_value(&draft).unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmBody {
    analysis_id: Uuid,
    values: ReviewValues,
}

async fn confirm(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ConfirmBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let confirmed = state
        .setup
        .adapter
        .confirm_review(
            principal.account_id(),
            project.id,
            session,
            body.analysis_id,
            body.values,
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({
        "confirmed": confirmed.confirmed,
        "values_sha256": confirmed.values_sha256,
        "confirmed_fields": confirmed.confirmed_fields,
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
    let conflicts: Vec<Value> = plan
        .pointer("/conflicts")
        .or_else(|| plan.pointer("/plan/conflicts"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|c| {
                    json!({
                        "path": c.get("path").and_then(Value::as_str).unwrap_or(""),
                        "options": c.get("options").cloned().unwrap_or_else(|| json!([])),
                        "selected": c.get("selected").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let unresolved: std::collections::BTreeSet<&str> = conflicts
        .iter()
        .filter(|c| c.get("selected").is_none_or(Value::is_null))
        .filter_map(|c| c.get("path").and_then(Value::as_str))
        .collect();
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
    let count = |kind: &str| {
        operations
            .iter()
            .filter(|op| op.get("action").and_then(Value::as_str) == Some(kind))
            .count()
    };
    let summary = json!({
        "create": count("create"),
        "change": count("change"),
        "preserve": count("preserve"),
        "conflict": count("conflict"),
    });
    let components: Vec<Value> = plan
        .pointer("/selected_components")
        .or_else(|| plan.pointer("/components"))
        .or_else(|| plan.pointer("/plan/components"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|c| {
                    if let Some(id) = c.as_str() {
                        return Some(json!({"name": id, "version": ""}));
                    }
                    let name = c.get("name").or_else(|| c.get("id"))?.as_str()?;
                    let version = c.get("version").and_then(Value::as_str).unwrap_or("");
                    Some(json!({"name": name, "version": version}))
                })
                .collect()
        })
        .unwrap_or_default();
    let source = plan
        .pointer("/source/resolved_revision")
        .and_then(Value::as_str)
        .map(|revision| json!({"revision": revision}))
        .unwrap_or(Value::Null);
    json!({
        "plan_id": view.plan_id,
        "plan_sha256": view.plan_sha256,
        "root_identity": view.root_identity,
        "state": view.state,
        "approved": view.state == "approved",
        "operations": operations,
        "conflicts": conflicts,
        "components": components,
        "source": source,
        "summary": summary,
    })
}

async fn build_plan(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let view = state
        .setup
        .adapter
        .plan_review(
            principal.account_id(),
            project.id,
            session,
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({ "plan": client_plan(&view) })))
}

async fn plan(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let adapter = &state.setup.adapter;
    let session = adapter
        .open_session_state(principal.account_id(), project.id)
        .await?
        .map(|(id, _, _)| id);
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionBody {
    path: String,
    choice: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConflictsBody {
    plan_id: Uuid,
    decisions: Vec<DecisionBody>,
}

async fn conflicts(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ConflictsBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let view = state
        .setup
        .adapter
        .resolve_setup_conflicts(
            principal.account_id(),
            project.id,
            session,
            body.plan_id,
            body.decisions
                .into_iter()
                .map(|d| factory_setup_adapter::ConflictDecision {
                    path: d.path,
                    choice: d.choice,
                })
                .collect(),
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({ "plan": client_plan(&view) })))
}

// ---------------------------------------------------------------------------------------------
// Approval and apply

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApproveBody {
    plan_id: Uuid,
    /// The hash shown with the plan. Nothing is approved if the stored plan differs.
    plan_sha256: String,
}

async fn approve(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ApproveBody>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let session = live_session(&state, &principal, &project).await?;
    let approval = state
        .setup
        .adapter
        .approve_setup_plan(
            principal.account_id(),
            project.id,
            session,
            body.plan_id,
            &body.plan_sha256,
            OpContext::default(),
        )
        .await?;
    Ok(Json(json!({
        "approval": {
            "approval_id": approval.approval_id,
            "plan_sha256": approval.approved_plan_sha256,
            "root_identity": approval.root_identity,
        }
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyBody {
    plan_id: Uuid,
    /// The hash shown with the plan. Present means: approve exactly this plan, then apply it.
    #[serde(default)]
    plan_sha256: Option<String>,
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
    let session = live_session(&state, &principal, &project).await?;
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

/// A scan as the browser shows it, from what the session holds.
fn client_scan(
    session: OnboardingSessionId,
    snapshot: &factory_setup_adapter::ReviewSnapshot,
) -> Value {
    let Some(scan) = &snapshot.scan else {
        return Value::Null;
    };
    let scan_json = serde_json::to_value(scan).unwrap_or(Value::Null);
    let (facts, incomplete) = scan_facts(&scan_json);
    json!({
        "scan_id": scan.scan_id,
        "session_id": session,
        "mode": snapshot.mode,
        "facts": facts,
        "incomplete": incomplete,
        "evidence": snapshot.evidence,
    })
}

async fn status(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let adapter = &state.setup.adapter;
    let account = principal.account_id();
    let open = adapter.open_session_state(account, project.id).await?;
    let (session, live, kept) = match open {
        Some((id, live, kept)) => (Some(id), live, kept),
        None => (None, false, Value::Null),
    };
    let setup = adapter.setup_status(account, project.id, session).await?;
    let mut out = json!({
        "status": setup,
        "session_open": live,
        "step": "none",
        "session": null,
        "scan": null,
        "analysis": null,
        "confirmed_values": null,
        "plan": null,
    });
    let Some(session_id) = session else {
        return Ok(Json(out));
    };
    let mode = kept
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("existing");
    out["session"] = json!({"session_id": session_id, "mode": mode});
    if !live {
        out["step"] = json!("expired");
        return Ok(Json(out));
    }
    let snapshot = adapter.review_snapshot(account, project.id, session_id)?;
    out["scan"] = client_scan(session_id, &snapshot);
    out["analysis"] = serde_json::to_value(&snapshot.analysis).unwrap_or(Value::Null);
    out["confirmed_values"] =
        serde_json::to_value(&snapshot.confirmed_values).unwrap_or(Value::Null);
    let plan = adapter
        .current_plan(account, project.id, Some(session_id))
        .await?;
    let required = snapshot.evidence.as_ref().is_some_and(|e| e.required);
    let step = if let Some(view) = &plan {
        out["plan"] = client_plan(view);
        match view.state.as_str() {
            "applied" => "applied",
            "approved" => "approved",
            _ => "planned",
        }
    } else if snapshot.confirmed_values.is_some() {
        "confirmed"
    } else if snapshot.analysis.is_some() {
        "analyzed"
    } else if required && snapshot.evidence_approved {
        "evidence_approved"
    } else if snapshot.scan.is_some() {
        "scanned"
    } else {
        "none"
    };
    out["step"] = json!(step);
    Ok(Json(out))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_version_becomes_a_release_line() {
        assert_eq!(supported_game_version(Some("1.19.3")), "1.19.*");
        assert_eq!(supported_game_version(Some("1.19.3.0")), "1.19.*");
        assert_eq!(supported_game_version(Some("beta")), "");
        assert_eq!(supported_game_version(None), "");
    }
}
