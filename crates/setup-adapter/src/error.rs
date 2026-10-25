//! Adapter errors. Each has a stable code and a retry class from the common envelope, and
//! converts into the accounts crate's `ApiError` so the server can return it unchanged.

use axum::http::StatusCode;
use factory_accounts::{ApiError, RetryClass};
use hoi4_mod_setup::AppError;
use uuid::Uuid;

use crate::ids::ProjectId;
use crate::recovery::RecoveryVariant;

/// Which native error family a refusal came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeKind {
    InvalidInput,
    PathSecurity,
    Source,
    Scan,
    Merge,
    Transaction,
    Io,
    Credential,
    UnsafeProviderOutput,
    Process,
    Protocol,
    Serialization,
    UnsupportedPlatform,
}

impl NativeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PathSecurity => "path_security",
            Self::Source => "source",
            Self::Scan => "scan",
            Self::Merge => "merge",
            Self::Transaction => "transaction",
            Self::Io => "io",
            Self::Credential => "credential",
            Self::UnsafeProviderOutput => "unsafe_provider_output",
            Self::Process => "process",
            Self::Protocol => "protocol",
            Self::Serialization => "serialization",
            Self::UnsupportedPlatform => "unsupported_platform",
        }
    }
}

/// Everything an adapter operation can fail with.
#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    /// The operation needs something this deployment does not have (a plan builder, a model
    /// client). Reported as unavailable, never faked.
    #[error("{capability} is unavailable: {reason}")]
    CapabilityUnavailable {
        capability: &'static str,
        reason: String,
    },
    /// A path or root that is not inside the project's workspace.
    #[error("refused: {0}")]
    OutsideWorkspace(String),
    /// Unknown project, session, plan or transaction, or one that belongs to someone else.
    #[error("not found")]
    NotFound,
    #[error("your role on this project does not allow that action")]
    Forbidden,
    /// Another owner drives this project's setup transaction right now.
    #[error("another setup operation is running on this project")]
    ProjectBusy { project: ProjectId },
    /// An incomplete native transaction must be recovered before anything else is applied.
    #[error("an incomplete setup transaction must be recovered first")]
    RecoveryRequired {
        journal_id: Uuid,
        variant: RecoveryVariant,
    },
    /// Apply started and did not finish. The native journal permits only the recovery named by
    /// `variant`; nothing may be applied on top of it.
    #[error("setup was interrupted: {cause}")]
    ApplyInterrupted {
        cause: String,
        journal_id: Uuid,
        variant: RecoveryVariant,
    },
    /// A gate refused the operation before any effect.
    #[error("{message}")]
    Precondition { code: &'static str, message: String },
    /// What the caller reviewed is no longer what exists.
    #[error("{message}")]
    Stale { code: &'static str, message: String },
    /// A value the person entered cannot be used. `field` names it for the form.
    #[error("{message}")]
    InvalidValues {
        field: &'static str,
        message: String,
    },
    /// Setup itself refused or failed.
    #[error("setup {}: {message}", kind.as_str())]
    Native { kind: NativeKind, message: String },
    /// The model client failed, or its reply was rejected by Setup's validator.
    #[error("analysis model: {0}")]
    Model(String),
    /// A read-only operation changed workspace bytes. The change is reported, never repaired.
    #[error("a read-only operation changed the workspace: {0}")]
    ReadOnlyViolated(String),
    /// The same idempotency key was already used for an effect.
    #[error("this request was already submitted")]
    IdempotencyReplay { receipt_id: Uuid },
    /// An effect may or may not have happened. Reconcile before repeating.
    #[error("the outcome of the setup operation is unknown: {0}")]
    UnknownEffect(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("workspace error: {0}")]
    Workspace(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl SetupError {
    /// Stable machine code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::CapabilityUnavailable { .. } => "capability_unavailable",
            Self::OutsideWorkspace(_) => "outside_workspace",
            Self::NotFound => "not_found",
            Self::Forbidden => "forbidden",
            Self::ProjectBusy { .. } => "project_busy",
            Self::RecoveryRequired { .. } => "recovery_required",
            Self::ApplyInterrupted { .. } => "apply_interrupted",
            Self::Precondition { code, .. } | Self::Stale { code, .. } => code,
            Self::InvalidValues { .. } => "invalid_request",
            Self::Native { .. } => "setup_refused",
            Self::Model(_) => "analysis_model_failed",
            Self::ReadOnlyViolated(_) => "read_only_violated",
            Self::IdempotencyReplay { .. } => "idempotency_replay",
            Self::UnknownEffect(_) => "unknown_effect",
            Self::Database(_) => "internal",
            Self::Workspace(_) => "workspace_unavailable",
            Self::Internal(_) => "internal",
        }
    }

    pub fn retry_class(&self) -> RetryClass {
        match self {
            Self::CapabilityUnavailable { .. } => RetryClass::Terminal,
            Self::OutsideWorkspace(_) | Self::NotFound => RetryClass::Terminal,
            Self::Forbidden => RetryClass::RequiresAuthorization,
            Self::ProjectBusy { .. } => RetryClass::SafeSameInputs,
            Self::RecoveryRequired { .. } => RetryClass::RefreshPreconditions,
            Self::ApplyInterrupted { .. } => RetryClass::ReconcileFirst,
            Self::Precondition { .. } => RetryClass::RefreshPreconditions,
            Self::Stale { .. } => RetryClass::RefreshPreconditions,
            Self::Native { .. } | Self::InvalidValues { .. } => RetryClass::Terminal,
            Self::Model(_) => RetryClass::SafeSameInputs,
            Self::ReadOnlyViolated(_) => RetryClass::ReconcileFirst,
            Self::IdempotencyReplay { .. } => RetryClass::ReconcileFirst,
            Self::UnknownEffect(_) => RetryClass::ReconcileFirst,
            Self::Database(_) | Self::Internal(_) => RetryClass::ReconcileFirst,
            Self::Workspace(_) => RetryClass::Terminal,
        }
    }

    pub(crate) fn precondition(code: &'static str, message: impl Into<String>) -> Self {
        Self::Precondition {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn stale(code: &'static str, message: impl Into<String>) -> Self {
        Self::Stale {
            code,
            message: message.into(),
        }
    }
}

impl From<ApiError> for SetupError {
    fn from(err: ApiError) -> Self {
        match err.code {
            "not_found" => Self::NotFound,
            "forbidden" => Self::Forbidden,
            other => Self::Internal(format!("accounts: {other}")),
        }
    }
}

impl From<factory_workspace::WorkspaceError> for SetupError {
    fn from(err: factory_workspace::WorkspaceError) -> Self {
        use factory_workspace::WorkspaceError as W;
        match err {
            W::NotFound => Self::Workspace("the project has no server workspace".into()),
            W::Path(p) => Self::OutsideWorkspace(p.to_string()),
            other => Self::Workspace(other.to_string()),
        }
    }
}

impl From<SetupError> for ApiError {
    fn from(err: SetupError) -> Self {
        let (status, message) = match &err {
            SetupError::CapabilityUnavailable { .. } => {
                (StatusCode::NOT_IMPLEMENTED, err.to_string())
            }
            SetupError::OutsideWorkspace(_) => (StatusCode::FORBIDDEN, err.to_string()),
            SetupError::NotFound => (StatusCode::NOT_FOUND, "Not found.".to_owned()),
            SetupError::Forbidden => (StatusCode::FORBIDDEN, err.to_string()),
            SetupError::ProjectBusy { .. }
            | SetupError::RecoveryRequired { .. }
            | SetupError::ApplyInterrupted { .. }
            | SetupError::IdempotencyReplay { .. }
            | SetupError::Precondition { .. }
            | SetupError::Stale { .. }
            | SetupError::Workspace(_) => (StatusCode::CONFLICT, err.to_string()),
            SetupError::Native { .. } | SetupError::InvalidValues { .. } => {
                (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
            }
            SetupError::Model(_) => (StatusCode::BAD_GATEWAY, err.to_string()),
            SetupError::ReadOnlyViolated(_) | SetupError::UnknownEffect(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            SetupError::Database(_) | SetupError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong on the server.".to_owned(),
            ),
        };
        let mut out = ApiError::new(status, err.code(), message, err.retry_class());
        match &err {
            SetupError::ProjectBusy { project } => out = out.with_id("project_id", project),
            SetupError::RecoveryRequired { journal_id, .. }
            | SetupError::ApplyInterrupted { journal_id, .. } => {
                out = out.with_id("journal_id", journal_id)
            }
            SetupError::IdempotencyReplay { receipt_id } => {
                out = out.with_id("receipt_id", receipt_id)
            }
            SetupError::InvalidValues { field, .. } => out = out.with_id("field", field),
            _ => {}
        }
        out
    }
}

pub(crate) fn native_kind(error: &AppError) -> NativeKind {
    match error {
        AppError::InvalidInput(_) => NativeKind::InvalidInput,
        AppError::PathSecurity(_) => NativeKind::PathSecurity,
        AppError::Source(_) => NativeKind::Source,
        AppError::Scan(_) => NativeKind::Scan,
        AppError::Merge(_) => NativeKind::Merge,
        AppError::Transaction(_) => NativeKind::Transaction,
        AppError::Io(_) => NativeKind::Io,
        AppError::Credential(_) => NativeKind::Credential,
        AppError::UnsafeProviderOutput(_) => NativeKind::UnsafeProviderOutput,
        AppError::Process(_) => NativeKind::Process,
        AppError::Protocol(_) => NativeKind::Protocol,
        AppError::Serialization(_) => NativeKind::Serialization,
        AppError::UnsupportedPlatform(_) => NativeKind::UnsupportedPlatform,
    }
}
