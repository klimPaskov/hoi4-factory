//! HOI4 Factory server: the HTTP host that wires accounts, workspaces, intake and the game
//! reference.
//!
//! * [`startup::initialize`] builds a [`ServerState`] (database, migrations, storage, game
//!   reference, accounts, intake); [`routes::build_app`] turns it into the hardened
//!   [`axum::Router`].
//! * [`migrations`] is the single list of every crate's migrations.
//! * [`routes::api_routes`] mounts the accounts routes, [`intake_api`] (uploads, GitHub import,
//!   the workspace descriptor) and is where the Setup adapter mounts its routes.
//! * [`github`] holds the GitHub App credentials and the lookup of a user's installation.
//! * [`agent`] is the native agent host (S03) and [`agent_api`] its task, key and event routes.
//! * [`hardening`], [`logging`] and [`redact`] hold the cross-cutting response headers, CORS,
//!   request ids and redacted JSON logs.

pub mod access;
pub mod agent;
pub mod agent_api;
pub mod api_error;
pub mod config;
pub mod game_reference;
pub mod github;
pub mod hardening;
pub mod healthcheck;
pub mod intake_api;
pub mod logging;
pub mod migrations;
pub mod model;
pub mod redact;
pub mod routes;
pub mod setup_api;
pub mod startup;
pub mod state;
pub mod web;
pub mod workspace_api;

pub use config::{ConfigError, ServerConfig};
pub use routes::build_app;
pub use startup::{Overrides, StartupError, initialize, initialize_with, serve, shutdown_signal};
pub use state::ServerState;
