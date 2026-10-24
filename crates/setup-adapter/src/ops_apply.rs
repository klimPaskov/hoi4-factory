//! applySetupPlan, inspectRecovery and recoverSetup.
//!
//! Effects run under the project lease ([`ProjectLease`]). The C-04 intent is written before the
//! effect, and the outcome afterwards. Whatever the outcome, the native journal in the state root
//! decides what recovery is allowed; the database only projects it.

use std::path::{Path, PathBuf};

use factory_accounts::Permission;
use factory_workspace::SnapshotInventory;
use hoi4_mod_setup::models::TransactionJournal;
use hoi4_mod_setup::service::RecoveryAction;
use serde_json::json;
use uuid::Uuid;

use crate::adapter::{
    Access, ReceiptHandle, SetupAdapter, blocking, capture_inventory, json_request, map_native,
    resolve_mod_root,
};
use crate::error::SetupError;
use crate::ids::{AccountId, OnboardingSessionId, OpContext, ProjectId};
use crate::lease::ProjectLease;
use crate::pin::SETUP_PIN;
use crate::recovery::{RecoveryVariant, RecoveryView, journal_sha256};
use crate::store::{
    ReceiptOutcome, ReceiptStatus, Scope, SetupOperation, TransactionInsert, TransactionRow,
    TransactionUpdate,
};
use crate::views::{ApplyOutcome, ChangeSummary, RecoverOutcome, RestorationCheck};

