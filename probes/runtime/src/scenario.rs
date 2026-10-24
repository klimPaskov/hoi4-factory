//! Fixed probe requests. Prompts are short to keep live spend small.

use serde_json::{Value, json};

/// Selected after listing models on 2026-10-24: the lowest-cost current
/// GPT-5.4 family snapshot that supports streaming function calls.
pub const MODEL: &str = "gpt-5.4-nano-2026-03-17";

pub const SYSTEM: &str = "You are the HOI4 Factory S01 runtime probe. When asked to create a file, call the write_file tool exactly once. Keep output minimal.";

/// Schema sent to the provider (strict mode accepts no length bounds).
pub fn provider_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "content"],
        "properties": {
            "path": {"type": "string", "description": "Workspace-relative file path using forward slashes"},
            "content": {"type": "string", "description": "Complete file content"}
        }
    })
}

/// Closed host schema used for final validation. Independent of the provider.
pub fn host_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "content"],
        "properties": {
            "path": {"type": "string", "minLength": 1, "maxLength": 200, "pattern": "^[A-Za-z0-9._/-]+$"},
            "content": {"type": "string", "maxLength": 4096}
        }
    })
}

pub fn tools() -> Vec<Value> {
    vec![factory_agent::openai::function_tool(
        "write_file",
        "Create or replace one file inside the task workspace.",
        &provider_schema(),
        true,
    )]
}

pub fn tool_turn_messages() -> Vec<Value> {
    vec![
        json!({"role": "system", "content": SYSTEM}),
        json!({"role": "user", "content": "Create the file notes/probe.txt whose content is one line: S01 runtime probe ok"}),
    ]
}

pub fn tool_turn_body() -> Value {
    factory_agent::openai::request_body(
        MODEL,
        tool_turn_messages(),
        &tools(),
        Some(json!({"type": "function", "function": {"name": "write_file"}})),
        200,
        Some(json!({"reasoning_effort": "none"})),
    )
}

/// Second turn: the tool result the host actually produced is sent back.
pub fn followup_body(call_id: &str, raw_args: &str, tool_result: &Value) -> Value {
    let mut msgs = tool_turn_messages();
    msgs.push(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{"id": call_id, "type": "function", "function": {"name": "write_file", "arguments": raw_args}}]
    }));
    msgs.push(json!({"role": "tool", "tool_call_id": call_id, "content": tool_result.to_string()}));
    factory_agent::openai::request_body(MODEL, msgs, &tools(), Some(json!("none")), 80, Some(json!({"reasoning_effort": "none"})))
}

pub fn text_body() -> Value {
    factory_agent::openai::request_body(
        MODEL,
        vec![
            json!({"role": "system", "content": "Answer in one short sentence."}),
            json!({"role": "user", "content": "What file extension do Hearts of Iron IV script files use?"}),
        ],
        &[],
        None,
        60,
        Some(json!({"reasoning_effort": "none"})),
    )
}

pub fn long_body() -> Value {
    factory_agent::openai::request_body(
        MODEL,
        vec![json!({"role": "user", "content": "List 40 numbered one-line tips for writing Hearts of Iron IV national focus trees."})],
        &[],
        None,
        600,
        Some(json!({"reasoning_effort": "none"})),
    )
}

/// Anthropic profile. Claude Haiku 5.5, the current low-cost Claude model.
pub const A_MODEL: &str = "claude-haiku-5-5";

pub fn a_tools() -> Vec<Value> {
    vec![factory_agent::anthropic::tool("write_file", "Create or replace one file inside the task workspace.", &provider_schema(), true)]
}

/// Thinking is disabled (accepted on Haiku 5.5 at its default effort) so the
/// recorded stream holds only text and tool_use blocks. Tool choice stays
/// `auto` with an explicit instruction.
fn a_extra(with_tool_choice: bool) -> Value {
    let mut v = json!({"thinking": {"type": "disabled"}});
    if with_tool_choice {
        v["tool_choice"] = json!({"type": "auto"});
    }
    v
}

pub fn a_user_tool_prompt() -> Value {
    json!({"role": "user", "content": "Use the write_file tool to create notes/probe.txt whose content is one line: S01 runtime probe ok"})
}

pub fn a_tool_turn_body() -> Value {
    factory_agent::anthropic::request_body(A_MODEL, Some(SYSTEM), vec![a_user_tool_prompt()], &a_tools(), 300, Some(a_extra(true)))
}

pub fn a_followup_body(call_id: &str, input: &Value, tool_result: &Value) -> Value {
    let msgs = vec![
        a_user_tool_prompt(),
        json!({"role": "assistant", "content": [{"type": "tool_use", "id": call_id, "name": "write_file", "input": input}]}),
        json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": call_id, "content": tool_result.to_string()}]}),
    ];
    factory_agent::anthropic::request_body(A_MODEL, Some(SYSTEM), msgs, &a_tools(), 120, Some(a_extra(true)))
}

pub fn a_text_body() -> Value {
    factory_agent::anthropic::request_body(
        A_MODEL,
        Some("Answer in one short sentence."),
        vec![json!({"role": "user", "content": "What file extension do Hearts of Iron IV script files use?"})],
        &[],
        60,
        Some(a_extra(false)),
    )
}

pub fn a_long_body() -> Value {
    factory_agent::anthropic::request_body(
        A_MODEL,
        None,
        vec![json!({"role": "user", "content": "List 40 numbered one-line tips for writing Hearts of Iron IV national focus trees."})],
        &[],
        600,
        Some(a_extra(false)),
    )
}
