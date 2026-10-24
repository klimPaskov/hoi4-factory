//! S01 comparison: one streamed tool-call turn through async-openai 0.42.2.
//! Usage: compare-async-openai <base_url>. Key from OPENAI_API_KEY (any value
//! for a local replay server). Prints how the outcome surfaces.
use async_openai::{Client, config::OpenAIConfig, types::chat::CreateChatCompletionRequest};
use futures::StreamExt;
use serde_json::json;

#[tokio::main]
async fn main() {
    let base = std::env::args().nth(1).unwrap_or_else(|| "https://api.openai.com/v1".into());
    let key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "no-key".into());
    let body: serde_json::Value = serde_json::from_str(include_str!("../../body.json")).unwrap();
    let req: CreateChatCompletionRequest = serde_json::from_value(body).expect("request maps to typed struct");
    let client = Client::with_config(OpenAIConfig::new().with_api_base(base).with_api_key(key));
    let mut items = 0;
    let mut args = String::new();
    let mut finish = None;
    let mut errors = vec![];
    match client.chat().create_stream(req).await {
        Err(e) => errors.push(format!("{e}")),
        Ok(mut s) => {
            while let Some(r) = s.next().await {
                match r {
                    Ok(chunk) => {
                        items += 1;
                        for c in chunk.choices {
                            if let Some(tcs) = c.delta.tool_calls {
                                for tc in tcs {
                                    if let Some(a) = tc.function.and_then(|f| f.arguments) {
                                        args.push_str(&a);
                                    }
                                }
                            }
                            if let Some(f) = c.finish_reason {
                                finish = Some(format!("{f:?}"));
                            }
                        }
                    }
                    Err(e) => errors.push(format!("{e}")),
                }
            }
        }
    }
    let parse = serde_json::from_str::<serde_json::Value>(&args).map(|_| "ok").unwrap_or("invalid");
    println!("{}", json!({"client": "async-openai 0.42.2", "chunks": items, "finish_reason": finish, "args_len": args.len(), "args_parse": parse, "errors": errors, "stream_ended_cleanly_without_error": errors.is_empty()}));
}
