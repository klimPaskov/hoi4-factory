//! The S04 focus editor through the assembled server (W1, W2 and gate 3), against a real
//! PostgreSQL with row-level security and the real HOI4 Agent Tools.
//!
//! Gated on `FACTORY_AGENT_TOOLS_NODE` and `FACTORY_AGENT_TOOLS_ENTRY` (the tests print a note and
//! pass without them) and on `FACTORY_TEST_ADMIN_URL` like every server test. The fixture is
//! F-FOCUS-SMALL (`tests/fixtures/focus-small`), the EXP-02 basic tree with icons and localisation.

#![allow(dead_code, unused_imports)]

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");

use std::path::Path;

const SOURCE: &str = "common/national_focus/exp_basic.txt";

fn fixture_entries() -> Vec<ZEntry> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/focus-small");
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<ZEntry>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(base, &e.path(), out);
            } else {
                let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.push(ZEntry::file(&rel, std::fs::read(e.path()).unwrap()));
            }
        }
    }
    walk(&root, &root, &mut out);
    out
}

struct Editor {
    h: Harness,
    alice: User,
    project: String,
    task: String,
}

impl Editor {
    fn base(&self) -> String {
        format!("/api/v1/projects/{}/tasks/{}", self.project, self.task)
    }

    fn worktree(&self) -> PathBuf {
        self.h.data_dir.path().join("workspaces").join(&self.alice.id).join(&self.project).join("tasks").join(&self.task)
    }

    async fn projection(&self) -> Value {
        let r = self.alice.client.get(&format!("{}/focus/trees/exp_basic_tree", self.base())).await;
        assert_eq!(r.status, 200, "{}", r.text);
        r.body
    }

    async fn ops(&self, user: &User, expected: &str, ops: Value) -> Resp {
        user.client
            .post(
                &format!("{}/focus/trees/exp_basic_tree/operations", self.base()),
                json!({ "expected_projection_hash": expected, "operations": ops }),
            )
            .await
    }

    fn source(&self) -> Vec<u8> {
        std::fs::read(self.worktree().join(SOURCE)).unwrap()
    }
}

