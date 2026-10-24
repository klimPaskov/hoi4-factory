//! inspectProject, confirmSetupEvidence, analyzeSetup and confirmSetupAnalysis.

use std::sync::Arc;

use factory_accounts::Permission;
use hoi4_mod_setup::codex::{
    ANALYSIS_ATTEMPTS, AiAnalysisRequest, ApprovedEvidence, CodexAnalysisRequest,
    correctable_output_error, corrective_analysis_prompt,
};
use hoi4_mod_setup::models::{CodexAnalysisRecord, ScanResult};
use hoi4_mod_setup::service::InspectOptions as FacadeInspectOptions;
use hoi4_mod_setup::source::SourceResolution;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::adapter::{
    ActiveSession, SetupAdapter, blocking, capture_inventory, json_request, map_native, sha256_json,
};
use crate::error::SetupError;
use crate::ids::{AccountId, OnboardingSessionId, OpContext, ProjectId};
use crate::model::{ModelRequest, analysis_contract};
use crate::store::{AnalysisInsert, ReceiptOutcome, ScanInsert, SetupOperation};
use crate::views::{AnalysisInput, AnalysisOutcome, InspectOptions, InspectOutcome};

/// The values a reviewer confirms, taken from the reviewed state exactly as Setup's own
/// confirmation does. The plan is built from the same state, and Setup refuses a plan whose
/// values differ from the confirmed ones.
pub fn confirmation_values(state: &Value) -> Value {
    json!({
        "description": state.get("description").cloned().unwrap_or(Value::String(String::new())),
        "folderProfile": state.get("folderProfile").cloned().unwrap_or_else(|| json!([])),
        "identity": state.get("identity").cloned().unwrap_or(Value::Null),
        "conventions": state.get("conventions").cloned().unwrap_or(Value::Null),
        "componentRecommendations": state
            .pointer("/codexAnalysis/component_recommendations")
            .cloned()
            .or_else(|| state.get("componentRecommendations").cloned())
            .unwrap_or_else(|| json!([])),
    })
}

/// Explicit coverage of a scan: a stopped or limited scan is never presented as complete.
fn coverage_text(scan: &ScanResult) -> String {
    if scan.cancelled {
        "cancelled".to_owned()
    } else if scan.partial {
        format!("partial ({})", scan.limits_hit.join(", "))
    } else {
        "complete".to_owned()
    }
}

