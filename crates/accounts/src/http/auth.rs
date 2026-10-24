//! Sign-up, sign-in, sign-out, email verification and GitHub sign-in endpoints.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use subtle::ConstantTimeEq;

use super::cookies::{
    GITHUB_STATE_COOKIE, SESSION_COOKIE, clear_github_state_cookie, clear_session_cookie,
    github_state_cookie, read_cookie, session_cookie,
};
use super::extract::{ApiJson, ClientIp};
use crate::error::{ApiError, RetryClass};
use crate::ids::AccountId;
use crate::service::{AccountsState, ClientInfo, GitHubOutcome, Principal, SessionGrant};

fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
}

fn push_cookie(response: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

fn session_cookie_for(state: &AccountsState, grant: &SessionGrant, now: DateTime<Utc>) -> String {
    let lifetime = (grant.expires_at - now).num_seconds();
    session_cookie(
        &grant.token,
        lifetime,
        state.config().environment.secure_cookies(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignUpBody {
    email: String,
    password: String,
    display_name: Option<String>,
}

pub async fn sign_up(
    State(state): State<AccountsState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ApiJson(body): ApiJson<SignUpBody>,
) -> Result<Response, ApiError> {
    let client = ClientInfo {
        ip: &ip,
        user_agent: user_agent(&headers),
    };
    let signed_in = state
        .sign_up(
            &body.email,
            body.password,
            body.display_name.as_deref(),
            &client,
        )
        .await?;
    let mut response = (
        StatusCode::CREATED,
        Json(json!({
            "account": signed_in.account,
            "verification_email_sent": signed_in.verification_email_sent,
        })),
    )
        .into_response();
    let cookie = session_cookie_for(&state, &signed_in.session, state.db().clock().now());
    push_cookie(&mut response, &cookie);
    Ok(response)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignInBody {
    email: String,
    password: String,
}

pub async fn sign_in(
    State(state): State<AccountsState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ApiJson(body): ApiJson<SignInBody>,
) -> Result<Response, ApiError> {
    let client = ClientInfo {
        ip: &ip,
        user_agent: user_agent(&headers),
    };
    let signed_in = state.sign_in(&body.email, body.password, &client).await?;
    let mut response = Json(json!({ "account": signed_in.account })).into_response();
    let cookie = session_cookie_for(&state, &signed_in.session, state.db().clock().now());
    push_cookie(&mut response, &cookie);
    Ok(response)
}

/// Idempotent: succeeds whether or not a session existed.
pub async fn sign_out(
    State(state): State<AccountsState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(token) = read_cookie(&headers, SESSION_COOKIE) {
        state.sign_out(&token).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    push_cookie(
        &mut response,
        &clear_session_cookie(state.config().environment.secure_cookies()),
    );
    Ok(response)
}

/// Ends every session of the signed-in account, this one included.
pub async fn sign_out_everywhere(
    State(state): State<AccountsState>,
    principal: Principal,
) -> Result<Response, ApiError> {
    let ended = state.revoke_all_sessions(principal.account_id()).await?;
    let mut response = Json(json!({ "sessions_ended": ended })).into_response();
    push_cookie(
        &mut response,
        &clear_session_cookie(state.config().environment.secure_cookies()),
    );
    Ok(response)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequestBody {
    email: String,
}

/// Always answers 202, whether or not the address has an account.
pub async fn request_password_reset(
    State(state): State<AccountsState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ApiJson(body): ApiJson<ResetRequestBody>,
) -> Result<Response, ApiError> {
    let client = ClientInfo {
        ip: &ip,
        user_agent: user_agent(&headers),
    };
    // The work continues in the background; the handle is not awaited on purpose.
    drop(state.request_password_reset(&body.email, &client)?);
    Ok((StatusCode::ACCEPTED, Json(json!({ "status": "ok" }))).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetConfirmBody {
    token: String,
    new_password: String,
}

pub async fn confirm_password_reset(
    State(state): State<AccountsState>,
    ApiJson(body): ApiJson<ResetConfirmBody>,
) -> Result<Response, ApiError> {
    state
        .confirm_password_reset(&body.token, body.new_password)
        .await?;
    Ok(Json(json!({ "status": "password_changed" })).into_response())
}

pub async fn me(principal: Principal) -> Json<serde_json::Value> {
    Json(json!({ "account": principal.account }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyBody {
    token: String,
}

pub async fn verify_email(
    State(state): State<AccountsState>,
    ApiJson(body): ApiJson<VerifyBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let account = state.verify_email(&body.token).await?;
    Ok(Json(json!({ "account": account })))
}

pub async fn resend_verification(
    State(state): State<AccountsState>,
    principal: Principal,
) -> Result<Response, ApiError> {
    let already_verified = principal.account.email_verified;
    let sent = state.resend_verification(&principal).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "sent": sent, "already_verified": already_verified })),
    )
        .into_response())
}

// ----- GitHub --------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct StartQuery {
    /// `link=1` attaches GitHub to the signed-in account instead of signing in with it.
    link: Option<String>,
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
}

pub async fn github_start(
    State(state): State<AccountsState>,
    headers: HeaderMap,
    Query(query): Query<StartQuery>,
) -> Result<Response, ApiError> {
    let (csrf_state, url) = state.github_authorize()?;
    let mut cookie_value = csrf_state;
    if matches!(query.link.as_deref(), Some("1" | "true")) {
        let token = read_cookie(&headers, SESSION_COOKIE).ok_or_else(ApiError::unauthenticated)?;
        let principal = state.authenticate(&token).await?;
        cookie_value = format!("{cookie_value}.{}", principal.account.id);
    }
    let mut response = redirect(&url);
    push_cookie(
        &mut response,
        &github_state_cookie(&cookie_value, state.config().environment.secure_cookies()),
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn state_mismatch() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "github_state_mismatch",
        "The GitHub sign-in could not be matched to this browser. Start again.",
        RetryClass::RefreshPreconditions,
    )
}

pub async fn github_callback(
    State(state): State<AccountsState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let secure = state.config().environment.secure_cookies();
    let result = github_callback_inner(&state, &ip, &headers, query).await;
    let mut response = match result {
        Ok(response) => response,
        // A browser navigation, not an API call: send the person back to the sign-in page with
        // the failure named, instead of showing raw JSON. The code is already stable; it is
        // prefixed with `github_` unless it has that prefix.
        Err(error) => {
            let code = error.code.strip_prefix("github_").unwrap_or(error.code);
            redirect(&format!("/signin?error=github_{code}"))
        }
    };
    // The state is single use, whatever the outcome.
    push_cookie(&mut response, &clear_github_state_cookie(secure));
    response
}

async fn github_callback_inner(
    state: &AccountsState,
    ip: &str,
    headers: &HeaderMap,
    query: CallbackQuery,
) -> Result<Response, ApiError> {
    let cookie = read_cookie(headers, GITHUB_STATE_COOKIE).ok_or_else(state_mismatch)?;
    let (cookie_state, link_marker) = match cookie.split_once('.') {
        Some((s, account)) => (s.to_owned(), Some(account.to_owned())),
        None => (cookie, None),
    };
    let returned = query.state.unwrap_or_default();
    let matches: bool = cookie_state.as_bytes().ct_eq(returned.as_bytes()).into();
    if cookie_state.is_empty() || !matches {
        return Err(state_mismatch());
    }
    if let Some(error) = query.error {
        let _ = error;
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "github_denied",
            "GitHub sign-in was cancelled or denied.",
            RetryClass::RefreshPreconditions,
        ));
    }
    let code = query.code.unwrap_or_default();

    // A link flow must still be running under the same signed-in account.
    let link_to: Option<AccountId> = match link_marker {
        None => None,
        Some(marker) => {
            let expected: AccountId = marker.parse().map_err(|_| state_mismatch())?;
            let token =
                read_cookie(headers, SESSION_COOKIE).ok_or_else(ApiError::unauthenticated)?;
            let principal = state.authenticate(&token).await?;
            if principal.account.id != expected {
                return Err(state_mismatch());
            }
            Some(expected)
        }
    };

    let client = ClientInfo {
        ip,
        user_agent: user_agent(headers),
    };
    let outcome = state.github_complete(&code, link_to, &client).await?;
    let mut response = redirect(&state.config().post_login_redirect);
    if let GitHubOutcome::SignedIn(signed_in) = outcome {
        let cookie = session_cookie_for(state, &signed_in.session, state.db().clock().now());
        push_cookie(&mut response, &cookie);
    }
    Ok(response)
}
