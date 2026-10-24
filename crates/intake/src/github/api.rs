//! The GitHub REST surface that intake uses, behind a trait.
//!
//! [`HttpGithubApi`] talks to `api.github.com` (or a test server) with `reqwest`. Nothing
//! else in the crate makes a GitHub HTTP call, so tests replace this one trait with a local
//! fake. The tokens passed to these methods are held only for the call.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::IntakeError;
use crate::secrets::SecretString;

/// Where GitHub's API and Git transport live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubEndpoints {
    /// REST base, no trailing slash.
    pub api_base: String,
    /// Git transport base. A repository is fetched from `<clone_base>/<owner>/<name>.git`.
    pub clone_base: String,
}

impl GithubEndpoints {
    pub fn github_com() -> Self {
        GithubEndpoints {
            api_base: "https://api.github.com".into(),
            clone_base: "https://github.com".into(),
        }
    }

    /// Custom endpoints. `https` is required unless the host is a loopback address, so a
    /// credential never travels in clear text to another machine.
    pub fn custom(api_base: &str, clone_base: &str) -> Result<Self, IntakeError> {
        for base in [api_base, clone_base] {
            let parsed = url::Url::parse(base)
                .map_err(|_| IntakeError::InvalidRequest("endpoint is not a URL".into()))?;
            let loopback = matches!(
                parsed.host_str(),
                Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
            );
            let ok = parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback);
            if !ok
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(IntakeError::InvalidRequest(
                    "endpoint must be https (http only on loopback) without credentials".into(),
                ));
            }
        }
        Ok(GithubEndpoints {
            api_base: api_base.trim_end_matches('/').to_owned(),
            clone_base: clone_base.trim_end_matches('/').to_owned(),
        })
    }

    pub(crate) fn clone_url(&self, full_name: &str) -> String {
        format!("{}/{}.git", self.clone_base, full_name)
    }
}

/// `GET /app/installations/{id}`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstallationInfo {
    pub id: u64,
    /// `all` or `selected`.
    pub repository_selection: String,
    #[serde(default)]
    pub suspended_at: Option<String>,
    #[serde(default)]
    pub account: Option<InstallationAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstallationAccount {
    pub login: String,
}

/// A repository as GitHub describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct RepositoryInfo {
    pub id: u64,
    pub node_id: String,
    pub full_name: String,
    pub default_branch: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub archived: bool,
    /// Kibibytes, as GitHub reports it.
    #[serde(default)]
    pub size: u64,
}

/// A short-lived installation token.
#[derive(Debug, Clone)]
pub struct InstallationToken {
    pub token: SecretString,
    pub expires_at: Option<String>,
    /// Full names of the repositories the token covers, as GitHub returned them.
    pub repositories: Vec<String>,
}

/// Failure of one GitHub call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubError {
    /// GitHub answered with this status.
    Status {
        status: u16,
        message: String,
    },
    RateLimited {
        retry_after: Option<Duration>,
    },
    /// No answer: connection, TLS or timeout.
    Transport(String),
    /// The answer was not what the API documents.
    Protocol(String),
}

impl GithubError {
    /// 401 and 404 on an installation or repository mean the connection is gone.
    pub fn is_revocation(&self) -> bool {
        matches!(
            self,
            GithubError::Status {
                status: 401 | 404,
                ..
            }
        )
    }
}

impl From<GithubError> for IntakeError {
    fn from(err: GithubError) -> Self {
        match err {
            GithubError::Status {
                status: 401 | 404, ..
            } => IntakeError::AccessRevoked,
            GithubError::Status {
                status: 403,
                message,
            } => IntakeError::GithubUnavailable(format!("permission denied: {message}")),
            GithubError::Status { status, message } => {
                IntakeError::GithubUnavailable(format!("status {status}: {message}"))
            }
            GithubError::RateLimited { retry_after } => {
                IntakeError::GithubRateLimited { retry_after }
            }
            GithubError::Transport(message) => IntakeError::GithubUnavailable(message),
            GithubError::Protocol(message) => IntakeError::GithubProtocol(message),
        }
    }
}

