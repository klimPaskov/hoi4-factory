//! Task ledger (S03-W2, S03-W4): pure host state in PostgreSQL.
//!
//! * C-03 ordered run events with gap detection ([`events`]).
//! * Tasks with expected-version transitions and durable pause ([`tasks`]).
//! * Attempts, distinct from task completion ([`attempts`]).
//! * Host incarnations, workspace leases and fencing ([`lease`]).
//! * C-06 context bundles: a pure builder ([`context`]) and an immutable store ([`bundle_store`]).
//! * C-04 intents before effects, receipts after, replay identity and reconciliation
//!   ([`receipts`]).
//! * Startup recovery discovery ([`recovery`]).
//!
//! The crate does no filesystem, network or model work and does not depend on the broker or the
//! agent. Every function takes `&mut PgConnection`, normally a transaction opened with
//! `factory_accounts::TenantRepo::begin`, so `app.account_id` is set and row-level security
//! applies, and so the host can compose a grant check, a fence check and an intent insert into
//! one transaction. [`Ledger`] wraps the common calls in their own scoped transaction.
//!
//! Every state-changing function writes its C-03 event in the same transaction as the change.

pub mod attempts;
pub mod bundle_store;
pub mod canon;
pub mod context;
pub mod enums;
pub mod error;
pub mod events;
pub mod ids;
pub mod lease;
pub mod migrate;
pub mod receipts;
pub mod recovery;
mod rows;
pub mod scoped;
pub mod tasks;

pub use attempts::{
    Admission, Attempt, admit_attempt, get_attempt, list_attempts, save_conversation,
    transition_attempt, unresolved_operations,
};
pub use bundle_store::{BundleError, load_bundle, store_bundle};
pub use canon::{canonical_json, canonical_sha256, sha256_hex};
pub use context::{
    ContextBundle, ContextError, ContextInputs, Manifest, TokenBudget, build_context,
};
pub use enums::{
    Actor, AttemptState, EffectClass, Funding, ParseEnumError, ReceiptStatus, RetryClass,
    TaskMode, TaskState,
};
pub use error::TaskError;
pub use events::{EventDraft, Gap, RunEvent, append_event, check_contiguous, events_after};
pub use ids::{AttemptId, BundleId, EventId, IncarnationId, OperationId, TaskId};
pub use lease::{
    FenceError, Lease, acquire_lease, check_fence, register_incarnation, renew_lease,
    touch_incarnation,
};
pub use migrate::{MIGRATION_VERSIONS, migrator};
pub use receipts::{
    Intent, IntentError, NewIntent, Outcome, Receipt, ReceiptError, Reconciliation,
    ReconciliationDecision, ReplayDecision, ReplayIdentity, mark_started, mark_unknown,
    pending_for_task, reconcile, record_intent, replay_decision, settle,
};
pub use recovery::{RecoveryItem, recovery_scan};
pub use scoped::Ledger;
pub use tasks::{NewTask, Task, create_task, get_task, list_tasks, set_paused, transition_task};
