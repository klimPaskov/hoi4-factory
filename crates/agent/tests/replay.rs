//! Provider turns through the real HTTP stack against the replay server.
//!
//! Anthropic cases replay recorded real bytes (evidence kind
//! `recorded-real-replay`); faults are injected into those bytes. OpenAI cases
//! use synthetic fixtures, labelled in their file names, because no real
//! OpenAI stream exists. Refusal bodies are the providers' documented error
//! shapes. Every case asserts the request count, so an accepted request that
//! failed is shown to have been sent exactly once.

mod common;

use common::*;
use factory_agent::errors::{CostCertainty, RetryClass};
use factory_agent::testing::{Recorded, ReplayServer, Reply};
use factory_agent::transport::Fault;
use factory_agent::types::{Acceptance, TurnError};

const MESSAGES: &str = "/v1/messages";
const COMPLETIONS: &str = "/chat/completions";

async fn anthropic_turn(
    reply: Reply,
) -> (Result<factory_agent::types::Turn, TurnError>, ReplayServer) {
    let server = ReplayServer::start(vec![(MESSAGES, reply)]).await;
    let c = client(anthropic_profile(&server.base_url));
    let (_tx, rx) = never_cancel();
    let r = c.run_turn(&tool_request(), rx, None).await;
    (r, server)
}

#[tokio::test]
async fn clean_tool_turn_from_recorded_bytes() {
    let rec = Recorded::load("anthropic-tool-turn");
    let (r, server) = anthropic_turn(rec.reply(Fault::None)).await;
    let turn = r.expect("clean turn");
    assert_eq!(turn.finish_reason, "tool_use");
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].name, "write_file");
    assert_eq!(turn.tool_calls[0].arguments["path"], "notes/probe.txt");
    assert_eq!(
        turn.usage
            .as_ref()
            .map(|u| (u.input_tokens, u.output_tokens)),
        Some((508, 84))
    );
    assert_eq!(server.request_count(MESSAGES), 1);
    // The rendered request is what the provider saw.
    let body: serde_json::Value =
        serde_json::from_str(&server.request_bodies(MESSAGES)[0]).unwrap();
    assert_eq!(body["model"], "claude-haiku-5-5");
    assert_eq!(body["tools"][0]["name"], "write_file");
    assert_eq!(body["stream"], true);
}

#[tokio::test]
async fn recorded_text_turn() {
    let rec = Recorded::load("anthropic-text-turn");
    let (r, _s) = anthropic_turn(rec.reply(Fault::None)).await;
    let turn = r.unwrap();
    assert_eq!(turn.finish_reason, "end_turn");
    assert!(turn.text.contains("`.txt` extension"));
}

#[tokio::test]
async fn missing_finish_reason_is_incomplete_and_never_retried() {
    let rec = Recorded::load("anthropic-tool-turn");
    let (r, server) =
        anthropic_turn(rec.reply(Fault::DropEvents(vec!["message_delta".into()]))).await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::IncompleteOutput { .. }), "{e}");
    let n = e.normalized();
    assert_eq!(
        (n.retry_class, n.cost),
        (RetryClass::ReconcileFirst, CostCertainty::Possible)
    );
    assert!(!e.auto_retry_allowed());
    assert_eq!(server.request_count(MESSAGES), 1);
}

#[tokio::test]
async fn one_byte_chunks_assemble_the_same_turn() {
    let rec = Recorded::load("anthropic-tool-turn");
    let sparse = Reply::sse(
        rec.bytes.clone(),
        Some(vec![1; rec.bytes.len()]),
        Fault::None,
    );
    let (r, _s) = anthropic_turn(sparse).await;
    let turn = r.expect("sparse chunks");
    assert_eq!(
        turn.tool_calls[0].raw_arguments,
        "{\"content\": \"S01 runtime probe ok\", \"path\": \"notes/probe.txt\"}"
    );
    assert_eq!(turn.bytes, rec.bytes.len());
}