/// The calls intake makes. Methods that take `jwt` authenticate as the App, methods that
/// take `token` authenticate as an installation.
#[async_trait]
pub trait GithubApi: Send + Sync + 'static {
    async fn get_installation(
        &self,
        jwt: &str,
        installation_id: u64,
    ) -> Result<InstallationInfo, GithubError>;

    /// Mint an installation token. `repositories` limits it to those repository names
    /// (without owner). `None` covers every repository of the installation.
    async fn create_installation_token(
        &self,
        jwt: &str,
        installation_id: u64,
        repositories: Option<&[String]>,
    ) -> Result<InstallationToken, GithubError>;

    /// Every repository the token can see.
    async fn list_installation_repositories(
        &self,
        token: &str,
    ) -> Result<Vec<RepositoryInfo>, GithubError>;

    async fn get_repository(
        &self,
        token: &str,
        full_name: &str,
    ) -> Result<RepositoryInfo, GithubError>;
}

/// `reqwest` implementation.
#[derive(Debug, Clone)]
pub struct HttpGithubApi {
    client: reqwest::Client,
    base: String,
}

const MAX_PAGES: u32 = 50;

impl HttpGithubApi {
    pub fn new(endpoints: &GithubEndpoints) -> Result<Self, IntakeError> {
        let client = reqwest::Client::builder()
            .user_agent("hoi4-factory-intake")
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| IntakeError::GithubUnavailable(e.without_url().to_string()))?;
        Ok(HttpGithubApi {
            client,
            base: endpoints.api_base.clone(),
        })
    }

    async fn send<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        bearer: &str,
    ) -> Result<T, GithubError> {
        let response = request
            .bearer_auth(bearer)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| GithubError::Transport(e.without_url().to_string()))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        let remaining_zero = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        if !status.is_success() {
            if status.as_u16() == 429
                || (status.as_u16() == 403 && (remaining_zero || retry_after.is_some()))
            {
                return Err(GithubError::RateLimited { retry_after });
            }
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                .unwrap_or_default();
            return Err(GithubError::Status {
                status: status.as_u16(),
                message: crate::error::shorten(&message),
            });
        }
        response
            .json::<T>()
            .await
            .map_err(|e| GithubError::Protocol(e.without_url().to_string()))
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    repositories: Vec<TokenRepository>,
}

#[derive(Deserialize)]
struct TokenRepository {
    full_name: String,
}

#[derive(Deserialize)]
struct RepositoryPage {
    #[serde(default)]
    repositories: Vec<RepositoryInfo>,
}

#[async_trait]
impl GithubApi for HttpGithubApi {
    async fn get_installation(
        &self,
        jwt: &str,
        installation_id: u64,
    ) -> Result<InstallationInfo, GithubError> {
        let url = format!("{}/app/installations/{installation_id}", self.base);
        self.send(self.client.get(url), jwt).await
    }

    async fn create_installation_token(
        &self,
        jwt: &str,
        installation_id: u64,
        repositories: Option<&[String]>,
    ) -> Result<InstallationToken, GithubError> {
        let url = format!(
            "{}/app/installations/{installation_id}/access_tokens",
            self.base
        );
        let body = match repositories {
            Some(names) => serde_json::json!({ "repositories": names }),
            None => serde_json::json!({}),
        };
        let response: TokenResponse = self.send(self.client.post(url).json(&body), jwt).await?;
        Ok(InstallationToken {
            token: SecretString::new(response.token),
            expires_at: response.expires_at,
            repositories: response
                .repositories
                .into_iter()
                .map(|r| r.full_name)
                .collect(),
        })
    }

    async fn list_installation_repositories(
        &self,
        token: &str,
    ) -> Result<Vec<RepositoryInfo>, GithubError> {
        let mut all = Vec::new();
        for page in 1..=MAX_PAGES {
            let url = format!(
                "{}/installation/repositories?per_page=100&page={page}",
                self.base
            );
            let batch: RepositoryPage = self.send(self.client.get(url), token).await?;
            let count = batch.repositories.len();
            all.extend(batch.repositories);
            if count < 100 {
                return Ok(all);
            }
        }
        Err(GithubError::Protocol(
            "installation lists more repositories than intake reads".into(),
        ))
    }

    async fn get_repository(
        &self,
        token: &str,
        full_name: &str,
    ) -> Result<RepositoryInfo, GithubError> {
        let url = format!("{}/repos/{full_name}", self.base);
        self.send(self.client.get(url), token).await
    }
}
