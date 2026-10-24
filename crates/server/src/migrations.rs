//! The one place that lists every crate's migrations.
//!
//! All crates share the `migrations/` directory and the `_sqlx_migrations` table, each owning a
//! version range (0100 accounts, 0200 workspace, 0300 intake, 0400 setup, 0500 game-ref). A
//! crate that adds tables exposes a `migrator()` limited to its range, with `ignore_missing`
//! set (see `factory_accounts::migrator`), and is added to [`components`] in range order.

use sqlx::PgPool;
use sqlx::migrate::Migrator;

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
        // Component { name: "workspace", migrator: factory_workspace::migrator(), probe_table: "..." },
        // Component { name: "intake", ... },
        // Component { name: "setup-adapter", ... },
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
