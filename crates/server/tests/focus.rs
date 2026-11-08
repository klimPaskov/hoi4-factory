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
