//! The server's [`ModelClient`] for Setup analysis: the Anthropic Messages API through the
//! `factory-agent` adapter.
//!
//! The server owns provider routing, the key and the model choice, so the model named in a
//! request is ignored in favour of the configured one (`FACTORY_ANTHROPIC_MODEL`, default
//! `claude-haiku-5-5`). Setup's system prompt and output schema are sent as the provider's native
//! structured output (`output_config.format = {type: "json_schema", schema}`), with the schema
//! keywords Anthropic refuses removed. Setup's own validator still enforces every limit on the
//! reply. The key is held in a [`SecretString`] and reaches the wire only as the `x-api-key`
//! header; it is not part of any error or log line.

use std::time::Duration;

use async_trait::async_trait;
use factory_agent::anthropic::{AnthropicClient, assemble, request_body};
use factory_agent::openai::{ApiKey, Control};
use factory_intake::SecretString;
use factory_setup_adapter::{
    ANTHROPIC_UNSUPPORTED_KEYWORDS, ModelClient, ModelError, ModelReply, ModelRequest,
    json_object_from_text, strip_schema_keywords,
};
use serde_json::json;

use crate::config::AnthropicSettings;

/// Largest reply the analysis may produce.
const MAX_TOKENS: u32 = 8000;
/// How long the provider may stay silent.
const READ_TIMEOUT: Duration = Duration::from_secs(180);

pub struct AnthropicModel {
    key: SecretString,
    model: String,
    base_url: String,
}

impl AnthropicModel {
    pub fn new(settings: &AnthropicSettings) -> Self {
        AnthropicModel {
            key: SecretString::new(settings.api_key.0.clone()),
            model: settings.model.clone(),
            base_url: settings.base_url.clone(),
        }
    }
}

impl std::fmt::Debug for AnthropicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicModel")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ModelClient for AnthropicModel {
    async fn complete(&self, request: ModelRequest) -> Result<ModelReply, ModelError> {
        let client = AnthropicClient::new(
            &self.base_url,
            ApiKey::new(self.key.expose().to_owned()),
            READ_TIMEOUT,
        )
        .map_err(|_| ModelError::Failed("the model client could not be created".into()))?;
        let schema = strip_schema_keywords(&request.output_schema, ANTHROPIC_UNSUPPORTED_KEYWORDS);
        let body = request_body(
            &self.model,
            Some(&request.system_prompt),
            vec![json!({"role": "user", "content": request.prompt})],
            &[],
            MAX_TOKENS,
            Some(json!({"output_config": {"format": {"type": "json_schema", "schema": schema}}})),
        );
        let (source, _meta) = client
            .start(&body, None)
            .await
            .map_err(|e| ModelError::Failed(e.to_string()))?;
        let turn = assemble(source, |_| Control::Continue)
            .await
            .map_err(|e| ModelError::Failed(e.to_string()))?;
        // The client extracts the JSON object from the text; Setup's validator decides whether
        // its content is acceptable.
        let reply = json_object_from_text(&turn.text).or_else(|first| {
            match (turn.text.find('{'), turn.text.rfind('}')) {
                (Some(a), Some(b)) if a < b => json_object_from_text(&turn.text[a..=b]),
                _ => Err(first),
            }
        })?;
        Ok(ModelReply {
            reply,
            response_id: turn.response_id,
            usage: turn
                .usage
                .map(|u| json!({"input_tokens": u.input_tokens, "output_tokens": u.output_tokens})),
        })
    }
}
