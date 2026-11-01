#![allow(dead_code)]
//! Shared helpers for the integration tests.

use factory_agent::client::ProviderClient;
use factory_agent::destination::DestinationPolicy;
use factory_agent::openai::ApiKey;
use factory_agent::profile::ModelProfile;
use factory_agent::request::{Message, ModelRequest, ToolSpec};
use serde_json::json;
use tokio::sync::watch;

/// The operator allowance path, tested on purpose: loopback is allowed.
pub fn loopback_policy() -> DestinationPolicy {
    DestinationPolicy::allowing(&["127.0.0.0/8"]).expect("valid range")
}

pub fn never_cancel() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}

pub fn write_file_tool() -> ToolSpec {
    ToolSpec {
        name: "write_file".into(),
        description: "Create or replace one file inside the task workspace.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Workspace-relative file path using forward slashes"},
                "content": {"type": "string", "description": "Complete file content"}
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }),
    }
}

pub fn tool_request() -> ModelRequest {
    ModelRequest {
        system: "You are the HOI4 Factory S01 runtime probe. When asked to create a file, call the write_file tool exactly once. Keep output minimal.".into(),
        messages: vec![Message::user_text("Use the write_file tool to create notes/probe.txt whose content is one line: S01 runtime probe ok")],
        tools: vec![write_file_tool()],
        max_output_tokens: 300,
    }
}

pub fn anthropic_profile(base_url: &str) -> ModelProfile {
    let mut p = ModelProfile::anthropic_haiku();
    p.base_url = base_url.to_owned();
    p
}

pub fn compat_profile(base_url: &str) -> ModelProfile {
    ModelProfile::openai_compatible(base_url, "synthetic-model")
}

pub fn client(profile: ModelProfile) -> ProviderClient {
    ProviderClient::new(
        profile,
        ApiKey::new("test-key-not-a-real-key".into()),
        &loopback_policy(),
    )
    .expect("client")
}

pub fn synthetic(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/synthetic")
            .join(name),
    )
    .expect("synthetic fixture")
}
