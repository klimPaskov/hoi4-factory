//! Failures of the intake and workspace routes in the common error envelope.
//!
//! The envelope is `factory_accounts::ApiError`. [`Fail`] wraps it so a few errors can carry a
//! `details` object (the GitHub install link) without changing the fields every client reads.
//! [`intake_error`] turns an intake failure into the right status, stable code and retry class.
//! Errors that can carry host paths, SQL or Git output (storage, Git, database, I/O) are
//! reported with their code and a fixed message; the detail goes to the log.

use std::time::Duration;

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use factory_accounts::{ApiError, RetryClass};
use factory_intake::{IntakeError, RetryClass as IntakeRetry, UploadFailure};
use factory_workspace::WorkspaceError;
use serde_json::{Value, json};

/// An [`ApiError`] with optional `error.details`.
#[derive(Debug)]
pub struct Fail {
    pub error: ApiError,
    pub details: Option<Value>,
}

impl Fail {
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_id(mut self, key: &'static str, value: impl ToString) -> Self {
        self.error = self.error.with_id(key, value);
        self
    }
}

impl From<ApiError> for Fail {
    fn from(error: ApiError) -> Self {
        Fail {
            error,
            details: None,
        }
    }
}

impl From<sqlx::Error> for Fail {
    fn from(err: sqlx::Error) -> Self {
        ApiError::from(err).into()
    }
}

