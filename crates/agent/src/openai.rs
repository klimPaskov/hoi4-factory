//! Thin OpenAI Chat Completions streaming adapter.
//!
//! Chat Completions is used because the same wire format serves OpenAI and
//! OpenAI-compatible self-hosted endpoints (ADR-019). The adapter builds the
//! request, classifies HTTP refusals, parses SSE itself and assembles tool-call
//! deltas by index. A turn is complete only when a finish reason, the
//! `[DONE]` marker and a clean SSE tail were all observed and every tool
//! call's argument text parses as JSON.

use serde_json::{Value, json};

use crate::profile::{Capabilities, Capability, ModelProfile};
use crate::request::{ModelRequest, Part, Role};
use crate::sse::SseParser;
use crate::transport::ByteSource;
use crate::types::*;

pub const PROVIDER: &str = "openai-chat-completions";

/// Holds the key without exposing it through Debug or Display.
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(s: String) -> Self {
        Self(s)
    }
    pub fn from_env(var: &str) -> Option<Self> {
        std::env::var(var).ok().filter(|v| !v.is_empty()).map(Self)
    }
    /// Only for writing the request header.
    #[cfg_attr(not(feature = "anthropic"), allow(dead_code))]
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey([REDACTED])")
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponseMeta {
    pub status: u16,
    pub request_id: Option<String>,
    pub processing_ms: Option<String>,
    pub content_type: Option<String>,
}

/// Build a streaming Chat Completions body. `extra` is merged last.
pub fn request_body(
    model: &str,
    messages: Vec<Value>,
    tools: &[Value],
    tool_choice: Option<Value>,
    max_completion_tokens: u32,
    extra: Option<Value>,
) -> Value {
    let mut b = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_completion_tokens": max_completion_tokens,
    });
    if !tools.is_empty() {
        b["tools"] = Value::Array(tools.to_vec());
        b["parallel_tool_calls"] = json!(false);
    }
    if let Some(tc) = tool_choice {
        b["tool_choice"] = tc;
    }
    if let Some(Value::Object(m)) = extra {
        for (k, v) in m {
            b[k] = v;
        }
    }
    b
}

/// Render a provider-neutral request as a Chat Completions body, with the
/// profile's starting capabilities. Assistant tool uses become `tool_calls`
/// and tool results become `role: "tool"` messages.
pub fn render(req: &ModelRequest, profile: &ModelProfile) -> Value {
    render_with(req, profile, &profile.starting_capabilities())
}