async fn editor() -> Option<Editor> {
    let Some(install) = factory_hoi4_tools::AgentToolsInstall::from_env() else {
        eprintln!("SKIPPED: FACTORY_AGENT_TOOLS_NODE / FACTORY_AGENT_TOOLS_ENTRY are not set");
        return None;
    };
    let h = Harness::start(Options {
        // The task route needs a model profile; no model call is made in these tests.
        anthropic_key: Some("sk-ant-not-used"),
        agent: AgentOptions {
            worker: None,
            settings: None,
            tools: Some(Some(factory_server::agent::AgentToolsSettings { install, game_root: None })),
            skip_recovery: true,
        },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Focus fixture").await;
    let up = alice
        .client
        .upload(&project, &[Part::text("kind", "zip"), Part::text("git_init_consent", "true"), Part::file("f.zip", build_zip(&fixture_entries()))])
        .await;
    assert_eq!(up.status, 201, "{}", up.text);
    let t = alice
        .client
        .post(
            &format!("/api/v1/projects/{project}/tasks"),
            json!({ "objective": "Edit the focus tree in the editor", "mode": "edit", "allowed_paths": ["common/national_focus", "localisation"] }),
        )
        .await;
    assert_eq!(t.status, 201, "{}", t.text);
    let task = t.body["id"].as_str().unwrap().to_owned();
    Some(Editor { h, alice, project, task })
}

#[tokio::test]
async fn gestures_become_proposals_with_minimal_byte_changes() {
    let Some(e) = editor().await else { return };
    let p = e.projection().await;
    assert_eq!(p["treeId"], "exp_basic_tree");
    assert_eq!(p["sourcePath"], SOURCE);
    assert_eq!(p["nodes"].as_array().unwrap().len(), 4);
    let root = p["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "exp_root").unwrap();
    assert_eq!(root["label"], "Root", "{root}");
    assert_eq!(root["editable"], true);
    assert_eq!(p["tool"]["version"], factory_hoi4_tools::profile::AGENT_TOOLS_VERSION);
    assert!(!p.to_string().contains(&e.h.data_dir.path().to_string_lossy().to_string()), "no server paths reach the browser");
    let before = e.source();

    // A drag, a cost edit and a new focus with localisation, as one proposal.
    let r = e
        .ops(
            &e.alice,
            p["projectionHash"].as_str().unwrap(),
            json!([
                {"op": "move", "focus_id": "exp_right", "x": 7, "y": 1},
                {"op": "set_cost", "focus_id": "exp_left", "cost": 9},
                {"op": "create", "focus_id": "exp_next", "x": 4, "y": 3, "cost": 3, "prerequisites": ["exp_join"],
                 "icon": "GFX_exp_icon_join", "title": "Next step", "description": "After the join.", "reward": "add_political_power = 15"}
            ]),
        )
        .await;
    assert_eq!(r.status, 201, "{}", r.text);
    let proposal = &r.body["proposal"];
    assert_eq!(proposal["actorKind"], "person");
    assert_eq!(proposal["state"], "applied");
    assert_eq!(proposal["toolOperation"], "hoi4.focus_rewrite");
    assert_eq!(proposal["changedIds"], json!(["exp_right", "exp_left", "exp_next"]));
    assert_eq!(
        proposal["changedPaths"],
        json!([SOURCE, "localisation/english/exp_basic_tree_l_english.yml"])
    );
    assert_eq!(proposal["summary"]["added"], json!(["exp_next"]));

    // Independent byte oracle: every byte outside the approved regions survives.
    let after = e.source();
    let expected = String::from_utf8(before.clone())
        .unwrap()
        .replacen("\t\tx = 6\n", "\t\tx = 7\n", 1)
        .replacen("\t\tcost = 5\n\t\tai_will_do = { factor = 2 }", "\t\tcost = 9\n\t\tai_will_do = { factor = 2 }", 1);
    let text = String::from_utf8(after.clone()).unwrap();
    assert!(text.starts_with(&expected[..expected.len() - 2]), "only the approved regions changed:\n{text}");
    assert!(text.contains("\tfocus = {\n\t\tid = exp_next\n"), "{text}");
    assert!(text.contains("completion_reward = { add_political_power = 15 }"), "{text}");
    assert!(!e.worktree().join("common/national_focus/exp_basic.focus-plan.json").exists(), "the sidecar is removed");
    let loc = std::fs::read(e.worktree().join("localisation/english/exp_basic_tree_l_english.yml")).unwrap();
    assert_eq!(loc, "\u{feff}l_english:\n exp_next:0 \"Next step\"\n exp_next_desc:0 \"After the join.\"\n".as_bytes());

    // The new projection shows the change, and the editor got a source_changed event.
    let p2 = &r.body["projection"];
    assert_ne!(p2["projectionHash"], p["projectionHash"]);
    let next = p2["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "exp_next").unwrap();
    assert_eq!(next["label"], "Next step");
    assert_eq!(next["prerequisiteGroups"], json!([["exp_join"]]));
    let ev = e.alice.client.get(&format!("{}/events?after=0&limit=100", e.base())).await;
    let changed: Vec<&Value> = ev.body["events"].as_array().unwrap().iter().filter(|x| x["type"] == "source_changed").collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["payload"]["changed_ids"], json!(["exp_right", "exp_left", "exp_next"]));
    let list = e.alice.client.get(&format!("{}/proposals", e.base())).await;
    assert_eq!(list.body["proposals"].as_array().unwrap().len(), 1);
    let diff = list.body["proposals"][0]["diffs"][0]["diff"].as_str().unwrap();
    assert!(diff.contains("-\t\tcost = 5") && diff.contains("+\t\tcost = 9"), "{diff}");
}

#[tokio::test]
async fn stale_unsupported_and_foreign_requests_write_nothing() {
    let Some(e) = editor().await else { return };
    let p = e.projection().await;
    let hash = p["projectionHash"].as_str().unwrap().to_owned();
    let before = e.source();

    // An operation outside the subset or with a bad value is refused whole.
    let r = e.ops(&e.alice, &hash, json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 4}, {"op": "move", "focus_id": "nope", "x": 1, "y": 1}])).await;
    assert_eq!(r.status, 422, "{}", r.text);
    assert_eq!(r.body["error"]["code"], "operation_refused");
    assert_eq!(e.source(), before);
    let r = e.ops(&e.alice, &hash, json!([{"op": "link", "focus_id": "exp_root", "prerequisites": ["exp_join"]}])).await;
    assert_eq!(r.status, 422, "a cycle is refused: {}", r.text);
    let r = e.ops(&e.alice, &hash, json!([{"op": "delete", "focus_id": "exp_root"}])).await;
    assert_eq!(r.status, 400, "an unknown operation never reaches the tool: {}", r.text);
    assert_eq!(e.source(), before);

    // An edit of the file outside the editor after the preview makes the old projection stale.
    let path = e.worktree().join(SOURCE);
    let edited = String::from_utf8(before.clone()).unwrap().replacen("cost = 7", "cost = 8", 1);
    std::fs::write(&path, &edited).unwrap();
    let r = e.ops(&e.alice, &hash, json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 4}])).await;
    assert_eq!(r.status, 409, "{}", r.text);
    assert_eq!(r.body["error"]["code"], "stale_base");
    assert_eq!(std::fs::read(&path).unwrap(), edited.as_bytes(), "the external edit survives");

    // A request based on an older projection is stale too, even after a refresh.
    let fresh = e.projection().await;
    assert_ne!(fresh["projectionHash"], json!(hash));
    let r = e.ops(&e.alice, &hash, json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 4}])).await;
    assert_eq!(r.status, 409, "{}", r.text);
    let ok = e.ops(&e.alice, fresh["projectionHash"].as_str().unwrap(), json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 4}])).await;
    assert_eq!(ok.status, 201, "{}", ok.text);

    // Another account sees nothing.
    let mallory = e.h.verified_user("mallory@example.org").await;
    let r = mallory.client.get(&format!("{}/focus/trees/exp_basic_tree", e.base())).await;
    assert_eq!(r.status, 404, "{}", r.text);
    let r = e.ops(&mallory, fresh["projectionHash"].as_str().unwrap(), json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 2}])).await;
    assert_eq!(r.status, 404, "{}", r.text);
    let r = mallory.client.get(&format!("{}/proposals", e.base())).await;
    assert_eq!(r.status, 404, "{}", r.text);
}