impl From<factory_setup_adapter::SetupError> for Fail {
    fn from(err: factory_setup_adapter::SetupError) -> Self {
        if matches!(
            err,
            factory_setup_adapter::SetupError::Database(_)
                | factory_setup_adapter::SetupError::Internal(_)
                | factory_setup_adapter::SetupError::UnknownEffect(_)
                | factory_setup_adapter::SetupError::ReadOnlyViolated(_)
        ) {
            tracing::error!(code = err.code(), error = %err, "setup operation failed");
        }
        ApiError::from(err).into()
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        let Some(details) = self.details else {
            return self.error.into_response();
        };
        let error = self.error;
        let body = json!({
            "error": {
                "code": error.code,
                "message": error.message,
                "retry_class": error.retry_class,
                "affected_ids": error.affected_ids,
                "evidence_refs": [],
                "details": details,
            }
        });
        let mut response = (error.status, Json(body)).into_response();
        if let Some(wait) = error.retry_after
            && let Ok(value) = HeaderValue::from_str(&wait.as_secs().max(1).to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        response
    }
}

fn retry_class(class: IntakeRetry) -> RetryClass {
    match class {
        IntakeRetry::SafeSameInputs => RetryClass::SafeSameInputs,
        IntakeRetry::RefreshPreconditions => RetryClass::RefreshPreconditions,
        IntakeRetry::ReconcileFirst => RetryClass::ReconcileFirst,
        IntakeRetry::RequiresAuthorization => RetryClass::RequiresAuthorization,
        IntakeRetry::Terminal => RetryClass::Terminal,
    }
}

fn status_for(error: &IntakeError) -> StatusCode {
    use IntakeError::*;
    match error {
        GitConsentRequired | InvalidRequest(_) | BranchNameUnsupported => StatusCode::BAD_REQUEST,
        UploadTooLarge { .. }
        | TooManyEntries { .. }
        | TooManyFiles { .. }
        | FileTooLarge { .. }
        | TotalTooLarge { .. }
        | RepositoryTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        Workspace(WorkspaceError::AlreadyExists) => StatusCode::CONFLICT,
        Workspace(_) | Git(_) | Io { .. } | Aborted | Database(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        AccessRevoked => StatusCode::CONFLICT,
        InstallationNotSelected
        | InstallationSuspended
        | RepositoryNotAllowed
        | RepositoryNotInInstallation => StatusCode::FORBIDDEN,
        BranchNotFound => StatusCode::UNPROCESSABLE_ENTITY,
        GithubRateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
        GithubUnavailable(_) | GithubProtocol(_) | CloneFailed => StatusCode::BAD_GATEWAY,
        Credentials(_) => StatusCode::SERVICE_UNAVAILABLE,
        // Everything left is a defect of the uploaded content: names, kinds, types, archive
        // structure, a hostile repository configuration.
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

/// Text that is safe to show: the intake display text names archive-relative paths only, and
/// never a token, a host path or Git's stderr. The variants that wrap lower layers get a fixed
/// sentence instead.
fn message_for(error: &IntakeError) -> String {
    use IntakeError::*;
    match error {
        Workspace(WorkspaceError::AlreadyExists) => {
            "This project already has a workspace. Create a new project to import another mod."
                .to_owned()
        }
        Workspace(_) | Git(_) | Io { .. } | Aborted | Database(_) => {
            "The server could not complete the operation. Try again, and report the problem if it persists."
                .to_owned()
        }
        Credentials(_) => "GitHub import is not available on this server right now.".to_owned(),
        GithubUnavailable(_) | GithubProtocol(_) => {
            "GitHub did not answer as expected. Try again in a moment.".to_owned()
        }
        other => other.to_string(),
    }
}

/// The envelope for an intake failure.
pub fn intake_error(error: &IntakeError) -> Fail {
    if matches!(
        error,
        IntakeError::Workspace(_)
            | IntakeError::Git(_)
            | IntakeError::Io { .. }
            | IntakeError::Aborted
            | IntakeError::Database(_)
    ) && !matches!(error, IntakeError::Workspace(WorkspaceError::AlreadyExists))
    {
        tracing::error!(code = error.code(), error = %error, "intake operation failed");
    }
    let mut api = ApiError::new(
        status_for(error),
        error.code(),
        message_for(error),
        retry_class(error.retry_class()),
    );
    if let IntakeError::GithubRateLimited { retry_after } = error {
        api.retry_after = Some(retry_after.unwrap_or(Duration::from_secs(30)));
    }
    api.into()
}

/// The envelope for a refused upload. `upload_id` is the identity the attempt was recorded under.
pub fn upload_failure(failure: &UploadFailure, upload_id: uuid::Uuid) -> Fail {
    intake_error(&failure.error).with_id("upload_id", upload_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_errors_are_unprocessable_with_their_stable_code() {
        let err = IntakeError::PathRejected {
            path: "../x".into(),
            reason: "traversal".into(),
        };
        let fail = intake_error(&err);
        assert_eq!(fail.error.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(fail.error.code, "path_rejected");
        assert_eq!(fail.error.retry_class, RetryClass::Terminal);
    }

    #[test]
    fn lower_layer_text_never_reaches_the_client() {
        let io = IntakeError::Io {
            operation: "write",
            source: std::io::Error::other("C:\\secret\\place"),
        };
        let fail = intake_error(&io);
        assert_eq!(fail.error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!fail.error.message.contains("secret"));
        assert_eq!(fail.error.retry_class, RetryClass::ReconcileFirst);
        let db = intake_error(&IntakeError::Database(sqlx::Error::PoolTimedOut));
        assert_eq!(db.error.code, "database_error");
        assert!(!db.error.message.to_lowercase().contains("pool"));
    }

    #[test]
    fn existing_workspace_is_a_conflict() {
        let fail = intake_error(&IntakeError::Workspace(WorkspaceError::AlreadyExists));
        assert_eq!(fail.error.status, StatusCode::CONFLICT);
        assert_eq!(fail.error.code, "workspace_exists");
    }

    #[test]
    fn rate_limit_carries_a_retry_delay() {
        let fail = intake_error(&IntakeError::GithubRateLimited {
            retry_after: Some(Duration::from_secs(7)),
        });
        assert_eq!(fail.error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(fail.error.retry_after, Some(Duration::from_secs(7)));
    }
}