/// Where the pre-apply inventory of a transaction is kept: beside the native journal's state
/// root, never inside `source/`.
fn pre_apply_inventory_path(state_dir: &Path, journal_id: Uuid) -> PathBuf {
    state_dir
        .join("adapter")
        .join(format!("pre-apply-{journal_id}.json"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().expect("a file has a parent");
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn load_inventory(path: &Path) -> Option<SnapshotInventory> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn terminal_state(journal_state: &str) -> Option<&'static str> {
    match journal_state {
        "completed" => Some("completed"),
        "rolled_back" => Some("rolled_back"),
        "staging_discarded" => Some("staging_discarded"),
        _ => None,
    }
}

/// Paths changed by apply that are neither a planned destination nor Setup-owned state.
fn unplanned(changes: &ChangeSummary, mod_root: &str, journal: &TransactionJournal) -> Vec<String> {
    let prefix = if mod_root.is_empty() {
        String::new()
    } else {
        format!("{mod_root}/")
    };
    let destinations: Vec<String> = journal
        .operations
        .iter()
        .map(|o| format!("{prefix}{}", o.destination.replace('\\', "/")))
        .collect();
    let setup_owned = format!("{prefix}.hoi4-mod-setup/");
    changes
        .paths()
        .filter(|path| {
            let p = path.replace('\\', "/");
            !p.starts_with(&setup_owned)
                && !destinations
                    .iter()
                    .any(|d| p == *d || p.starts_with(&format!("{d}/")))
        })
        .cloned()
        .collect()
}

/// A failed effect: the error to return and how to settle the receipt.
struct EffectFailure {
    error: SetupError,
    status: ReceiptStatus,
    outcome: ReceiptOutcome,
}

impl EffectFailure {
    fn new(error: SetupError, status: ReceiptStatus) -> Self {
        let outcome = ReceiptOutcome {
            error_code: Some(error.code().to_owned()),
            error_message: Some(error.to_string()),
            ..ReceiptOutcome::default()
        };
        Self {
            error,
            status,
            outcome,
        }
    }
}

impl From<SetupError> for EffectFailure {
    fn from(error: SetupError) -> Self {
        let status = if error.code() == "unknown_effect" {
            ReceiptStatus::UnknownEffect
        } else {
            ReceiptStatus::Failed
        };
        Self::new(error, status)
    }
}

impl SetupAdapter {
    async fn settle(
        &self,
        handle: &ReceiptHandle,
        status: ReceiptStatus,
        outcome: &ReceiptOutcome,
    ) {
        let mut outcome = ReceiptOutcome {
            before_inventory_sha256: outcome.before_inventory_sha256.clone(),
            after_inventory_sha256: outcome.after_inventory_sha256.clone(),
            result: outcome.result.clone(),
            result_sha256: outcome.result_sha256.clone(),
            error_code: outcome.error_code.clone(),
            error_message: outcome.error_message.clone(),
        };
        if let Some(r) = &outcome.result {
            outcome.result_sha256 = Some(crate::adapter::sha256_json(r));
        }
        if let Err(e) = self
            .inner
            .store
            .settle_receipt(&handle.scope, handle.id, status, &outcome)
            .await
        {
            tracing::error!(receipt = %handle.id, error = %e, "could not settle a setup receipt");
        }
    }

    /// Applies the approved plan. Order of events:
    ///
    /// 1. take the project lease (refused at once with `project_busy` if another owner has it);
    /// 2. refuse while an unfinished transaction exists (`recovery_required`), unless the plan is
    ///    approved with a confirmed analysis (`plan_not_approved`, `analysis_not_confirmed`);
    /// 3. write the C-04 intent and the transaction row, take the pre-apply inventory;
    /// 4. call Setup, which rechecks the approval, the plan hash, the root identity and the
    ///    confirmed analysis itself and consumes the approval;
    /// 5. record the outcome from the native journal, never from the error text.
    pub async fn apply_setup_plan(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
        plan_id: Uuid,
        ctx: OpContext,
    ) -> Result<ApplyOutcome, SetupError> {
        let access = self
            .access(account, project, Permission::ApproveProposal)
            .await?;
        let active = self.session(account, project, session)?;
        let request = json_request(json!({ "plan_id": plan_id }));
        let lease =
            match ProjectLease::try_acquire(self.inner.store.database(), account, project).await {
                Ok(lease) => lease,
                Err(e) => {
                    self.record_refusal(
                        &active.scope,
                        Some(session.0),
                        SetupOperation::ApplySetupPlan,
                        &ctx,
                        request,
                        Some(plan_id),
                        &e,
                    )
                    .await;
                    return Err(e);
                }
            };
        let result = self
            .apply_locked(&access, &active, plan_id, &ctx, request, &lease)
            .await;
        if let Err(e) = lease.release().await {
            tracing::error!(error = %e, "could not release the project lease");
        }
        result
    }

    async fn apply_locked(
        &self,
        access: &Access,
        active: &crate::adapter::ActiveSession,
        plan_id: Uuid,
        ctx: &OpContext,
        request: serde_json::Value,
        lease: &ProjectLease,
    ) -> Result<ApplyOutcome, SetupError> {
        // Gates before any intent: a refusal is recorded as a refused receipt.
        let gate = async {
            if let Some(row) = self
                .inner
                .store
                .unfinished_transaction(&active.scope)
                .await?
            {
                return Err(SetupError::RecoveryRequired {
                    journal_id: row.journal_id,
                    variant: row
                        .recovery_variant
                        .as_deref()
                        .and_then(RecoveryVariant::parse)
                        .unwrap_or(RecoveryVariant::ManualReview),
                });
            }
            let facade = active.facade.clone();
            let root = active.mod_root.clone();
            if let Some(status) = blocking(move || facade.inspect_recovery(&root))
                .await
                .map_err(|e| map_native(e, &active.redactor))?
            {
                return Err(SetupError::RecoveryRequired {
                    journal_id: status.journal.transaction_id,
                    variant: self.view_of(&active.scope, &status).await?.variant,
                });
            }
            let row = self
                .inner
                .store
                .plan(&active.scope, plan_id)
                .await?
                .filter(|r| r.session_id == active.id.0 && r.state == "approved")
                .ok_or_else(|| {
                    SetupError::precondition(
                        "plan_not_approved",
                        "The plan is not approved. Review and approve it before it is applied.",
                    )
                })?;
            self.inner
                .store
                .active_approval(&active.scope, plan_id)
                .await?
                .ok_or_else(|| {
                    SetupError::precondition(
                        "plan_not_approved",
                        "The approval was withdrawn or consumed. Approve the plan again.",
                    )
                })?;
            self.require_confirmed_analysis(active, row.analysis_id)
                .await?;
            Ok::<_, SetupError>(row)
        }
        .await;
        let row = match gate {
            Ok(row) => row,
            Err(e) => {
                self.record_refusal(
                    &active.scope,
                    Some(active.id.0),
                    SetupOperation::ApplySetupPlan,
                    ctx,
                    request,
                    Some(plan_id),
                    &e,
                )
                .await;
                return Err(e);
            }
        };

        let key = ctx
            .idempotency_key
            .clone()
            .unwrap_or_else(|| format!("apply:{plan_id}"));
        let handle = self
            .open_receipt(
                &active.scope,
                Some(active.id.0),
                SetupOperation::ApplySetupPlan,
                ctx,
                request,
                Some(plan_id),
                Some(row.root_identity.clone()),
                Some(lease.generation()),
                Some(key),
            )
            .await?;
        match self
            .apply_effect(
                access,
                active,
                plan_id,
                &row.plan_sha256,
                &row.root_identity,
                lease,
            )
            .await
        {
            Ok((mut outcome, receipt)) => {
                self.settle(&handle, ReceiptStatus::Succeeded, &receipt)
                    .await;
                outcome.receipt_id = handle.id;
                Ok(outcome)
            }
            Err(failure) => {
                self.settle(&handle, failure.status, &failure.outcome).await;
                Err(failure.error)
            }
        }
    }

    async fn apply_effect(
        &self,
        access: &Access,
        active: &crate::adapter::ActiveSession,
        plan_id: Uuid,
        plan_sha256: &str,
        root_identity: &str,
        lease: &ProjectLease,
    ) -> Result<(ApplyOutcome, ReceiptOutcome), EffectFailure> {
        let scope = active.scope;
        let before = capture_inventory(&active.workspace).await?;
        let before_hash = before.content_hash();
        let pre_path = pre_apply_inventory_path(&access.state_dir, plan_id);
        let bytes = serde_json::to_vec(&before).map_err(|e| SetupError::Internal(e.to_string()))?;
        write_atomic(&pre_path, &bytes).map_err(|e| {
            SetupError::Workspace(format!("cannot keep the pre-apply inventory: {e}"))
        })?;
        let tx_id = Uuid::new_v4();
        self.inner
            .store
            .insert_transaction(
                &scope,
                &TransactionInsert {
                    id: tx_id,
                    journal_id: plan_id,
                    plan_id: Some(plan_id),
                    session_id: Some(active.id.0),
                    mod_root: active.mod_root_rel.clone(),
                    state: "applying",
                    plan_sha256: Some(plan_sha256.to_owned()),
                    root_identity: Some(root_identity.to_owned()),
                    pre_inventory_sha256: Some(before_hash.clone()),
                    fence_generation: Some(lease.generation()),
                    holder: self.inner.holder.clone(),
                    started_at: self.inner.store.now(),
                },
            )
            .await?;
        self.inner
            .store
            .set_plan_state(&scope, plan_id, "applying")
            .await?;

        let facade = active.facade.clone();
        let applied = blocking(move || facade.apply(plan_id)).await;
        let after = capture_inventory(&active.workspace).await;
        let after_hash = after.as_ref().ok().map(|i| i.content_hash());

        match applied {
            Ok(journal) => {
                let after = after?;
                let diff = before.diff(&after);
                let changes = ChangeSummary::from(&diff);
                let unplanned_changes = unplanned(&changes, &active.mod_root_rel, &journal);
                let state = terminal_state(&journal.state).unwrap_or("unknown");
                self.inner
                    .store
                    .update_transaction(
                        &scope,
                        tx_id,
                        &TransactionUpdate {
                            state: Some(state),
                            journal_state: Some(journal.state.clone()),
                            project_apply_started: Some(true),
                            journal_sha256: Some(journal_sha256(&journal)),
                            post_inventory_sha256: after_hash.clone(),
                            finished: state != "unknown",
                            ..TransactionUpdate::default()
                        },
                    )
                    .await?;
                self.inner
                    .store
                    .consume_approval(&scope, plan_id, "applied")
                    .await?;
                let outcome = ApplyOutcome {
                    plan_id,
                    journal_id: journal.transaction_id,
                    journal_state: journal.state.clone(),
                    receipt_id: Uuid::nil(),
                    fence_generation: lease.generation(),
                    inventory_before_sha256: before_hash.clone(),
                    inventory_after_sha256: after.content_hash(),
                    changes: changes.clone(),
                    unplanned_changes: unplanned_changes.clone(),
                    setup: SETUP_PIN,
                };
                let receipt = ReceiptOutcome {
                    before_inventory_sha256: Some(before_hash),
                    after_inventory_sha256: after_hash,
                    result: Some(json!({
                        "journal_id": journal.transaction_id,
                        "journal_state": journal.state,
                        "journal_sha256": journal_sha256(&journal),
                        "changes": changes,
                        "unplanned_changes": unplanned_changes,
                        "setup_revision": SETUP_PIN.revision,
                        "setup_has_pr105_fix": SETUP_PIN.contains_pr_105_fix,
                    })),
                    ..ReceiptOutcome::default()
                };
                Ok((outcome, receipt))
            }
            Err(native) => {
                let cause = active.redactor.text(&native.to_string());
                // The approval is spent whatever happened.
                let spent = self
                    .inner
                    .store
                    .consume_approval(&scope, plan_id, "apply_failed")
                    .await;
                if let Err(e) = spent {
                    tracing::error!(error = %e, "could not record the spent approval");
                }
                let mut receipt = ReceiptOutcome {
                    before_inventory_sha256: Some(before_hash.clone()),
                    after_inventory_sha256: after_hash.clone(),
                    error_code: Some("apply_failed".into()),
                    error_message: Some(cause.clone()),
                    ..ReceiptOutcome::default()
                };
                // What really happened comes from the native journal.
                let facade = active.facade.clone();
                let root = active.mod_root.clone();
                let incomplete = blocking(move || facade.inspect_recovery(&root)).await;
                match incomplete {
                    Ok(Some(status)) => {
                        let view = self.view_of(&scope, &status).await?;
                        let _ = self
                            .inner
                            .store
                            .update_transaction(
                                &scope,
                                tx_id,
                                &projection_update(&view, "recovery_required", after_hash),
                            )
                            .await;
                        receipt.result = Some(json!({"recovery": view}));
                        Err(EffectFailure {
                            error: SetupError::ApplyInterrupted {
                                cause,
                                journal_id: view.journal_id,
                                variant: view.variant,
                            },
                            status: ReceiptStatus::UnknownEffect,
                            outcome: receipt,
                        })
                    }
                    Ok(None) => {
                        let facade = active.facade.clone();
                        let root = active.mod_root.clone();
                        let journal = blocking(move || facade.read_journal(&root, plan_id)).await;
                        match journal {
                            Ok(journal) => {
                                let state = terminal_state(&journal.state).unwrap_or("unknown");
                                let _ = self
                                    .inner
                                    .store
                                    .update_transaction(
                                        &scope,
                                        tx_id,
                                        &TransactionUpdate {
                                            state: Some(state),
                                            journal_state: Some(journal.state.clone()),
                                            journal_sha256: Some(journal_sha256(&journal)),
                                            post_inventory_sha256: after_hash,
                                            finished: state != "unknown",
                                            ..TransactionUpdate::default()
                                        },
                                    )
                                    .await;
                                receipt.result = Some(json!({"journal_state": journal.state}));
                                Err(EffectFailure {
                                    error: SetupError::Native {
                                        kind: crate::error::native_kind(&native),
                                        message: cause,
                                    },
                                    status: ReceiptStatus::Failed,
                                    outcome: receipt,
                                })
                            }
                            Err(_) => {
                                // No journal: the failure was before the transaction began. The
                                // inventories say whether anything changed anyway.
                                let untouched = after
                                    .as_ref()
                                    .map(|a| before.diff(a).is_empty())
                                    .unwrap_or(false);
                                let state = if untouched {
                                    "refused_before_effect"
                                } else {
                                    "unknown"
                                };
                                let _ = self
                                    .inner
                                    .store
                                    .update_transaction(
                                        &scope,
                                        tx_id,
                                        &TransactionUpdate {
                                            state: Some(state),
                                            post_inventory_sha256: after_hash,
                                            finished: untouched,
                                            ..TransactionUpdate::default()
                                        },
                                    )
                                    .await;
                                let _ = self
                                    .inner
                                    .store
                                    .set_plan_state(&scope, plan_id, "apply_failed")
                                    .await;
                                if untouched {
                                    let error = map_native(native, &active.redactor);
                                    Err(EffectFailure {
                                        error,
                                        status: ReceiptStatus::Refused,
                                        outcome: receipt,
                                    })
                                } else {
                                    Err(EffectFailure {
                                        error: SetupError::UnknownEffect(format!(
                                            "setup failed without a journal, but the workspace inventory changed: {cause}"
                                        )),
                                        status: ReceiptStatus::UnknownEffect,
                                        outcome: receipt,
                                    })
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = self
                            .inner
                            .store
                            .update_transaction(
                                &scope,
                                tx_id,
                                &TransactionUpdate {
                                    state: Some("unknown"),
                                    ..TransactionUpdate::default()
                                },
                            )
                            .await;
                        Err(EffectFailure {
                            error: SetupError::UnknownEffect(format!(
                                "setup failed ({cause}) and the journal could not be read: {}",
                                active.redactor.text(&e.to_string())
                            )),
                            status: ReceiptStatus::UnknownEffect,
                            outcome: receipt,
                        })
                    }
                }
            }
        }
    }

    // ----- recovery ----------------------------------------------------------------------

    /// The view of an incomplete transaction, with the adapter's own memory applied: a
    /// finalizing journal whose resume already failed for lack of a committed lock stays manual
    /// review for as long as the journal is byte-identical to the one that failed.
    async fn view_of(
        &self,
        scope: &Scope,
        status: &hoi4_mod_setup::service::RecoveryStatus,
    ) -> Result<RecoveryView, SetupError> {
        let view = RecoveryView::from_status(status);
        if view.variant == RecoveryVariant::VerifiableFinalization {
            let known = self
                .inner
                .store
                .transaction_by_journal(scope, view.journal_id)
                .await?;
            if known.is_some_and(|row| {
                row.recovery_variant.as_deref() == Some(RecoveryVariant::ManualReview.as_str())
                    && row.journal_sha256.as_deref() == Some(view.journal_sha256.as_str())
            }) {
                return Ok(view.with_variant(RecoveryVariant::ManualReview));
            }
        }
        Ok(view)
    }

    async fn recovery_root(&self, access: &Access) -> Result<(String, PathBuf), SetupError> {
        let rel = self
            .inner
            .store
            .latest_mod_root(&access.scope)
            .await?
            .unwrap_or_default();
        resolve_mod_root(&access.workspace, Some(&rel))
    }

    /// Reads the native journal and returns what recovery is allowed, or `None` when no
    /// transaction waits. Needs no onboarding session and no model. While another owner holds
    /// the project lease the view says so and offers no action.
    pub async fn inspect_recovery(
        &self,
        account: AccountId,
        project: ProjectId,
    ) -> Result<Option<RecoveryView>, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        let (_, root) = self.recovery_root(&access).await?;
        let facade = std::sync::Arc::new(self.recovery_facade(&access)?);
        let f = facade.clone();
        let r = root.clone();
        let status = blocking(move || f.inspect_recovery(&r))
            .await
            .map_err(|e| map_native(e, &access.redactor))?;
        let lease =
            match ProjectLease::try_acquire(self.inner.store.database(), account, project).await {
                Ok(l) => l,
                Err(SetupError::ProjectBusy { .. }) => {
                    return Ok(status.map(|s| {
                        let mut view = RecoveryView::from_status(&s);
                        view.owner_active = true;
                        view.allowed_action = None;
                        view
                    }));
                }
                Err(e) => return Err(e),
            };
        let result = self
            .reconcile_projection(&access, &facade, &root, status.as_ref())
            .await;
        let _ = lease.release().await;
        result
    }

    /// With the lease held, bring the transaction row in line with the native journal.
    async fn reconcile_projection(
        &self,
        access: &Access,
        facade: &std::sync::Arc<hoi4_mod_setup::service::SetupSession>,
        root: &Path,
        status: Option<&hoi4_mod_setup::service::RecoveryStatus>,
    ) -> Result<Option<RecoveryView>, SetupError> {
        let scope = access.scope;
        let row = self.inner.store.unfinished_transaction(&scope).await?;
        match status {
            Some(status) => {
                let view = self.view_of(&scope, status).await?;
                let update = projection_update(&view, "recovery_required", None);
                match row {
                    Some(row) if row.journal_id == view.journal_id => {
                        self.inner
                            .store
                            .update_transaction(&scope, row.id, &update)
                            .await?;
                    }
                    _ => {
                        // A journal with no matching row (the row was lost, or the journal came
                        // from another owner). Adopt it so the project shows as unfinished.
                        let known = self
                            .inner
                            .store
                            .transaction_by_journal(&scope, view.journal_id)
                            .await?;
                        if let Some(known) = known {
                            self.inner
                                .store
                                .update_transaction(&scope, known.id, &update)
                                .await?;
                        } else {
                            let id = Uuid::new_v4();
                            let rel = self
                                .inner
                                .store
                                .latest_mod_root(&scope)
                                .await?
                                .unwrap_or_default();
                            self.inner
                                .store
                                .insert_transaction(
                                    &scope,
                                    &TransactionInsert {
                                        id,
                                        journal_id: view.journal_id,
                                        plan_id: None,
                                        session_id: None,
                                        mod_root: rel,
                                        state: "recovery_required",
                                        plan_sha256: view.plan_sha256.clone(),
                                        root_identity: None,
                                        pre_inventory_sha256: None,
                                        fence_generation: None,
                                        holder: format!("adopted:{}", self.inner.holder),
                                        started_at: self.inner.store.now(),
                                    },
                                )
                                .await?;
                            self.inner
                                .store
                                .update_transaction(&scope, id, &update)
                                .await?;
                        }
                    }
                }
                Ok(Some(view))
            }
            None => {
                if let Some(row) = row {
                    self.reconcile_finished(access, facade, root, &row).await?;
                }
                Ok(None)
            }
        }
    }

    /// The row says unfinished, the native side shows no incomplete transaction: read the journal
    /// by ID, or compare inventories when there is none.
    async fn reconcile_finished(
        &self,
        access: &Access,
        facade: &std::sync::Arc<hoi4_mod_setup::service::SetupSession>,
        root: &Path,
        row: &TransactionRow,
    ) -> Result<(), SetupError> {
        let scope = access.scope;
        let (f, r, id) = (facade.clone(), root.to_path_buf(), row.journal_id);
        match blocking(move || f.read_journal(&r, id)).await {
            Ok(journal) => {
                let state = terminal_state(&journal.state).unwrap_or("unknown");
                self.inner
                    .store
                    .update_transaction(
                        &scope,
                        row.id,
                        &TransactionUpdate {
                            state: Some(state),
                            journal_state: Some(journal.state.clone()),
                            journal_sha256: Some(journal_sha256(&journal)),
                            finished: state != "unknown",
                            ..TransactionUpdate::default()
                        },
                    )
                    .await?;
                if let Some(plan_id) = row.plan_id {
                    let plan_state = match state {
                        "completed" => "applied",
                        _ => "apply_failed",
                    };
                    self.inner
                        .store
                        .set_plan_state(&scope, plan_id, plan_state)
                        .await?;
                }
            }
            Err(_) => {
                let pre =
                    load_inventory(&pre_apply_inventory_path(&access.state_dir, row.journal_id));
                let now = capture_inventory(&access.workspace).await?;
                let untouched = pre.is_some_and(|p| p.diff(&now).is_empty());
                self.inner
                    .store
                    .update_transaction(
                        &scope,
                        row.id,
                        &TransactionUpdate {
                            state: Some(if untouched {
                                "refused_before_effect"
                            } else {
                                "unknown"
                            }),
                            post_inventory_sha256: Some(now.content_hash()),
                            finished: untouched,
                            ..TransactionUpdate::default()
                        },
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Takes the one recovery action the native journal permits for `journal_id`. The caller
    /// names the variant it was shown. If the journal now allows something else the call is
    /// refused as stale; the variant never widens into a generic resume.
    pub async fn recover_setup(
        &self,
        account: AccountId,
        project: ProjectId,
        journal_id: Uuid,
        expected: RecoveryVariant,
        ctx: OpContext,
    ) -> Result<RecoverOutcome, SetupError> {
        let access = self
            .access(account, project, Permission::ApproveProposal)
            .await?;
        let scope = access.scope;
        let request = json_request(json!({
            "journal_id": journal_id,
            "variant": expected.as_str(),
            "action": expected.action().map(RecoveryAction::as_str),
        }));
        let lease =
            match ProjectLease::try_acquire(self.inner.store.database(), account, project).await {
                Ok(l) => l,
                Err(e) => {
                    self.record_refusal(
                        &scope,
                        None,
                        SetupOperation::RecoverSetup,
                        &ctx,
                        request,
                        Some(journal_id),
                        &e,
                    )
                    .await;
                    return Err(e);
                }
            };
        let result = self
            .recover_locked(&access, journal_id, expected, &ctx, request, &lease)
            .await;
        let _ = lease.release().await;
        result
    }

    async fn recover_locked(
        &self,
        access: &Access,
        journal_id: Uuid,
        expected: RecoveryVariant,
        ctx: &OpContext,
        request: serde_json::Value,
        lease: &ProjectLease,
    ) -> Result<RecoverOutcome, SetupError> {
        let scope = access.scope;
        let (rel, root) = self.recovery_root(access).await?;
        let facade = std::sync::Arc::new(self.recovery_facade(access)?);
        let gate = async {
            let (f, r) = (facade.clone(), root.clone());
            let status = blocking(move || f.inspect_recovery(&r))
                .await
                .map_err(|e| map_native(e, &access.redactor))?
                .filter(|s| s.journal.transaction_id == journal_id)
                .ok_or_else(|| {
                    SetupError::stale(
                        "no_incomplete_transaction",
                        "No incomplete setup transaction with that ID waits for recovery.",
                    )
                })?;
            let current = self.view_of(&scope, &status).await?.variant;
            if current != expected {
                return Err(SetupError::stale(
                    "recovery_variant_changed",
                    format!(
                        "The transaction is now `{}`, not `{}`. Review it again.",
                        current.as_str(),
                        expected.as_str()
                    ),
                ));
            }
            let action = current.action().ok_or_else(|| {
                SetupError::precondition(
                    "manual_review_required",
                    "This transaction permits no automatic recovery.",
                )
            })?;
            Ok::<_, SetupError>((status, action))
        }
        .await;
        let (status, action) = match gate {
            Ok(x) => x,
            Err(e) => {
                self.record_refusal(
                    &scope,
                    None,
                    SetupOperation::RecoverSetup,
                    ctx,
                    request,
                    Some(journal_id),
                    &e,
                )
                .await;
                return Err(e);
            }
        };
        let handle = self
            .open_receipt(
                &scope,
                None,
                SetupOperation::RecoverSetup,
                ctx,
                request,
                Some(journal_id),
                None,
                Some(lease.generation()),
                ctx.idempotency_key.clone(),
            )
            .await?;

        // Make sure a row exists, then mark it as being recovered.
        let row = match self
            .inner
            .store
            .transaction_by_journal(&scope, journal_id)
            .await?
        {
            Some(row) => row,
            None => {
                let view = RecoveryView::from_status(&status);
                let id = Uuid::new_v4();
                self.inner
                    .store
                    .insert_transaction(
                        &scope,
                        &TransactionInsert {
                            id,
                            journal_id,
                            plan_id: None,
                            session_id: None,
                            mod_root: rel.clone(),
                            state: "recovering",
                            plan_sha256: view.plan_sha256,
                            root_identity: None,
                            pre_inventory_sha256: None,
                            fence_generation: Some(lease.generation()),
                            holder: format!("adopted:{}", self.inner.holder),
                            started_at: self.inner.store.now(),
                        },
                    )
                    .await?;
                self.inner
                    .store
                    .transaction_by_journal(&scope, journal_id)
                    .await?
                    .ok_or_else(|| SetupError::Internal("the adopted row vanished".into()))?
            }
        };
        self.inner
            .store
            .update_transaction(
                &scope,
                row.id,
                &TransactionUpdate {
                    state: Some("recovering"),
                    fence_generation: Some(lease.generation()),
                    ..TransactionUpdate::default()
                },
            )
            .await?;

        let before = capture_inventory(&access.workspace).await;
        let before_hash = before.as_ref().ok().map(|i| i.content_hash());
        let (f, r) = (facade.clone(), root.clone());
        let done = blocking(move || f.recover(&r, journal_id, action)).await;
        let after = capture_inventory(&access.workspace).await;
        let after_hash = after.as_ref().ok().map(|i| i.content_hash());

        // Whatever the result, read the journal again for what remains.
        let (f, r) = (facade.clone(), root.clone());
        let remaining_status = blocking(move || f.inspect_recovery(&r)).await;
        let remaining = match &remaining_status {
            Ok(Some(s)) => Some(self.view_of(&scope, s).await?),
            _ => None,
        };

        match done {
            Ok(journal) => {
                let state = terminal_state(&journal.state).unwrap_or(if remaining.is_some() {
                    "recovery_required"
                } else {
                    "unknown"
                });
                let update = match &remaining {
                    Some(view) => projection_update(view, "recovery_required", after_hash.clone()),
                    None => TransactionUpdate {
                        state: Some(state),
                        journal_state: Some(journal.state.clone()),
                        journal_sha256: Some(journal_sha256(&journal)),
                        post_inventory_sha256: after_hash.clone(),
                        finished: terminal_state(&journal.state).is_some(),
                        ..TransactionUpdate::default()
                    },
                };
                self.inner
                    .store
                    .update_transaction(&scope, row.id, &update)
                    .await?;
                if let Some(plan_id) = row.plan_id {
                    let plan_state = match journal.state.as_str() {
                        "completed" => "applied",
                        "rolled_back" | "staging_discarded" => "apply_failed",
                        _ => "applying",
                    };
                    self.inner
                        .store
                        .set_plan_state(&scope, plan_id, plan_state)
                        .await?;
                }
                // After a rollback, the project must equal the inventory taken before apply.
                let restoration = if action == RecoveryAction::Rollback {
                    let pre =
                        load_inventory(&pre_apply_inventory_path(&access.state_dir, journal_id))
                            .filter(|inv| {
                                row.pre_inventory_sha256
                                    .as_deref()
                                    .is_none_or(|h| h == inv.content_hash())
                            });
                    match (pre, &after) {
                        (Some(pre), Ok(after)) => {
                            let diff = pre.diff(after);
                            Some(RestorationCheck {
                                exact: diff.is_empty(),
                                differences: ChangeSummary::from(&diff),
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let receipt = ReceiptOutcome {
                    before_inventory_sha256: before_hash,
                    after_inventory_sha256: after_hash,
                    result: Some(json!({
                        "journal_id": journal_id,
                        "variant": expected.as_str(),
                        "action": action.as_str(),
                        "journal_state": journal.state,
                        "remaining": remaining,
                        "restoration": restoration,
                        "setup_revision": SETUP_PIN.revision,
                        "setup_has_pr105_fix": SETUP_PIN.contains_pr_105_fix,
                    })),
                    ..ReceiptOutcome::default()
                };
                self.settle(&handle, ReceiptStatus::Succeeded, &receipt)
                    .await;
                Ok(RecoverOutcome {
                    journal_id,
                    variant: expected,
                    action: action.as_str(),
                    journal_state: journal.state,
                    receipt_id: handle.id,
                    remaining,
                    restoration,
                    setup: SETUP_PIN,
                })
            }
            Err(native) => {
                let cause = access.redactor.text(&native.to_string());
                // A finalizing journal whose lock was never committed advertises only `resume`,
                // and that resume refuses to run. Offering it again would loop forever, so
                // this journal state is held as manual review until the journal changes.
                let resume_dead_end = expected == RecoveryVariant::VerifiableFinalization
                    && cause.contains("rollback or manual review is required");
                let remaining = remaining.map(|view| {
                    if resume_dead_end {
                        view.with_variant(RecoveryVariant::ManualReview)
                    } else {
                        view
                    }
                });
                let update = match &remaining {
                    Some(view) => projection_update(view, "recovery_required", after_hash.clone()),
                    None => TransactionUpdate {
                        state: Some("unknown"),
                        post_inventory_sha256: after_hash.clone(),
                        ..TransactionUpdate::default()
                    },
                };
                let _ = self
                    .inner
                    .store
                    .update_transaction(&scope, row.id, &update)
                    .await;
                let receipt = ReceiptOutcome {
                    before_inventory_sha256: before_hash,
                    after_inventory_sha256: after_hash,
                    error_code: Some("recovery_failed".into()),
                    error_message: Some(cause.clone()),
                    result: Some(json!({"remaining": remaining})),
                    ..ReceiptOutcome::default()
                };
                self.settle(&handle, ReceiptStatus::Failed, &receipt).await;
                Err(SetupError::Native {
                    kind: crate::error::native_kind(&native),
                    message: cause,
                })
            }
        }
    }
}

fn projection_update(
    view: &RecoveryView,
    state: &'static str,
    post_inventory_sha256: Option<String>,
) -> TransactionUpdate {
    TransactionUpdate {
        state: Some(state),
        journal_state: Some(view.journal_state.clone()),
        recovery_variant: Some(view.variant.as_str()),
        allowed_action: view.allowed_action,
        project_apply_started: Some(view.project_apply_started),
        journal_sha256: Some(view.journal_sha256.clone()),
        post_inventory_sha256,
        ..TransactionUpdate::default()
    }
}
