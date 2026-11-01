//! Records the real Anthropic streams the agent tests replay (`tests/fixtures/recorded/agent-*`).
//!
//! Ignored: it calls the live API with `ANTHROPIC_API_KEY` (pre-approved development spend for
//! `claude-haiku-5-5`, a few hundred tokens per turn). Run it only to re-record:
//!
//! ```sh
//! cargo test -p factory-server --test agent_record -- --ignored --nocapture
//! ```
//!
//! Each request uses the agent's real system prompt and the real built-in tool schemas, so the
//! recorded tool calls are what the model produces for this host. The tests replay the bytes in
//! order regardless of request content (evidence kind: recorded-real replay). Only status and
//! body bytes are kept; no request or organization header is stored.

use std::io::Write;
use std::path::PathBuf;

use factory_agent::client::ProviderClient;
use factory_agent::destination::DestinationPolicy;
use factory_agent::openai::ApiKey;
use factory_agent::profile::ModelProfile;
use factory_agent::request::{Message, ModelRequest, Part, Role, ToolSpec};
use factory_agent::transport::Tee;
use factory_agent::types::Turn;
use factory_server::agent::runner::SYSTEM_PROMPT;
use factory_server::agent::workspace_tools::schemas;
use serde_json::{Value, json};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorded")
}

fn tools(write: bool) -> Vec<ToolSpec> {
    schemas()
        .into_iter()
        .filter(|(name, _, _)| write || *name != "write_file")
        .map(|(name, description, schema)| ToolSpec {
            name: name.to_owned(),
            description: description.to_owned(),
            input_schema: schema,
        })
        .collect()
}

struct FileSink(std::fs::File);
impl Write for FileSink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

