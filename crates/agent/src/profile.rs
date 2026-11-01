//! Model profiles and honest capability records.
//!
//! A profile names an endpoint and a model. Its capabilities are not assumed
//! from the provider kind alone: each carries a state (supported, unsupported,
//! unknown), the basis for that state (probe, advertised by documentation,
//! recorded real traffic, not probed) and the evidence. `Unknown` counts as
//! unsatisfied when a caller requires a capability, so an endpoint that merely
//! looks OpenAI-compatible never gets strict JSON schema or tool calling by
//! naming.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    OpenAiCompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// The model's context window in tokens (for Anthropic Haiku the price
    /// tier boundary).
    pub context_tokens: u32,
    pub max_output_tokens: u32,
    pub timeout_secs: u32,
}

impl ModelProfile {
    pub fn anthropic_haiku() -> Self {
        Self {
            id: "anthropic:claude-haiku-5-5".into(),
            provider: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            model: "claude-haiku-5-5".into(),
            context_tokens: 100_000,
            max_output_tokens: 4096,
            timeout_secs: 120,
        }
    }

    pub fn openai_nano() -> Self {
        Self {
            id: "openai:gpt-5.4-nano".into(),
            provider: ProviderKind::OpenAi,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-5.4-nano".into(),
            context_tokens: 100_000,
            max_output_tokens: 4096,
            timeout_secs: 120,
        }
    }

    /// A self-hosted OpenAI-compatible endpoint. The base URL includes the
    /// version prefix, for example `http://10.0.0.5:11434/v1`.
    pub fn openai_compatible(base_url: &str, model: &str) -> Self {
        Self {
            id: format!("compatible:{model}@{}", base_url.trim_end_matches('/')),
            provider: ProviderKind::OpenAiCompatible,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model: model.to_owned(),
            context_tokens: 8192,
            max_output_tokens: 2048,
            timeout_secs: 120,
        }
    }

