//! Request extractors: JSON bodies with envelope errors, the authenticated principal and the
//! client address.

use std::net::SocketAddr;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRef, FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use super::cookies::{SESSION_COOKIE, read_cookie};
use crate::error::ApiError;
use crate::service::{AccountsState, Principal};

/// `Json<T>` whose failures use the error envelope.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(match rejection {
                JsonRejection::MissingJsonContentType(_) => {
                    ApiError::invalid_request("Send the body as application/json.")
                }
                _ => ApiError::invalid_request("The request body is not valid for this endpoint."),
            }),
        }
    }
}

/// The signed-in account, resolved from the `hf_session` cookie. Any router whose state can
/// produce an [`AccountsState`] may use it as an extractor argument.
impl<S> FromRequestParts<S> for Principal
where
    AccountsState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let accounts = AccountsState::from_ref(state);
        let token =
            read_cookie(&parts.headers, SESSION_COOKIE).ok_or_else(ApiError::unauthenticated)?;
        accounts.authenticate(&token).await
    }
}

/// The caller's address for rate limiting: the connection peer, or the last `X-Forwarded-For`
/// entry (the one appended by the trusted proxy) when `trust_forwarded_for` is set.
pub struct ClientIp(pub String);

impl<S> FromRequestParts<S> for ClientIp
where
    AccountsState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let accounts = AccountsState::from_ref(state);
        if accounts.config().trust_forwarded_for {
            let forwarded = parts
                .headers
                .get_all("x-forwarded-for")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|v| v.split(','))
                .map(str::trim)
                .rfind(|v| !v.is_empty());
            if let Some(ip) = forwarded {
                return Ok(Self(ip.chars().take(64).collect()));
            }
        }
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip().to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        Ok(Self(peer))
    }
}
