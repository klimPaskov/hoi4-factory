//! The axum router. Mount it at `/api/v1`.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use crate::config::origin_of;
use crate::error::{ApiError, RetryClass};
use crate::service::AccountsState;

mod auth;
pub mod cookies;
mod extract;
mod projects;

pub use extract::{ApiJson, ClientIp};

/// Routes relative to `/api/v1`:
///
/// | Method and path | Purpose |
/// |---|---|
/// | `POST /auth/signup` | Create an email and password account, send the verification mail, sign in |
/// | `POST /auth/signin` | Email and password sign-in |
/// | `POST /auth/signout` | End the session |
/// | `POST /auth/signout-all` | End every session of the account |
/// | `POST /auth/password-reset/request` | Mail a reset link (always answers 202) |
/// | `POST /auth/password-reset/confirm` | Set a new password with the link's token; ends all sessions |
/// | `GET /auth/me` | The signed-in account |
/// | `POST /auth/verify-email` | Redeem a verification token |
/// | `POST /auth/resend-verification` | Send a new verification mail |
/// | `GET /auth/github/start` | Begin GitHub sign-in (`?link=1` links the signed-in account) |
/// | `GET /auth/github/callback` | Finish GitHub sign-in |
/// | `POST /projects`, `GET /projects` | Create, list |
/// | `GET`, `PATCH`, `DELETE /projects/{id}` | Open, rename or change visibility, mark for deletion |
/// | `POST /projects/{id}/restore` | Cancel a pending deletion |
///
/// For the client address (rate limiting), serve the application with
/// `into_make_service_with_connect_info::<SocketAddr>()` or set `FACTORY_TRUST_FORWARDED_FOR`
/// behind a trusted proxy.
pub fn router(state: AccountsState) -> Router {
    Router::new()
        .route("/auth/signup", post(auth::sign_up))
        .route("/auth/signin", post(auth::sign_in))
        .route("/auth/signout", post(auth::sign_out))
        .route("/auth/signout-all", post(auth::sign_out_everywhere))
        .route(
            "/auth/password-reset/request",
            post(auth::request_password_reset),
        )
        .route(
            "/auth/password-reset/confirm",
            post(auth::confirm_password_reset),
        )
        .route("/auth/me", get(auth::me))
        .route("/auth/verify-email", post(auth::verify_email))
        .route("/auth/resend-verification", post(auth::resend_verification))
        .route("/auth/github/start", get(auth::github_start))
        .route("/auth/github/callback", get(auth::github_callback))
        .route("/projects", post(projects::create).get(projects::list))
        .route(
            "/projects/{id}",
            get(projects::get)
                .patch(projects::update)
                .delete(projects::mark_for_deletion),
        )
        .route("/projects/{id}/restore", post(projects::restore))
        .fallback(|| async { ApiError::not_found().into_response() })
        .layer(middleware::from_fn_with_state(state.clone(), origin_guard))
        .with_state(state)
}

/// Cross-site request protection for state-changing requests, on top of SameSite=Lax. A request
/// that names an origin must name one of ours. Without an Origin header, a browser-declared
/// cross-site fetch is refused. Non-browser clients send neither and pass.
async fn origin_guard(
    State(state): State<AccountsState>,
    request: Request,
    next: Next,
) -> Response {
    let unsafe_method = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    if unsafe_method {
        let headers = request.headers();
        let refused = match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            Some(origin) => !state
                .config()
                .accepted_origins()
                .contains(&origin_of(origin)),
            None => headers
                .get("sec-fetch-site")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == "cross-site"),
        };
        if refused {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "origin_not_allowed",
                "This request came from an origin that is not allowed.",
                RetryClass::Terminal,
            )
            .into_response();
        }
    }
    next.run(request).await
}
