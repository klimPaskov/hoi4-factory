//! Inspect, refusals, gates, receipts, isolation and roles. Needs PostgreSQL (see `common`).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::native::{FixtureBuilder, MustNotBeCalled};
use common::*;
use factory_accounts::{Role, Visibility};
use factory_setup_adapter::*;
use factory_workspace::{FileState, SnapshotInventory};
use hoi4_mod_setup::codex::ApprovedEvidence;
use hoi4_mod_setup::security::sha256_bytes;
use serde_json::{Value, json};
use uuid::Uuid;

fn inventory(env: &Env, fx: &Fixture) -> SnapshotInventory {
    let ws = env
        .storage
        .open_workspace(
            &factory_workspace::AccountId::new(fx.account.to_string()).unwrap(),
            &factory_workspace::ProjectId::new(fx.project.to_string()).unwrap(),
        )
        .unwrap();
    let repo = ws.git().unwrap();
    SnapshotInventory::capture(&fx.source, repo.as_ref()).unwrap()
}

async fn started(env: &Env, adapter: &SetupAdapter, fx: &Fixture) -> SessionInfo {
    let _ = env;
    adapter
        .open_session(fx.account, fx.project, None)
        .await
        .expect("session opens")
}

#[tokio::test]
async fn inspect_leaves_every_byte_mode_and_the_git_index_identical() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();

    // The fixture really is dirty: staged edit, unstaged edit and untracked files.
    let before = inventory(&env, &fx);
    assert!(before.git_index_sha256.is_some());
    assert!(!before.paths_in_state(FileState::Staged).is_empty());
    assert!(!before.paths_in_state(FileState::TrackedModified).is_empty());
    assert!(!before.paths_in_state(FileState::Untracked).is_empty());

    let session = started(&env, &adapter, &fx).await;
    let out = adapter
        .inspect_project(
            fx.account,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .expect("inspect succeeds");

    // Independent comparison with the workspace crate's inventory.
    let after = inventory(&env, &fx);
    let diff = before.diff(&after);
    assert!(diff.is_empty(), "inspect changed the workspace: {diff:?}");
    assert_eq!(before.git_index_sha256, after.git_index_sha256);
    assert_eq!(before.content_hash(), after.content_hash());
    // The adapter's own guard saw the same.
    assert_eq!(out.workspace_unchanged, Some(true));
    assert_eq!(out.inventory_before_sha256, out.inventory_after_sha256);
    assert_eq!(out.inventory_before_sha256, Some(before.content_hash()));

    assert!(!out.scan.partial && !out.scan.cancelled);
    assert!(!out.root_identity.is_empty());
    assert!(
        out.scan.project_root.starts_with("workspace:"),
        "{}",
        out.scan.project_root
    );
    // No host path leaves the server.
    let json = serde_json::to_string(&out).unwrap();
    for p in [&fx.source, &fx.state, env.tmp.path()] {
        let text = p.display().to_string();
        assert!(!json.contains(&text), "host path {text} leaked");
        assert!(
            !json.contains(&text.replace('\\', "\\\\")),
            "escaped host path leaked"
        );
    }
    // The state root is outside source/ and holds no transaction.
    assert!(fx.state.is_dir());
    assert!(!fx.source.join(STATE_DIR_NAME).exists());
    assert!(!fx.state.join("transactions").exists());

    // Receipts and rows are persisted.
    assert_eq!(env.count("SELECT count(*) FROM setup_sessions").await, 1);
    assert_eq!(
        env.count(&format!(
            "SELECT count(*) FROM setup_scans WHERE id = '{}' AND snapshot_unchanged",
            out.scan_id
        ))
        .await,
        1
    );
    let receipts = adapter
        .list_receipts(fx.account, fx.project, 10)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    let r = &receipts[0];
    assert_eq!(
        (r.operation.as_str(), r.status.as_str()),
        ("inspect_project", "succeeded")
    );
    assert_eq!(r.id, out.receipt_id);
    assert_eq!(r.setup_revision, SETUP_PIN.revision);
    assert!(!r.setup_has_pr105_fix);
    assert_eq!(r.before_inventory_sha256, r.after_inventory_sha256);
    assert!(r.finished_at.is_some());
}

