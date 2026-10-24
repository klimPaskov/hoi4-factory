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
//! | `FACTORY_UPLOAD_MAX_BYTES` | Body limit of the upload route only, also the intake limit for the upload as received | `536870912` (512 MiB) |
//! | `FACTORY_INTAKE_MAX_TOTAL_BYTES`, `FACTORY_INTAKE_MAX_FILE_BYTES`, `FACTORY_INTAKE_MAX_FILES`, `FACTORY_INTAKE_MAX_ENTRIES` | Further upload limits (bytes after extraction, bytes of one file, files, entries) | intake defaults |
//! | `FACTORY_GITHUB_APP_ID` | Numeric id of the HOI4 Factory GitHub App. GitHub import is unavailable when absent | none |
//! | `FACTORY_GITHUB_APP_PRIVATE_KEY_FILE` | Path of the App private key (PEM), read at startup and never logged | none |
//! | `FACTORY_GITHUB_APP_SLUG` | App slug, for the installation link | `hoi4-factory-dev` |
//! | `FACTORY_GITHUB_APP_API_BASE`, `FACTORY_GITHUB_CLONE_BASE` | GitHub REST and Git transport bases (tests; `https`, or `http` on loopback) | `https://api.github.com`, `https://github.com` |
//! | `RUST_LOG` | Log filter | `info,sqlx=warn` |
//!
//! Pool limits (`FACTORY_DB_*`) are read by [`factory_accounts::PoolSettings`].

use std::path::PathBuf;

use factory_accounts::{AccountsConfig, Environment, PoolSettings, Redacted};
use factory_intake::{GithubEndpoints, IntakeLimits};

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
    pub intake: IntakeSettings,
    /// `None` when `FACTORY_GITHUB_APP_ID` or the key file setting is absent.
    pub github_app: Option<GithubAppSettings>,
}

/// Upload route and intake limits.
#[derive(Debug, Clone)]
pub struct IntakeSettings {
    /// Body limit of the upload route. Equals `limits.max_upload_bytes`.
    pub upload_max_bytes: usize,
    pub limits: IntakeLimits,
}

/// The HOI4 Factory GitHub App as the server is configured to use it.
#[derive(Debug, Clone)]
pub struct GithubAppSettings {
    pub app_id: String,
    /// Where the PEM lives. The server reads it once at startup.
    pub private_key_file: PathBuf,
    pub slug: String,
    pub endpoints: GithubEndpoints,
}

const DEFAULT_UPLOAD_MAX_BYTES: usize = 512 * 1024 * 1024;

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
        let intake = intake_settings(&non_empty)?;
        let github_app = github_app_settings(&non_empty)?;
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
            intake,
            github_app,
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

fn positive(key: &'static str, raw: &str) -> Result<u64, ConfigError> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| ConfigError::Invalid(key, "expected a positive number".into()))
}

fn intake_settings(
    non_empty: &impl Fn(&str) -> Option<String>,
) -> Result<IntakeSettings, ConfigError> {
    let mut limits = IntakeLimits::default();
    let upload_max_bytes = match non_empty("FACTORY_UPLOAD_MAX_BYTES") {
        None => DEFAULT_UPLOAD_MAX_BYTES,
        Some(raw) => usize::try_from(positive("FACTORY_UPLOAD_MAX_BYTES", &raw)?)
            .map_err(|_| ConfigError::Invalid("FACTORY_UPLOAD_MAX_BYTES", "too large".into()))?,
    };
    limits.max_upload_bytes = upload_max_bytes as u64;
    for (key, slot) in [
        (
            "FACTORY_INTAKE_MAX_TOTAL_BYTES",
            &mut limits.max_total_bytes,
        ),
        ("FACTORY_INTAKE_MAX_FILE_BYTES", &mut limits.max_file_bytes),
        ("FACTORY_INTAKE_MAX_FILES", &mut limits.max_files),
        ("FACTORY_INTAKE_MAX_ENTRIES", &mut limits.max_entries),
    ] {
        if let Some(raw) = non_empty(key) {
            *slot = positive(key, &raw)?;
        }
    }
    Ok(IntakeSettings {
        upload_max_bytes,
        limits,
    })
}

