//! The destination policy through the real HTTP client (RT-11, EXT-14).

mod common;

use common::*;
use factory_agent::client::ProviderClient;
use factory_agent::destination::{DestinationPolicy, DestinationRefusal, outbound_client};
use factory_agent::openai::ApiKey;
use factory_agent::profile::ModelProfile;
use factory_agent::testing::{ReplayServer, Reply};
use factory_agent::types::TurnError;

fn refusal_in_chain(e: &reqwest::Error) -> Option<DestinationRefusal> {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = cur {
        if let Some(r) = err.downcast_ref::<DestinationRefusal>() {
            return Some(r.clone());
        }
        cur = std::error::Error::source(err);
    }
    None
}

#[tokio::test]
async fn private_destinations_are_refused_by_default_and_allowed_by_range() {
    let server = ReplayServer::start(vec![("/models", Reply::json(r#"{"data":[]}"#))]).await;
    let profile = ModelProfile::openai_compatible(&server.base_url, "m");
    let refused = ProviderClient::new(
        profile.clone(),
        ApiKey::new("k".into()),
        &DestinationPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(refused.code, "blocked_address");
    assert!(ProviderClient::new(profile, ApiKey::new("k".into()), &loopback_policy()).is_ok());
    assert_eq!(
        server.request_count("/models"),
        0,
        "construction made no request"
    );
}

#[tokio::test]
async fn a_hostname_that_resolves_to_loopback_is_refused_at_connect_time() {
    let server = ReplayServer::start(vec![("/", Reply::json("{}"))]).await;
    let port = server.base_url.rsplit(':').next().unwrap().to_owned();
    let http = outbound_client(
        &DestinationPolicy::default(),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    // `localhost` is only refused by name in check_url; here the URL check is bypassed
    // on purpose so the connect-time resolver is what stops the request.
    let err = http
        .get(format!("http://localhost:{port}/"))
        .send()
        .await
        .unwrap_err();
    let r = refusal_in_chain(&err).expect("policy refusal in the error chain");
    assert_eq!(r.code, "blocked_address");
    assert_eq!(
        server.request_count("/"),
        0,
        "no connection reached the server"
    );
    // The same client connects once the operator allows the range (both families).
    let allow = DestinationPolicy::allowing(&["127.0.0.0/8", "::1/128"]).unwrap();
    let http = outbound_client(&allow, std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(
        http.get(format!("http://localhost:{port}/"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

#[tokio::test]
async fn an_unresolvable_name_is_a_resolution_failure_not_a_pass() {
    let http = outbound_client(
        &DestinationPolicy::default(),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    let err = http
        .get("http://no-such-host.invalid/")
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        refusal_in_chain(&err).expect("refusal").code,
        "resolution_failed"
    );
}

#[tokio::test]
async fn redirects_are_refused_and_not_followed() {
    let target = ReplayServer::start(vec![("/v1/messages", Reply::json("{}"))]).await;
    let redirector = ReplayServer::start(vec![(
        "/v1/messages",
        Reply::Redirect {
            status: 307,
            location: format!("{}/v1/messages", target.base_url),
        },
    )])
    .await;
    let c = client(anthropic_profile(&redirector.base_url));
    let (_t, rx) = never_cancel();
    let e = c.run_turn(&tool_request(), rx, None).await.unwrap_err();
    let TurnError::Destination(r) = &e else {
        panic!("expected a destination refusal, got {e}")
    };
    assert_eq!(r.code, "redirect_refused");
    assert_eq!(e.normalized().code, "destination_blocked");
    assert_eq!(redirector.request_count("/v1/messages"), 1);
    assert_eq!(
        target.request_count("/v1/messages"),
        0,
        "the redirect target was never contacted"
    );
}

#[tokio::test]
async fn blocked_url_forms_fail_before_any_network_use() {
    for url in [
        "http://169.254.169.254/latest",
        "https://user:pass@api.anthropic.com",
        "ftp://x/y",
        "http://localhost:11434/v1",
        "http://[::ffff:10.0.0.1]/v1",
    ] {
        let mut p = ModelProfile::anthropic_haiku();
        p.base_url = url.into();
        assert!(
            ProviderClient::new(p, ApiKey::new("k".into()), &loopback_policy()).is_err(),
            "{url}"
        );
    }
    // The built-in public endpoints pass the URL check without any lookup.
    for p in [ModelProfile::anthropic_haiku(), ModelProfile::openai_nano()] {
        assert!(DestinationPolicy::default().check_url(&p.base_url).is_ok());
    }
}
