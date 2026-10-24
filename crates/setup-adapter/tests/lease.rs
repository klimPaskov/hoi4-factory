//! One owner per project: the lease refuses a second owner at once, across adapter instances,
//! and a dead or cancelled owner does not hold the project forever.

mod common;

use std::time::Duration;

use common::*;
use factory_setup_adapter::*;
use uuid::Uuid;

#[tokio::test]
async fn a_second_owner_is_refused_while_one_drives_the_project() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let first = env.adapter();
    let second = env.adapter(); // another instance on the same database, like another process
    let session = first.open_session(alice, fx.project, None).await.unwrap();
    let session_b = second.open_session(alice, fx.project, None).await.unwrap();

    let lease = ProjectLease::try_acquire(&env.database, alice, fx.project)
        .await
        .expect("first owner");
    let generation = lease.generation();

    // Neither instance can apply or recover while the lease is held, and both say why.
    for (adapter, s) in [(&first, &session), (&second, &session_b)] {
        let err = adapter
            .apply_setup_plan(
                alice,
                fx.project,
                s.session_id,
                Uuid::new_v4(),
                OpContext::default(),
            )
            .await
            .expect_err("busy");
        assert_eq!(err.code(), "project_busy", "{err}");
        assert!(matches!(err, SetupError::ProjectBusy { .. }));
        let err = adapter
            .recover_setup(
                alice,
                fx.project,
                Uuid::new_v4(),
                RecoveryVariant::RollbackRequiredPartialApply,
                OpContext::default(),
            )
            .await
            .expect_err("busy");
        assert_eq!(err.code(), "project_busy", "{err}");
    }
    // The refusals are receipts, and nothing touched the project.
    assert!(
        env.count("SELECT count(*) FROM setup_receipts WHERE status = 'refused' AND error_code = 'project_busy'").await >= 4
    );
    assert_eq!(
        env.count("SELECT count(*) FROM setup_transactions").await,
        0
    );
    assert!(!fx.state.join("transactions").exists());

    // Reads stay available, and a view taken meanwhile offers no action.
    assert!(
        first
            .inspect_recovery(alice, fx.project)
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        ProjectLease::try_acquire(&env.database, alice, fx.project).await,
        Err(SetupError::ProjectBusy { .. })
    ));

    lease.release().await.unwrap();
    let again = ProjectLease::try_acquire(&env.database, alice, fx.project)
        .await
        .expect("free after release");
    assert!(
        again.generation() > generation,
        "fencing generations only grow"
    );
    again.release().await.unwrap();
}

#[tokio::test]
async fn simultaneous_attempts_have_exactly_one_winner() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let attempts = futures_join(&env, alice, fx.project, 6).await;
    let winners: Vec<_> = attempts.iter().filter(|r| r.is_ok()).collect();
    assert_eq!(winners.len(), 1, "{} winners", winners.len());
    for r in attempts.iter().filter(|r| r.is_err()) {
        assert!(matches!(r, Err(SetupError::ProjectBusy { .. })));
    }
}

async fn futures_join(
    env: &Env,
    account: factory_accounts::AccountId,
    project: factory_accounts::ProjectId,
    n: usize,
) -> Vec<Result<ProjectLease, SetupError>> {
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(n));
    let mut tasks = Vec::new();
    for _ in 0..n {
        let (db, b) = (env.database.clone(), barrier.clone());
        tasks.push(tokio::spawn(async move {
            b.wait().await;
            ProjectLease::try_acquire(&db, account, project).await
        }));
    }
    // Collect after all finished, so the winner still holds its lease while the others try.
    let mut out = Vec::new();
    for t in tasks {
        out.push(t.await.unwrap());
    }
    out
}

#[tokio::test]
async fn a_dropped_or_cancelled_owner_frees_the_project() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    {
        let _lease = ProjectLease::try_acquire(&env.database, alice, fx.project)
            .await
            .unwrap();
        // dropped here without release: a cancelled request or a panic
    }
    let mut freed = false;
    for _ in 0..40 {
        match ProjectLease::try_acquire(&env.database, alice, fx.project).await {
            Ok(l) => {
                l.release().await.unwrap();
                freed = true;
                break;
            }
            Err(SetupError::ProjectBusy { .. }) => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert!(
        freed,
        "the lease was still held after the guard was dropped"
    );

    // Leases are per project.
    let other = env.project_with_import(alice).await;
    let a = ProjectLease::try_acquire(&env.database, alice, fx.project)
        .await
        .unwrap();
    let b = ProjectLease::try_acquire(&env.database, alice, other.project)
        .await
        .expect("a different project is independent");
    a.release().await.unwrap();
    b.release().await.unwrap();
}

#[tokio::test]
async fn the_database_allows_one_unfinished_transaction_per_project() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let store = Store::new(env.database.clone());
    let row = |state: &'static str| TransactionInsert {
        id: Uuid::new_v4(),
        journal_id: Uuid::new_v4(),
        plan_id: None,
        session_id: None,
        mod_root: String::new(),
        state,
        plan_sha256: None,
        root_identity: None,
        pre_inventory_sha256: None,
        fence_generation: None,
        holder: "test".into(),
        started_at: chrono::Utc::now(),
    };
    let scope = Scope {
        actor: alice,
        owner: alice,
        project: fx.project,
    };
    store
        .insert_transaction(&scope, &row("applying"))
        .await
        .unwrap();
    let err = store
        .insert_transaction(&scope, &row("recovering"))
        .await
        .unwrap_err();
    assert!(matches!(err, SetupError::ProjectBusy { .. }), "{err}");
    // A finished transaction does not count, and another project is independent.
    let other = env.project_with_import(alice).await;
    let scope2 = Scope {
        actor: alice,
        owner: alice,
        project: other.project,
    };
    store
        .insert_transaction(&scope2, &row("completed"))
        .await
        .unwrap();
    store
        .insert_transaction(&scope2, &row("applying"))
        .await
        .unwrap();
}