/// S04-W3, live: the native agent adds a small branch through focus_projection and focus_edit,
/// with no browser connected, on the maintainer's DeepSeek key through a probed OpenAI-compatible
/// profile (D-21, D-31). Real calls with real spend: run explicitly with `--ignored` and
/// `DEEPSEEK_API_KEY` set. The key is read from the environment and never printed.
#[tokio::test]
#[ignore = "live DeepSeek calls with real spend"]
async fn live_deepseek_agent_adds_a_branch_through_the_editor_path() {
    let Ok(key) = std::env::var("DEEPSEEK_API_KEY") else {
        eprintln!("SKIPPED: DEEPSEEK_API_KEY is not set");
        return;
    };
    let Some(install) = factory_hoi4_tools::AgentToolsInstall::from_env() else {
        eprintln!("SKIPPED: FACTORY_AGENT_TOOLS_NODE / FACTORY_AGENT_TOOLS_ENTRY are not set");
        return;
    };
    let key_dir = tempfile::tempdir().unwrap();
    let key_file = key_dir.path().join("secret.key");
    std::fs::write(&key_file, "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=").unwrap();
    let h = Harness::start(Options {
        extra_vars: vec![("FACTORY_SECRET_KEY_FILE", key_file.to_string_lossy().into_owned())],
        agent: AgentOptions {
            worker: Some(factory_server::agent::WorkerLauncher {
                program: PathBuf::from(env!("CARGO_BIN_EXE_factory-server")),
                args: vec!["agent-worker".into()],
                env: Default::default(),
            }),
            settings: Some(factory_server::agent::RunnerSettings {
                lease_ttl: Duration::from_secs(30),
                turn_timeout: Duration::from_secs(180),
                cancel_grace: Duration::from_secs(5),
                handshake_timeout: Duration::from_secs(20),
                safe_retries: 0,
            }),
            tools: Some(Some(factory_server::agent::AgentToolsSettings { install, game_root: None })),
            skip_recovery: true,
        },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Focus live").await;
    let up = alice
        .client
        .upload(&project, &[Part::text("kind", "zip"), Part::text("git_init_consent", "true"), Part::file("f.zip", build_zip(&fixture_entries()))])
        .await;
    assert_eq!(up.status, 201, "{}", up.text);

    let saved = alice
        .client
        .post("/api/v1/account/api-keys", json!({ "provider": "openai_compatible", "label": "DeepSeek", "api_key": key, "base_url": "https://api.deepseek.com" }))
        .await;
    assert_eq!(saved.status, 201, "{}", saved.text);
    let key_id = saved.body["id"].as_str().unwrap().to_owned();
    let probe = alice
        .client
        .post(&format!("/api/v1/account/api-keys/{key_id}/probe"), json!({ "mode": "inference", "model": "deepseek-flash", "context_tokens": 64000 }))
        .await;
    assert_eq!(probe.status, 200, "{}", probe.text);
    eprintln!("probe capabilities: {}", probe.body["capabilities"]);

    let objective = "In the national focus tree exp_basic_tree, add a short branch of three new focuses below exp_join. \
        Read the tree with focus_projection first, then add them with one focus_edit call: exp_reform_a at x 3 y 3 requiring exp_join, \
        exp_reform_b at x 5 y 3 requiring exp_join, and exp_reform_c at x 4 y 4 requiring exp_reform_a or exp_reform_b. \
        Give each a cost of 5, the icon GFX_exp_icon_join, a short English title and description, and a small reward such as add_political_power = 25. \
        Then finish.";
    let t = alice
        .client
        .post(
            &format!("/api/v1/projects/{project}/tasks"),
            json!({ "objective": objective, "mode": "edit", "allowed_paths": ["common/national_focus", "localisation"],
                    "budget_usd": 0.5, "model": { "profile_id": format!("user:{key_id}"), "key_id": key_id } }),
        )
        .await;
    assert_eq!(t.status, 201, "{}", t.text);
    let task = t.body["id"].as_str().unwrap().to_owned();
    let tasks = format!("/api/v1/projects/{project}/tasks");
    let start = alice.client.post(&format!("{tasks}/{task}/attempts"), json!({ "expected_version": t.body["version"] })).await;
    assert_eq!(start.status, 201, "{}", start.text);

    // No browser is connected: only the server drives the attempt.
    let started = std::time::Instant::now();
    let detail = loop {
        let d = alice.client.get(&format!("{tasks}/{task}")).await;
        let state = d.body["attempts"][0]["state"].as_str().unwrap_or("").to_owned();
        if matches!(state.as_str(), "succeeded" | "failed" | "cancelled" | "unknown_effect") {
            break d.body;
        }
        assert!(started.elapsed() < Duration::from_secs(900), "the attempt did not end: {:#}", d.body);
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    let events = alice.client.get(&format!("{tasks}/{task}/events?after=0&limit=1000")).await;
    let evidence_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.work/s04/live-w3");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    std::fs::write(evidence_dir.join("detail.json"), serde_json::to_vec_pretty(&detail).unwrap()).unwrap();
    std::fs::write(evidence_dir.join("events.json"), serde_json::to_vec_pretty(&events.body).unwrap()).unwrap();
    let proposals = alice.client.get(&format!("{tasks}/{task}/proposals")).await;
    std::fs::write(evidence_dir.join("proposals.json"), serde_json::to_vec_pretty(&proposals.body).unwrap()).unwrap();
    let usage = alice.client.get("/api/v1/account/usage").await;
    std::fs::write(evidence_dir.join("usage.json"), serde_json::to_vec_pretty(&usage.body).unwrap()).unwrap();
    eprintln!("attempt: {}", detail["attempts"][0]["state"]);
    eprintln!("elapsed: {:?}", started.elapsed());

    assert_eq!(detail["attempts"][0]["state"], "succeeded", "{detail:#}");
    let list = proposals.body["proposals"].as_array().unwrap();
    let agent_props: Vec<&Value> = list.iter().filter(|p| p["actorKind"] == "native_agent").collect();
    assert!(!agent_props.is_empty(), "the agent made no focus proposal: {:#}", proposals.body);
    let created: Vec<String> = agent_props
        .iter()
        .flat_map(|p| p["summary"]["added"].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    assert!((3..=5).contains(&created.len()), "created {created:?}");
    let worktree = h.data_dir.path().join("workspaces").join(&alice.id).join(&project).join("tasks").join(&task);
    let source = std::fs::read_to_string(worktree.join(SOURCE)).unwrap();
    for id in &created {
        assert!(source.contains(&format!("id = {id}\n")), "{id} missing from the source");
    }
    let loc = std::fs::read_to_string(worktree.join("localisation/english/exp_basic_tree_l_english.yml")).unwrap();
    for id in &created {
        assert!(loc.contains(&format!(" {id}:0 \"")) && loc.contains(&format!(" {id}_desc:0 \"")), "{id} has no localisation");
    }
    // The original focuses are untouched: the old bytes are a prefix up to the closing brace.
    let original = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/focus-small").join(SOURCE)).unwrap();
    assert!(source.starts_with(&original[..original.len() - 2]), "untouched bytes changed");
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git").args(["-c", "core.autocrlf=false"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// S04-W5 and gate 5, locally: a reviewed proposal is committed under the expected HEAD with only
/// its own paths, unrelated work in the worktree survives, a repeated request does not commit
/// twice, and restore undoes the commit while keeping other files as they are.
#[tokio::test]
async fn commit_binds_to_reviewed_proposals_and_restore_keeps_later_edits() {
    let Some(e) = editor().await else { return };
    let p = e.projection().await;
    let r = e.ops(&e.alice, p["projectionHash"].as_str().unwrap(), json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 9}])).await;
    assert_eq!(r.status, 201, "{}", r.text);
    let proposal = r.body["proposal"].clone();
    let wt = e.worktree();
    let head_before = git(&wt, &["rev-parse", "HEAD"]).trim().to_owned();
    // Unrelated work in the worktree that no proposal names.
    std::fs::write(wt.join("notes-unrelated.txt"), "keep me\n").unwrap();

    let check = e.alice.client.post(&format!("{}/check", e.base()), json!({})).await;
    assert_eq!(check.status, 200, "{}", check.text);
    assert_eq!(check.body["check"]["engine_rev"], factory_engine_check::ENGINE_REV);
    let verdict = check.body["check"]["verdict"].as_str().unwrap().to_owned();
    assert_ne!(verdict, "fail", "{:#}", check.body);

    let body = |hash: &str, key: &str, accept: bool| {
        json!({ "proposals": [{"id": proposal["id"], "proposal_hash": hash}], "message": "Make the left road longer", "idempotency_key": key, "accept_incomplete": accept })
    };
    // Approval is bound to the reviewed hash.
    let wrong = e.alice.client.post(&format!("{}/commit", e.base()), body(&"0".repeat(64), "c-1", true)).await;
    assert_eq!(wrong.status, 409, "{}", wrong.text);
    assert_eq!(wrong.body["error"]["code"], "proposal_changed");
    let hash = proposal["proposalHash"].as_str().unwrap();
    if verdict == "incomplete" {
        // Incomplete is not a pass: it needs the person's explicit acceptance.
        let r = e.alice.client.post(&format!("{}/commit", e.base()), body(hash, "c-0", false)).await;
        assert_eq!(r.status, 409, "{}", r.text);
        assert_eq!(r.body["error"]["code"], "check_incomplete");
    }
    let c = e.alice.client.post(&format!("{}/commit", e.base()), body(hash, "c-1", true)).await;
    assert_eq!(c.status, 201, "{}", c.text);
    let commit = c.body["result"]["commit"].as_str().unwrap().to_owned();
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]).trim(), commit);
    assert_eq!(git(&wt, &["rev-parse", "HEAD^"]).trim(), head_before);
    let files = git(&wt, &["show", "--name-only", "--format=", "HEAD"]);
    assert_eq!(files.trim(), "common/national_focus/exp_basic.txt", "only the proposal's path");
    let message = git(&wt, &["log", "-1", "--format=%B"]);
    assert!(message.contains(&format!("Factory-Publication: {}", c.body["publication_id"].as_str().unwrap())), "{message}");
    let author = git(&wt, &["log", "-1", "--format=%ae"]);
    assert!(author.trim().ends_with("@users.hoi4-factory.invalid"), "{author}");
    assert_eq!(std::fs::read_to_string(wt.join("notes-unrelated.txt")).unwrap(), "keep me\n");
    assert!(git(&wt, &["status", "--porcelain"]).contains("notes-unrelated.txt"), "the unrelated file stays uncommitted");

    // The same request again is a replay: one commit, the same answer.
    let again = e.alice.client.post(&format!("{}/commit", e.base()), body(hash, "c-1", true)).await;
    assert_eq!(again.status, 200, "{}", again.text);
    assert_eq!(again.body["result"]["commit"], json!(commit));
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]).trim(), commit);
    let list = e.alice.client.get(&format!("{}/proposals", e.base())).await;
    assert_eq!(list.body["proposals"][0]["state"], "committed");

    // Restore puts the focus file back and keeps every other file as it is.
    let source_committed = std::fs::read(wt.join(SOURCE)).unwrap();
    let rs = e
        .alice
        .client
        .post(&format!("{}/restore", e.base()), json!({ "publication_id": c.body["publication_id"], "idempotency_key": "r-1" }))
        .await;
    assert_eq!(rs.status, 201, "{}", rs.text);
    assert_eq!(rs.body["result"]["restored"], json!([SOURCE]));
    let restored = std::fs::read(wt.join(SOURCE)).unwrap();
    assert_ne!(restored, source_committed);
    assert_eq!(git(&wt, &["show", &format!("{head_before}:{SOURCE}")]).as_bytes(), restored.as_slice());
    assert_eq!(std::fs::read_to_string(wt.join("notes-unrelated.txt")).unwrap(), "keep me\n");
    let pubs = e.alice.client.get(&format!("{}/publications", e.base())).await;
    assert_eq!(pubs.body["publications"].as_array().unwrap().len(), 2);
}

