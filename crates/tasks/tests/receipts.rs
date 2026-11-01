//! C-04: intent before effect, observed outcome after, replay identity and reconciliation.

mod common;

use common::*;
use factory_tasks::*;
use serde_json::json;

fn outcome(status: ReceiptStatus) -> Outcome {
    Outcome {
        status,
        before: Some(hash(1)),
        after: Some(hash(2)),
        result_artifact: Some(json!({"bytes_written": 42})),
        partial: false,
    }
}

#[tokio::test]
async fn intent_is_recorded_before_any_receipt_and_never_started_is_visible() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let intent = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k1", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    assert_eq!(intent.fencing_generation, attempt.fencing_generation);
    assert_eq!(intent.grant_epoch, attempt.grant_epoch);
    assert_eq!(
        intent.args_sha256,
        canonical_sha256(&intent.request_artifact)
    );
    assert_eq!(intent.replay, replay_identity());
    // Intent exists, effect has not started: no receipt, nothing pending, listed as never started.
    assert!(get_receipt_of(&mut tx, intent.operation_id).await.is_none());
    assert!(pending_for_task(&mut tx, task.id).await.unwrap().is_empty());
    let never = never_started(&mut tx, task.id).await;
    assert_eq!(never.len(), 1);
    assert_eq!(never[0].operation_id, intent.operation_id);
    let events = events_after(&mut tx, task.id, 0, 100).await.unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.event_type, "tool_intent");
    assert_eq!(last.payload["operation_id"], json!(intent.operation_id));
    assert_eq!(last.attempt_id, Some(attempt.id));
    // Unresolved-effect admission only looks at receipts: a never-started intent does not block.
    assert!(
        unresolved_operations(&mut tx, task.id)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn get_receipt_of(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    op: OperationId,
) -> Option<Receipt> {
    factory_tasks::receipts::get_receipt(tx, op).await.unwrap()
}

async fn never_started(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    task: TaskId,
) -> Vec<Intent> {
    factory_tasks::receipts::never_started_for_task(tx, task)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_duplicate_idempotency_key_returns_the_existing_operation() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let first = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "same", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    let again = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "same", EffectClass::IdempotentWrite),
    )
    .await;
    match again {
        Err(IntentError::Duplicate {
            existing,
            same_args,
        }) => {
            assert_eq!(existing.operation_id, first.operation_id);
            assert!(same_args);
        }
        other => panic!("{other:?}"),
    }
    // The same key with different arguments is still the same operation, flagged.
    let mut changed = new_intent(&task, &attempt, "same", EffectClass::IdempotentWrite);
    changed.request_artifact = json!({"path": "other"});
    match record_intent(&mut tx, changed).await {
        Err(IntentError::Duplicate {
            existing,
            same_args,
        }) => {
            assert_eq!(existing.operation_id, first.operation_id);
            assert!(!same_args);
        }
        other => panic!("{other:?}"),
    }
    tx.commit().await.unwrap();
    assert_eq!(w.count("SELECT count(*) FROM tool_intents").await, 1);
    // A different key is a different operation.
    let mut tx = w.begin(&w.a).await;
    let second = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "other", EffectClass::PureRead),
    )
    .await
    .unwrap();
    assert_ne!(second.operation_id, first.operation_id);
}

#[tokio::test]
async fn concurrent_requests_with_one_key_make_one_operation() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let repo = w.repo(&w.a);
    let mut handles = Vec::new();
    for _ in 0..6 {
        let repo = repo.clone();
        let (task, attempt) = (task.clone(), attempt.clone());
        handles.push(tokio::spawn(async move {
            let mut tx = repo.begin().await.unwrap();
            let r = record_intent(
                &mut tx,
                new_intent(&task, &attempt, "race", EffectClass::IdempotentWrite),
            )
            .await;
            tx.commit().await.unwrap();
            r
        }));
    }
    let mut created = 0;
    let mut ids = std::collections::HashSet::new();
    for h in handles {
        match h.await.unwrap() {
            Ok(i) => {
                created += 1;
                ids.insert(i.operation_id);
            }
            Err(IntentError::Duplicate { existing, .. }) => {
                ids.insert(existing.operation_id);
            }
            Err(e) => panic!("{e:?}"),
        }
    }
    assert_eq!(created, 1);
    assert_eq!(ids.len(), 1);
    assert_eq!(w.count("SELECT count(*) FROM tool_intents").await, 1);
    // Event stream stays gapless under the race.
    let mut tx = w.begin(&w.a).await;
    let events = events_after(&mut tx, task.id, 0, 1000).await.unwrap();
    check_contiguous(0, &events).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event_type == "tool_intent")
            .count(),
        1
    );
}

