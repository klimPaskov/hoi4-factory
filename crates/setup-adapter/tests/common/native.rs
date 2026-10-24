//! Native test doubles.
//!
//! `FixtureBuilder` is a TEST-ONLY `PlanBuilder`: it produces one generated file. Setup's own
//! default builder is not available at the pinned revision, and these tests use the double only to
//! exercise the adapter's gates, receipts, leases and recovery. It is never installed outside
//! tests, and no test treats it as evidence that real plan building works.
//!
//! `native_plan` builds an `InstallationPlan` with a structurally valid confirmed-analysis
//! record so that `hoi4_mod_setup::transaction::run_transaction` can be driven directly to leave a
//! REAL interrupted journal (through its public fault options) for the recovery tests. That
//! record is a structural stand-in. It is never given to the facade, and it is not a model reply.

use std::collections::HashMap;
use std::path::Path;

use hoi4_mod_setup::AppError;
use hoi4_mod_setup::ai;
use hoi4_mod_setup::codex::REQUIRED_ANALYSIS_PROPOSAL_KEYS;
use hoi4_mod_setup::models::*;
use hoi4_mod_setup::security::sha256_bytes;
use hoi4_mod_setup::service::{BuiltPlan, PlanBuilder, PlanRequest};
use hoi4_mod_setup::source::parse_manifest;
use serde_json::Value;
use uuid::Uuid;

pub const LOCAL_ENDPOINT: &str = "http://127.0.0.1:11434/v1/chat/completions";
/// Wiki pages the manifest requires, read from the copied manifest.
pub fn manifest_wiki_pages() -> Vec<String> {
    parse_manifest(
        include_bytes!("../fixtures/source/hoi4-mod-setup.manifest.json"),
        None,
    )
    .map(|m| m.wiki.required_pages)
    .unwrap_or_default()
}

/// Setup's readiness check wants the wiki snapshot beside the project; the real planner would
/// install it. The fixture project gets small stand-in pages.
pub fn add_wiki_pages(project: &Path) {
    let root = project.join("paradox_wiki");
    for page in manifest_wiki_pages() {
        let path = root.join(page.replace('/', std::path::MAIN_SEPARATOR_STR));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            "# Test wiki page
",
        )
        .unwrap();
    }
    // Readiness also wants the conventional thumbnail named in the descriptor.
    let descriptor = project.join("descriptor.mod");
    let mut text = std::fs::read(&descriptor).unwrap();
    if !String::from_utf8_lossy(&text).contains("picture=") {
        text.extend_from_slice(b"\npicture=\"thumbnail.png\"\n");
        std::fs::write(&descriptor, text).unwrap();
    }
}

pub const GENERATED: &[u8] = b"# Agent guidance\n";

/// A confirmed record that passes `validate_confirmed_record`. Structural only.
pub fn structural_record(source: &SourceIdentity) -> CodexAnalysisRecord {
    CodexAnalysisRecord {
        engine: "provider_api".into(),
        auth_mode: "local_endpoint".into(),
        provider: Some("local".into()),
        model: Some("fixture".into()),
        reasoning_effort: Some("high".into()),
        optimization_profile: Some(ai::profile("local").unwrap().optimization_profile),
        analysis_id: Uuid::new_v4(),
        schema_version: "1.0.0".into(),
        input_sha256: sha256_bytes(b"input"),
        output_sha256: sha256_bytes(b"output"),
        confirmed_fields: REQUIRED_ANALYSIS_PROPOSAL_KEYS
            .iter()
            .map(|k| (*k).to_owned())
            .collect(),
        confirmed_at: "2026-10-24T00:00:00Z".into(),
        account_identity_persisted: false,
        analysis_purpose: None,
        project_root: None,
        scan_id: None,
        evidence_sha256: None,
        source_revision: Some(source.resolved_revision.clone()),
        source_manifest_sha256: Some(source.manifest_sha256.clone()),
    }
}

