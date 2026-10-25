//! A live tool session: one MCP client, its inventory and its profile.
//!
//! The session is the last local line of defense before bytes reach the server. It refuses
//! tools that are not Available, validates the final arguments against the closed host schema
//! (the broker validates first; this is the second check), refuses anything that is not a read
//! in read-only mode without sending it, bounds the output, and retries only `PureRead` calls.
//! A `DerivedJob` or any call whose outcome is unknown is never repeated automatically.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use crate::inventory::{Inventory, ToolState, discover_live};
use crate::mcp::{McpClient, McpError, McpServerSpec, ToolOutput};
use crate::profile::{EffectClass, ProfileMode, ToolProfile};

/// A tool as the model sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExposedTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub effect_class: EffectClass,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCallResult {
    pub output: ToolOutput,
    pub attempts: u32,
    pub elapsed_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown tool {0}")]
    UnknownTool(String),
    #[error("tool {name} is not available: {reason}")]
    NotAvailable { name: String, reason: String },
    #[error("arguments do not match the host schema: {}", messages.join("; "))]
    InvalidArguments { messages: Vec<String> },
    #[error("tool {name} is not a read and the profile is read-only")]
    ReadOnlyRefused { name: String },
    #[error("{error} (after {attempts} attempt(s))")]
    Mcp { error: McpError, attempts: u32 },
}

impl ToolError {
    /// Whether the effect of the call on the server is unknown (it may have run).
    pub fn outcome_unknown(&self) -> bool {
        matches!(
            self,
            ToolError::Mcp {
                error: McpError::Timeout { .. }
                    | McpError::ProcessExited { .. }
                    | McpError::Transport { .. }
                    | McpError::Protocol { .. },
                ..
            }
        )
    }
}

/// One attempt of one call, for the host's receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttemptRecord {
    /// `ok`, `tool_error` (the tool answered with `isError`) or an [`McpError::class`].
    pub outcome: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CallRecord {
    pub exposed_name: String,
    pub mcp_name: String,
    pub effect_class: EffectClass,
    pub attempts: Vec<AttemptRecord>,
    pub truncated: bool,
}

/// What `refresh_catalog` changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CatalogRefresh {
    /// Exposed names withheld because their schema hash changed.
    pub withheld: Vec<String>,
    /// Exposed names whose tool is no longer listed.
    pub missing: Vec<String>,
}

pub struct ToolSession {
    spec: McpServerSpec,
    profile: ToolProfile,
    client: Option<McpClient>,
    inventory: Inventory,
    validators: HashMap<String, jsonschema::Validator>,
    records: Vec<CallRecord>,
}

impl std::fmt::Debug for ToolSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSession")
            .field("profile", &self.profile.id)
            .field("healthy", &self.inventory.is_healthy())
            .finish_non_exhaustive()
    }
}

fn retryable(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Timeout { .. } | McpError::ProcessExited { .. } | McpError::Transport { .. }
    )
}

impl ToolSession {
    /// Starts the server and discovers its tools. Never fails: an unhealthy server gives an
    /// inventory that says so, and every call then reports `NotAvailable`.
    pub async fn start(spec: McpServerSpec, profile: ToolProfile) -> ToolSession {
        let (inventory, client) = discover_live(&spec, &profile).await;
        // A profile with an uncompilable host schema is a programming error; the tool is
        // simply left without a validator and refused.
        let validators = profile
            .tools
            .iter()
            .filter_map(|t| {
                jsonschema::validator_for(&t.host_schema)
                    .ok()
                    .map(|v| (t.exposed_name.clone(), v))
            })
            .collect();
        ToolSession {
            spec,
            profile,
            client,
            inventory,
            validators,
            records: Vec::new(),
        }
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn profile(&self) -> &ToolProfile {
        &self.profile
    }

    /// The tools the model may call: configured and Available only.
    pub fn exposed_tools(&self) -> Vec<ExposedTool> {
        self.profile
            .tools
            .iter()
            .filter(|t| {
                self.inventory
                    .tool(&t.exposed_name)
                    .is_some_and(|i| i.state == ToolState::Available)
            })
            .map(|t| ExposedTool {
                name: t.exposed_name.clone(),
                description: t.description.clone(),
                input_schema: t.host_schema.clone(),
                effect_class: t.effect_class,
            })
            .collect()
    }

    /// Per-call records since the last `take_records`.
    pub fn records(&self) -> &[CallRecord] {
        &self.records
    }

    pub fn take_records(&mut self) -> Vec<CallRecord> {
        std::mem::take(&mut self.records)
    }

    pub fn client_pid(&self) -> Option<u32> {
        self.client.as_ref().and_then(McpClient::pid)
    }

    fn withhold(&mut self, exposed: &str, reason: &str) {
        if let Some(t) = self
            .inventory
            .tools
            .iter_mut()
            .find(|t| t.exposed_name == exposed)
        {
            t.state = ToolState::Withheld {
                reason: reason.into(),
            };
        }
    }

    /// Lists the catalog again and withholds every tool whose schema hash changed since
    /// discovery (EXT-05). A withheld tool stays withheld until [`ToolSession::rediscover`].
    pub async fn refresh_catalog(&mut self) -> Result<CatalogRefresh, McpError> {
        let client = self.client.as_ref().ok_or(McpError::Transport {
            message: "no live server".into(),
        })?;
        let tools = client.list_tools().await?;
        Ok(self.apply_catalog(&tools))
    }

    fn apply_catalog(&mut self, tools: &[crate::mcp::DiscoveredTool]) -> CatalogRefresh {
        let mut refresh = CatalogRefresh::default();
        let names: Vec<(String, String)> = self
            .inventory
            .tools
            .iter()
            .filter(|t| t.state == ToolState::Available)
            .map(|t| (t.mcp_name.clone(), t.exposed_name.clone()))
            .collect();
        for (mcp, exposed) in names {
            let known = self
                .inventory
                .tool(&exposed)
                .and_then(|t| t.schema_sha256.clone());
            match tools.iter().find(|t| t.name == mcp) {
                None => {
                    self.withhold(&exposed, "missing_after_refresh");
                    refresh.missing.push(exposed);
                }
                Some(found) if Some(&found.schema_sha256) != known.as_ref() => {
                    self.withhold(&exposed, "schema_changed");
                    refresh.withheld.push(exposed);
                }
                Some(_) => {}
            }
        }
        refresh
    }

    /// Stops the server and runs discovery from scratch, resetting the schema baseline.
    pub async fn rediscover(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.shutdown().await;
        }
        let (inventory, client) = discover_live(&self.spec, &self.profile).await;
        self.inventory = inventory;
        self.client = client;
    }

