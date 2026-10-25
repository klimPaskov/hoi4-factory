//! Discovery and inventory.
//!
//! Health (the process starts and answers `initialize` and `tools/list` in time) is reported
//! separately from feature presence (which configured tools exist and are trusted). Discovery
//! never fails hard: a missing or broken server yields an `Unavailable` inventory so the host can
//! report the profile as unavailable without breaking anything else.
//!
//! A server whose version differs from the profile pin withholds every tool. A discovered tool
//! outside the profile is counted, never exposed.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::mcp::{DiscoveredTool, Handshake, McpClient, McpError, McpServerSpec};
use crate::profile::ToolProfile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Health {
    Healthy { startup_ms: u64 },
    Unavailable { reason: String, stderr_tail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ToolState {
    Available,
    /// The server does not list the tool.
    Missing,
    /// The tool exists but must not be used, for example `version_mismatch` or `schema_changed`.
    Withheld {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InventoryTool {
    pub mcp_name: String,
    pub exposed_name: String,
    pub present: bool,
    pub schema_sha256: Option<String>,
    pub state: ToolState,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inventory {
    pub profile_id: String,
    pub health: Health,
    pub server: Option<Handshake>,
    pub version_matches: bool,
    pub tools: Vec<InventoryTool>,
    /// Discovered tools that are not in the profile. Counted only.
    pub ignored_tool_count: usize,
    pub discovered_at: DateTime<Utc>,
}

impl Inventory {
    pub fn is_healthy(&self) -> bool {
        matches!(self.health, Health::Healthy { .. })
    }

    pub fn tool(&self, exposed_name: &str) -> Option<&InventoryTool> {
        self.tools.iter().find(|t| t.exposed_name == exposed_name)
    }

    pub fn available_count(&self) -> usize {
        self.tools
            .iter()
            .filter(|t| t.state == ToolState::Available)
            .count()
    }
}

pub(crate) fn unavailable(profile: &ToolProfile, error: &McpError) -> Inventory {
    let stderr_tail = match error {
        McpError::ProcessExited { stderr_tail, .. } => stderr_tail.clone(),
        _ => String::new(),
    };
    Inventory {
        profile_id: profile.id.clone(),
        health: Health::Unavailable {
            reason: error.to_string(),
            stderr_tail,
        },
        server: None,
        version_matches: false,
        tools: profile
            .tools
            .iter()
            .map(|t| InventoryTool {
                mcp_name: t.mcp_name.clone(),
                exposed_name: t.exposed_name.clone(),
                present: false,
                schema_sha256: None,
                state: ToolState::Withheld {
                    reason: "server_unavailable".into(),
                },
            })
            .collect(),
        ignored_tool_count: 0,
        discovered_at: Utc::now(),
    }
}

pub(crate) fn build_inventory(
    profile: &ToolProfile,
    handshake: Handshake,
    discovered: &[DiscoveredTool],
    startup_ms: u64,
) -> Inventory {
    let version_matches = handshake.server_name == profile.server.name
        && handshake.server_version == profile.server.version_req;
    let mut ignored = discovered.len();
    let tools = profile
        .tools
        .iter()
        .map(|pt| {
            let found = discovered.iter().find(|d| d.name == pt.mcp_name);
            if found.is_some() {
                ignored -= 1;
            }
            let state = match found {
                _ if !version_matches => ToolState::Withheld {
                    reason: "version_mismatch".into(),
                },
                None => ToolState::Missing,
                Some(d) => match &pt.expected_schema_sha256 {
                    Some(expected) if *expected != d.schema_sha256 => ToolState::Withheld {
                        reason: "schema_hash_differs_from_profile".into(),
                    },
                    _ => ToolState::Available,
                },
            };
            InventoryTool {
                mcp_name: pt.mcp_name.clone(),
                exposed_name: pt.exposed_name.clone(),
                present: found.is_some(),
                schema_sha256: found.map(|d| d.schema_sha256.clone()),
                state,
            }
        })
        .collect();
    Inventory {
        profile_id: profile.id.clone(),
        health: Health::Healthy { startup_ms },
        server: Some(handshake),
        version_matches,
        tools,
        ignored_tool_count: ignored,
        discovered_at: Utc::now(),
    }
}

/// Starts the server, lists its tools and returns the inventory together with the live client
/// when discovery succeeded.
pub(crate) async fn discover_live(
    spec: &McpServerSpec,
    profile: &ToolProfile,
) -> (Inventory, Option<McpClient>) {
    let started = Instant::now();
    let (client, handshake) = match McpClient::start(spec).await {
        Ok(pair) => pair,
        Err(error) => return (unavailable(profile, &error), None),
    };
    let tools = match client.list_tools().await {
        Ok(tools) => tools,
        Err(error) => {
            let mut inventory = unavailable(profile, &error);
            if let Health::Unavailable { stderr_tail, .. } = &mut inventory.health
                && stderr_tail.is_empty()
            {
                *stderr_tail = client.stderr_tail();
            }
            let _ = client.shutdown().await;
            return (inventory, None);
        }
    };
    let startup_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    (
        build_inventory(profile, handshake, &tools, startup_ms),
        Some(client),
    )
}

/// One-shot discovery: starts the server, builds the inventory and stops the server again.
pub async fn discover(spec: &McpServerSpec, profile: &ToolProfile) -> Inventory {
    let (inventory, client) = discover_live(spec, profile).await;
    if let Some(client) = client {
        let _ = client.shutdown().await;
    }
    inventory
}
