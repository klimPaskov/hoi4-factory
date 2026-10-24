//! Provider dispatch for recorded real streams. A recording's metadata names
//! the provider that produced it, and every fault offset is computed from that
//! provider's own event shapes.

use factory_agent::openai::Control;
use factory_agent::transport::ByteSource;
use factory_agent::types::{Partial, Turn, TurnError};
use serde_json::Value;

use crate::util::Recording;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prov {
    OpenAi,
    Anthropic,
}

pub fn of(rec: &Recording) -> Prov {
    if rec.meta["provider"] == factory_agent::anthropic::PROVIDER { Prov::Anthropic } else { Prov::OpenAi }
}

pub async fn assemble(p: Prov, src: ByteSource, observe: impl FnMut(&Partial) -> Control) -> Result<Turn, TurnError> {
    match p {
        Prov::OpenAi => factory_agent::openai::assemble(src, observe).await,
        Prov::Anthropic => factory_agent::anthropic::assemble(src, observe).await,
    }
}

fn parsed(rec: &Recording) -> Vec<(usize, usize, String, Value)> {
    rec.events().into_iter().map(|(s, e, d)| {
        let v = serde_json::from_str::<Value>(&d).unwrap_or(Value::Null);
        (s, e, d, v)
    }).collect()
}

/// Events that carry a non-empty fragment of tool-call argument JSON.
pub fn tool_arg_events(rec: &Recording) -> Vec<(usize, usize, String)> {
    let p = of(rec);
    parsed(rec).into_iter().filter(|(_, _, _, v)| match p {
        Prov::OpenAi => v["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str().is_some_and(|s| !s.is_empty()),
        Prov::Anthropic => v["type"] == "content_block_delta" && v["delta"]["type"] == "input_json_delta" && v["delta"]["partial_json"].as_str().is_some_and(|s| !s.is_empty()),
    }).map(|(s, e, d, _)| (s, e, d)).collect()
}

/// The event carrying the finish or stop reason.
pub fn finish_event(rec: &Recording) -> Option<(usize, usize, String)> {
    let p = of(rec);
    parsed(rec).into_iter().find(|(_, _, _, v)| match p {
        Prov::OpenAi => v["choices"][0]["finish_reason"].is_string(),
        Prov::Anthropic => v["type"] == "message_delta" && v["delta"]["stop_reason"].is_string(),
    }).map(|(s, e, d, _)| (s, e, d))
}

/// Needle that identifies the completion marker event.
pub fn done_needle(rec: &Recording) -> String {
    match of(rec) {
        Prov::OpenAi => "data: [DONE]".into(),
        Prov::Anthropic => "\"type\":\"message_stop\"".into(),
    }
}

pub fn content_events(rec: &Recording) -> Vec<(usize, usize, String)> {
    let p = of(rec);
    parsed(rec).into_iter().filter(|(_, _, _, v)| match p {
        Prov::OpenAi => v["choices"][0]["delta"]["content"].as_str().is_some_and(|s| !s.is_empty()),
        Prov::Anthropic => v["type"] == "content_block_delta" && v["delta"]["type"] == "text_delta" && v["delta"]["text"].as_str().is_some_and(|s| !s.is_empty()),
    }).map(|(s, e, d, _)| (s, e, d)).collect()
}
