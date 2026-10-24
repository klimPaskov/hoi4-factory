//! HOI4 Factory server binary. Configuration comes from the environment; see
//! `factory_server::config`. `factory-server healthcheck` is the container health probe.

use std::process::ExitCode;

use factory_server::{ServerConfig, build_app, initialize, logging, serve, shutdown_signal};

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return if factory_server::healthcheck::run_from_env() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    logging::init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!(error = %message, "the server stopped");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let config = ServerConfig::from_env().map_err(|e| e.to_string())?;
    let listen = config.listen.clone();
    let environment = config.is_development();
    let state = initialize(config).await.map_err(|e| e.to_string())?;
    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .map_err(|e| format!("cannot listen on {listen}: {e}"))?;
    tracing::info!(listen = %listen, development = environment, "listening");
    serve(listener, app, shutdown_signal())
        .await
        .map_err(|e| e.to_string())
}
