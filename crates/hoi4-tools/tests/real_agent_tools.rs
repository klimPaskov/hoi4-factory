//! Integration tests against the real HOI4 Agent Tools server.
//!
//! Gated on `FACTORY_AGENT_TOOLS_NODE`, `FACTORY_AGENT_TOOLS_ENTRY` and
//! `FACTORY_AGENT_TOOLS_FIXTURE` (a mod folder, copied into a temp dir first). Without them the
//! tests print a note and pass, so CI without Node still works. Set `HOI4_GAME_ROOT_FOR_TEST`
//! to also pass a game folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use factory_hoi4_tools::profile::is_closed_schema;
use factory_hoi4_tools::{
    AgentToolsInstall, McpClient, ToolSession, ToolState, agent_tools_read_only, agent_tools_spec,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Env {
    install: AgentToolsInstall,
    fixture: PathBuf,
}

fn env() -> Option<Env> {
    let node = std::env::var_os("FACTORY_AGENT_TOOLS_NODE");
    let entry = std::env::var_os("FACTORY_AGENT_TOOLS_ENTRY");
    let fixture = std::env::var_os("FACTORY_AGENT_TOOLS_FIXTURE");
    match (node, entry, fixture) {
        (Some(n), Some(e), Some(f)) => Some(Env {
            install: AgentToolsInstall {
                node: n.into(),
                entry: e.into(),
            },
            fixture: f.into(),
        }),
        _ => {
            eprintln!(
                "SKIPPED: set FACTORY_AGENT_TOOLS_NODE, FACTORY_AGENT_TOOLS_ENTRY and FACTORY_AGENT_TOOLS_FIXTURE to run the real Agent Tools tests"
            );
            None
        }
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

fn hash_tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("type").is_dir() {
                walk(base, &path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read");
                let rel = path
                    .strip_prefix(base)
                    .expect("rel")
                    .to_string_lossy()
                    .into_owned();
                let digest = Sha256::digest(&bytes);
                out.insert(rel, digest.iter().map(|b| format!("{b:02x}")).collect());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

struct Workspace {
    _dir: tempfile::TempDir,
    mod_root: PathBuf,
    state: PathBuf,
}

fn workspace(env: &Env) -> Workspace {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().canonicalize().expect("canonical temp dir");
    let mod_root = base.join("mod");
    copy_dir(&env.fixture, &mod_root);
    let state = factory_hoi4_tools::prepare_state_dir(&base.join("state")).expect("state");
    Workspace {
        _dir: dir,
        mod_root,
        state,
    }
}

fn game_root() -> Option<PathBuf> {
    std::env::var_os("HOI4_GAME_ROOT_FOR_TEST").map(PathBuf::from)
}

fn first_hit(value: &Value) -> Option<(String, String)> {
    // Look for any object with a 64-hex `id` and `revision` anywhere in the structure.
    match value {
        Value::Object(map) => {
            if let (Some(Value::String(id)), Some(Value::String(rev))) =
                (map.get("id"), map.get("revision"))
                && id.len() == 64
                && rev.len() == 64
            {
                return Some((id.clone(), rev.clone()));
            }
            map.values().find_map(first_hit)
        }
        Value::Array(items) => items.iter().find_map(first_hit),
        _ => None,
    }
}

#[tokio::test]
async fn real_server_discovery_schemas_and_read_only_calls() {
    let Some(env) = env() else { return };
    let ws = workspace(&env);
    let before = hash_tree(&ws.mod_root);
    assert!(!before.is_empty());
    let game = game_root();
    let spec = agent_tools_spec(&env.install, &ws.mod_root, &ws.state, game.as_deref());
    let profile = agent_tools_read_only();

    // Every host schema is closed and names only arguments the real tool also declares.
    let (probe, hs) = McpClient::start(&spec).await.expect("start");
    eprintln!(
        "handshake: {} {} protocol {}",
        hs.server_name, hs.server_version, hs.protocol_version
    );
    let real = probe.list_tools().await.expect("list");
    eprintln!("server lists {} tools", real.len());
    for pt in &profile.tools {
        assert!(is_closed_schema(&pt.host_schema), "{} is open", pt.mcp_name);
        let found = real
            .iter()
            .find(|t| t.name == pt.mcp_name)
            .unwrap_or_else(|| panic!("{} missing", pt.mcp_name));
        eprintln!("schema {} {}", pt.mcp_name, found.schema_sha256);
        let real_props = &found.input_schema["properties"];
        for key in pt.host_schema["properties"]
            .as_object()
            .expect("props")
            .keys()
        {
            assert!(
                real_props.get(key).is_some(),
                "{}: host property {key} is not a real argument",
                pt.mcp_name
            );
        }
    }
    probe.shutdown().await;

    let mut session = ToolSession::start(spec, profile.clone()).await;
    let inv = session.inventory().clone();
    assert!(inv.is_healthy(), "{:?}", inv.health);
    assert!(inv.version_matches);
    assert_eq!(
        inv.server.as_ref().map(|s| s.server_version.as_str()),
        Some("4.4.0")
    );
    for t in &inv.tools {
        assert_eq!(
            t.state,
            ToolState::Available,
            "{} not available",
            t.mcp_name
        );
    }
    assert_eq!(session.exposed_tools().len(), 9);
    eprintln!("startup_ms: {:?}", inv.health);

    let calls: Vec<(&str, Value)> = vec![
        (
            "agent_tools_source_lookup",
            json!({"symbol": "synthetic_focus_reward_effect"}),
        ),
        ("agent_tools_mod_index", json!({"mode": "overview"})),
        (
            "agent_tools_script_validate",
            json!({"source": "add_political_power = 10", "kind": "effect", "scope": "country"}),
        ),
        (
            "agent_tools_reference_search",
            json!({"query": "add_timed_idea"}),
        ),
        (
            "agent_tools_reference_context",
            json!({"surface": "event", "question": "fire an event after a delay"}),
        ),
        (
            "agent_tools_focus_inspect",
            json!({"treeId": "synthetic_acceptance_tree"}),
        ),
        ("agent_tools_event_inspect", json!({"mode": "scan"})),
        ("agent_tools_decision_inspect", json!({"mode": "inventory"})),
    ];
    let mut search_hit = None;
    for (name, args) in &calls {
        let deadline = Instant::now() + Duration::from_secs(180);
        let r = session
            .call(name, args, deadline)
            .await
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        eprintln!(
            "CALL {name}: is_error={} bytes={} truncated={} attempts={} ms={}",
            r.output.is_error, r.output.bytes, r.output.truncated, r.attempts, r.elapsed_ms
        );
        eprintln!(
            "  content: {}",
            serde_json::to_string(&r.output.content)
                .unwrap_or_default()
                .chars()
                .take(350)
                .collect::<String>()
        );
        eprintln!(
            "  structured: {}",
            serde_json::to_string(&r.output.structured)
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect::<String>()
        );
        if r.output.is_error {
            eprintln!(
                "  error content: {}",
                serde_json::to_string(&r.output.content)
                    .unwrap_or_default()
                    .chars()
                    .take(400)
                    .collect::<String>()
            );
        }
        if *name == "agent_tools_reference_search" {
            search_hit = r.output.structured.as_ref().and_then(first_hit);
        }
        // An argument or protocol problem would be a ServerError (an Err above) or an isError
        // result mentioning invalid arguments.
        let text = serde_json::to_string(&r.output.content).unwrap_or_default();
        assert!(
            !text.contains("Invalid arguments") && !text.contains("INVALID_ARGUMENT"),
            "{name}: {text}"
        );
    }
    match search_hit {
        Some((id, revision)) => {
            let r = session
                .call(
                    "agent_tools_reference_read",
                    &json!({"id": id, "revision": revision}),
                    Instant::now() + Duration::from_secs(60),
                )
                .await
                .expect("reference_read");
            eprintln!(
                "CALL agent_tools_reference_read: is_error={} bytes={}",
                r.output.is_error, r.output.bytes
            );
            assert!(!r.output.is_error);
        }
        None => {
            // Without the game folder the search has no sections. An unknown id must still be a
            // tool result, not a protocol or argument failure.
            let r = session
                .call(
                    "agent_tools_reference_read",
                    &json!({"id": "0".repeat(64), "revision": "0".repeat(64)}),
                    Instant::now() + Duration::from_secs(60),
                )
                .await;
            eprintln!(
                "CALL agent_tools_reference_read (made-up id): {:?}",
                r.as_ref()
                    .map(|r| (r.output.is_error, r.output.bytes))
                    .map_err(|e| e.to_string())
            );
            assert!(r.is_ok());
        }
    }
    let records = session.take_records();
    assert!(records.iter().all(|r| r.attempts.len() == 1));
    session.shutdown().await;

    let after = hash_tree(&ws.mod_root);
    assert_eq!(before, after, "the read-only profile changed mod source");
    eprintln!(
        "mod tree: {} files byte-identical before and after",
        after.len()
    );
}

#[tokio::test]
async fn real_server_unavailable_when_not_a_mod() {
    let Some(env) = env() else { return };
    let dir = tempfile::tempdir().expect("tempdir");
    let empty = dir.path().join("not-a-mod");
    std::fs::create_dir_all(&empty).expect("mkdir");
    let state = factory_hoi4_tools::prepare_state_dir(&dir.path().join("state")).expect("state");
    let mut spec = agent_tools_spec(&env.install, &empty, &state, None);
    spec.startup_timeout = Duration::from_secs(20);
    let inv = factory_hoi4_tools::discover(&spec, &agent_tools_read_only()).await;
    eprintln!("not-a-mod health: {:?}", inv.health);
    assert!(!inv.is_healthy());
    assert_eq!(inv.available_count(), 0);
}
