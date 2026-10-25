//! Model profiles a task can use, and who pays for each.
//!
//! The built-in profiles are the default `claude-haiku-5-5` and OpenAI `gpt-5.4-nano`. A profile
//! runs on the operator allowance when the server holds an operator key for its provider, or on
//! the user's own saved key of the same provider. A self-hosted OpenAI-compatible endpoint is a
//! user key with a base URL; its capabilities start unknown (SPEC-002) and are learned only by
//! probing, so an agent task that needs tool calls is refused on it with
//! `capability_unsatisfied` until a probe shows them.

use factory_accounts::Redacted;
use factory_agent::profile::{Capabilities, Capability, CapabilityState, ModelProfile, ProviderKind};
use factory_broker::KeyProvider;
use serde_json::{Value, json};

/// The server's own provider keys, which fund the monthly allowance.
#[derive(Clone, Default)]
pub struct OperatorKeys {
    pub anthropic: Option<Redacted<String>>,
    pub openai: Option<Redacted<String>>,
    /// API bases of the built-in profiles (`FACTORY_ANTHROPIC_BASE_URL`; tests point them at a
    /// replay server).
    pub anthropic_base: Option<String>,
    pub openai_base: Option<String>,
}

impl OperatorKeys {
    pub fn profiles(&self) -> Vec<ModelProfile> {
        builtin(self.anthropic_base.as_deref(), self.openai_base.as_deref())
    }

    pub fn find(&self, id: &str) -> Option<ModelProfile> {
        self.profiles().into_iter().find(|p| p.id == id)
    }

    pub fn for_provider(&self, provider: ProviderKind) -> Option<&str> {
        match provider {
            ProviderKind::Anthropic => self.anthropic.as_ref().map(|k| k.0.as_str()),
            ProviderKind::OpenAi => self.openai.as_ref().map(|k| k.0.as_str()),
            ProviderKind::OpenAiCompatible => None,
        }
    }
}

/// The default profile when the user picks none (S03 inputs).
pub const DEFAULT_PROFILE: &str = "anthropic:claude-haiku-5-5";

/// What an agent attempt needs from a model.
pub const REQUIRED: &[Capability] = &[Capability::Text, Capability::ToolCalls, Capability::Streaming];

/// The built-in profiles, with the operator's configured API bases.
pub fn builtin(anthropic_base: Option<&str>, openai_base: Option<&str>) -> Vec<ModelProfile> {
    let mut anthropic = ModelProfile::anthropic_haiku();
    if let Some(base) = anthropic_base {
        anthropic.base_url = base.trim_end_matches('/').to_owned();
    }
    let mut openai = ModelProfile::openai_nano();
    if let Some(base) = openai_base {
        openai.base_url = base.trim_end_matches('/').to_owned();
    }
    vec![anthropic, openai]
}

pub fn label(profile: &ModelProfile) -> &'static str {
    match profile.provider {
        ProviderKind::Anthropic => "Claude Haiku 5.5",
        ProviderKind::OpenAi => "OpenAI GPT-5.4 nano",
        ProviderKind::OpenAiCompatible => "Self-hosted (OpenAI-compatible)",
    }
}

pub fn provider_name(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::OpenAi => "openai",
        ProviderKind::OpenAiCompatible => "openai_compatible",
    }
}

/// The key provider a profile accepts.
pub fn key_provider(kind: ProviderKind) -> KeyProvider {
    match kind {
        ProviderKind::Anthropic => KeyProvider::Anthropic,
        ProviderKind::OpenAi => KeyProvider::Openai,
        ProviderKind::OpenAiCompatible => KeyProvider::OpenaiCompatible,
    }
}

/// The destination string a grant names for a profile.
pub fn destination(profile: &ModelProfile) -> String {
    format!("{}:{}", provider_name(profile.provider), profile.base_url)
}

fn state_name(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::Supported => "supported",
        CapabilityState::Unsupported => "unsupported",
        CapabilityState::Unknown => "unknown",
    }
}

/// Capability summary for the browser: one state per capability, plus its evidence.
pub fn capability_summary(caps: &Capabilities) -> (Value, Value) {
    let mut states = serde_json::Map::new();
    let mut evidence = serde_json::Map::new();
    for (capability, outcome) in caps.iter() {
        let key = serde_json::to_value(capability)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        states.insert(key.clone(), json!(state_name(outcome.state)));
        evidence.insert(key, json!(outcome.evidence));
    }
    (Value::Object(states), Value::Object(evidence))
}
