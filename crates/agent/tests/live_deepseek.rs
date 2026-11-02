//! Live OpenAI-compatible calls against DeepSeek (maintainer decision D-21, issue #2).
//! Ignored by default.
//!
//! DeepSeek speaks the Chat Completions protocol, so it exercises the same
//! `openai-compatible` profile and adapter a self-hosted endpoint uses. It does
//! not prove the `openai:gpt-5.4-nano` profile, which still needs a funded OpenAI
//! key.
//!
//! Run, one test at a time so the calls and their timestamps stay in order:
//!
//! ```text
//! DEEPSEEK_API_KEY=... DEEPSEEK_EVIDENCE_DIR=<dir> \
//!   cargo test -p factory-agent --test live_deepseek -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! The key is read with `ApiKey::from_env`, which redacts it in Debug output, and
//! is never printed. `DEEPSEEK_MODEL` overrides the model (default `deepseek-flash`,
//! the name DeepSeek's pricing page and model listing use on 2026-11-02).
//! `DEEPSEEK_EVIDENCE_DIR`, when set, receives for each recorded call the exact
//! request body, the response bytes unchanged (`.sse`) and an outcome file with
//! usage and the cost computed from DeepSeek's published prices. Response bytes
//! are recorded-real. Nothing in them is a credential.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use common::*;
use factory_agent::client::ProviderClient;
use factory_agent::destination::{DestinationPolicy, outbound_client};
use factory_agent::openai::{self, ApiKey, Control, OpenAiClient};
use factory_agent::probe::{ProbeMode, now_rfc3339, probe};
use factory_agent::profile::{Basis, Capability, CapabilityState, ModelProfile};
use factory_agent::request::{Message, ModelRequest, Part, Role};
use factory_agent::transport::{DEFAULT_MAX_RESPONSE_BYTES, Tee};
use factory_agent::types::{HttpClass, Turn, TurnError, Usage};
use serde_json::{Value, json};

const BASE_URL: &str = "https://api.deepseek.com";

fn key() -> ApiKey {
    ApiKey::from_env("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY is set")
}

fn model() -> String {
    std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-flash".into())
}

fn profile_for(model: &str) -> ModelProfile {
    ModelProfile::openai_compatible(BASE_URL, model)
}

fn profile() -> ModelProfile {
    profile_for(&model())
}

/// DeepSeek prices in nano-dollars per token (USD per million tokens times 1000),
/// from https://api-docs.deepseek.com/quick_start/pricing, read 2026-11-02.
/// Off-peak rates are half the peak rates. Peak is 01:00-04:00 and 06:00-10:00 UTC
/// on weekdays.
struct Rate {
    input_cache_miss: u64,
    input_cache_hit: u64,
    output: u64,
}

const DEEPSEEK_FLASH_OFF_PEAK: Rate = Rate {
    input_cache_miss: 150,
    input_cache_hit: 3,
    output: 600,
};
const DEEPSEEK_FLASH_PEAK: Rate = Rate {
    input_cache_miss: 300,
    input_cache_hit: 6,
    output: 1200,
};

fn nano_dollars(u: &Usage, r: &Rate) -> u64 {
    let hit = u.cached_input_tokens.min(u.input_tokens);
    (u.input_tokens - hit) * r.input_cache_miss
        + hit * r.input_cache_hit
        + u.output_tokens * r.output
}

fn cost_json(u: Option<&Usage>) -> Value {
    let Some(u) = u else {
        return json!({"kind": "usage_unknown"});
    };
    let usd = |n: u64| n as f64 / 1e9;
    json!({
        "kind": "estimated",
        "model_priced_as": "deepseek-flash",
        "usd_off_peak": usd(nano_dollars(u, &DEEPSEEK_FLASH_OFF_PEAK)),
        "usd_peak": usd(nano_dollars(u, &DEEPSEEK_FLASH_PEAK)),
        "source": "https://api-docs.deepseek.com/quick_start/pricing (read 2026-11-02)",
    })
}

/// A tee sink that keeps every byte and the size of every write, which is the
/// size of every chunk the transport read.
#[derive(Clone, Default)]
struct Recorder {
    bytes: Arc<Mutex<Vec<u8>>>,
    chunk_sizes: Arc<Mutex<Vec<usize>>>,
}

