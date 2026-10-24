//! The server side of the GitHub App: where the credentials come from and how a signed-in user's
//! App installation is found.
//!
//! * [`ConfigSecrets`] is the [`SecretSource`] that intake reads. It serves the App id and the
//!   private key from the server configuration, and answers an installation reference such as
//!   `github_app.installation.123456` with the id it names. Nothing is persisted: a
//!   `repository_links` row stores only the reference (C-10).
//! * [`AppInstallations`] lists the installations of the App. [`HttpAppInstallations`] is the
//!   `reqwest` implementation. Tests replace it with a fixed list.
//! * [`GithubRuntime`] is what the routes use. It exists only when the App is configured and its
//!   key parsed at startup, so a server without GitHub credentials reports GitHub as unavailable
//!   and keeps serving everything else.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use factory_intake::{
    GithubEndpoints, GithubError, IntakeError, SecretError, SecretSource, SecretString, app_jwt,
};
use serde::Deserialize;

use crate::config::GithubAppSettings;

/// Secret-store key of the App id (the default of `factory_intake::GithubConfig`).
pub const APP_ID_SECRET: &str = "github_app.id";
/// Secret-store key of the App private key (the default of `factory_intake::GithubConfig`).
pub const APP_KEY_SECRET: &str = "github_app.private_key";
const INSTALLATION_PREFIX: &str = "github_app.installation.";

/// Installation reference for `installation_id`: a key name, never a credential.
pub fn installation_reference(installation_id: u64) -> String {
    format!("{INSTALLATION_PREFIX}{installation_id}")
}

/// Secrets from the server configuration.
pub struct ConfigSecrets {
    app_id: Option<String>,
    private_key: Option<SecretString>,
}

impl ConfigSecrets {
    pub fn new(app_id: Option<String>, private_key: Option<SecretString>) -> Self {
        ConfigSecrets {
            app_id,
            private_key,
        }
    }
}

impl std::fmt::Debug for ConfigSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigSecrets").finish_non_exhaustive()
    }
}

impl SecretSource for ConfigSecrets {
    fn get(&self, name: &str) -> Result<Option<SecretString>, SecretError> {
        if name == APP_ID_SECRET {
            return Ok(self.app_id.as_deref().map(SecretString::new));
        }
        if name == APP_KEY_SECRET {
            return Ok(self.private_key.clone());
        }
        if let Some(id) = name.strip_prefix(INSTALLATION_PREFIX)
            && !id.is_empty()
            && id.len() <= 20
            && id.bytes().all(|b| b.is_ascii_digit())
        {
            return Ok(Some(SecretString::new(id)));
        }
        Ok(None)
    }
}

/// One installation of the App, as `GET /app/installations` describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppInstallation {
    pub id: u64,
    /// `all` or `selected`.
    pub repository_selection: String,
    #[serde(default)]
    pub suspended_at: Option<String>,
    pub account: AppInstallationAccount,
}

/// The user or organization the App is installed on.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppInstallationAccount {
    /// GitHub's numeric id, the same id as `accounts.github_user_id` for a user.
    pub id: u64,
    pub login: String,
}

/// The installations of the App, found with the App JWT.
#[async_trait]
pub trait AppInstallations: Send + Sync + 'static {
    async fn list(&self, jwt: &str) -> Result<Vec<AppInstallation>, GithubError>;
}

const MAX_PAGES: u32 = 20;

/// `GET {api}/app/installations`, all pages.
#[derive(Debug, Clone)]
pub struct HttpAppInstallations {
    client: reqwest::Client,
    base: String,
}

impl HttpAppInstallations {
    pub fn new(endpoints: &GithubEndpoints) -> Result<Self, IntakeError> {
        let client = reqwest::Client::builder()
            .user_agent("hoi4-factory-server")
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| IntakeError::GithubUnavailable(e.without_url().to_string()))?;
        Ok(HttpAppInstallations {
            client,
            base: endpoints.api_base.clone(),
        })
    }
}