#[tokio::test]
async fn paths_outside_the_workspace_are_refused_and_leave_nothing_behind() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let before = inventory(&env, &fx);

    // A sibling directory that a traversal would reach.
    std::fs::create_dir_all(fx.source.parent().unwrap().join("tasks/other")).unwrap();
    let absolute_outside = fx.source.parent().unwrap().display().to_string();
    let cases: Vec<String> = vec![
        "..".into(),
        "../tasks".into(),
        "common/../../tasks".into(),
        "/etc".into(),
        "C:\\Windows".into(),
        "\\\\server\\share".into(),
        absolute_outside,
        "no/such/dir".into(),
    ];
    for bad in &cases {
        let err = adapter
            .open_session(fx.account, fx.project, Some(bad))
            .await
            .expect_err(bad);
        assert_eq!(err.code(), "outside_workspace", "{bad}: {err}");
    }

    // A link out of source/ is refused when the platform lets the test create one.
    let outside = fx.source.parent().unwrap().join("tasks/other");
    let link = fx.source.join("escape");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, &link).is_ok();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&outside, &link).is_ok();
    if linked {
        let err = adapter
            .open_session(fx.account, fx.project, Some("escape"))
            .await
            .expect_err("a link out of source/");
        assert_eq!(err.code(), "outside_workspace", "{err}");
        std::fs::remove_dir(&link)
            .or_else(|_| std::fs::remove_file(&link))
            .unwrap();
    } else {
        eprintln!(
            "NOTE: the platform refused to create a symlink; the link case was not exercised"
        );
    }

    assert_eq!(env.count("SELECT count(*) FROM setup_sessions").await, 0);
    assert_eq!(env.count("SELECT count(*) FROM setup_receipts").await, 0);
    assert!(before.diff(&inventory(&env, &fx)).is_empty());
}

