//! This crate's migrations in the shared `migrations/` directory (versions 0600 to 0699).

use sqlx::migrate::Migrator;

/// Version range this crate owns.
pub const MIGRATION_VERSIONS: std::ops::Range<i64> = 600..700;

/// The migrations of this crate only. They need the accounts migrations (0100 range) applied
/// first: they reference `projects`, `project_members`, `accounts`, `factory_app` and
/// `app_current_account()`.
///
/// `ignore_missing` is set because `_sqlx_migrations` also records the other crates' versions.
/// Run it with the migration (table owner) role, not the application role: the recovery function
/// is owned by whoever runs it.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_crates_versions() {
        let m = migrator();
        let versions: Vec<i64> = m.migrations.iter().map(|m| m.version).collect();
        assert!(versions.contains(&600), "{versions:?}");
        assert!(
            m.migrations
                .iter()
                .all(|m| MIGRATION_VERSIONS.contains(&m.version))
        );
    }
}