#[tokio::test]
async fn credential_values_cannot_enter_a_request_artifact() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    for artifact in [
        json!({"headers": {"Authorization": "Bearer abc"}}),
        json!({"nested": [{"api_key": "sk-123"}]}),
        json!({"password": "hunter2"}),
    ] {
        let mut intent = new_intent(&task, &attempt, "s", EffectClass::IdempotentWrite);
        intent.request_artifact = artifact;
        assert!(matches!(
            record_intent(&mut tx, intent).await,
            Err(IntentError::SecretInArtifact { .. })
        ));
    }
    // References to stored credentials are fine, as is max_tokens.
    let mut ok = new_intent(&task, &attempt, "ok", EffectClass::PureRead);
    ok.request_artifact = json!({"api_key_ref": "key_9", "max_tokens": 100, "credential_id": "c1"});
    record_intent(&mut tx, ok).await.unwrap();
}

#[tokio::test]
async fn receipt_status_moves_forward_only() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let intent = record_intent(
        &mut tx,
        new_intent(&task, &attempt, "k", EffectClass::IdempotentWrite),
    )
    .await
    .unwrap();
    let op = intent.operation_id;
    assert!(
        matches!(
            settle(
                &mut tx,
                op,
                ReceiptStatus::Started,
                outcome(ReceiptStatus::Succeeded)
            )
            .await,
            Err(ReceiptError::NotFound) | Err(ReceiptError::StatusConflict { .. })
        ),
        "settling before the effect started is refused"
    );
    let started = mark_started(&mut tx, op).await.unwrap();
    assert_eq!(started.status, ReceiptStatus::Started);
    assert!(matches!(
        mark_started(&mut tx, op).await,
        Err(ReceiptError::AlreadyStarted {
            current: ReceiptStatus::Started
        })
    ));
    assert_eq!(pending_for_task(&mut tx, task.id).await.unwrap().len(), 1);
    // Only the four observed outcomes can be settled.
    for bad in [
        ReceiptStatus::Started,
        ReceiptStatus::UnknownEffect,
        ReceiptStatus::ReconciledApplied,
    ] {
        assert!(
            matches!(
                settle(&mut tx, op, ReceiptStatus::Started, outcome(bad)).await,
                Err(ReceiptError::Invalid(_))
            ),
            "{bad}"
        );
    }
    let mut bad_hash = outcome(ReceiptStatus::Succeeded);
    bad_hash.after = Some("not-a-hash".into());
    assert!(matches!(
        settle(&mut tx, op, ReceiptStatus::Started, bad_hash).await,
        Err(ReceiptError::Invalid(_))
    ));
    let settled = settle(
        &mut tx,
        op,
        ReceiptStatus::Started,
        outcome(ReceiptStatus::Succeeded),
    )
    .await
    .unwrap();
    assert_eq!(settled.status, ReceiptStatus::Succeeded);
    assert_eq!(settled.after_sha256, Some(hash(2)));
    assert_eq!(
        settled.result_sha256,
        Some(canonical_sha256(&json!({"bytes_written": 42})))
    );
    assert!(settled.finished_at.is_some());
    // A late or duplicate result cannot overwrite it.
    assert!(matches!(
        settle(
            &mut tx,
            op,
            ReceiptStatus::Started,
            outcome(ReceiptStatus::Failed)
        )
        .await,
        Err(ReceiptError::StatusConflict {
            current: ReceiptStatus::Succeeded,
            expected: ReceiptStatus::Started
        })
    ));
    assert!(matches!(
        mark_unknown(&mut tx, op, json!({})).await,
        Err(ReceiptError::StatusConflict { .. })
    ));
    assert!(pending_for_task(&mut tx, task.id).await.unwrap().is_empty());
    let types: Vec<String> = events_after(&mut tx, task.id, 0, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    assert!(
        types.ends_with(&[
            "tool_intent".into(),
            "tool_started".into(),
            "tool_settled".into()
        ]),
        "{types:?}"
    );
    tx.commit().await.unwrap();

    // The database refuses a backwards move even from a superuser statement, and the app role
    // cannot touch intents at all.
    let err = must_fail(
        sqlx::query("UPDATE tool_receipts SET status = 'started'")
            .execute(&w.db.admin)
            .await,
    )
    .await;
    assert!(err.contains("not a forward move"), "{err}");
    let mut tx = w.begin(&w.a).await;
    let err = must_fail(
        sqlx::query("UPDATE tool_intents SET tool = 'x'")
            .execute(&mut *tx)
            .await,
    )
    .await;
    assert!(err.contains("permission denied"), "{err}");
}