#[tokio::test]
async fn truncated_streams_are_incomplete() {
    let rec = Recorded::load("anthropic-tool-turn");
    let mid_event = rec.end_of_event_containing("01 runt") - 20;
    let at_boundary = rec.end_of_event_containing("01 runt");
    for cut in [mid_event, at_boundary] {
        let (r, server) = anthropic_turn(rec.reply(Fault::TruncateAt(cut))).await;
        let e = r.unwrap_err();
        assert!(
            matches!(e, TurnError::IncompleteOutput { .. }),
            "cut {cut}: {e}"
        );
        if let TurnError::IncompleteOutput { partial, .. } = &e {
            assert!(
                partial.tool_calls[0].raw_arguments.len() < 60,
                "partial arguments are kept for diagnosis only"
            );
        }
        assert_eq!(e.normalized().retry_class, RetryClass::ReconcileFirst);
        assert_eq!(server.request_count(MESSAGES), 1);
    }
}

#[tokio::test]
async fn disconnect_after_acceptance_is_reconcile_first_with_one_request() {
    let rec = Recorded::load("anthropic-tool-turn");
    let cut = rec.end_of_event_containing("01 runt");
    let (r, server) = anthropic_turn(rec.reply(Fault::DisconnectAt(cut))).await;
    let e = r.unwrap_err();
    let n = e.normalized();
    assert_eq!(
        (n.retry_class, n.cost),
        (RetryClass::ReconcileFirst, CostCertainty::Possible),
        "{e}"
    );
    assert!(!e.auto_retry_allowed());
    if let TurnError::Transport { acceptance, .. } = &e {
        assert_eq!(*acceptance, Acceptance::Accepted);
    }
    assert_eq!(server.request_count(MESSAGES), 1, "no automatic retry");
}

#[tokio::test]
async fn malformed_tool_arguments_are_terminal_for_the_turn() {
    let rec = Recorded::load("anthropic-tool-turn");
    // Remove the last argument fragment so the assembled text is not valid JSON.
    let (r, server) = anthropic_turn(rec.reply(Fault::DropEvents(vec!["e.txt\\\"}".into()]))).await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::MalformedToolArguments { .. }), "{e}");
    let n = e.normalized();
    assert_eq!(
        (n.code.as_str(), n.retry_class),
        ("malformed_tool_arguments", RetryClass::Terminal)
    );
    assert_eq!(server.request_count(MESSAGES), 1);
}

#[tokio::test]
async fn anthropic_refusals_are_classified_from_the_response_body() {
    let cases = [
        (
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
            "provider_auth_failed",
            RetryClass::RequiresAuthorization,
        ),
        (
            429,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#,
            "provider_rate_limited",
            RetryClass::SafeSameInputs,
        ),
        (
            400,
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API."}}"#,
            "provider_quota_exhausted",
            RetryClass::Terminal,
        ),
        (
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
            "provider_overloaded",
            RetryClass::SafeSameInputs,
        ),
    ];
    for (status, body, code, class) in cases {
        let (r, server) = anthropic_turn(Reply::refusal(status, body)).await;
        let n = r.unwrap_err().normalized();
        assert_eq!(
            (n.code.as_str(), n.retry_class, n.provider_status),
            (code, class, Some(status))
        );
        assert_eq!(n.cost, CostCertainty::None);
        assert_eq!(server.request_count(MESSAGES), 1);
    }
}

