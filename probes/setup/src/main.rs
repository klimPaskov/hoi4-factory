//! EXP-01 binding probe for HOI4 Factory S01.
//!
//! Embeds HOI4-Mod-Setup's library crate (`hoi4-mod-setup`, desktop feature
//! off) and its portable core directly, then exercises scan, plan, apply and
//! recovery on server workspace paths. No model is called. The probe records
//! that provider analysis is required instead of asserting a confirmation.
//!
//! Usage: exp01-setup-probe <workspace_root> <project_rel> <state_root> [<interrupted_app_rel>]

use hoi4_mod_setup::models::*;
use hoi4_mod_setup::scanner::{scan_project, ScanOptions};
use hoi4_mod_setup::transaction::{
    find_incomplete_transaction, read_journal, recovery_action, rollback_transaction, run_transaction,
    validate_plan, TransactionOptions,
};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

/// The factory-side confinement gate. Setup has no notion of a workspace root,
/// so the server adapter must refuse anything outside it before calling Setup.
fn resolve_in_workspace(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    if rel.is_absolute() || relative.contains('\\') {
        return Err("absolute or backslash path refused".into());
    }
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("traversal or non-normal component refused".into());
    }
    let workspace = workspace
        .canonicalize()
        .map_err(|e| format!("workspace root unavailable: {e}"))?;
    let mut current = workspace.clone();
    for component in rel.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("symlink component refused: {}", current.display()))
            }
            Ok(_) => {}
            Err(_) => {} // a not-yet-existing leaf is allowed (create route)
        }
    }
    let probe = current.canonicalize().unwrap_or(current.clone());
    if !probe.starts_with(&workspace) {
        return Err("resolved path is outside the workspace root".into());
    }
    Ok(current)
}