impl SetupAdapter {
    /// Read-only scan of the session's mod root. The workspace inventory (every file's hash and
    /// mode, plus the Git index hash) is taken before and after; a difference is reported as
    /// `read_only_violated` and never repaired.
    pub async fn inspect_project(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        options: InspectOptions,
        ctx: OpContext,
    ) -> Result<InspectOutcome, SetupError> {
        self.access(account, project, Permission::ReadProject)
            .await?;
        let active = self.session(account, project, session)?;
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::InspectProject,
                &ctx,
                json_request(json!({ "mod_root": active.mod_root_rel })),
                None,
                None,
                None,
                None,
            )
            .await?;
        let receipt_id = handle.id;
        let result = self.inspect_inner(&active, options).await;
        let mut out = self.close_receipt(handle, result).await?;
        out.receipt_id = receipt_id;
        Ok(out)
    }

    async fn inspect_inner(
        &self,
        active: &Arc<ActiveSession>,
        options: InspectOptions,
    ) -> Result<(InspectOutcome, ReceiptOutcome), SetupError> {
        let before = if self.inner.verify_read_only {
            Some(capture_inventory(&active.workspace).await?)
        } else {
            None
        };
        let facade = active.facade.clone();
        let root = active.mod_root.clone();
        let cancel = options.cancel.clone();
        let progress = options.progress.clone();
        let scan = blocking(move || {
            facade.inspect(&root, FacadeInspectOptions { cancel }, move |p| {
                if let Some(sink) = &progress {
                    sink(p)
                }
            })
        })
        .await
        .map_err(|e| map_native(e, &active.redactor))?;
        let facade = active.facade.clone();
        let root = active.mod_root.clone();
        let root_identity = blocking(move || facade.root_identity(&root))
            .await
            .map_err(|e| map_native(e, &active.redactor))?;
        let after = if before.is_some() {
            Some(capture_inventory(&active.workspace).await?)
        } else {
            None
        };
        let diff = match (&before, &after) {
            (Some(b), Some(a)) => Some(b.diff(a)),
            _ => None,
        };
        let unchanged = diff.as_ref().map(|d| d.is_empty());
        let before_hash = before.as_ref().map(|i| i.content_hash());
        let after_hash = after.as_ref().map(|i| i.content_hash());

        SetupAdapter::note_scan(active, &scan);
        let raw = serde_json::to_value(&scan).map_err(|e| SetupError::Internal(e.to_string()))?;
        let scan_sha256 = sha256_json(&raw);
        let redacted_value = active.redactor.value(&raw);
        let redacted: ScanResult = serde_json::from_value(redacted_value.clone())
            .map_err(|e| SetupError::Internal(e.to_string()))?;
        self.inner
            .store
            .insert_scan(
                &active.scope,
                &ScanInsert {
                    id: scan.scan_id,
                    session_id: active.id.0,
                    root_identity: root_identity.clone(),
                    mod_root: active.mod_root_rel.clone(),
                    coverage: coverage_text(&scan),
                    partial: scan.partial,
                    cancelled: scan.cancelled,
                    files_scanned: i64::from(scan.files_scanned),
                    directories_scanned: i64::from(scan.directories_scanned),
                    bytes_read: i64::try_from(scan.bytes_read).unwrap_or(i64::MAX),
                    finding_count: i32::try_from(scan.findings.len()).unwrap_or(i32::MAX),
                    conflict_count: i32::try_from(scan.conflicts.len()).unwrap_or(i32::MAX),
                    semantic_analysis_required: scan.semantic_analysis.required,
                    scan_sha256: scan_sha256.clone(),
                    result: redacted_value,
                    snapshot_before_sha256: before_hash.clone(),
                    snapshot_after_sha256: after_hash.clone(),
                    snapshot_unchanged: unchanged,
                    created_at: self.inner.store.now(),
                },
            )
            .await?;
        if let Some(d) = diff.as_ref().filter(|d| !d.is_empty()) {
            let sample: Vec<&String> = d
                .added
                .iter()
                .chain(&d.removed)
                .chain(&d.changed)
                .chain(&d.mode_changed)
                .take(10)
                .collect();
            return Err(SetupError::ReadOnlyViolated(format!(
                "{} paths differ (index changed: {}), for example {:?}",
                d.added.len() + d.removed.len() + d.changed.len() + d.mode_changed.len(),
                d.index_changed,
                sample
            )));
        }
        let outcome = ReceiptOutcome {
            before_inventory_sha256: before_hash.clone(),
            after_inventory_sha256: after_hash.clone(),
            result: Some(json!({
                "scan_id": scan.scan_id,
                "scan_sha256": scan_sha256,
                "root_identity": root_identity,
                "coverage": coverage_text(&scan),
                "partial": scan.partial,
                "cancelled": scan.cancelled,
                "workspace_unchanged": unchanged,
            })),
            ..ReceiptOutcome::default()
        };
        Ok((
            InspectOutcome {
                scan_id: scan.scan_id,
                root_identity,
                scan: redacted,
                receipt_id: Uuid::nil(),
                inventory_before_sha256: before_hash,
                inventory_after_sha256: after_hash,
                workspace_unchanged: unchanged,
            },
            outcome,
        ))
    }

    /// Approve the exact evidence that may be sent to the model. Setup accepts only values it
    /// scanned itself in the latest scan of this root.
    pub async fn confirm_setup_evidence(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        scan_id: Uuid,
        evidence: Vec<ApprovedEvidence>,
        ctx: OpContext,
    ) -> Result<(), SetupError> {
        self.access(account, project, Permission::CreateTask)
            .await?;
        let active = self.session(account, project, session)?;
        let refs: Vec<Value> = evidence
            .iter()
            .map(|e| json!({"reference": e.reference, "path": e.path, "excerpt_sha256": e.excerpt_sha256}))
            .collect();
        let evidence_sha256 = sha256_json(&Value::Array(refs.clone()));
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::ConfirmSetupEvidence,
                &ctx,
                json_request(json!({"scan_id": scan_id, "evidence": refs})),
                Some(scan_id),
                None,
                None,
                None,
            )
            .await?;
        let result = async {
            let facade = active.facade.clone();
            let root = active.mod_root.clone();
            let count = evidence.len();
            blocking(move || facade.approve_evidence(&root, scan_id, &evidence))
                .await
                .map_err(|e| map_native(e, &active.redactor))?;
            self.inner
                .store
                .mark_evidence_approved(
                    &active.scope,
                    scan_id,
                    i32::try_from(count).unwrap_or(i32::MAX),
                    &evidence_sha256,
                )
                .await?;
            Ok::<_, SetupError>((
                (),
                ReceiptOutcome {
                    result: Some(
                        json!({"evidence_count": count, "evidence_sha256": evidence_sha256}),
                    ),
                    ..ReceiptOutcome::default()
                },
            ))
        }
        .await;
        self.close_receipt(handle, result).await
    }

    /// Host-run analysis: Setup builds the prompt, the server's [`ModelClient`](crate::ModelClient)
    /// sends it, and Setup's validator checks the reply before it is retained. A reply cannot
    /// choose its analysis ID, skip its evidence bindings or confirm itself. One correction turn is
    /// allowed when the validator names a correctable defect, as the desktop does.
    pub async fn analyze_setup(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        input: AnalysisInput,
        source: Arc<SourceResolution>,
        ctx: OpContext,
    ) -> Result<AnalysisOutcome, SetupError> {
        self.access(account, project, Permission::CreateTask)
            .await?;
        let active = self.session(account, project, session)?;
        let evidence_refs: Vec<Value> = input
            .evidence
            .iter()
            .map(|e| json!({"reference": e.reference, "path": e.path, "excerpt_sha256": e.excerpt_sha256}))
            .collect();
        let request_json = json_request(json!({
            "provider": input.provider,
            "model": input.model,
            "reasoning_effort": input.reasoning_effort,
            "endpoint": input.endpoint,
            "credential": input.credential.as_ref().map(|c| json!({"name": c.name, "provider": c.provider, "reference": c.reference})),
            "mode": input.mode,
            "scan_id": input.scan_id,
            "purpose": input.purpose,
            "evidence": evidence_refs,
            "source_revision": source.identity.resolved_revision,
            "source_manifest_sha256": source.identity.manifest_sha256,
        }));
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::AnalyzeSetup,
                &ctx,
                request_json,
                input.scan_id,
                None,
                None,
                None,
            )
            .await?;
        let receipt_id = handle.id;
        let result = self.analyze_inner(&active, input, source).await;
        let mut out = self.close_receipt(handle, result).await?;
        out.receipt_id = receipt_id;
        Ok(out)
    }

    async fn analyze_inner(
        &self,
        active: &Arc<ActiveSession>,
        input: AnalysisInput,
        source: Arc<SourceResolution>,
    ) -> Result<(AnalysisOutcome, ReceiptOutcome), SetupError> {
        let model = self
            .inner
            .model
            .clone()
            .ok_or_else(|| SetupError::CapabilityUnavailable {
                capability: "model_client",
                reason: "the server supplied no ModelClient; analysis is not available".into(),
            })?;
        let request = AiAnalysisRequest {
            provider: input.provider.clone(),
            model: input.model.clone(),
            reasoning_effort: input.reasoning_effort.clone(),
            endpoint: input.endpoint.clone(),
            analysis: CodexAnalysisRequest {
                mode: input.mode.clone(),
                brief: input.brief.clone(),
                evidence: input.evidence.clone(),
                constraints: input.constraints.clone(),
                analysis_purpose: input.purpose.clone(),
                project_root: input.scan_id.map(|_| active.mod_root.display().to_string()),
                scan_id: input.scan_id,
            },
        };
        let turn = {
            let facade = active.facade.clone();
            let (req, cred, src) = (request.clone(), input.credential.clone(), source.clone());
            blocking(move || facade.analysis_turn(&req, cred, &src))
                .await
                .map_err(|e| map_native(e, &active.redactor))?
        };
        let (system_prompt, output_schema) =
            analysis_contract(&input.model).map_err(|e| SetupError::Model(e.to_string()))?;
        let mut prompt = turn.prompt.clone();
        let mut attempts = 0u32;
        let (result, reply) = loop {
            attempts += 1;
            let reply = model
                .complete(ModelRequest {
                    provider: input.provider.clone(),
                    model: input.model.clone(),
                    reasoning_effort: input.reasoning_effort.clone(),
                    endpoint: input.endpoint.clone(),
                    prompt: prompt.clone(),
                    system_prompt: system_prompt.clone(),
                    output_schema: output_schema.clone(),
                    input_sha256: turn.input_sha256.clone(),
                    credential: input.credential.clone(),
                })
                .await
                .map_err(|e| SetupError::Model(active.redactor.text(&e.to_string())))?;
            let facade = active.facade.clone();
            let (req, cred, src, value) = (
                request.clone(),
                input.credential.clone(),
                source.clone(),
                reply.reply.clone(),
            );
            match blocking(move || facade.record_analysis(req, cred, &src, value)).await {
                Ok(result) => break (result, reply),
                Err(error) => {
                    let reason = correctable_output_error(&error).map(str::to_owned);
                    match reason {
                        Some(reason) if attempts < ANALYSIS_ATTEMPTS => {
                            prompt = format!(
                                "{}\n\n{}",
                                turn.prompt,
                                corrective_analysis_prompt(&reason, &turn.input_sha256)
                            );
                        }
                        _ => {
                            return Err(SetupError::Model(format!(
                                "the reply was rejected by Setup's validator: {}",
                                active.redactor.text(&error.to_string())
                            )));
                        }
                    }
                }
            }
        };
        let record = &result.record;
        let analysis_id = record.analysis_id;
        let mode = serde_json::to_value(&result.analysis.mode)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| input.mode.clone());
        self.inner
            .store
            .insert_analysis(
                &active.scope,
                &AnalysisInsert {
                    id: analysis_id,
                    session_id: active.id.0,
                    scan_id: input.scan_id,
                    mode,
                    provider: input.provider.clone(),
                    model: input.model.clone(),
                    reasoning_effort: input.reasoning_effort.clone(),
                    input_sha256: record.input_sha256.clone(),
                    output_sha256: record.output_sha256.clone(),
                    source_revision: record.source_revision.clone(),
                    source_manifest_sha256: record.source_manifest_sha256.clone(),
                    record: serde_json::to_value(record)
                        .map_err(|e| SetupError::Internal(e.to_string()))?,
                    proposals: serde_json::to_value(&result.analysis.proposals)
                        .map_err(|e| SetupError::Internal(e.to_string()))?,
                    model_response_id: reply.response_id.clone(),
                    usage: reply.usage.clone(),
                    attempts: i32::try_from(attempts).unwrap_or(i32::MAX),
                    created_at: self.inner.store.now(),
                },
            )
            .await?;
        let outcome = ReceiptOutcome {
            result: Some(json!({
                "analysis_id": analysis_id,
                "input_sha256": record.input_sha256,
                "output_sha256": record.output_sha256,
                "attempts": attempts,
                "response_id": reply.response_id,
                "usage": reply.usage,
            })),
            ..ReceiptOutcome::default()
        };
        Ok((
            AnalysisOutcome {
                analysis_id,
                result,
                receipt_id: Uuid::nil(),
                attempts,
            },
            outcome,
        ))
    }

    /// Confirm a retained analysis with the exact values the reviewer saw (`reviewed_state` is the
    /// state the plan will later be built from). Only an analysis this session retained can be
    /// confirmed.
    #[allow(clippy::too_many_arguments)]
    pub async fn confirm_setup_analysis(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        record: CodexAnalysisRecord,
        confirmed_fields: Vec<String>,
        reviewed_state: &Value,
        ctx: OpContext,
    ) -> Result<CodexAnalysisRecord, SetupError> {
        self.access(account, project, Permission::CreateTask)
            .await?;
        let active = self.session(account, project, session)?;
        let values = confirmation_values(reviewed_state);
        let values_sha256 = sha256_json(&values);
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::ConfirmSetupAnalysis,
                &ctx,
                json_request(json!({
                    "analysis_id": record.analysis_id,
                    "confirmed_fields": confirmed_fields,
                    "confirmed_values_sha256": values_sha256,
                })),
                Some(record.analysis_id),
                None,
                None,
                None,
            )
            .await?;
        let result = async {
            let facade = active.facade.clone();
            let (rec, fields) = (record.clone(), confirmed_fields.clone());
            let confirmed = blocking(move || facade.confirm_analysis(&rec, &fields, &values))
                .await
                .map_err(|e| map_native(e, &active.redactor))?;
            self.inner
                .store
                .mark_analysis_confirmed(
                    &active.scope,
                    record.analysis_id,
                    &json!(confirmed_fields),
                    &values_sha256,
                    &serde_json::to_value(&confirmed)
                        .map_err(|e| SetupError::Internal(e.to_string()))?,
                )
                .await?;
            Ok::<_, SetupError>((
                confirmed,
                ReceiptOutcome {
                    result: Some(json!({"analysis_id": record.analysis_id, "confirmed_values_sha256": values_sha256})),
                    ..ReceiptOutcome::default()
                },
            ))
        }
        .await;
        self.close_receipt(handle, result).await
    }
}
