//! Tenant isolation, grants, startup recovery scan and the down migration.

mod common;

use std::time::Duration;

use common::*;
use factory_tasks::*;
use serde_json::json;

#[tokio::test]
async fn another_account_sees_and_changes_nothing() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let intent = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    mark_started(&mut tx, intent.operation_id).await.unwrap();
    let inc = register_incarnation(&mut tx, "host-a").await.unwrap();
    acquire_lease(
        &mut tx,
        w.a.project,
        w.a.account,
        inc,
        Duration::from_secs(60),
    )
    .await
    .unwrap()
    .unwrap();
    let bundle_id = attempt.bundle_id;
    tx.commit().await.unwrap();

    // Bob, scoped to his own account, asks for Alice's rows by ID.
    let mut b = w.begin(&w.b).await;
    assert!(matches!(
        get_task(&mut b, task.id).await,
        Err(TaskError::NotFound)
    ));
    assert!(list_tasks(&mut b, w.a.project).await.unwrap().is_empty());
    assert!(matches!(
        get_attempt(&mut b, attempt.id).await,
        Err(TaskError::NotFound)
    ));
    assert!(list_attempts(&mut b, task.id).await.unwrap().is_empty());
    assert!(
        events_after(&mut b, task.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        load_bundle(&mut b, bundle_id).await,
        Err(BundleError::NotFound)
    ));
    assert!(matches!(
        get_intent_of(&mut b, intent.operation_id).await,
        Err(TaskError::NotFound)
    ));
    assert!(pending_for_task(&mut b, task.id).await.unwrap().is_empty());
    assert!(
        unresolved_operations(&mut b, task.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(current_lease_of(&mut b, w.a.project).await.is_none());

    // Mutations hit zero rows or are refused, and leave no trace.
    assert!(matches!(
        transition_task(
            &mut b,
            task.id,
            task.version,
            TaskState::Cancelled,
            EventDraft::new(Actor::Host, "x", json!({}))
        )
        .await,
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        set_paused(&mut b, task.id, task.version, true, None, Actor::Host).await,
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        append_event(
            &mut b,
            task.id,
            EventDraft::new(Actor::Host, "x", json!({}))
        )
        .await,
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        transition_attempt(
            &mut b,
            attempt.id,
            attempt.version,
            AttemptState::Failed,
            None,
            None,
            Actor::Host
        )
        .await,
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        save_conversation(&mut b, attempt.id, attempt.version, &json!({})).await,
        Err(TaskError::NotFound)
    ));
    assert!(matches!(
        settle(
            &mut b,
            intent.operation_id,
            ReceiptStatus::Started,
            Outcome {
                status: ReceiptStatus::Succeeded,
                before: None,
                after: None,
                result_artifact: None,
                partial: false
            }
        )
        .await,
        Err(ReceiptError::NotFound)
    ));
    assert!(matches!(
        mark_unknown(&mut b, intent.operation_id, json!({})).await,
        Err(ReceiptError::NotFound)
    ));
    b.rollback().await.unwrap();

    // Creating work in Alice's project is refused, whoever Bob claims to be. A refused insert
    // aborts its transaction, so each attempt gets its own.
    for created_by in [w.b.account, w.a.account] {
        let mut b = w.begin(&w.b).await;
        let mut forged = new_task(&w.a);
        forged.created_by = created_by;
        assert!(matches!(
            create_task(&mut b, forged).await,
            Err(TaskError::NotFound)
        ));
    }
    // Bob cannot store a bundle on Alice's task or take her lease.
    let mut b = w.begin(&w.b).await;
    assert!(matches!(
        store_bundle(
            &mut b,
            w.a.project,
            w.a.account,
            task.id,
            &sample_bundle(&task)
        )
        .await,
        Err(BundleError::TaskNotFound)
    ));
    let mut b = w.begin(&w.b).await;
    let bob_inc = register_incarnation(&mut b, "host-b").await.unwrap();
    b.commit().await.unwrap();
    let mut b = w.begin(&w.b).await;
    assert!(
        acquire_lease(
            &mut b,
            w.a.project,
            w.a.account,
            bob_inc,
            Duration::from_secs(5)
        )
        .await
        .is_err()
    );

    // An intent for Alice's attempt from Bob's scope is refused.
    let mut b = w.begin(&w.b).await;
    assert!(matches!(
        record_intent(
            &mut b,
            new_intent(&task, &attempt, "bob", EffectClass::PureRead)
        )
        .await,
        Err(IntentError::NotFound)
    ));
    b.rollback().await.unwrap();

    // Alice's data is untouched.
    let mut a = w.begin(&w.a).await;
    let back = get_task(&mut a, task.id).await.unwrap();
    assert_eq!(back.state, TaskState::Running);
    assert!(!back.paused);
    assert_eq!(
        w.count("SELECT count(*) FROM agent_events").await,
        back.last_seq
    );

    // Without any account scope nothing is visible at all (fail closed).
    let mut conn = w.db.app.acquire().await.unwrap();
    for table in [
        "agent_tasks",
        "agent_attempts",
        "agent_events",
        "context_bundles",
        "tool_intents",
        "tool_receipts",
        "workspace_leases",
    ] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} without app.account_id");
    }
}