impl Write for Recorder {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.bytes.lock().unwrap().extend_from_slice(b);
        self.chunk_sizes.lock().unwrap().push(b.len());
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Recorder {
    fn tee(&self) -> Tee {
        Tee::new(Box::new(self.clone()))
    }
}

fn evidence_dir() -> Option<std::path::PathBuf> {
    std::env::var("DEEPSEEK_EVIDENCE_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
}

fn turn_json(r: &Result<Turn, TurnError>) -> Value {
    match r {
        Ok(t) => json!({"ok": true, "turn": t}),
        Err(e) => json!({
            "ok": false,
            "class": e.class(),
            "normalized": e.normalized(),
            "http": match e { TurnError::Http(h) => serde_json::to_value(h).unwrap_or(Value::Null), _ => Value::Null },
        }),
    }
}

/// Save one recorded call when an evidence directory is given. The request body
/// is the exact JSON the adapter sent. The bytes are the unchanged response body.
fn save(
    name: &str,
    started: &str,
    request_body: &Value,
    rec: &Recorder,
    result: &Result<Turn, TurnError>,
) {
    let usage = match result {
        Ok(t) => t.usage.clone(),
        Err(_) => None,
    };
    println!(
        "[{name}] {started} {}",
        serde_json::to_string(&turn_json(result)).unwrap()
    );
    println!("[{name}] cost {}", cost_json(usage.as_ref()));
    let Some(dir) = evidence_dir() else { return };
    std::fs::create_dir_all(&dir).expect("evidence dir");
    let bytes = rec.bytes.lock().unwrap().clone();
    let sizes = rec.chunk_sizes.lock().unwrap().clone();
    let write =
        |file: String, data: &[u8]| std::fs::write(dir.join(file), data).expect("write evidence");
    write(
        format!("{name}.request.json"),
        serde_json::to_string_pretty(request_body)
            .unwrap()
            .as_bytes(),
    );
    if !bytes.is_empty() {
        write(format!("{name}.sse"), &bytes);
    }
    let outcome = json!({
        "name": name,
        "captured_utc": started,
        "evidence_kind": "live",
        "endpoint": format!("{BASE_URL}/chat/completions"),
        "response_bytes": bytes.len(),
        "chunk_sizes": sizes,
        "outcome": turn_json(result),
        "cost": cost_json(usage.as_ref()),
    });
    write(
        format!("{name}.outcome.json"),
        serde_json::to_string_pretty(&outcome).unwrap().as_bytes(),
    );
}

/// Run one turn through the production `ProviderClient` (destination policy,
/// no redirects, no proxy) and record it.
async fn recorded_turn(
    name: &str,
    profile: ModelProfile,
    req: &ModelRequest,
) -> Result<Turn, TurnError> {
    let client = ProviderClient::new(profile, key(), &DestinationPolicy::default())
        .expect("api.deepseek.com is a public endpoint and passes the policy");
    let body = client.render(req);
    let rec = Recorder::default();
    let (_tx, rx) = never_cancel();
    let started = now_rfc3339();
    let result = client.run_turn(req, rx, Some(rec.tee())).await;
    save(name, &started, &body, &rec, &result);
    result
}

/// Run one turn through the adapter's own `render`, `OpenAiClient::start` and
/// `assemble`, merging provider-specific request fields the neutral request
/// cannot carry yet (`thinking`, `max_tokens`). Used to observe the wire format
/// with DeepSeek's thinking mode switched off.
async fn recorded_turn_with_extra(
    name: &str,
    profile: &ModelProfile,
    req: &ModelRequest,
    extra: Value,
) -> Result<Turn, TurnError> {
    let http = outbound_client(
        &DestinationPolicy::default(),
        std::time::Duration::from_secs(120),
    )
    .expect("client");
    let c = OpenAiClient::with_client(http, &profile.base_url, key(), DEFAULT_MAX_RESPONSE_BYTES);
    let mut body = openai::render(req, profile);
    for (k, v) in extra.as_object().expect("extra is an object") {
        body[k] = v.clone();
    }
    let rec = Recorder::default();
    let started = now_rfc3339();
    let result = match c.start(&body, Some(rec.tee())).await {
        Ok((src, _meta)) => openai::assemble(src, |_| Control::Continue).await,
        Err(e) => Err(e),
    };
    save(name, &started, &body, &rec, &result);
    result
}

/// The assistant tool call and the tool result that continue a turn.
fn continue_with_result(first: &ModelRequest, turn: &Turn, result: &str) -> ModelRequest {
    let mut req = first.clone();
    let mut parts = Vec::new();
    if !turn.text.is_empty() {
        parts.push(Part::text(turn.text.clone()));
    }
    let mut results = Vec::new();
    for c in &turn.tool_calls {
        parts.push(Part::ToolUse {
            id: c.id.clone(),
            name: c.name.clone(),
            input: c.arguments.clone(),
        });
        results.push(Part::ToolResult {
            tool_use_id: c.id.clone(),
            content: result.into(),
            is_error: false,
        });
    }
    req.messages.push(Message {
        role: Role::Assistant,
        parts,
    });
    req.messages.push(Message {
        role: Role::User,
        parts: results,
    });
    req
}

fn print_capabilities(r: &factory_agent::probe::ProbeReport) {
    for c in Capability::ALL {
        let o = r.capabilities.get(c);
        println!("{c:?}: {:?} via {:?}: {}", o.state, o.basis, o.evidence);
    }
}

#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY"]
async fn live_deepseek_listing_probe() {
    let p = profile();
    let r = probe(&p, key(), &DestinationPolicy::default(), ProbeMode::Listing).await;
    println!(
        "connection: {:?}; models listed: {:?}; error: {:?}",
        r.connection.state, r.models, r.error
    );
    print_capabilities(&r);
    println!("lists deepseek-chat: {}", r.lists_model("deepseek-chat"));
    if let Some(dir) = evidence_dir() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("listing-probe.report.json"),
            serde_json::to_string_pretty(&r).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(r.connection.state, CapabilityState::Supported);
    assert!(r.lists_model(&p.model), "{} is listed", p.model);
    assert_eq!(
        r.capabilities.get(Capability::ModelListing).basis,
        Basis::Probe
    );
    // A listing alone observes nothing else.
    for c in [
        Capability::Text,
        Capability::ToolCalls,
        Capability::Streaming,
        Capability::UsageAccounting,
    ] {
        assert_eq!(
            r.capabilities.get(c).state,
            CapabilityState::Unknown,
            "{c:?}"
        );
    }
}

/// A refused key is a free call and shows how a real DeepSeek refusal is classified.
#[tokio::test]
#[ignore = "live: reaches api.deepseek.com with a deliberately invalid key"]
async fn live_deepseek_invalid_key_is_an_auth_refusal() {
    let bad = ApiKey::new("sk-not-a-real-key-00000000000000000000".into());
    let r = probe(
        &profile(),
        bad,
        &DestinationPolicy::default(),
        ProbeMode::Listing,
    )
    .await;
    println!(
        "connection: {:?}; refusal: {:?}; error: {:?}",
        r.connection, r.refusal, r.error
    );
    if let Some(dir) = evidence_dir() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("invalid-key.report.json"),
            serde_json::to_string_pretty(&r).unwrap(),
        )
        .unwrap();
    }
    let refusal = r.refusal.expect("a refusal was recorded");
    assert_eq!(refusal.status, 401);
    assert_eq!(refusal.class, HttpClass::Auth);
    assert_eq!(r.connection.state, CapabilityState::Unsupported);
}