    pub fn starting_capabilities(&self) -> Capabilities {
        Capabilities::starting(self.provider)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Text,
    ToolCalls,
    ParallelToolCalls,
    Streaming,
    StrictJsonSchema,
    ImageInput,
    UsageAccounting,
    Cancellation,
    ModelListing,
}

impl Capability {
    pub const ALL: [Capability; 9] = [
        Capability::Text,
        Capability::ToolCalls,
        Capability::ParallelToolCalls,
        Capability::Streaming,
        Capability::StrictJsonSchema,
        Capability::ImageInput,
        Capability::UsageAccounting,
        Capability::Cancellation,
        Capability::ModelListing,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Observed by a probe against this endpoint.
    Probe,
    /// Stated by the provider's documentation.
    Advertised,
    /// Observed in recorded real traffic of this model.
    Recorded,
    NotProbed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOutcome {
    pub state: CapabilityState,
    pub basis: Basis,
    pub evidence: String,
    pub observed_at: Option<String>,
}

impl CapabilityOutcome {
    pub fn new(
        state: CapabilityState,
        basis: Basis,
        evidence: impl Into<String>,
        observed_at: Option<String>,
    ) -> Self {
        Self {
            state,
            basis,
            evidence: evidence.into(),
            observed_at,
        }
    }
    pub fn unknown(evidence: impl Into<String>) -> Self {
        Self::new(CapabilityState::Unknown, Basis::NotProbed, evidence, None)
    }
    pub fn advertised(evidence: impl Into<String>) -> Self {
        Self::new(
            CapabilityState::Supported,
            Basis::Advertised,
            evidence,
            None,
        )
    }
    pub fn recorded(evidence: impl Into<String>) -> Self {
        Self::new(CapabilityState::Supported, Basis::Recorded, evidence, None)
    }
    pub fn probed(state: CapabilityState, evidence: impl Into<String>, at: &str) -> Self {
        Self::new(state, Basis::Probe, evidence, Some(at.to_owned()))
    }
    pub fn is_supported(&self) -> bool {
        self.state == CapabilityState::Supported
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[error("capabilities not satisfied: {missing:?}")]
pub struct CapabilityUnsatisfied {
    pub missing: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities(BTreeMap<Capability, CapabilityOutcome>);

impl Capabilities {
    /// What can honestly be said before any probe. OpenAI-compatible endpoints
    /// start all `Unknown`: the wire format is shared, the behavior is not.
    pub fn starting(kind: ProviderKind) -> Self {
        let mut m = BTreeMap::new();
        for c in Capability::ALL {
            m.insert(c, CapabilityOutcome::unknown("not probed"));
        }
        let docs =
            |what: &str| CapabilityOutcome::advertised(format!("provider documentation: {what}"));
        match kind {
            ProviderKind::Anthropic => {
                m.insert(Capability::Text, docs("Messages API text output"));
                m.insert(Capability::ToolCalls, docs("Messages API tool use"));
                m.insert(
                    Capability::Streaming,
                    docs("Messages API server-sent events"),
                );
                m.insert(
                    Capability::UsageAccounting,
                    docs("usage in message_start and message_delta"),
                );
                m.insert(Capability::ModelListing, docs("GET /v1/models"));
                m.insert(
                    Capability::Cancellation,
                    docs("closing the connection ends generation"),
                );
                m.insert(
                    Capability::StrictJsonSchema,
                    CapabilityOutcome::recorded("strict tool accepted by claude-haiku-5-5 in recording anthropic-tool-turn (2026-10-24)"),
                );
            }
            ProviderKind::OpenAi => {
                m.insert(Capability::Text, docs("Chat Completions text output"));
                m.insert(
                    Capability::ToolCalls,
                    docs("Chat Completions function tools"),
                );
                m.insert(
                    Capability::Streaming,
                    docs("Chat Completions server-sent events"),
                );
                m.insert(
                    Capability::UsageAccounting,
                    docs("stream_options.include_usage"),
                );
                m.insert(Capability::ModelListing, docs("GET /v1/models"));
                m.insert(
                    Capability::Cancellation,
                    docs("closing the connection ends generation"),
                );
                m.insert(
                    Capability::StrictJsonSchema,
                    docs("function tools with strict: true"),
                );
            }
            ProviderKind::OpenAiCompatible => {}
        }
        m.insert(
            Capability::ImageInput,
            CapabilityOutcome::unknown("image input is not used and not probed"),
        );
        Self(m)
    }

    pub fn get(&self, c: Capability) -> &CapabilityOutcome {
        // Every capability is inserted by `starting`; a deserialized map that
        // lacks one reads as unknown.
        static UNKNOWN: std::sync::OnceLock<CapabilityOutcome> = std::sync::OnceLock::new();
        self.0
            .get(&c)
            .unwrap_or_else(|| UNKNOWN.get_or_init(|| CapabilityOutcome::unknown("not recorded")))
    }

    pub fn set(&mut self, c: Capability, outcome: CapabilityOutcome) {
        self.0.insert(c, outcome);
    }

    pub fn is_supported(&self, c: Capability) -> bool {
        self.get(c).is_supported()
    }

    /// Succeeds only when every capability is `Supported`. `Unknown` and
    /// `Unsupported` both count as missing.
    pub fn require(&self, needed: &[Capability]) -> Result<(), CapabilityUnsatisfied> {
        let missing: Vec<Capability> = needed
            .iter()
            .copied()
            .filter(|c| !self.is_supported(*c))
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(CapabilityUnsatisfied { missing })
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Capability, &CapabilityOutcome)> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles() {
        let a = ModelProfile::anthropic_haiku();
        assert_eq!(
            (a.id.as_str(), a.base_url.as_str(), a.model.as_str()),
            (
                "anthropic:claude-haiku-5-5",
                "https://api.anthropic.com",
                "claude-haiku-5-5"
            )
        );
        assert_eq!(
            (a.context_tokens, a.max_output_tokens, a.timeout_secs),
            (100_000, 4096, 120)
        );
        let o = ModelProfile::openai_nano();
        assert_eq!(
            (o.id.as_str(), o.base_url.as_str()),
            ("openai:gpt-5.4-nano", "https://api.openai.com/v1")
        );
        let c = ModelProfile::openai_compatible("http://10.0.0.5:11434/v1/", "llama");
        assert_eq!(c.provider, ProviderKind::OpenAiCompatible);
        assert_eq!(c.base_url, "http://10.0.0.5:11434/v1");
    }

    #[test]
    fn compatible_endpoints_start_unknown_and_never_satisfy_requirements() {
        let caps = Capabilities::starting(ProviderKind::OpenAiCompatible);
        for c in Capability::ALL {
            assert_eq!(caps.get(c).state, CapabilityState::Unknown, "{c:?}");
        }
        let err = caps
            .require(&[Capability::ToolCalls, Capability::StrictJsonSchema])
            .unwrap_err();
        assert_eq!(
            err.missing,
            vec![Capability::ToolCalls, Capability::StrictJsonSchema]
        );
    }

    #[test]
    fn hosted_providers_start_advertised_and_image_input_is_not_probed() {
        for kind in [ProviderKind::Anthropic, ProviderKind::OpenAi] {
            let caps = Capabilities::starting(kind);
            caps.require(&[
                Capability::Text,
                Capability::ToolCalls,
                Capability::Streaming,
                Capability::UsageAccounting,
            ])
            .unwrap();
            assert_eq!(caps.get(Capability::Text).basis, Basis::Advertised);
            assert_eq!(caps.get(Capability::ImageInput).basis, Basis::NotProbed);
            assert!(!caps.is_supported(Capability::ImageInput));
            assert!(!caps.is_supported(Capability::ParallelToolCalls));
        }
    }

    #[test]
    fn unsupported_and_unknown_both_fail_require() {
        let mut caps = Capabilities::starting(ProviderKind::Anthropic);
        caps.set(
            Capability::Streaming,
            CapabilityOutcome::probed(
                CapabilityState::Unsupported,
                "no sse",
                "2026-10-25T00:00:00Z",
            ),
        );
        assert_eq!(
            caps.require(&[Capability::Streaming]).unwrap_err().missing,
            vec![Capability::Streaming]
        );
    }

    #[test]
    fn capabilities_roundtrip_json() {
        let caps = Capabilities::starting(ProviderKind::Anthropic);
        let back: Capabilities =
            serde_json::from_str(&serde_json::to_string(&caps).unwrap()).unwrap();
        assert_eq!(caps, back);
    }
}