#[async_trait]
impl AppInstallations for HttpAppInstallations {
    async fn list(&self, jwt: &str) -> Result<Vec<AppInstallation>, GithubError> {
        let mut all = Vec::new();
        for page in 1..=MAX_PAGES {
            let url = format!("{}/app/installations?per_page=100&page={page}", self.base);
            let response = self
                .client
                .get(url)
                .bearer_auth(jwt)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send()
                .await
                .map_err(|e| GithubError::Transport(e.without_url().to_string()))?;
            let status = response.status();
            if status.as_u16() == 429 || status.as_u16() == 403 {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(Duration::from_secs);
                let rate_limited = status.as_u16() == 429
                    || retry_after.is_some()
                    || response
                        .headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        == Some("0");
                if rate_limited {
                    return Err(GithubError::RateLimited { retry_after });
                }
            }
            if !status.is_success() {
                return Err(GithubError::Status {
                    status: status.as_u16(),
                    message: String::new(),
                });
            }
            let batch: Vec<AppInstallation> = response
                .json()
                .await
                .map_err(|e| GithubError::Protocol(e.without_url().to_string()))?;
            let count = batch.len();
            all.extend(batch);
            if count < 100 {
                return Ok(all);
            }
        }
        Err(GithubError::Protocol(
            "the App has more installations than the server reads".into(),
        ))
    }
}

/// GitHub as the routes use it. Present only when the App is configured.
#[derive(Clone)]
pub struct GithubRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    app_id: String,
    private_key: SecretString,
    slug: String,
    installations: Arc<dyn AppInstallations>,
}

impl std::fmt::Debug for GithubRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubRuntime")
            .field("app_id", &self.inner.app_id)
            .finish_non_exhaustive()
    }
}

impl GithubRuntime {
    pub fn new(
        settings: &GithubAppSettings,
        private_key: SecretString,
        installations: Arc<dyn AppInstallations>,
    ) -> Self {
        GithubRuntime {
            inner: Arc::new(RuntimeInner {
                app_id: settings.app_id.clone(),
                private_key,
                slug: settings.slug.clone(),
                installations,
            }),
        }
    }

    /// Where a user installs the App.
    pub fn install_url(&self) -> String {
        format!(
            "https://github.com/apps/{}/installations/new",
            self.inner.slug
        )
    }

    /// A fresh App JWT.
    pub fn jwt(&self) -> Result<SecretString, IntakeError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        app_jwt(&self.inner.app_id, &self.inner.private_key, now)
    }

    /// The installation on the GitHub account with numeric id `github_user_id`, if the App is
    /// installed there.
    pub async fn installation_of(
        &self,
        github_user_id: u64,
    ) -> Result<Option<AppInstallation>, IntakeError> {
        let jwt = self.jwt()?;
        let installations = self.inner.installations.list(jwt.expose()).await?;
        Ok(installations
            .into_iter()
            .find(|i| i.account.id == github_user_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_secrets_serve_only_what_is_configured() {
        let secrets = ConfigSecrets::new(Some("42".into()), Some(SecretString::new("PEM")));
        assert_eq!(secrets.get(APP_ID_SECRET).unwrap().unwrap().expose(), "42");
        assert_eq!(
            secrets.get(APP_KEY_SECRET).unwrap().unwrap().expose(),
            "PEM"
        );
        assert_eq!(
            secrets
                .get(&installation_reference(777))
                .unwrap()
                .unwrap()
                .expose(),
            "777"
        );
        for other in [
            "github_app.installation.",
            "github_app.installation.12x",
            "github_app.installation.123456789012345678901",
            "something.else",
            "",
        ] {
            assert!(secrets.get(other).unwrap().is_none(), "{other}");
        }
        let empty = ConfigSecrets::new(None, None);
        assert!(empty.get(APP_ID_SECRET).unwrap().is_none());
        assert!(empty.get(APP_KEY_SECRET).unwrap().is_none());
        assert!(!format!("{secrets:?}").contains("PEM"));
    }

    #[test]
    fn installation_references_are_valid_key_names() {
        assert!(factory_intake::valid_secret_name(&installation_reference(
            5_453_675
        )));
    }
}
