//! Database handle, migrations and the role posture check.

use std::sync::Arc;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

use crate::clock::Clock;
use crate::ids::AccountId;
use crate::tenant::TenantRepo;

/// Version range this crate owns in the shared `migrations/` directory.
pub const MIGRATION_VERSIONS: std::ops::Range<i64> = 100..200;

/// The migrations of this crate only (versions 0100 to 0199).
///
/// `ignore_missing` is set because the same `_sqlx_migrations` table also records the other
/// crates' migrations (0200 and up). A server that wants one ordered run over the whole
/// directory can use its own `sqlx::migrate!` instead.
pub fn migrator() -> Migrator {
    let mut m = sqlx::migrate!("../../migrations");
    let own: Vec<_> = m
        .migrations
        .iter()
        .filter(|mig| MIGRATION_VERSIONS.contains(&mig.version))
        .cloned()
        .collect();
    m.migrations = std::borrow::Cow::Owned(own);
    m.ignore_missing = true;
    m
}

/// Connection pool limits.
#[derive(Debug, Clone)]
pub struct PoolSettings {
    pub max_connections: u32,
    pub min_connections: u32,
    /// How long a request waits for a free connection before failing.
    pub acquire_timeout: std::time::Duration,
    pub idle_timeout: std::time::Duration,
    pub max_lifetime: std::time::Duration,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: std::time::Duration::from_secs(5),
            idle_timeout: std::time::Duration::from_secs(600),
            max_lifetime: std::time::Duration::from_secs(1800),
        }
    }
}