/// The compatible-profile inference probe: listing plus one tiny streaming request
/// with a closed-schema tool, exactly as the production probe sends it.
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_inference_probe() {
    let p = profile();
    let started = now_rfc3339();
    let r = probe(
        &p,
        key(),
        &DestinationPolicy::default(),
        ProbeMode::Inference,
    )
    .await;
    println!(
        "{started} connection: {:?}; notes: {:?}; error: {:?}; refusal: {:?}",
        r.connection.state, r.notes, r.error, r.refusal
    );
    print_capabilities(&r);
    println!(
        "usage: {:?}; cost: {}",
        r.usage,
        cost_json(r.usage.as_ref())
    );
    if let Some(dir) = evidence_dir() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("inference-probe.report.json"),
            serde_json::to_string_pretty(
                &json!({"report": r, "cost": cost_json(r.usage.as_ref())}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    assert_eq!(r.connection.state, CapabilityState::Supported);
    for c in [
        Capability::Text,
        Capability::ToolCalls,
        Capability::Streaming,
        Capability::UsageAccounting,
        Capability::ModelListing,
    ] {
        let o = r.capabilities.get(c);
        assert_eq!(
            (o.state, o.basis),
            (CapabilityState::Supported, Basis::Probe),
            "{c:?}: {:?}",
            r.notes
        );
    }
    // Wire compatibility is not capability: these stay unproven.
    for c in [Capability::StrictJsonSchema, Capability::ParallelToolCalls] {
        assert_eq!(
            r.capabilities.get(c).state,
            CapabilityState::Unknown,
            "{c:?}"
        );
    }
}

/// One tool turn through the policy client, with the stream recorded. The adapter
/// sends no `thinking` field, so this is DeepSeek's default (thinking on).
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_tool_turn_default_thinking() {
    let turn = recorded_turn("deepseek-tool-turn", profile(), &tool_request())
        .await
        .expect("live tool turn");
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].name, "write_file");
    assert!(turn.tool_calls[0].arguments["path"].is_string());
    assert!(turn.tool_calls[0].arguments["content"].is_string());
    assert!(turn.usage.as_ref().is_some_and(|u| u.output_tokens > 0));
    assert!(turn.events > 1, "streamed in several events");
}

