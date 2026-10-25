//! Migrations 0400 to 0402: reversible, indexed, minimal privileges, forced row-level security.
//! Also the Setup pin: one place, and it agrees with Cargo.lock.

mod common;

use common::*;
use factory_setup_adapter::*;

const TABLES: [&str; 7] = [
    "setup_sessions",
    "setup_scans",
    "setup_analyses",
    "setup_plans",
    "setup_approvals",
    "setup_transactions",
    "setup_receipts",
];

#[tokio::test]
async fn migrations_are_idempotent_and_reversible() {
    let env = Env::new().await;
    let count_tables = format!(
        "SELECT count(*) FROM pg_tables WHERE schemaname = 'public' AND tablename IN ({})",
        TABLES
            .iter()
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(env.count(&count_tables).await, 7);
    // Running again is a no-op and the other crate's versions are tolerated.
    migrator().run(&env.db.owner).await.expect("second run");
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&env.db.admin)
            .await
            .unwrap();
    assert_eq!(versions, vec![100, 101, 400, 401, 402]);

    // Undo the newest, then everything of this crate; the accounts tables stay.
    let has_state = "SELECT count(*) FROM information_schema.columns                      WHERE table_name = 'setup_sessions' AND column_name = 'client_state'";
    assert_eq!(env.count(has_state).await, 1);
    migrator()
        .undo(&env.db.owner, 401)
        .await
        .expect("undo 0402");
    assert_eq!(env.count(has_state).await, 0);
    assert_eq!(env.count(&count_tables).await, 7);
    migrator()
        .undo(&env.db.owner, 400)
        .await
        .expect("undo 0401");
    assert_eq!(env.count(&count_tables).await, 5);
    assert_eq!(
        env.count("SELECT count(*) FROM pg_class WHERE relname = 'setup_fence_seq'")
            .await,
        0
    );
    migrator().undo(&env.db.owner, 0).await.expect("undo 0400");
    assert_eq!(env.count(&count_tables).await, 0);
    assert_eq!(
        env.count("SELECT count(*) FROM pg_proc WHERE proname = 'setup_can_see_project'")
            .await,
        0
    );
    assert_eq!(
        env.count("SELECT count(*) FROM pg_tables WHERE tablename = 'projects'")
            .await,
        1
    );
    migrator().run(&env.db.owner).await.expect("re-apply");
    assert_eq!(env.count(&count_tables).await, 7);
}

#[tokio::test]
async fn row_level_security_is_forced_and_privileges_are_minimal() {
    let env = Env::new().await;
    for table in TABLES {
        let forced = env
            .count(&format!(
                "SELECT count(*) FROM pg_class WHERE relname = '{table}' AND relrowsecurity AND relforcerowsecurity"
            ))
            .await;
        assert_eq!(forced, 1, "{table}: RLS not enabled and forced");
        let policies = env
            .count(&format!(
                "SELECT count(*) FROM pg_policies WHERE tablename = '{table}'"
            ))
            .await;
        assert!(policies >= 3, "{table}: {policies} policies");
        // No DELETE policy, so the application role cannot remove setup records.
        assert_eq!(
            env.count(&format!(
                "SELECT count(*) FROM pg_policies WHERE tablename = '{table}' AND cmd = 'DELETE'"
            ))
            .await,
            0
        );
        for (privilege, expected) in [
            ("SELECT", true),
            ("INSERT", true),
            ("DELETE", false),
            ("TRUNCATE", false),
        ] {
            let has: bool = sqlx::query_scalar("SELECT has_table_privilege($1, $2, $3)")
                .bind(APP_ROLE)
                .bind(table)
                .bind(privilege)
                .fetch_one(&env.db.admin)
                .await
                .unwrap();
            assert_eq!(has, expected, "{table} {privilege}");
        }
    }
    // The intent columns of a receipt are not updatable by the application role.
    for (column, expected) in [
        ("request", false),
        ("request_sha256", false),
        ("idempotency_key", false),
        ("operation", false),
        ("status", true),
        ("result", true),
    ] {
        let has: bool =
            sqlx::query_scalar("SELECT has_column_privilege($1, 'setup_receipts', $2, 'UPDATE')")
                .bind(APP_ROLE)
                .bind(column)
                .fetch_one(&env.db.admin)
                .await
                .unwrap();
        assert_eq!(has, expected, "setup_receipts.{column}");
    }
    // The plan hash and root identity bound at approval time are not rewritable on approvals.
    for column in [
        "reviewed_plan_sha256",
        "approved_plan_sha256",
        "root_identity",
    ] {
        let has: bool =
            sqlx::query_scalar("SELECT has_column_privilege($1, 'setup_approvals', $2, 'UPDATE')")
                .bind(APP_ROLE)
                .bind(column)
                .fetch_one(&env.db.admin)
                .await
                .unwrap();
        assert!(!has, "setup_approvals.{column}");
    }
}