    /// Replaces a dead or hung process with a fresh one, keeping the inventory baseline: a tool
    /// whose schema or the server version changed across the restart is withheld.
    async fn restart(&mut self) -> Result<(), McpError> {
        if let Some(client) = self.client.take() {
            let _ = client.shutdown().await;
        }
        let (client, handshake) = McpClient::start(&self.spec).await?;
        let tools = match client.list_tools().await {
            Ok(tools) => tools,
            Err(error) => {
                let _ = client.shutdown().await;
                return Err(error);
            }
        };
        if Some(&handshake) != self.inventory.server.as_ref()
            && (handshake.server_name != self.profile.server.name
                || handshake.server_version != self.profile.server.version_req)
        {
            let names: Vec<String> = self
                .inventory
                .tools
                .iter()
                .map(|t| t.exposed_name.clone())
                .collect();
            for name in names {
                self.withhold(&name, "version_mismatch");
            }
            self.inventory.version_matches = false;
        }
        self.apply_catalog(&tools);
        self.client = Some(client);
        Ok(())
    }

    fn check_args(&self, exposed: &str, args: &Value) -> Result<(), ToolError> {
        let validator = self
            .validators
            .get(exposed)
            .ok_or_else(|| ToolError::NotAvailable {
                name: exposed.into(),
                reason: "host schema does not compile".into(),
            })?;
        let messages: Vec<String> = validator
            .iter_errors(args)
            .take(8)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        if messages.is_empty() {
            Ok(())
        } else {
            Err(ToolError::InvalidArguments { messages })
        }
    }

    /// Calls one exposed tool with already-final arguments.
    pub async fn call(
        &mut self,
        exposed_name: &str,
        final_arguments: &Value,
        deadline: Instant,
    ) -> Result<ToolCallResult, ToolError> {
        let tool = self
            .profile
            .tool_by_exposed(exposed_name)
            .cloned()
            .ok_or_else(|| ToolError::UnknownTool(exposed_name.to_owned()))?;
        match self.inventory.tool(exposed_name).map(|t| t.state.clone()) {
            Some(ToolState::Available) => {}
            Some(ToolState::Missing) => {
                return Err(ToolError::NotAvailable {
                    name: exposed_name.into(),
                    reason: "missing".into(),
                });
            }
            Some(ToolState::Withheld { reason }) => {
                return Err(ToolError::NotAvailable {
                    name: exposed_name.into(),
                    reason,
                });
            }
            None => return Err(ToolError::UnknownTool(exposed_name.to_owned())),
        }
        match (self.profile.mode, tool.effect_class) {
            (ProfileMode::ReadOnly, EffectClass::PureRead | EffectClass::DerivedJob) => {}
            (ProfileMode::ReadOnly, EffectClass::Write) => {
                return Err(ToolError::ReadOnlyRefused {
                    name: exposed_name.into(),
                });
            }
        }
        self.check_args(exposed_name, final_arguments)?;
        let mut arguments = final_arguments.clone();
        if tool.inject_workspace
            && let Some(map) = arguments.as_object_mut()
        {
            map.insert("workspaceId".into(), json!("current"));
        }

        let max_attempts = match tool.effect_class {
            EffectClass::PureRead => tool.retry.max_attempts.max(1),
            _ => 1,
        };
        let started = Instant::now();
        let mut record = CallRecord {
            exposed_name: tool.exposed_name.clone(),
            mcp_name: tool.mcp_name.clone(),
            effect_class: tool.effect_class,
            attempts: Vec::new(),
            truncated: false,
        };
        let mut attempts = 0u32;
        let outcome = loop {
            attempts += 1;
            let attempt_started = Instant::now();
            let result = self
                .attempt(&tool.mcp_name, &arguments, tool.timeout, deadline)
                .await;
            let elapsed_ms = millis(attempt_started.elapsed());
            record.attempts.push(AttemptRecord {
                outcome: match &result {
                    Ok(o) if o.is_error => "tool_error".into(),
                    Ok(_) => "ok".into(),
                    Err(e) => e.class().into(),
                },
                elapsed_ms,
            });
            match result {
                Err(error) if retryable(&error) && attempts < max_attempts => {
                    if Instant::now() >= deadline {
                        break Err(error);
                    }
                    if let Err(restart_error) = self.restart().await {
                        break Err(restart_error);
                    }
                    // The restart may have withheld this very tool.
                    if self.inventory.tool(exposed_name).map(|t| &t.state)
                        != Some(&ToolState::Available)
                    {
                        break Err(error);
                    }
                }
                other => break other,
            }
        };
        let result = match outcome {
            Ok(mut output) => {
                record.truncated = bound_output(&mut output, tool.max_output_bytes);
                Ok(ToolCallResult {
                    output,
                    attempts,
                    elapsed_ms: millis(started.elapsed()),
                })
            }
            Err(error) => Err(ToolError::Mcp { error, attempts }),
        };
        self.records.push(record);
        result
    }

