//! Provider-neutral model request.
//!
//! This is the only conversation shape that crosses a host/worker boundary or
//! a provider handoff. It carries text, tool calls and tool results. It has no
//! field for provider-private reasoning (thinking blocks, signatures, encrypted
//! reasoning items), so such state can neither be persisted by accident nor
//! replayed to a different provider.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

impl Part {
    pub fn text(s: impl Into<String>) -> Self {
        Part::Text { text: s.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Message {
    pub fn user_text(s: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part::text(s)],
        }
    }
}

/// A tool the model may call. `input_schema` is a closed JSON Schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_output_tokens: u32,
}

impl ModelRequest {
    /// A rough upper bound on input tokens (4 bytes per token, rounded up,
    /// plus a fixed overhead per message and tool) for spend reservations.
    pub fn input_token_bound(&self) -> u64 {
        let mut bytes = self.system.len();
        for m in &self.messages {
            bytes += 16;
            for p in &m.parts {
                bytes += match p {
                    Part::Text { text } => text.len(),
                    Part::ToolUse { id, name, input } => {
                        id.len() + name.len() + input.to_string().len()
                    }
                    Part::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => tool_use_id.len() + content.len(),
                };
            }
        }
        for t in &self.tools {
            bytes += 32 + t.name.len() + t.description.len() + t.input_schema.to_string().len();
        }
        (bytes as u64).div_ceil(3) + 64
    }
}
