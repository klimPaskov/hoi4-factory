//! Recovery on REAL native journals.
//!
//! Each case drives Setup's transaction engine (`run_transaction`, with its public fault options)
//! to leave a genuine interrupted journal in the project's state root, then uses only the adapter
//! to inspect and recover it. The plan used to leave the journal is a native fixture (see
//! `common::native`); it does not go through the facade and is not model output.
//!
//! These tests inject graceful faults. They do NOT hard-kill the process mid-placement, which is
//! the case the PR 105 fix covers; that evidence stays pending until a Setup revision with the fix
//! is pinned (`SETUP_PIN.contains_pr_105_fix`).

mod common;

use common::native::{add_wiki_pages, native_plan, structural_record};
use common::*;
use factory_setup_adapter::*;
use factory_workspace::SnapshotInventory;
use hoi4_mod_setup::paths::validate_project_root;
use hoi4_mod_setup::service::root_identity;
use hoi4_mod_setup::transaction::{TransactionOptions, run_transaction};
use uuid::Uuid;

struct Interrupted {
    env: Env,
    adapter: SetupAdapter,
    fx: Fixture,
    plan_id: Uuid,
    before: SnapshotInventory,
}

fn inventory(env: &Env, fx: &Fixture) -> SnapshotInventory {
    let ws = env
        .storage
        .open_workspace(
            &factory_workspace::AccountId::new(fx.account.to_string()).unwrap(),
            &factory_workspace::ProjectId::new(fx.project.to_string()).unwrap(),
        )
        .unwrap();
    SnapshotInventory::capture(&fx.source, ws.git().unwrap().as_ref()).unwrap()
}

/// Leave an interrupted transaction of 2 generated files with the given fault.
async fn interrupted(options: impl FnOnce(&mut TransactionOptions)) -> Interrupted {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    // Opening a session creates the state root, as any real use does.
    adapter.open_session(alice, fx.project, None).await.unwrap();
    add_wiki_pages(&fx.source);
    let root = validate_project_root(&fx.source).unwrap();
    let identity = root_identity(&root).unwrap();
    let source = source_resolution().identity.clone();
    let (plan, files) = native_plan(&source, structural_record(&source), &identity, &fx.state, 2);
    let before = inventory(&env, &fx);
    // The file the adapter keeps beside the journal when it applies a plan itself.
    let adapter_dir = fx.state.join("adapter");
    std::fs::create_dir_all(&adapter_dir).unwrap();
    std::fs::write(
        adapter_dir.join(format!("pre-apply-{}.json", plan.plan_id)),
        serde_json::to_vec(&before).unwrap(),
    )
    .unwrap();
    let mut opts = TransactionOptions {
        app_data_root: Some(fx.state.clone()),
        ..Default::default()
    };
    options(&mut opts);
    let err =
        run_transaction(&root, &plan, &files, &opts).expect_err("the fault interrupts the run");
    assert!(err.to_string().contains("fault injected") || !err.to_string().is_empty());
    Interrupted {
        plan_id: plan.plan_id,
        env,
        adapter,
        fx,
        before,
    }
}

async fn view(t: &Interrupted) -> RecoveryView {
    t.adapter
        .inspect_recovery(t.fx.account, t.fx.project)
        .await
        .unwrap()
        .expect("an incomplete transaction")
}

