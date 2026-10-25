//! Capability probing against the replay server.
//!
//! The Anthropic inference case replays the recorded tool turn, whose tool is
//! `write_file`, not the probe's `probe_echo`: the probe must upgrade only what
//! it observed (streaming, usage) and leave tool calls and text as they were.

mod common;

use common::*;
use factory_agent::destination::DestinationPolicy;
use factory_agent::openai::ApiKey;
use factory_agent::probe::{ProbeMode, probe};
use factory_agent::profile::{Basis, Capability, CapabilityState};
use factory_agent::testing::{Recorded, ReplayServer, Reply};
use factory_agent::transport::Fault;

fn key() -> ApiKey {
    ApiKey::new("test-key-not-a-real-key".into())
}

#[tokio::test]
async fn listing_probe_sets_connection_and_listing_and_spends_nothing() {
    let server = ReplayServer::start(vec![
        (
            "/models",
            Reply::json(r#"{"data":[{"id":"llama3"},{"id":"qwen"}]}"#),
        ),
        ("/chat/completions", Reply::refusal(500, "{}")),
    ])
    .await;
    let profile = compat_profile(&server.base_url);
    let r = probe(&profile, key(), &loopback_policy(), ProbeMode::Listing).await;
    assert_eq!(r.connection.state, CapabilityState::Supported);
    assert_eq!(r.connection.basis, Basis::Probe);
    assert_eq!(r.models, vec!["llama3", "qwen"]);
    assert!(r.lists_model("qwen") && !r.lists_model("synthetic-model"));
    let caps = &r.capabilities;
    assert_eq!(caps.get(Capability::ModelListing).basis, Basis::Probe);
    // Nothing else is claimed for an OpenAI-compatible endpoint.
    for c in [
        Capability::Text,
        Capability::ToolCalls,
        Capability::Streaming,
        Capability::StrictJsonSchema,
        Capability::UsageAccounting,
    ] {
        assert_eq!(caps.get(c).state, CapabilityState::Unknown, "{c:?}");
    }
    assert_eq!(
        server.request_count("/chat/completions"),
        0,
        "listing mode sends no inference request"
    );
    assert_eq!(server.request_count("/models"), 1);
    assert!(
        r.probed_at.ends_with('Z') && r.probed_at.len() == 20,
        "{}",
        r.probed_at
    );
}

#[tokio::test]
async fn anthropic_inference_probe_upgrades_only_what_was_observed() {
    let rec = Recorded::load("anthropic-tool-turn");
    let server = ReplayServer::start(vec![
        (
            "/v1/models",
            Reply::json(r#"{"data":[{"id":"claude-haiku-5-5"}],"has_more":false}"#),
        ),
        ("/v1/messages", rec.reply(Fault::None)),
    ])
    .await;
    let r = probe(
        &anthropic_profile(&server.base_url),
        key(),
        &loopback_policy(),
        ProbeMode::Inference,
    )
    .await;
    assert_eq!(r.connection.state, CapabilityState::Supported);
    assert!(r.lists_model("claude-haiku-5-5"));
    assert_eq!(
        r.capabilities.get(Capability::Streaming).basis,
        Basis::Probe
    );
    assert_eq!(
        r.capabilities.get(Capability::UsageAccounting).basis,
        Basis::Probe
    );
    assert_eq!(
        r.capabilities.get(Capability::ToolCalls).basis,
        Basis::Advertised,
        "write_file is not probe_echo"
    );
    assert_eq!(
        r.capabilities.get(Capability::Text).basis,
        Basis::Advertised,
        "no text was observed"
    );
    assert!(r.notes.iter().any(|n| n.contains("probe_echo")));
    assert_eq!(r.usage.as_ref().map(|u| u.output_tokens), Some(84));
    let sent: serde_json::Value =
        serde_json::from_str(&server.request_bodies("/v1/messages")[0]).unwrap();
    assert_eq!(sent["max_tokens"], 64);
    assert_eq!(sent["tools"].as_array().unwrap().len(), 1);
    assert_eq!(
        sent["tools"][0]["input_schema"]["additionalProperties"],
        false
    );
}

#[tokio::test]
async fn compatible_inference_probe_observes_text_tools_streaming_and_usage() {
    let server = ReplayServer::start(vec![
        (
            "/models",
            Reply::json(r#"{"data":[{"id":"synthetic-model"}]}"#),
        ),
        (
            "/chat/completions",
            Reply::sse(
                synthetic("openai-synthetic-probe-turn.sse"),
                None,
                Fault::None,
            ),
        ),
    ])
    .await;
    let r = probe(
        &compat_profile(&server.base_url),
        key(),
        &loopback_policy(),
        ProbeMode::Inference,
    )
    .await;
    for c in [
        Capability::Text,
        Capability::ToolCalls,
        Capability::Streaming,
        Capability::UsageAccounting,
        Capability::ModelListing,
    ] {
        let o = r.capabilities.get(c);
        assert_eq!(
            (o.state, o.basis),
            (CapabilityState::Supported, Basis::Probe),
            "{c:?}"
        );
        assert!(o.observed_at.is_some());
    }
    // Strict JSON schema is never inferred from the wire format.
    assert_eq!(
        r.capabilities.get(Capability::StrictJsonSchema).state,
        CapabilityState::Unknown
    );
    assert_eq!(
        r.capabilities.get(Capability::ImageInput).basis,
        Basis::NotProbed
    );
    let sent: serde_json::Value =
        serde_json::from_str(&server.request_bodies("/chat/completions")[0]).unwrap();
    assert!(sent["tools"][0]["function"].get("strict").is_none());
}

#[tokio::test]
async fn auth_refusal_is_recorded_and_unobserved_capabilities_stay() {
    let body =
        r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let server = ReplayServer::start(vec![
        ("/v1/models", Reply::refusal(401, body)),
        ("/v1/messages", Reply::refusal(401, body)),
    ])
    .await;
    let profile = anthropic_profile(&server.base_url);
    let before = profile.starting_capabilities();
    let r = probe(&profile, key(), &loopback_policy(), ProbeMode::Inference).await;
    assert_eq!(r.connection.state, CapabilityState::Unsupported);
    assert_eq!(r.refusal.as_ref().map(|h| h.status), Some(401));
    assert_eq!(
        r.error.as_ref().map(|e| e.code.as_str()),
        Some("provider_auth_failed")
    );
    assert_eq!(
        r.capabilities, before,
        "an auth refusal observes nothing about the endpoint"
    );
    assert_eq!(
        server.request_count("/v1/messages"),
        0,
        "no inference was attempted with a refused key"
    );
}

#[tokio::test]
async fn quota_refusal_on_inference_keeps_capabilities_and_is_terminal() {
    let quota = r#"{"error":{"type":"insufficient_quota","code":"credit_balance_exhausted","message":"m"}}"#;
    let server = ReplayServer::start(vec![
        ("/models", Reply::json(r#"{"data":[]}"#)),
        ("/chat/completions", Reply::refusal(429, quota)),
    ])
    .await;
    let profile = compat_profile(&server.base_url);
    let r = probe(&profile, key(), &loopback_policy(), ProbeMode::Inference).await;
    assert_eq!(
        r.error.as_ref().map(|e| e.code.as_str()),
        Some("provider_quota_exhausted")
    );
    assert_ne!(
        r.connection.state,
        CapabilityState::Supported,
        "a refused inference is not a working connection"
    );
    assert_eq!(
        r.capabilities.get(Capability::ToolCalls).state,
        CapabilityState::Unknown
    );
}

#[tokio::test]
async fn a_broken_stream_never_leaves_streaming_supported() {
    let rec = Recorded::load("anthropic-tool-turn");
    let server = ReplayServer::start(vec![
        ("/v1/models", Reply::json(r#"{"data":[]}"#)),
        ("/v1/messages", rec.reply(Fault::TruncateAt(300))),
    ])
    .await;
    let r = probe(
        &anthropic_profile(&server.base_url),
        key(),
        &loopback_policy(),
        ProbeMode::Inference,
    )
    .await;
    assert_ne!(
        r.capabilities.get(Capability::Streaming).state,
        CapabilityState::Supported
    );
    assert_eq!(
        r.capabilities.get(Capability::Streaming).basis,
        Basis::Probe
    );
    assert_ne!(r.connection.state, CapabilityState::Supported);
    assert!(r.error.is_some());
}

#[tokio::test]
async fn listing_404_marks_listing_unsupported_and_blocked_destination_marks_connection() {
    let server = ReplayServer::start(vec![]).await; // every path answers 404
    let r = probe(
        &compat_profile(&server.base_url),
        key(),
        &loopback_policy(),
        ProbeMode::Listing,
    )
    .await;
    assert_eq!(
        r.capabilities.get(Capability::ModelListing).state,
        CapabilityState::Unsupported
    );
    assert_ne!(r.connection.state, CapabilityState::Supported);

    let r = probe(
        &compat_profile(&server.base_url),
        key(),
        &DestinationPolicy::default(),
        ProbeMode::Inference,
    )
    .await;
    assert_eq!(r.connection.state, CapabilityState::Unsupported);
    assert_eq!(
        r.error.as_ref().map(|e| e.code.as_str()),
        Some("destination_blocked")
    );
    assert_eq!(
        server.request_count("/models"),
        1,
        "only the earlier allowed probe reached the server"
    );
}

#[tokio::test]
async fn an_unreachable_endpoint_is_unsupported() {
    // Port 9 (discard) on loopback is closed on test hosts; the connection is refused before acceptance.
    let mut p = compat_profile("http://127.0.0.1:9");
    p.timeout_secs = 5;
    let r = probe(&p, key(), &loopback_policy(), ProbeMode::Inference).await;
    assert_eq!(r.connection.state, CapabilityState::Unsupported);
    assert_eq!(
        r.error.as_ref().map(|e| e.code.as_str()),
        Some("provider_unreachable")
    );
}
