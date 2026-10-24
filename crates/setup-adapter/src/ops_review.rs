//! The review steps between a scan and a plan, as one browser flow needs them.
//!
//! The C-09 operations in `ops_scan.rs` and `ops_plan.rs` take what Setup's facade takes: full
//! evidence excerpts, a fully built analysis input, and the complete reviewed state. A browser
//! must not supply those: excerpts would be client-controlled bytes sent to a model provider, and
//! the state carries a host path. This file keeps the session's review progress on the server and
//! offers the steps by reference instead:
//!
//! 1. [`SetupAdapter::set_intent`]: create or existing, and the brief of a new mod;
//! 2. [`SetupAdapter::evidence_candidates`] and [`SetupAdapter::approve_evidence_refs`]: the
//!    person approves excerpts by reference; the server takes the bytes from the scan it ran;
//! 3. [`SetupAdapter::analyze_session`]: resolves the Setup source revision, sends Setup's own
//!    prompt through the server's [`ModelClient`](crate::ModelClient), validates the reply with
//!    Setup's validator and returns the values to review;
//! 4. [`SetupAdapter::confirm_review`]: the person confirms the exact values (edits included);
//!    the adapter composes the reviewed state and confirms the analysis with those values;
//! 5. [`SetupAdapter::plan_review`]: builds the plan from the confirmed state only.
//!
//! No step can be skipped: the facade's own gates (latest scan, approved evidence, confirmed
//! values, confirmed record in this session) run unchanged underneath. The gates live in memory
//! in the facade, so a restart ends the session ([`SetupAdapter::session_live`]).

use std::sync::Arc;

use hoi4_mod_setup::codex::{
    ApprovedEvidence, CodexAnalysis, MAX_EVIDENCE_BYTES, MAX_EVIDENCE_ITEMS,
};
use hoi4_mod_setup::credentials::{AI_PROVIDER_ENVIRONMENT_NAME, provider_name};
use hoi4_mod_setup::models::{
    CodexAnalysisRecord, CredentialReference, Platform, ScanResult, SourceMode,
};
use hoi4_mod_setup::security::sha256_bytes;
use hoi4_mod_setup::source::{SourceRequest, SourceResolution, resolve_source};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::adapter::{ActiveSession, SetupAdapter, blocking, map_native, sha256_json};
use crate::error::SetupError;
use crate::ids::{AccountId, OnboardingSessionId, OpContext, ProjectId};
use crate::ops_scan::confirmation_values;
use crate::views::{AnalysisInput, PlanView};

/// Longest brief, description or value accepted from a browser.
const MAX_TEXT_CHARS: usize = 4000;
const MAX_LIST_ITEMS: usize = 64;

/// What the person is doing with the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupMode {
    /// Plan a new mod in the empty workspace.
    Create,
    /// Prepare the mod that was uploaded or imported.
    Existing,
}

impl SetupMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Existing => "existing",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "create" => Some(Self::Create),
            "existing" => Some(Self::Existing),
            _ => None,
        }
    }
}

/// Progress of one session's review, kept beside the facade's own gates.
#[derive(Default)]
pub(crate) struct ReviewState {
    pub intent: Option<Intent>,
    /// The scan exactly as Setup returned it (host paths included; never leaves the adapter).
    pub scan: Option<Arc<ScanResult>>,
    /// Evidence approved after that scan. `None` until the person decided, an empty list is a
    /// decision.
    pub approved: Option<Vec<ApprovedEvidence>>,
    pub analysis: Option<AnalysisDraft>,
    pub confirmed: Option<ConfirmedReview>,
}

pub(crate) struct Intent {
    pub mode: SetupMode,
    pub brief: String,
}

pub(crate) struct AnalysisDraft {
    pub record: CodexAnalysisRecord,
    pub analysis: CodexAnalysis,
    pub source: Arc<SourceResolution>,
    pub view: ReviewDraft,
}

pub(crate) struct ConfirmedReview {
    pub values: ReviewValues,
    pub state: Value,
}

