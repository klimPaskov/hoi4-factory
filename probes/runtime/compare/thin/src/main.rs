//! S01 comparison: one streamed tool-call turn through the thin adapter.
//! Usage: compare-thin <base_url>. Key from OPENAI_API_KEY.
use factory_agent::openai::{ApiKey, Control, OpenAiClient, assemble};
use serde_json::json;

#[tokio::main]
async fn main() {
    let base = std::env::args().nth(1).unwrap_or_else(|| "https://api.openai.com/v1".into());
    let key = ApiKey::from_env("OPENAI_API_KEY").unwrap_or_else(|| ApiKey::new("no-key".into()));
    let body: serde_json::Value = serde_json::from_str(include_str!("../../body.json")).unwrap();
    let c = OpenAiClient::new(&base, key, std::time::Duration::from_secs(60)).unwrap();
    let out = match c.start(&body, None).await {
        Err(e) => json!({"ok": false, "class": e.class(), "error": e.to_string(), "auto_retry_allowed": e.auto_retry_allowed()}),
        Ok((src, _)) => match assemble(src, |_| Control::Continue).await {
            Ok(t) => json!({"ok": true, "finish_reason": t.finish_reason, "tool_calls": t.tool_calls.len(), "args_parse": "ok"}),
            Err(e) => json!({"ok": false, "class": e.class(), "error": e.to_string(), "auto_retry_allowed": e.auto_retry_allowed()}),
        },
    };
    println!("{}", json!({"client": "factory-agent thin adapter", "result": out}));
}
