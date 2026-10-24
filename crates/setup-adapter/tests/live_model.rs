//! End to end with a REAL model reply. Ignored by default.
//!
//! Run with: `cargo test -p factory-setup-adapter --test live_model -- --ignored --nocapture`
//!
//! It calls the Anthropic Messages API (`claude-haiku-5-5`) through the factory-agent Anthropic
//! adapter, with the key read from the Windows user environment
//! (`[Environment]::GetEnvironmentVariable('ANTHROPIC_API_KEY','User')`, falling back to the
//! process environment elsewhere). The reply is whatever the model returns, validated by Setup's
//! own validator; nothing is scripted. The plan is produced by the TEST-ONLY `FixtureBuilder`
//! (no default plan builder exists at the pinned Setup revision), so this proves the adapter's
//! analyze, confirm, plan, approve, apply and receipt path, not Setup's real plan building.

mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::native::{FixtureBuilder, add_wiki_pages};
use common::*;
use factory_agent::anthropic::{AnthropicClient, assemble, request_body};
use factory_agent::openai::{ApiKey, Control};
use factory_setup_adapter::*;
use factory_workspace::SnapshotInventory;
use hoi4_mod_setup::codex::ApprovedEvidence;
use hoi4_mod_setup::credentials::{MemoryCredentialStore, save_ai_provider_key};
use hoi4_mod_setup::security::sha256_bytes;
use serde_json::{Value, json};

const MODEL: &str = "claude-haiku-5-5";
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

fn api_key() -> String {
    let from_user_env = if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Environment]::GetEnvironmentVariable('ANTHROPIC_API_KEY','User')",
            ])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    } else {
        std::env::var("ANTHROPIC_API_KEY").ok()
    };
    from_user_env
        .filter(|k| !k.is_empty())
        .expect("ANTHROPIC_API_KEY is not set for this user")
}

/// The server's `ModelClient`, here for Anthropic.
struct Anthropic {
    key: String,
}

#[async_trait]
impl ModelClient for Anthropic {
    async fn complete(&self, request: ModelRequest) -> Result<ModelReply, ModelError> {
        let client = AnthropicClient::new(
            "https://api.anthropic.com",
            ApiKey::new(self.key.clone()),
            Duration::from_secs(180),
        )
        .map_err(|e| ModelError::Failed(e.to_string()))?;
        let body = request_body(
            MODEL,
            Some(&request.system_prompt),
            vec![json!({"role": "user", "content": request.prompt})],
            &[],
            8000,
            Some(
                json!({"output_config": {"format": {"type": "json_schema", "schema": strip_schema_keywords(&request.output_schema, ANTHROPIC_UNSUPPORTED_KEYWORDS)}}}),
            ),
        );
        if let Ok(dir) = std::env::var("FACTORY_LIVE_DEBUG_DIR") {
            let _ = std::fs::write(format!("{dir}/body.json"), body.to_string());
        }
        let (source, _meta) = client
            .start(&body, None)
            .await
            .map_err(|e| ModelError::Failed(e.to_string()))?;
        let turn = assemble(source, |_| Control::Continue)
            .await
            .map_err(|e| ModelError::Failed(e.to_string()))?;
        // The client's job is to extract the JSON object from the text. Setup's validator then
        // decides whether its content is acceptable.
        if let Ok(dir) = std::env::var("FACTORY_LIVE_DEBUG_DIR") {
            // Debug aid for a rejected reply: the prompt and the raw text, never the key.
            let _ = std::fs::write(format!("{dir}/prompt.txt"), &request.prompt);
            let _ = std::fs::write(format!("{dir}/reply.txt"), &turn.text);
        }
        let reply = json_object_from_text(&turn.text).or_else(|first| {
            match (turn.text.find('{'), turn.text.rfind('}')) {
                (Some(a), Some(b)) if a < b => json_object_from_text(&turn.text[a..=b]),
                _ => Err(first),
            }
        })?;
        Ok(ModelReply {
            reply,
            response_id: turn.response_id,
            usage: turn
                .usage
                .map(|u| json!({"input_tokens": u.input_tokens, "output_tokens": u.output_tokens})),
        })
    }
}

fn snapshot(env: &Env, fx: &Fixture) -> SnapshotInventory {
    let ws = env
        .storage
        .open_workspace(
            &factory_workspace::AccountId::new(fx.account.to_string()).unwrap(),
            &factory_workspace::ProjectId::new(fx.project.to_string()).unwrap(),
        )
        .unwrap();
    SnapshotInventory::capture(&fx.source, ws.git().unwrap().as_ref()).unwrap()
}