#[tokio::test]
async fn a_partial_apply_is_rollback_only_and_rollback_restores_every_byte() {
    let t = interrupted(|o| o.fail_after_operation = Some(0)).await;
    assert!(
        t.fx.source.join("GENERATED-0.md").exists(),
        "the fault must leave a partial apply"
    );

    let v = view(&t).await;
    assert_eq!(v.variant, RecoveryVariant::RollbackRequiredPartialApply);
    assert_eq!(v.allowed_action, Some("rollback"));
    assert_eq!(v.journal_id, t.plan_id);
    assert!(v.project_apply_started && !v.needs_model && !v.owner_active);
    // Projected, not authoritative.
    let row = &t
        .adapter
        .setup_status(t.fx.account, t.fx.project, None)
        .await
        .unwrap();
    let tx = row.unfinished_transaction.as_ref().expect("projection row");
    assert_eq!(tx.state, "recovery_required");
    assert_eq!(
        tx.recovery_variant.as_deref(),
        Some("rollback_required_partial_apply")
    );
    assert_eq!(tx.allowed_action.as_deref(), Some("rollback"));
    assert_eq!(tx.journal_id, t.plan_id);

    // Handoff is blocked while the partial installation waits.
    let handoff = t
        .adapter
        .open_workspace(t.fx.account, t.fx.project, OpenWorkspaceOptions::default())
        .await
        .unwrap();
    assert!(!handoff.ready && handoff.descriptor.is_none() && !handoff.blockers.is_empty());

    // A new apply is refused while it waits.
    let session = t
        .adapter
        .open_session(t.fx.account, t.fx.project, None)
        .await
        .unwrap();
    let err = t
        .adapter
        .apply_setup_plan(
            t.fx.account,
            t.fx.project,
            session.session_id,
            Uuid::new_v4(),
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "recovery_required", "{err}");

    // Resume or discard is never accepted for this journal, whatever the caller asks.
    for wrong in [
        RecoveryVariant::ResumableStaging,
        RecoveryVariant::DiscardableStaging,
        RecoveryVariant::VerifiableFinalization,
    ] {
        let err = t
            .adapter
            .recover_setup(
                t.fx.account,
                t.fx.project,
                t.plan_id,
                wrong,
                OpContext::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "recovery_variant_changed", "{wrong:?}: {err}");
        assert!(
            t.fx.source.join("GENERATED-0.md").exists(),
            "a refused recovery changed the project"
        );
    }
    // A journal ID that does not exist.
    let err = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            Uuid::new_v4(),
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "no_incomplete_transaction");

    let out = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .expect("rollback");
    assert_eq!(out.action, "rollback");
    assert_eq!(out.journal_state, "rolled_back");
    assert!(out.remaining.is_none());

    // The project equals what it was before, by our own comparison and by the adapter's.
    let after = inventory(&t.env, &t.fx);
    let diff = t.before.diff(&after);
    assert!(
        diff.is_empty(),
        "rollback did not restore exactly: {diff:?}"
    );
    assert_eq!(out.restoration.as_ref().map(|r| r.exact), Some(true));
    assert!(out.restoration.unwrap().differences.is_empty());
    assert!(
        t.adapter
            .inspect_recovery(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_none()
    );

    // The row and the receipts follow the journal.
    let status = t
        .adapter
        .setup_status(t.fx.account, t.fx.project, None)
        .await
        .unwrap();
    assert!(status.unfinished_transaction.is_none());
    let finished = &status.recent_transactions[0];
    assert_eq!(finished.state, "rolled_back");
    assert!(finished.finished_at.is_some());
    let receipts = t
        .adapter
        .list_receipts(t.fx.account, t.fx.project, 20)
        .await
        .unwrap();
    let recover = receipts
        .iter()
        .find(|r| r.operation == "recover_setup" && r.status == "succeeded")
        .expect("recovery receipt");
    assert!(recover.fence_generation.is_some());
    assert_eq!(recover.setup_revision, SETUP_PIN.revision);
    assert_eq!(recover.setup_has_pr105_fix, SETUP_PIN.contains_pr_105_fix);
    assert!(
        t.adapter
            .unsettled_receipts(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_empty()
    );

    // With nothing waiting, the handoff descriptor is produced and says what is unresolved.
    let handoff = t
        .adapter
        .open_workspace(t.fx.account, t.fx.project, OpenWorkspaceOptions::default())
        .await
        .unwrap();
    assert!(handoff.ready, "{:?}", handoff.blockers);
    let d = handoff.descriptor.unwrap();
    assert_eq!(
        d.component_closure.setup_revision.as_deref(),
        Some(SETUP_PIN.revision)
    );
    // The PR 105 check stays unresolved only on a Setup revision without the fix.
    assert_eq!(
        d.unresolved_checks
            .iter()
            .any(|c| c.check.contains("PR 105")),
        !SETUP_PIN.contains_pr_105_fix
    );
    assert!(
        d.root_handles
            .iter()
            .all(|h| h.locator.starts_with("workspace:"))
    );
}

#[tokio::test]
async fn staged_work_before_apply_resumes_only_as_resumable_staging() {
    // Failing before stage 7 (validation) leaves complete, verified staging and no project change.
    let t = interrupted(|o| o.fail_before_stage = Some(7)).await;
    assert!(!t.fx.source.join("GENERATED-0.md").exists());
    let v = view(&t).await;
    assert_eq!(v.variant, RecoveryVariant::ResumableStaging);
    assert_eq!(v.allowed_action, Some("resume"));
    assert!(!v.project_apply_started);

    // Rollback is not offered for a project that was never touched.
    let err = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "recovery_variant_changed", "{err}");

    let out = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::ResumableStaging,
            OpContext::default(),
        )
        .await
        .expect("resume");
    assert_eq!(out.action, "resume");
    assert_eq!(out.journal_state, "completed");
    assert!(
        t.fx.source.join("GENERATED-0.md").exists() && t.fx.source.join("GENERATED-1.md").exists()
    );
    assert!(out.restoration.is_none());
    assert!(
        t.adapter
            .inspect_recovery(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_none()
    );
    let status = t
        .adapter
        .setup_status(t.fx.account, t.fx.project, None)
        .await
        .unwrap();
    assert_eq!(status.recent_transactions[0].state, "completed");
}