/// Render with explicit capabilities. The tool `strict` flag is sent only when
/// strict JSON schema is `Supported`; it is never inferred from compatibility.
pub fn render_with(req: &ModelRequest, profile: &ModelProfile, caps: &Capabilities) -> Value {
    let strict = caps.is_supported(Capability::StrictJsonSchema);
    let mut messages: Vec<Value> = Vec::new();
    if !req.system.is_empty() {
        messages.push(json!({"role": "system", "content": req.system}));
    }
    for m in &req.messages {
        match m.role {
            Role::User => {
                let mut text = String::new();
                for p in &m.parts {
                    match p {
                        Part::Text { text: t } => {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(t);
                        }
                        Part::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let content = if *is_error && !content.starts_with("Error:") {
                                format!("Error: {content}")
                            } else {
                                content.clone()
                            };
                            messages.push(json!({"role": "tool", "tool_call_id": tool_use_id, "content": content}));
                        }
                        Part::ToolUse { .. } => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
            Role::Assistant => {
                let text: String = m
                    .parts
                    .iter()
                    .filter_map(|p| {
                        if let Part::Text { text } = p {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let calls: Vec<Value> = m
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::ToolUse { id, name, input } => Some(json!({
                            "id": id, "type": "function",
                            "function": {"name": name, "arguments": input.to_string()}
                        })),
                        _ => None,
                    })
                    .collect();
                if text.is_empty() && calls.is_empty() {
                    continue;
                }
                let mut msg = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
                if !calls.is_empty() {
                    msg["tool_calls"] = Value::Array(calls);
                }
                messages.push(msg);
            }
        }
    }
    let tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| {
            let mut v = function_tool(&t.name, &t.description, &t.input_schema, strict);
            if !strict {
                v["function"].as_object_mut().map(|o| o.remove("strict"));
            }
            v
        })
        .collect();
    request_body(
        &profile.model,
        messages,
        &tools,
        None,
        req.max_output_tokens.min(profile.max_output_tokens),
        None,
    )
}

/// Map an OpenAI function tool definition from a closed JSON Schema.
pub fn function_tool(name: &str, description: &str, schema: &Value, strict: bool) -> Value {
    json!({
        "type": "function",
        "function": {"name": name, "description": description, "parameters": schema, "strict": strict}
    })
}

/// Classify an HTTP refusal. A 429 is a quota or billing stop, not a rate
/// limit, when the error type is `insufficient_quota` (observed live on
/// 2026-10-24 with code `credit_balance_exhausted`) or the code names quota.
pub fn classify_status(status: u16, error_type: Option<&str>, code: Option<&str>) -> HttpClass {
    let quota = error_type == Some("insufficient_quota")
        || matches!(
            code,
            Some("insufficient_quota" | "credit_balance_exhausted" | "billing_hard_limit_reached")
        );
    match status {
        401 => HttpClass::Auth,
        403 => HttpClass::Permission,
        404 => HttpClass::NotFound,
        429 if quota => HttpClass::Quota,
        429 => HttpClass::RateLimited,
        400 | 409 | 413 | 422 => HttpClass::InvalidRequest,
        503 | 529 => HttpClass::Overloaded,
        500..=599 => HttpClass::Server,
        _ => HttpClass::Other,
    }
}

pub fn refusal_from_body(status: u16, body: &[u8]) -> HttpRefusal {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let e = &v["error"];
    let code = e["code"].as_str().map(str::to_owned);
    HttpRefusal {
        status,
        class: classify_status(status, e["type"].as_str(), code.as_deref()),
        provider_type: e["type"].as_str().map(str::to_owned),
        message: e["message"].as_str().map(redact),
        provider_code: code,
    }
}

#[cfg(feature = "http")]
pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    key: ApiKey,
    max_response_bytes: u64,
}

#[cfg(feature = "http")]
impl OpenAiClient {
    /// A plain client without the destination policy. Production code uses
    /// [`OpenAiClient::with_client`] with [`crate::destination::outbound_client`].
    pub fn new(base_url: &str, key: ApiKey, timeout: std::time::Duration) -> reqwest::Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self::with_client(
            http,
            base_url,
            key,
            crate::transport::DEFAULT_MAX_RESPONSE_BYTES,
        ))
    }

    pub fn with_client(
        http: reqwest::Client,
        base_url: &str,
        key: ApiKey,
        max_response_bytes: u64,
    ) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            key,
            max_response_bytes,
        }
    }

    /// Send the request. Returns the body as a byte source once a 2xx status is
    /// observed. The tee, when given, receives every body byte unchanged.
    pub async fn start(
        &self,
        body: &Value,
        tee: Option<crate::transport::Tee>,
    ) -> Result<(ByteSource, ResponseMeta), TurnError> {
        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.key.0)
            .header("accept", "text/event-stream")
            .header("content-type", "application/json")
            .body(serde_json::to_vec(body).unwrap_or_default())
            .send()
            .await
            .map_err(crate::transport::map_send_error)?;
        let h = resp.headers();
        let hv = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).map(str::to_owned);
        let meta = ResponseMeta {
            status: resp.status().as_u16(),
            request_id: hv("x-request-id"),
            processing_ms: hv("openai-processing-ms"),
            content_type: hv("content-type"),
        };
        crate::transport::redirect_check(meta.status)?;
        if !resp.status().is_success() {
            let status = meta.status;
            let body = crate::transport::read_capped(resp, 64 * 1024)
                .await
                .unwrap_or_default();
            return Err(TurnError::Http(refusal_from_body(status, &body)));
        }
        Ok((ByteSource::live(resp, tee, self.max_response_bytes), meta))
    }

    /// Inspect-provider operation: list model IDs (no inference, no spend).
    pub async fn list_models(&self) -> Result<(u16, Vec<String>), TurnError> {
        let resp = self
            .http
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&self.key.0)
            .send()
            .await
            .map_err(crate::transport::map_send_error)?;
        let status = resp.status().as_u16();
        crate::transport::redirect_check(status)?;
        let body = crate::transport::read_capped(resp, self.max_response_bytes.min(1024 * 1024))
            .await
            .map_err(|error| TurnError::Transport {
                error,
                acceptance: Acceptance::Accepted,
                partial: Partial::default(),
            })?;
        if !(200..300).contains(&status) {
            return Err(TurnError::Http(refusal_from_body(status, &body)));
        }
        let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let ids = v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok((status, ids))
    }
}