#[tokio::test]
async fn a_session_belongs_to_one_account_and_one_project() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let first = env.project_with_import(alice).await;
    let second = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let session = started(&env, &adapter, &first).await;

    // The session ID is no capability on another project of the same account.
    let err = adapter
        .inspect_project(
            alice,
            second.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .expect_err("wrong project");
    assert_eq!(err.code(), "not_found");
    // An unknown session ID.
    let err = adapter
        .inspect_project(
            alice,
            first.project,
            OnboardingSessionId::new(),
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .expect_err("unknown session");
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn another_account_cannot_reach_setup_records_or_the_workspace() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let bob = env.account("bob").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let session = started(&env, &adapter, &fx).await;
    adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();

    // Through the adapter: not found for every operation, nothing revealed.
    let ctx = OpContext::default;
    let denied = |e: SetupError| assert_eq!(e.code(), "not_found", "{e}");
    denied(
        adapter
            .open_session(bob, fx.project, None)
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .inspect_project(
                bob,
                fx.project,
                session.session_id,
                InspectOptions::default(),
                ctx(),
            )
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .setup_status(bob, fx.project, None)
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .list_receipts(bob, fx.project, 10)
            .await
            .unwrap_err(),
    );
    denied(adapter.inspect_recovery(bob, fx.project).await.unwrap_err());
    denied(
        adapter
            .open_workspace(bob, fx.project, OpenWorkspaceOptions::default())
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .current_plan(bob, fx.project, None)
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .apply_setup_plan(bob, fx.project, session.session_id, Uuid::new_v4(), ctx())
            .await
            .unwrap_err(),
    );
    denied(
        adapter
            .recover_setup(
                bob,
                fx.project,
                Uuid::new_v4(),
                RecoveryVariant::RollbackRequiredPartialApply,
                ctx(),
            )
            .await
            .unwrap_err(),
    );

    // In the database, as the application role: bob sees no row of any setup table, alice sees
    // hers, and an unscoped connection sees nothing at all.
    let tables = [
        "setup_sessions",
        "setup_scans",
        "setup_analyses",
        "setup_plans",
        "setup_approvals",
        "setup_transactions",
        "setup_receipts",
    ];
    async fn visible(env: &Env, account: Option<Uuid>, table: &str) -> i64 {
        let mut tx = env.db.app.begin().await.unwrap();
        if let Some(a) = account {
            sqlx::query("SELECT set_config('app.account_id', $1, true)")
                .bind(a.to_string())
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        n
    }
    for table in tables {
        assert_eq!(
            visible(&env, Some(bob.0), table).await,
            0,
            "{table} visible to bob"
        );
        assert_eq!(
            visible(&env, None, table).await,
            0,
            "{table} visible without a scope"
        );
    }
    assert_eq!(visible(&env, Some(alice.0), "setup_sessions").await, 1);
    assert_eq!(visible(&env, Some(alice.0), "setup_scans").await, 1);
    assert!(visible(&env, Some(alice.0), "setup_receipts").await >= 1);

    // And bob cannot write a row into alice's project (row-level policy).
    let mut tx = env.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(bob.0.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let forged = sqlx::query(
        "INSERT INTO setup_sessions (id, project_id, owner_account_id, opened_by_account_id,
             setup_repository, setup_revision, setup_has_pr105_fix, setup_default_plan_builder, opened_at)
         VALUES ($1, $2, $3, $4, 'x', $5, false, false, now())",
    )
    .bind(Uuid::new_v4())
    .bind(fx.project.0)
    .bind(alice.0)
    .bind(bob.0)
    .bind(SETUP_PIN.revision)
    .execute(&mut *tx)
    .await;
    assert!(
        forged.is_err(),
        "a forged row for another account's project was accepted"
    );
}

#[tokio::test]
async fn roles_limit_what_each_member_may_do() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let viewer = env.account("viewer").await;
    let fx = env.project_with_import(alice).await;
    env.database
        .scoped(alice)
        .add_member(fx.project, viewer, Role::Viewer)
        .await
        .unwrap();
    let adapter = env.adapter_with(|d| d.with_plan_builder(Arc::new(MustNotBeCalled)));

    // A viewer reads and may inspect in a session of their own.
    let session = adapter
        .open_session(viewer, fx.project, None)
        .await
        .unwrap();
    adapter
        .inspect_project(
            viewer,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    adapter
        .setup_status(viewer, fx.project, None)
        .await
        .unwrap();

    // But cannot analyze, plan, approve, apply or recover.
    let forbidden = |e: SetupError| assert_eq!(e.code(), "forbidden", "{e}");
    forbidden(
        adapter
            .confirm_setup_evidence(
                viewer,
                fx.project,
                session.session_id,
                Uuid::new_v4(),
                vec![],
                OpContext::default(),
            )
            .await
            .unwrap_err(),
    );
    forbidden(
        adapter
            .plan_setup(
                viewer,
                fx.project,
                session.session_id,
                json!({}),
                OpContext::default(),
            )
            .await
            .unwrap_err(),
    );
    forbidden(
        adapter
            .approve_setup_plan(
                viewer,
                fx.project,
                session.session_id,
                Uuid::new_v4(),
                "x",
                OpContext::default(),
            )
            .await
            .unwrap_err(),
    );
    forbidden(
        adapter
            .apply_setup_plan(
                viewer,
                fx.project,
                session.session_id,
                Uuid::new_v4(),
                OpContext::default(),
            )
            .await
            .unwrap_err(),
    );
    forbidden(
        adapter
            .recover_setup(
                viewer,
                fx.project,
                Uuid::new_v4(),
                RecoveryVariant::ResumableStaging,
                OpContext::default(),
            )
            .await
            .unwrap_err(),
    );
    // The viewer's session is the viewer's: the owner cannot use it, and it left no effect.
    assert_eq!(
        adapter
            .inspect_project(
                alice,
                fx.project,
                session.session_id,
                InspectOptions::default(),
                OpContext::default()
            )
            .await
            .unwrap_err()
            .code(),
        "not_found"
    );
    assert_eq!(env.count("SELECT count(*) FROM setup_plans").await, 0);
}

#[tokio::test]
async fn planning_is_reported_unavailable_not_faked() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let session = started(&env, &adapter, &fx).await;

    let caps = adapter.capabilities();
    let plan = caps.get("planSetup").unwrap();
    assert!(!plan.available);
    assert!(
        plan.reason.as_deref().unwrap().contains("PlanBuilder"),
        "{:?}",
        plan.reason
    );
    assert!(!caps.get("applySetupPlan").unwrap().available);

    let err = adapter
        .plan_setup(
            alice,
            fx.project,
            session.session_id,
            json!({"identity": {}}),
            OpContext::default(),
        )
        .await
        .expect_err("no builder");
    assert_eq!(err.code(), "capability_unavailable");
    assert!(matches!(
        err,
        SetupError::CapabilityUnavailable {
            capability: "plan_builder",
            ..
        }
    ));
    // The envelope the server returns.
    let api: factory_accounts::ApiError = err.into();
    assert_eq!(api.status.as_u16(), 501);
    assert_eq!(api.code, "capability_unavailable");

    // The refusal is on record; no plan exists.
    let receipts = adapter.list_receipts(alice, fx.project, 10).await.unwrap();
    let r = receipts
        .iter()
        .find(|r| r.operation == "plan_setup")
        .expect("plan receipt");
    assert_eq!(r.status, "refused");
    assert_eq!(r.error_code.as_deref(), Some("capability_unavailable"));
    assert_eq!(env.count("SELECT count(*) FROM setup_plans").await, 0);
    assert!(
        adapter
            .current_plan(alice, fx.project, None)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn apply_is_refused_without_an_approved_plan_and_changes_nothing() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter_with(|d| d.with_plan_builder(Arc::new(MustNotBeCalled)));
    let session = started(&env, &adapter, &fx).await;
    let before = inventory(&env, &fx);

    let err = adapter
        .apply_setup_plan(
            alice,
            fx.project,
            session.session_id,
            Uuid::new_v4(),
            OpContext::default(),
        )
        .await
        .expect_err("no such plan");
    assert_eq!(err.code(), "plan_not_approved", "{err}");
    assert!(before.diff(&inventory(&env, &fx)).is_empty());
    assert!(!fx.state.join("transactions").exists());
    assert_eq!(
        env.count("SELECT count(*) FROM setup_transactions").await,
        0
    );
    let r = &adapter.list_receipts(alice, fx.project, 5).await.unwrap()[0];
    assert_eq!(
        (r.operation.as_str(), r.status.as_str()),
        ("apply_setup_plan", "refused")
    );
}

#[tokio::test]
async fn planning_needs_a_confirmed_analysis_and_the_builder_is_never_reached_without_one() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter_with(|d| d.with_plan_builder(Arc::new(MustNotBeCalled)));
    let session = started(&env, &adapter, &fx).await;
    let state = json!({
        "aiProvider": "local",
        "aiModel": "fixture",
        "aiReasoningEffort": "high",
        "aiEndpoint": common::native_endpoint(),
        "identity": {"projectRoot": fx.source.display().to_string(), "projectId": "example"},
    });
    let err = adapter
        .plan_setup(
            alice,
            fx.project,
            session.session_id,
            state,
            OpContext::default(),
        )
        .await
        .expect_err("no analysis");
    assert_eq!(err.code(), "analysis_not_confirmed", "{err}");
    assert_eq!(env.count("SELECT count(*) FROM setup_plans").await, 0);
}

#[tokio::test]
async fn a_database_plan_with_an_unconfirmed_analysis_is_refused_before_any_effect() {
    // A plan row the facade never produced (inserted as the superuser) cannot pass the adapter's
    // own gate either: apply needs an approved plan AND a confirmed analysis on record.
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let session = started(&env, &adapter, &fx).await;
    let plan_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO setup_plans (id, session_id, project_id, owner_account_id, plan_sha256, root_identity,
             plan, operation_count, state, created_at, updated_at)
         VALUES ($1, $2, $3, $4, 'h', 'r', '{}', 0, 'approved', now(), now())",
    )
    .bind(plan_id)
    .bind(session.session_id.0)
    .bind(fx.project.0)
    .bind(alice.0)
    .execute(&env.db.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO setup_approvals (id, plan_id, session_id, project_id, owner_account_id, approved_by_account_id,
             reviewed_plan_sha256, approved_plan_sha256, root_identity, created_at)
         VALUES ($1, $2, $3, $4, $5, $5, 'h', 'h', 'r', now())",
    )
    .bind(Uuid::new_v4())
    .bind(plan_id)
    .bind(session.session_id.0)
    .bind(fx.project.0)
    .bind(alice.0)
    .execute(&env.db.admin)
    .await
    .unwrap();
    let before = inventory(&env, &fx);
    let err = adapter
        .apply_setup_plan(
            alice,
            fx.project,
            session.session_id,
            plan_id,
            OpContext::default(),
        )
        .await
        .expect_err("unconfirmed analysis");
    assert_eq!(err.code(), "analysis_not_confirmed", "{err}");
    assert!(before.diff(&inventory(&env, &fx)).is_empty());
    assert_eq!(
        env.count("SELECT count(*) FROM setup_transactions").await,
        0
    );
}

