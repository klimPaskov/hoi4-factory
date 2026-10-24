//! Live provider calls: recording capture, auth refusal, cancellation and the
//! single containerized request. Every result is labelled `live`.

use factory_agent::cost;
use factory_agent::openai::{self, ApiKey, Control, OpenAiClient};
use factory_agent::transport::Tee;
use factory_agent::types::TurnError;
use serde_json::{Value, json};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::scenario;
use crate::util::*;

pub const BASE: &str = "https://api.openai.com/v1";

fn client() -> Result<OpenAiClient, String> {
    let key = ApiKey::from_env("OPENAI_API_KEY").ok_or("OPENAI_API_KEY is not set")?;
    OpenAiClient::new(BASE, key, Duration::from_secs(60)).map_err(|e| e.to_string())
}

/// A sink the caller can read back after the stream ends.
/// Each `write` from the tee is exactly one network chunk, so chunk sizes are
/// kept alongside the bytes for faithful replay.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<(Vec<u8>, Vec<usize>)>>);
impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let mut g = self.0.lock().unwrap();
        g.0.extend_from_slice(b);
        g.1.push(b.len());
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn input_bound(body: &Value) -> u64 {
    (canonical(body).len() as u64) / 2 + 50
}

pub fn spend_entry(label: &str, model: &str, usage: Option<&factory_agent::types::Usage>, body: &Value) -> Value {
    let max_out = body["max_completion_tokens"].as_u64().or(body["max_tokens"].as_u64()).unwrap_or(0);
    json!({"call": label, "model": model, "estimate": cost::estimate(model, usage, input_bound(body), max_out)})
}

