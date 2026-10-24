//! Startup sequence: database, migrations, role posture, storage layout, game reference,
//! accounts. Everything here either succeeds, degrades visibly (game reference, storage mode
//! problems reported by `/readyz`) or stops the process with a clear error.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use factory_accounts::{
    AccountsDeps, AccountsState, Clock, Database, PoolSettings, PostureError, SetupError,
    SystemClock,
};
use factory_workspace::{StorageRoot, WorkspaceError};
use tokio::net::TcpListener;

use crate::config::{ServerConfig, cors_origins};
use crate::game_reference::GameReferenceHandle;
use crate::migrations;
use crate::state::{RuntimeSettings, ServerState};
use crate::web::StaticSite;

const CONNECT_ATTEMPTS: u32 = 15;
const CONNECT_PAUSE: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("could not connect to the database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migrations failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    Posture(#[from] PostureError),
    #[error("storage root is unusable: {0}")]
    Storage(#[from] WorkspaceError),
    #[error("{0}")]
    Setup(#[from] SetupError),
}

/// Replaceable collaborators, for tests.
#[derive(Default)]
pub struct Overrides {
    pub accounts_deps: Option<AccountsDeps>,
    pub clock: Option<Arc<dyn Clock>>,
}

pub async fn initialize(config: ServerConfig) -> Result<ServerState, StartupError> {
    initialize_with(config, Overrides::default()).await
}

pub async fn initialize_with(
    config: ServerConfig,
    overrides: Overrides,
) -> Result<ServerState, StartupError> {
    let clock: Arc<dyn Clock> = overrides.clock.unwrap_or_else(|| Arc::new(SystemClock));

    let db = connect(&config.database_url.0, &config.pool, &clock).await?;

    // The migration role owns the tables; the application role must not (row-level security).
    let report = match &config.migration_database_url {
        Some(url) => {
            let settings = PoolSettings {
                max_connections: 1,
                min_connections: 0,
                ..config.pool.clone()
            };
            let owner = connect(&url.0, &settings, &clock).await?;
            let report = migrations::run_all(owner.pool()).await;
            owner.pool().close().await;
            report?
        }
        None => migrations::run_all(db.pool()).await?,
    };

    if config.is_development() {
        match db.rls_posture().await {
            Ok(posture) if posture.superuser || posture.bypass_rls => tracing::warn!(
                role = %posture.role,
                "the database role bypasses row-level security; acceptable in development only"
            ),
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "could not inspect the database role"),
        }
    } else {
        let posture = db.ensure_rls_enforced().await?;
        tracing::info!(role = %posture.role, "row-level security is enforced");
    }

    let storage = StorageRoot::new(&config.data_root);
    storage.ensure_layout()?;
    for issue in storage.verify_layout(None) {
        tracing::error!(
            directory = %issue.directory,
            problem = %issue.problem,
            "storage layout deviates from the required modes"
        );
    }

    let game_reference = GameReferenceHandle::load(config.game_ref_dir.clone()).await;
    game_reference.spawn_retry();

    let deps = match overrides.accounts_deps {
        Some(deps) => deps,
        None => AccountsDeps::production(&config.accounts, &db)?,
    };
    let accounts = AccountsState::new(db.clone(), config.accounts.clone(), deps)?;

    let web = config.web_dist.as_deref().and_then(StaticSite::open);
    match (&config.web_dist, &web) {
        (Some(_), Some(_)) => tracing::info!("serving the browser client"),
        (Some(_), None) => tracing::warn!("FACTORY_WEB_DIST is set but is not a directory"),
        (None, _) => {}
    }

    Ok(ServerState {
        accounts,
        db,
        storage,
        game_reference,
        migrations: Arc::new(report),
        settings: Arc::new(RuntimeSettings {
            development: config.is_development(),
            max_body_bytes: config.max_body_bytes,
            cors_origins: cors_origins(&config.accounts.allowed_origins),
        }),
        web,
    })
}

/// Connects, retrying while the database comes up.
async fn connect(
    url: &str,
    settings: &PoolSettings,
    clock: &Arc<dyn Clock>,
) -> Result<Database, sqlx::Error> {
    let mut attempt = 1;
    loop {
        match Database::connect(url, settings, clock.clone()).await {
            Ok(db) => return Ok(db),
            Err(err) if attempt < CONNECT_ATTEMPTS => {
                tracing::warn!(attempt, error = %err, "database not reachable yet; retrying");
                tokio::time::sleep(CONNECT_PAUSE).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// Serves `app` with the peer address available to handlers (rate limiting).
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Completes on Ctrl-C or, on Unix, SIGTERM (what `docker stop` sends).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}