#[tokio::test]
async fn every_foreign_key_has_an_index_led_by_its_first_column() {
    let env = Env::new().await;
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT c.conrelid::regclass::text || ' -> ' || c.conname
         FROM pg_constraint c
         WHERE c.contype = 'f' AND c.connamespace = 'public'::regnamespace
           AND c.conrelid::regclass::text LIKE 'setup\\_%'
           AND NOT EXISTS (
               SELECT 1 FROM pg_index i
               WHERE i.indrelid = c.conrelid
                 AND (i.indkey::int2[])[0] = c.conkey[1])",
    )
    .fetch_all(&env.db.admin)
    .await
    .unwrap();
    assert!(
        missing.is_empty(),
        "foreign keys without a leading index: {missing:?}"
    );
}

#[test]
fn the_pin_agrees_with_cargo_lock_and_is_a_single_place() {
    let lock =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    let marker = format!(
        "git+{}?rev={}#{}",
        SETUP_PIN.git, SETUP_PIN.revision, SETUP_PIN.revision
    );
    assert!(
        lock.contains(&marker),
        "Cargo.lock does not resolve hoi4-mod-setup to {}",
        SETUP_PIN.revision
    );
    // Default features stay off: no keyring and no Tauri in the dependency set.
    for banned in [
        "name = \"keyring\"",
        "name = \"tauri\"",
        "name = \"dbus\"",
        "name = \"libdbus-sys\"",
    ] {
        assert!(!lock.contains(banned), "{banned} is in Cargo.lock");
    }
    // The flags are the maintainer's statement in Cargo.toml and are never hard-coded here, so
    // bumping the pin to a merged revision is a change of that one place. The derived rule must
    // follow the flag, and this build uses Setup's DefaultPlanBuilder, so the flag must be true.
    let pin = std::hint::black_box(SETUP_PIN);
    assert_eq!(pin.counts_for_recovery_gates(), pin.contains_pr_105_fix);
    assert!(
        pin.default_plan_builder,
        "the server plans with DefaultPlanBuilder"
    );
    assert_eq!(pin.pr_105_head, "a300b90439fcf907b9e684f3df8443322d7b7848");
    assert_eq!(pin.repository, "klimPaskov/HOI4-Mod-Setup");
}

#[test]
fn setup_dependencies_share_one_revision() {
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml")).unwrap();
    let revs: Vec<&str> = manifest
        .lines()
        .filter(|l| l.starts_with("hoi4-mod-setup =") || l.starts_with("hoi4-setup-core ="))
        .filter_map(|l| l.split("rev = \"").nth(1))
        .filter_map(|r| r.split('"').next())
        .collect();
    assert_eq!(revs.len(), 2, "{revs:?}");
    assert!(
        revs.iter().all(|r| *r == SETUP_PIN.revision),
        "{revs:?} vs {}",
        SETUP_PIN.revision
    );
}
