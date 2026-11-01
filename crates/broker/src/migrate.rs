//! This crate's migrations in the shared `migrations/` directory (versions 0700 to 0799).

use sqlx::migrate::Migrator;

/// Version range this crate owns.
pub const MIGRATION_VERSIONS: std::ops::Range<i64> = 700..800;

/// The migrations of this crate only. They need the accounts migrations (0100 range) applied
/// first: they reference `accounts`, `projects`, `project_members` and `app_current_account()`.
///
/// `ignore_missing` is set because `_sqlx_migrations` also records the other crates' versions.
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
        for v in [700, 701, 702] {
            assert!(versions.contains(&v), "{versions:?}");
        }
        assert!(
            m.migrations
                .iter()
                .all(|m| MIGRATION_VERSIONS.contains(&m.version))
        );
    }
}
