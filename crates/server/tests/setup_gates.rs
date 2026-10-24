//! Stage S02 gates G1 to G4 and account isolation for Setup, through the assembled server over
//! HTTP, against a real PostgreSQL (`FACTORY_TEST_ADMIN_URL`, see `http_api.rs`).
//!
//! What is real here: the server, the Setup facade at the pinned revision, Setup's
//! `DefaultPlanBuilder` fetching and verifying the real workflow package (network on first run;
//! files are cached), Setup's transaction engine, Git and the file system. GitHub is the intake
//! crate's local double.
//!
//! The analysis step needs a model reply. Tests marked `#[ignore]` call the Anthropic API with
//! the key from the user environment and record the reply (`FACTORY_RECORD_REPLIES=1`). The other
//! tests replay such a recording: the reply is the provider's bytes, with only `input_sha256`
//! rebound to the current request (see `support/setup_support.rs`). No reply is ever written by
//! hand.
#![allow(dead_code, private_interfaces)]

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");
include!("support/setup_support.rs");

use std::process::Command;

// ----- the fixture mod ---------------------------------------------------------------------

const BOM: &[u8] = b"\xEF\xBB\xBF";

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn asset_bytes() -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut state = b"gate fixture asset".to_vec();
    // A PNG signature first, so the upload policy sees an image and not a binary of unknown kind.
    let mut out = PNG_SIGNATURE.to_vec();
    while out.len() < 4096 {
        state = Sha256::digest(&state).to_vec();
        out.extend_from_slice(&state);
    }
    out.truncate(4096);
    out
}

fn fixture_files() -> Vec<(&'static str, Vec<u8>, u32)> {
    let mut descriptor = BOM.to_vec();
    descriptor.extend_from_slice(
        b"name=\"Gate Fixture\"\r\nversion=\"0.3.1\"\r\nsupported_version=\"1.19.*\"\r\ntags={\r\n\t\"Alternative History\"\r\n}\r\n",
    );
    let mut loc = BOM.to_vec();
    loc.extend_from_slice(b"l_english:\r\n gate_focus_title:0 \"Gate focus\"\r\n");
    vec![
        ("descriptor.mod", descriptor, 0o644),
        (
            "common/national_focus/gate_focus.txt",
            b"focus_tree = {\r\n\tid = gate_tree\r\n\tfocus = {\r\n\t\tid = gate_focus\r\n\t}\r\n}\r\n".to_vec(),
            0o644,
        ),
        ("localisation/english/gate_l_english.yml", loc, 0o644),
        ("gfx/gate_asset.png", asset_bytes(), 0o644),
        (
            "AGENTS.md",
            b"# My own agent instructions\n\nKeep exactly these words.\n".to_vec(),
            0o644,
        ),
        (
            ".agents/skills/gate-custom/SKILL.md",
            b"---\nname: gate-custom\ndescription: a custom skill of the author\n---\nDo the custom thing.\n".to_vec(),
            0o644,
        ),
        ("README.md", b"# Gate Fixture\n\nMy readme.\n".to_vec(), 0o644),
        ("docs/author-notes.md", b"# Notes\n\nNot for the package.\n".to_vec(), 0o644),
        ("notes/idea.txt", "Ideas \u{2014} \u{00e9}\u{00e8} \u{4e2d}\u{6587}\n".as_bytes().to_vec(), 0o644),
    ]
}

fn fixture_zip() -> Vec<u8> {
    let entries: Vec<ZEntry> = fixture_files()
        .into_iter()
        .map(|(name, data, mode)| ZEntry::file(name, data).mode(0o100000 | mode))
        .collect();
    build_zip(&entries)
}

async fn upload_fixture(h: &Harness, user: &User, name: &str) -> String {
    let project = h.project(user, name).await;
    let resp = user
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::text("git_init_consent", "true"),
                Part::file("gate.zip", fixture_zip()),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    project
}

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let empty = dir.parent().unwrap().join("empty.gitconfig");
    let _ = std::fs::write(&empty, "");
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &empty)
        .env("GIT_AUTHOR_NAME", "gate")
        .env("GIT_AUTHOR_EMAIL", "gate@example.invalid")
        .env("GIT_COMMITTER_NAME", "gate")
        .env("GIT_COMMITTER_EMAIL", "gate@example.invalid")
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ----- shared steps -------------------------------------------------------------------------

/// A model for one test: the real provider (recording its replies) or a replay of a recording.
struct ModelSetup {
    client: Arc<dyn factory_setup_adapter::ModelClient>,
    recorder: Option<Arc<RecordingModel>>,
    replay: Option<Arc<ReplayModel>>,
}

fn model_for(label: &str, live: bool) -> ModelSetup {
    if live {
        let recorder = RecordingModel::new(&live_key(), LIVE_MODEL);
        ModelSetup {
            client: recorder.clone(),
            recorder: Some(recorder),
            replay: None,
        }
    } else {
        let replay = ReplayModel::load(label);
        ModelSetup {
            client: replay.clone(),
            recorder: None,
            replay: Some(replay),
        }
    }
}

impl ModelSetup {
    /// Prints the spend of a live run and, with `FACTORY_RECORD_REPLIES`, saves the replies.
    fn finish(&self, label: &str, note: &str) {
        if let Some(recorder) = &self.recorder {
            recorder.save(label, note);
        }
    }
}

async fn gate_harness(model: &ModelSetup, github: Option<GithubFixture>) -> Harness {
    Harness::start(Options {
        github,
        anthropic_key: Some("test-key-not-used"),
        model: Some(model.client.clone()),
        setup_source: Some(PinnedSource::new().await),
        ..Options::default()
    })
    .await
}