fn candidate_plan_without_analysis() -> (InstallationPlan, Vec<PreparedFile>) {
    let bytes = b"EXP01 probe managed file\n".to_vec();
    let sha = hoi4_setup_core::text::sha256_bytes(&bytes);
    let plan = InstallationPlan {
        schema_version: "1.0.0".into(),
        plan_id: uuid::Uuid::new_v4(),
        project_id: "exp01_synthetic_import".into(),
        script_prefix: Some("EXP01".into()),
        primary_namespace: Some("exp01".into()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        maintenance_mode: None,
        source: SourceIdentity {
            repository: format!(
                "{}/{}",
                hoi4_setup_core::manifest::SOURCE_OWNER,
                hoi4_setup_core::manifest::SOURCE_NAME
            ),
            mode: SourceMode::PinnedCommit,
            resolved_revision: "599497ea2f93612d9094461c6fde114fc87a5c0f".into(),
            requested_ref: None,
            release: None,
            manifest_sha256: "a".repeat(64),
            manifest_origin: "remote".into(),
        },
        ai_provider: "codex".into(),
        ai_model: "gpt-5.6-luna".into(),
        ai_reasoning_effort: "xhigh".into(),
        ai_endpoint: None,
        ai_optimization_profile: default_ai_optimization_profile(),
        primary_coding_environment: "codex".into(),
        additional_coding_environments: vec![],
        flatten_chat_sources: false,
        // No analysis record: the factory has not run or confirmed one.
        codex_analysis: None,
        selected_components: vec!["core.agents".into()],
        wiki_required_pages: vec![],
        wiki_metadata: None,
        generated_artifacts: vec![],
        download_ledger: vec![],
        git_setup: None,
        credential_references: vec![],
        optional_workflows: Default::default(),
        portrait_pipeline: None,
        image_generation: None,
        live_testing: None,
        operations: vec![PlanOperation {
            id: "probe-op-1".into(),
            component_id: "core.agents".into(),
            ownership: Some(Ownership::Managed),
            location_scope: None,
            action: OperationAction::Create,
            source_path: Some("generated:exp01-probe".into()),
            destination: "exp01_setup/managed/probe.md".into(),
            source_sha256: Some(sha.clone()),
            source_size: Some(bytes.len() as u64),
            platform: Some(ManifestPlatform::All),
            executable: false,
            result_sha256: None,
            base_sha256: None,
            local_sha256: None,
            local_state: LocalState::Absent,
            resolution: None,
            external: false,
            rollback: RollbackAction::RemoveCreated,
            external_parent_identity: None,
        }],
        conflicts: vec![],
        external_actions: vec![],
        transaction: TransactionPlanInfo {
            stages: TRANSACTION_STAGES.iter().map(|s| (*s).into()).collect(),
            backup_root: "external".into(),
            staging_root: "external".into(),
            directories: vec![],
            atomic_apply_expected: true,
            project_root_mode: ProjectRootMode::Existing,
            project_root_parent: None,
            project_root_leaf: None,
            project_root_identity: None,
        },
        approvals: PlanApprovals {
            dry_run_reviewed: true,
            external_actions_reviewed: true,
            git_remote_approved: false,
            push_approved: false,
        },
    };
    let prepared = vec![PreparedFile {
        operation_id: "probe-op-1".into(),
        destination: "exp01_setup/managed/probe.md".into(),
        bytes,
        expected_sha256: sha,
    }];
    (plan, prepared)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let workspace = PathBuf::from(&args[1]);
    let project_rel = &args[2];
    let state_root = PathBuf::from(&args[3]);
    let interrupted = args.get(4).cloned();
    let mut report = json!({ "probe": "exp01-setup-probe", "workspace_root": workspace });

    // 1. Confinement gate.
    let mut refusals = vec![];
    for candidate in ["../outside", "/etc", "acct-a/../../etc", "acct-a/escape-link", "acct-a/escape-link/passwd"] {
        let outcome = resolve_in_workspace(&workspace, candidate);
        refusals.push(json!({ "input": candidate, "refused": outcome.is_err(), "detail": outcome.err() }));
    }
    report["confinement"] = json!(refusals);
    let root = match resolve_in_workspace(&workspace, project_rel) {
        Ok(root) => root,
        Err(error) => {
            report["fatal"] = json!(error);
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            std::process::exit(2);
        }
    };
    report["project_root"] = json!(root);

    // 2. Read-only scan through the desktop scanner service, embedded as-is.
    match scan_project(&root, &ScanOptions::default()) {
        Ok(scan) => {
            report["scan"] = json!({
                "ok": true,
                "scan_id": scan.scan_id.to_string(),
                "read_only": scan.read_only,
                "partial": scan.partial,
                "cancelled": scan.cancelled,
                "files_scanned": scan.files_scanned,
                "directories_scanned": scan.directories_scanned,
                "bytes_read": scan.bytes_read,
                "findings": scan.findings.len(),
                "conflicts": scan.conflicts.iter().map(|c| serde_json::to_value(c).unwrap_or(Value::Null)).collect::<Vec<_>>(),
                "limits_hit": scan.limits_hit,
                "semantic_analysis": serde_json::to_value(&scan.semantic_analysis).unwrap_or(Value::Null),
                "detected": hoi4_setup_core::proposals::detect(&scan.findings).iter().map(|d| serde_json::to_value(d).unwrap_or(Value::Null)).collect::<Vec<_>>(),
            });
        }
        Err(error) => report["scan"] = json!({ "ok": false, "error": error.to_string() }),
    }

    // 3. Plan. The real plan builder (`build_plan`) lives in the desktop-only
    // `commands` module and is not reachable without Tauri. A candidate plan
    // without a confirmed analysis must be refused by the transaction layer.
    let (plan, prepared) = candidate_plan_without_analysis();
    report["plan"] = json!({
        "desktop_build_plan_reachable": false,
        "reason": "hoi4_mod_setup::commands is #[cfg(feature = \"desktop\")] and private; build_plan, approve_scan_evidence, confirm_codex_analysis and the prepared-plan store live there",
        "validate_plan_without_analysis": match validate_plan(&plan) { Ok(()) => json!({"ok": true}), Err(e) => json!({"refused": e.to_string()}) },
    });

    // 4. Apply attempt. Must refuse before any write.
    let app_root = state_root.join("setup-app");
    let applied = run_transaction(
        &root,
        &plan,
        &prepared,
        &TransactionOptions { app_data_root: Some(app_root.clone()), ..Default::default() },
    );
    report["apply_without_analysis"] = match applied {
        Ok((journal, _)) => json!({ "refused": false, "state": journal.state }),
        Err(error) => json!({ "refused": true, "error": error.to_string(), "app_root_created": app_root.exists() }),
    };

    // 5. Recovery inspection on a fresh server state root, then on an
    // interrupted transaction left by the harness, with no provider at all.
    report["recovery_fresh"] = match find_incomplete_transaction(&app_root, &root) {
        Ok(found) => json!({ "ok": true, "incomplete": found.map(|j| j.transaction_id.to_string()) }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    };
    if let Some(interrupted_rel) = interrupted {
        let interrupted_app = match resolve_in_workspace(&state_root, &interrupted_rel) {
            Ok(path) => path,
            Err(error) => {
                report["recovery_interrupted"] = json!({ "refused": error });
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                return;
            }
        };
        report["recovery_interrupted"] = match find_incomplete_transaction(&interrupted_app, &root) {
            Ok(Some(journal)) => {
                let action = recovery_action(&journal);
                let journal_path = hoi4_mod_setup::paths::transaction_root(&interrupted_app, journal.transaction_id)
                    .transaction
                    .join("journal.json");
                let mut entry = json!({
                    "transaction_id": journal.transaction_id.to_string(),
                    "state": journal.state,
                    "recommended_action": action,
                    "resume_allowed": journal.recovery.resume_allowed,
                    "rollback_allowed": journal.recovery.rollback_allowed,
                    "project_apply_started": journal.recovery.project_apply_started,
                });
                if action == "rollback" {
                    let mut journal = read_journal(&journal_path).unwrap();
                    entry["rollback"] = match rollback_transaction(&root, &mut journal, &journal_path) {
                        Ok(()) => json!({ "ok": journal.state }),
                        Err(error) => json!({ "err": error.to_string() }),
                    };
                }
                entry
            }
            Ok(None) => json!({ "incomplete": null }),
            Err(error) => json!({ "error": error.to_string() }),
        };
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