#[tokio::test]
#[ignore = "calls the Anthropic API with the user's key"]
async fn real_analysis_then_plan_approve_apply_with_receipts() {
    let key = api_key();
    let env = Env::new().await;
    let alice = env.account("alice").await;
    let fx = env.project_with_import(alice).await;
    add_wiki_pages(&fx.source);

    let source = source_resolution();
    let credentials = Arc::new(MemoryCredentialStore::default());
    let reference = save_ai_provider_key(credentials.as_ref(), "claude", &key).unwrap();
    let adapter = env.adapter_with(|d| {
        d.with_credentials(credentials.clone())
            .with_model(Arc::new(Anthropic { key: key.clone() }))
            .with_plan_builder(Arc::new(FixtureBuilder {
                source: source.identity.clone(),
            }))
    });
    let session = adapter.open_session(alice, fx.project, None).await.unwrap();
    let before = snapshot(&env, &fx);

    let scan = adapter
        .inspect_project(
            alice,
            fx.project,
            session.session_id,
            InspectOptions::default(),
            OpContext::default(),
        )
        .await
        .unwrap();
    let finding = scan
        .scan
        .findings
        .iter()
        .filter(|f| !f.evidence.is_empty() && f.model_excerpt.is_some())
        .max_by_key(|f| f.model_excerpt.as_deref().map_or(0, str::len))
        .expect("a finding with evidence");
    let excerpt = finding.model_excerpt.clone().unwrap();
    let evidence = vec![ApprovedEvidence {
        reference: finding.id.clone(),
        path: finding.evidence[0].path.clone(),
        excerpt_sha256: sha256_bytes(excerpt.as_bytes()),
        excerpt,
        confidence: None,
    }];
    adapter
        .confirm_setup_evidence(
            alice,
            fx.project,
            session.session_id,
            scan.scan_id,
            evidence.clone(),
            OpContext::default(),
        )
        .await
        .unwrap();

    let analysis = adapter
        .analyze_setup(
            alice,
            fx.project,
            session.session_id,
            AnalysisInput {
                provider: "claude".into(),
                model: MODEL.into(),
                reasoning_effort: "low".into(),
                endpoint: ENDPOINT.into(),
                credential: Some(reference),
                mode: "existing_project_semantics".into(),
                brief: String::new(),
                evidence,
                constraints: json!({}),
                purpose: Some("existing_project_import".into()),
                scan_id: Some(scan.scan_id),
            },
            source.clone(),
            OpContext::default(),
        )
        .await
        .expect("a real model reply passes Setup's validator");
    println!("analysis attempts: {}", analysis.attempts);
    let receipts = adapter.list_receipts(alice, fx.project, 20).await.unwrap();
    let analyze_receipt = receipts
        .iter()
        .find(|r| r.operation == "analyze_setup")
        .unwrap();
    println!(
        "analysis usage: {:?}",
        analyze_receipt.result.as_ref().and_then(|r| r.get("usage"))
    );
    assert_eq!(analyze_receipt.status, "succeeded");
    assert_eq!(analysis.result.record.model.as_deref(), Some(MODEL));

    // Confirm exactly the values the reviewer saw.
    let mut state = json!({
        "aiProvider": "claude",
        "aiModel": MODEL,
        "aiReasoningEffort": "low",
        "aiEndpoint": ENDPOINT,
        "description": "A synthetic import fixture.",
        "folderProfile": ["common", "localisation/english"],
        "identity": {
            "displayName": "EXP01 Synthetic Import Fixture",
            "projectId": "exp01_synthetic",
            "author": "Fixture",
            "version": "0.3.1",
            "supportedGameVersion": "1.17.*",
            "projectRoot": fx.source.display().to_string(),
            "defaultBranch": "main",
            "scriptPrefix": "exp",
            "primaryNamespace": "exp",
            "descriptorTags": ["Alternative History"],
        },
        "conventions": {"agents_profile": "default", "localisation_convention": "english", "documentation_convention": "markdown"},
        "scanContext": {"scanId": scan.scan_id},
    });
    state["codexAnalysis"] = serde_json::to_value(&analysis.result.analysis).unwrap();
    let fields: Vec<String> = analysis
        .result
        .analysis
        .proposals
        .iter()
        .map(|p| {
            serde_json::to_value(&p.key)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let record = adapter
        .confirm_setup_analysis(
            alice,
            fx.project,
            session.session_id,
            analysis.result.record.clone(),
            fields,
            &state,
            OpContext::default(),
        )
        .await
        .expect("confirmation");
    state["codexAnalysisRecord"] = serde_json::to_value(&record).unwrap();

    let plan = adapter
        .plan_setup(
            alice,
            fx.project,
            session.session_id,
            state,
            OpContext::default(),
        )
        .await
        .expect("plan from the test-only builder");
    // Approval needs the hash that was reviewed.
    let stale = adapter
        .approve_setup_plan(
            alice,
            fx.project,
            session.session_id,
            plan.plan_id,
            &"0".repeat(64),
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(stale.code(), "plan_changed");
    let approval = adapter
        .approve_setup_plan(
            alice,
            fx.project,
            session.session_id,
            plan.plan_id,
            &plan.plan_sha256,
            OpContext::default().with_grant("grant-1", 7),
        )
        .await
        .unwrap();
    assert_eq!(approval.reviewed_plan_sha256, plan.plan_sha256);
    assert_eq!(approval.root_identity, plan.root_identity);

    // Two owners race to apply: exactly one wins.
    let (a, b) = (adapter.clone(), adapter.clone());
    let sid = session.session_id;
    let pid = plan.plan_id;
    let project = fx.project;
    let t1 = tokio::spawn(async move {
        a.apply_setup_plan(alice, project, sid, pid, OpContext::default())
            .await
    });
    let t2 = tokio::spawn(async move {
        b.apply_setup_plan(alice, project, sid, pid, OpContext::default())
            .await
    });
    let (r1, r2) = (t1.await.unwrap(), t2.await.unwrap());
    let (ok, err) = match (r1, r2) {
        (Ok(o), Err(e)) | (Err(e), Ok(o)) => (o, e),
        (Ok(_), Ok(_)) => panic!("both applies succeeded"),
        (Err(a), Err(b)) => panic!("neither apply succeeded: {a} / {b}"),
    };
    assert!(
        ["project_busy", "plan_not_approved", "idempotency_replay"].contains(&err.code()),
        "the loser must be a refusal, not a second effect: {err}"
    );
    assert_eq!(ok.journal_state, "completed");
    assert_eq!(ok.setup.revision, SETUP_PIN.revision);
    assert!(!ok.setup.contains_pr_105_fix);

    // Preservation: nothing that existed changed except the descriptor edit we made before
    // `before`... which was captured after it, so nothing at all may differ except additions.
    let after = snapshot(&env, &fx);
    let diff = before.diff(&after);
    println!("apply changes: {diff:?}");
    assert!(
        diff.changed.is_empty() && diff.removed.is_empty() && diff.mode_changed.is_empty(),
        "{diff:?}"
    );
    assert!(diff.added.iter().any(|p| p == "GENERATED-0.md"));
    assert!(
        ok.unplanned_changes.is_empty(),
        "{:?}",
        ok.unplanned_changes
    );

    // Receipts for every step, a single transaction, nothing left unsettled.
    let receipts = adapter.list_receipts(alice, fx.project, 50).await.unwrap();
    for op in [
        "inspect_project",
        "confirm_setup_evidence",
        "analyze_setup",
        "confirm_setup_analysis",
        "plan_setup",
        "approve_setup_plan",
        "apply_setup_plan",
    ] {
        assert!(
            receipts
                .iter()
                .any(|r| r.operation == op && r.status == "succeeded"),
            "no succeeded receipt for {op}"
        );
    }
    assert_eq!(
        env.count("SELECT count(*) FROM setup_transactions WHERE state = 'completed'")
            .await,
        1
    );
    assert_eq!(
        env.count("SELECT count(*) FROM setup_transactions").await,
        1
    );
    assert_eq!(
        env.count("SELECT count(*) FROM setup_approvals WHERE state = 'consumed'")
            .await,
        1
    );
    assert_eq!(
        env.count("SELECT count(*) FROM setup_plans WHERE state = 'applied'")
            .await,
        1
    );
    assert!(
        adapter
            .unsettled_receipts(alice, fx.project)
            .await
            .unwrap()
            .is_empty()
    );
    let apply_receipt = receipts
        .iter()
        .find(|r| r.operation == "apply_setup_plan" && r.status == "succeeded")
        .unwrap();
    assert_eq!(apply_receipt.grant_id, None); // the apply call carried no grant; the approval did
    assert_eq!(
        apply_receipt.idempotency_key.as_deref(),
        Some(format!("apply:{}", plan.plan_id).as_str())
    );
    assert_eq!(
        apply_receipt.before_inventory_sha256.as_deref(),
        Some(before.content_hash().as_str())
    );
    let approval_grant: Option<String> = sqlx::query_scalar("SELECT grant_id FROM setup_approvals")
        .fetch_one(&env.db.admin)
        .await
        .unwrap();
    assert_eq!(approval_grant.as_deref(), Some("grant-1"));

    // The same plan cannot be applied again.
    let again = adapter
        .apply_setup_plan(
            alice,
            fx.project,
            session.session_id,
            plan.plan_id,
            OpContext::default(),
        )
        .await
        .unwrap_err();
    assert!(
        ["plan_not_approved", "idempotency_replay"].contains(&again.code()),
        "{again}"
    );

    // Handoff.
    let handoff = adapter
        .open_workspace(alice, fx.project, OpenWorkspaceOptions::default())
        .await
        .unwrap();
    assert!(handoff.ready, "{:?}", handoff.blockers);
    let descriptor = handoff.descriptor.unwrap();
    let receipt = descriptor
        .setup_receipt
        .as_ref()
        .expect("setup receipt in C-01");
    assert_eq!(receipt.transaction_state, "completed");
    assert!(
        receipt
            .generated_files
            .iter()
            .any(|f| f.path == "GENERATED-0.md")
    );
    let _: Value = serde_json::to_value(&descriptor).unwrap();
}