// ----- analysis (offline: the model is a test client that only fails or answers garbage) -----

struct Scripted {
    calls: AtomicUsize,
    reply: Result<Value, String>,
}

#[async_trait]
impl ModelClient for Scripted {
    async fn complete(&self, _: ModelRequest) -> Result<ModelReply, ModelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.reply {
            Ok(v) => Ok(ModelReply {
                reply: v.clone(),
                response_id: None,
                usage: None,
            }),
            Err(e) => Err(ModelError::Failed(e.clone())),
        }
    }
}

fn analysis_input(scan_id: Uuid, evidence: Vec<ApprovedEvidence>) -> AnalysisInput {
    AnalysisInput {
        provider: "local".into(),
        model: "fixture".into(),
        reasoning_effort: "high".into(),
        endpoint: common::native_endpoint().into(),
        credential: None,
        mode: "existing_project_semantics".into(),
        brief: String::new(),
        evidence,
        constraints: json!({}),
        purpose: Some("existing_project_import".into()),
        scan_id: Some(scan_id),
    }
}

fn evidence_from(out: &InspectOutcome) -> Vec<ApprovedEvidence> {
    let finding = out
        .scan
        .findings
        .iter()
        .find(|f| !f.evidence.is_empty() && f.model_excerpt.is_some())
        .expect("the fixture yields a finding with evidence");
    let excerpt = finding.model_excerpt.clone().unwrap();
    vec![ApprovedEvidence {
        reference: finding.id.clone(),
        path: finding.evidence[0].path.clone(),
        excerpt_sha256: sha256_bytes(excerpt.as_bytes()),
        excerpt,
        confidence: None,
    }]
}