/// Gate 8 and T-ENG-01: a commit that holds agent work with a new finding is blocked. Needs the
/// game folder for the engine's vocabulary (`HOI4_GAME_ROOT_FOR_TEST`).
#[tokio::test]
async fn a_new_engine_finding_blocks_an_agent_commit() {
    let Some(game) = std::env::var_os("HOI4_GAME_ROOT_FOR_TEST").map(PathBuf::from).filter(|p| p.is_dir()) else {
        eprintln!("SKIPPED: HOI4_GAME_ROOT_FOR_TEST is not set");
        return;
    };
    let Some(install) = factory_hoi4_tools::AgentToolsInstall::from_env() else { return };
    let h = Harness::start(Options {
        anthropic_key: Some("sk-ant-not-used"),
        agent: AgentOptions {
            worker: None,
            settings: None,
            tools: Some(Some(factory_server::agent::AgentToolsSettings { install, game_root: Some(game) })),
            skip_recovery: true,
        },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Engine gate").await;
    let up = alice
        .client
        .upload(&project, &[Part::text("kind", "zip"), Part::text("git_init_consent", "true"), Part::file("f.zip", build_zip(&fixture_entries()))])
        .await;
    assert_eq!(up.status, 201, "{}", up.text);
    let t = alice
        .client
        .post(&format!("/api/v1/projects/{project}/tasks"), json!({ "objective": "Engine gate", "mode": "edit", "allowed_paths": ["common/national_focus", "localisation"] }))
        .await;
    let task_id = t.body["id"].as_str().unwrap().to_owned();
    let base = format!("/api/v1/projects/{project}/tasks/{task_id}");
    let p = alice.client.get(&format!("{base}/focus/trees/exp_basic_tree")).await;
    assert_eq!(p.status, 200, "{}", p.text);

    // Agent work through the guarded focus path: Agent Tools refuses a reward that calls a helper
    // nothing defines, and nothing is written.
    let rt = h.state.agent.clone().unwrap();
    let account = factory_accounts::AccountId(alice.id.parse().unwrap());
    let mut tx = rt.tx(account).await.unwrap();
    let task = factory_tasks::get_task(&mut tx, factory_tasks::TaskId(task_id.parse().unwrap())).await.unwrap();
    tx.commit().await.unwrap();
    let keys = factory_server::focus::store::TaskKeys::of(&task, account);
    let wt = h.data_dir.path().join("workspaces").join(&alice.id).join(&project).join("tasks").join(&task_id);
    let before = std::fs::read(wt.join(SOURCE)).unwrap();
    let refused = factory_server::focus::apply::apply_operations(
        &rt,
        &keys,
        &wt,
        factory_server::focus::store::ActorKind::CodingClient,
        "exp_basic_tree",
        None,
        serde_json::from_value(json!([{"op": "create", "focus_id": "exp_bad", "x": 4, "y": 3, "cost": 5, "prerequisites": ["exp_join"],
            "title": "Bad", "description": "Calls an unknown effect.", "reward": "ecf_unknown_scripted_effect = yes"}]))
        .unwrap(),
    )
    .await;
    match refused {
        Err(e) => assert_eq!(e.code(), "tool_refused", "{e:?}"),
        Ok(a) => panic!("the guarded rewrite applied: {:#}", a.proposal),
    }
    assert_eq!(std::fs::read(wt.join(SOURCE)).unwrap(), before, "nothing was written");
    assert!(!wt.join("localisation/english/exp_basic_tree_l_english.yml").exists(), "no localisation for a refused focus");

    // Agent work written directly with write_file becomes a file proposal at the end of the
    // attempt. This one adds a tree whose focus requires an undefined scripted trigger.
    std::fs::write(
        wt.join("common/national_focus/agent_extra.txt"),
        "focus_tree = {\n\tid = agent_extra_tree\n\tcountry = { factor = 0 }\n\tfocus = {\n\t\tid = AGX_one\n\t\tx = 1\n\t\ty = 0\n\t\tcost = 10\n\t\tavailable = { agx_unknown_trigger = yes }\n\t\tcompletion_reward = { add_political_power = 10 }\n\t}\n}\n",
    )
    .unwrap();
    let applied = factory_server::focus::apply::record_file_proposal(
        &rt,
        &keys,
        &wt,
        factory_server::focus::store::ActorKind::CodingClient,
        &["common/national_focus/agent_extra.txt".to_owned()],
    )
    .await
    .unwrap()
    .expect("a file proposal");
    assert_eq!(applied["toolOperation"], "write_file");
    let hash = applied["proposalHash"].as_str().unwrap();
    let id = applied["id"].clone();
    let head = git(&wt, &["rev-parse", "HEAD"]);
    let c = alice
        .client
        .post(&format!("{base}/commit"), json!({ "proposals": [{"id": id, "proposal_hash": hash}], "message": "Agent branch", "idempotency_key": "a-1", "accept_incomplete": true }))
        .await;
    assert_eq!(c.status, 409, "{}", c.text);
    assert_eq!(c.body["error"]["code"], "commit_blocked", "{:#}", c.body);
    let findings = c.body["error"]["details"]["findings"].to_string();
    assert!(findings.contains("REF_UNKNOWN_TRIGGER") && findings.contains("agx_unknown_trigger"), "{findings}");
    assert_eq!(git(&wt, &["rev-parse", "HEAD"]), head, "nothing was committed");
}

/// T-EDIT-01 for focus creation: the operations the native agent sent in the live DeepSeek run
/// (`evidence/S04/live-w3/proposals.json`, recorded real output), replayed as a person's gesture
/// and as a coding client's raw `hoi4.focus_rewrite` call on separate copies of F-FOCUS-SMALL,
/// give byte-identical focus files and the same projection of the tree.
#[tokio::test]
async fn gesture_agent_and_coding_client_paths_give_identical_bytes() {
    let Some(e) = editor().await else { return };
    let recorded: Value = serde_json::from_slice(
        &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/S04/live-w3/proposals.json")).unwrap(),
    )
    .unwrap();
    let agent = &recorded["proposals"][0];
    assert_eq!(agent["actorKind"], "native_agent");
    let ops = agent["operations"].clone();
    let agent_source = agent["after"][SOURCE].as_str().unwrap().to_owned();
    let agent_loc = agent["after"]["localisation/english/exp_basic_tree_l_english.yml"].as_str().unwrap().to_owned();
    // The agent started from the fixture's exact bytes.
    let fixture_source = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/focus-small").join(SOURCE)).unwrap();
    assert_eq!(agent["before"][SOURCE].as_str().unwrap(), factory_server::focus::session::sha256_hex(&fixture_source));

    // 1. The gesture path: the same operations through the editor route.
    let p = e.projection().await;
    let r = e.ops(&e.alice, p["projectionHash"].as_str().unwrap(), ops.clone()).await;
    assert_eq!(r.status, 201, "{}", r.text);
    let gesture_source = factory_server::focus::session::sha256_hex(&e.source());
    let gesture_loc = factory_server::focus::session::sha256_hex(&std::fs::read(e.worktree().join("localisation/english/exp_basic_tree_l_english.yml")).unwrap());
    assert_eq!(gesture_source, agent_source, "gesture and native agent give the same focus file");
    assert_eq!(gesture_loc, agent_loc, "and the same localisation");

    // 2. The coding-client path: a separate Agent Tools server on a clone, called directly with
    // the request the host builds for these operations, as a local Claude Code or Codex session
    // with the same tool profile would send it.
    let install = factory_hoi4_tools::AgentToolsInstall::from_env().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let clone = base.join("mod");
    for entry in fixture_entries() {
        let path = clone.join(String::from_utf8(entry.name.clone()).unwrap());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &entry.data).unwrap();
    }
    let settings = factory_server::agent::AgentToolsSettings { install: install.clone(), game_root: None };
    let mut host = factory_server::focus::session::FocusSession::start(&settings, &clone, &base.join("host-state")).await.unwrap();
    let built = host.inspect("exp_basic_tree").await.unwrap();
    let parsed: Vec<factory_server::focus::ops::FocusOp> = serde_json::from_value(ops.clone()).unwrap();
    let (request, _) = factory_server::focus::session::FocusSession::request(&built, &parsed).unwrap();
    host.shutdown().await;
    let state = factory_hoi4_tools::prepare_state_dir(&base.join("client-state")).unwrap();
    let spec = factory_hoi4_tools::agent_tools_spec(&install, &clone, &state, None);
    let (client, _) = factory_hoi4_tools::McpClient::start(&spec).await.unwrap();
    let out = client.call_tool("hoi4.focus_rewrite", request, Some(Duration::from_secs(240))).await.unwrap();
    assert!(!out.is_error, "{:?}", out.structured);
    client.shutdown().await;
    let client_source = factory_server::focus::session::sha256_hex(&std::fs::read(clone.join(SOURCE)).unwrap());
    assert_eq!(client_source, agent_source, "the coding-client call gives the same focus file");

    // The projections agree on the tree's structure.
    let shape = |p: &Value| -> Vec<Value> {
        let mut nodes: Vec<Value> = p["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| json!([n["id"], n["x"], n["y"], n["cost"], n["prerequisiteGroups"], n["editable"]]))
            .collect();
        nodes.sort_by_key(|v| v.to_string());
        nodes
    };
    let mut host2 = factory_server::focus::session::FocusSession::start(&settings, &clone, &base.join("host-state-2")).await.unwrap();
    let client_projection = host2.inspect("exp_basic_tree").await.unwrap();
    host2.shutdown().await;
    assert_eq!(shape(&client_projection.projection), shape(&r.body["projection"]));
}

/// W4: a pending proposal can be discarded on the canvas. Its files return to their exact earlier
/// bytes, but only while nothing else changed them since.
#[tokio::test]
async fn an_uncommitted_proposal_is_discarded_exactly_or_not_at_all() {
    let Some(e) = editor().await else { return };
    let p = e.projection().await;
    let original = e.source();
    let r = e
        .ops(
            &e.alice,
            p["projectionHash"].as_str().unwrap(),
            json!([{"op": "create", "focus_id": "exp_tmp", "x": 4, "y": 3, "cost": 2, "prerequisites": ["exp_join"], "icon": "GFX_exp_icon_join", "title": "Temporary", "description": "Will be discarded.", "reward": "add_political_power = 5"}]),
        )
        .await;
    assert_eq!(r.status, 201, "{}", r.text);
    let id = r.body["proposal"]["id"].as_str().unwrap().to_owned();
    let loc = e.worktree().join("localisation/english/exp_basic_tree_l_english.yml");
    assert!(loc.exists());

    let d = e.alice.client.post(&format!("{}/proposals/{id}/discard", e.base()), json!({})).await;
    assert_eq!(d.status, 200, "{}", d.text);
    assert_eq!(e.source(), original, "the focus file is byte-identical to before");
    assert!(!loc.exists(), "the localisation file the proposal created is gone");
    let again = e.alice.client.post(&format!("{}/proposals/{id}/discard", e.base()), json!({})).await;
    assert_eq!(again.status, 422, "{}", again.text);

    // A proposal whose file was changed since is not discarded, and nothing is written.
    let p = e.projection().await;
    let r = e.ops(&e.alice, p["projectionHash"].as_str().unwrap(), json!([{"op": "set_cost", "focus_id": "exp_left", "cost": 9}])).await;
    assert_eq!(r.status, 201, "{}", r.text);
    let id = r.body["proposal"]["id"].as_str().unwrap().to_owned();
    let edited = String::from_utf8(e.source()).unwrap().replacen("cost = 7", "cost = 8", 1);
    std::fs::write(e.worktree().join(SOURCE), &edited).unwrap();
    let refused = e.alice.client.post(&format!("{}/proposals/{id}/discard", e.base()), json!({})).await;
    assert_eq!(refused.status, 409, "{}", refused.text);
    assert_eq!(e.source(), edited.as_bytes(), "the later edit survives");
}
