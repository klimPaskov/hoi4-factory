//! GitHub App user authorization (the OAuth web flow) behind a trait.
//!
//! The HTTP implementation takes every endpoint from configuration, so tests point it at a local
//! fake server. The user access token lives only in memory for the duration of the callback.

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::config::GitHubOAuthConfig;

/// A GitHub user-to-server token. Its debug output is redacted and it is never stored.
pub struct UserToken(String);

impl UserToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for UserToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UserToken([redacted])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("could not reach GitHub: {0}")]
    Transport(String),
    /// GitHub answered with an OAuth error such as `bad_verification_code`.
    #[error("GitHub rejected the request: {0}")]
    Rejected(String),
    #[error("unexpected response from GitHub: {0}")]
    BadResponse(String),
}

#[async_trait]
pub trait GitHubClient: Send + Sync + 'static {
    /// The URL to send the browser to. `state` is the anti-forgery value.
    fn authorize_url(&self, state: &str) -> String;
    async fn exchange_code(&self, code: &str) -> Result<UserToken, GitHubError>;
    async fn fetch_user(&self, token: &UserToken) -> Result<GitHubUser, GitHubError>;
    /// The primary email address if GitHub has verified it. `None` when there is none, or when the
    /// App lacks the "Email addresses" permission.
    async fn fetch_verified_primary_email(
        &self,
        token: &UserToken,
    ) -> Result<Option<String>, GitHubError>;
}

pub struct HttpGitHubClient {
    config: GitHubOAuthConfig,
    http: reqwest::Client,
}

impl HttpGitHubClient {
    pub fn new(config: GitHubOAuthConfig) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("hoi4-factory/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { config, http })
    }

    fn api(&self, path: &str) -> String {
        format!("{}{}", self.config.api_base.trim_end_matches('/'), path)
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct UserResponse {
    id: i64,
    login: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct EmailEntry {
    email: String,
    primary: bool,
    verified: bool,
}

fn transport(e: reqwest::Error) -> GitHubError {
    // reqwest errors can embed the URL. The URLs here carry no secrets, but keep the text short.
    GitHubError::Transport(if e.is_timeout() {
        "timeout".to_owned()
    } else {
        "request failed".to_owned()
    })
}

#[async_trait]
impl GitHubClient for HttpGitHubClient {
    fn authorize_url(&self, state: &str) -> String {
        match url::Url::parse_with_params(
            &self.config.authorize_url,
            &[
                ("client_id", self.config.client_id.as_str()),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("state", state),
            ],
        ) {
            Ok(u) => u.to_string(),
            Err(_) => self.config.authorize_url.clone(),
        }
    }

    async fn exchange_code(&self, code: &str) -> Result<UserToken, GitHubError> {
        let response = self
            .http
            .post(&self.config.token_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.0.as_str()),
                ("code", code),
                ("redirect_uri", self.config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(transport)?;
        let status = response.status();
        let body: TokenResponse = response.json().await.map_err(|_| {
            GitHubError::BadResponse(format!("token endpoint answered {status} without JSON"))
        })?;
        match (body.access_token, body.error) {
            (Some(token), _) if !token.is_empty() => Ok(UserToken::new(token)),
            (_, Some(error)) => Err(GitHubError::Rejected(error)),
            _ => Err(GitHubError::BadResponse(
                "no access token in the response".to_owned(),
            )),
        }
    }

    async fn fetch_user(&self, token: &UserToken) -> Result<GitHubUser, GitHubError> {
        let response = self
            .http
            .get(self.api("/user"))
            .bearer_auth(token.expose())
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(transport)?;
        if !response.status().is_success() {
            return Err(GitHubError::BadResponse(format!(
                "/user answered {}",
                response.status()
            )));
        }
        let user: UserResponse = response.json().await.map_err(|_| {
            GitHubError::BadResponse("/user did not return the expected JSON".to_owned())
        })?;
        Ok(GitHubUser {
            id: user.id,
            login: user.login,
            name: user.name,
        })
    }

    async fn fetch_verified_primary_email(
        &self,
        token: &UserToken,
    ) -> Result<Option<String>, GitHubError> {
        let response = self
            .http
            .get(self.api("/user/emails"))
            .bearer_auth(token.expose())
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(transport)?;
        if matches!(response.status().as_u16(), 403 | 404) {
            // The App was not granted the email permission.
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(GitHubError::BadResponse(format!(
                "/user/emails answered {}",
                response.status()
            )));
        }
        let entries: Vec<EmailEntry> = response.json().await.map_err(|_| {
            GitHubError::BadResponse("/user/emails did not return the expected JSON".to_owned())
        })?;
        Ok(entries
            .into_iter()
            .find(|e| e.primary && e.verified)
            .map(|e| e.email))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_url_carries_client_id_redirect_and_state_but_no_secret() {
        let cfg =
            GitHubOAuthConfig::new("Iv1.client", "the-client-secret", "https://factory.example");
        let client = HttpGitHubClient::new(cfg).unwrap();
        let url = client.authorize_url("st4te");
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("github.com"));
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "Iv1.client");
        assert_eq!(q["state"], "st4te");
        assert_eq!(
            q["redirect_uri"],
            "https://factory.example/api/v1/auth/github/callback"
        );
        assert!(!url.contains("the-client-secret"));
    }

    #[test]
    fn tokens_are_redacted_in_debug_output() {
        assert!(!format!("{:?}", UserToken::new("ghu_secret")).contains("ghu_secret"));
    }
}