/// Observation, not a pass/fail claim about DeepSeek: with thinking on (the
/// default) DeepSeek documents that `reasoning_content` must be sent back with
/// every later request that has tools. The neutral request has no field for it, so
/// the follow-up after a tool result either works or is refused. The outcome is
/// recorded and only its shape is asserted.
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_follow_up_after_a_tool_result_default_thinking() {
    let first = tool_request();
    let turn = recorded_turn("deepseek-default-thinking-turn-1", profile(), &first)
        .await
        .expect("first turn");
    let second = continue_with_result(&first, &turn, "written");
    let r = recorded_turn("deepseek-default-thinking-turn-2", profile(), &second).await;
    match &r {
        Ok(t) => println!(
            "follow-up completed: finish {} text {:?}",
            t.finish_reason, t.text
        ),
        Err(TurnError::Http(h)) => println!(
            "follow-up refused: status {} class {:?} type {:?} code {:?} message {:?}",
            h.status, h.class, h.provider_type, h.provider_code, h.message
        ),
        Err(e) => panic!("unexpected failure shape: {e:?}"),
    }
}

/// A request that makes the model reason before it calls the tool, so the first
/// answer carries real `reasoning_content` that the neutral request drops.
fn deliberate_request() -> ModelRequest {
    ModelRequest {
        system: "You are the HOI4 Factory runtime probe. Create files only with the write_file tool, once per task.".into(),
        messages: vec![Message::user_text(
            "Two candidate file names are notes/zeta.txt and notes/alpha.txt. Work out which one sorts first alphabetically, then use write_file to create that file with the content: sorted first",
        )],
        tools: vec![write_file_tool()],
        max_output_tokens: 400,
    }
}

/// Observation: the same follow-up, after a first answer that really reasoned.
/// This is the case DeepSeek's documentation says needs `reasoning_content` sent
/// back. The outcome is recorded and only its shape is asserted.
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_follow_up_after_a_reasoned_tool_call_default_thinking() {
    let first = deliberate_request();
    let turn = recorded_turn("deepseek-reasoned-turn-1", profile(), &first)
        .await
        .expect("first turn");
    let reasoning = turn.usage.as_ref().map_or(0, |u| u.reasoning_tokens);
    println!(
        "first turn reasoning tokens: {reasoning}; calls: {}",
        turn.tool_calls.len()
    );
    assert_eq!(turn.finish_reason, "tool_calls");
    let second = continue_with_result(&first, &turn, "written");
    let r = recorded_turn("deepseek-reasoned-turn-2", profile(), &second).await;
    match &r {
        Ok(t) => println!(
            "follow-up completed: finish {} text {:?}",
            t.finish_reason, t.text
        ),
        Err(TurnError::Http(h)) => println!(
            "follow-up refused: status {} class {:?} type {:?} code {:?} message {:?}",
            h.status, h.class, h.provider_type, h.provider_code, h.message
        ),
        Err(e) => panic!("unexpected failure shape: {e:?}"),
    }
}

/// The full round trip with thinking switched off through a request field the
/// adapter cannot set yet: tool call, tool result as a `role: "tool"` message,
/// final text. `max_tokens` is also set because DeepSeek ignores
/// `max_completion_tokens`.
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_tool_round_trip_with_thinking_disabled() {
    let p = profile();
    let extra = json!({"thinking": {"type": "disabled"}, "max_tokens": 200});
    let first = tool_request();
    let turn = recorded_turn_with_extra("deepseek-nothink-turn-1", &p, &first, extra.clone())
        .await
        .expect("first turn");
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(turn.tool_calls[0].name, "write_file");
    let second = continue_with_result(&first, &turn, "written");
    let turn2 = recorded_turn_with_extra("deepseek-nothink-turn-2", &p, &second, extra)
        .await
        .expect("follow-up turn");
    println!("final text: {:?}", turn2.text);
    assert_eq!(turn2.finish_reason, "stop");
    assert!(!turn2.text.is_empty());
    assert!(turn2.tool_calls.is_empty());
    assert!(turn2.usage.as_ref().is_some_and(|u| u.output_tokens > 0));
}

/// The model name the maintainer's request used. DeepSeek's listing on 2026-11-02
/// does not include it. One tiny call records whether it is still accepted.
#[tokio::test]
#[ignore = "live: needs DEEPSEEK_API_KEY, spends a fraction of a cent"]
async fn live_deepseek_legacy_model_name_deepseek_chat() {
    let req = ModelRequest {
        system: String::new(),
        messages: vec![Message::user_text("Say ok.")],
        tools: vec![],
        max_output_tokens: 32,
    };
    let r = recorded_turn(
        "deepseek-chat-legacy-name",
        profile_for("deepseek-chat"),
        &req,
    )
    .await;
    match &r {
        Ok(t) => println!(
            "deepseek-chat accepted: model reported {:?}, text {:?}",
            t.model, t.text
        ),
        Err(TurnError::Http(h)) => println!(
            "deepseek-chat refused: status {} class {:?} message {:?}",
            h.status, h.class, h.message
        ),
        Err(e) => panic!("unexpected failure shape: {e:?}"),
    }
}
