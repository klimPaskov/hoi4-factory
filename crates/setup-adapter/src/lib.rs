//! HOI4 Factory Setup adapter (C-09, ADR-001 hybrid binding).
//!
//! The adapter binds the HOI4-Mod-Setup service facade (`hoi4_mod_setup::service`) to server
//! workspaces. It adds what a hosted, multi-account deployment needs around Setup's gates:
//!
//! * typed account, project and onboarding-session IDs, and a role check per operation through
//!   `factory_accounts::TenantRepo`; another account's project is `not_found`;
//! * no path from the caller: the project root is `<workspace>/source` (or a validated relative
//!   mod root below it), the Setup state root is `<workspace>/setup-state`, outside `source/`;
//! * a per-project lease so one owner drives a transaction ([`ProjectLease`]);
//! * C-04 style receipts and projections of sessions, scans, analyses, plans, approvals and
//!   transactions in PostgreSQL (migrations 0400 and 0401, row-level security);
//! * recovery variants mapped from what the native journal permits ([`RecoveryVariant`]);
//! * host paths replaced by logical locators in everything returned.
//!
//! Setup keeps the authority. Its own gates (confirmed analysis, evidence from the latest scan,
//! root identity, approval bound to the plan hash, rollback-only recovery after a partial apply)
//! run unchanged inside the facade. The adapter never fabricates a confirmation, an analysis or
//! a plan: with no plan builder, `plan_setup` returns `capability_unavailable`.
//!
//! Which Setup revision runs, and whether it contains the PR 105 recovery fix, is [`SETUP_PIN`].

mod adapter;
mod error;
mod ids;
mod lease;
mod migrate;
mod model;
mod ops_apply;
mod ops_plan;
mod ops_scan;
mod pin;
mod recovery;
mod redact;
mod store;
mod views;
mod workspace_view;

pub use adapter::{STATE_DIR_NAME, SetupAdapter, SetupAdapterDeps};
pub use error::{NativeKind, SetupError};
pub use ids::{
    AccountId, AnalysisId, JournalId, OnboardingSessionId, OpContext, PlanId, ProjectId, ReceiptId,
    ScanId,
};
pub use lease::ProjectLease;
pub use migrate::{MIGRATION_VERSIONS, migrator};
pub use model::{
    ANTHROPIC_UNSUPPORTED_KEYWORDS, ModelClient, ModelError, ModelReply, ModelRequest,
    analysis_contract, json_object_from_text, strip_schema_keywords,
};
pub use ops_scan::confirmation_values;
pub use pin::{SETUP_PIN, SetupPin};
pub use recovery::{RecoveryVariant, RecoveryView, classify as classify_recovery, journal_sha256};
pub use redact::Redactor;
pub use store::{
    AnalysisRow, ApprovalRow, PlanRow, ReceiptRow, ReceiptStatus, ScanRow, Scope, SetupOperation,
    Store, TransactionInsert, TransactionRow, TransactionUpdate,
};
pub use views::{
    AnalysisInput, AnalysisOutcome, ApplyOutcome, ApprovalView, ChangeSummary, ConflictDecision,
    InspectOptions, InspectOutcome, OpenWorkspaceOptions, OperationCapability, PlanView,
    ProgressSink, RecoverOutcome, RestorationCheck, SessionInfo, SetupCapabilities, SetupStatus,
    WorkspaceHandoff,
};

/// Re-exports the server needs to implement the traits this crate takes.
pub mod setup {
    pub use hoi4_mod_setup::credentials::{CredentialStore, DeniedCredentialStore};
    pub use hoi4_mod_setup::models::CredentialReference;
    pub use hoi4_mod_setup::service::{
        BuiltPlan, NodeRuntime, PlanBuilder, PlanRequest, RecoveryAction, root_identity,
    };
    pub use hoi4_mod_setup::source::SourceResolution;
}
