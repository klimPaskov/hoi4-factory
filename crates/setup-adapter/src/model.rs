//! The model seam of host-run analysis.
//!
//! Setup's analysis gate expects a structured JSON reply from a provider. The desktop calls the
//! provider itself; the factory server owns provider routing, keys and cost, so it supplies a
//! [`ModelClient`]. The adapter builds the prompt with Setup (`analysis_turn`), hands it to the
//! client, and gives the reply to Setup's deterministic validator (`record_analysis`). A reply
//! cannot choose its own analysis ID, skip its evidence bindings or confirm itself.

use async_trait::async_trait;
use hoi4_mod_setup::models::CredentialReference;
use serde_json::Value;

/// One analysis turn for the client to send.
#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub provider: String,
    pub model: String,
    pub reasoning_effort: String,
    pub endpoint: String,
    /// The user message Setup built: approved evidence, constraints and the input digest. It says
    /// "matching the supplied output schema" and does not repeat the schema, so a client must send
    /// `system_prompt` and `output_schema` with it.
    pub prompt: String,
    /// Setup's system prompt for the analysis, taken from Setup itself.
    pub system_prompt: String,
    /// Setup's analysis output schema (JSON Schema without `$schema` and `$id`). Send it as the
    /// provider's native structured output where the route supports it (Anthropic:
    /// `output_config.format = {type: "json_schema", schema}`).
    pub output_schema: Value,
    /// Digest of the analysis input; the reply must echo it.
    pub input_sha256: String,
    /// Reference into the server secret store. Never a key value.
    pub credential: Option<CredentialReference>,
}

/// Setup's analysis contract: the system prompt and the output schema it validates against.
/// Read from Setup's own request builder so the text cannot drift from the validator.
pub fn analysis_contract(model: &str) -> Result<(String, Value), ModelError> {
    let request = hoi4_setup_core::analysis::provider_request(
        "claude",
        "anthropic_messages",
        "https://api.anthropic.com/v1/messages",
        model,
        "high",
        "",
        false,
    )
    .map_err(|e| ModelError::Failed(format!("Setup could not build the analysis contract: {e}")))?;
    let system = request.body["system"].as_str().map(str::to_owned);
    let schema = request
        .body
        .pointer("/output_config/format/schema")
        .cloned();
    match (system, schema) {
        (Some(system), Some(schema)) => Ok((system, schema)),
        _ => Err(ModelError::Failed(
            "Setup's analysis request had no system prompt or schema".into(),
        )),
    }
}

/// The client's answer: the parsed JSON object the model produced.
#[derive(Debug, Clone)]
pub struct ModelReply {
    pub reply: Value,
    /// Provider response identifier, kept in the receipt.
    pub response_id: Option<String>,
    /// Token usage as the client reports it.
    pub usage: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    /// The provider could not be reached or refused the request.
    #[error("{0}")]
    Failed(String),
    /// The reply was not a JSON object.
    #[error("the reply was not a JSON object: {0}")]
    NotJson(String),
}

/// Supplied by the server. Implementations route to the configured provider.
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn complete(&self, request: ModelRequest) -> Result<ModelReply, ModelError>;
}

/// Keywords Anthropic's structured-output API refuses in a schema (observed from the live API:
/// "For 'number' type, properties maximum, minimum are not supported"). Setup's own validator
/// still enforces every limit on the reply, so dropping them from what is sent loses nothing that
/// is not checked afterwards.
pub const ANTHROPIC_UNSUPPORTED_KEYWORDS: &[&str] = &[
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "maxItems",
];

/// A copy of `schema` without the given keywords at any depth. A property that is itself named
/// like a keyword is kept: only schema positions are filtered, not `properties` names.
pub fn strip_schema_keywords(schema: &Value, keywords: &[&str]) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                if keywords.contains(&key.as_str()) && !value.is_object() {
                    continue;
                }
                if key == "minItems" && value.as_u64().is_some_and(|n| n > 1) {
                    continue;
                }
                let cleaned = if key == "properties" || key == "$defs" || key == "definitions" {
                    match value {
                        Value::Object(props) => Value::Object(
                            props
                                .iter()
                                .map(|(k, v)| (k.clone(), strip_schema_keywords(v, keywords)))
                                .collect(),
                        ),
                        other => other.clone(),
                    }
                } else {
                    strip_schema_keywords(value, keywords)
                };
                out.insert(key.clone(), cleaned);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| strip_schema_keywords(v, keywords))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Parse the JSON object out of model text. Accepts a bare object or one inside a Markdown code
/// fence, and nothing else: prose around the object is rejected rather than guessed at.
pub fn json_object_from_text(text: &str) -> Result<Value, ModelError> {
    let trimmed = text.trim();
    let body = match trimmed.strip_prefix("```") {
        Some(rest) => {
            let rest = rest.strip_prefix("json").unwrap_or(rest);
            rest.trim()
                .strip_suffix("```")
                .ok_or_else(|| ModelError::NotJson("unterminated code fence".into()))?
                .trim()
        }
        None => trimmed,
    };
    let value: Value =
        serde_json::from_str(body).map_err(|e| ModelError::NotJson(e.to_string()))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(ModelError::NotJson(
            "top-level value is not an object".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_unsupported_keywords_only_at_schema_positions() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "confidence": {"type": "number", "minimum": 0, "maximum": 1},
                "minimum": {"type": "string", "maxLength": 10},
                "tags": {"type": "array", "minItems": 1, "maxItems": 4, "items": {"type": "string", "minLength": 2}},
                "many": {"type": "array", "minItems": 3}
            },
            "required": ["confidence"]
        });
        let cleaned = strip_schema_keywords(&schema, ANTHROPIC_UNSUPPORTED_KEYWORDS);
        assert_eq!(
            cleaned,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "confidence": {"type": "number"},
                    "minimum": {"type": "string"},
                    "tags": {"type": "array", "minItems": 1, "items": {"type": "string"}},
                    "many": {"type": "array"}
                },
                "required": ["confidence"]
            })
        );
    }

    #[test]
    fn accepts_bare_and_fenced_objects_only() {
        assert!(json_object_from_text("{\"a\":1}").is_ok());
        assert!(json_object_from_text("```json\n{\"a\":1}\n```").is_ok());
        assert!(json_object_from_text("```\n{\"a\":1}\n```").is_ok());
        assert!(json_object_from_text("Here you go: {\"a\":1}").is_err());
        assert!(json_object_from_text("[1]").is_err());
        assert!(json_object_from_text("```json\n{\"a\":1}").is_err());
    }
}