/// Observer decision after each parsed event.
pub enum Control {
    Continue,
    Cancel,
}

#[derive(Default)]
struct State {
    p: Partial,
    model: Option<String>,
}

/// Assemble one streamed turn. `observe` sees the partial state after every
/// event and may cancel. Cancelling drops the byte source, which closes a live
/// connection.
pub async fn assemble(
    mut src: ByteSource,
    mut observe: impl FnMut(&Partial) -> Control,
) -> Result<Turn, TurnError> {
    let mut parser = SseParser::new();
    let mut st = State::default();
    loop {
        let chunk = match src.next_chunk().await {
            None => break,
            Some(Ok(b)) => b,
            Some(Err(error)) => {
                return Err(TurnError::Transport {
                    error,
                    acceptance: Acceptance::Accepted,
                    partial: st.p,
                });
            }
        };
        st.p.bytes += chunk.len();
        let events = parser.push(&chunk).map_err(|e| TurnError::MalformedEvent {
            detail: e.to_string(),
            partial: st.p.clone(),
        })?;
        for ev in events {
            st.p.events += 1;
            apply_event(&mut st, &ev.data)?;
            if let Control::Cancel = observe(&st.p) {
                drop(src);
                return Err(TurnError::Cancelled { partial: st.p });
            }
        }
    }
    finish(st, parser.finish())
}

fn apply_event(st: &mut State, data: &str) -> Result<(), TurnError> {
    if data == "[DONE]" {
        st.p.done_marker = true;
        return Ok(());
    }
    let v: Value = serde_json::from_str(data).map_err(|e| TurnError::MalformedEvent {
        detail: format!("event JSON: {e}"),
        partial: st.p.clone(),
    })?;
    if v.get("error").is_some_and(|e| !e.is_null()) {
        let e = &v["error"];
        return Err(TurnError::ProviderStreamError {
            detail: redact(&format!(
                "{} {}",
                e["type"].as_str().unwrap_or(""),
                e["message"].as_str().unwrap_or("")
            )),
            partial: st.p.clone(),
        });
    }
    if st.p.response_id.is_none() {
        st.p.response_id = v["id"].as_str().map(str::to_owned);
    }
    if st.model.is_none() {
        st.model = v["model"].as_str().map(str::to_owned);
    }
    if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
        st.p.usage = Some(Usage {
            input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
            cached_input_tokens: u["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0),
            output_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
            reasoning_tokens: u["completion_tokens_details"]["reasoning_tokens"]
                .as_u64()
                .unwrap_or(0),
        });
    }
    if let Some(choices) = v["choices"].as_array() {
        for c in choices {
            if c["index"].as_u64().unwrap_or(0) != 0 {
                continue;
            }
            let d = &c["delta"];
            if let Some(t) = d["content"].as_str() {
                st.p.text.push_str(t);
            }
            if let Some(tcs) = d["tool_calls"].as_array() {
                for tc in tcs {
                    let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                    let slot = match st.p.tool_calls.iter_mut().position(|x| x.index == idx) {
                        Some(i) => &mut st.p.tool_calls[i],
                        None => {
                            st.p.tool_calls.push(PartialToolCall {
                                index: idx,
                                ..Default::default()
                            });
                            st.p.tool_calls.last_mut().unwrap()
                        }
                    };
                    if let Some(id) = tc["id"].as_str() {
                        slot.id = Some(id.to_owned());
                    }
                    if let Some(n) = tc["function"]["name"].as_str() {
                        slot.name.get_or_insert_with(String::new).push_str(n);
                    }
                    if let Some(a) = tc["function"]["arguments"].as_str() {
                        slot.raw_arguments.push_str(a);
                    }
                }
            }
            if let Some(fr) = c["finish_reason"].as_str() {
                st.p.finish_reason = Some(fr.to_owned());
            }
        }
    }
    Ok(())
}