    async fn attempt(
        &mut self,
        mcp_name: &str,
        arguments: &Value,
        tool_timeout: Duration,
        deadline: Instant,
    ) -> Result<ToolOutput, McpError> {
        let dead = self.client.as_ref().is_none_or(|c| !c.is_alive());
        if dead {
            self.restart().await?;
            // The restart may have withheld the tool; the caller re-checks before retrying, but
            // the first attempt must not reach a server whose schema changed.
            let still_ok = self
                .inventory
                .tools
                .iter()
                .any(|t| t.mcp_name == mcp_name && t.state == ToolState::Available);
            if !still_ok {
                return Err(McpError::Handshake {
                    message: "tool withheld after restart".into(),
                });
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(McpError::Timeout { sent: false });
        }
        let Some(client) = self.client.as_ref() else {
            return Err(McpError::Transport {
                message: "no live server".into(),
            });
        };
        client
            .call_tool(
                mcp_name,
                arguments.clone(),
                Some(tool_timeout.min(remaining)),
            )
            .await
    }

    /// Stops the server process (and its group).
    pub async fn shutdown(mut self) -> Option<crate::mcp::ShutdownReport> {
        match self.client.take() {
            Some(client) => Some(client.shutdown().await),
            None => None,
        }
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Cuts `output` down to `max_bytes` of serialized content. Returns whether anything was cut.
fn bound_output(output: &mut ToolOutput, max_bytes: usize) -> bool {
    let size = |v: &Value| serde_json::to_vec(v).map(|b| b.len()).unwrap_or(0);
    let total: usize = output.content.iter().map(size).sum::<usize>()
        + output.structured.as_ref().map(size).unwrap_or(0);
    if total <= max_bytes {
        return false;
    }
    let mut budget = max_bytes;
    // Structured content is kept whole or dropped: a cut JSON document would mislead.
    if let Some(structured) = output.structured.take() {
        let n = size(&structured);
        if n <= budget / 2 {
            budget -= n;
            output.structured = Some(structured);
        }
    }
    let mut kept = Vec::new();
    for item in std::mem::take(&mut output.content) {
        let n = size(&item);
        if n <= budget {
            budget -= n;
            kept.push(item);
            continue;
        }
        if let Some(text) = item.get("text").and_then(Value::as_str) {
            // Leave room for JSON overhead of the wrapper object and escapes.
            let mut take = budget.saturating_sub(64).min(text.len());
            while take > 0 && !text.is_char_boundary(take) {
                take -= 1;
            }
            if take > 0 {
                let mut cut = item.clone();
                cut["text"] = json!(&text[..take]);
                kept.push(cut);
            }
        }
        break;
    }
    output.content = kept;
    output.truncated = true;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_cuts_text_and_flags() {
        let mut out = ToolOutput {
            content: vec![json!({"type":"text","text":"é".repeat(5000)})],
            structured: Some(json!({"big":"x".repeat(9000)})),
            is_error: false,
            bytes: 19000,
            truncated: false,
        };
        assert!(bound_output(&mut out, 2000));
        assert!(out.truncated);
        assert!(out.structured.is_none());
        let n: usize = out.content.iter().map(|c| c.to_string().len()).sum();
        assert!(n <= 2000, "{n}");
    }

    #[test]
    fn small_output_is_untouched() {
        let mut out = ToolOutput {
            content: vec![json!({"type":"text","text":"hi"})],
            structured: None,
            is_error: false,
            bytes: 10,
            truncated: false,
        };
        assert!(!bound_output(&mut out, 2000));
        assert!(!out.truncated);
    }
}