/// An unapproved plan with `files` generated files (`GENERATED-<n>.md`), bound to `identity`.
pub fn native_plan(
    source: &SourceIdentity,
    record: CodexAnalysisRecord,
    root_identity: &str,
    state_root: &Path,
    files: usize,
) -> (InstallationPlan, Vec<PreparedFile>) {
    let mut operations = Vec::new();
    let mut prepared = Vec::new();
    for n in 0..files {
        let destination = format!("GENERATED-{n}.md");
        let bytes = format!("# Agent guidance {n}\n").into_bytes();
        operations.push(PlanOperation {
            id: format!("op-{n}"),
            component_id: "core.agents".into(),
            ownership: Some(Ownership::Managed),
            location_scope: None,
            action: OperationAction::Create,
            source_path: Some("generated:test".into()),
            destination: destination.clone(),
            source_sha256: Some(sha256_bytes(&bytes)),
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
        });
        prepared.push(PreparedFile {
            operation_id: format!("op-{n}"),
            destination,
            expected_sha256: sha256_bytes(&bytes),
            bytes,
        });
    }
    let plan = InstallationPlan {
        schema_version: "1.0.0".into(),
        plan_id: Uuid::new_v4(),
        project_id: "example".into(),
        script_prefix: Some("example".into()),
        primary_namespace: Some("example".into()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        maintenance_mode: None,
        source: source.clone(),
        ai_provider: "local".into(),
        ai_model: "fixture".into(),
        ai_reasoning_effort: "high".into(),
        ai_endpoint: Some(LOCAL_ENDPOINT.into()),
        ai_optimization_profile: ai::profile("local").unwrap().optimization_profile,
        primary_coding_environment: "codex".into(),
        additional_coding_environments: vec![],
        flatten_chat_sources: false,
        codex_analysis: Some(record),
        selected_components: vec!["core.agents".into(), "wiki.snapshot".into()],
        wiki_required_pages: manifest_wiki_pages(),
        wiki_metadata: Some(WikiInstallMetadata {
            snapshot_marker: None,
            required_media_policy: "all_declared".into(),
            source_status: "verified_snapshot".into(),
            license_status: "not_verified".into(),
            repository_license_status: "unknown".into(),
            notes: vec![],
        }),
        generated_artifacts: vec![],
        download_ledger: vec![],
        git_setup: None,
        credential_references: vec![],
        optional_workflows: Default::default(),
        portrait_pipeline: None,
        image_generation: None,
        live_testing: None,
        operations,
        conflicts: vec![],
        external_actions: vec![],
        transaction: TransactionPlanInfo {
            stages: TRANSACTION_STAGES.iter().map(|s| (*s).into()).collect(),
            backup_root: state_root.join("backups").display().to_string(),
            staging_root: state_root.join("staging").display().to_string(),
            directories: Vec::new(),
            atomic_apply_expected: true,
            project_root_mode: ProjectRootMode::Existing,
            project_root_parent: None,
            project_root_leaf: None,
            project_root_identity: Some(root_identity.into()),
        },
        approvals: PlanApprovals {
            dry_run_reviewed: true,
            external_actions_reviewed: true,
            git_remote_approved: false,
            push_approved: false,
        },
    };
    (plan, prepared)
}

/// Test-only plan builder (see the module comment).
pub struct FixtureBuilder {
    pub source: SourceIdentity,
}

impl PlanBuilder for FixtureBuilder {
    fn build(&self, request: &PlanRequest<'_>) -> Result<BuiltPlan, AppError> {
        let (mut plan, prepared_files) = native_plan(
            &self.source,
            request.analysis.clone(),
            request.project_root_identity,
            request.state_root,
            1,
        );
        // The plan carries the provider the analysis was confirmed with.
        let record = request.analysis;
        if let Some(provider) = record.provider.clone() {
            plan.ai_provider = provider;
        }
        if let Some(model) = record.model.clone() {
            plan.ai_model = model;
        }
        if let Some(effort) = record.reasoning_effort.clone() {
            plan.ai_reasoning_effort = effort;
        }
        if let Some(profile) = record.optimization_profile.clone() {
            plan.ai_optimization_profile = profile;
        }
        plan.ai_endpoint = request
            .state
            .get("aiEndpoint")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or(plan.ai_endpoint);
        plan.approvals = PlanApprovals {
            dry_run_reviewed: false,
            external_actions_reviewed: false,
            git_remote_approved: false,
            push_approved: false,
        };
        Ok(BuiltPlan {
            plan,
            prepared_files,
            merge_contexts: HashMap::new(),
        })
    }
}

/// A builder that fails the test if the facade ever reaches it. Proves a gate ran first.
pub struct MustNotBeCalled;

impl PlanBuilder for MustNotBeCalled {
    fn build(&self, _: &PlanRequest<'_>) -> Result<BuiltPlan, AppError> {
        panic!("the plan builder was reached although a gate should have refused first");
    }
}