async fn get_intent_of(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    op: OperationId,
) -> Result<Intent, TaskError> {
    factory_tasks::receipts::get_intent(tx, op).await
}

async fn current_lease_of(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project: factory_accounts::ProjectId,
) -> Option<()> {
    factory_tasks::lease::current_lease(tx, project)
        .await
        .unwrap()
        .map(|_| ())
}

#[tokio::test]
async fn a_project_member_works_on_the_projects_tasks() {
    let w = World::new().await;
    sqlx::query(
        "INSERT INTO project_members (project_id, account_id, project_owner_id, role) VALUES ($1, $2, $3, 'contributor')",
    )
    .bind(w.a.project.0)
    .bind(w.b.account.0)
    .bind(w.a.account.0)
    .execute(&w.db.admin)
    .await
    .unwrap();
    let task = w.task(&w.a).await;
    let mut b = w.begin(&w.b).await;
    assert_eq!(get_task(&mut b, task.id).await.unwrap().id, task.id);
    let mut mine = new_task(&w.a);
    mine.created_by = w.b.account;
    let theirs = create_task(&mut b, mine).await.unwrap();
    assert_eq!(theirs.created_by_account_id, w.b.account);
    assert_eq!(theirs.owner_account_id, w.a.account);
}

#[tokio::test]
async fn the_application_role_can_never_delete_or_rewrite_history() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    for table in [
        "agent_tasks",
        "agent_attempts",
        "agent_events",
        "context_bundles",
        "tool_intents",
        "tool_receipts",
        "workspace_leases",
        "host_incarnations",
    ] {
        let mut tx = w.begin(&w.a).await;
        let err = must_fail(
            sqlx::query(&format!("DELETE FROM {table}"))
                .execute(&mut *tx)
                .await,
        )
        .await;
        assert!(err.contains("permission denied"), "{table}: {err}");
    }
    for sql in [
        "UPDATE agent_events SET event_type = 'x'",
        "UPDATE agent_events SET payload = '{}'::jsonb",
        "UPDATE tool_intents SET args_sha256 = repeat('0', 64)",
        "UPDATE agent_tasks SET objective = 'rewritten'",
        "UPDATE agent_tasks SET allowed_paths = ARRAY['/']",
        "UPDATE agent_tasks SET owner_account_id = created_by_account_id",
        "UPDATE agent_attempts SET grant_epoch = 0",
        "UPDATE agent_attempts SET fencing_generation = 99",
        "UPDATE tool_receipts SET operation_id = operation_id",
        "UPDATE workspace_leases SET owner_account_id = owner_account_id",
        "UPDATE host_incarnations SET label = 'x'",
    ] {
        let mut tx = w.begin(&w.a).await;
        let err = must_fail(sqlx::query(sql).execute(&mut *tx).await).await;
        assert!(err.contains("permission denied"), "{sql}: {err}");
    }
    // The permitted columns do work.
    let mut tx = w.begin(&w.a).await;
    sqlx::query("UPDATE host_incarnations SET last_seen_at = now()")
        .execute(&mut *tx)
        .await
        .unwrap();
    // The application role is not a superuser and is subject to row-level security.
    let (sup, bypass): (bool, bool) =
        sqlx::query_as("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert!(!sup && !bypass);
    // Every tenant table has row-level security enabled and forced.
    let weak: Vec<String> = sqlx::query_scalar(
        "SELECT relname::text FROM pg_class WHERE relname IN ('agent_tasks','agent_attempts','agent_events','context_bundles','tool_intents','tool_receipts','workspace_leases') AND NOT (relrowsecurity AND relforcerowsecurity)",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    assert!(weak.is_empty(), "{weak:?}");
}

#[tokio::test]
async fn recovery_scan_lists_nonterminal_attempts_across_accounts_and_nothing_else() {
    let w = World::new().await;
    let (task_a, attempt_a) = w.running_attempt(&w.a).await; // running
    let (task_b, attempt_b) = w.running_attempt(&w.b).await;
    // Bob's attempt is waiting on an uncertain effect: still unfinished work.
    {
        let mut tx = w.begin(&w.b).await;
        transition_attempt(
            &mut tx,
            attempt_b.id,
            attempt_b.version,
            AttemptState::UnknownEffect,
            None,
            None,
            Actor::Host,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    // A finished attempt, and a task that never had one, are not recovery work.
    let (_, finished) = w.running_attempt(&w.a).await;
    {
        let mut tx = w.begin(&w.a).await;
        transition_attempt(
            &mut tx,
            finished.id,
            finished.version,
            AttemptState::Succeeded,
            None,
            None,
            Actor::Host,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    w.ready_task(&w.b).await;

    // The restarting server has no account scope: a plain connection as the application role.
    let mut conn = w.db.app.acquire().await.unwrap();
    let items = recovery_scan(&mut conn).await.unwrap();
    assert_eq!(items.len(), 2, "{items:?}");
    let a = items.iter().find(|i| i.attempt_id == attempt_a.id).unwrap();
    assert_eq!(
        (a.account_id, a.project_id, a.task_id, a.attempt_state),
        (w.a.account, w.a.project, task_a.id, AttemptState::Running)
    );
    let b = items.iter().find(|i| i.attempt_id == attempt_b.id).unwrap();
    assert_eq!(
        (b.account_id, b.project_id, b.task_id, b.attempt_state),
        (
            w.b.account,
            w.b.project,
            task_b.id,
            AttemptState::UnknownEffect
        )
    );

    // The bypass is the function only: the same connection still cannot read the table.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_attempts")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(n, 0);
    // Execute is limited to the application role: PUBLIC lost it.
    let public: bool = sqlx::query_scalar(
        "SELECT has_function_privilege('public', 'tasks_recovery_scan()', 'EXECUTE')",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(!public);
    let app: bool = sqlx::query_scalar(
        "SELECT has_function_privilege('factory_app', 'tasks_recovery_scan()', 'EXECUTE')",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(app);
    let (secdef, config): (bool, Option<Vec<String>>) = sqlx::query_as(
        "SELECT prosecdef, proconfig FROM pg_proc WHERE proname = 'tasks_recovery_scan'",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(secdef);
    assert!(
        config
            .unwrap()
            .iter()
            .any(|c| c.starts_with("search_path="))
    );
}

#[tokio::test]
async fn recovery_after_restart_reads_the_same_state_from_a_fresh_pool() {
    // Process death: nothing in memory survives, the journal decides.
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let op = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap()
    .operation_id;
    mark_started(&mut tx, op).await.unwrap();
    save_conversation(&mut tx, attempt.id, attempt.version, &json!({"turns": 3}))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let fresh = w.db.fresh_app_pool();
    let mut conn = fresh.acquire().await.unwrap();
    let items = recovery_scan(&mut conn).await.unwrap();
    assert_eq!(items.len(), 1);
    drop(conn);
    let db =
        factory_accounts::Database::new(fresh, std::sync::Arc::new(factory_accounts::SystemClock));
    let mut tx = db.scoped(items[0].account_id).begin().await.unwrap();
    let pending = pending_for_task(&mut tx, items[0].task_id).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1.status, ReceiptStatus::Started);
    let reloaded = get_attempt(&mut tx, items[0].attempt_id).await.unwrap();
    assert_eq!(reloaded.conversation["turns"], 3);
    // Recovery never retries: the uncertain effect is recorded as unknown and left for reconciliation.
    mark_unknown(
        &mut tx,
        op,
        json!({"classification": "target_matches_intended_after_hash"}),
    )
    .await
    .unwrap();
    assert!(matches!(
        replay_decision(&pending[0].0, Some(&pending[0].1), &replay_identity()),
        ReplayDecision::MustReconcile
    ));
}

#[tokio::test]
async fn down_migration_runs_cleanly_and_up_again() {
    let w = World::new().await;
    w.task(&w.a).await;
    let migrator = factory_tasks::migrator();
    migrator
        .undo(&w.db.owner, 599)
        .await
        .expect("down migration");
    let left: i64 = w
        .count(
            "SELECT count(*) FROM pg_class WHERE relname IN ('agent_tasks','agent_attempts','agent_events','context_bundles','tool_intents','tool_receipts','workspace_leases','host_incarnations')",
        )
        .await;
    assert_eq!(left, 0);
    let funcs: i64 = w
        .count("SELECT count(*) FROM pg_proc WHERE proname LIKE 'tasks\\_%' OR proname LIKE 'agent\\_attempts%' OR proname LIKE 'tool\\_receipts%'")
        .await;
    assert_eq!(funcs, 0);
    // The accounts schema it depends on is untouched.
    assert!(w.count("SELECT count(*) FROM projects").await >= 2);
    migrator.run(&w.db.owner).await.expect("up again");
    w.task(&w.a).await;
}