async fn record(name: &str, request: &ModelRequest) -> Turn {
    let key = ApiKey::from_env("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is set");
    let profile = ModelProfile::anthropic_haiku();
    let client = ProviderClient::new(profile.clone(), key, &DestinationPolicy::public_only()).unwrap();
    std::fs::create_dir_all(dir()).unwrap();
    let file = std::fs::File::create(dir().join(format!("{name}.sse"))).unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let turn = client
        .run_turn(request, rx, Some(Tee::new(Box::new(FileSink(file)))))
        .await
        .expect("the live turn completes");
    let meta = json!({
        "name": name,
        "evidence_kind": "live",
        "captured_utc": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "endpoint": "https://api.anthropic.com/v1/messages",
        "model_requested": profile.model,
        "model_reported": turn.model,
        "anthropic_version": factory_agent::anthropic::API_VERSION,
        "request_summary": {
            "system": "factory_server::agent::runner::SYSTEM_PROMPT",
            "tools": request.tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
            "messages": request.messages.len(),
        },
        "outcome": { "finish_reason": turn.finish_reason, "tool_calls": turn.tool_calls, "usage": turn.usage, "response_id": turn.response_id },
        "redaction": "x-api-key is never recorded; only the response body bytes are kept.",
    });
    std::fs::write(
        dir().join(format!("{name}.meta.json")),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();
    println!("{name}: {:?} {:?}", turn.finish_reason, turn.usage);
    turn
}

fn tool_use_message(turn: &Turn) -> Message {
    Message {
        role: Role::Assistant,
        parts: turn
            .tool_calls
            .iter()
            .map(|c| Part::ToolUse { id: c.id.clone(), name: c.name.clone(), input: c.arguments.clone() })
            .collect(),
    }
}

fn result_message(turn: &Turn, content: Value) -> Message {
    Message {
        role: Role::User,
        parts: turn
            .tool_calls
            .iter()
            .map(|c| Part::ToolResult { tool_use_id: c.id.clone(), content: content.to_string(), is_error: false })
            .collect(),
    }
}

#[tokio::test]
#[ignore = "calls the live Anthropic API"]
async fn record_agent_streams() {
    // An edit task: one write under the allowed path, then the final report.
    let write_task = "Task: Create the file common/ideas/agent_test.txt containing exactly the line \
        `# written by the HOI4 Factory agent test` followed by a newline. Allowed paths: common/ideas. \
        The file does not exist yet, so use expected_sha256 \"absent\". Do not read or list anything \
        first. After the write succeeds, call finish.";
    let mut req = ModelRequest {
        system: SYSTEM_PROMPT.to_owned(),
        messages: vec![Message::user_text(write_task)],
        tools: tools(true),
        max_output_tokens: 1024,
    };
    let write = record("agent-write-turn", &req).await;
    assert_eq!(write.tool_calls.first().map(|c| c.name.as_str()), Some("write_file"));
    let content = write.tool_calls[0].arguments["content"].as_str().unwrap_or("").to_owned();
    let sha = factory_server::agent::workspace_tools::sha256_hex(content.as_bytes());
    req.messages.push(tool_use_message(&write));
    req.messages.push(result_message(&write, json!({ "path": "common/ideas/agent_test.txt", "written": true, "sha256": sha })));
    let finish = record("agent-finish-turn", &req).await;
    assert_eq!(finish.tool_calls.first().map(|c| c.name.as_str()), Some("finish"));

    // A write outside the allowed path, for the zero-write gate.
    let outside = ModelRequest {
        system: SYSTEM_PROMPT.to_owned(),
        messages: vec![Message::user_text(
            "Task: Create the file events/outside.txt containing the line `outside`. The file does not \
             exist, so use expected_sha256 \"absent\". Do not read or list anything first.",
        )],
        tools: tools(true),
        max_output_tokens: 1024,
    };
    let o = record("agent-write-outside-turn", &outside).await;
    assert_eq!(o.tool_calls.first().map(|c| c.name.as_str()), Some("write_file"));

    // A read-only task that reads one file and reports.
    let read = ModelRequest {
        system: SYSTEM_PROMPT.to_owned(),
        messages: vec![Message::user_text(
            "Task: Read descriptor.mod with read_file and nothing else. Read-only task.",
        )],
        tools: tools(false),
        max_output_tokens: 1024,
    };
    let r = record("agent-read-turn", &read).await;
    assert_eq!(r.tool_calls.first().map(|c| c.name.as_str()), Some("read_file"));
}

/// A read-only task through the real HOI4 Agent Tools profile: the model calls
/// `agent_tools_mod_index`, the real tool answers (run here against the Agent Tools fixture mod),
/// and the model finishes. Needs `FACTORY_AGENT_TOOLS_NODE`, `FACTORY_AGENT_TOOLS_ENTRY` and
/// `FACTORY_AGENT_TOOLS_FIXTURE`, like the hoi4-tools integration tests.
#[tokio::test]
#[ignore = "calls the live Anthropic API and needs a local Agent Tools build"]
async fn record_agent_tools_streams() {
    let install = factory_hoi4_tools::AgentToolsInstall::from_env().expect("FACTORY_AGENT_TOOLS_NODE and _ENTRY");
    let fixture = std::env::var("FACTORY_AGENT_TOOLS_FIXTURE").expect("FACTORY_AGENT_TOOLS_FIXTURE");
    let work = tempfile::tempdir().unwrap();
    let mod_root = work.path().join("mod");
    copy_dir(std::path::Path::new(&fixture), &mod_root);
    let state = factory_hoi4_tools::prepare_state_dir(&work.path().join("state")).unwrap();
    let spec = factory_hoi4_tools::agent_tools_spec(&install, &mod_root, &state, None);
    let mut session = factory_hoi4_tools::ToolSession::start(spec, factory_hoi4_tools::agent_tools_read_only()).await;
    assert!(session.inventory().is_healthy());
    let mut specs = tools(false);
    for t in session.exposed_tools() {
        specs.push(ToolSpec { name: t.name, description: t.description, input_schema: t.input_schema });
    }
    let mut req = ModelRequest {
        system: SYSTEM_PROMPT.to_owned(),
        messages: vec![Message::user_text(
            "Task: Summarize what content this mod defines. Call agent_tools_mod_index once with mode \
             \"overview\", then call finish with a one-sentence summary. Read-only task; do nothing else.",
        )],
        tools: specs,
        max_output_tokens: 1024,
    };
    let call = record("agent-tools-call-turn", &req).await;
    assert_eq!(call.tool_calls.first().map(|c| c.name.as_str()), Some("agent_tools_mod_index"));
    let result = session
        .call("agent_tools_mod_index", &call.tool_calls[0].arguments, std::time::Instant::now() + std::time::Duration::from_secs(120))
        .await
        .expect("the real tool answers");
    let body = result.output.structured.clone().unwrap_or(json!({ "content": result.output.content }));
    req.messages.push(tool_use_message(&call));
    req.messages.push(result_message(&call, body));
    let finish = record("agent-tools-finish-turn", &req).await;
    assert_eq!(finish.tool_calls.first().map(|c| c.name.as_str()), Some("finish"));
    let _ = session.shutdown().await;
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}