/// One excerpt the person may approve for the model.
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceCandidate {
    pub reference: String,
    pub path: String,
    pub category: String,
    pub key: String,
    pub excerpt: String,
    pub excerpt_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceCandidates {
    /// False for a new mod: there is nothing to send.
    pub required: bool,
    /// Candidates beyond the limits were left out.
    pub truncated: bool,
    pub candidates: Vec<EvidenceCandidate>,
}

/// The values a person reviews, confirms and may edit. They become the reviewed state Setup
/// binds the plan to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewValues {
    pub description: String,
    pub folder_profile: Vec<String>,
    pub identity: ReviewIdentity,
    pub conventions: ReviewConventions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewIdentity {
    pub display_name: String,
    pub project_id: String,
    pub author: String,
    pub version: String,
    pub supported_game_version: String,
    pub script_prefix: String,
    pub primary_namespace: String,
    pub default_branch: String,
    pub descriptor_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewConventions {
    pub agents_profile: String,
    pub localisation_convention: String,
    pub documentation_convention: String,
}

/// Defaults for what the model does not propose.
#[derive(Debug, Clone)]
pub struct ReviewDefaults {
    pub author: String,
    /// For example `1.19.*`, from the server's game reference.
    pub supported_game_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposalView {
    pub key: String,
    pub value: Value,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendationView {
    pub component_id: String,
    pub recommendation: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSummary {
    pub revision: String,
    pub manifest_sha256: String,
}

/// What the person reviews after the model answered.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewDraft {
    pub analysis_id: Uuid,
    pub attempts: u32,
    pub model: String,
    pub source: SourceSummary,
    pub summary: String,
    pub warnings: Vec<String>,
    pub proposals: Vec<ProposalView>,
    pub values: ReviewValues,
    pub component_recommendations: Vec<RecommendationView>,
    pub receipt_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfirmedView {
    pub confirmed: bool,
    pub values_sha256: String,
    pub confirmed_fields: Vec<String>,
    pub receipt_id: Option<Uuid>,
}

/// Where a session stands, for a browser that reloaded.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewSnapshot {
    pub mode: Option<SetupMode>,
    pub brief: String,
    pub scan: Option<ScanResult>,
    pub evidence: Option<EvidenceCandidates>,
    pub evidence_approved: bool,
    pub analysis: Option<ReviewDraft>,
    pub confirmed_values: Option<ReviewValues>,
}

fn invalid(field: &'static str, message: impl Into<String>) -> SetupError {
    SetupError::InvalidValues {
        field,
        message: message.into(),
    }
}

fn credential_reference(provider: &str) -> CredentialReference {
    CredentialReference {
        name: AI_PROVIDER_ENVIRONMENT_NAME.into(),
        provider: provider_name(Platform::current()).into(),
        reference: format!("credential://ai_provider_api_key/{provider}"),
        provider_id: Some(provider.into()),
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap_or("").trim().to_owned()
}

fn list_of(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The reviewable values from a validated analysis, with the host's defaults for what a model
/// does not decide.
pub fn values_from_analysis(analysis: &CodexAnalysis, defaults: &ReviewDefaults) -> ReviewValues {
    let get = |name: &str| {
        analysis
            .proposals
            .iter()
            .find(|p| {
                serde_json::to_value(&p.key)
                    .ok()
                    .and_then(|k| k.as_str().map(str::to_owned))
                    .as_deref()
                    == Some(name)
            })
            .map(|p| p.value.clone())
            .unwrap_or(Value::Null)
    };
    ReviewValues {
        description: text_of(&get("project_description")),
        folder_profile: list_of(&get("folder_profile")),
        identity: ReviewIdentity {
            display_name: text_of(&get("display_name")),
            project_id: text_of(&get("project_id")),
            author: defaults.author.clone(),
            version: "0.1.0".into(),
            supported_game_version: defaults.supported_game_version.clone(),
            script_prefix: text_of(&get("script_prefix")),
            primary_namespace: text_of(&get("primary_namespace")),
            default_branch: "main".into(),
            descriptor_tags: list_of(&get("descriptor_tags")),
        },
        conventions: ReviewConventions {
            agents_profile: text_of(&get("agents_profile")),
            localisation_convention: text_of(&get("localisation_convention")),
            documentation_convention: text_of(&get("documentation_convention")),
        },
    }
}

fn check_text(field: &'static str, value: &str, required: bool) -> Result<(), SetupError> {
    if required && value.trim().is_empty() {
        return Err(invalid(field, format!("{field} is required.")));
    }
    if value.chars().count() > MAX_TEXT_CHARS || value.chars().any(|c| c == '\0') {
        return Err(invalid(
            field,
            format!("{field} is too long or contains an invalid character."),
        ));
    }
    Ok(())
}

fn check_list(field: &'static str, items: &[String], required: bool) -> Result<(), SetupError> {
    if required && items.is_empty() {
        return Err(invalid(field, format!("{field} needs at least one entry.")));
    }
    if items.len() > MAX_LIST_ITEMS {
        return Err(invalid(field, format!("{field} has too many entries.")));
    }
    for item in items {
        check_text(field, item, true)?;
    }
    Ok(())
}

impl ReviewValues {
    /// Shape checks that need no Setup knowledge. Setup validates the rest when it plans, and
    /// a rejected value comes back as the planning error.
    pub fn validate(&self) -> Result<(), SetupError> {
        check_text("description", &self.description, false)?;
        check_list("folder_profile", &self.folder_profile, true)?;
        let i = &self.identity;
        check_text("display_name", &i.display_name, true)?;
        check_text("project_id", &i.project_id, true)?;
        check_text("author", &i.author, true)?;
        check_text("version", &i.version, true)?;
        check_text("supported_game_version", &i.supported_game_version, true)?;
        check_text("script_prefix", &i.script_prefix, false)?;
        check_text("primary_namespace", &i.primary_namespace, false)?;
        check_text("default_branch", &i.default_branch, true)?;
        check_list("descriptor_tags", &i.descriptor_tags, false)?;
        let c = &self.conventions;
        check_text("agents_profile", &c.agents_profile, true)?;
        check_text("localisation_convention", &c.localisation_convention, true)?;
        check_text(
            "documentation_convention",
            &c.documentation_convention,
            true,
        )?;
        Ok(())
    }
}

/// The reviewed state Setup builds the plan from. The project root is the session's mod root; no
/// path comes from the browser.
fn compose_state(
    active: &ActiveSession,
    route: &crate::adapter::AnalysisRoute,
    mode: SetupMode,
    source: &SourceResolution,
    scan_id: Uuid,
    values: &ReviewValues,
    recommendations: Value,
) -> Value {
    let i = &values.identity;
    let c = &values.conventions;
    json!({
        "mode": match mode { SetupMode::Create => "new", SetupMode::Existing => "existing" },
        "description": values.description,
        // The plan resolves exactly the revision the analysis was bound to, not whatever the
        // default branch holds by then.
        "sourceMode": "pinned_commit",
        "pinnedRef": source.identity.resolved_revision,
        "aiProvider": route.provider,
        "aiModel": route.model,
        "aiReasoningEffort": route.reasoning_effort,
        "aiEndpoint": route.endpoint,
        "gitMode": "skip",
        "primaryCodingEnvironment": "claude_code",
        "additionalCodingEnvironments": ["codex"],
        "selectedComponents": [],
        "folderProfile": values.folder_profile,
        "conventions": {
            "agents_profile": c.agents_profile,
            "localisation_convention": c.localisation_convention,
            "documentation_convention": c.documentation_convention,
        },
        "identity": {
            "displayName": i.display_name,
            "projectId": i.project_id,
            "author": i.author,
            "version": i.version,
            "supportedGameVersion": i.supported_game_version,
            "projectRoot": active.mod_root.display().to_string(),
            "defaultBranch": i.default_branch,
            "scriptPrefix": i.script_prefix,
            "primaryNamespace": i.primary_namespace,
            "descriptorTags": i.descriptor_tags,
        },
        "componentRecommendations": recommendations,
        "scanContext": {"scanId": scan_id},
    })
}

fn proposal_keys(analysis: &CodexAnalysis) -> Vec<String> {
    analysis
        .proposals
        .iter()
        .filter_map(|p| {
            serde_json::to_value(&p.key)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
        })
        .collect()
}

impl SetupAdapter {
    /// Whether this process still holds the session's Setup gates.
    pub fn session_live(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> bool {
        self.session(account, project, session).is_ok()
    }

    /// Ends a session this process may or may not still hold, and closes its database row. A
    /// session lost to a restart is closed the same way.
    pub async fn discard_session(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> Result<(), SetupError> {
        let access = self
            .access(account, project, factory_accounts::Permission::ReadProject)
            .await?;
        self.inner
            .sessions
            .lock()
            .map_err(|_| SetupError::Internal("session registry poisoned".into()))?
            .remove(&(account, project, session));
        self.inner
            .store
            .close_session(&access.scope, session.0)
            .await
    }

    /// The newest session this account left open on the project, and whether this process still
    /// holds its gates. `None` when the account has none.
    pub async fn open_session_state(
        &self,
        account: AccountId,
        project: ProjectId,
    ) -> Result<Option<(OnboardingSessionId, bool, Value)>, SetupError> {
        let access = self
            .access(account, project, factory_accounts::Permission::ReadProject)
            .await?;
        let Some(row) = self.inner.store.latest_open_session(&access.scope).await? else {
            return Ok(None);
        };
        let id = OnboardingSessionId(row.id);
        let live = self.session_live(account, project, id);
        Ok(Some((id, live, row.client_state)))
    }

    /// Records whether the person creates a new mod or prepares an existing one, and the brief of
    /// a new mod. Kept in memory with the gates and, bounded, in the database.
    pub async fn set_intent(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        mode: SetupMode,
        brief: Option<String>,
    ) -> Result<(), SetupError> {
        let active = self.session(account, project, session)?;
        let brief = brief.unwrap_or_default();
        check_text("description", &brief, false)?;
        self.inner
            .store
            .update_session_state(
                &active.scope,
                session.0,
                &json!({"mode": mode.as_str(), "brief": brief}),
            )
            .await?;
        let mut review = active
            .review
            .lock()
            .map_err(|_| SetupError::Internal("review state poisoned".into()))?;
        review.intent = Some(Intent { mode, brief });
        Ok(())
    }

    fn review_of<T>(
        active: &ActiveSession,
        f: impl FnOnce(&mut ReviewState) -> Result<T, SetupError>,
    ) -> Result<T, SetupError> {
        let mut review = active
            .review
            .lock()
            .map_err(|_| SetupError::Internal("review state poisoned".into()))?;
        f(&mut review)
    }

    /// The excerpts the scan found that could be sent to the model. A new mod has none.
    pub async fn evidence_candidates(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> Result<EvidenceCandidates, SetupError> {
        let active = self.session(account, project, session)?;
        Self::review_of(&active, |review| {
            let mode = review.intent.as_ref().map(|i| i.mode);
            let scan = review.scan.clone();
            Ok(candidates_of(&active, mode, scan.as_deref()))
        })
    }

    /// Approves the excerpts named by reference for the model. The bytes come from the scan the
    /// server ran, never from the caller. Approving none is a decision too.
    pub async fn approve_evidence_refs(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        scan_id: Uuid,
        references: &[String],
        ctx: OpContext,
    ) -> Result<usize, SetupError> {
        let active = self.session(account, project, session)?;
        let (offered, scan) = Self::review_of(&active, |review| {
            let scan = review
                .scan
                .clone()
                .ok_or_else(|| SetupError::precondition("scan_required", "Scan the mod first."))?;
            let mode = review.intent.as_ref().map(|i| i.mode);
            Ok((candidates_of(&active, mode, Some(&scan)), scan))
        })?;
        if scan.scan_id != scan_id {
            return Err(SetupError::stale(
                "scan_stale",
                "That is not the latest scan of this mod. Scan it again.",
            ));
        }
        if !offered.required && !references.is_empty() {
            return Err(SetupError::precondition(
                "evidence_not_required",
                "A new mod has no excerpts to approve.",
            ));
        }
        let mut chosen: Vec<ApprovedEvidence> = Vec::new();
        for reference in references {
            if chosen.iter().any(|e| &e.reference == reference) {
                continue;
            }
            let candidate = offered
                .candidates
                .iter()
                .find(|c| &c.reference == reference)
                .ok_or_else(|| {
                    SetupError::precondition(
                        "unknown_evidence_reference",
                        "That excerpt was not offered by the scan.",
                    )
                })?;
            chosen.push(ApprovedEvidence {
                reference: candidate.reference.clone(),
                path: candidate.path.clone(),
                excerpt: candidate.excerpt.clone(),
                excerpt_sha256: candidate.excerpt_sha256.clone(),
                confidence: None,
            });
        }
        self.confirm_setup_evidence(account, project, session, scan_id, chosen.clone(), ctx)
            .await?;
        let count = chosen.len();
        let state = Self::review_of(&active, |review| {
            review.approved = Some(chosen.clone());
            // New evidence invalidates what was derived from the old.
            review.analysis = None;
            review.confirmed = None;
            Ok(json!({
                "mode": review.intent.as_ref().map(|i| i.mode.as_str()),
                "brief": review.intent.as_ref().map(|i| i.brief.clone()),
                "approved_references": chosen.iter().map(|e| e.reference.clone()).collect::<Vec<_>>(),
            }))
        })?;
        self.inner
            .store
            .update_session_state(&active.scope, session.0, &state)
            .await?;
        Ok(count)
    }

    /// Runs the analysis: Setup's prompt, the server's model, Setup's validator. Returns the
    /// values to review.
    pub async fn analyze_session(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        defaults: &ReviewDefaults,
        ctx: OpContext,
    ) -> Result<ReviewDraft, SetupError> {
        let active = self.session(account, project, session)?;
        let route = self
            .inner
            .analysis_route
            .clone()
            .filter(|_| self.inner.model.is_some())
            .ok_or_else(|| SetupError::CapabilityUnavailable {
                capability: "analyzeSetup",
                reason: "no model is configured on this server".into(),
            })?;
        let fetcher =
            self.inner
                .source
                .clone()
                .ok_or_else(|| SetupError::CapabilityUnavailable {
                    capability: "source",
                    reason: "this server has no source for the Setup workflow package".into(),
                })?;
        let (mode, brief, scan, approved) =
            Self::review_of(&active, |review| {
                let intent = review.intent.as_ref().ok_or_else(|| {
                    SetupError::precondition("scan_required", "Scan the mod first.")
                })?;
                let scan = review.scan.clone().ok_or_else(|| {
                    SetupError::precondition("scan_required", "Scan the mod first.")
                })?;
                Ok((
                    intent.mode,
                    intent.brief.clone(),
                    scan,
                    review.approved.clone(),
                ))
            })?;
        let evidence = match (mode, approved) {
            (SetupMode::Create, _) => Vec::new(),
            (SetupMode::Existing, Some(list)) => list,
            (SetupMode::Existing, None) => {
                return Err(SetupError::precondition(
                    "evidence_not_approved",
                    "Decide which excerpts may be sent to the model first.",
                ));
            }
        };
        let source: Arc<SourceResolution> = Arc::new(
            blocking(move || {
                resolve_source(
                    fetcher.as_ref(),
                    &SourceRequest {
                        mode: SourceMode::Latest,
                        requested_ref: None,
                        release: None,
                    },
                )
            })
            .await
            .map_err(|e| map_native(e, &active.redactor))?,
        );
        let input = AnalysisInput {
            provider: route.provider.clone(),
            model: route.model.clone(),
            reasoning_effort: route.reasoning_effort.clone(),
            endpoint: route.endpoint.clone(),
            credential: Some(credential_reference(&route.provider)),
            mode: match mode {
                SetupMode::Create => "new_project_identity".into(),
                SetupMode::Existing => "existing_project_semantics".into(),
            },
            brief,
            evidence,
            constraints: json!({}),
            purpose: (mode == SetupMode::Existing).then(|| "existing_project_import".to_owned()),
            scan_id: Some(scan.scan_id),
        };
        let outcome = self
            .analyze_setup(account, project, session, input, source.clone(), ctx)
            .await?;
        let analysis = outcome.result.analysis.clone();
        let values = values_from_analysis(&analysis, defaults);
        let view = ReviewDraft {
            analysis_id: outcome.analysis_id,
            attempts: outcome.attempts,
            model: route.model.clone(),
            source: SourceSummary {
                revision: source.identity.resolved_revision.clone(),
                manifest_sha256: source.identity.manifest_sha256.clone(),
            },
            summary: active.redactor.text(&analysis.project_summary),
            warnings: analysis
                .warnings
                .iter()
                .map(|w| active.redactor.text(w))
                .collect(),
            proposals: analysis
                .proposals
                .iter()
                .map(|p| ProposalView {
                    key: serde_json::to_value(&p.key)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    value: p.value.clone(),
                    confidence: p.confidence,
                    reason: active.redactor.text(&p.reason),
                })
                .collect(),
            values,
            component_recommendations: analysis
                .component_recommendations
                .iter()
                .map(|r| RecommendationView {
                    component_id: r.component_id.clone(),
                    recommendation: serde_json::to_value(&r.recommendation)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    reason: active.redactor.text(&r.reason),
                })
                .collect(),
            receipt_id: outcome.receipt_id,
        };
        Self::review_of(&active, |review| {
            review.analysis = Some(AnalysisDraft {
                record: outcome.result.record.clone(),
                analysis,
                source,
                view: view.clone(),
            });
            review.confirmed = None;
            Ok(())
        })?;
        Ok(view)
    }

    /// Confirms the analysis with exactly these values. They are the values the plan is built
    /// from; changing them later means confirming again.
    pub async fn confirm_review(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        analysis_id: Uuid,
        values: ReviewValues,
        ctx: OpContext,
    ) -> Result<ConfirmedView, SetupError> {
        let active = self.session(account, project, session)?;
        let route =
            self.inner
                .analysis_route
                .clone()
                .ok_or_else(|| SetupError::CapabilityUnavailable {
                    capability: "analyzeSetup",
                    reason: "no model is configured on this server".into(),
                })?;
        values.validate()?;
        let (record, analysis, source, mode, scan_id) =
            Self::review_of(&active, |review| {
                let draft = review.analysis.as_ref().ok_or_else(|| {
                    SetupError::stale(
                        "analysis_not_in_session",
                        "There is no analysis to confirm. Run the analysis first.",
                    )
                })?;
                if draft.view.analysis_id != analysis_id {
                    return Err(SetupError::stale(
                        "analysis_not_in_session",
                        "That analysis is no longer the current one. Run the analysis again.",
                    ));
                }
                let mode = review.intent.as_ref().map(|i| i.mode).ok_or_else(|| {
                    SetupError::precondition("scan_required", "Scan the mod first.")
                })?;
                let scan_id = review.scan.as_ref().map(|s| s.scan_id).ok_or_else(|| {
                    SetupError::precondition("scan_required", "Scan the mod first.")
                })?;
                Ok((
                    draft.record.clone(),
                    draft.analysis.clone(),
                    draft.source.clone(),
                    mode,
                    scan_id,
                ))
            })?;
        let recommendations = serde_json::to_value(&analysis.component_recommendations)
            .map_err(|e| SetupError::Internal(e.to_string()))?;
        let mut state = compose_state(
            &active,
            &route,
            mode,
            &source,
            scan_id,
            &values,
            recommendations,
        );
        let fields = proposal_keys(&analysis);
        let confirmed = self
            .confirm_setup_analysis(
                account,
                project,
                session,
                record,
                fields.clone(),
                &state,
                ctx,
            )
            .await?;
        state["codexAnalysisRecord"] =
            serde_json::to_value(&confirmed).map_err(|e| SetupError::Internal(e.to_string()))?;
        let values_sha256 = sha256_json(&confirmation_values(&state));
        Self::review_of(&active, |review| {
            review.confirmed = Some(ConfirmedReview {
                values: values.clone(),
                state,
            });
            Ok(())
        })?;
        Ok(ConfirmedView {
            confirmed: true,
            values_sha256,
            confirmed_fields: fields,
            receipt_id: None,
        })
    }

    /// Builds the plan from the confirmed values. Nothing is written to the mod.
    pub async fn plan_review(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        ctx: OpContext,
    ) -> Result<PlanView, SetupError> {
        let active = self.session(account, project, session)?;
        let state = Self::review_of(&active, |review| {
            review
                .confirmed
                .as_ref()
                .map(|c| c.state.clone())
                .ok_or_else(|| {
                    SetupError::precondition(
                        "analysis_not_confirmed",
                        "Confirm the analysis values before planning.",
                    )
                })
        })?;
        self.plan_setup(account, project, session, state, ctx).await
    }

    /// Where the session stands, for a browser that reloaded.
    pub fn review_snapshot(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> Result<ReviewSnapshot, SetupError> {
        let active = self.session(account, project, session)?;
        Self::review_of(&active, |review| {
            let mode = review.intent.as_ref().map(|i| i.mode);
            let scan = review.scan.clone();
            let evidence = scan
                .as_deref()
                .map(|s| candidates_of(&active, mode, Some(s)));
            let redacted_scan = scan.as_deref().and_then(|s| {
                serde_json::to_value(s)
                    .ok()
                    .map(|v| active.redactor.value(&v))
                    .and_then(|v| serde_json::from_value::<ScanResult>(v).ok())
            });
            Ok(ReviewSnapshot {
                mode,
                brief: review
                    .intent
                    .as_ref()
                    .map(|i| i.brief.clone())
                    .unwrap_or_default(),
                scan: redacted_scan,
                evidence,
                evidence_approved: review.approved.is_some(),
                analysis: review.analysis.as_ref().map(|a| a.view.clone()),
                confirmed_values: review.confirmed.as_ref().map(|c| c.values.clone()),
            })
        })
    }

    /// Used by the scan step: a new scan resets what was derived from the previous one.
    pub(crate) fn note_scan(active: &ActiveSession, scan: &ScanResult) {
        if let Ok(mut review) = active.review.lock() {
            review.scan = Some(Arc::new(scan.clone()));
            review.approved = None;
            review.analysis = None;
            review.confirmed = None;
        }
    }
}

fn candidates_of(
    active: &ActiveSession,
    mode: Option<SetupMode>,
    scan: Option<&ScanResult>,
) -> EvidenceCandidates {
    if mode != Some(SetupMode::Existing) {
        return EvidenceCandidates {
            required: false,
            truncated: false,
            candidates: Vec::new(),
        };
    }
    let mut candidates = Vec::new();
    let mut truncated = false;
    let mut bytes = 0usize;
    if let Some(scan) = scan {
        for finding in &scan.findings {
            let Some(excerpt) = finding.model_excerpt.as_deref().filter(|e| !e.is_empty()) else {
                continue;
            };
            let Some(first) = finding.evidence.first() else {
                continue;
            };
            // Setup refuses to send the local portrait routes of an earlier installation to any
            // provider, so they are not offered.
            if finding.id == hoi4_mod_setup::scanner::LOCAL_ONLY_PORTRAIT_ROUTES_FINDING {
                continue;
            }
            // An excerpt that carries a host path cannot be shown or hashed faithfully.
            if active.redactor.leaks(excerpt) || active.redactor.leaks(&first.path) {
                continue;
            }
            if candidates.len() >= MAX_EVIDENCE_ITEMS || bytes + excerpt.len() > MAX_EVIDENCE_BYTES
            {
                truncated = true;
                continue;
            }
            bytes += excerpt.len();
            candidates.push(EvidenceCandidate {
                reference: finding.id.clone(),
                path: first.path.clone(),
                category: finding.category.clone(),
                key: finding.key.clone(),
                excerpt: excerpt.to_owned(),
                excerpt_sha256: sha256_bytes(excerpt.as_bytes()),
            });
        }
    }
    EvidenceCandidates {
        required: true,
        truncated,
        candidates,
    }
}