#[tokio::test]
async fn analysis_without_a_model_client_is_unavailable() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    let session = started(&env, &adapter, &fx).await;
    let out = adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let err = adapter
        .analyze_setup(
            alice,
            fx.project,
            session.session_id,
            analysis_input(out.scan_id, vec![]),
            source_resolution(),
            OpContext::default(),
        )
        .await
        .expect_err("no client");
    assert_eq!(err.code(), "capability_unavailable");
    assert!(
        !adapter
            .capabilities()
            .get("analyzeSetup")
            .unwrap()
            .available
    );
}

#[tokio::test]
async fn analysis_needs_approved_evidence_and_rejects_replies_the_validator_refuses() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;

    // 1. Evidence not approved: Setup refuses before the model is called.
    let never = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        reply: Err("must not be called".into()),
    });
    let adapter = env.adapter_with(|d| d.with_model(never.clone()));
    let session = started(&env, &adapter, &fx).await;
    let out = adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let evidence = evidence_from(&out);
    let err = adapter
        .analyze_setup(
            alice,
            fx.project,
            session.session_id,
            analysis_input(out.scan_id, evidence.clone()),
            source_resolution(),
            OpContext::default(),
        )
        .await
        .expect_err("evidence not approved");
    assert_eq!(err.code(), "evidence_not_approved", "{err}");
    assert_eq!(
        never.calls.load(Ordering::SeqCst),
        0,
        "the model was called before the evidence gate"
    );

    // Evidence that Setup did not scan itself is refused as well.
    let mut forged = evidence.clone();
    forged[0].excerpt = "something the scanner never saw".into();
    forged[0].excerpt_sha256 = sha256_bytes(forged[0].excerpt.as_bytes());
    let err = adapter
        .confirm_setup_evidence(
            alice,
            fx.project,
            session.session_id,
            out.scan_id,
            forged,
            OpContext::default(),
        )
        .await
        .expect_err("forged evidence");
    assert_eq!(err.code(), "evidence_not_approved", "{err}");

    // 2. With approved evidence, a failing provider and a garbage reply are both errors that
    //    retain nothing.
    adapter
        .confirm_setup_evidence(
            alice,
            fx.project,
            session.session_id,
            out.scan_id,
            evidence.clone(),
            OpContext::default(),
        )
        .await
        .expect("genuine evidence is approved");
    assert_eq!(
        env.count(&format!(
            "SELECT count(*) FROM setup_scans WHERE evidence_count = 1 AND id = '{}'",
            out.scan_id
        ))
        .await,
        1
    );

    let failing = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        reply: Err("provider down".into()),
    });
    let adapter = env.adapter_with(|d| d.with_model(failing.clone()));
    // A new adapter has no sessions: open one and repeat the gates on the same project.
    let session = started(&env, &adapter, &fx).await;
    let out = adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let evidence = evidence_from(&out);
    adapter
        .confirm_setup_evidence(
            alice,
            fx.project,
            session.session_id,
            out.scan_id,
            evidence.clone(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let err = adapter
        .analyze_setup(
            alice,
            fx.project,
            session.session_id,
            analysis_input(out.scan_id, evidence.clone()),
            source_resolution(),
            OpContext::default(),
        )
        .await
        .expect_err("provider failure");
    assert_eq!(err.code(), "analysis_model_failed");
    assert_eq!(failing.calls.load(Ordering::SeqCst), 1);

    let garbage = Arc::new(Scripted {
        calls: AtomicUsize::new(0),
        reply: Ok(json!({"nonsense": true})),
    });
    let adapter = env.adapter_with(|d| d.with_model(garbage.clone()));
    let session = started(&env, &adapter, &fx).await;
    let out = adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let evidence = evidence_from(&out);
    adapter
        .confirm_setup_evidence(
            alice,
            fx.project,
            session.session_id,
            out.scan_id,
            evidence.clone(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let err = adapter
        .analyze_setup(
            alice,
            fx.project,
            session.session_id,
            analysis_input(out.scan_id, evidence),
            source_resolution(),
            OpContext::default(),
        )
        .await
        .expect_err("validator rejects");
    assert_eq!(err.code(), "analysis_model_failed");
    assert!(err.to_string().contains("rejected"), "{err}");
    assert!(garbage.calls.load(Ordering::SeqCst) <= 2);

    // Nothing was retained or confirmed, and the failures are on record.
    assert_eq!(env.count("SELECT count(*) FROM setup_analyses").await, 0);
    assert!(
        env.count("SELECT count(*) FROM setup_receipts WHERE operation = 'analyze_setup' AND status = 'failed'").await >= 2
    );
    // A plan cannot be built from it either.
    let adapter = env.adapter_with(|d| {
        d.with_plan_builder(Arc::new(FixtureBuilder {
            source: source_resolution().identity.clone(),
        }))
    });
    let session = started(&env, &adapter, &fx).await;
    let err = adapter
        .plan_setup(
            alice,
            fx.project,
            session.session_id,
            json!({"identity": {"projectRoot": fx.source.display().to_string()}}),
            OpContext::default(),
        )
        .await
        .expect_err("no confirmed analysis");
    assert_eq!(err.code(), "analysis_not_confirmed", "{err}");
}

#[tokio::test]
async fn private_projects_and_pending_deletion_projects_are_not_reachable() {
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    let adapter = env.adapter();
    env.database
        .scoped(alice)
        .mark_for_deletion(fx.project)
        .await
        .unwrap();
    let err = adapter
        .open_session(alice, fx.project, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "project_pending_deletion");
    let _ = Visibility::Private;
}
