//! Thin Anthropic Messages streaming adapter.
//!
//! The adapter builds the request, classifies HTTP refusals, parses SSE with
//! the crate's own parser and assembles `tool_use` input from
//! `input_json_delta` fragments by content-block index. A turn is complete
//! only after a `message_delta` stop reason, the `message_stop` event and a
//! clean SSE tail were all observed and every tool input parses as JSON.
//! `thinking` blocks and `signature_delta` fragments are tolerated and never
//! surfaced, so provider-private reasoning cannot leak into a handoff.

use serde_json::{Value, json};

use crate::openai::{ApiKey, Control};
use crate::sse::SseParser;
use crate::transport::{ByteSource, TransportError};
use crate::types::*;

pub const PROVIDER: &str = "anthropic-messages";
/// API version header value. 2026-10-21 rejects sampling parameters, which
/// these requests never send.
pub const API_VERSION: &str = "2026-10-21";

/// Build a streaming Messages body. `tools` are full tool definitions
/// (name, description, input_schema, optional strict). `extra` is merged last.
pub fn request_body(
    model: &str,
    system: Option<&str>,
    messages: Vec<Value>,
    tools: &[Value],
    max_tokens: u32,
    extra: Option<Value>,
) -> Value {
    let mut b = json!({"model": model, "messages": messages, "max_tokens": max_tokens, "stream": true});
    if let Some(s) = system {
        b["system"] = json!(s);
    }
    if !tools.is_empty() {
        b["tools"] = Value::Array(tools.to_vec());
    }
    if let Some(Value::Object(m)) = extra {
        for (k, v) in m {
            b[k] = v;
        }
    }
    b
}

/// A client tool definition from a closed JSON Schema.
pub fn tool(name: &str, description: &str, schema: &Value, strict: bool) -> Value {
    json!({"name": name, "description": description, "input_schema": schema, "strict": strict})
}

pub fn classify(status: u16, error_type: Option<&str>, message: Option<&str>) -> HttpClass {
    let credit = message.is_some_and(|m| m.to_ascii_lowercase().contains("credit balance"));
    match (status, error_type) {
        (401, _) => HttpClass::Auth,
        (403, _) => HttpClass::Permission,
        (404, _) => HttpClass::NotFound,
        (400, _) if credit => HttpClass::Quota,
        (429, _) => HttpClass::RateLimited,
        (529, _) | (_, Some("overloaded_error")) => HttpClass::Overloaded,
        (400 | 413 | 422, _) => HttpClass::InvalidRequest,
        (500..=599, _) => HttpClass::Server,
        _ => HttpClass::Other,
    }
}

pub fn refusal_from_body(status: u16, body: &[u8]) -> HttpRefusal {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let t = v["error"]["type"].as_str();
    let m = v["error"]["message"].as_str();
    HttpRefusal {
        status,
        class: classify(status, t, m),
        provider_code: None,
        provider_type: t.map(str::to_owned),
        message: m.map(redact),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseMeta {
    pub status: u16,
    pub content_type: Option<String>,
}

#[cfg(feature = "http")]
pub struct AnthropicClient {
    http: reqwest::Client,
    base_url: String,
    key: ApiKey,
}

#[cfg(feature = "http")]
impl AnthropicClient {
    pub fn new(base_url: &str, key: ApiKey, timeout: std::time::Duration) -> reqwest::Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { http, base_url: base_url.trim_end_matches('/').to_owned(), key })
    }

    /// Send the request. Response headers are not returned except status and
    /// content type, so request and organization identifiers are never kept.
    pub async fn start(
        &self,
        body: &Value,
        tee: Option<crate::transport::Tee>,
    ) -> Result<(ByteSource, ResponseMeta), TurnError> {
        let resp = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", self.key.expose())
            .header("anthropic-version", API_VERSION)
            .header("accept", "text/event-stream")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(body).unwrap_or_default())
            .send()
            .await
            .map_err(|e| {
                let (error, acceptance) = if e.is_connect() {
                    (TransportError::ConnectFailed(redact(&e.to_string())), Acceptance::NotAccepted)
                } else {
                    (TransportError::AfterSend(redact(&e.to_string())), Acceptance::Uncertain)
                };
                TurnError::Transport { error, acceptance, partial: Partial::default() }
            })?;
        let meta = ResponseMeta {
            status: resp.status().as_u16(),
            content_type: resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_owned),
        };
        if !resp.status().is_success() {
            let status = meta.status;
            let body = resp.bytes().await.unwrap_or_default();
            return Err(TurnError::Http(refusal_from_body(status, &body[..body.len().min(65536)])));
        }
        Ok((ByteSource::Live { resp, tee }, meta))
    }
}

#[derive(Default)]
struct St {
    p: Partial,
    model: Option<String>,
    stopped: bool,
}

/// Assemble a Messages stream. `observe` sees the partial state after every
/// event and may cancel, which drops the byte source and closes the connection.
pub async fn assemble(mut src: ByteSource, mut observe: impl FnMut(&Partial) -> Control) -> Result<Turn, TurnError> {
    let mut parser = SseParser::new();
    let mut st = St::default();
    loop {
        let chunk = match src.next_chunk().await {
            None => break,
            Some(Ok(b)) => b,
            Some(Err(error)) => {
                return Err(TurnError::Transport { error, acceptance: Acceptance::Accepted, partial: st.p });
            }
        };
        st.p.bytes += chunk.len();
        let evs = parser
            .push(&chunk)
            .map_err(|e| TurnError::MalformedEvent { detail: e.to_string(), partial: st.p.clone() })?;
        for ev in evs {
            st.p.events += 1;
            apply(&mut st, &ev.data)?;
            if let Control::Cancel = observe(&st.p) {
                drop(src);
                return Err(TurnError::Cancelled { partial: st.p });
            }
        }
    }
    finish(st, parser.finish())
}

