//! Configuration for the accounts component.
//!
//! [`AccountsConfig::from_env`] reads these variables:
//!
//! | Variable | Meaning | Default |
//! |---|---|---|
//! | `FACTORY_ENV` | `development` or `production` | `production` |
//! | `FACTORY_PUBLIC_URL` | Public origin of the service, used for links, redirect URI and origin checks | `http://localhost:8080` in development, required in production (https) |
//! | `FACTORY_ALLOWED_ORIGINS` | Extra comma separated origins accepted on unsafe requests (for example a Vite dev server) | none |
//! | `FACTORY_SMTP_URL` | SMTP transport URL | `smtp://mailpit:1025` |
//! | `FACTORY_MAIL_FROM` | From address | `HOI4 Factory <noreply@localhost>` |
//! | `FACTORY_TRUST_FORWARDED_FOR` | `true` when a trusted reverse proxy sets `X-Forwarded-For` | `false` |
//! | `FACTORY_GITHUB_CLIENT_ID`, `FACTORY_GITHUB_CLIENT_SECRET` | GitHub App user authorization credentials. GitHub sign-in is disabled when absent | none |
//! | `FACTORY_GITHUB_API_BASE`, `FACTORY_GITHUB_AUTHORIZE_URL`, `FACTORY_GITHUB_TOKEN_URL` | Overrides for the GitHub endpoints (tests, GitHub Enterprise) | public GitHub |
//! | `FACTORY_POST_LOGIN_REDIRECT` | Where the GitHub callback sends the browser | `/` |
//! | `FACTORY_SESSION_TTL_DAYS` | Absolute session lifetime | `30` |
//! | `FACTORY_SESSION_IDLE_DAYS` | A session unused for this long expires | `7` |
//! | `DATABASE_URL`, `FACTORY_DB_MAX_CONNECTIONS`, `FACTORY_DB_MIN_CONNECTIONS`, `FACTORY_DB_ACQUIRE_TIMEOUT_SECS`, `FACTORY_DB_IDLE_TIMEOUT_SECS`, `FACTORY_DB_MAX_LIFETIME_SECS` | Connection and pool (read by `Database::from_env`) | required, `10`, `0`, `5`, `600`, `1800` |

use std::fmt;
use std::time::Duration;

use crate::ratelimit::RateLimitConfig;

/// Deployment mode. Controls cookie `Secure` and the database role posture check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    pub fn secure_cookies(self) -> bool {
        self == Environment::Production
    }
}

