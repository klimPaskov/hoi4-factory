//! HOI4 Factory native agent loop (ADR-011).
//!
//! S01 probe scope: thin provider adapters with self-owned SSE parsing and
//! tool-call delta assembly, a recording tee and a fault-injecting replay
//! transport, and the host/worker frame protocol. The crate holds no
//! filesystem, Git, secret-store or grant authority. Every tool call it
//! assembles is only a request that the host broker validates again.

pub mod cost;
pub mod openai;
pub mod sse;
pub mod transport;
pub mod types;

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "protocol")]
pub mod protocol;

pub const RUNTIME_BUILD: &str = concat!("factory-agent/", env!("CARGO_PKG_VERSION"));