#[tokio::test]
async fn unfinished_staging_can_only_be_discarded() {
    let t = interrupted(|o| o.fail_before_stage = Some(6)).await;
    let v = view(&t).await;
    assert_eq!(v.variant, RecoveryVariant::DiscardableStaging);
    assert_eq!(v.allowed_action, Some("discard_staging"));
    let err = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::ResumableStaging,
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "recovery_variant_changed", "{err}");
    let out = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::DiscardableStaging,
            OpContext::default(),
        )
        .await
        .expect("discard");
    assert_eq!(out.action, "discard_staging");
    assert_eq!(out.journal_state, "staging_discarded");
    assert!(t.before.diff(&inventory(&t.env, &t.fx)).is_empty());
    assert!(
        t.adapter
            .inspect_recovery(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_none()
    );
}
#[tokio::test]
async fn a_finalizing_journal_without_a_committed_lock_is_rolled_back_exactly() {
    // Stage 11 is the rollback record. Failing after it leaves the transaction finalizing, with
    // the rollback record durable and the success lock never committed. Before HOI4-Mod-Setup
    // #107 the facade offered only a `resume` that refused, a dead end. The pinned revision
    // classifies it as a partial apply that must be rolled back, and the rollback is exact.
    let t = interrupted(|o| o.fail_after_stage = Some(11)).await;
    let v = view(&t).await;
    assert_eq!(v.journal_state, "finalizing");
    assert_eq!(v.variant, RecoveryVariant::RollbackRequiredPartialApply);
    assert_eq!(v.allowed_action, Some("rollback"));

    let out = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .expect("rollback");
    assert_eq!(out.action, "rollback");
    assert_eq!(out.journal_state, "rolled_back");
    let diff = t.before.diff(&inventory(&t.env, &t.fx));
    assert!(diff.is_empty(), "rollback did not restore exactly: {diff:?}");
    assert_eq!(out.restoration.as_ref().map(|r| r.exact), Some(true));
    assert!(
        t.adapter
            .inspect_recovery(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        t.adapter
            .unsettled_receipts(t.fx.account, t.fx.project)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn while_another_owner_recovers_the_view_offers_nothing_and_recovery_is_refused() {
    let t = interrupted(|o| o.fail_after_operation = Some(0)).await;
    let lease = ProjectLease::try_acquire(&t.env.database, t.fx.account, t.fx.project)
        .await
        .unwrap();
    let v = view(&t).await;
    assert!(v.owner_active);
    assert_eq!(v.allowed_action, None);
    let err = t
        .adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "project_busy");
    assert!(
        t.fx.source.join("GENERATED-0.md").exists(),
        "a busy refusal changed the project"
    );
    lease.release().await.unwrap();
    // Once free, the same request goes through.
    t.adapter
        .recover_setup(
            t.fx.account,
            t.fx.project,
            t.plan_id,
            RecoveryVariant::RollbackRequiredPartialApply,
            OpContext::default(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn a_second_account_cannot_inspect_or_recover_the_journal() {
    let t = interrupted(|o| o.fail_after_operation = Some(0)).await;
    let bob = t.env.account("bob").await;
    assert_eq!(
        t.adapter
            .inspect_recovery(bob, t.fx.project)
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    assert_eq!(
        t.adapter
            .recover_setup(
                bob,
                t.fx.project,
                t.plan_id,
                RecoveryVariant::RollbackRequiredPartialApply,
                OpContext::default()
            )
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    assert!(t.fx.source.join("GENERATED-0.md").exists());
}

#[tokio::test]
async fn a_journal_with_no_row_is_adopted_and_a_lost_apply_is_reconciled() {
    // The native journal exists but the database knows nothing: a restored database, say.
    let t = interrupted(|o| o.fail_after_operation = Some(0)).await;
    assert_eq!(
        t.env.count("SELECT count(*) FROM setup_transactions").await,
        0
    );
    let v = view(&t).await;
    assert_eq!(v.variant, RecoveryVariant::RollbackRequiredPartialApply);
    let row = sqlx::query_scalar::<_, String>("SELECT holder FROM setup_transactions")
        .fetch_one(&t.env.db.admin)
        .await
        .unwrap();
    assert!(row.starts_with("adopted:"), "{row}");

    // An 'applying' row whose process died before it wrote any journal: reconciled from the
    // inventories (the project is untouched), not left blocking forever.
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    adapter.open_session(alice, fx.project, None).await.unwrap();
    let pre = inventory(&env, &fx);
    let journal = Uuid::new_v4();
    let dir = fx.state.join("adapter");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("pre-apply-{journal}.json")),
        serde_json::to_vec(&pre).unwrap(),
    )
    .unwrap();
    Store::new(env.database.clone())
        .insert_transaction(
            &Scope {
                actor: alice,
                owner: alice,
                project: fx.project,
            },
            &TransactionInsert {
                id: Uuid::new_v4(),
                journal_id: journal,
                plan_id: None,
                session_id: None,
                mod_root: String::new(),
                state: "applying",
                plan_sha256: None,
                root_identity: None,
                pre_inventory_sha256: Some(pre.content_hash()),
                fence_generation: Some(1),
                holder: "dead-process".into(),
                started_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();
    assert!(
        adapter
            .inspect_recovery(alice, fx.project)
            .await
            .unwrap()
            .is_none()
    );
    let status = adapter.setup_status(alice, fx.project, None).await.unwrap();
    assert!(status.unfinished_transaction.is_none());
    assert_eq!(status.recent_transactions[0].state, "refused_before_effect");
}
