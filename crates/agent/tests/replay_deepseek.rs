//! The OpenAI-compatible adapter against recorded real DeepSeek streams (evidence
//! kind `recorded-real-replay`).
//!
//! The streams were captured live on 2026-11-02 through `live_deepseek.rs`
//! (maintainer decision D-21, issue #2) and are kept byte for byte under
//! `tests/fixtures/recorded/deepseek-*`. They cover what the synthetic OpenAI
//! fixtures cannot: DeepSeek's real chunk shapes (usage riding on the final chunk
//! with `finish_reason`, `reasoning_content` deltas, `prompt_cache_*` fields, a
//! `[DONE]` marker). Faults are injected into the real bytes, never into invented
//! model output. DeepSeek is not OpenAI, so this is evidence for the compatible
//! profile only.

mod common;

use common::*;
use factory_agent::errors::RetryClass;
use factory_agent::testing::{Recorded, ReplayServer, Reply};
use factory_agent::transport::Fault;
use factory_agent::types::{Turn, TurnError};

const COMPLETIONS: &str = "/chat/completions";

async fn turn(reply: Reply) -> (Result<Turn, TurnError>, ReplayServer) {
    let server = ReplayServer::start(vec![(COMPLETIONS, reply)]).await;
    let c = client(compat_profile(&server.base_url));
    let (_tx, rx) = never_cancel();
    let r = c.run_turn(&tool_request(), rx, None).await;
    (r, server)
}

#[tokio::test]
async fn a_real_deepseek_tool_turn_assembles() {
    let rec = Recorded::load("deepseek-tool-turn");
    let (r, server) = turn(rec.reply(Fault::None)).await;
    let t = r.expect("recorded tool turn");
    assert_eq!(t.finish_reason, "tool_calls");
    assert_eq!(t.model.as_deref(), Some("deepseek-flash"));
    assert_eq!(t.tool_calls.len(), 1);
    assert_eq!(t.tool_calls[0].name, "write_file");
    assert_eq!(
        t.tool_calls[0].arguments,
        serde_json::json!({"path": "notes/probe.txt", "content": "S01 runtime probe ok\n"})
    );
    assert_eq!(t.events, 25);
    assert_eq!(t.bytes, rec.bytes.len());
    // The final chunk carries both finish_reason and usage. Cached tokens come
    // from prompt_tokens_details, which equals DeepSeek's prompt_cache_hit_tokens.
    let u = t.usage.expect("usage on the final chunk");
    assert_eq!(
        (
            u.input_tokens,
            u.cached_input_tokens,
            u.output_tokens,
            u.reasoning_tokens
        ),
        (385, 251, 62, 0)
    );
    assert_eq!(server.request_count(COMPLETIONS), 1);
}

#[tokio::test]
async fn reasoning_deltas_are_not_part_of_the_turn() {
    let rec = Recorded::load("deepseek-reasoned-turn-1");
    let raw = String::from_utf8_lossy(&rec.bytes);
    assert!(
        raw.matches("\"reasoning_content\":\"").count() > 40,
        "the recording holds real reasoning deltas"
    );
    let (r, _server) = turn(rec.reply(Fault::None)).await;
    let t = r.expect("recorded reasoned turn");
    assert_eq!(t.finish_reason, "tool_calls");
    assert_eq!(t.text, "", "reasoning text is neither output nor persisted");
    assert_eq!(t.tool_calls[0].arguments["path"], "notes/alpha.txt");
    let u = t.usage.clone().expect("usage");
    assert_eq!((u.output_tokens, u.reasoning_tokens), (106, 48));
    assert!(
        !format!("{t:?}").contains("alphabetically"),
        "no reasoning text leaks into the turn"
    );
}

#[tokio::test]
async fn a_real_deepseek_text_turn_assembles() {
    let rec = Recorded::load("deepseek-nothink-turn-2");
    let (r, _server) = turn(rec.reply(Fault::None)).await;
    let t = r.expect("recorded text turn");
    assert_eq!(t.finish_reason, "stop");
    assert_eq!(t.text, "Created notes/probe.txt.");
    assert!(t.tool_calls.is_empty());
    let u = t.usage.clone().expect("usage");
    assert_eq!(
        (u.input_tokens, u.cached_input_tokens, u.output_tokens),
        (434, 256, 6)
    );
}

#[tokio::test]
async fn one_byte_chunks_assemble_the_same_deepseek_turn() {
    let rec = Recorded::load("deepseek-tool-turn");
    let whole = turn(rec.reply(Fault::None)).await.0.expect("whole");
    let sparse = Reply::sse(
        rec.bytes.clone(),
        Some(vec![1; rec.bytes.len()]),
        Fault::None,
    );
    let split = turn(sparse).await.0.expect("one-byte chunks");
    assert_eq!(split.tool_calls, whole.tool_calls);
    assert_eq!(split.usage, whole.usage);
    assert_eq!(split.events, whole.events);
}

#[tokio::test]
async fn faults_in_real_deepseek_bytes_are_incomplete_and_never_retried() {
    let rec = Recorded::load("deepseek-tool-turn");

    // The [DONE] marker is missing.
    let (r, server) = turn(rec.reply(Fault::DropEvents(vec!["[DONE]".into()]))).await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::IncompleteOutput { .. }), "{e}");
    assert!(!e.auto_retry_allowed());
    assert_eq!(e.normalized().retry_class, RetryClass::ReconcileFirst);
    assert_eq!(server.request_count(COMPLETIONS), 1);

    // The final chunk, which holds finish_reason and usage, is missing.
    let (r, server) = turn(rec.reply(Fault::DropEvents(vec![
        "\"finish_reason\":\"tool_calls\"".into(),
    ])))
    .await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::IncompleteOutput { .. }), "{e}");
    assert_eq!(server.request_count(COMPLETIONS), 1);

    // The stream ends cleanly in the middle of the tool arguments.
    let cut = rec.end_of_event_containing("\"name\":\"write_file\"");
    let (r, server) = turn(rec.reply(Fault::TruncateAt(cut))).await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::IncompleteOutput { .. }), "{e}");
    assert_eq!(server.request_count(COMPLETIONS), 1);

    // The connection drops after acceptance.
    let (r, server) = turn(rec.reply(Fault::DisconnectAt(cut))).await;
    let e = r.unwrap_err();
    assert!(matches!(e, TurnError::Transport { .. }), "{e}");
    assert!(!e.auto_retry_allowed());
    assert_eq!(server.request_count(COMPLETIONS), 1, "no automatic retry");
}

/// The max-token bound the adapter sends is `max_completion_tokens`. DeepSeek
/// ignores that field (observed live: 15 completion tokens against a bound of 8),
/// and honors `max_tokens`. The test pins what the adapter sends so a change
/// to either side is a deliberate one.
#[tokio::test]
async fn the_adapter_sends_max_completion_tokens_not_max_tokens() {
    let rec = Recorded::load("deepseek-tool-turn");
    let (r, server) = turn(rec.reply(Fault::None)).await;
    r.expect("turn");
    let body: serde_json::Value =
        serde_json::from_str(&server.request_bodies(COMPLETIONS)[0]).unwrap();
    assert_eq!(body["max_completion_tokens"], 300);
    assert!(body.get("max_tokens").is_none());
    assert!(
        body.get("thinking").is_none(),
        "no provider-specific fields are sent"
    );
    assert_eq!(
        rec.meta["request_body"]["max_completion_tokens"], 300,
        "the recording was made with the same body"
    );
}