#[tokio::test]
async fn openai_refusals_401_429_and_the_recorded_quota_shape() {
    let cases = [
        (
            401,
            r#"{"error":{"message":"Incorrect API key provided: sk-abc***xyz.","type":"invalid_request_error","code":"invalid_api_key"}}"#,
            "provider_auth_failed",
            RetryClass::RequiresAuthorization,
        ),
        (
            429,
            r#"{"error":{"message":"Rate limit reached","type":"requests","code":"rate_limit_exceeded"}}"#,
            "provider_rate_limited",
            RetryClass::SafeSameInputs,
        ),
        // Shape of the live S01-L06 refusal (evidence/S01/exp03/runs/live-record-quota-refusal.json).
        (
            429,
            r#"{"error":{"type":"insufficient_quota","code":"credit_balance_exhausted","message":"m"}}"#,
            "provider_quota_exhausted",
            RetryClass::Terminal,
        ),
    ];
    for (status, body, code, class) in cases {
        let server = ReplayServer::start(vec![(COMPLETIONS, Reply::refusal(status, body))]).await;
        let c = client(compat_profile(&server.base_url));
        let (_tx, rx) = never_cancel();
        let e = c.run_turn(&tool_request(), rx, None).await.unwrap_err();
        let n = e.normalized();
        assert_eq!((n.code.as_str(), n.retry_class), (code, class));
        assert_eq!(e.auto_retry_allowed(), class == RetryClass::SafeSameInputs);
        assert_eq!(server.request_count(COMPLETIONS), 1);
        let shown = format!("{e:?}");
        assert!(!shown.contains("sk-abc"), "key-like text is redacted");
    }
}

#[tokio::test]
async fn openai_synthetic_streams() {
    let tool = synthetic("openai-synthetic-tool-turn.sse");
    let server = ReplayServer::start(vec![(
        COMPLETIONS,
        Reply::sse(tool.clone(), None, Fault::None),
    )])
    .await;
    let c = client(compat_profile(&server.base_url));
    let (_tx, rx) = never_cancel();
    let turn = c
        .run_turn(&tool_request(), rx, None)
        .await
        .expect("synthetic tool turn");
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(
        turn.tool_calls[0].arguments,
        serde_json::json!({"path": "notes/synthetic.txt", "content": "ok"})
    );
    assert_eq!(turn.usage.as_ref().map(|u| u.input_tokens), Some(120));
    // An OpenAI-compatible endpoint is never assumed to support strict schemas.
    let body: serde_json::Value =
        serde_json::from_str(&server.request_bodies(COMPLETIONS)[0]).unwrap();
    assert!(body["tools"][0]["function"].get("strict").is_none());
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["max_completion_tokens"], 300);

    // Missing [DONE] marker is an incomplete turn.
    let server = ReplayServer::start(vec![(
        COMPLETIONS,
        Reply::sse(tool, None, Fault::DropEvents(vec!["[DONE]".into()])),
    )])
    .await;
    let c = client(compat_profile(&server.base_url));
    let (_tx, rx) = never_cancel();
    let e = c.run_turn(&tool_request(), rx, None).await.unwrap_err();
    assert!(matches!(e, TurnError::IncompleteOutput { .. }), "{e}");
    assert_eq!(server.request_count(COMPLETIONS), 1);

    let text = synthetic("openai-synthetic-text-turn.sse");
    let server = ReplayServer::start(vec![(
        COMPLETIONS,
        Reply::sse(text, Some(vec![7; 200]), Fault::None),
    )])
    .await;
    let c = client(compat_profile(&server.base_url));
    let (_tx, rx) = never_cancel();
    assert_eq!(
        c.run_turn(&tool_request(), rx, None).await.unwrap().text,
        "Hello synthetic world"
    );
}

