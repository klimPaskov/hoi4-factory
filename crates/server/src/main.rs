//! HOI4 Factory server binary. Configuration comes from the environment; see
//! `factory_server::config`. `factory-server healthcheck` is the container health probe,
//! `factory-server agent-worker` is the agent's supervised model worker (S03), and
//! `factory-server engine-status` reports the bundled and linked hoi4-engine (S04, T-ENG-01).

use std::process::ExitCode;

use factory_server::{ServerConfig, build_app, initialize, logging, serve, shutdown_signal};

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("agent-worker") {
        return agent_worker().await;
    }
    if std::env::args().nth(1).as_deref() == Some("engine-status") {
        return engine_status();
    }
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

/// The supervised model worker: `factory-server agent-worker --generation N`. It speaks the framed
/// protocol on stdin and stdout and holds no database, file or grant authority.
async fn agent_worker() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let generation = args
        .iter()
        .position(|a| a == "--generation")
        .and_then(|i| args.get(i + 1))
        .and_then(|g| g.parse::<u64>().ok());
    let Some(generation) = generation else {
        eprintln!("usage: factory-server agent-worker --generation N");
        return ExitCode::FAILURE;
    };
    match factory_agent::worker::serve(tokio::io::stdin(), tokio::io::stdout(), generation).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("agent worker stopped: {e}");
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

/// `factory-server engine-status`: prints the bundled hoi4-engine state of the installed HOI4
/// Agent Tools and the engine linked into this binary as JSON (T-ENG-01). Exit code 0 when the
/// bundled engine is available, 1 when it is not or Agent Tools are not installed.
fn engine_status() -> ExitCode {
    let bundled = factory_hoi4_tools::AgentToolsInstall::from_env().map(|install| {
        match factory_hoi4_tools::engine::package_root(&install.entry) {
            Some(root) => factory_hoi4_tools::bundled_engine_status(&root),
            None => factory_hoi4_tools::EngineStatus::Unavailable {
                reason: "binary_not_found",
                message: "the Agent Tools entry point has no package folder".into(),
            },
        }
    });
    let available = bundled.as_ref().is_some_and(|s| s.is_available());
    let report = serde_json::json!({
        "agent_tools_installed": bundled.is_some(),
        "agent_tools_version": factory_hoi4_tools::profile::AGENT_TOOLS_VERSION,
        "bundled_engine": bundled,
        "linked_engine": {
            "version": factory_engine_check::ENGINE_VERSION,
            "rev": factory_engine_check::ENGINE_REV,
        },
    });
    println!("{report}");
    if available { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
