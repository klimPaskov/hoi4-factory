//! The common error envelope: stable code, user-facing explanation, retry class, affected IDs
//! and evidence references (contracts, "Common envelope").

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

/// What a caller may do after this error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    /// The same request may be repeated, after the indicated delay if any.
    SafeSameInputs,
    /// Re-read current state, then decide again.
    RefreshPreconditions,
    /// The effect may or may not have happened. Query the outcome before repeating.
    ReconcileFirst,
    /// The caller lacks authority. Obtain it (sign in, verify, ask the owner) first.
    RequiresAuthorization,
    /// Repeating the same request cannot succeed.
    Terminal,
}

/// Every failure this crate reports, in envelope form.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retry_class: RetryClass,
    pub affected_ids: BTreeMap<&'static str, String>,
    pub retry_after: Option<Duration>,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
        retry_class: RetryClass,
    ) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_class,
            affected_ids: BTreeMap::new(),
            retry_after: None,
        }
    }

    pub fn with_id(mut self, key: &'static str, value: impl ToString) -> Self {
        self.affected_ids.insert(key, value.to_string());
        self
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            message,
            RetryClass::Terminal,
        )
    }

    pub fn invalid_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "The email address or password is not correct.",
            RetryClass::Terminal,
        )
    }

    pub fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Sign in to continue.",
            RetryClass::RequiresAuthorization,
        )
    }

    pub fn account_disabled() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "account_disabled",
            "This account is disabled.",
            RetryClass::Terminal,
        )
    }

    pub fn email_not_verified() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "email_not_verified",
            "Verify your email address to use this feature.",
            RetryClass::RequiresAuthorization,
        )
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            message,
            RetryClass::RequiresAuthorization,
        )
    }

    /// Also used for objects that exist but belong to someone else, so that existence is not
    /// revealed.
    pub fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Not found.",
            RetryClass::Terminal,
        )
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message, RetryClass::Terminal)
    }

    pub fn rate_limited(retry_after: Duration) -> Self {
        let mut e = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many attempts. Wait a moment and try again.",
            RetryClass::SafeSameInputs,
        );
        e.retry_after = Some(retry_after);
        e
    }

    /// An unexpected server-side failure. The outcome of a mutation is unknown.
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "Something went wrong on the server.",
            RetryClass::ReconcileFirst,
        )
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        // The database error text can contain row values. Keep it out of the response.
        tracing::error!(error = %err, "database error");
        Self::internal()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "retry_class": self.retry_class,
                "affected_ids": self.affected_ids,
                "evidence_refs": [],
            }
        });
        let mut response = (self.status, Json(body)).into_response();
        if let Some(wait) = self.retry_after {
            let secs = wait.as_secs().max(1).to_string();
            if let Ok(value) = HeaderValue::from_str(&secs) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}