/// Decides every open conflict with `choice`; returns the plan as the browser would show it.
async fn settle_conflicts(flow: &Flow<'_>, plan: &Resp, choice: &str) -> Value {
    assert_eq!(plan.status, 200, "plan: {}", plan.text);
    let shown = &plan.body["plan"];
    let open: Vec<Value> = shown["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["selected"].is_null())
        .map(|c| json!({"path": c["path"], "choice": choice}))
        .collect();
    if open.is_empty() {
        return shown.clone();
    }
    let decided = flow
        .post(
            "conflicts",
            json!({"plan_id": shown["plan_id"], "decisions": open}),
        )
        .await;
    assert_eq!(decided.status, 200, "conflicts: {}", decided.text);
    assert_ne!(
        decided.body["plan"]["plan_sha256"], shown["plan_sha256"],
        "a decision changes the plan, so the hash"
    );
    assert_eq!(decided.body["plan"]["approved"], false);
    decided.body["plan"].clone()
}

/// Approves exactly this plan hash, then applies.
async fn approve_apply(flow: &Flow<'_>, plan: &Value) -> Resp {
    let approved = flow
        .post(
            "approve",
            json!({"plan_id": plan["plan_id"], "plan_sha256": plan["plan_sha256"]}),
        )
        .await;
    assert_eq!(approved.status, 200, "approve: {}", approved.text);
    assert_eq!(
        approved.body["approval"]["plan_sha256"],
        plan["plan_sha256"]
    );
    flow.post("apply", json!({"plan_id": plan["plan_id"]}))
        .await
}

fn sha_of(path: &std::path::Path) -> String {
    sha256_hex(&std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
}

fn print_pin(test: &str) {
    let pin = factory_setup_adapter::SETUP_PIN;
    println!(
        "PIN [{test}]: {} contains_pr_105_fix={} counts_for_recovery_gates={}",
        pin.short(),
        pin.contains_pr_105_fix,
        pin.counts_for_recovery_gates()
    );
}

/// The C-01 descriptor after a completed setup: Setup's receipt, tenant, roots, no host path.
async fn assert_c01(h: &Harness, user: &User, project: &str, applied: &Resp, created: &[&str]) {
    assert_eq!(applied.status, 200, "apply: {}", applied.text);
    assert_eq!(applied.body["status"], "applied", "{}", applied.text);
    assert_eq!(applied.body["journal_state"], "completed");
    assert_eq!(
        applied.body["unplanned_changes"],
        json!([]),
        "nothing changed that the plan did not name"
    );
    let ws = user
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.status, 200, "{}", ws.text);
    assert_eq!(ws.body["setup"]["ready"], true);
    assert_eq!(ws.body["summary"]["setup_state"], "completed");
    let descriptor = &ws.body["descriptor"];
    assert_eq!(descriptor["schema_version"], 1);
    assert_eq!(descriptor["workspace_id"], project);
    assert_eq!(descriptor["tenant"]["account_id"], user.id);
    assert_eq!(descriptor["tenant"]["project_id"], project);
    let receipt = &descriptor["setup_receipt"];
    assert_eq!(receipt["transaction_state"], "completed");
    assert_eq!(
        descriptor["component_closure"]["setup_revision"],
        factory_setup_adapter::SETUP_PIN.revision
    );
    let source = h.workspace_source(user, project);
    let generated = receipt["generated_files"].as_array().unwrap();
    for path in created {
        let entry = generated
            .iter()
            .find(|g| g["path"] == *path)
            .unwrap_or_else(|| panic!("{path} is a generated file of the receipt"));
        assert_eq!(
            entry["sha256"],
            sha_of(&source.join(path)),
            "{path}: the receipt names the bytes on disk ({:?})",
            String::from_utf8_lossy(&std::fs::read(source.join(path)).unwrap())
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
    let handles = descriptor["root_handles"].as_array().unwrap();
    assert!(handles.iter().any(
        |r| r["kind"] == "mod_source" && r["locator"] == format!("workspace:{project}/source")
    ));
    assert!(
        !ws.text
            .contains(&h.data_dir.path().to_string_lossy().into_owned()),
        "no host path in the descriptor"
    );
    // The workspace is the account's own directory.
    assert!(source.starts_with(h.data_dir.path().join("workspaces").join(&user.id)));
    // Receipts in PostgreSQL: one completed transaction, every step settled.
    let admin = h.admin().await;
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM setup_transactions WHERE state = 'completed' AND project_id = $1::uuid",
    )
    .bind(project)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(completed, 1);
    let unsettled: i64 =
        sqlx::query_scalar("SELECT count(*) FROM setup_receipts WHERE status = 'intent'")
            .fetch_one(&admin)
            .await
            .unwrap();
    assert_eq!(unsettled, 0, "every Setup step has a settled receipt");
}

// ----- G1: new, upload and GitHub import each reach a C-01 descriptor ------------------------

const NEW_MOD_BRIEF: &str = "A small alternate-history mod about a divided Europe: a handful of political events and two national focuses.";

async fn g1_upload(live: bool) {
    let model = model_for("upload-fixture", live);
    let h = gate_harness(&model, None).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = upload_fixture(&h, &alice, "Uploaded").await;
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze: {}", analysis.text);
    model.finish(
        "upload-fixture",
        "G1/G2/G4 fixture: the uploaded gate mod, existing-mod analysis",
    );
    assert_eq!(analysis.body["model"], LIVE_MODEL);
    let plan = flow.confirm_and_plan(&analysis).await;
    // The author's AGENTS.md, descriptor and README collide with the package: the person decides.
    let paths: Vec<&str> = plan.body["plan"]["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"AGENTS.md"), "{paths:?}");
    let settled = settle_conflicts(&flow, &plan, "keep").await;
    let applied = approve_apply(&flow, &settled).await;
    assert_c01(&h, &alice, &project, &applied, &["CLAUDE.md"]).await;
    // The author's instructions are what the author wrote.
    assert_eq!(
        std::fs::read(h.workspace_source(&alice, &project).join("AGENTS.md")).unwrap(),
        b"# My own agent instructions\n\nKeep exactly these words.\n"
    );
    print_pin("g1_upload");
}

async fn g1_new(live: bool) {
    let model = model_for("new-mod", live);
    let h = gate_harness(&model, None).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Brand new").await;
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let analysis = flow.scan_and_analyze("create", Some(NEW_MOD_BRIEF)).await;
    assert_eq!(analysis.status, 200, "analyze: {}", analysis.text);
    model.finish("new-mod", "G1: a new mod from a brief");
    let plan = flow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "plan: {}", plan.text);
    assert!(
        plan.body["plan"]["conflicts"]
            .as_array()
            .unwrap()
            .is_empty(),
        "an empty workspace has nothing to collide with"
    );
    let applied = approve_apply(&flow, &plan.body["plan"]).await;
    assert_c01(&h, &alice, &project, &applied, &["CLAUDE.md"]).await;
    print_pin("g1_new");
}

async fn g1_import(live: bool) {
    let model = model_for("github-import", live);
    let gh = github_fixture().await;
    let h = gate_harness(&model, Some(gh)).await;
    let alice = h.verified_user("alice@example.org").await;
    h.link_github(&alice, 424_242).await;
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "selected")];
    let project = h.project(&alice, "Imported").await;
    let imported = alice
        .client
        .post(
            &format!("/api/v1/projects/{project}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert_eq!(imported.status, 201, "{}", imported.text);
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze: {}", analysis.text);
    model.finish(
        "github-import",
        "G1: the GitHub-imported test mod, existing-mod analysis",
    );
    let plan = flow.confirm_and_plan(&analysis).await;
    let settled = settle_conflicts(&flow, &plan, "keep").await;
    let applied = approve_apply(&flow, &settled).await;
    assert_c01(&h, &alice, &project, &applied, &["CLAUDE.md"]).await;
    // Importing never pushed: the remote still has exactly its two branches.
    let heads = git(
        &h.github.as_ref().unwrap().remote.bare,
        &["for-each-ref", "refs/heads"],
    );
    assert_eq!(heads.lines().count(), 2, "{heads}");
    print_pin("g1_import");
}

#[tokio::test]
#[ignore = "live: calls the Anthropic API with the user's key"]
async fn g1_upload_live() {
    g1_upload(true).await
}

#[tokio::test]
#[ignore = "live: calls the Anthropic API with the user's key"]
async fn g1_new_live() {
    g1_new(true).await
}

#[tokio::test]
#[ignore = "live: calls the Anthropic API with the user's key"]
async fn g1_import_live() {
    g1_import(true).await
}

#[tokio::test]
async fn g1_upload_replayed() {
    g1_upload(false).await
}

#[tokio::test]
async fn g1_new_replayed() {
    g1_new(false).await
}

#[tokio::test]
async fn g1_import_replayed() {
    g1_import(false).await
}

// ----- inventories and unrelated Git state --------------------------------------------------

use factory_workspace::{SnapshotInventory, Workspace};

fn open_ws(h: &Harness, user: &User, project: &str) -> Workspace {
    h.state
        .storage
        .open_workspace(
            &factory_workspace::AccountId::new(user.id.clone()).unwrap(),
            &factory_workspace::ProjectId::new(project.to_owned()).unwrap(),
        )
        .unwrap()
}

fn inventory_of(h: &Harness, user: &User, project: &str) -> SnapshotInventory {
    let ws = open_ws(h, user, project);
    let repo = ws.git().unwrap();
    SnapshotInventory::capture(&ws.source_dir(), repo.as_ref()).unwrap()
}

/// A staged edit, an unstaged edit and an untracked file: the author's own work in progress.
fn dirty_the_repository(source: &std::path::Path) {
    let idea = source.join("notes/idea.txt");
    let mut bytes = std::fs::read(&idea).unwrap();
    bytes.extend_from_slice(b"staged addition\n");
    std::fs::write(&idea, bytes).unwrap();
    git(source, &["add", "notes/idea.txt"]);
    let notes = source.join("docs/author-notes.md");
    let mut bytes = std::fs::read(&notes).unwrap();
    bytes.extend_from_slice(b"unstaged addition\n");
    std::fs::write(&notes, bytes).unwrap();
    std::fs::write(source.join("gfx/scratch.tmp"), b"scratch bytes\n").unwrap();
}

fn status_lines(source: &std::path::Path) -> Vec<String> {
    git(source, &["status", "--porcelain=v1", "-uall"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Paths the plan creates, from the plan as shown.
fn created_paths(plan: &Value) -> Vec<String> {
    plan["operations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["action"] == "create")
        .map(|op| op["path"].as_str().unwrap().to_owned())
        .collect()
}

/// Decides each open conflict with `choose(path)`.
async fn settle_by(flow: &Flow<'_>, plan: &Resp, choose: impl Fn(&str) -> &'static str) -> Value {
    let shown = &plan.body["plan"];
    let open: Vec<Value> = shown["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["selected"].is_null())
        .map(|c| json!({"path": c["path"], "choice": choose(c["path"].as_str().unwrap())}))
        .collect();
    if open.is_empty() {
        return shown.clone();
    }
    let decided = flow
        .post(
            "conflicts",
            json!({"plan_id": shown["plan_id"], "decisions": open}),
        )
        .await;
    assert_eq!(decided.status, 200, "conflicts: {}", decided.text);
    decided.body["plan"].clone()
}

// ----- G2: preservation ---------------------------------------------------------------------

async fn g2_preserves(label: &str, replace_readme: bool, live: bool) {
    let model = model_for(label, live);
    let h = gate_harness(&model, None).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = upload_fixture(&h, &alice, "Preserved").await;
    let source = h.workspace_source(&alice, &project);
    dirty_the_repository(&source);
    let head_before = git(&source, &["rev-parse", "HEAD"]);
    let status_before = status_lines(&source);
    let originals: Vec<(String, Vec<u8>)> = fixture_files()
        .into_iter()
        .map(|(name, _, _)| (name.to_owned(), std::fs::read(source.join(name)).unwrap()))
        .collect();
    let before = inventory_of(&h, &alice, &project);

    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze: {}", analysis.text);
    let plan = flow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "plan: {}", plan.text);
    // Planning itself writes nothing.
    assert!(before.diff(&inventory_of(&h, &alice, &project)).is_empty());

    let covered = |path: &str| {
        if replace_readme && path == "README.md" {
            "replace"
        } else {
            "keep"
        }
    };
    let settled = settle_by(&flow, &plan, covered).await;
    let planned_new = created_paths(&settled);
    let applied = approve_apply(&flow, &settled).await;
    assert_eq!(applied.status, 200, "apply: {}", applied.text);
    assert_eq!(applied.body["status"], "applied", "{}", applied.text);
    assert_eq!(applied.body["unplanned_changes"], json!([]));

    let after = inventory_of(&h, &alice, &project);
    let diff = before.diff(&after);
    let allowed_changed: Vec<&str> = if replace_readme { vec!["README.md"] } else { vec![] };
    assert_eq!(
        diff.changed.iter().map(String::as_str).collect::<Vec<_>>(),
        allowed_changed,
        "only a change the reviewer chose may alter an existing file: {diff:?}"
    );
    assert!(diff.removed.is_empty() && diff.mode_changed.is_empty(), "{diff:?}");
    assert!(
        !diff.index_changed,
        "the staged state of the author's Git index is untouched"
    );
    for path in &diff.added {
        assert!(
            planned_new.contains(path) || path.starts_with(".hoi4-mod-setup/"),
            "{path} was added but is neither a planned file nor Setup's own state"
        );
    }
    // Byte for byte: protected native files, the custom asset, the author's instructions.
    for (name, bytes) in &originals {
        let now = std::fs::read(source.join(name)).unwrap();
        if replace_readme && name == "README.md" {
            assert_ne!(&now, bytes, "the reviewed replacement happened");
        } else {
            assert_eq!(&now, bytes, "{name} is byte-identical");
        }
    }
    // Unrelated Git state: same commit, same staged, unstaged and untracked entries.
    assert_eq!(git(&source, &["rev-parse", "HEAD"]), head_before);
    let status_after = status_lines(&source);
    for line in &status_before {
        if replace_readme && line.contains("README.md") {
            continue;
        }
        assert!(status_after.contains(line), "git status lost {line:?}");
    }
    assert!(git(&source, &["stash", "list"]).is_empty());
    print_pin(label);
    println!(
        "G2 [{label}]: added={} changed={:?} index_changed={}",
        diff.added.len(),
        diff.changed,
        diff.index_changed
    );
}

#[tokio::test]
async fn g2_preserves_everything_unless_a_reviewed_change_covers_it() {
    g2_preserves("upload-fixture", false, false).await
}

#[tokio::test]
async fn g2_a_reviewed_replacement_changes_exactly_that_file() {
    g2_preserves("upload-fixture", true, false).await
}

// ----- G3: re-running a completed setup -----------------------------------------------------

async fn g3_rerun(live: bool) {
    let label = "rerun-fixture";
    let model = model_for(label, live);
    let h = gate_harness(&model, None).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = upload_fixture(&h, &alice, "Rerun").await;
    let source = h.workspace_source(&alice, &project);
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };

    // First run.
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze 1: {}", analysis.text);
    model.finish(label, "G3: three analyses of one project, before and after setup");
    let plan = flow.confirm_and_plan(&analysis).await;
    let settled = settle_conflicts(&flow, &plan, "keep").await;
    let first = approve_apply(&flow, &settled).await;
    assert_eq!(first.body["status"], "applied", "{}", first.text);
    let after_first = inventory_of(&h, &alice, &project);

    // Second run, nothing changed in between.
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze 2: {}", analysis.text);
    model.finish(label, "G3: three analyses of one project, before and after setup");
    let plan = flow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "plan 2: {}", plan.text);
    assert_eq!(
        plan.body["plan"]["summary"]["create"], 0,
        "a completed setup has nothing left to create: {}",
        plan.text
    );
    let settled = settle_conflicts(&flow, &plan, "keep").await;
    let second = approve_apply(&flow, &settled).await;
    assert_eq!(second.body["status"], "applied", "{}", second.text);
    assert_eq!(second.body["unplanned_changes"], json!([]));
    let after_second = inventory_of(&h, &alice, &project);
    let diff = after_first.diff(&after_second);
    assert!(
        diff.added.is_empty() && diff.removed.is_empty() && diff.mode_changed.is_empty(),
        "re-running produced unexpected changes: {diff:?}"
    );
    for path in &diff.changed {
        assert!(
            path.starts_with(".hoi4-mod-setup/"),
            "{path} changed on a re-run"
        );
    }

    // A generated file the author edited is a conflict, and the author's bytes survive.
    let claude = source.join("CLAUDE.md");
    let mut edited = std::fs::read(&claude).unwrap();
    edited.extend_from_slice(b"\nMy own rule: never rename the tree.\n");
    std::fs::write(&claude, &edited).unwrap();
    let analysis = flow.scan_and_analyze("existing", None).await;
    assert_eq!(analysis.status, 200, "analyze 3: {}", analysis.text);
    model.finish(label, "G3: three analyses of one project, before and after setup");
    let plan = flow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "plan 3: {}", plan.text);
    let conflict = plan.body["plan"]["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["path"] == "CLAUDE.md")
        .unwrap_or_else(|| panic!("the edited generated file is a conflict: {}", plan.text));
    assert!(
        conflict["options"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o == "keep")
    );
    let settled = settle_conflicts(&flow, &plan, "keep").await;
    let third = approve_apply(&flow, &settled).await;
    assert_eq!(third.body["status"], "applied", "{}", third.text);
    assert_eq!(
        std::fs::read(&claude).unwrap(),
        edited,
        "the edited generated file is preserved"
    );
    print_pin("g3");
}

#[tokio::test]
#[ignore = "live: calls the Anthropic API with the user's key"]
async fn g3_rerun_live() {
    g3_rerun(true).await
}

#[tokio::test]
async fn g3_rerun_is_clean_and_an_edited_generated_file_is_a_conflict() {
    g3_rerun(false).await
}

// ----- G4: interruptions and recovery ----------------------------------------------------------
//
// The plan is Setup's real plan for the uploaded mod, built by `DefaultPlanBuilder` from the
// verified package. It is applied by Setup's transaction engine with Setup's own fault options
// (graceful interruptions at named boundaries) or in a child process that is killed (a hard stop).
// The analysis record the plan is built for is a structural stand-in: it is not model output, it
// never enters the facade, and the gates that concern the model are tested by the HTTP flow.
// Recovery itself goes only through the server: `GET .../recovery` and `POST .../recover`.

use hoi4_mod_setup::models::{CodexAnalysisRecord, InstallationPlan, PreparedFile};
use hoi4_mod_setup::service::{DefaultPlanBuilder, PlanBuilder, PlanRequest, root_identity};
use hoi4_mod_setup::transaction::{
    LiveMutationBarrier, TransactionOptions, run_transaction,
};

const LOCAL_ENDPOINT: &str = "http://127.0.0.1:11434/v1/chat/completions";

fn structural_record(source: &hoi4_mod_setup::models::SourceIdentity) -> CodexAnalysisRecord {
    CodexAnalysisRecord {
        engine: "provider_api".into(),
        auth_mode: "local_endpoint".into(),
        provider: Some("local".into()),
        model: Some("fixture".into()),
        reasoning_effort: Some("high".into()),
        optimization_profile: Some(
            hoi4_mod_setup::ai::profile("local")
                .unwrap()
                .optimization_profile,
        ),
        analysis_id: Uuid::new_v4(),
        schema_version: "1.0.0".into(),
        input_sha256: sha256_hex(b"input"),
        output_sha256: sha256_hex(b"output"),
        confirmed_fields: hoi4_mod_setup::codex::REQUIRED_ANALYSIS_PROPOSAL_KEYS
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

/// Setup's real plan for the project at `root`, every conflict kept, reviewed and ready to run.
fn real_plan(
    root: &std::path::Path,
    state_root: &std::path::Path,
    fetcher: Arc<dyn factory_setup_adapter::setup::SourceFetcher>,
) -> (InstallationPlan, Vec<PreparedFile>) {
    use hoi4_mod_setup::source::{SourceRequest, resolve_source};
    let root = hoi4_mod_setup::paths::validate_project_root(root).unwrap();
    let identity = root_identity(&root).unwrap();
    let resolution = resolve_source(
        fetcher.as_ref(),
        &SourceRequest {
            mode: hoi4_mod_setup::models::SourceMode::Latest,
            requested_ref: None,
            release: None,
        },
    )
    .unwrap();
    let record = structural_record(&resolution.identity);
    let state = json!({
        "mode": "existing",
        "description": "G4 fixture",
        "sourceMode": "pinned_commit",
        "pinnedRef": resolution.identity.resolved_revision,
        "aiProvider": "local",
        "aiModel": "fixture",
        "aiReasoningEffort": "high",
        "aiEndpoint": LOCAL_ENDPOINT,
        "gitMode": "skip",
        "conflictChoice": "keep",
        "primaryCodingEnvironment": "claude_code",
        "additionalCodingEnvironments": ["codex"],
        "selectedComponents": [],
        "folderProfile": ["common/national_focus", "localisation"],
        "conventions": {
            "agents_profile": "default",
            "localisation_convention": "english",
            "documentation_convention": "markdown",
        },
        "identity": {
            "displayName": "Gate Fixture",
            "projectId": "gate_fixture",
            "author": "alice",
            "version": "0.3.1",
            "supportedGameVersion": "1.19.*",
            "projectRoot": root.display().to_string(),
            "defaultBranch": "main",
            "scriptPrefix": "gate",
            "primaryNamespace": "gate",
            "descriptorTags": ["Alternative History"],
        },
        "componentRecommendations": [],
    });
    let built = DefaultPlanBuilder::with_source(fetcher)
        .build(&PlanRequest {
            state: &state,
            project_root: &root,
            project_root_exists: true,
            project_root_identity: &identity,
            analysis: &record,
            state_root,
            node: None,
        })
        .expect("Setup builds the plan");
    let mut plan = built.plan;
    plan.approvals.dry_run_reviewed = true;
    plan.approvals.external_actions_reviewed = true;
    (plan, built.prepared_files)
}

/// A project with the uploaded gate mod, its inventory before anything runs, and the file the
/// adapter keeps beside a journal when it applies a plan itself.
struct Scene {
    user: User,
    project: String,
    source: PathBuf,
    state_root: PathBuf,
    before: SnapshotInventory,
}

async fn scene(h: &Harness, email: &str, name: &str) -> Scene {
    let user = h.verified_user(email).await;
    let project = upload_fixture(h, &user, name).await;
    // Any read of the project creates the Setup state directory next to `source/`.
    let status = user
        .client
        .get(&format!("/api/v1/projects/{project}/setup/status"))
        .await;
    assert_eq!(status.status, 200, "{}", status.text);
    let source = h.workspace_source(&user, &project);
    let state_root = source.parent().unwrap().join("setup-state");
    assert!(state_root.is_dir());
    let before = inventory_of(h, &user, &project);
    Scene {
        user,
        project,
        source,
        state_root,
        before,
    }
}

fn keep_pre_apply_inventory(scene: &Scene, plan_id: Uuid) {
    let dir = scene.state_root.join("adapter");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("pre-apply-{plan_id}.json")),
        serde_json::to_vec(&scene.before).unwrap(),
    )
    .unwrap();
}

async fn recovery_of(scene: &Scene) -> Value {
    let resp = scene
        .user
        .client
        .get(&format!("/api/v1/projects/{}/setup/recovery", scene.project))
        .await;
    assert_eq!(resp.status, 200, "{}", resp.text);
    resp.body["recovery"].clone()
}

async fn recover_via_http(scene: &Scene, recovery: &Value) -> Resp {
    scene
        .user
        .client
        .post(
            &format!("/api/v1/projects/{}/setup/recover", scene.project),
            json!({"journal_id": recovery["journal_id"], "variant": recovery["variant"]}),
        )
        .await
}

/// How an interrupted run is expected to be classified and recovered.
struct Case {
    name: &'static str,
    fault: fn(&mut TransactionOptions),
    variant: &'static str,
    action: &'static str,
    /// Journal state after the recovery action.
    ends: &'static str,
    /// The project must equal the inventory before the run.
    restores: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "staging unfinished",
        fault: |o| o.fail_before_stage = Some(6),
        variant: "discardable_staging",
        action: "discard_staging",
        ends: "staging_discarded",
        restores: true,
    },
    Case {
        name: "staging complete, before the first change",
        fault: |o| o.fail_before_stage = Some(7),
        variant: "resumable_staging",
        action: "resume",
        ends: "completed",
        restores: false,
    },
    Case {
        name: "first apply, after operation 0",
        fault: |o| o.fail_after_operation = Some(0),
        variant: "rollback_required_partial_apply",
        action: "rollback",
        ends: "rolled_back",
        restores: true,
    },
    Case {
        name: "first apply, live change made and not journaled",
        fault: |o| o.fail_after_live_mutation = Some(0),
        variant: "rollback_required_partial_apply",
        action: "rollback",
        ends: "rolled_back",
        restores: true,
    },
];

#[tokio::test]
async fn g4_graceful_interruptions_are_classified_and_recoverable() {
    let model = model_for("upload-fixture", false);
    let h = gate_harness(&model, None).await;
    let fetcher = PinnedSource::new().await;
    print_pin("g4");
    for (n, case) in CASES.iter().enumerate() {
        let scene = scene(&h, &format!("user{n}@example.org"), case.name).await;
        let (source, state_root) = (scene.source.clone(), scene.state_root.clone());
        let fault = case.fault;
        let f = fetcher.clone();
        let (plan_id, err) = tokio::task::spawn_blocking(move || {
            let (plan, files) = real_plan(&source, &state_root, f);
            let mut options = TransactionOptions {
                app_data_root: Some(state_root.clone()),
                ..Default::default()
            };
            fault(&mut options);
            let root = hoi4_mod_setup::paths::validate_project_root(&source).unwrap();
            let err = run_transaction(&root, &plan, &files, &options)
                .expect_err("the fault interrupts the run");
            (plan.plan_id, err.to_string())
        })
        .await
        .unwrap();
        assert!(err.contains("fault injected"), "{}: {err}", case.name);
        keep_pre_apply_inventory(&scene, plan_id);

        // Classified: exactly one action, the one the journal permits.
        let recovery = recovery_of(&scene).await;
        assert_eq!(recovery["variant"], case.variant, "{}: {recovery}", case.name);
        assert_eq!(recovery["allowed_action"], case.action, "{}", case.name);
        assert_eq!(recovery["owner_active"], false);
        // The workspace handoff is withheld while a transaction waits.
        let ws = scene
            .user
            .client
            .get(&format!("/api/v1/projects/{}/workspace", scene.project))
            .await;
        assert_eq!(ws.body["setup"]["ready"], false, "{}", case.name);
        // Any other variant than the shown one is refused, and nothing changes.
        let other = if case.variant == "rollback_required_partial_apply" {
            "resumable_staging"
        } else {
            "rollback_required_partial_apply"
        };
        let refused = scene
            .user
            .client
            .post(
                &format!("/api/v1/projects/{}/setup/recover", scene.project),
                json!({"journal_id": recovery["journal_id"], "variant": other}),
            )
            .await;
        assert_eq!(
            (refused.status, refused.error_code()),
            (409, "recovery_variant_changed"),
            "{}: {}",
            case.name,
            refused.text
        );

        // Recovered.
        let done = recover_via_http(&scene, &recovery).await;
        assert_eq!(done.status, 200, "{}: {}", case.name, done.text);
        assert_eq!(done.body["recovery"]["action"], case.action);
        assert_eq!(done.body["recovery"]["journal_state"], case.ends, "{}", case.name);
        let after = inventory_of(&h, &scene.user, &scene.project);
        let diff = scene.before.diff(&after);
        if case.restores {
            assert!(diff.is_empty(), "{}: restoration differs: {diff:?}", case.name);
            if case.ends == "rolled_back" {
                assert_eq!(done.body["recovery"]["restoration"]["exact"], true, "{}", case.name);
            }
        } else {
            assert!(
                diff.changed.is_empty() && diff.removed.is_empty(),
                "{}: {diff:?}",
                case.name
            );
        }
        assert!(recovery_of(&scene).await.is_null(), "{}", case.name);
        println!(
            "G4 [{}]: {} -> {} -> {} (restored exactly: {})",
            case.name, case.variant, case.action, case.ends, case.restores
        );
    }
}

/// The post-install and finalization boundaries. Their outcome depends on the Setup revision:
/// the finalization case is the dead end that Setup pull request 107 fixes, so the test records
/// what this revision does instead of asserting an outcome it cannot have.
#[tokio::test]
async fn g4_post_install_and_finalization_boundaries_are_classified() {
    let model = model_for("upload-fixture", false);
    let h = gate_harness(&model, None).await;
    let fetcher = PinnedSource::new().await;
    print_pin("g4_boundaries");
    for (n, (name, stage)) in [("post-install checks done", 9usize), ("finalization", 11)]
        .into_iter()
        .enumerate()
    {
        let scene = scene(&h, &format!("late{n}@example.org"), name).await;
        let (source, state_root) = (scene.source.clone(), scene.state_root.clone());
        let f = fetcher.clone();
        let plan_id = tokio::task::spawn_blocking(move || {
            let (plan, files) = real_plan(&source, &state_root, f);
            let options = TransactionOptions {
                app_data_root: Some(state_root.clone()),
                fail_after_stage: Some(stage),
                ..Default::default()
            };
            let root = hoi4_mod_setup::paths::validate_project_root(&source).unwrap();
            let err = run_transaction(&root, &plan, &files, &options)
                .expect_err("the fault interrupts the run");
            assert!(err.to_string().contains("fault injected"), "{err}");
            plan.plan_id
        })
        .await
        .unwrap();
        keep_pre_apply_inventory(&scene, plan_id);
        let recovery = recovery_of(&scene).await;
        assert!(!recovery.is_null(), "{name}: an interrupted run is visible");
        let variant = recovery["variant"].as_str().unwrap().to_owned();
        let action = recovery["allowed_action"].as_str().map(str::to_owned);
        println!("G4 [{name}]: classified {variant}, permitted action {action:?}");
        assert!(
            ["rollback_required_partial_apply", "verifiable_finalization", "manual_review"]
                .contains(&variant.as_str()),
            "{name}: {variant}"
        );
        // Nothing new may be applied on top of it.
        let blocked = scene
            .user
            .client
            .get(&format!("/api/v1/projects/{}/workspace", scene.project))
            .await;
        assert_eq!(blocked.body["setup"]["ready"], false);
        if action.is_none() {
            println!("G4 [{name}]: manual review, no action offered");
            continue;
        }
        let done = recover_via_http(&scene, &recovery).await;
        let end = if done.status == 200 {
            done.body["recovery"]["journal_state"].as_str().unwrap().to_owned()
        } else {
            format!("refused: {} {}", done.error_code(), done.body["error"]["message"])
        };
        println!("G4 [{name}]: recover {action:?} ended as {end}");
        let after = recovery_of(&scene).await;
        if variant == "rollback_required_partial_apply" {
            assert_eq!(done.status, 200, "{}", done.text);
            assert_eq!(done.body["recovery"]["restoration"]["exact"], true);
            assert!(scene.before.diff(&inventory_of(&h, &scene.user, &scene.project)).is_empty());
            assert!(after.is_null());
        } else if done.status == 200 {
            assert_eq!(done.body["recovery"]["journal_state"], "completed");
        } else {
            // The dead end: resume refused. The view must then be manual review, not a loop.
            assert_eq!(after["variant"], "manual_review", "{after}");
            assert!(after["allowed_action"].is_null());
        }
    }
}

// ----- a real process kill ---------------------------------------------------------------------

/// Called by the transaction engine inside the window around a live change. In the child it
/// reports "ready" and then waits to be killed.
fn kill_window(_destination: &std::path::Path, index: usize, barrier: LiveMutationBarrier) {
    let want_index: usize = std::env::var("HF_G4_KILL_OP").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
    let want = std::env::var("HF_G4_KILL_AT").unwrap_or_default();
    let here = match barrier {
        LiveMutationBarrier::AfterPrecondition => "precondition",
        LiveMutationBarrier::AfterQuarantineVerified => "quarantine",
        LiveMutationBarrier::AfterPlacement => "placement",
    };
    if index == want_index && here == want {
        if let Ok(signal) = std::env::var("HF_G4_SIGNAL") {
            std::fs::write(signal, b"ready").unwrap();
        }
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

/// The child process: builds Setup's real plan and applies it. Started (and killed) by
/// `g4_a_killed_process_leaves_a_recoverable_state`; does nothing when run on its own.
#[test]
#[ignore = "child process of the hard-kill test"]
fn g4_child_applies() {
    let (Ok(root), Ok(state_root), Ok(plan_file)) = (
        std::env::var("HF_G4_ROOT"),
        std::env::var("HF_G4_STATE"),
        std::env::var("HF_G4_PLAN_FILE"),
    ) else {
        return;
    };
    let fetcher: Arc<dyn factory_setup_adapter::setup::SourceFetcher> = PinnedSource::build();
    let (root, state_root) = (PathBuf::from(root), PathBuf::from(state_root));
    let (plan, files) = real_plan(&root, &state_root, fetcher);
    std::fs::write(plan_file, plan.plan_id.to_string()).unwrap();
    let options = TransactionOptions {
        app_data_root: Some(state_root),
        live_mutation_barrier: Some(kill_window),
        ..Default::default()
    };
    let root = hoi4_mod_setup::paths::validate_project_root(&root).unwrap();
    let _ = run_transaction(&root, &plan, &files, &options);
}

async fn hard_kill(window: &str, op: usize) {
    let model = model_for("upload-fixture", false);
    let h = gate_harness(&model, None).await;
    print_pin("g4_kill");
    let scene = scene(&h, "killed@example.org", "Killed").await;
    let signal = scene.state_root.parent().unwrap().join("child-ready");
    let plan_file = scene.state_root.parent().unwrap().join("child-plan-id");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "g4_child_applies", "--nocapture", "--test-threads=1"])
        .env("HF_G4_ROOT", &scene.source)
        .env("HF_G4_STATE", &scene.state_root)
        .env("HF_G4_PLAN_FILE", &plan_file)
        .env("HF_G4_SIGNAL", &signal)
        .env("HF_G4_KILL_AT", window)
        .env("HF_G4_KILL_OP", op.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the child process starts");
    let started = std::time::Instant::now();
    while !signal.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the child ended ({status}) before it reached the kill window");
        }
        assert!(started.elapsed().as_secs() < 900, "the child never reached the kill window");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let plan_id: Uuid = std::fs::read_to_string(&plan_file).unwrap().trim().parse().unwrap();
    keep_pre_apply_inventory(&scene, plan_id);
    // The hard stop: TerminateProcess on Windows, SIGKILL on Linux. No cleanup code runs.
    child.kill().expect("kill the child");
    let status = child.wait().unwrap();
    assert!(!status.success(), "the child was killed, not finished: {status}");

    let recovery = recovery_of(&scene).await;
    assert!(!recovery.is_null(), "a killed apply leaves a journal the server finds");
    let variant = recovery["variant"].as_str().unwrap().to_owned();
    println!(
        "G4 kill [{window} of operation {op}]: classified {variant}, permitted {}",
        recovery["allowed_action"]
    );
    assert_eq!(variant, "rollback_required_partial_apply");
    assert_eq!(recovery["allowed_action"], "rollback", "resume is never offered after a partial apply");
    let done = recover_via_http(&scene, &recovery).await;
    assert_eq!(done.status, 200, "{}", done.text);
    assert_eq!(done.body["recovery"]["journal_state"], "rolled_back");
    let diff = scene.before.diff(&inventory_of(&h, &scene.user, &scene.project));
    let exact = done.body["recovery"]["restoration"]["exact"] == true;
    println!("G4 kill [{window} of operation {op}]: restored exactly = {exact}; differences = {diff:?}");
    if factory_setup_adapter::SETUP_PIN.contains_pr_105_fix {
        assert!(exact && diff.is_empty(), "{diff:?}");
    } else {
        // Without the PR 105 fix a placement that was never journaled can leave its temporary
        // behind. Nothing else may differ, and this result does not count for S02-G4.
        let strays: Vec<&String> = diff
            .added
            .iter()
            .chain(&diff.changed)
            .chain(&diff.removed)
            .collect();
        assert!(
            strays.iter().all(|p| p.contains(".hoi4ms-") || p.ends_with(".tmp")),
            "only a placement temporary may be left behind: {diff:?}"
        );
        assert!(diff.mode_changed.is_empty() && !diff.index_changed);
    }
    assert!(recovery_of(&scene).await.is_null());
}

#[tokio::test]
async fn g4_a_killed_process_leaves_a_recoverable_state() {
    // Before the first live change of the first operation, then in the placement window of the
    // second one (one file is installed, the journal does not know it yet).
    hard_kill("precondition", 0).await;
    hard_kill("placement", 1).await;
}

// ----- the gates, in order, over HTTP ----------------------------------------------------------

#[tokio::test]
async fn the_gates_hold_in_order_and_a_restart_expires_the_session() {
    let model = model_for("upload-fixture", false);
    let mut h = gate_harness(&model, None).await;
    let mut alice = h.verified_user("alice@example.org").await;
    let project = upload_fixture(&h, &alice, "Gated").await;
    let base = format!("/api/v1/projects/{project}/setup");
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let code = |r: &Resp| (r.status, r.error_code().to_owned());

    // No session yet: nothing downstream works, and the status says so.
    let status = flow.get("status").await;
    assert_eq!(status.body["step"], "none");
    for tail in ["analyze", "plan"] {
        let r = flow.post(tail, json!({})).await;
        assert_eq!(code(&r), (409, "setup_session_missing".into()), "{tail}");
    }

    // A scan opens a session. The model is not reachable before the evidence is decided.
    let scan = flow.post("scan", json!({"mode": "existing"})).await;
    assert_eq!(scan.status, 200, "{}", scan.text);
    assert_eq!(flow.get("status").await.body["step"], "scanned");
    let r = flow.post("analyze", json!({})).await;
    assert_eq!(code(&r), (409, "evidence_not_approved".into()), "{}", r.text);
    assert_eq!(model.replay.as_ref().unwrap().calls_made(), 0, "the model was not called");
    // Only what the scan offered, and only for the latest scan.
    let r = flow
        .post(
            "evidence",
            json!({"scan_id": scan.body["scan_id"], "references": ["not-offered"]}),
        )
        .await;
    assert_eq!(code(&r), (409, "unknown_evidence_reference".into()), "{}", r.text);
    let r = flow
        .post(
            "evidence",
            json!({"scan_id": Uuid::new_v4(), "references": []}),
        )
        .await;
    assert_eq!(code(&r), (409, "scan_stale".into()), "{}", r.text);
    let references: Vec<Value> = scan.body["evidence"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["reference"].clone())
        .collect();
    let r = flow
        .post(
            "evidence",
            json!({"scan_id": scan.body["scan_id"], "references": references}),
        )
        .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(flow.get("status").await.body["step"], "evidence_approved");

    // No plan without a confirmed analysis.
    let analysis = flow.post("analyze", json!({})).await;
    assert_eq!(analysis.status, 200, "{}", analysis.text);
    assert_eq!(flow.get("status").await.body["step"], "analyzed");
    let r = flow.post("plan", json!({})).await;
    assert_eq!(code(&r), (409, "analysis_not_confirmed".into()), "{}", r.text);
    let r = flow
        .post(
            "confirm",
            json!({"analysis_id": Uuid::new_v4(), "values": analysis.body["values"]}),
        )
        .await;
    assert_eq!(code(&r), (409, "analysis_not_in_session".into()), "{}", r.text);
    let mut bad = analysis.body["values"].clone();
    bad["identity"]["project_id"] = json!("");
    let r = flow
        .post(
            "confirm",
            json!({"analysis_id": analysis.body["analysis_id"], "values": bad}),
        )
        .await;
    assert_eq!(code(&r), (422, "invalid_request".into()), "{}", r.text);
    assert_eq!(r.body["error"]["affected_ids"]["field"], "project_id");
    assert_eq!(flow.get("status").await.body["step"], "analyzed");

    // The person confirms exactly the values shown.
    let plan = flow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "{}", plan.text);
    assert_eq!(flow.get("status").await.body["step"], "planned");
    let shown = plan.body["plan"].clone();
    let plan_id = shown["plan_id"].clone();

    // No apply without an approval; an approval names the hash that was reviewed.
    let r = flow.post("apply", json!({"plan_id": plan_id})).await;
    assert_eq!(code(&r), (409, "plan_not_approved".into()), "{}", r.text);
    let r = flow
        .post("approve", json!({"plan_id": plan_id, "plan_sha256": "0".repeat(64)}))
        .await;
    assert_eq!(code(&r), (409, "plan_changed".into()), "{}", r.text);
    let r = flow
        .post(
            "approve",
            json!({"plan_id": plan_id, "plan_sha256": shown["plan_sha256"]}),
        )
        .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(flow.get("status").await.body["step"], "approved");

    // A conflict decision changes the plan: new hash, approval withdrawn, old hash refused.
    let open: Vec<Value> = shown["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!({"path": c["path"], "choice": "keep"}))
        .collect();
    assert!(!open.is_empty());
    let decided = flow
        .post("conflicts", json!({"plan_id": plan_id, "decisions": open}))
        .await;
    assert_eq!(decided.status, 200, "{}", decided.text);
    let reshown = decided.body["plan"].clone();
    assert_ne!(reshown["plan_sha256"], shown["plan_sha256"]);
    assert_eq!(reshown["approved"], false);
    let r = flow.post("apply", json!({"plan_id": plan_id})).await;
    assert_eq!(code(&r), (409, "plan_not_approved".into()), "{}", r.text);
    let r = flow
        .post(
            "apply",
            json!({"plan_id": plan_id, "plan_sha256": shown["plan_sha256"]}),
        )
        .await;
    assert_eq!(code(&r), (409, "plan_changed".into()), "{}", r.text);
    let inventory = inventory_of(&h, &alice, &project);
    assert!(
        inventory.diff(&inventory_of(&h, &alice, &project)).is_empty(),
        "every refusal so far changed nothing"
    );
    let source = h.workspace_source(&alice, &project);
    assert!(!source.join("CLAUDE.md").exists(), "nothing was written before the apply");

    // One owner: two applies of the approved plan race, one wins.
    let r = flow
        .post(
            "approve",
            json!({"plan_id": plan_id, "plan_sha256": reshown["plan_sha256"]}),
        )
        .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let (a, b) = tokio::join!(
        flow.post("apply", json!({"plan_id": plan_id})),
        flow.post("apply", json!({"plan_id": plan_id}))
    );
    let winners = [&a, &b].iter().filter(|r| r.status == 200 && r.body["status"] == "applied").count();
    assert_eq!(winners, 1, "exactly one apply wins: {} / {}", a.text, b.text);
    let loser = if a.status == 200 { &b } else { &a };
    assert!(
        ["project_busy", "plan_not_approved"].contains(&loser.error_code()),
        "the loser is a refusal: {}",
        loser.text
    );
    assert_eq!(flow.get("status").await.body["step"], "applied");
    // The applied plan cannot be applied again.
    let r = flow.post("apply", json!({"plan_id": plan_id})).await;
    assert_eq!(code(&r), (409, "plan_not_approved".into()), "{}", r.text);

    // A restart ends the session: the facade holds its gates in memory. The database remembers
    // that a session existed, so the answer is "expired", not "never opened".
    h.restart().await;
    alice = h.rebind(&alice);
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let status = flow.get("status").await;
    assert_eq!(status.status, 200, "{}", status.text);
    assert_eq!(status.body["step"], "expired");
    assert_eq!(status.body["session"]["mode"], "existing");
    for (tail, body) in [
        ("analyze", json!({})),
        ("plan", json!({})),
        ("apply", json!({"plan_id": plan_id})),
        ("approve", json!({"plan_id": plan_id, "plan_sha256": "0"})),
    ] {
        let r = flow.post(tail, body).await;
        assert_eq!(code(&r), (409, "setup_session_expired".into()), "{tail}: {}", r.text);
    }
    // What does not need a session still works: the plan record, the recovery view, the workspace.
    assert_eq!(flow.get("plan").await.status, 200);
    assert_eq!(flow.get("recovery").await.status, 200);
    let ws = alice.client.get(&format!("/api/v1/projects/{project}/workspace")).await;
    assert_eq!(ws.body["summary"]["setup_state"], "completed");
    // Scanning again starts over.
    let scan = flow.post("scan", json!({"mode": "existing"})).await;
    assert_eq!(scan.status, 200, "{}", scan.text);
    assert_eq!(flow.get("status").await.body["step"], "scanned");
    let _ = base;
}

#[tokio::test]
async fn a_lost_new_mod_session_keeps_its_brief_for_the_next_scan() {
    let model = model_for("new-mod", false);
    let mut h = gate_harness(&model, None).await;
    let mut alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Brief").await;
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let scan = flow
        .post("scan", json!({"mode": "create", "description": NEW_MOD_BRIEF}))
        .await;
    assert_eq!(scan.status, 200, "{}", scan.text);
    assert_eq!(scan.body["evidence"]["required"], false);
    h.restart().await;
    alice = h.rebind(&alice);
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    assert_eq!(flow.get("status").await.body["step"], "expired");
    // No description this time: the lost session's brief is reused, so the analysis can run.
    let again = flow.post("scan", json!({"mode": "create"})).await;
    assert_eq!(again.status, 200, "{}", again.text);
    let analysis = flow.post("analyze", json!({})).await;
    assert_eq!(analysis.status, 200, "{}", analysis.text);
    // A new mod can only start in an empty workspace.
    let other = upload_fixture(&h, &alice, "Occupied").await;
    let flow = Flow {
        client: &alice.client,
        project: other,
    };
    let r = flow.post("scan", json!({"mode": "create", "description": "x"})).await;
    assert_eq!((r.status, r.error_code()), (409, "workspace_not_empty"), "{}", r.text);
}

// ----- two accounts ----------------------------------------------------------------------------

#[tokio::test]
async fn two_accounts_cannot_see_each_others_setup_records_over_http() {
    let model = model_for("upload-fixture", false);
    let h = gate_harness(&model, None).await;
    let alice = h.verified_user("alice@example.org").await;
    let bob = h.verified_user("bob@example.org").await;
    let alice_project = upload_fixture(&h, &alice, "Alice's").await;
    let aflow = Flow {
        client: &alice.client,
        project: alice_project.clone(),
    };
    let analysis = aflow.scan_and_analyze("existing", None).await;
    let plan = aflow.confirm_and_plan(&analysis).await;
    assert_eq!(plan.status, 200, "{}", plan.text);
    let alice_plan = plan.body["plan"].clone();

    // Bob, on Alice's project: every Setup route answers exactly as for a project that does not exist.
    let missing = Uuid::new_v4().to_string();
    let calls: Vec<(&str, &str, Value)> = vec![
        ("GET", "status", Value::Null),
        ("GET", "plan", Value::Null),
        ("GET", "recovery", Value::Null),
        ("POST", "scan", json!({"mode": "existing"})),
        ("POST", "analyze", json!({})),
        ("POST", "plan", json!({})),
        (
            "POST",
            "approve",
            json!({"plan_id": alice_plan["plan_id"], "plan_sha256": alice_plan["plan_sha256"]}),
        ),
        ("POST", "apply", json!({"plan_id": alice_plan["plan_id"]})),
        (
            "POST",
            "recover",
            json!({"journal_id": Uuid::new_v4(), "variant": "manual_review"}),
        ),
    ];
    for (method, tail, body) in &calls {
        let foreign = setup_call(&bob.client, method, &alice_project, tail, body).await;
        let absent = setup_call(&bob.client, method, &missing, tail, body).await;
        assert_eq!(
            (foreign.status, foreign.error_code()),
            (404, "not_found"),
            "{method} {tail}: {}",
            foreign.text
        );
        assert_eq!(foreign.body, absent.body, "{method} {tail}");
    }
    // Bob's own project knows nothing of Alice's records, and Bob's session cannot name her plan.
    let bob_project = h.project(&bob, "Bob's").await;
    let bflow = Flow {
        client: &bob.client,
        project: bob_project.clone(),
    };
    let status = bflow.get("status").await;
    assert_eq!(status.body["step"], "none");
    assert!(status.body["status"]["latest_plan"].is_null());
    assert!(status.body["status"]["latest_scan"].is_null());
    let r = bflow.get("plan").await;
    assert_eq!((r.status, r.error_code()), (409, "plan_not_built"));
    let created = bflow
        .post("scan", json!({"mode": "create", "description": "Bob's own mod"}))
        .await;
    assert_eq!(created.status, 200, "{}", created.text);
    for (tail, body) in [
        (
            "approve",
            json!({"plan_id": alice_plan["plan_id"], "plan_sha256": alice_plan["plan_sha256"]}),
        ),
        ("apply", json!({"plan_id": alice_plan["plan_id"]})),
        (
            "conflicts",
            json!({"plan_id": alice_plan["plan_id"], "decisions": [{"path": "AGENTS.md", "choice": "replace"}]}),
        ),
    ] {
        let r = bflow.post(tail, body).await;
        assert_eq!(
            (r.status, r.error_code()),
            (404, "not_found"),
            "Alice's plan id is nothing in Bob's session ({tail}): {}",
            r.text
        );
    }
    // Alice is unaffected: her session, her plan and her bytes.
    let status = aflow.get("status").await;
    assert_eq!(status.body["step"], "planned");
    assert_eq!(status.body["plan"]["plan_sha256"], alice_plan["plan_sha256"]);
    assert!(!h.data_dir.path().join("workspaces").join(&bob.id).join(&alice_project).exists());
    assert!(
        !status.text.contains(&bob.id),
        "no trace of the other account in Alice's answers"
    );
}

// ----- a revoked GitHub installation during import -----------------------------------------------

#[tokio::test]
async fn a_revoked_installation_during_import_leaves_the_workspace_untouched() {
    let model = model_for("github-import", false);
    let gh = github_fixture().await;
    let h = gate_harness(&model, Some(gh)).await;
    let alice = h.verified_user("alice@example.org").await;
    h.link_github(&alice, 424_242).await;
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "selected")];

    // The person lists the repositories, then the installation is revoked before the import ends.
    let project = h.project(&alice, "Revoked").await;
    let listed = alice
        .client
        .get(&format!("/api/v1/projects/{project}/github/repositories"))
        .await;
    assert_eq!(listed.status, 200, "{}", listed.text);
    h.github.as_ref().unwrap().fake.with(|s| s.revoked = true);
    let refused = alice
        .client
        .post(
            &format!("/api/v1/projects/{project}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert!(
        refused.status >= 400 && !refused.error_code().is_empty(),
        "{}",
        refused.text
    );
    println!("revoked import: {} {}", refused.status, refused.error_code());
    // Nothing was created: no workspace directory, no Setup scan target, no repository link in use.
    let project_dir = h
        .data_dir
        .path()
        .join("workspaces")
        .join(&alice.id)
        .join(&project);
    assert!(!project_dir.exists(), "the failed import left no workspace behind");
    let ws = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.body["summary"]["has_source"], false);
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let scan = flow.post("scan", json!({"mode": "existing"})).await;
    assert_eq!((scan.status, scan.error_code()), (409, "no_source"));

    // And a project that already holds a mod keeps it byte for byte when an import is refused.
    let occupied = upload_fixture(&h, &alice, "Occupied").await;
    let before = inventory_of(&h, &alice, &occupied);
    let refused = alice
        .client
        .post(
            &format!("/api/v1/projects/{occupied}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert!(refused.status >= 400, "{}", refused.text);
    assert!(before.diff(&inventory_of(&h, &alice, &occupied)).is_empty());
    let scan = Flow {
        client: &alice.client,
        project: occupied,
    }
    .post("scan", json!({"mode": "existing"}))
    .await;
    assert_eq!(scan.status, 200, "{}", scan.text);
}

async fn setup_call(
    client: &Client,
    method: &str,
    project: &str,
    tail: &str,
    body: &Value,
) -> Resp {
    let path = format!("/api/v1/projects/{project}/setup/{tail}");
    if method == "POST" {
        client.post(&path, body.clone()).await
    } else {
        client.get(&path).await
    }
}