#[tokio::test]
async fn a_late_result_after_recovery_marked_the_effect_unknown_is_refused() {
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
    let unknown = mark_unknown(
        &mut tx,
        op,
        json!({"observed_hash": hash(9), "classification": "differs_from_both"}),
    )
    .await
    .unwrap();
    assert_eq!(unknown.status, ReceiptStatus::UnknownEffect);
    assert_eq!(
        unknown.observation.as_ref().unwrap()["classification"],
        "differs_from_both"
    );
    // The old worker finishes late.
    assert!(matches!(
        settle(
            &mut tx,
            op,
            ReceiptStatus::Started,
            outcome(ReceiptStatus::Succeeded)
        )
        .await,
        Err(ReceiptError::StatusConflict {
            current: ReceiptStatus::UnknownEffect,
            ..
        })
    ));
    assert_eq!(pending_for_task(&mut tx, task.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn reconciliation_needs_an_unknown_effect_and_an_accountable_decider() {
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
    let rec = |decision, by| Reconciliation {
        method: "observe_target_hash".into(),
        observation: json!({"observed": hash(2)}),
        decision,
        decided_by: by,
        observed_after_sha256: Some(hash(2)),
    };
    // Not yet unknown: reconcile refuses, so a started operation cannot be waved through.
    assert!(matches!(
        reconcile(
            &mut tx,
            op,
            rec(ReconciliationDecision::Applied, Actor::Host)
        )
        .await,
        Err(ReceiptError::StatusConflict {
            current: ReceiptStatus::Started,
            ..
        })
    ));
    mark_unknown(&mut tx, op, json!({})).await.unwrap();
    assert!(matches!(
        reconcile(
            &mut tx,
            op,
            rec(ReconciliationDecision::Applied, Actor::Worker(1))
        )
        .await,
        Err(ReceiptError::ReconcileByWorker)
    ));
    let done = reconcile(
        &mut tx,
        op,
        rec(ReconciliationDecision::Applied, Actor::user(w.a.account)),
    )
    .await
    .unwrap();
    assert_eq!(done.status, ReceiptStatus::ReconciledApplied);
    assert_eq!(done.after_sha256, Some(hash(2)));
    let r = done.reconciliation.unwrap();
    assert_eq!(r["method"], "observe_target_hash");
    assert_eq!(r["decided_by"], format!("user:{}", w.a.account));
    assert!(r["at"].is_string());
    assert!(matches!(
        reconcile(
            &mut tx,
            op,
            rec(ReconciliationDecision::NotApplied, Actor::Host)
        )
        .await,
        Err(ReceiptError::StatusConflict {
            current: ReceiptStatus::ReconciledApplied,
            ..
        })
    ));
    let last = events_after(&mut tx, task.id, 0, 100)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(last.event_type, "tool_reconciled");
    assert_eq!(last.actor, Actor::user(w.a.account));
}

#[tokio::test]
async fn an_unresolved_effect_blocks_the_next_attempt_until_reconciled() {
    // ORCH-03/04/09: kill after intent, the effect is uncertain, and a new attempt must not run.
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
    // The worker dies: the attempt fails, the task is released.
    let failed = transition_attempt(
        &mut tx,
        attempt.id,
        attempt.version,
        AttemptState::Failed,
        None,
        Some("worker_lost"),
        Actor::Host,
    )
    .await
    .unwrap();
    let task_now = get_task(&mut tx, task.id).await.unwrap();
    let task_ready = transition_task(
        &mut tx,
        task.id,
        task_now.version,
        TaskState::Ready,
        EventDraft::new(Actor::Host, "", json!({})),
    )
    .await
    .unwrap();
    let bundle = sample_bundle(&task_ready);
    store_bundle(&mut tx, w.a.project, w.a.account, task.id, &bundle)
        .await
        .unwrap();
    let admission = Admission {
        bundle_id: bundle.id,
        grant_id: failed.grant_id,
        grant_epoch: failed.grant_epoch,
        incarnation: failed.host_incarnation,
        generation: failed.fencing_generation,
        model_profile: failed.model_profile.clone(),
        funding: Funding::OperatorAllowance,
        user_key_id: None,
    };
    // Started: blocked, and the blockers are named.
    match admit_attempt(&mut tx, task.id, task_ready.version, admission.clone()).await {
        Err(TaskError::UnresolvedEffects { operations }) => assert_eq!(operations, [op]),
        other => panic!("{other:?}"),
    }
    // Recovery marks it unknown: still blocked.
    mark_unknown(
        &mut tx,
        op,
        json!({"classification": "target_unchanged_from_before_hash"}),
    )
    .await
    .unwrap();
    assert!(matches!(
        admit_attempt(&mut tx, task.id, task_ready.version, admission.clone()).await,
        Err(TaskError::UnresolvedEffects { .. })
    ));
    assert_eq!(unresolved_operations(&mut tx, task.id).await.unwrap(), [op]);
    // The next bundle must show it.
    let unresolved = factory_tasks::receipts::receipt_summaries(&mut tx, task.id, true)
        .await
        .unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].status, ReceiptStatus::UnknownEffect);
    // Reconciled as not applied: the way is clear and the old operation is not repeated.
    reconcile(
        &mut tx,
        op,
        Reconciliation {
            method: "observe_target_hash".into(),
            observation: json!({"observed": hash(1)}),
            decision: ReconciliationDecision::NotApplied,
            decided_by: Actor::Host,
            observed_after_sha256: None,
        },
    )
    .await
    .unwrap();
    let second = admit_attempt(&mut tx, task.id, task_ready.version, admission)
        .await
        .unwrap();
    assert_eq!(second.attempt_no, 2);
    let settled = factory_tasks::receipts::receipt_summaries(&mut tx, task.id, false)
        .await
        .unwrap();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].status, ReceiptStatus::ReconciledNotApplied);
}

