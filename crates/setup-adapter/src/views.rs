//! Request and response types of the C-09 operations. Responses carry logical locators only.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use chrono::{DateTime, Utc};
use factory_workspace::{InventoryDiff, WorkspaceDescriptor};
use hoi4_mod_setup::codex::{ApprovedEvidence, CodexAnalysisResult};
use hoi4_mod_setup::models::{CredentialReference, ScanResult};
use hoi4_mod_setup::scanner::ScanProgress;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::ids::{AccountId, OnboardingSessionId, ProjectId};
use crate::pin::SetupPin;
use crate::recovery::{RecoveryVariant, RecoveryView};
use crate::store::{ApprovalRow, PlanRow, ReceiptRow, ScanRow, TransactionRow};

/// Whether an operation can run in this deployment, and why not when it cannot.
#[derive(Debug, Clone, Serialize)]
pub struct OperationCapability {
    pub operation: &'static str,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupCapabilities {
    pub setup: SetupPin,
    pub operations: Vec<OperationCapability>,
}

impl SetupCapabilities {
    pub fn get(&self, operation: &str) -> Option<&OperationCapability> {
        self.operations.iter().find(|o| o.operation == operation)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: OnboardingSessionId,
    pub account_id: AccountId,
    pub project_id: ProjectId,
    /// Relative path of the mod root inside `source/` (empty for `source/` itself).
    pub mod_root: String,
    /// Logical locator of the mod root.
    pub root_locator: String,
    pub setup: SetupPin,
    pub capabilities: SetupCapabilities,
}

pub type ProgressSink = Arc<dyn Fn(ScanProgress) + Send + Sync>;

/// Options of `inspect_project`.
#[derive(Clone, Default)]
pub struct InspectOptions {
    /// Setting the flag stops the scan. A stopped scan reports explicit incomplete coverage and
    /// approves no evidence.
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: Option<ProgressSink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectOutcome {
    pub scan_id: Uuid,
    pub root_identity: String,
    /// The scan with host paths replaced by logical locators.
    pub scan: ScanResult,
    pub receipt_id: Uuid,
    /// Workspace inventory hashes around the scan. `None` when the guard is off.
    pub inventory_before_sha256: Option<String>,
    pub inventory_after_sha256: Option<String>,
    pub workspace_unchanged: Option<bool>,
}

/// One analysis turn. `scan_id` binds an existing-project analysis to a scan; a new project
/// brief has none.
#[derive(Debug, Clone)]
pub struct AnalysisInput {
    pub provider: String,
    pub model: String,
    pub reasoning_effort: String,
    pub endpoint: String,
    /// Reference into the server secret store; never a key value.
    pub credential: Option<CredentialReference>,
    /// `existing_project_semantics` or `new_project_identity`.
    pub mode: String,
    pub brief: String,
    pub evidence: Vec<ApprovedEvidence>,
    pub constraints: Value,
    pub purpose: Option<String>,
    pub scan_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisOutcome {
    pub analysis_id: Uuid,
    pub result: CodexAnalysisResult,
    pub receipt_id: Uuid,
    /// Model attempts: 2 when Setup's validator asked for one correction.
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanView {
    pub plan_id: Uuid,
    /// The digest a reviewer approves. A conflict decision produces a new one.
    pub plan_sha256: String,
    pub root_identity: String,
    pub state: String,
    pub operation_count: i32,
    /// Plan as stored for the browser, host paths replaced by logical locators.
    pub plan: Value,
    pub conflict_decisions: Value,
    pub analysis_id: Option<Uuid>,
    pub scan_id: Option<Uuid>,
    pub receipt_id: Option<Uuid>,
}

impl PlanView {
    pub(crate) fn from_row(row: PlanRow, plan: Value, receipt_id: Option<Uuid>) -> Self {
        Self {
            plan_id: row.id,
            plan_sha256: row.plan_sha256,
            root_identity: row.root_identity,
            state: row.state,
            operation_count: row.operation_count,
            plan,
            conflict_decisions: row.conflict_decisions,
            analysis_id: row.analysis_id,
            scan_id: row.scan_id,
            receipt_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConflictDecision {
    pub path: String,
    /// keep, replace, rename, merge or skip, as the plan offered it.
    pub choice: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalView {
    pub approval_id: Uuid,
    pub plan_id: Uuid,
    pub reviewed_plan_sha256: String,
    pub approved_plan_sha256: String,
    pub root_identity: String,
    pub receipt_id: Uuid,
}

/// Files that changed between two inventories, as paths.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChangeSummary {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub mode_changed: Vec<String>,
    pub git_index_changed: bool,
}

impl From<&InventoryDiff> for ChangeSummary {
    fn from(d: &InventoryDiff) -> Self {
        Self {
            added: d.added.clone(),
            removed: d.removed.clone(),
            changed: d.changed.clone(),
            mode_changed: d.mode_changed.clone(),
            git_index_changed: d.index_changed,
        }
    }
}

impl ChangeSummary {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.mode_changed.is_empty()
            && !self.git_index_changed
    }

    pub fn paths(&self) -> impl Iterator<Item = &String> {
        self.added
            .iter()
            .chain(&self.removed)
            .chain(&self.changed)
            .chain(&self.mode_changed)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyOutcome {
    pub plan_id: Uuid,
    pub journal_id: Uuid,
    pub journal_state: String,
    pub receipt_id: Uuid,
    pub fence_generation: i64,
    pub inventory_before_sha256: String,
    pub inventory_after_sha256: String,
    pub changes: ChangeSummary,
    /// Changed paths that are neither a planned destination nor Setup-owned state. Reported as
    /// preservation evidence; never repaired by the adapter.
    pub unplanned_changes: Vec<String>,
    pub setup: SetupPin,
}

/// Whether a rollback restored the project exactly to the inventory taken before apply.
#[derive(Debug, Clone, Serialize)]
pub struct RestorationCheck {
    pub exact: bool,
    pub differences: ChangeSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoverOutcome {
    pub journal_id: Uuid,
    pub variant: RecoveryVariant,
    pub action: &'static str,
    pub journal_state: String,
    pub receipt_id: Uuid,
    /// The recovery still pending after this action, if the transaction is not finished.
    pub remaining: Option<RecoveryView>,
    /// Set after a rollback, when the pre-apply inventory is available.
    pub restoration: Option<RestorationCheck>,
    pub setup: SetupPin,
}

#[derive(Debug, Clone, Default)]
pub struct OpenWorkspaceOptions {
    pub game_profile: Option<factory_workspace::GameProfile>,
    pub member_roles: Vec<factory_workspace::MemberRole>,
    pub extra_roots: Vec<factory_workspace::RootHandle>,
    /// Capabilities the host reports besides the `setup.*` ones (for example the game reference).
    pub extra_capabilities: Vec<factory_workspace::Capability>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceHandoff {
    /// C-01. Present only when no setup transaction is waiting for recovery.
    pub descriptor: Option<WorkspaceDescriptor>,
    /// True when the descriptor is present and no blocker remains.
    pub ready: bool,
    pub blockers: Vec<String>,
    pub setup: SetupPin,
    pub recovery: Option<RecoveryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    pub setup: SetupPin,
    pub capabilities: SetupCapabilities,
    pub latest_scan: Option<ScanRow>,
    pub latest_plan: Option<PlanRow>,
    pub active_approval: Option<ApprovalRow>,
    pub unfinished_transaction: Option<TransactionRow>,
    pub recent_transactions: Vec<TransactionRow>,
    pub recent_receipts: Vec<ReceiptRow>,
    pub generated_at: DateTime<Utc>,
}