fn finish(st: State, tail: crate::sse::SseTail) -> Result<Turn, TurnError> {
    let p = st.p;
    let incomplete = |detail: &str, p: Partial| TurnError::IncompleteOutput {
        detail: detail.into(),
        partial: p,
    };
    if !tail.is_clean() {
        return Err(incomplete(
            &format!(
                "stream ended mid-event ({} partial line bytes, undispatched={})",
                tail.partial_line_bytes, tail.undispatched_event
            ),
            p,
        ));
    }
    let Some(fr) = p.finish_reason.clone() else {
        return Err(incomplete("stream ended without finish_reason", p));
    };
    if !p.done_marker {
        return Err(incomplete("stream ended without [DONE] marker", p));
    }
    if fr == "length" || fr == "content_filter" {
        return Err(incomplete(
            &format!("finish_reason={fr}, output was cut by the provider"),
            p,
        ));
    }
    let mut calls = Vec::new();
    for tc in &p.tool_calls {
        let arguments: Value = match serde_json::from_str(&tc.raw_arguments) {
            Ok(v) => v,
            Err(e) => {
                return Err(TurnError::MalformedToolArguments {
                    index: tc.index,
                    detail: e.to_string(),
                    partial: p.clone(),
                });
            }
        };
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
        finish_reason: fr,
        usage: p.usage.clone(),
        bytes: p.bytes,
        events: p.events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classes() {
        assert_eq!(classify_status(401, None, None), HttpClass::Auth);
        assert_eq!(
            classify_status(429, None, Some("insufficient_quota")),
            HttpClass::Quota
        );
        assert_eq!(
            classify_status(429, Some("requests"), Some("rate_limit_exceeded")),
            HttpClass::RateLimited
        );
        assert_eq!(classify_status(503, None, None), HttpClass::Overloaded);
    }

    /// Regression from the live refusal recorded in
    /// evidence/S01/exp03/runs/live-record-quota-refusal.json. The status, type
    /// and code are the provider's values. The first live run classified this
    /// as rate_limited with automatic retry allowed, which was wrong.
    #[test]
    fn live_credit_exhausted_is_quota_not_retryable() {
        let body = br#"{"error":{"type":"insufficient_quota","code":"credit_balance_exhausted","message":"m"}}"#;
        let r = refusal_from_body(429, body);
        assert_eq!(r.class, HttpClass::Quota);
        assert!(!TurnError::Http(r).auto_retry_allowed());
    }

    #[test]
    fn body_shape() {
        let b = request_body(
            "m",
            vec![json!({"role":"user","content":"x"})],
            &[],
            None,
            16,
            None,
        );
        assert_eq!(b["stream"], true);
        assert_eq!(b["stream_options"]["include_usage"], true);
        assert!(b.get("tools").is_none());
    }
}
