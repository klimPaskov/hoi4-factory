//! Provider-neutral turn results and failure classes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::transport::TransportError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub index: usize,
    pub id: String,
    pub name: String,
    /// Raw argument text exactly as assembled from deltas.
    pub raw_arguments: String,
    /// Parsed arguments. Only set on a complete turn whose text parsed.
    pub arguments: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

/// Partial state captured when a turn fails. Nothing in it is executable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Partial {
    pub text: String,
    pub tool_calls: Vec<PartialToolCall>,
    pub finish_reason: Option<String>,
    pub done_marker: bool,
    pub bytes: usize,
    pub events: usize,
    pub response_id: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartialToolCall {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub raw_arguments: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Turn {
    pub provider: &'static str,
    pub model: Option<String>,
    pub response_id: Option<String>,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub usage: Option<Usage>,
    pub bytes: usize,
    pub events: usize,
}

/// Deserialization shape of [`Turn`]: the provider name is owned on the wire
/// and mapped back to the known static constant.
#[derive(Deserialize)]
struct TurnWire {
    provider: String,
    model: Option<String>,
    response_id: Option<String>,
    text: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: String,
    usage: Option<Usage>,
    bytes: usize,
    events: usize,
}

impl<'de> Deserialize<'de> for Turn {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        TurnWire::deserialize(d).map(Turn::from)
    }
}

impl From<TurnWire> for Turn {
    fn from(w: TurnWire) -> Self {
        let provider = match w.provider.as_str() {
            "anthropic-messages" => "anthropic-messages",
            "openai-chat-completions" => "openai-chat-completions",
            _ => "unknown-provider",
        };
        Turn {
            provider,
            model: w.model,
            response_id: w.response_id,
            text: w.text,
            tool_calls: w.tool_calls,
            finish_reason: w.finish_reason,
            usage: w.usage,
            bytes: w.bytes,
            events: w.events,
        }
    }
}

/// Why a turn did not produce an executable result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[serde(tag = "class")]
pub enum TurnError {
    /// Stream ended without its completion marker or finish reason.
    #[error("incomplete output: {detail}")]
    IncompleteOutput { detail: String, partial: Partial },
    /// The transport broke. `acceptance` records whether the provider had
    /// accepted the request. Accepted or uncertain requests are never repeated
    /// automatically.
    #[error("transport failure ({acceptance:?}): {error}")]
    Transport {
        error: TransportError,
        acceptance: Acceptance,
        partial: Partial,
    },
    /// A tool call's final argument text is not valid JSON.
    #[error("malformed tool arguments for call {index}: {detail}")]
    MalformedToolArguments {
        index: usize,
        detail: String,
        partial: Partial,
    },
    /// An event payload was not valid JSON for the provider format.
    #[error("malformed stream event: {detail}")]
    MalformedEvent { detail: String, partial: Partial },
    /// The provider sent an error object inside the stream.
    #[error("provider stream error: {detail}")]
    ProviderStreamError { detail: String, partial: Partial },
    /// The caller cancelled. The request had been accepted.
    #[error("cancelled after acceptance")]
    Cancelled { partial: Partial },
    #[error("provider refused request: {0}")]
    Http(HttpRefusal),
    /// The outbound destination policy refused the request before any byte
    /// was sent to a provider (RT-11, EXT-14).
    #[error("destination refused: {0}")]
    Destination(crate::destination::DestinationRefusal),
}

impl TurnError {
    pub fn class(&self) -> &'static str {
        match self {
            TurnError::IncompleteOutput { .. } => "incomplete_output",
            TurnError::Transport { .. } => "transport",
            TurnError::MalformedToolArguments { .. } => "malformed_tool_arguments",
            TurnError::MalformedEvent { .. } => "malformed_event",
            TurnError::ProviderStreamError { .. } => "provider_stream_error",
            TurnError::Cancelled { .. } => "cancelled_after_acceptance",
            TurnError::Http(_) => "http_refusal",
            TurnError::Destination(_) => "destination_refused",
        }
    }

    /// Whether an automatic retry of the same provider request is allowed.
    /// Only a request known not to have been accepted, or one the provider
    /// refused with a transient status, may be retried. This is exactly the
    /// `safe_same_inputs` retry class of [`TurnError::normalized`].
    pub fn auto_retry_allowed(&self) -> bool {
        self.normalized().retry_class == crate::errors::RetryClass::SafeSameInputs
    }

    /// The contract-envelope view of this failure (retry class, cost certainty).
    pub fn normalized(&self) -> crate::errors::NormalizedError {
        crate::errors::normalize(self)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Acceptance {
    /// Connection failed before the request was written.
    NotAccepted,
    /// A response status was received.
    Accepted,
    /// The request may have been written but no status was observed.
    Uncertain,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HttpClass {
    Auth,
    Permission,
    Quota,
    RateLimited,
    InvalidRequest,
    NotFound,
    Overloaded,
    Server,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("HTTP {status} {class:?} {provider_code:?}")]
pub struct HttpRefusal {
    pub status: u16,
    pub class: HttpClass,
    pub provider_code: Option<String>,
    pub provider_type: Option<String>,
    /// Provider message with anything resembling a key redacted.
    pub message: Option<String>,
}

/// Remove anything that looks like a secret key from provider text.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for word in s.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        let trimmed = word.trim_end_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
        let looks_secret = trimmed.starts_with("sk-")
            || trimmed.starts_with("sk_")
            || (trimmed.len() > 32
                && trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        if looks_secret {
            out.push_str("[REDACTED]");
            out.push_str(&word[trimmed.len()..]);
        } else {
            out.push_str(word);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn redacts_key_like_tokens() {
        let r = super::redact("Incorrect API key provided: sk-abc123***xyz. See docs");
        assert!(!r.contains("sk-abc"));
        assert!(r.contains("[REDACTED]"));
    }
}
