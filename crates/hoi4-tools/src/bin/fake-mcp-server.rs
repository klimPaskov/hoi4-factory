//! TEST DOUBLE. Not a real MCP server and never shipped: it exists so fault tests can make a
//! server crash, hang, send garbage or change its catalog on demand.
//!
//! Usage: `fake-mcp-server <mode>`. Modes: `normal`, `slow` (never answers `tools/call`),
//! `crash_on_call`, `crash_once` (crashes on the first call across restarts, using a marker file
//! next to the log), `garbage_stdout`, `oversized_line`, `catalog_changes` (the second
//! `tools/list` has a different schema), `wrong_protocol_version`, `server_request` (sends a
//! `roots/list` request before answering a call), `other_version` (reports version 9.9.9) and
//! `stubborn` (spawns a `sleep` helper and ignores stdin EOF).
//!
//! If `FAKE_MCP_LOG` is set, the server appends one line per event: `spawn <pid>`,
//! `env <sorted keys>`, `call <tool>`, `cancelled <id>`, `roots_reply <json>`, `helper <pid>`.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

fn log(line: &str) {
    if let Some(path) = std::env::var_os("FAKE_MCP_LOG")
        && let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

fn send(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

fn tools(variant: u32) -> Value {
    let extra = if variant > 1 {
        json!({"extra": {"type": "string"}})
    } else {
        json!({})
    };
    let mut props = json!({"text": {"type": "string"}, "repeat": {"type": "integer"}});
    if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            p.insert(k.clone(), v.clone());
        }
    }
    json!([
        {"name": "fake.read", "description": "echo", "inputSchema": {"type": "object", "properties": props, "additionalProperties": false}},
        {"name": "fake.job", "description": "derived job", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "additionalProperties": false}},
        {"name": "fake.write", "description": "writes", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "additionalProperties": false}}
    ])
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "normal".into());
    log(&format!("spawn {}", std::process::id()));
    let mut keys: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    keys.sort();
    log(&format!("env {}", keys.join(",")));
    if mode == "stubborn" {
        if let Ok(child) = std::process::Command::new("sleep").arg("300").spawn() {
            log(&format!("helper {}", child.id()));
        }
    }

    let stdin = std::io::stdin();
    let mut list_count = 0u32;
    let mut pending_call: Option<Value> = None;
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();
        match (method, id) {
            ("initialize", Some(id)) => {
                let version = if mode == "wrong_protocol_version" {
                    "2024-11-05"
                } else {
                    "2025-06-18"
                };
                let server_version = if mode == "other_version" {
                    "9.9.9"
                } else {
                    "1.0.0"
                };
                send(&json!({"jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion": version,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "fake-mcp", "version": server_version},
                    "instructions": "test double"
                }}));
            }
            ("tools/list", Some(id)) => {
                list_count += 1;
                let variant = if mode == "catalog_changes" {
                    list_count
                } else {
                    1
                };
                send(&json!({"jsonrpc":"2.0","id":id,"result":{"tools": tools(variant)}}));
            }
            ("tools/call", Some(id)) => {
                let name = msg["params"]["name"].as_str().unwrap_or("").to_owned();
                log(&format!("call {name}"));
                match mode.as_str() {
                    "slow" => continue,
                    "crash_on_call" => std::process::exit(3),
                    "crash_once" => {
                        let marker = std::env::var("FAKE_MCP_LOG").unwrap_or_default() + ".crashed";
                        if !std::path::Path::new(&marker).exists() {
                            let _ = std::fs::write(&marker, "x");
                            std::process::exit(3);
                        }
                    }
                    "garbage_stdout" => {
                        let mut out = std::io::stdout().lock();
                        let _ = writeln!(out, "this is not json");
                        let _ = out.flush();
                        continue;
                    }
                    "oversized_line" => {
                        let mut out = std::io::stdout().lock();
                        let _ = out.write_all(&vec![b'a'; 300_000]);
                        let _ = out.write_all(b"\n");
                        let _ = out.flush();
                        continue;
                    }
                    "server_request" => {
                        pending_call = Some(msg.clone());
                        send(&json!({"jsonrpc":"2.0","id":"srv-1","method":"roots/list"}));
                        // Also an unsupported request, which the client must refuse.
                        send(
                            &json!({"jsonrpc":"2.0","id":"srv-2","method":"sampling/createMessage","params":{}}),
                        );
                        continue;
                    }
                    _ => {}
                }
                send(&answer(&id, &msg));
            }
            ("notifications/cancelled", None) => {
                log(&format!("cancelled {}", msg["params"]["requestId"]));
            }
            ("", Some(id)) if id == json!("srv-1") || id == json!("srv-2") => {
                log(&format!("roots_reply {}", line));
                if id == json!("srv-2") {
                    if let Some(call) = pending_call.take() {
                        send(&answer(&call["id"], &call));
                    }
                }
            }
            _ => {}
        }
    }
    if mode == "stubborn" {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

fn answer(id: &Value, msg: &Value) -> Value {
    let args = &msg["params"]["arguments"];
    let text = args["text"].as_str().unwrap_or("");
    let repeat = args["repeat"].as_u64().unwrap_or(1).min(1_000_000) as usize;
    json!({"jsonrpc":"2.0","id":id,"result":{
        "content": [{"type": "text", "text": text.repeat(repeat)}],
        "structuredContent": {"echo": text, "args": args},
        "isError": false
    }})
}
