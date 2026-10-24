//! Migration behaviour: idempotent, coexists with other crates' migrations, grants are minimal.

mod common;

use std::sync::Arc;

use common::*;
use factory_accounts::{Database, ManualClock};

#[tokio::test]
async fn migrations_are_idempotent_and_ignore_other_crates_versions() {
    let app = TestApp::new().await;
    let owner_db = Database::new(app.db.owner.clone(), Arc::new(ManualClock::starting_now()));
    owner_db.migrate().await.expect("second run is a no-op");
    assert_eq!(
        app.count("SELECT count(*) FROM _sqlx_migrations WHERE success")
            .await,
        2
    );
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&app.db.admin)
            .await
            .unwrap();
    assert_eq!(versions, vec![100, 101]);

    // Another crate's migration (0200 range) is recorded in the same table. This crate's
    // migrator must not choke on a version it does not know.
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES ($1, $2, true, $3, 1)")
        .bind(200_i64)
        .bind("workspace")
        .bind(vec![0u8; 48])
        .execute(&app.db.admin)
        .await
        .unwrap();
    owner_db
        .migrate()
        .await
        .expect("unknown later versions are ignored");
}

#[tokio::test]
async fn the_application_role_has_only_the_privileges_it_needs() {
    let app = TestApp::new().await;
    let can = |privilege: &'static str, table: &'static str| {
        let pool = app.db.admin.clone();
        async move {
            sqlx::query_scalar::<_, bool>("SELECT has_table_privilege($1, $2, $3)")
                .bind(APP_ROLE)
                .bind(table)
                .bind(privilege)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    for table in [
        "accounts",
        "sessions",
        "email_verification_tokens",
        "projects",
        "project_members",
    ] {
        assert!(can("SELECT", table).await, "{table}");
        assert!(!can("TRUNCATE", table).await, "{table}");
    }
    assert!(!can("DELETE", "accounts").await);
    assert!(!can("DELETE", "projects").await);
    assert!(can("DELETE", "sessions").await);
    // The application role is not the owner, not a superuser and has no BYPASSRLS.
    let (superuser, bypass): (bool, bool) =
        sqlx::query_as("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = $1")
            .bind(APP_ROLE)
            .fetch_one(&app.db.admin)
            .await
            .unwrap();
    assert!(!superuser && !bypass);
    let owners: Vec<String> = sqlx::query_scalar("SELECT DISTINCT tableowner::text FROM pg_tables WHERE tablename IN ('projects', 'project_members')")
        .fetch_all(&app.db.admin)
        .await
        .unwrap();
    assert_eq!(owners, vec![OWNER_ROLE.to_owned()]);
    // Policies exist for both tables and RLS is forced.
    let forced: i64 = app
        .count("SELECT count(*) FROM pg_class WHERE relname IN ('projects', 'project_members') AND relrowsecurity AND relforcerowsecurity")
        .await;
    assert_eq!(forced, 2);
}

#[tokio::test]
async fn migrations_are_reversible() {
    let app = TestApp::new().await;
    let owner_db = Database::new(app.db.owner.clone(), Arc::new(ManualClock::starting_now()));
    let tables = "SELECT count(*) FROM pg_tables WHERE schemaname = 'public' AND tablename IN ('accounts', 'sessions', 'projects', 'project_members', 'password_reset_tokens', 'email_verification_tokens')";
    assert_eq!(app.count(tables).await, 6);

    // Undo only the newest, then everything.
    owner_db.undo_migrations(100).await.expect("undo 0101");
    assert_eq!(app.count(tables).await, 5);
    owner_db.undo_migrations(0).await.expect("undo 0100");
    assert_eq!(app.count(tables).await, 0);
    assert_eq!(
        app.count("SELECT count(*) FROM pg_proc WHERE proname = 'app_current_account'")
            .await,
        0
    );

    // And the up migrations apply again on the emptied database.
    owner_db.migrate().await.expect("re-apply");
    assert_eq!(app.count(tables).await, 6);
}

#[tokio::test]
async fn every_foreign_key_and_lookup_column_is_indexed() {
    let app = TestApp::new().await;
    // For each foreign key, some index of the referencing table must start with its columns.
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT c.conrelid::regclass::text || ' -> ' || c.conname
         FROM pg_constraint c
         WHERE c.contype = 'f' AND c.connamespace = 'public'::regnamespace
           AND NOT EXISTS (
               SELECT 1 FROM pg_index i
               WHERE i.indrelid = c.conrelid
                 AND (i.indkey::int2[])[0:cardinality(c.conkey) - 1] = c.conkey)",
    )
    .fetch_all(&app.db.admin)
    .await
    .unwrap();
    assert!(
        missing.is_empty(),
        "foreign keys without a leading index: {missing:?}"
    );
    // Lookup columns used by the application.
    for (table, column_list) in [
        ("accounts", "lower(email)"),
        ("sessions", "token_hash"),
        ("email_verification_tokens", "token_hash"),
        ("password_reset_tokens", "token_hash"),
    ] {
        let found: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_indexes WHERE tablename = $1 AND indexdef LIKE '%' || $2 || '%'",
        )
        .bind(table)
        .bind(column_list)
        .fetch_one(&app.db.admin)
        .await
        .unwrap();
        assert!(found > 0, "{table}.{column_list}");
    }
}
