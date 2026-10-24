//! openWorkspace (C-01 handoff), status and receipts for the browser's read routes.

use factory_accounts::Permission;
use factory_workspace::{
    Capability, CapabilityStatus, ComponentClosure, DescriptorInputs, GeneratedFile,
    SetupReceiptSummary, UnresolvedCheck, WorkspaceId,
};

use crate::adapter::SetupAdapter;
use crate::error::SetupError;
use crate::ids::{AccountId, OnboardingSessionId, ProjectId};
use crate::pin::SETUP_PIN;
use crate::store::ReceiptRow;
use crate::views::{OpenWorkspaceOptions, SetupStatus, WorkspaceHandoff};

impl SetupAdapter {
    /// Opens the project workspace for handoff. The C-01 descriptor is returned only when no
    /// setup transaction waits for recovery: a partial installation blocks handoff. Optional
    /// capabilities are labelled separately from the setup invariants, and checks that did not
    /// run stay listed as unresolved.
    pub async fn open_workspace(
        &self,
        account: AccountId,
        project: ProjectId,
        options: OpenWorkspaceOptions,
    ) -> Result<WorkspaceHandoff, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        let recovery = self.inspect_recovery(account, project).await?;
        let unfinished = self
            .inner
            .store
            .unfinished_transaction(&access.scope)
            .await?;
        let mut blockers = Vec::new();
        if let Some(view) = &recovery {
            blockers.push(format!(
                "A setup transaction is waiting for recovery ({}).",
                view.variant.as_str()
            ));
        } else if let Some(row) = &unfinished {
            blockers.push(format!(
                "A setup transaction is unfinished in state `{}` and must be reconciled.",
                row.state
            ));
        }
        if !blockers.is_empty() {
            return Ok(WorkspaceHandoff {
                descriptor: None,
                ready: false,
                blockers,
                setup: SETUP_PIN,
                recovery,
            });
        }

        let capabilities = self.capabilities();
        let latest = self
            .inner
            .store
            .list_transactions(&access.scope, 1)
            .await?
            .into_iter()
            .next();
        let latest_plan = match &latest {
            Some(row) if row.state == "completed" => match row.plan_id {
                Some(id) => {
                    self.inner
                        .store
                        .latest_plan_by_id(&access.scope, id)
                        .await?
                }
                None => None,
            },
            _ => None,
        };
        let setup_receipt = match (&latest, &latest_plan) {
            (Some(row), Some(plan)) if row.state == "completed" => Some(SetupReceiptSummary {
                plan_id: row.plan_id.map(|p| p.to_string()).unwrap_or_default(),
                transaction_state: row.state.clone(),
                generated_files: plan
                    .get("operations")
                    .and_then(|o| o.as_array())
                    .map(|ops| {
                        ops.iter()
                            .filter_map(|op| {
                                let path = op.get("destination")?.as_str()?.to_owned();
                                let text = |key: &str| op.get(key).and_then(|v| v.as_str());
                                let sha = text("result_sha256")
                                    .or_else(|| text("source_sha256"))?
                                    .to_owned();
                                Some(GeneratedFile { path, sha256: sha })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
            _ => None,
        };
        let manifest_hash = self.inner.store.latest_manifest_hash(&access.scope).await?;

        let mut inputs =
            DescriptorInputs::new(WorkspaceId::new(project.to_string()).map_err(SetupError::from)?);
        inputs.member_roles = options.member_roles;
        inputs.extra_roots = options.extra_roots;
        if let Some(profile) = options.game_profile {
            inputs.game_profile = profile;
        }
        inputs.component_closure = ComponentClosure {
            setup_revision: Some(SETUP_PIN.revision.to_owned()),
            manifest_hash,
            ..ComponentClosure::default()
        };
        inputs.capabilities = capabilities
            .operations
            .iter()
            .map(|o| Capability {
                name: format!("setup.{}", o.operation),
                status: if o.available {
                    CapabilityStatus::Available
                } else {
                    CapabilityStatus::Unavailable
                },
                reason: o.reason.clone(),
            })
            .collect();
        inputs.setup_receipt = setup_receipt;
        let mut unresolved = vec![UnresolvedCheck {
            check: "In-game behaviour of the installed files was not observed".into(),
            owner: "mod author".into(),
            impact: "Setup installs source files; playing the mod happens on the author's machine"
                .into(),
        }];
        if !SETUP_PIN.contains_pr_105_fix {
            unresolved.push(UnresolvedCheck {
                check: format!(
                    "Setup revision {} lacks the PR 105 recovery fix",
                    SETUP_PIN.short()
                ),
                owner: "maintainer".into(),
                impact: "apply and recovery evidence does not count for gates S02-G2 and S02-G4 until a merged revision with the fix is pinned".into(),
            });
        }
        inputs.unresolved_checks = unresolved;

        let workspace = access.workspace.clone();
        let descriptor = tokio::task::spawn_blocking(move || workspace.describe(inputs))
            .await
            .map_err(|e| SetupError::Internal(format!("descriptor task: {e}")))??;
        Ok(WorkspaceHandoff {
            descriptor: Some(descriptor),
            ready: true,
            blockers,
            setup: SETUP_PIN,
            recovery: None,
        })
    }

    /// Everything the browser's scan, plan and recovery panels read, in one call.
    pub async fn setup_status(
        &self,
        account: AccountId,
        project: ProjectId,
        session: Option<OnboardingSessionId>,
    ) -> Result<SetupStatus, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        let store = &self.inner.store;
        let latest_plan = store
            .latest_plan(&access.scope, session.map(|s| s.0))
            .await?;
        let active_approval = match &latest_plan {
            Some((row, _)) => store.active_approval(&access.scope, row.id).await?,
            None => None,
        };
        Ok(SetupStatus {
            setup: SETUP_PIN,
            capabilities: self.capabilities(),
            latest_scan: store
                .latest_scan(&access.scope, session.map(|s| s.0))
                .await?,
            latest_plan: latest_plan.map(|(row, _)| row),
            active_approval,
            unfinished_transaction: store.unfinished_transaction(&access.scope).await?,
            recent_transactions: store.list_transactions(&access.scope, 10).await?,
            recent_receipts: store.list_receipts(&access.scope, 20).await?,
            generated_at: store.now(),
        })
    }

    /// Receipts of the project, newest first.
    pub async fn list_receipts(
        &self,
        account: AccountId,
        project: ProjectId,
        limit: i64,
    ) -> Result<Vec<ReceiptRow>, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        self.inner.store.list_receipts(&access.scope, limit).await
    }

    /// Intents that never settled, for a reconciler after a crash.
    pub async fn unsettled_receipts(
        &self,
        account: AccountId,
        project: ProjectId,
    ) -> Result<Vec<ReceiptRow>, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        self.inner.store.unsettled_receipts(&access.scope).await
    }
}