fn apply(st: &mut St, data: &str) -> Result<(), TurnError> {
    let v: Value = serde_json::from_str(data).map_err(|e| TurnError::MalformedEvent {
        detail: format!("event JSON: {e}"),
        partial: st.p.clone(),
    })?;
    let p = &mut st.p;
    match v["type"].as_str().unwrap_or("") {
        "message_start" => {
            let m = &v["message"];
            p.response_id = m["id"].as_str().map(str::to_owned);
            st.model = m["model"].as_str().map(str::to_owned);
            let u = &m["usage"];
            p.usage = Some(Usage {
                input_tokens: u["input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_creation_input_tokens"].as_u64().unwrap_or(0),
                cached_input_tokens: u["cache_read_input_tokens"].as_u64().unwrap_or(0),
                output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                reasoning_tokens: 0,
            });
        }
        "content_block_start" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            let cb = &v["content_block"];
            match cb["type"].as_str() {
                Some("tool_use") => p.tool_calls.push(PartialToolCall {
                    index: idx,
                    id: cb["id"].as_str().map(str::to_owned),
                    name: cb["name"].as_str().map(str::to_owned),
                    raw_arguments: String::new(),
                }),
                Some("text") => p.text.push_str(cb["text"].as_str().unwrap_or("")),
                _ => {}
            }
        }
        "content_block_delta" => {
            let idx = v["index"].as_u64().unwrap_or(0) as usize;
            let d = &v["delta"];
            match d["type"].as_str() {
                Some("text_delta") => p.text.push_str(d["text"].as_str().unwrap_or("")),
                Some("input_json_delta") => {
                    if let Some(tc) = p.tool_calls.iter_mut().find(|t| t.index == idx) {
                        tc.raw_arguments.push_str(d["partial_json"].as_str().unwrap_or(""));
                    }
                }
                // thinking_delta, signature_delta and unknown delta types are not surfaced.
                _ => {}
            }
        }
        "message_delta" => {
            if let Some(sr) = v["delta"]["stop_reason"].as_str() {
                p.finish_reason = Some(sr.to_owned());
            }
            if let Some(u) = p.usage.as_mut() {
                u.output_tokens = v["usage"]["output_tokens"].as_u64().unwrap_or(u.output_tokens);
            }
        }
        "message_stop" => {
            st.stopped = true;
            p.done_marker = true;
        }
        "error" => {
            return Err(TurnError::ProviderStreamError {
                detail: redact(&format!(
                    "{} {}",
                    v["error"]["type"].as_str().unwrap_or(""),
                    v["error"]["message"].as_str().unwrap_or("")
                )),
                partial: p.clone(),
            });
        }
        _ => {} // ping, content_block_stop
    }
    Ok(())
}

fn finish(st: St, tail: crate::sse::SseTail) -> Result<Turn, TurnError> {
    let p = st.p;
    let inc = |d: &str, p: Partial| TurnError::IncompleteOutput { detail: d.into(), partial: p };
    if !tail.is_clean() {
        return Err(inc(
            &format!(
                "stream ended mid-event ({} partial line bytes, undispatched={})",
                tail.partial_line_bytes, tail.undispatched_event
            ),
            p,
        ));
    }
    let Some(sr) = p.finish_reason.clone() else {
        return Err(inc("stream ended without a message_delta stop_reason", p));
    };
    if !st.stopped {
        return Err(inc("stream ended without message_stop", p));
    }
    if sr == "max_tokens" || sr == "refusal" || sr == "pause_turn" {
        return Err(inc(&format!("stop_reason={sr}, output is not a complete executable turn"), p));
    }
    let mut calls = Vec::new();
    for tc in &p.tool_calls {
        let raw = if tc.raw_arguments.is_empty() { "{}" } else { tc.raw_arguments.as_str() };
        let arguments = serde_json::from_str(raw).map_err(|e| TurnError::MalformedToolArguments {
            index: tc.index,
            detail: e.to_string(),
            partial: p.clone(),
        })?;
        calls.push(ToolCall {
            index: tc.index,
            id: tc.id.clone().unwrap_or_default(),
            name: tc.name.clone().unwrap_or_default(),
            raw_arguments: tc.raw_arguments.clone(),
            arguments,
        });
    }
    Ok(Turn {
        provider: PROVIDER,
        model: st.model,
        response_id: p.response_id.clone(),
        text: p.text.clone(),
        tool_calls: calls,
        finish_reason: sr,
        usage: p.usage.clone(),
        bytes: p.bytes,
        events: p.events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Request shape and status classification only. No model output is used.
    #[test]
    fn request_shape() {
        let t = tool("write_file", "d", &json!({"type":"object"}), true);
        let b = request_body("model-id", Some("sys"), vec![json!({"role":"user","content":"x"})], &[t], 64, None);
        assert_eq!(b["stream"], true);
        assert_eq!(b["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(b["tools"][0]["strict"], true);
        assert_eq!(b["system"], "sys");
        assert!(b.get("temperature").is_none());
    }

    #[test]
    fn classes() {
        assert_eq!(classify(401, Some("authentication_error"), None), HttpClass::Auth);
        assert_eq!(classify(529, Some("overloaded_error"), None), HttpClass::Overloaded);
        assert_eq!(
            classify(400, Some("invalid_request_error"), Some("Your credit balance is too low")),
            HttpClass::Quota
        );
    }
}