#[tokio::test]
async fn pause_and_finished_attempts_stop_new_effects() {
    let w = World::new().await;
    let (task, attempt) = w.running_attempt(&w.a).await;
    let mut tx = w.begin(&w.a).await;
    let started_op = record_intent(
        &mut tx,
        new_intent(
            &task,
            &attempt,
            "before-pause",
            EffectClass::IdempotentWrite,
        ),
    )
    .await
    .unwrap()
    .operation_id;
    let paused = set_paused(&mut tx, task.id, task.version, true, None, Actor::Host)
        .await
        .unwrap();
    assert!(matches!(
        record_intent(
            &mut tx,
            new_intent(&task, &attempt, "new", EffectClass::PureRead)
        )
        .await,
        Err(IntentError::Paused)
    ));
    assert!(matches!(
        mark_started(&mut tx, started_op).await,
        Err(ReceiptError::Paused)
    ));
    // A retried request still gets its original operation while paused.
    assert!(matches!(
        record_intent(
            &mut tx,
            new_intent(
                &task,
                &attempt,
                "before-pause",
                EffectClass::IdempotentWrite
            )
        )
        .await,
        Err(IntentError::Duplicate { .. })
    ));
    set_paused(&mut tx, task.id, paused.version, false, None, Actor::Host)
        .await
        .unwrap();
    // An interrupt request revokes further effects.
    transition_attempt(
        &mut tx,
        attempt.id,
        attempt.version,
        AttemptState::InterruptRequested,
        None,
        None,
        Actor::Host,
    )
    .await
    .unwrap();
    assert!(matches!(
        record_intent(
            &mut tx,
            new_intent(&task, &attempt, "late", EffectClass::PureRead)
        )
        .await,
        Err(IntentError::AttemptNotActive {
            state: AttemptState::InterruptRequested
        })
    ));
    assert!(matches!(
        mark_started(&mut tx, started_op).await,
        Err(ReceiptError::AttemptNotActive { .. })
    ));
}

// ----- replay_decision --------------------------------------------------------------------------

fn intent_of(class: EffectClass) -> Intent {
    Intent {
        operation_id: OperationId::new(),
        task_id: TaskId::new(),
        attempt_id: AttemptId::new(),
        request_id: "r".into(),
        idempotency_key: "k".into(),
        replay: replay_identity(),
        schema_version: "1".into(),
        effect_class: class,
        args_sha256: hash(1),
        request_artifact: json!({}),
        grant_id: uuid::Uuid::new_v4(),
        grant_epoch: 1,
        fencing_generation: 1,
        retry_class: RetryClass::ReconcileFirst,
        reconciliation_method: "m".into(),
        observation_inputs: json!({}),
        created_at: chrono::Utc::now(),
    }
}

fn receipt_of(status: ReceiptStatus, partial: bool) -> Receipt {
    Receipt {
        operation_id: OperationId::new(),
        task_id: TaskId::new(),
        status,
        started_at: chrono::Utc::now(),
        finished_at: None,
        before_sha256: None,
        after_sha256: None,
        result_artifact: None,
        result_sha256: None,
        partial,
        observation: None,
        reconciliation: None,
    }
}

