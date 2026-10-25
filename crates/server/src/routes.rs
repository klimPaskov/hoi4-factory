//! Route table and the assembled application.
//!
//! | Path | Auth | Purpose |
//! |---|---|---|
//! | `GET /healthz` | none | Liveness: the process answers |
//! | `GET /readyz` | none | Readiness: database, migrations, storage layout, game reference status |
//! | `GET /api/v1/game-reference` | signed in | Manifest summary of the server game reference |
//! | `/api/v1/auth/*`, `/api/v1/projects`, `/api/v1/projects/{id}`, `.../restore` | see `factory_accounts::router` | Accounts and projects |
//! | `/api/v1/projects/{id}/uploads`, `.../github/*`, `.../workspace` | signed in, project role | See [`crate::intake_api`] |
//! | anything else | none | The browser client (single-page fallback), JSON 404 under `/api` |

use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use factory_accounts::{API_PREFIX, ApiError, Principal};
use serde_json::{Value, json};

use crate::game_reference::GameReferenceHandle;
use crate::hardening::{catch_panic_layer, cors_layer, request_context, security_headers};
use crate::migrations;
use crate::state::ServerState;

const DB_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// The whole application with its hardening layers.
pub fn build_app(state: ServerState) -> Router {
    let api = api_routes(&state);
    let hsts = !state.settings.development;
    let origins = state.settings.cors_origins.clone();
    let body_limit = state.settings.max_body_bytes;
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(fallback)
        .with_state(state)
        .nest(API_PREFIX, api)
        // Innermost first. Preflight and error responses still get the security headers and
        // a request id because those layers sit outside CORS and the panic guard.
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(catch_panic_layer())
        .layer(cors_layer(&origins))
        .layer(from_fn_with_state(hsts, security_headers))
        .layer(from_fn(request_context))
}

/// Everything mounted under `/api/v1`.
///
/// Extension point: merge the Setup adapter routes here. A merged router must not set its own
/// fallback (the accounts router owns the JSON 404) and may raise the body limit for its upload
/// routes with its own `DefaultBodyLimit::max` layer, as the upload route of
/// [`crate::intake_api`] does.
pub fn api_routes(state: &ServerState) -> Router {
    Router::new()
        .route("/game-reference", get(game_reference))
        .with_state(state.clone())
        .merge(crate::intake_api::router(state))
        .merge(crate::setup_api::router(state))
        .merge(crate::agent_api::router(state))
        .merge(factory_accounts::router(state.accounts.clone()))
}

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn readyz(State(state): State<ServerState>) -> Response {
    let pool = state.db.pool();
    let database = matches!(
        tokio::time::timeout(DB_PROBE_TIMEOUT, sqlx::query("SELECT 1").execute(pool)).await,
        Ok(Ok(_))
    );
    let migrations_check = if database {
        match tokio::time::timeout(
            DB_PROBE_TIMEOUT,
            migrations::verify_tables(pool, &state.migrations),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(why)) => Err(why),
            Err(_) => Err("timed out".to_owned()),
        }
    } else {
        Err("database unreachable".to_owned())
    };
    let storage = state.storage.clone();
    let layout_issues = tokio::task::spawn_blocking(move || storage.verify_layout(None))
        .await
        .map_err(|_| "the storage check failed".to_owned());
    let (storage_ok, storage_issues) = match layout_issues {
        Ok(issues) => (issues.is_empty(), json!(issues)),
        Err(why) => (false, json!([{ "directory": "data_root", "problem": why }])),
    };
    let game = state.game_reference.status();
    let game_summary = game.summary();

    let core_ok = database && migrations_check.is_ok() && storage_ok;
    let (code, status) = if !core_ok {
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    } else if !game.is_healthy() {
        (StatusCode::OK, "degraded")
    } else {
        (StatusCode::OK, "ok")
    };
    let body = json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": { "ok": database },
            "migrations": {
                "ok": migrations_check.is_ok(),
                "latest_version": state.migrations.latest_version(),
                "detail": migrations_check.err(),
            },
            "storage": { "ok": storage_ok, "issues": storage_issues },
            "game_reference": {
                "available": game_summary.available,
                "read_only": game_summary.read_only,
                "reason": game_summary.reason,
            },
        },
    });
    (code, Json(body)).into_response()
}

/// Manifest summary only: version, revision, DLC ids, manifest hash and read-only status.
async fn game_reference(
    _principal: Principal,
    State(handle): State<GameReferenceHandle>,
) -> Json<crate::game_reference::GameReferenceSummary> {
    Json(handle.status().summary())
}

async fn fallback(State(state): State<ServerState>, request: Request) -> Response {
    match &state.web {
        Some(site) => site.serve(request).await,
        None => ApiError::not_found().into_response(),
    }
}