/// A value that must never reach a log line or a debug dump.
#[derive(Clone)]
pub struct Redacted<T>(pub T);

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy)]
pub struct Argon2Params {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Params {
    /// OWASP's minimum recommendation for Argon2id: 19 MiB, 2 iterations, 1 lane.
    fn default() -> Self {
        Self {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }
    }
}

/// GitHub App user-authorization (OAuth web flow) settings.
#[derive(Debug, Clone)]
pub struct GitHubOAuthConfig {
    pub client_id: String,
    pub client_secret: Redacted<String>,
    /// Must equal a callback URL registered on the GitHub App.
    pub redirect_uri: String,
    pub authorize_url: String,
    pub token_url: String,
    pub api_base: String,
}

impl GitHubOAuthConfig {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        public_url: &str,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: Redacted(client_secret.into()),
            redirect_uri: format!(
                "{}{}/auth/github/callback",
                public_url.trim_end_matches('/'),
                crate::API_PREFIX
            ),
            authorize_url: "https://github.com/login/oauth/authorize".to_owned(),
            token_url: "https://github.com/login/oauth/access_token".to_owned(),
            api_base: "https://api.github.com".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccountsConfig {
    pub environment: Environment,
    pub public_url: String,
    pub allowed_origins: Vec<String>,
    /// Link target in verification mails. The token is appended as `?token=`.
    pub verify_link_base: String,
    pub post_login_redirect: String,
    pub smtp_url: String,
    pub mail_from: String,
    pub trust_forwarded_for: bool,
    /// Absolute lifetime of a session.
    pub session_ttl: Duration,
    /// A session unused for this long expires.
    pub session_idle_ttl: Duration,
    pub verification_ttl: Duration,
    /// Grace period between marking a project for deletion and its permanent removal.
    pub deletion_grace: Duration,
    /// Lifetime of a password reset link.
    pub reset_ttl: Duration,
    /// Link target in password reset mails. The token is appended as `?token=`.
    pub reset_link_base: String,
    pub argon2: Argon2Params,
    pub rate_limits: RateLimitConfig,
    pub github: Option<GitHubOAuthConfig>,
}

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

impl AccountsConfig {
    /// Defaults suitable for local development (insecure cookies, Mailpit, no GitHub).
    pub fn development() -> Self {
        let public_url = "http://localhost:8080".to_owned();
        Self {
            environment: Environment::Development,
            verify_link_base: format!("{public_url}/verify-email"),
            reset_link_base: format!("{public_url}/reset-password"),
            public_url,
            allowed_origins: Vec::new(),
            post_login_redirect: "/".to_owned(),
            smtp_url: "smtp://mailpit:1025".to_owned(),
            mail_from: "HOI4 Factory <noreply@localhost>".to_owned(),
            trust_forwarded_for: false,
            session_ttl: 30 * DAY,
            session_idle_ttl: 7 * DAY,
            verification_ttl: DAY,
            deletion_grace: 30 * DAY,
            reset_ttl: Duration::from_secs(30 * 60),
            argon2: Argon2Params::default(),
            rate_limits: RateLimitConfig::default(),
            github: None,
        }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// `from_env` with an injectable lookup, for tests.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let environment = match get("FACTORY_ENV").as_deref() {
            None | Some("production") => Environment::Production,
            Some("development") => Environment::Development,
            Some(other) => {
                return Err(ConfigError::Invalid(
                    "FACTORY_ENV",
                    format!("unknown environment `{other}`"),
                ));
            }
        };
        let mut cfg = Self::development();
        cfg.environment = environment;
        match get("FACTORY_PUBLIC_URL") {
            Some(url) => cfg.public_url = url.trim_end_matches('/').to_owned(),
            None if environment == Environment::Production => {
                return Err(ConfigError::Missing("FACTORY_PUBLIC_URL"));
            }
            None => {}
        }
        if environment == Environment::Production && !cfg.public_url.starts_with("https://") {
            return Err(ConfigError::Invalid(
                "FACTORY_PUBLIC_URL",
                "must be https in production".to_owned(),
            ));
        }
        cfg.verify_link_base = format!("{}/verify-email", cfg.public_url);
        cfg.reset_link_base = format!("{}/reset-password", cfg.public_url);
        if let Some(list) = get("FACTORY_ALLOWED_ORIGINS") {
            cfg.allowed_origins = list
                .split(',')
                .map(|s| s.trim().trim_end_matches('/').to_owned())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(v) = get("FACTORY_SMTP_URL") {
            cfg.smtp_url = v;
        }
        if let Some(v) = get("FACTORY_MAIL_FROM") {
            cfg.mail_from = v;
        }
        if let Some(v) = get("FACTORY_POST_LOGIN_REDIRECT") {
            cfg.post_login_redirect = v;
        }
        for (key, target) in [
            ("FACTORY_SESSION_TTL_DAYS", &mut cfg.session_ttl),
            ("FACTORY_SESSION_IDLE_DAYS", &mut cfg.session_idle_ttl),
        ] {
            if let Some(v) = get(key) {
                let days: u64 = v.trim().parse().ok().filter(|d| *d > 0).ok_or_else(|| {
                    ConfigError::Invalid(key, "expected a positive number of days".to_owned())
                })?;
                *target = Duration::from_secs(days * 24 * 60 * 60);
            }
        }
        if cfg.session_idle_ttl > cfg.session_ttl {
            cfg.session_idle_ttl = cfg.session_ttl;
        }
        cfg.trust_forwarded_for = matches!(
            get("FACTORY_TRUST_FORWARDED_FOR").as_deref(),
            Some("true" | "1")
        );
        let id = get("FACTORY_GITHUB_CLIENT_ID").filter(|s| !s.is_empty());
        let secret = get("FACTORY_GITHUB_CLIENT_SECRET").filter(|s| !s.is_empty());
        match (id, secret) {
            (Some(id), Some(secret)) => {
                let mut gh = GitHubOAuthConfig::new(id, secret, &cfg.public_url);
                if let Some(v) = get("FACTORY_GITHUB_API_BASE") {
                    gh.api_base = v;
                }
                if let Some(v) = get("FACTORY_GITHUB_AUTHORIZE_URL") {
                    gh.authorize_url = v;
                }
                if let Some(v) = get("FACTORY_GITHUB_TOKEN_URL") {
                    gh.token_url = v;
                }
                cfg.github = Some(gh);
            }
            (None, None) => {}
            _ => {
                return Err(ConfigError::Invalid(
                    "FACTORY_GITHUB_CLIENT_ID",
                    "client id and client secret must be set together".to_owned(),
                ));
            }
        }
        Ok(cfg)
    }

