//! Tasks, events, attempts, pause, leases and fencing against real PostgreSQL with row-level
//! security on (the application talks as a non-superuser role).

mod common;

use std::time::Duration;

use common::*;
use factory_tasks::*;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn create_task_writes_event_one_with_a_verifiable_payload() {
    let w = World::new().await;
    let task = w.task(&w.a).await;
    assert_eq!(task.state, TaskState::Planned);
    assert_eq!((task.version, task.last_seq), (1, 1));
    assert!(!task.paused);
    let events = w.ledger(&w.a).events_after(task.id, 0, 100).await.unwrap();
    assert_eq!(events.len(), 1);
    let e = &events[0];
    assert_eq!((e.seq, e.event_type.as_str()), (1, "task_created"));
    assert_eq!(e.actor, Actor::user(w.a.account));
    assert!(e.payload_intact());
    assert_eq!(e.payload["objective"], "Add one national focus");
}

#[tokio::test]
async fn invalid_new_tasks_are_refused_before_the_database() {
    let w = World::new().await;
    let mut tx = w.begin(&w.a).await;
    let mut bad = new_task(&w.a);
    bad.objective = String::new();
    assert!(matches!(
        create_task(&mut tx, bad).await,
        Err(TaskError::Invalid(_))
    ));
    let mut bad = new_task(&w.a);
    bad.allowed_paths = vec!["../escape".into()];
    assert!(matches!(
        create_task(&mut tx, bad).await,
        Err(TaskError::Invalid(_))
    ));
    // The database refuses a contradictory scope too, whatever the Rust side says.
    let sql = "INSERT INTO agent_tasks (id, project_id, owner_account_id, created_by_account_id, objective, mode, allowed_paths, budget_micros, max_turns)
               VALUES (gen_random_uuid(), $1, $2, $2, 'x', 'read_only', ARRAY['a'], 1, 1)";
    let err = must_fail(
        sqlx::query(sql)
            .bind(w.a.project.0)
            .bind(w.a.account.0)
            .execute(&w.db.admin)
            .await,
    )
    .await;
    assert!(err.contains("agent_tasks_mode_paths"), "{err}");
}

