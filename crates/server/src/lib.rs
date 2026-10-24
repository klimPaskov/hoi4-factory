//! HOI4 Factory server: the HTTP host that wires accounts, workspaces and the game reference.
//!
//! * [`startup::initialize`] builds a [`ServerState`] (database, migrations, storage, game
//!   reference, accounts); [`routes::build_app`] turns it into the hardened [`axum::Router`].
//! * [`migrations`] is the single list of every crate's migrations.
//! * [`routes::api_routes`] is where the intake crate and the Setup adapter mount their routes.
//! * [`hardening`], [`logging`] and [`redact`] hold the cross-cutting response headers, CORS,
//!   request ids and redacted JSON logs.

pub mod config;
pub mod game_reference;
pub mod hardening;
pub mod healthcheck;
pub mod logging;
pub mod migrations;
pub mod redact;
pub mod routes;
pub mod startup;
pub mod state;
pub mod web;

pub use config::{ConfigError, ServerConfig};
pub use routes::build_app;
pub use startup::{Overrides, StartupError, initialize, initialize_with, serve, shutdown_signal};
pub use state::ServerState;
