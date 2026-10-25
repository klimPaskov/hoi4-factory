//! Live Anthropic calls (maintainer pre-approved, S03). Ignored by default.
//!
//! Run with `ANTHROPIC_API_KEY` set: `cargo test -p factory-agent -- --ignored`.
//! The key is read with `ApiKey::from_env` and never printed. Each test prints
//! the reported usage and the estimated cost for the spend record.

mod common;

use common::*;
use factory_agent::client::ProviderClient;
use factory_agent::cost::{MicroCost, estimate_micros};
use factory_agent::destination::DestinationPolicy;
use factory_agent::openai::ApiKey;
use factory_agent::probe::{ProbeMode, probe};
use factory_agent::profile::{Basis, Capability, CapabilityState, ModelProfile};

fn key() -> ApiKey {
    ApiKey::from_env("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is set")
}

#[tokio::test]
#[ignore = "live: needs ANTHROPIC_API_KEY"]
async fn live_listing_probe() {
    let profile = ModelProfile::anthropic_haiku();
    let r = probe(
        &profile,
        key(),
        &DestinationPolicy::default(),
        ProbeMode::Listing,
    )
    .await;
    println!(
        "connection: {:?}; models listed: {}; error: {:?}",
        r.connection.state,
        r.models.len(),
        r.error
    );
    assert_eq!(r.connection.state, CapabilityState::Supported);
    assert!(r.lists_model(&profile.model), "claude-haiku-5-5 is listed");
    assert_eq!(
        r.capabilities.get(Capability::ModelListing).basis,
        Basis::Probe
    );
}

#[tokio::test]
#[ignore = "live: needs ANTHROPIC_API_KEY"]
async fn live_inference_probe() {
    let profile = ModelProfile::anthropic_haiku();
    let r = probe(
        &profile,
        key(),
        &DestinationPolicy::default(),
        ProbeMode::Inference,
    )
    .await;
    println!(
        "connection: {:?}; notes: {:?}; error: {:?}",
        r.connection.state, r.notes, r.error
    );
    for c in [
        Capability::Text,
        Capability::ToolCalls,
        Capability::Streaming,
        Capability::UsageAccounting,
    ] {
        let o = r.capabilities.get(c);
        println!("{c:?}: {:?} via {:?}: {}", o.state, o.basis, o.evidence);
    }
    let usage = r.usage.clone().expect("usage reported");
    println!(
        "usage: {usage:?}; estimate: {:?}",
        estimate_micros(&profile.model, Some(&usage), 0, 0)
    );
    assert_eq!(r.connection.state, CapabilityState::Supported);
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
        Basis::Probe,
        "{:?}",
        r.notes
    );
}

#[tokio::test]
#[ignore = "live: needs ANTHROPIC_API_KEY"]
async fn live_tool_turn_through_the_policy_client() {
    let profile = ModelProfile::anthropic_haiku();
    let client = ProviderClient::new(profile.clone(), key(), &DestinationPolicy::default())
        .expect("public endpoint passes the policy");
    let (_tx, rx) = never_cancel();
    let turn = client
        .run_turn(&tool_request(), rx, None)
        .await
        .expect("live turn");
    println!(
        "finish: {}; calls: {:?}; usage: {:?}",
        turn.finish_reason,
        turn.tool_calls
            .iter()
            .map(|c| (&c.name, &c.arguments))
            .collect::<Vec<_>>(),
        turn.usage
    );
    let cost = estimate_micros(&profile.model, turn.usage.as_ref(), 0, 0);
    println!("estimated cost: {cost:?}");
    assert_eq!(turn.finish_reason, "tool_use");
    assert_eq!(turn.tool_calls[0].name, "write_file");
    assert!(matches!(cost, MicroCost::Estimated { .. }));
}