impl PoolSettings {
    /// Reads `FACTORY_DB_MAX_CONNECTIONS`, `FACTORY_DB_MIN_CONNECTIONS`,
    /// `FACTORY_DB_ACQUIRE_TIMEOUT_SECS`, `FACTORY_DB_IDLE_TIMEOUT_SECS` and
    /// `FACTORY_DB_MAX_LIFETIME_SECS`; unset ones keep their default.
    pub fn from_env() -> Result<Self, DatabaseSetupError> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, DatabaseSetupError> {
        fn number(
            get: &impl Fn(&str) -> Option<String>,
            key: &'static str,
            default: u64,
        ) -> Result<u64, DatabaseSetupError> {
            match get(key) {
                None => Ok(default),
                Some(v) => v
                    .trim()
                    .parse()
                    .map_err(|_| DatabaseSetupError::Invalid(key)),
            }
        }
        let defaults = Self::default();
        let settings = Self {
            max_connections: number(
                &get,
                "FACTORY_DB_MAX_CONNECTIONS",
                defaults.max_connections.into(),
            )? as u32,
            min_connections: number(
                &get,
                "FACTORY_DB_MIN_CONNECTIONS",
                defaults.min_connections.into(),
            )? as u32,
            acquire_timeout: std::time::Duration::from_secs(number(
                &get,
                "FACTORY_DB_ACQUIRE_TIMEOUT_SECS",
                defaults.acquire_timeout.as_secs(),
            )?),
            idle_timeout: std::time::Duration::from_secs(number(
                &get,
                "FACTORY_DB_IDLE_TIMEOUT_SECS",
                defaults.idle_timeout.as_secs(),
            )?),
            max_lifetime: std::time::Duration::from_secs(number(
                &get,
                "FACTORY_DB_MAX_LIFETIME_SECS",
                defaults.max_lifetime.as_secs(),
            )?),
        };
        if settings.max_connections == 0 || settings.min_connections > settings.max_connections {
            return Err(DatabaseSetupError::Invalid("FACTORY_DB_MAX_CONNECTIONS"));
        }
        if settings.acquire_timeout.is_zero() {
            return Err(DatabaseSetupError::Invalid(
                "FACTORY_DB_ACQUIRE_TIMEOUT_SECS",
            ));
        }
        Ok(settings)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseSetupError {
    #[error("DATABASE_URL is not set")]
    MissingUrl,
    #[error("invalid value for {0}")]
    Invalid(&'static str),
    #[error("could not connect to the database: {0}")]
    Connect(#[from] sqlx::Error),
}

/// Handle on the application database. Cloning shares the pool.
///
/// Rows of `projects` and `project_members` are only reachable through [`Database::scoped`]
/// (or through SQL that PostgreSQL's row-level security then filters). The raw pool is exposed
/// for running migrations and for other crates' own tables.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Database {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Connects with explicit pool settings.
    pub async fn connect(
        url: &str,
        settings: &PoolSettings,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(settings.max_connections)
            .min_connections(settings.min_connections)
            .acquire_timeout(settings.acquire_timeout)
            .idle_timeout(Some(settings.idle_timeout))
            .max_lifetime(Some(settings.max_lifetime))
            .connect(url)
            .await?;
        Ok(Self::new(pool, clock))
    }

    /// `DATABASE_URL` plus the pool settings from the environment (see [`PoolSettings::from_env`]).
    pub async fn from_env(clock: Arc<dyn Clock>) -> Result<Self, DatabaseSetupError> {
        let url = std::env::var("DATABASE_URL").map_err(|_| DatabaseSetupError::MissingUrl)?;
        let settings = PoolSettings::from_env()?;
        Ok(Self::connect(&url, &settings, clock).await?)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Applies this crate's migrations. Run it with the migration (table owner) role, not the
    /// application role.
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        migrator().run(&self.pool).await
    }

    /// Reverts this crate's migrations newer than `target_version` (`0` reverts all of them),
    /// newest first, using the paired `.down.sql` files. For disposable environments and
    /// rollbacks of a release; it destroys the data in the dropped tables.
    pub async fn undo_migrations(
        &self,
        target_version: i64,
    ) -> Result<(), sqlx::migrate::MigrateError> {
        migrator().undo(&self.pool, target_version).await
    }

    /// The only way to reach project data: every call on the returned repository runs inside a
    /// transaction that first sets `app.account_id`.
    pub fn scoped(&self, account: AccountId) -> TenantRepo {
        TenantRepo::new(self.pool.clone(), self.clock.clone(), account)
    }

    /// What the connected role is allowed to do to row-level security.
    pub async fn rls_posture(&self) -> Result<RolePosture, sqlx::Error> {
        let row = sqlx::query(
            "SELECT r.rolname::text AS role, r.rolsuper AS superuser, r.rolbypassrls AS bypass_rls,
                    COALESCE((SELECT bool_and(c.relrowsecurity AND c.relforcerowsecurity)
                              FROM pg_class c
                              WHERE c.oid IN ('projects'::regclass, 'project_members'::regclass)), false) AS rls_forced
             FROM pg_roles r WHERE r.rolname = current_user",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(RolePosture {
            role: row.try_get("role")?,
            superuser: row.try_get("superuser")?,
            bypass_rls: row.try_get("bypass_rls")?,
            rls_forced: row.try_get("rls_forced")?,
        })
    }

    /// Fails when the connected role would bypass row-level security. Call it at startup in
    /// production. In development a superuser is tolerated by the caller, since the Compose
    /// database user is one, but the repository still filters by account in SQL.
    pub async fn ensure_rls_enforced(&self) -> Result<RolePosture, PostureError> {
        let posture = self.rls_posture().await?;
        if posture.superuser {
            return Err(PostureError::Superuser(posture.role));
        }
        if posture.bypass_rls {
            return Err(PostureError::BypassRls(posture.role));
        }
        if !posture.rls_forced {
            return Err(PostureError::NotForced);
        }
        Ok(posture)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePosture {
    pub role: String,
    pub superuser: bool,
    pub bypass_rls: bool,
    /// Row-level security is enabled and forced on both tenant tables.
    pub rls_forced: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PostureError {
    #[error(
        "database role `{0}` is a superuser and bypasses row-level security; connect as a member of factory_app instead"
    )]
    Superuser(String),
    #[error("database role `{0}` has BYPASSRLS; connect as a member of factory_app instead")]
    BypassRls(String),
    #[error(
        "row-level security is not enabled and forced on projects and project_members; run the migrations"
    )]
    NotForced,
    #[error("could not inspect the database role: {0}")]
    Query(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_settings_come_from_the_environment() {
        let d = PoolSettings::default();
        assert_eq!((d.max_connections, d.acquire_timeout.as_secs()), (10, 5));
        let got = PoolSettings::from_lookup(|k| match k {
            "FACTORY_DB_MAX_CONNECTIONS" => Some("25".into()),
            "FACTORY_DB_ACQUIRE_TIMEOUT_SECS" => Some("2".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            (
                got.max_connections,
                got.acquire_timeout.as_secs(),
                got.min_connections
            ),
            (25, 2, 0)
        );
        assert!(
            PoolSettings::from_lookup(|k| (k == "FACTORY_DB_MAX_CONNECTIONS").then(|| "0".into()))
                .is_err()
        );
        assert!(PoolSettings::from_lookup(|k| (k == "FACTORY_DB_MAX_CONNECTIONS").then(|| "ten".into())).is_err());
        assert!(
            PoolSettings::from_lookup(
                |k| (k == "FACTORY_DB_ACQUIRE_TIMEOUT_SECS").then(|| "0".into())
            )
            .is_err()
        );
    }

    #[test]
    fn migrator_contains_only_this_crates_versions() {
        let m = migrator();
        assert!(!m.migrations.is_empty());
        assert!(
            m.migrations
                .iter()
                .all(|mig| MIGRATION_VERSIONS.contains(&mig.version))
        );
        assert!(m.migrations.iter().any(|mig| mig.version == 100));
        assert!(m.ignore_missing);
    }
}
