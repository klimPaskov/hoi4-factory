//! Errors of the ledger. Every variant that a caller can act on is typed; anything the database
//! reports that this crate did not anticipate is `Database`.

use crate::enums::{AttemptState, ParseEnumError, RetryClass, TaskState};
use crate::ids::{AttemptId, OperationId};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    /// No such row, or a row the current tenant scope cannot see. The two are indistinguishable
    /// on purpose.
    #[error("not found")]
    NotFound,
    /// An expected-version update lost against a competing command.
    #[error("version conflict: the current version is {current}")]
    VersionConflict { current: i64 },
    #[error("task cannot move from {from} to {to}")]
    InvalidTransition { from: TaskState, to: TaskState },
    #[error("attempt cannot move from {from} to {to}")]
    InvalidAttemptTransition {
        from: AttemptState,
        to: AttemptState,
    },
    /// Pause is a durable scheduler condition: no new steps until it is cleared.
    #[error("the task is paused")]
    Paused,
    #[error("the task is {state} and admits no further attempts")]
    TaskTerminal { state: TaskState },
    #[error("a task in state {state} cannot admit an attempt")]
    NotAdmissible { state: TaskState },
    /// Single writer: the task already has a nonterminal attempt.
    #[error("attempt {attempt} is still active on this task")]
    AttemptActive { attempt: AttemptId },
    /// A started or unknown-effect operation must be reconciled before another attempt.
    #[error("{} operation(s) have an unresolved effect", operations.len())]
    UnresolvedEffects { operations: Vec<OperationId> },
    /// The acceptance decision belongs to the host or a user, never to worker output.
    #[error("completion cannot be decided by worker output")]
    CompletionNotByWorker,
    #[error("the context bundle does not belong to this task")]
    BundleMismatch,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("stored data is not understood: {0}")]
    Corrupt(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl TaskError {
    /// Stable code for the common error envelope.
    pub fn code(&self) -> &'static str {
        match self {
            TaskError::NotFound => "not_found",
            TaskError::VersionConflict { .. } => "version_conflict",
            TaskError::InvalidTransition { .. } | TaskError::InvalidAttemptTransition { .. } => {
                "invalid_transition"
            }
            TaskError::Paused => "task_paused",
            TaskError::TaskTerminal { .. } => "task_terminal",
            TaskError::NotAdmissible { .. } => "task_not_admissible",
            TaskError::AttemptActive { .. } => "attempt_active",
            TaskError::UnresolvedEffects { .. } => "unresolved_effects",
            TaskError::CompletionNotByWorker => "completion_not_by_worker",
            TaskError::BundleMismatch => "bundle_mismatch",
            TaskError::Invalid(_) => "invalid_request",
            TaskError::Corrupt(_) => "integrity_failure",
            TaskError::Database(_) => "internal",
        }
    }

    /// Retry class of the common envelope.
    pub fn retry_class(&self) -> RetryClass {
        match self {
            TaskError::VersionConflict { .. }
            | TaskError::AttemptActive { .. }
            | TaskError::NotFound => RetryClass::RefreshPreconditions,
            TaskError::UnresolvedEffects { .. } => RetryClass::ReconcileFirst,
            TaskError::Paused => RetryClass::RequiresAuthorization,
            _ => RetryClass::Terminal,
        }
    }
}

impl From<ParseEnumError> for TaskError {
    fn from(e: ParseEnumError) -> Self {
        TaskError::Corrupt(e.to_string())
    }
}

/// True for a unique-constraint violation (23505).
pub(crate) fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

/// True when the row was refused by row-level security (42501) or points at a parent the scope
/// cannot see (23503).
pub(crate) fn is_scope_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db)
        if matches!(db.code().as_deref(), Some("42501") | Some("23503")))
}