#[tokio::test]
async fn tool_result_round_trip_reaches_the_provider() {
    use factory_agent::conversation::{Conversation, Step, ToolResultEntry};
    let rec = Recorded::load("anthropic-followup-turn");
    let server = ReplayServer::start(vec![(MESSAGES, rec.reply(Fault::None))]).await;
    let c = client(anthropic_profile(&server.base_url));
    let mut conv = Conversation::new(
        "sys",
        "Use the write_file tool to create notes/probe.txt whose content is one line: S01 runtime probe ok",
    );
    // Feed the recorded first turn into the conversation state.
    let first = Recorded::load("anthropic-tool-turn");
    let s1 = ReplayServer::start(vec![(MESSAGES, first.reply(Fault::None))]).await;
    let c1 = client(anthropic_profile(&s1.base_url));
    let (_t, rx) = never_cancel();
    let turn1 = c1
        .run_turn(&conv.request(&[write_file_tool()], 300).unwrap(), rx, None)
        .await
        .unwrap();
    let Step::ToolCalls(calls) = conv.push_turn(&turn1) else {
        panic!("expected tool calls")
    };
    conv.push_tool_results(vec![ToolResultEntry::ok(
        calls[0].id.clone(),
        "{\"ok\":true}",
    )])
    .unwrap();
    let (_t, rx) = never_cancel();
    let turn2 = c
        .run_turn(&conv.request(&[write_file_tool()], 300).unwrap(), rx, None)
        .await
        .unwrap();
    assert!(matches!(conv.push_turn(&turn2), Step::Finished { .. }));
    let sent: serde_json::Value =
        serde_json::from_str(&server.request_bodies(MESSAGES)[0]).unwrap();
    assert_eq!(sent["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(sent["messages"][2]["content"][0]["type"], "tool_result");
    assert_eq!(
        sent["messages"][2]["content"][0]["tool_use_id"],
        calls[0].id.as_str()
    );
}

#[tokio::test]
async fn cancellation_closes_a_stalled_stream() {
    let rec = Recorded::load("anthropic-cancelled-turn");
    let reply = Reply::Sse {
        bytes: rec.bytes.clone(),
        chunk_sizes: Some(rec.chunk_sizes.clone()),
        fault: Fault::None,
        stall_after_bytes: Some(1),
    };
    let server = ReplayServer::start(vec![(MESSAGES, reply)]).await;
    let c = client(anthropic_profile(&server.base_url));
    let (tx, rx) = never_cancel();
    let run = tokio::spawn(async move { c.run_turn(&tool_request(), rx, None).await });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    tx.send(true).unwrap();
    let e = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .expect("cancel is prompt")
        .unwrap()
        .unwrap_err();
    assert!(matches!(e, TurnError::Cancelled { .. }), "{e}");
    assert_eq!(e.normalized().retry_class, RetryClass::ReconcileFirst);
    assert_eq!(server.request_count(MESSAGES), 1);
}

#[tokio::test]
async fn oversize_response_is_refused() {
    let rec = Recorded::load("anthropic-tool-turn");
    let server = ReplayServer::start(vec![(MESSAGES, rec.reply(Fault::None))]).await;
    let c = factory_agent::client::ProviderClient::with_max_response_bytes(
        anthropic_profile(&server.base_url),
        factory_agent::openai::ApiKey::new("k".into()),
        &loopback_policy(),
        500,
    )
    .unwrap();
    let (_t, rx) = never_cancel();
    let e = c.run_turn(&tool_request(), rx, None).await.unwrap_err();
    assert_eq!(e.normalized().code, "provider_response_too_large");
    assert_eq!(e.normalized().retry_class, RetryClass::Terminal);
}

#[tokio::test]
async fn model_listing_through_the_policy_client() {
    let anthropic = ReplayServer::start(vec![(
        "/v1/models",
        Reply::json(r#"{"data":[{"id":"claude-haiku-5-5","type":"model"}],"has_more":false}"#),
    )])
    .await;
    let (status, ids) = client(anthropic_profile(&anthropic.base_url))
        .list_models()
        .await
        .unwrap();
    assert_eq!((status, ids), (200, vec!["claude-haiku-5-5".to_string()]));
    let compat = ReplayServer::start(vec![(
        "/models",
        Reply::json(r#"{"object":"list","data":[{"id":"llama3"},{"id":"qwen"}]}"#),
    )])
    .await;
    let (_, ids) = client(compat_profile(&compat.base_url))
        .list_models()
        .await
        .unwrap();
    assert_eq!(ids, vec!["llama3", "qwen"]);
}
