//! HOI4 tool adapters (S03-W5): configured tool profiles over MCP stdio.
//!
//! Only tools named by a [`profile::ToolProfile`] are ever exposed. Discovery
//! ([`inventory`]) confirms or withholds them, [`session::ToolSession`] runs calls with a second
//! schema check, read-only enforcement, timeouts, bounded output and bounded retries, and
//! [`mcp`] is the thin JSON-RPC client underneath. [`agent_tools`] builds the launch spec for
//! HOI4 Agent Tools with an environment allowlist.

pub mod agent_tools;
pub mod inventory;
pub mod mcp;
pub mod profile;
pub mod session;

pub use agent_tools::{AgentToolsInstall, agent_tools_spec, prepare_state_dir};
pub use inventory::{Health, Inventory, InventoryTool, ToolState, discover};
pub use mcp::{
    DiscoveredTool, Handshake, McpClient, McpError, McpServerSpec, ShutdownReport, ToolOutput,
};
pub use profile::{
    EffectClass, ProfileMode, ProfileTool, RetryPolicy, ServerIdentity, ToolProfile,
    agent_tools_read_only,
};
pub use session::{
    AttemptRecord, CallRecord, CatalogRefresh, ExposedTool, ToolCallResult, ToolError, ToolSession,
};