    /// Origins accepted on state-changing requests.
    pub(crate) fn accepted_origins(&self) -> Vec<String> {
        let mut origins = vec![origin_of(&self.public_url)];
        origins.extend(self.allowed_origins.iter().map(|o| origin_of(o)));
        origins
    }
}

/// `scheme://host[:port]` of a URL, lowercased, without a trailing slash.
pub(crate) fn origin_of(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => u.origin().ascii_serialization(),
        Err(_) => url.trim_end_matches('/').to_ascii_lowercase(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing environment variable {0}")]
    Missing(&'static str),
    #[error("invalid {0}: {1}")]
    Invalid(&'static str, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn production_is_the_default_and_requires_https_public_url() {
        assert!(matches!(
            AccountsConfig::from_lookup(lookup(&[])),
            Err(ConfigError::Missing("FACTORY_PUBLIC_URL"))
        ));
        assert!(
            AccountsConfig::from_lookup(lookup(&[("FACTORY_PUBLIC_URL", "http://example.org")]))
                .is_err()
        );
        let cfg =
            AccountsConfig::from_lookup(lookup(&[("FACTORY_PUBLIC_URL", "https://example.org/")]))
                .unwrap();
        assert_eq!(cfg.environment, Environment::Production);
        assert!(cfg.environment.secure_cookies());
        assert_eq!(cfg.verify_link_base, "https://example.org/verify-email");
    }

    #[test]
    fn development_uses_mailpit_and_insecure_cookies() {
        let cfg = AccountsConfig::from_lookup(lookup(&[("FACTORY_ENV", "development")])).unwrap();
        assert!(!cfg.environment.secure_cookies());
        assert_eq!(cfg.smtp_url, "smtp://mailpit:1025");
        assert!(cfg.github.is_none());
    }

    #[test]
    fn github_needs_both_halves_and_never_prints_the_secret() {
        let both = lookup(&[
            ("FACTORY_ENV", "development"),
            ("FACTORY_GITHUB_CLIENT_ID", "Iv1.abc"),
            ("FACTORY_GITHUB_CLIENT_SECRET", "super-secret-value"),
        ]);
        let cfg = AccountsConfig::from_lookup(both).unwrap();
        let gh = cfg.github.as_ref().unwrap();
        assert_eq!(
            gh.redirect_uri,
            "http://localhost:8080/api/v1/auth/github/callback"
        );
        assert!(!format!("{cfg:?}").contains("super-secret-value"));
        let half = lookup(&[
            ("FACTORY_ENV", "development"),
            ("FACTORY_GITHUB_CLIENT_ID", "Iv1.abc"),
        ]);
        assert!(AccountsConfig::from_lookup(half).is_err());
    }

    #[test]
    fn session_lifetimes_are_configurable() {
        let cfg = AccountsConfig::from_lookup(lookup(&[
            ("FACTORY_ENV", "development"),
            ("FACTORY_SESSION_TTL_DAYS", "14"),
            ("FACTORY_SESSION_IDLE_DAYS", "3"),
        ]))
        .unwrap();
        assert_eq!(cfg.session_ttl.as_secs(), 14 * 86_400);
        assert_eq!(cfg.session_idle_ttl.as_secs(), 3 * 86_400);
        assert!(
            AccountsConfig::from_lookup(lookup(&[
                ("FACTORY_ENV", "development"),
                ("FACTORY_SESSION_TTL_DAYS", "0")
            ]))
            .is_err()
        );
        // Idle can never exceed the absolute lifetime.
        let cfg = AccountsConfig::from_lookup(lookup(&[
            ("FACTORY_ENV", "development"),
            ("FACTORY_SESSION_TTL_DAYS", "2"),
        ]))
        .unwrap();
        assert_eq!(cfg.session_idle_ttl, cfg.session_ttl);
    }

    #[test]
    fn origin_normalisation() {
        assert_eq!(origin_of("https://Example.org/path"), "https://example.org");
        assert_eq!(origin_of("http://localhost:5173/"), "http://localhost:5173");
    }
}
