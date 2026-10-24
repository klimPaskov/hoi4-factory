//! Cheap hardening applied to every response: security headers, CORS restricted to the
//! configured origins, request ids, one request log line, and a catch-all for handler panics.

use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_TYPE, ORIGIN, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, USER_AGENT,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use factory_accounts::ApiError;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::redact::redact_query;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// The browser client is a same-origin single-page app: scripts, styles, fonts and connections
/// come from this origin only. Images may also be `data:` and `blob:` (previews of the user's
/// own files). Nothing may frame the app and no plugin content is allowed.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data: blob:; \
connect-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; object-src 'none'";

const HSTS: &str = "max-age=31536000";
const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=(), payment=()";

/// Adds the security headers. `hsts` is off in development, where there is no TLS.
pub async fn security_headers(State(hsts): State<bool>, request: Request, next: Next) -> Response {
    let is_api = {
        let path = request.uri().path();
        path == "/api" || path.starts_with("/api/") || path == "/healthz" || path == "/readyz"
    };
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    if hsts {
        headers.insert(STRICT_TRANSPORT_SECURITY, HeaderValue::from_static(HSTS));
    }
    if is_api && !headers.contains_key(CACHE_CONTROL) {
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// CORS for the configured origins only. With none configured no origin is ever allowed: the
/// app and its API share an origin, so same-origin requests need no CORS at all.
pub fn cors_layer(origins: &[String]) -> CorsLayer {
    let allowed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();
    let request_id = HeaderName::from_static(REQUEST_ID_HEADER);
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([CONTENT_TYPE, request_id.clone()])
        .expose_headers([request_id])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600))
}

/// Turns a handler panic into the error envelope instead of a dropped connection.
pub fn catch_panic_layer()
-> CatchPanicLayer<fn(Box<dyn std::any::Any + Send + 'static>) -> Response> {
    CatchPanicLayer::custom(on_panic as fn(Box<dyn std::any::Any + Send + 'static>) -> Response)
}

fn on_panic(_: Box<dyn std::any::Any + Send + 'static>) -> Response {
    tracing::error!("a request handler panicked");
    ApiError::internal().into_response()
}

fn valid_request_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Accepts a well-formed `x-request-id` or makes one, echoes it, and logs one line per request.
///
/// Only the allowlisted fields are logged. Headers and bodies (in particular the bodies of the
/// auth routes) are never read for logging, and the query string is masked by key.
pub async fn request_context(mut request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_request_id(v))
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let id_value = HeaderValue::from_str(&id).expect("validated request ids are header-safe");
    request
        .headers_mut()
        .insert(REQUEST_ID_HEADER, id_value.clone());

    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = redact_query(request.uri().query());
    let origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.chars().take(200).collect::<String>());
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.chars().take(200).collect::<String>());
    let started = Instant::now();

    let mut response = next.run(request).await;
    response.headers_mut().insert(REQUEST_ID_HEADER, id_value);

    let status = response.status().as_u16();
    let latency_ms = started.elapsed().as_millis() as u64;
    let probe = path == "/healthz" || path == "/readyz";
    if status >= 500 {
        tracing::error!(target: "http", request_id = %id, method = %method, path = %path,
            query = query.as_deref(), status, latency_ms, origin = origin.as_deref(),
            user_agent = user_agent.as_deref(), "request failed");
    } else if probe {
        tracing::debug!(target: "http", request_id = %id, method = %method, path = %path,
            status, latency_ms, "probe");
    } else {
        tracing::info!(target: "http", request_id = %id, method = %method, path = %path,
            query = query.as_deref(), status, latency_ms, origin = origin.as_deref(),
            user_agent = user_agent.as_deref(), "request");
    }
    response
}
