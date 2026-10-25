//! HOI4 Factory native agent loop (ADR-011).
//!
//! Thin provider adapters with self-owned SSE parsing and tool-call delta
//! assembly, a provider-neutral request and loop state, honest capability
//! probing, an outbound destination policy, normalized errors for the contract
//! envelope, a recording tee and a fault-injecting replay transport, and the
//! host/worker frame protocol with a supervised worker process. The crate
//! holds no filesystem, Git, secret-store or grant authority. Every tool call
//! it assembles is only a request that the host broker validates again.
//!
//! The host drives the loop: for each turn it sends a request to the worker,
//! gets a `Turn` back, validates and executes tool calls itself and sends the
//! next request.

// TurnError carries the partial stream state for diagnosis, so it is large by design.
#![allow(clippy::result_large_err)]

pub mod conversation;
pub mod cost;
pub mod destination;
pub mod errors;
pub mod openai;
pub mod profile;
pub mod request;
pub mod sse;
pub mod transport;
pub mod types;

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "anthropic")]
pub mod client;
#[cfg(feature = "anthropic")]
pub mod probe;
#[cfg(feature = "protocol")]
pub mod protocol;
#[cfg(feature = "worker")]
pub mod supervisor;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
#[cfg(feature = "worker")]
pub mod worker;

pub const RUNTIME_BUILD: &str = concat!("factory-agent/", env!("CARGO_PKG_VERSION"));