#[test]
fn replay_decision_table() {
    use ReplayDecision::*;
    type C = EffectClass;
    type S = ReceiptStatus;
    let id = replay_identity();
    let cases: Vec<(C, S, bool, ReplayDecision)> = vec![
        // Settled results are reused, whatever the class.
        (C::IdempotentWrite, S::Succeeded, false, ReuseResult),
        (C::IdempotentWrite, S::Unchanged, false, ReuseResult),
        (C::IdempotentWrite, S::ReconciledApplied, false, ReuseResult),
        (C::PureRead, S::Succeeded, false, ReuseResult),
        (C::UnknownEffect, S::Succeeded, false, ReuseResult),
        // A write that may have happened must be reconciled; a read may just be repeated.
        (C::IdempotentWrite, S::Started, false, MustReconcile),
        (C::IdempotentWrite, S::UnknownEffect, false, MustReconcile),
        (C::UnknownEffect, S::Started, false, MustReconcile),
        (C::PureRead, S::Started, false, SafeToRepeat),
        (C::PureRead, S::UnknownEffect, false, SafeToRepeat),
        (C::DerivedJob, S::Started, false, SafeToRepeat),
        // Failures: partial writes and unknown-effect tools need reconciliation.
        (C::IdempotentWrite, S::Failed, false, SafeToRepeat),
        (C::IdempotentWrite, S::Failed, true, MustReconcile),
        (C::UnknownEffect, S::Failed, false, MustReconcile),
        (C::PureRead, S::Failed, true, SafeToRepeat),
        // Nothing happened.
        (C::IdempotentWrite, S::Refused, false, SafeToRepeat),
        (C::UnknownEffect, S::Refused, false, SafeToRepeat),
        (
            C::IdempotentWrite,
            S::ReconciledNotApplied,
            false,
            SafeToRepeat,
        ),
        (
            C::IdempotentWrite,
            S::ReconciledConflict,
            false,
            MustReconcile,
        ),
    ];
    for (class, status, partial, expected) in cases {
        let got = replay_decision(&intent_of(class), Some(&receipt_of(status, partial)), &id);
        assert_eq!(got, expected, "{class} / {status} / partial={partial}");
    }
    // Never started: the first execution is not a replay.
    assert_eq!(
        replay_decision(&intent_of(C::IdempotentWrite), None, &id),
        SafeToRepeat
    );
}

#[test]
fn any_changed_identity_rejects_the_old_decision() {
    // ORCH-09: change cwd/root, source hash or tool version before retry.
    let intent = intent_of(EffectClass::PureRead);
    let settled = receipt_of(ReceiptStatus::Succeeded, false);
    let mut cases: Vec<(&str, ReplayIdentity)> = Vec::new();
    for (field, edit) in [
        (
            "workspace_id",
            (|i: &mut ReplayIdentity| i.workspace_id = "ws-2".into()) as fn(&mut ReplayIdentity),
        ),
        ("root_identity", |i| i.root_identity = "root-2".into()),
        ("source_snapshot", |i| i.source_snapshot = "snap-2".into()),
        ("tool", |i| i.tool = "read_file".into()),
        ("tool_version", |i| i.tool_version = "1.0.1".into()),
    ] {
        let mut current = replay_identity();
        edit(&mut current);
        cases.push((field, current));
    }
    for (field, current) in &cases {
        // Even a settled result and even a pure read are rejected: the world moved.
        for receipt in [
            Some(&settled),
            Some(&receipt_of(ReceiptStatus::Started, false)),
            None,
        ] {
            assert_eq!(
                replay_decision(&intent, receipt, current),
                ReplayDecision::Rejected {
                    changed: vec![*field]
                },
                "{field}"
            );
        }
    }
    let mut two = replay_identity();
    two.source_snapshot = "x".into();
    two.tool_version = "9".into();
    assert_eq!(
        replay_decision(
            &intent_of(EffectClass::IdempotentWrite),
            Some(&settled),
            &two
        ),
        ReplayDecision::Rejected {
            changed: vec!["source_snapshot", "tool_version"]
        }
    );
}

#[tokio::test]
async fn replay_decision_over_stored_rows() {
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
    let (stored, receipt) = pending_for_task(&mut tx, task.id)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        replay_decision(&stored, Some(&receipt), &replay_identity()),
        ReplayDecision::MustReconcile
    );
    let mut moved = replay_identity();
    moved.source_snapshot = "snap-2".into();
    assert_eq!(
        replay_decision(&stored, Some(&receipt), &moved),
        ReplayDecision::Rejected {
            changed: vec!["source_snapshot"]
        }
    );
}
