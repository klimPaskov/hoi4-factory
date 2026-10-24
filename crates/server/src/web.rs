//! Static serving of the built browser client with a single-page-app fallback.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use factory_accounts::ApiError;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

/// True for paths that belong to the server API and probes. They never fall back to
/// `index.html`: an unknown API path is a JSON 404.
pub fn is_server_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/healthz" || path == "/readyz"
}

/// The directory of the built client. Cloning is cheap.
#[derive(Clone)]
pub struct StaticSite {
    service: ServeDir<ServeFile>,
}

impl StaticSite {
    /// `Some` when `dist` is a directory.
    pub fn open(dist: &Path) -> Option<Self> {
        if !dist.is_dir() {
            return None;
        }
        let index: PathBuf = dist.join("index.html");
        if !index.is_file() {
            tracing::warn!(
                "the web client directory has no index.html; single-page fallback may fail"
            );
        }
        Some(Self {
            service: ServeDir::new(dist).fallback(ServeFile::new(index)),
        })
    }

    /// Serves a file, or `index.html` for an unknown client route. Hashed build assets under
    /// `/assets/` are cached for a year; HTML is always revalidated.
    pub async fn serve(&self, request: Request) -> Response {
        if is_server_path(request.uri().path()) {
            return ApiError::not_found().into_response();
        }
        let immutable = request.uri().path().starts_with("/assets/");
        let mut response = match self.service.clone().oneshot(request).await {
            Ok(response) => response.map(Body::new),
            Err(never) => match never {},
        };
        let is_html = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/html"));
        if !response.headers().contains_key(CACHE_CONTROL) {
            if response.status() == StatusCode::OK && immutable && !is_html {
                response.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            } else if is_html {
                response
                    .headers_mut()
                    .insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            }
        }
        response
    }
}