fn github_app_settings(
    non_empty: &impl Fn(&str) -> Option<String>,
) -> Result<Option<GithubAppSettings>, ConfigError> {
    let id = non_empty("FACTORY_GITHUB_APP_ID");
    let key_file = non_empty("FACTORY_GITHUB_APP_PRIVATE_KEY_FILE");
    let (app_id, key_file) = match (id, key_file) {
        (None, None) => return Ok(None),
        (Some(id), Some(file)) => (id.trim().to_owned(), file),
        (Some(_), None) => {
            return Err(ConfigError::Missing("FACTORY_GITHUB_APP_PRIVATE_KEY_FILE"));
        }
        (None, Some(_)) => return Err(ConfigError::Missing("FACTORY_GITHUB_APP_ID")),
    };
    if app_id.is_empty() || app_id.len() > 20 || !app_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ConfigError::Invalid(
            "FACTORY_GITHUB_APP_ID",
            "expected the numeric App id".into(),
        ));
    }
    let slug = non_empty("FACTORY_GITHUB_APP_SLUG").unwrap_or_else(|| "hoi4-factory-dev".into());
    if slug.len() > 100 || !slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(ConfigError::Invalid(
            "FACTORY_GITHUB_APP_SLUG",
            "expected letters, digits and hyphens".into(),
        ));
    }
    let endpoints = match (
        non_empty("FACTORY_GITHUB_APP_API_BASE"),
        non_empty("FACTORY_GITHUB_CLONE_BASE"),
    ) {
        (None, None) => GithubEndpoints::github_com(),
        (api, clone) => {
            let default = GithubEndpoints::github_com();
            GithubEndpoints::custom(
                &api.unwrap_or(default.api_base),
                &clone.unwrap_or(default.clone_base),
            )
            .map_err(|e| ConfigError::Invalid("FACTORY_GITHUB_APP_API_BASE", e.to_string()))?
        }
    };
    Ok(Some(GithubAppSettings {
        app_id,
        private_key_file: PathBuf::from(key_file),
        slug,
        endpoints,
    }))
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

    fn base<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
        let mut vars = vec![
            ("FACTORY_ENV", "development"),
            ("DATABASE_URL", "postgres://x@y/z"),
        ];
        vars.extend_from_slice(extra);
        vars
    }

    #[test]
    fn upload_limits_default_and_override() {
        let cfg = ServerConfig::from_lookup(lookup(&base(&[]))).unwrap();
        assert_eq!(cfg.intake.upload_max_bytes, 512 * 1024 * 1024);
        assert_eq!(cfg.intake.limits.max_upload_bytes, 512 * 1024 * 1024);
        assert_eq!(
            cfg.intake.limits,
            IntakeLimits {
                max_upload_bytes: 512 * 1024 * 1024,
                ..IntakeLimits::default()
            }
        );
        let cfg = ServerConfig::from_lookup(lookup(&base(&[
            ("FACTORY_UPLOAD_MAX_BYTES", "1000"),
            ("FACTORY_INTAKE_MAX_FILES", "7"),
        ])))
        .unwrap();
        assert_eq!(cfg.intake.upload_max_bytes, 1000);
        assert_eq!(cfg.intake.limits.max_upload_bytes, 1000);
        assert_eq!(cfg.intake.limits.max_files, 7);
        for bad in ["0", "-1", "many"] {
            let r = ServerConfig::from_lookup(lookup(&base(&[("FACTORY_UPLOAD_MAX_BYTES", bad)])));
            assert!(r.is_err(), "{bad}");
        }
    }

    #[test]
    fn github_app_is_off_without_settings_and_needs_both_when_set() {
        let cfg = ServerConfig::from_lookup(lookup(&base(&[]))).unwrap();
        assert!(cfg.github_app.is_none());
        for partial in [
            ("FACTORY_GITHUB_APP_ID", "5453675"),
            ("FACTORY_GITHUB_APP_PRIVATE_KEY_FILE", "/run/secrets/k.pem"),
        ] {
            let r = ServerConfig::from_lookup(lookup(&base(&[partial])));
            assert!(matches!(r, Err(ConfigError::Missing(_))), "{partial:?}");
        }
        let cfg = ServerConfig::from_lookup(lookup(&base(&[
            ("FACTORY_GITHUB_APP_ID", "5453675"),
            ("FACTORY_GITHUB_APP_PRIVATE_KEY_FILE", "/run/secrets/k.pem"),
        ])))
        .unwrap();
        let app = cfg.github_app.unwrap();
        assert_eq!(app.app_id, "5453675");
        assert_eq!(app.slug, "hoi4-factory-dev");
        assert_eq!(app.endpoints, GithubEndpoints::github_com());
        assert_eq!(app.private_key_file, PathBuf::from("/run/secrets/k.pem"));
    }

    #[test]
    fn github_app_values_are_validated() {
        for (key, bad) in [
            ("FACTORY_GITHUB_APP_ID", "abc"),
            ("FACTORY_GITHUB_APP_SLUG", "has space"),
            ("FACTORY_GITHUB_APP_API_BASE", "http://example.org"),
        ] {
            let mut vars = base(&[
                ("FACTORY_GITHUB_APP_ID", "1"),
                ("FACTORY_GITHUB_APP_PRIVATE_KEY_FILE", "k.pem"),
            ]);
            vars.retain(|(k, _)| *k != key);
            vars.push((key, bad));
            assert!(ServerConfig::from_lookup(lookup(&vars)).is_err(), "{key}");
        }
    }
}
