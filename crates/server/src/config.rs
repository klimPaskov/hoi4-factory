//! Server configuration from the environment.
//!
//! The accounts variables (`FACTORY_ENV`, `FACTORY_PUBLIC_URL`, `FACTORY_ALLOWED_ORIGINS`,
//! `FACTORY_SMTP_URL`, GitHub and session settings) are read by
//! [`factory_accounts::AccountsConfig::from_lookup`]; see its documentation. The server adds:
//!
//! | Variable | Meaning | Default |
//! |---|---|---|
//! | `DATABASE_URL` | Connection of the application role (must not be a superuser or own the tables) | required |
//! | `DATABASE_MIGRATION_URL` | Connection of the table-owner role that runs the migrations | `DATABASE_URL` |
//! | `FACTORY_LISTEN` | Listen address | `0.0.0.0:8080` |
//! | `FACTORY_DATA_ROOT` | Root of the server workspaces | `/data` |
//! | `FACTORY_GAME_REF` | Read-only mount of the game folder | `/game` |
//! | `FACTORY_WEB_DIST` | Directory of the built browser client; served when it exists | none |
//! | `FACTORY_MAX_BODY_BYTES` | Default request body limit (routes may raise their own) | `1048576` |
//! | `RUST_LOG` | Log filter | `info,sqlx=warn` |
//!
//! Pool limits (`FACTORY_DB_*`) are read by [`factory_accounts::PoolSettings`].

use std::path::PathBuf;

use factory_accounts::{AccountsConfig, Environment, PoolSettings, Redacted};

/// Everything the server needs to start.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub accounts: AccountsConfig,
    pub listen: String,
    pub database_url: Redacted<String>,
    pub migration_database_url: Option<Redacted<String>>,
    pub pool: PoolSettings,
    pub data_root: PathBuf,
    pub game_ref_dir: PathBuf,
    pub web_dist: Option<PathBuf>,
    pub max_body_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Accounts(#[from] factory_accounts::config::ConfigError),
    #[error(transparent)]
    Database(#[from] factory_accounts::DatabaseSetupError),
    #[error("missing environment variable {0}")]
    Missing(&'static str),
    #[error("invalid {0}: {1}")]
    Invalid(&'static str, String),
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// `from_env` with an injectable lookup, for tests.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let accounts = AccountsConfig::from_lookup(&get)?;
        let non_empty = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = non_empty("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;
        let max_body_bytes = match non_empty("FACTORY_MAX_BODY_BYTES") {
            None => 1024 * 1024,
            Some(v) => v
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    ConfigError::Invalid(
                        "FACTORY_MAX_BODY_BYTES",
                        "expected a positive number".into(),
                    )
                })?,
        };
        validate_origins(&accounts.allowed_origins)?;
        Ok(Self {
            listen: non_empty("FACTORY_LISTEN").unwrap_or_else(|| "0.0.0.0:8080".to_owned()),
            database_url: Redacted(database_url),
            migration_database_url: non_empty("DATABASE_MIGRATION_URL").map(Redacted),
            pool: PoolSettings::from_lookup(&get)?,
            data_root: PathBuf::from(
                non_empty("FACTORY_DATA_ROOT").unwrap_or_else(|| "/data".into()),
            ),
            game_ref_dir: PathBuf::from(
                non_empty("FACTORY_GAME_REF").unwrap_or_else(|| "/game".into()),
            ),
            web_dist: non_empty("FACTORY_WEB_DIST").map(PathBuf::from),
            max_body_bytes,
            accounts,
        })
    }

    pub fn environment(&self) -> Environment {
        self.accounts.environment
    }

    pub fn is_development(&self) -> bool {
        self.accounts.environment == Environment::Development
    }
}

/// CORS must never be allow-all: every entry is a plain `scheme://host[:port]` origin.
fn validate_origins(origins: &[String]) -> Result<(), ConfigError> {
    for origin in origins {
        let invalid = |why: &str| {
            ConfigError::Invalid("FACTORY_ALLOWED_ORIGINS", format!("`{origin}`: {why}"))
        };
        if origin.contains('*') {
            return Err(invalid("wildcards are not allowed"));
        }
        let url = url::Url::parse(origin).map_err(|_| invalid("not a URL"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(invalid("expected http(s)://host[:port]"));
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(invalid("an origin has no path, query or fragment"));
        }
    }
    Ok(())
}

/// The normalised origins CORS may echo.
pub(crate) fn cors_origins(origins: &[String]) -> Vec<String> {
    origins
        .iter()
        .filter_map(|o| url::Url::parse(o).ok())
        .map(|u| u.origin().ascii_serialization())
        .collect()
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
    fn defaults_and_required_values() {
        assert!(matches!(
            ServerConfig::from_lookup(lookup(&[("FACTORY_ENV", "development")])),
            Err(ConfigError::Missing("DATABASE_URL"))
        ));
        let cfg = ServerConfig::from_lookup(lookup(&[
            ("FACTORY_ENV", "development"),
            ("DATABASE_URL", "postgres://app@db/factory"),
        ]))
        .unwrap();
        assert_eq!(cfg.listen, "0.0.0.0:8080");
        assert_eq!(cfg.data_root, PathBuf::from("/data"));
        assert_eq!(cfg.game_ref_dir, PathBuf::from("/game"));
        assert!(cfg.web_dist.is_none() && cfg.migration_database_url.is_none());
        assert_eq!(cfg.max_body_bytes, 1024 * 1024);
        assert!(!format!("{cfg:?}").contains("postgres://"));
    }

    #[test]
    fn production_needs_an_https_public_url() {
        let r = ServerConfig::from_lookup(lookup(&[("DATABASE_URL", "postgres://x@y/z")]));
        assert!(matches!(r, Err(ConfigError::Accounts(_))));
    }

    #[test]
    fn allowed_origins_reject_wildcards_and_paths() {
        for bad in [
            "*",
            "https://*.example.org",
            "https://example.org/app",
            "ftp://x",
            "nonsense",
        ] {
            let r = ServerConfig::from_lookup(lookup(&[
                ("FACTORY_ENV", "development"),
                ("DATABASE_URL", "postgres://x@y/z"),
                ("FACTORY_ALLOWED_ORIGINS", bad),
            ]));
            assert!(r.is_err(), "{bad} must be rejected");
        }
        let ok = ServerConfig::from_lookup(lookup(&[
            ("FACTORY_ENV", "development"),
            ("DATABASE_URL", "postgres://x@y/z"),
            (
                "FACTORY_ALLOWED_ORIGINS",
                "http://localhost:5173, https://factory.example",
            ),
        ]))
        .unwrap();
        assert_eq!(
            cors_origins(&ok.accounts.allowed_origins),
            ["http://localhost:5173", "https://factory.example"]
        );
    }

    #[test]
    fn body_limit_must_be_positive() {
        let r = ServerConfig::from_lookup(lookup(&[
            ("FACTORY_ENV", "development"),
            ("DATABASE_URL", "postgres://x@y/z"),
            ("FACTORY_MAX_BODY_BYTES", "0"),
        ]));
        assert!(r.is_err());
    }
}