/// Run one live request with a tee. Saves `<name>.sse` and `<name>.meta.json`.
pub async fn record_one(name: &str, body: &Value, cancel_after_content_deltas: Option<usize>) -> Value {
    let c = match client() {
        Ok(c) => c,
        Err(e) => return json!({"name": name, "error": e}),
    };
    let sink = Shared::default();
    let t0 = Instant::now();
    let started = c.start(body, Some(Tee::new(Box::new(sink.clone())))).await;
    let ttfb_ms = t0.elapsed().as_secs_f64() * 1e3;
    let (src, meta) = match started {
        Ok(x) => x,
        Err(e) => return json!({"name": name, "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed()}),
    };
    let mut cancel_at_ms = None;
    let res = openai::assemble(src, |p| {
        if let Some(n) = cancel_after_content_deltas {
            if p.events > n && !p.text.is_empty() {
                cancel_at_ms = Some(t0.elapsed().as_secs_f64() * 1e3);
                return Control::Cancel;
            }
        }
        Control::Continue
    })
    .await;
    let total_ms = t0.elapsed().as_secs_f64() * 1e3;
    let (bytes, chunk_sizes) = sink.0.lock().unwrap().clone();
    let (usage, model_reported, outcome) = match &res {
        Ok(t) => (t.usage.clone(), t.model.clone(), json!({"ok": true, "turn": t})),
        Err(e) => {
            let partial_usage = match e {
                TurnError::Cancelled { partial } | TurnError::IncompleteOutput { partial, .. } => partial.usage.clone(),
                _ => None,
            };
            (partial_usage, None, json!({"ok": false, "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed()}))
        }
    };
    let model = model_reported.clone().unwrap_or_else(|| body["model"].as_str().unwrap_or("").to_owned());
    let d = recordings_dir();
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(format!("{name}.sse")), &bytes).unwrap();
    let spend = spend_entry(name, &model, usage.as_ref(), body);
    let meta_v = json!({
        "name": name,
        "evidence_kind": "live",
        "captured_utc": now_utc(),
        "endpoint": format!("{BASE}/chat/completions"),
        "request_body": body,
        "response": {"status": meta.status, "x_request_id": meta.request_id, "openai_processing_ms": meta.processing_ms, "content_type": meta.content_type},
        "model_requested": body["model"],
        "model_reported": model_reported,
        "sse_bytes": bytes.len(),
        "sse_sha256": sha256_hex(&bytes),
        "chunk_sizes": chunk_sizes,
        "ttfb_ms": ttfb_ms,
        "total_ms": total_ms,
        "cancel_at_ms": cancel_at_ms,
        "outcome": outcome,
        "spend": spend,
        "redaction": "Authorization header is never recorded. Response headers other than status, x-request-id, openai-processing-ms and content-type are dropped, including openai-organization and openai-project.",
    });
    write_json(&d.join(format!("{name}.meta.json")), &meta_v);
    meta_v
}

/// Live request with an invalid key. Expected 401, classified as auth, no retry.
pub async fn auth_probe() -> Value {
    let c = OpenAiClient::new(BASE, ApiKey::new("sk-s01-invalid-probe-key".into()), Duration::from_secs(30)).unwrap();
    let body = scenario::text_body();
    let t0 = Instant::now();
    let r = c.start(&body, None).await;
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    match r {
        Ok(_) => json!({"unexpected": "invalid key was accepted"}),
        Err(e) => json!({"evidence_kind": "live", "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed(), "elapsed_ms": ms, "spend": {"usd": 0.0, "basis": "request refused at authentication"}}),
    }
}

/// One live request for EXP-04. Prints JSON to stdout, records nothing.
pub async fn live_once() -> Value {
    let c = match client() {
        Ok(c) => c,
        Err(e) => return json!({"evidence_kind": "live", "ok": false, "error": e, "request_sent": false}),
    };
    let body = scenario::tool_turn_body();
    let t0 = Instant::now();
    let r = match c.start(&body, None).await {
        Ok((src, meta)) => {
            let turn = openai::assemble(src, |_| Control::Continue).await;
            json!({"status": meta.status, "x_request_id": meta.request_id, "turn": turn.as_ref().ok(), "error": turn.as_ref().err(),
                   "spend": spend_entry("exp04-container", turn.as_ref().ok().and_then(|t| t.model.as_deref()).unwrap_or(scenario::MODEL), turn.as_ref().ok().and_then(|t| t.usage.as_ref()), &body)})
        }
        Err(e) => json!({"error": e, "class": e.class()}),
    };
    json!({"evidence_kind": "live", "runtime_build": factory_agent::RUNTIME_BUILD, "elapsed_ms": t0.elapsed().as_secs_f64() * 1e3, "result": r,
           "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH}})
}

/// Inspect-provider call: GET /v1/models. No inference and no spend. Used
/// from the container when inference is blocked by the account balance.
pub async fn models_check() -> Value {
    let c = match client() {
        Ok(c) => c,
        Err(e) => return json!({"evidence_kind": "live", "ok": false, "error": e, "request_sent": false}),
    };
    let t0 = Instant::now();
    let r = c.list_models().await;
    json!({
        "evidence_kind": "live",
        "operation": "GET /v1/models (no inference)",
        "runtime_build": factory_agent::RUNTIME_BUILD,
        "elapsed_ms": t0.elapsed().as_secs_f64() * 1e3,
        "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "result": match r {
            Ok((status, ids)) => json!({"ok": true, "status": status, "model_count": ids.len(), "selected_model_listed": ids.iter().any(|m| m == scenario::MODEL)}),
            Err(e) => json!({"ok": false, "class": e.class(), "error": e}),
        },
        "spend": {"usd": 0.0, "basis": "model listing is not billed"},
    })
}

pub const ANTHROPIC_BASE: &str = "https://api.anthropic.com";

fn a_client() -> Result<factory_agent::anthropic::AnthropicClient, String> {
    let key = ApiKey::from_env("ANTHROPIC_API_KEY").ok_or("ANTHROPIC_API_KEY is not set")?;
    factory_agent::anthropic::AnthropicClient::new(ANTHROPIC_BASE, key, Duration::from_secs(60)).map_err(|e| e.to_string())
}

/// Anthropic variant of `record_one`. The body bytes are kept byte-for-byte.
/// No response header except status and content type is kept, so the
/// request-id and organization headers never reach the recording.
pub async fn record_anthropic(name: &str, body: &Value, cancel_after_events: Option<usize>) -> Value {
    let c = match a_client() {
        Ok(c) => c,
        Err(e) => return json!({"name": name, "error": e, "request_sent": false}),
    };
    let sink = Shared::default();
    let t0 = Instant::now();
    let started = c.start(body, Some(Tee::new(Box::new(sink.clone())))).await;
    let ttfb_ms = t0.elapsed().as_secs_f64() * 1e3;
    let (src, meta) = match started {
        Ok(x) => x,
        Err(e) => return json!({"name": name, "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed(), "spend": {"usd": 0.0, "basis": "refused before generation"}}),
    };
    let mut cancel_at_ms = None;
    let mut events_at_cancel = None;
    let res = factory_agent::anthropic::assemble(src, |p| {
        if let Some(n) = cancel_after_events
            && p.events > n
            && !p.text.is_empty()
        {
            cancel_at_ms = Some(t0.elapsed().as_secs_f64() * 1e3);
            events_at_cancel = Some(p.events);
            return Control::Cancel;
        }
        Control::Continue
    })
    .await;
    let total_ms = t0.elapsed().as_secs_f64() * 1e3;
    let (bytes, chunk_sizes) = sink.0.lock().unwrap().clone();
    let (usage, model_reported, outcome) = match &res {
        Ok(t) => (t.usage.clone(), t.model.clone(), json!({"ok": true, "turn": t})),
        Err(e) => {
            let partial_usage = match e {
                TurnError::Cancelled { partial } | TurnError::IncompleteOutput { partial, .. } => partial.usage.clone(),
                _ => None,
            };
            (partial_usage, None, json!({"ok": false, "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed()}))
        }
    };
    let model = model_reported.clone().unwrap_or_else(|| {
        let mut p = factory_agent::sse::SseParser::new();
        p.push(&bytes)
            .ok()
            .and_then(|evs| evs.into_iter().find_map(|e| serde_json::from_str::<Value>(&e.data).ok().and_then(|v| v["message"]["model"].as_str().map(str::to_owned))))
            .unwrap_or_else(|| body["model"].as_str().unwrap_or("").to_owned())
    });
    // A cancelled stream reports only message_start usage. The estimate is an
    // upper bound: reported input plus the full output allowance.
    let spend = if cancel_at_ms.is_some() {
        let mut u = usage.clone().unwrap_or_default();
        u.output_tokens = body["max_tokens"].as_u64().unwrap_or(0);
        json!({"call": name, "model": model, "basis": "cancelled: usage incomplete, output counted at the full max_tokens allowance as an upper bound",
               "estimate": cost::estimate(&model, Some(&u), 0, 0), "reported_partial_usage": usage})
    } else {
        spend_entry(name, &model, usage.as_ref(), body)
    };
    let d = recordings_dir();
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(format!("{name}.sse")), &bytes).unwrap();
    let meta_v = json!({
        "name": name,
        "provider": factory_agent::anthropic::PROVIDER,
        "evidence_kind": "live",
        "captured_utc": now_utc(),
        "endpoint": format!("{ANTHROPIC_BASE}/v1/messages"),
        "anthropic_version": factory_agent::anthropic::API_VERSION,
        "request_body": body,
        "response": {"status": meta.status, "content_type": meta.content_type},
        "model_requested": body["model"],
        "model_reported": model,
        "sse_bytes": bytes.len(),
        "sse_sha256": sha256_hex(&bytes),
        "chunk_sizes": chunk_sizes,
        "ttfb_ms": ttfb_ms,
        "total_ms": total_ms,
        "cancel_at_ms": cancel_at_ms,
        "events_at_cancel": events_at_cancel,
        "outcome": outcome,
        "spend": spend,
        "redaction": "x-api-key is never recorded. Response headers other than status and content-type are dropped, including request-id and anthropic-organization-id. The body is byte-for-byte. Message IDs (msg_...) inside the body are kept because altering them would break byte fidelity, and they are not credentials.",
    });
    write_json(&d.join(format!("{name}.meta.json")), &meta_v);
    meta_v
}

pub async fn auth_probe_anthropic() -> Value {
    let c = factory_agent::anthropic::AnthropicClient::new(ANTHROPIC_BASE, ApiKey::new("sk-ant-s01-invalid-probe-key".into()), Duration::from_secs(30)).unwrap();
    let body = scenario::a_text_body();
    let t0 = Instant::now();
    match c.start(&body, None).await {
        Ok(_) => json!({"unexpected": "invalid key was accepted"}),
        Err(e) => json!({"evidence_kind": "live", "provider": factory_agent::anthropic::PROVIDER, "class": e.class(), "error": e, "auto_retry_allowed": e.auto_retry_allowed(),
                          "elapsed_ms": t0.elapsed().as_secs_f64() * 1e3, "spend": {"usd": 0.0, "basis": "request refused at authentication"}}),
    }
}

/// The single containerized native request (EXP-04), Anthropic Messages.
pub async fn live_once_anthropic() -> Value {
    let c = match a_client() {
        Ok(c) => c,
        Err(e) => return json!({"evidence_kind": "live", "ok": false, "error": e, "request_sent": false}),
    };
    let body = scenario::a_tool_turn_body();
    let t0 = Instant::now();
    let r = match c.start(&body, None).await {
        Ok((src, meta)) => {
            let turn = factory_agent::anthropic::assemble(src, |_| Control::Continue).await;
            let model = turn.as_ref().ok().and_then(|t| t.model.clone()).unwrap_or_else(|| scenario::A_MODEL.into());
            json!({"status": meta.status, "turn": turn.as_ref().ok(), "error": turn.as_ref().err(),
                   "spend": spend_entry("exp04-container", &model, turn.as_ref().ok().and_then(|t| t.usage.as_ref()), &body)})
        }
        Err(e) => json!({"error": e, "class": e.class(), "auto_retry_allowed": e.auto_retry_allowed()}),
    };
    json!({"evidence_kind": "live", "provider": factory_agent::anthropic::PROVIDER, "anthropic_version": factory_agent::anthropic::API_VERSION,
           "runtime_build": factory_agent::RUNTIME_BUILD, "elapsed_ms": t0.elapsed().as_secs_f64() * 1e3, "result": r,
           "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH}})
}