#[tokio::test]
async fn concurrent_appends_are_gapless_and_unique() {
    let w = World::new().await;
    let task = w.task(&w.a).await;
    let repo = w.repo(&w.a);
    let mut handles = Vec::new();
    for worker in 0..8i64 {
        let repo = repo.clone();
        handles.push(tokio::spawn(async move {
            for n in 0..6 {
                let mut tx = repo.begin().await.unwrap();
                let draft = EventDraft::new(
                    Actor::Worker(worker),
                    "progress",
                    json!({"worker": worker, "n": n}),
                );
                append_event(&mut tx, task.id, draft).await.unwrap();
                tx.commit().await.unwrap();
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let mut tx = w.begin(&w.a).await;
    let events = events_after(&mut tx, task.id, 0, 1000).await.unwrap();
    assert_eq!(events.len(), 1 + 8 * 6);
    let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(
        seqs,
        (1..=49).collect::<Vec<_>>(),
        "gapless, ordered, unique"
    );
    check_contiguous(0, &events).unwrap();
    let ids: std::collections::HashSet<_> = events.iter().map(|e| e.event_id).collect();
    assert_eq!(ids.len(), events.len());
    assert!(events.iter().all(RunEvent::payload_intact));
    assert_eq!(get_task(&mut tx, task.id).await.unwrap().last_seq, 49);
}

#[tokio::test]
async fn a_rolled_back_change_leaves_neither_state_nor_event() {
    // ORCH-08: the state change and its event commit together or not at all.
    let w = World::new().await;
    let task = w.task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    transition_task(
        &mut tx,
        task.id,
        task.version,
        TaskState::Ready,
        EventDraft::new(Actor::Host, "task_ready", json!({})),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let mut tx = w.begin(&w.a).await;
    let after = get_task(&mut tx, task.id).await.unwrap();
    assert_eq!(
        (after.state, after.version, after.last_seq),
        (TaskState::Planned, 1, 1)
    );
    assert_eq!(
        events_after(&mut tx, task.id, 0, 10).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn events_after_pages_by_cursor_and_detects_a_gap() {
    let w = World::new().await;
    let task = w.task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    for n in 0..5 {
        append_event(
            &mut tx,
            task.id,
            EventDraft::new(Actor::Host, "tick", json!({"n": n})),
        )
        .await
        .unwrap();
    }
    let first = events_after(&mut tx, task.id, 0, 3).await.unwrap();
    assert_eq!(first.iter().map(|e| e.seq).collect::<Vec<_>>(), [1, 2, 3]);
    let rest = events_after(&mut tx, task.id, 3, 100).await.unwrap();
    assert_eq!(rest.iter().map(|e| e.seq).collect::<Vec<_>>(), [4, 5, 6]);
    check_contiguous(3, &rest).unwrap();
    // A consumer that lost event 5 sees the gap.
    let mut holey = rest.clone();
    holey.remove(1);
    assert_eq!(
        check_contiguous(3, &holey),
        Err(Gap {
            expected: 5,
            found: 6
        })
    );
    // Reading past the end is empty, not an error.
    assert!(
        events_after(&mut tx, task.id, 6, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn competing_transitions_succeed_exactly_once() {
    let w = World::new().await;
    let task = w.task(&w.a).await;
    let repo = w.repo(&w.a);

    let mut first = repo.begin().await.unwrap();
    transition_task(
        &mut first,
        task.id,
        task.version,
        TaskState::Ready,
        EventDraft::new(Actor::Host, "task_ready", json!({})),
    )
    .await
    .unwrap();

    // The rival reads the same version, waits on the row lock, then loses.
    let rival_repo = repo.clone();
    let rival = tokio::spawn(async move {
        let mut tx = rival_repo.begin().await.unwrap();
        let r = transition_task(
            &mut tx,
            task.id,
            task.version,
            TaskState::Cancelled,
            EventDraft::new(Actor::Host, "task_cancelled", json!({})),
        )
        .await;
        tx.commit().await.unwrap();
        r
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!rival.is_finished(), "the rival waits for the first commit");
    first.commit().await.unwrap();
    match rival.await.unwrap() {
        Err(TaskError::VersionConflict { current }) => assert_eq!(current, 2),
        other => panic!("expected a version conflict, got {other:?}"),
    }
    let mut tx = w.begin(&w.a).await;
    let now = get_task(&mut tx, task.id).await.unwrap();
    assert_eq!((now.state, now.version), (TaskState::Ready, 2));
    // Exactly one transition event.
    let events = events_after(&mut tx, task.id, 0, 10).await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event_type == "task_ready")
            .count(),
        1
    );
    assert!(events.iter().all(|e| e.event_type != "task_cancelled"));
}

#[tokio::test]
async fn invalid_transitions_are_refused_and_change_nothing() {
    let w = World::new().await;
    let task = w.task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    for to in [
        TaskState::Running,
        TaskState::Completed,
        TaskState::InReview,
    ] {
        let r = transition_task(
            &mut tx,
            task.id,
            task.version,
            to,
            EventDraft::new(Actor::Host, "x", json!({})),
        )
        .await;
        assert!(
            matches!(r, Err(TaskError::InvalidTransition { from: TaskState::Planned, to: t }) if t == to),
            "{to}: {r:?}"
        );
    }
    let cancelled = transition_task(
        &mut tx,
        task.id,
        task.version,
        TaskState::Cancelled,
        EventDraft::new(Actor::Host, "", json!({"why": "user"})),
    )
    .await
    .unwrap();
    // The default event type is task_<state>, and the payload records from and to.
    let events = events_after(&mut tx, task.id, 1, 10).await.unwrap();
    assert_eq!(events[0].event_type, "task_cancelled");
    assert_eq!(events[0].payload["from"], "planned");
    assert_eq!(events[0].payload["to"], "cancelled");
    assert_eq!(events[0].payload["detail"]["why"], "user");
    let r = transition_task(
        &mut tx,
        task.id,
        cancelled.version,
        TaskState::Ready,
        EventDraft::new(Actor::Host, "x", json!({})),
    )
    .await;
    assert!(
        matches!(r, Err(TaskError::InvalidTransition { .. })),
        "terminal: {r:?}"
    );
}

#[tokio::test]
async fn pause_is_durable_and_blocks_admission_until_resumed() {
    let w = World::new().await;
    let ready = w.ready_task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let paused = set_paused(
        &mut tx,
        ready.task.id,
        ready.task.version,
        true,
        Some("checking the budget"),
        Actor::user(w.a.account),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(paused.paused);
    assert_eq!(
        paused.state,
        TaskState::Ready,
        "pause is not a state change"
    );
    assert_eq!(paused.version, ready.task.version + 1);

    // A fresh pool, as after a restart, still sees it.
    let fresh = w.db.fresh_app_pool();
    let fresh_db =
        factory_accounts::Database::new(fresh, std::sync::Arc::new(factory_accounts::SystemClock));
    let mut tx = fresh_db.scoped(w.a.account).begin().await.unwrap();
    let reloaded = get_task(&mut tx, ready.task.id).await.unwrap();
    assert!(reloaded.paused);
    assert_eq!(
        reloaded.pause_reason.as_deref(),
        Some("checking the budget")
    );

    let r = admit_attempt(&mut tx, ready.task.id, reloaded.version, ready.admission()).await;
    assert!(matches!(r, Err(TaskError::Paused)), "{r:?}");
    let r = transition_task(
        &mut tx,
        ready.task.id,
        reloaded.version,
        TaskState::Running,
        EventDraft::new(Actor::Host, "x", json!({})),
    )
    .await;
    assert!(matches!(r, Err(TaskError::Paused)), "{r:?}");
    // Pausing twice is refused; a stale version loses.
    assert!(matches!(
        set_paused(
            &mut tx,
            ready.task.id,
            reloaded.version,
            true,
            None,
            Actor::Host
        )
        .await,
        Err(TaskError::Invalid(_))
    ));
    assert!(matches!(
        set_paused(&mut tx, ready.task.id, 1, false, None, Actor::Host).await,
        Err(TaskError::VersionConflict { .. })
    ));
    let resumed = set_paused(
        &mut tx,
        ready.task.id,
        reloaded.version,
        false,
        None,
        Actor::Host,
    )
    .await
    .unwrap();
    assert!(!resumed.paused && resumed.pause_reason.is_none());
    let attempt = admit_attempt(&mut tx, ready.task.id, resumed.version, ready.admission())
        .await
        .unwrap();
    assert_eq!(attempt.attempt_no, 1);
    let types: Vec<String> = events_after(&mut tx, ready.task.id, 0, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    assert!(types.contains(&"task_paused".to_owned()));
    assert!(types.contains(&"task_resumed".to_owned()));
    assert_eq!(types.last().unwrap(), "attempt_admitted");
}

#[tokio::test]
async fn admission_allows_one_nonterminal_attempt_per_task() {
    let w = World::new().await;
    let ready = w.ready_task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let first = admit_attempt(
        &mut tx,
        ready.task.id,
        ready.task.version,
        ready.admission(),
    )
    .await
    .unwrap();
    assert_eq!(first.state, AttemptState::Admitted);
    let task = get_task(&mut tx, ready.task.id).await.unwrap();
    assert_eq!(task.state, TaskState::Running);
    assert_eq!(task.version, ready.task.version + 1);

    let r = admit_attempt(&mut tx, ready.task.id, task.version, ready.admission()).await;
    match r {
        Err(TaskError::AttemptActive { attempt }) => assert_eq!(attempt, first.id),
        other => panic!("{other:?}"),
    }
    // A stale expected version is a conflict, not a second attempt.
    let r = admit_attempt(
        &mut tx,
        ready.task.id,
        ready.task.version,
        ready.admission(),
    )
    .await;
    assert!(matches!(r, Err(TaskError::VersionConflict { current }) if current == task.version));
    tx.commit().await.unwrap();

    // The database enforces it too, whatever the Rust code checks.
    let err = must_fail(
        sqlx::query(
            "INSERT INTO agent_attempts (id, task_id, project_id, owner_account_id, attempt_no, state,
                 bundle_id, grant_id, grant_epoch, host_incarnation, fencing_generation, model_profile, funding)
             SELECT gen_random_uuid(), task_id, project_id, owner_account_id, 2, 'running', bundle_id,
                    grant_id, grant_epoch, host_incarnation, fencing_generation, model_profile, funding
             FROM agent_attempts WHERE id = $1",
        )
        .bind(first.id)
        .execute(&w.db.admin)
        .await,
    )
    .await;
    assert!(err.contains("agent_attempts_one_active"), "{err}");

    // After the first attempt ends, a second is numbered 2.
    let mut tx = w.begin(&w.a).await;
    let started = transition_attempt(
        &mut tx,
        first.id,
        first.version,
        AttemptState::Running,
        None,
        None,
        Actor::Host,
    )
    .await
    .unwrap();
    transition_attempt(
        &mut tx,
        first.id,
        started.version,
        AttemptState::Failed,
        None,
        Some("provider_unavailable"),
        Actor::Host,
    )
    .await
    .unwrap();
    let task = get_task(&mut tx, ready.task.id).await.unwrap();
    let second = admit_attempt(&mut tx, task.id, task.version, ready.admission())
        .await
        .unwrap();
    assert_eq!(second.attempt_no, 2);
}

#[tokio::test]
async fn admission_rejects_a_foreign_bundle_and_bad_funding() {
    let w = World::new().await;
    let one = w.ready_task(&w.a).await;
    let two = w.ready_task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let mut admission = one.admission();
    admission.bundle_id = two.bundle.id;
    let r = admit_attempt(&mut tx, one.task.id, one.task.version, admission).await;
    assert!(matches!(r, Err(TaskError::BundleMismatch)), "{r:?}");
    let mut admission = one.admission();
    admission.funding = Funding::UserKey;
    let r = admit_attempt(&mut tx, one.task.id, one.task.version, admission).await;
    assert!(matches!(r, Err(TaskError::Invalid(_))), "{r:?}");
    let mut admission = one.admission();
    admission.funding = Funding::UserKey;
    admission.user_key_id = Some(Uuid::new_v4());
    let a = admit_attempt(&mut tx, one.task.id, one.task.version, admission)
        .await
        .unwrap();
    assert_eq!(a.funding, Funding::UserKey);
    assert!(a.user_key_id.is_some());
}

#[tokio::test]
async fn admission_needs_a_ready_task() {
    let w = World::new().await;
    let task = w.task(&w.a).await; // planned
    let ready = w.ready_task(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let r = admit_attempt(&mut tx, task.id, task.version, ready.admission()).await;
    assert!(
        matches!(
            r,
            Err(TaskError::NotAdmissible {
                state: TaskState::Planned
            })
        ),
        "{r:?}"
    );
}

#[tokio::test]
async fn attempt_success_is_not_task_completion() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let done = transition_attempt(
        &mut tx,
        attempt.id,
        attempt.version,
        AttemptState::Succeeded,
        Some(json!({"report": "one focus added"})),
        None,
        Actor::Worker(1),
    )
    .await
    .unwrap();
    assert!(done.finished_at.is_some());
    let task_after = get_task(&mut tx, task.id).await.unwrap();
    assert_eq!(
        task_after.state,
        TaskState::Running,
        "the task is untouched"
    );
    assert_eq!(task_after.version, task.version);

    // The model's success cannot complete the task: only review can.
    let r = transition_task(
        &mut tx,
        task.id,
        task.version,
        TaskState::Completed,
        EventDraft::new(Actor::Worker(1), "task_completed", json!({})),
    )
    .await;
    assert!(
        matches!(r, Err(TaskError::InvalidTransition { .. })),
        "{r:?}"
    );
    let reviewing = transition_task(
        &mut tx,
        task.id,
        task.version,
        TaskState::InReview,
        EventDraft::new(
            Actor::Host,
            "task_in_review",
            json!({"attempt": attempt.id}),
        ),
    )
    .await
    .unwrap();
    let r = transition_task(
        &mut tx,
        task.id,
        reviewing.version,
        TaskState::Completed,
        EventDraft::new(Actor::Worker(1), "task_completed", json!({})),
    )
    .await;
    assert!(matches!(r, Err(TaskError::CompletionNotByWorker)), "{r:?}");
    let completed = transition_task(
        &mut tx,
        task.id,
        reviewing.version,
        TaskState::Completed,
        EventDraft::new(Actor::user(w.a.account), "task_completed", json!({})),
    )
    .await
    .unwrap();
    assert_eq!(completed.state, TaskState::Completed);

    // A completed task admits no attempt; a terminal attempt never changes.
    let ready = ReadyTask {
        task: completed.clone(),
        bundle: sample_bundle(&completed),
        incarnation: IncarnationId::new(),
    };
    let r = admit_attempt(&mut tx, task.id, completed.version, ready.admission()).await;
    assert!(matches!(r, Err(TaskError::TaskTerminal { .. })), "{r:?}");
    let r = transition_attempt(
        &mut tx,
        attempt.id,
        done.version,
        AttemptState::Running,
        None,
        None,
        Actor::Host,
    )
    .await;
    assert!(
        matches!(
            r,
            Err(TaskError::InvalidAttemptTransition {
                from: AttemptState::Succeeded,
                ..
            })
        ),
        "{r:?}"
    );
    let r = save_conversation(&mut tx, attempt.id, done.version, &json!({"late": true})).await;
    assert!(r.is_err());
    tx.commit().await.unwrap();

    // Even a superuser UPDATE of a terminal attempt is refused by the trigger.
    let err = must_fail(
        sqlx::query("UPDATE agent_attempts SET error_code = 'x' WHERE id = $1")
            .bind(attempt.id)
            .execute(&w.db.admin)
            .await,
    )
    .await;
    assert!(err.contains("terminal attempt"), "{err}");
}

#[tokio::test]
async fn attempt_transition_table_and_conversation_versioning() {
    let w = World::new().await;
    let (_task, attempt) = w.running_attempt(&w.a).await;
    assert_eq!(attempt.state, AttemptState::Running);
    assert!(attempt.started_at.is_some());
    let mut tx = w.begin(&w.a).await;
    let saved = save_conversation(&mut tx, attempt.id, attempt.version, &json!({"turns": [1]}))
        .await
        .unwrap();
    assert_eq!(saved.conversation["turns"], json!([1]));
    // An old writer holding the previous version loses.
    let r = save_conversation(&mut tx, attempt.id, attempt.version, &json!({"turns": [9]})).await;
    assert!(
        matches!(r, Err(TaskError::VersionConflict { current }) if current == saved.version),
        "{r:?}"
    );
    let r = transition_attempt(
        &mut tx,
        attempt.id,
        attempt.version,
        AttemptState::Failed,
        None,
        None,
        Actor::Host,
    )
    .await;
    assert!(matches!(r, Err(TaskError::VersionConflict { .. })), "{r:?}");
    // running -> admitted is not in the table.
    let r = transition_attempt(
        &mut tx,
        attempt.id,
        saved.version,
        AttemptState::Admitted,
        None,
        None,
        Actor::Host,
    )
    .await;
    assert!(
        matches!(r, Err(TaskError::InvalidAttemptTransition { .. })),
        "{r:?}"
    );
    // The cancellation path passes through interrupt_requested and reconciling.
    let mut v = saved.version;
    for to in [
        AttemptState::InterruptRequested,
        AttemptState::Reconciling,
        AttemptState::Cancelled,
    ] {
        v = transition_attempt(&mut tx, attempt.id, v, to, None, None, Actor::Host)
            .await
            .unwrap()
            .version;
    }
}

#[tokio::test]
async fn lease_takeover_increments_the_generation_and_fences_the_old_holder() {
    let w = World::new().await;
    let mut tx = w.begin(&w.a).await;
    let old = register_incarnation(&mut tx, "host-old").await.unwrap();
    let new = register_incarnation(&mut tx, "host-new").await.unwrap();
    let ttl = Duration::from_millis(400);
    let (project, owner) = (w.a.project, w.a.account);

    let first = acquire_lease(&mut tx, project, owner, old, ttl)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.generation, 1);
    check_fence(&mut tx, project, old, 1).await.unwrap();
    // Re-acquiring a live lease you hold keeps the generation.
    let again = acquire_lease(&mut tx, project, owner, old, ttl)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.generation, 1);
    // Another incarnation cannot take a live lease.
    assert!(
        acquire_lease(&mut tx, project, owner, new, ttl)
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();

    tokio::time::sleep(Duration::from_millis(600)).await;
    let mut tx = w.begin(&w.a).await;
    // Expired: the old holder is refused even before anyone takes over.
    assert!(matches!(
        check_fence(&mut tx, project, old, 1).await,
        Err(FenceError::LeaseExpired)
    ));
    assert!(matches!(
        renew_lease(&mut tx, project, old, 1, ttl).await,
        Err(FenceError::LeaseExpired)
    ));
    let taken = acquire_lease(&mut tx, project, owner, new, Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(taken.generation, 2);
    // The old generation can never act again, by generation or by holder.
    assert!(matches!(
        check_fence(&mut tx, project, old, 1).await,
        Err(FenceError::StaleGeneration {
            presented: 1,
            current: 2
        })
    ));
    assert!(matches!(
        check_fence(&mut tx, project, old, 2).await,
        Err(FenceError::NotHeld)
    ));
    assert!(matches!(
        renew_lease(&mut tx, project, old, 1, ttl).await,
        Err(FenceError::StaleGeneration { .. })
    ));
    assert!(
        acquire_lease(&mut tx, project, owner, old, ttl)
            .await
            .unwrap()
            .is_none()
    );
    check_fence(&mut tx, project, new, 2).await.unwrap();
    let renewed = renew_lease(&mut tx, project, new, 2, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(renewed.generation, 2);
    assert!(renewed.expires_at > taken.expires_at);
}

#[tokio::test]
async fn reacquiring_an_expired_lease_bumps_the_generation_even_for_the_same_incarnation() {
    let w = World::new().await;
    let mut tx = w.begin(&w.a).await;
    let inc = register_incarnation(&mut tx, "host").await.unwrap();
    let ttl = Duration::from_millis(200);
    let l1 = acquire_lease(&mut tx, w.a.project, w.a.account, inc, ttl)
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut tx = w.begin(&w.a).await;
    let l2 = acquire_lease(
        &mut tx,
        w.a.project,
        w.a.account,
        inc,
        Duration::from_secs(30),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!((l1.generation, l2.generation), (1, 2));
    // A worker still holding generation 1 from before the gap is refused.
    assert!(matches!(
        check_fence(&mut tx, w.a.project, inc, 1).await,
        Err(FenceError::StaleGeneration { .. })
    ));
}

#[tokio::test]
async fn a_fence_check_holds_the_lease_row_against_a_concurrent_renewal() {
    let w = World::new().await;
    let mut setup = w.begin(&w.a).await;
    let inc = register_incarnation(&mut setup, "host").await.unwrap();
    acquire_lease(
        &mut setup,
        w.a.project,
        w.a.account,
        inc,
        Duration::from_secs(30),
    )
    .await
    .unwrap()
    .unwrap();
    setup.commit().await.unwrap();

    let mut checker = w.begin(&w.a).await;
    check_fence(&mut checker, w.a.project, inc, 1)
        .await
        .unwrap();
    let repo = w.repo(&w.a);
    let project = w.a.project;
    let writer = tokio::spawn(async move {
        let mut tx = repo.begin().await.unwrap();
        let r = renew_lease(&mut tx, project, inc, 1, Duration::from_secs(60)).await;
        tx.commit().await.unwrap();
        r.is_ok()
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !writer.is_finished(),
        "the lease row is held FOR SHARE until the checker finishes"
    );
    checker.commit().await.unwrap();
    assert!(writer.await.unwrap());
}

#[tokio::test]
async fn incarnation_labels_carry_no_paths() {
    let w = World::new().await;
    let mut tx = w.begin(&w.a).await;
    for bad in ["", "/home/me/host", "has space", &"x".repeat(65)] {
        assert!(
            matches!(
                register_incarnation(&mut tx, bad).await,
                Err(TaskError::Invalid(_))
            ),
            "{bad:?}"
        );
    }
    let id = register_incarnation(&mut tx, "factory-server:1.2")
        .await
        .unwrap();
    touch_incarnation(&mut tx, id).await.unwrap();
}

#[tokio::test]
async fn project_purge_cascades_through_the_append_only_tables() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k1", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(w.count("SELECT count(*) FROM agent_events").await > 3);
    // The retention job's hard delete (as superuser here) is not blocked by the append-only
    // triggers when it arrives through the foreign key cascade.
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(w.a.project.0)
        .execute(&w.db.admin)
        .await
        .unwrap();
    for table in [
        "agent_tasks",
        "agent_events",
        "context_bundles",
        "agent_attempts",
        "tool_intents",
    ] {
        assert_eq!(
            w.count(&format!("SELECT count(*) FROM {table}")).await,
            0,
            "{table}"
        );
    }
    // A direct DELETE is still refused.
    let (task_b, _) = w.running_attempt(&w.b).await;
    let err = must_fail(
        sqlx::query("DELETE FROM agent_events WHERE task_id = $1")
            .bind(task_b.id)
            .execute(&w.db.admin)
            .await,
    )
    .await;
    assert!(err.contains("append-only"), "{err}");
}
