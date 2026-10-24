//! The one place that lists every crate's migrations.
//!
//! All crates share the `migrations/` directory and the `_sqlx_migrations` table, each owning a
//! version range (0100 accounts, 0200 workspace, 0300 intake, 0400 setup, 0500 game-ref). A
//! crate that adds tables exposes a `migrator()` limited to its range, with `ignore_missing`
//! set (see `factory_accounts::migrator`), and is added to [`components`] in range order.

use sqlx::PgPool;
use sqlx::migrate::Migrator;

/// Versions 0300 to 0399: the intake tables (0300 to 0302) and the foreign keys that join them to
/// accounts and projects (0303, owned by the server because it is the one place that sees both).
const INTAKE_VERSIONS: std::ops::RangeInclusive<i64> = 300..=399;

/// A migrator limited to `range`, with `ignore_missing` set because the shared
/// `_sqlx_migrations` table also records the other components' versions (see
/// `factory_accounts::migrator`).
fn range_migrator(range: std::ops::RangeInclusive<i64>) -> Migrator {
    let mut migrator = sqlx::migrate!("../../migrations");
    let own: Vec<_> = migrator
        .migrations
        .iter()
        .filter(|m| range.contains(&m.version))
        .cloned()
        .collect();
    migrator.migrations = std::borrow::Cow::Owned(own);
    migrator.ignore_missing = true;
    migrator
}

/// One crate's migrations.
pub struct Component {
    pub name: &'static str,
    pub migrator: Migrator,
    /// A table the migrations create. Readiness checks that it exists.
    pub probe_table: &'static str,
}

/// Every component, in ascending version-range order. Add later crates here.
pub fn components() -> Vec<Component> {
    vec![
        Component {
            name: "accounts",
            migrator: factory_accounts::migrator(),
            probe_table: "accounts",
        },
        // The workspace crate has no tables.
        Component {
            name: "intake",
            migrator: range_migrator(INTAKE_VERSIONS),
            probe_table: "intake_uploads",
        },
        Component {
            name: "setup-adapter",
            migrator: factory_setup_adapter::migrator(),
            probe_table: "setup_sessions",
        },
    ]
}

/// What the startup run applied, for the readiness report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MigrationReport {
    pub components: Vec<ComponentReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ComponentReport {
    pub name: &'static str,
    pub latest_version: i64,
    #[serde(skip)]
    pub probe_table: &'static str,
}

impl MigrationReport {
    pub fn latest_version(&self) -> i64 {
        self.components
            .iter()
            .map(|c| c.latest_version)
            .max()
            .unwrap_or(0)
    }
}

/// Applies every component's migrations in order with the table-owner pool.
pub async fn run_all(pool: &PgPool) -> Result<MigrationReport, sqlx::migrate::MigrateError> {
    let mut report = MigrationReport {
        components: Vec::new(),
    };
    for component in components() {
        component.migrator.run(pool).await?;
        tracing::info!(component = component.name, "migrations applied");
        report.components.push(ComponentReport {
            name: component.name,
            latest_version: component
                .migrator
                .iter()
                .filter(|m| m.migration_type.is_up_migration())
                .map(|m| m.version)
                .max()
                .unwrap_or(0),
            probe_table: component.probe_table,
        });
    }
    Ok(report)
}

/// Live check that the tables the migrations created are reachable by the connected role.
pub async fn verify_tables(pool: &PgPool, report: &MigrationReport) -> Result<(), String> {
    for component in &report.components {
        let exists: Option<bool> = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(format!("public.{}", component.probe_table))
            .fetch_one(pool)
            .await
            .map_err(|_| "database query failed".to_owned())?;
        if exists != Some(true) {
            return Err(format!("{} schema is missing", component.name));
        }
    }
    Ok(())
}
