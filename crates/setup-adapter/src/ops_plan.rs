//! planSetup, resolveSetupConflicts, approveSetupPlan and reading the current plan.

use std::sync::Arc;

use factory_accounts::Permission;
use hoi4_mod_setup::service::plan_sha256;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::adapter::{
    ActiveSession, SetupAdapter, blocking, json_request, map_native, sha256_json,
};
use crate::error::SetupError;
use crate::ids::{AccountId, OnboardingSessionId, OpContext, ProjectId};
use crate::store::{ApprovalInsert, PlanInsert, ReceiptOutcome, SetupOperation};
use crate::views::{ApprovalView, ConflictDecision, PlanView};

impl SetupAdapter {
    /// Builds an installation plan from the reviewed state. It needs a confirmed analysis, which
    /// Setup finds in this session, and a plan builder. Without a builder the operation reports
    /// `capability_unavailable`; no plan is invented.
    pub async fn plan_setup(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        state: Value,
        ctx: OpContext,
    ) -> Result<PlanView, SetupError> {
        self.access(account, project, Permission::CreateTask)
            .await?;
        let active = self.session(account, project, session)?;
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::PlanSetup,
                &ctx,
                json_request(json!({
                    "state_sha256": sha256_json(&state),
                    "analysis_id": state.pointer("/codexAnalysisRecord/analysis_id"),
                })),
                None,
                None,
                None,
                None,
            )
            .await?;
        let receipt_id = handle.id;
        let result = self.plan_inner(&active, state).await;
        let mut view = self.close_receipt(handle, result).await?;
        view.receipt_id = Some(receipt_id);
        Ok(view)
    }

    async fn plan_inner(
        &self,
        active: &Arc<ActiveSession>,
        state: Value,
    ) -> Result<(PlanView, ReceiptOutcome), SetupError> {
        if !self.inner.has_plan_builder {
            return Err(SetupError::CapabilityUnavailable {
                capability: "plan_builder",
                reason: format!(
                    "no PlanBuilder is configured: Setup revision {} ships no default builder (its extraction pull request is not merged) and the host injected none",
                    crate::pin::SETUP_PIN.short()
                ),
            });
        }
        let facade = active.facade.clone();
        let plan = blocking(move || facade.build_plan(&state))
            .await
            .map_err(|e| map_native(e, &active.redactor))?;
        let plan_hash = plan_sha256(&plan).map_err(|e| map_native(e, &active.redactor))?;
        let root_identity = plan
            .transaction
            .project_root_identity
            .clone()
            .unwrap_or_default();
        let analysis_id = plan.codex_analysis.as_ref().map(|r| r.analysis_id);
        let analysis = match analysis_id {
            Some(id) => self.inner.store.analysis(&active.scope, id).await?,
            None => None,
        };
        let plan_json = active
            .redactor
            .value(&serde_json::to_value(&plan).map_err(|e| SetupError::Internal(e.to_string()))?);
        let operation_count = i32::try_from(plan.operations.len()).unwrap_or(i32::MAX);
        let insert = PlanInsert {
            id: plan.plan_id,
            session_id: active.id.0,
            analysis_id: analysis.as_ref().map(|a| a.id),
            scan_id: analysis.as_ref().and_then(|a| a.scan_id),
            plan_sha256: plan_hash.clone(),
            root_identity: root_identity.clone(),
            plan: plan_json.clone(),
            operation_count,
            created_at: self.inner.store.now(),
        };
        self.inner.store.insert_plan(&active.scope, &insert).await?;
        let row = self
            .inner
            .store
            .plan(&active.scope, plan.plan_id)
            .await?
            .ok_or_else(|| SetupError::Internal("the plan row was not stored".into()))?;
        let outcome = ReceiptOutcome {
            result: Some(json!({
                "plan_id": plan.plan_id,
                "plan_sha256": plan_hash,
                "root_identity": root_identity,
                "operation_count": operation_count,
            })),
            ..ReceiptOutcome::default()
        };
        Ok((PlanView::from_row(row, plan_json, None), outcome))
    }

    /// Records keep, replace, rename, merge or skip decisions. Each decision changes the plan, so
    /// the result carries a new plan hash and any approval already given is withdrawn.
    pub async fn resolve_setup_conflicts(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        plan_id: Uuid,
        decisions: Vec<ConflictDecision>,
        ctx: OpContext,
    ) -> Result<PlanView, SetupError> {
        self.access(account, project, Permission::CreateTask)
            .await?;
        let active = self.session(account, project, session)?;
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::ResolveSetupConflicts,
                &ctx,
                json_request(json!({
                    "plan_id": plan_id,
                    "decisions": decisions.iter().map(|d| json!({"path": d.path, "choice": d.choice})).collect::<Vec<_>>(),
                })),
                Some(plan_id),
                None,
                None,
                None,
            )
            .await?;
        let receipt_id = handle.id;
        let result = async {
            if decisions.is_empty() {
                return Err(SetupError::precondition(
                    "no_decisions",
                    "At least one conflict decision is required.",
                ));
            }
            let row = self
                .inner
                .store
                .plan(&active.scope, plan_id)
                .await?
                .filter(|r| r.session_id == active.id.0)
                .ok_or(SetupError::NotFound)?;
            let previous_hash = row.plan_sha256.clone();
            let mut latest = None;
            for decision in &decisions {
                let facade = active.facade.clone();
                let (path, choice) = (decision.path.clone(), decision.choice.clone());
                latest = Some(
                    blocking(move || facade.decide_conflict(plan_id, &path, &choice))
                        .await
                        .map_err(|e| map_native(e, &active.redactor))?,
                );
            }
            let plan = latest.expect("at least one decision");
            let new_hash = plan_sha256(&plan).map_err(|e| map_native(e, &active.redactor))?;
            let plan_json = active.redactor.value(
                &serde_json::to_value(&plan).map_err(|e| SetupError::Internal(e.to_string()))?,
            );
            let mut all = row.conflict_decisions.as_array().cloned().unwrap_or_default();
            for d in &decisions {
                all.push(json!({
                    "path": d.path,
                    "choice": d.choice,
                    "decided_by": account,
                    "plan_sha256_after": new_hash,
                }));
            }
            self.inner
                .store
                .record_conflict_decision(
                    &active.scope,
                    plan_id,
                    &new_hash,
                    &plan_json,
                    &Value::Array(all),
                )
                .await?;
            let row = self
                .inner
                .store
                .plan(&active.scope, plan_id)
                .await?
                .ok_or(SetupError::NotFound)?;
            Ok::<_, SetupError>((
                PlanView::from_row(row, plan_json, None),
                ReceiptOutcome {
                    result: Some(json!({"plan_id": plan_id, "previous_plan_sha256": previous_hash, "plan_sha256": new_hash})),
                    ..ReceiptOutcome::default()
                },
            ))
        }
        .await;
        let mut view = self.close_receipt(handle, result).await?;
        view.receipt_id = Some(receipt_id);
        Ok(view)
    }

    /// Approves exactly the plan the reviewer saw. `reviewed_plan_sha256` is the hash shown with
    /// the plan; if the stored plan differs nothing is approved. The approval binds the plan hash
    /// and the root identity, and `grant` fields in `ctx` are recorded with it.
    pub async fn approve_setup_plan(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        plan_id: Uuid,
        reviewed_plan_sha256: &str,
        ctx: OpContext,
    ) -> Result<ApprovalView, SetupError> {
        self.access(account, project, Permission::ApproveProposal)
            .await?;
        let active = self.session(account, project, session)?;
        let handle = self
            .open_receipt(
                &active.scope,
                Some(session.0),
                SetupOperation::ApproveSetupPlan,
                &ctx,
                json_request(json!({
                    "plan_id": plan_id,
                    "reviewed_plan_sha256": reviewed_plan_sha256,
                })),
                Some(plan_id),
                None,
                None,
                None,
            )
            .await?;
        let receipt_id = handle.id;
        let reviewed = reviewed_plan_sha256.to_owned();
        let result = async {
            let row = self
                .inner
                .store
                .plan(&active.scope, plan_id)
                .await?
                .filter(|r| r.session_id == active.id.0)
                .ok_or(SetupError::NotFound)?;
            if row.plan_sha256 != reviewed {
                return Err(SetupError::stale(
                    "plan_changed",
                    "The plan changed after it was reviewed; review it again.",
                ));
            }
            self.require_confirmed_analysis(&active, row.analysis_id)
                .await?;
            let facade = active.facade.clone();
            let reviewed_for_facade = reviewed.clone();
            let approved = blocking(move || facade.approve_plan(plan_id, &reviewed_for_facade))
                .await
                .map_err(|e| map_native(e, &active.redactor))?;
            let approval_id = Uuid::new_v4();
            self.inner
                .store
                .insert_approval(
                    &active.scope,
                    &ApprovalInsert {
                        id: approval_id,
                        plan_id,
                        session_id: active.id.0,
                        reviewed_plan_sha256: approved.reviewed_plan_sha256.clone(),
                        approved_plan_sha256: approved.approved_plan_sha256.clone(),
                        root_identity: approved.project_root_identity.clone(),
                        grant_id: ctx.grant_id.clone(),
                        revocation_epoch: ctx.revocation_epoch,
                        created_at: self.inner.store.now(),
                    },
                )
                .await?;
            Ok::<_, SetupError>((
                ApprovalView {
                    approval_id,
                    plan_id,
                    reviewed_plan_sha256: approved.reviewed_plan_sha256.clone(),
                    approved_plan_sha256: approved.approved_plan_sha256.clone(),
                    root_identity: approved.project_root_identity.clone(),
                    receipt_id: Uuid::nil(),
                },
                ReceiptOutcome {
                    result: Some(json!({
                        "approval_id": approval_id,
                        "reviewed_plan_sha256": approved.reviewed_plan_sha256,
                        "approved_plan_sha256": approved.approved_plan_sha256,
                        "root_identity": approved.project_root_identity,
                    })),
                    ..ReceiptOutcome::default()
                },
            ))
        }
        .await;
        let mut view = self.close_receipt(handle, result).await?;
        view.receipt_id = receipt_id;
        Ok(view)
    }

    /// Refuses unless the plan's analysis is recorded as confirmed.
    pub(crate) async fn require_confirmed_analysis(
        &self,
        active: &ActiveSession,
        analysis_id: Option<Uuid>,
    ) -> Result<(), SetupError> {
        let confirmed = match analysis_id {
            Some(id) => self
                .inner
                .store
                .analysis(&active.scope, id)
                .await?
                .is_some_and(|a| a.status == "confirmed"),
            None => false,
        };
        if confirmed {
            Ok(())
        } else {
            Err(SetupError::precondition(
                "analysis_not_confirmed",
                "A confirmed analysis is required before the plan can be approved or applied.",
            ))
        }
    }

    /// The latest plan of the project (or of one session) as the browser shows it, or `None`
    /// when no plan was built. Reads only.
    pub async fn current_plan(
        &self,
        account: AccountId,
        project: ProjectId,
        session: Option<OnboardingSessionId>,
    ) -> Result<Option<PlanView>, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        Ok(self
            .inner
            .store
            .latest_plan(&access.scope, session.map(|s| s.0))
            .await?
            .map(|(row, plan)| PlanView::from_row(row, plan, None)))
    }
}
