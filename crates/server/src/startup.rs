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
use factory_intake::{
    FileTypePolicy, GithubConfig, GithubImporter, HttpGithubApi, IntakeError, IntakeService,
    IntakeStore, SecretString, UploadIntake,
};
use factory_setup_adapter::{SetupAdapter, SetupAdapterDeps};
use factory_workspace::{StorageRoot, WorkspaceError};
use tokio::net::TcpListener;

use crate::config::{GithubAppSettings, ServerConfig, cors_origins};
use crate::game_reference::GameReferenceHandle;
use crate::github::{AppInstallations, ConfigSecrets, GithubRuntime, HttpAppInstallations};
use crate::migrations;
use crate::model::AnthropicModel;
use crate::state::{IntakeState, RuntimeSettings, ServerState, SetupState};
use crate::web::StaticSite;

/// Directory under the data root where received uploads are spooled.
const INCOMING_DIR: &str = "incoming";

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
    #[error("intake setup failed: {0}")]
    Intake(#[from] IntakeError),
    #[error("the upload spool directory is unusable: {0}")]
    Spool(std::io::Error),
}

/// Replaceable collaborators, for tests.
#[derive(Default)]
pub struct Overrides {
    pub accounts_deps: Option<AccountsDeps>,
    pub clock: Option<Arc<dyn Clock>>,
    /// Replaces the HTTP lookup of the App's installations.
    pub app_installations: Option<Arc<dyn AppInstallations>>,
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

    let intake = build_intake(&config, &db, &storage, overrides.app_installations)?;
    let setup = build_setup(&config, &db, &storage);

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
        intake,
        setup,
        migrations: Arc::new(report),
        settings: Arc::new(RuntimeSettings {
            development: config.is_development(),
            max_body_bytes: config.max_body_bytes,
            cors_origins: cors_origins(&config.accounts.allowed_origins),
        }),
        web,
    })
}

/// The Setup adapter. Analysis needs the model key; without it that one operation reports itself
/// unavailable. No plan builder is injected until the extraction lands upstream, so planning
/// reports `capability_unavailable`.
fn build_setup(config: &ServerConfig, db: &Database, storage: &StorageRoot) -> SetupState {
    let mut deps = SetupAdapterDeps::new(db.clone(), storage.clone());
    match &config.anthropic {
        Some(settings) => {
            tracing::info!(model = %settings.model, "setup analysis model configured");
            deps = deps.with_model(Arc::new(AnthropicModel::new(settings)));
        }
        None => {
            tracing::info!("setup analysis is unavailable: FACTORY_ANTHROPIC_API_KEY is not set")
        }
    }
    SetupState {
        adapter: SetupAdapter::new(deps),
        sessions: Arc::default(),
    }
}

/// Intake: upload limits, the spool directory and the GitHub importer. A GitHub App that is not
/// configured, or whose key cannot be used, disables the GitHub routes and nothing else.
fn build_intake(
    config: &ServerConfig,
    db: &Database,
    storage: &StorageRoot,
    app_installations: Option<Arc<dyn AppInstallations>>,
) -> Result<IntakeState, StartupError> {
    let limits = config.intake.limits.clone();
    let policy = FileTypePolicy::default();

    // Nothing uploaded survives a restart in the spool: whatever is there belongs to a request
    // that no longer exists.
    let incoming_dir = storage.data_root().join(INCOMING_DIR);
    if incoming_dir.exists() {
        std::fs::remove_dir_all(&incoming_dir).map_err(StartupError::Spool)?;
    }
    std::fs::create_dir_all(&incoming_dir).map_err(StartupError::Spool)?;

    let (app_id, private_key) = load_github_credentials(config.github_app.as_ref());
    let endpoints = config
        .github_app
        .as_ref()
        .map(|g| g.endpoints.clone())
        .unwrap_or_else(factory_intake::GithubEndpoints::github_com);
    let secrets = Arc::new(ConfigSecrets::new(app_id, private_key.clone()));
    let api = Arc::new(HttpGithubApi::new(&endpoints)?);
    let importer = Arc::new(GithubImporter::new(
        api,
        secrets,
        storage.clone(),
        GithubConfig::new(endpoints.clone()),
        limits.clone(),
        policy.clone(),
    ));
    let uploads = Arc::new(UploadIntake::new(storage.clone(), limits.clone(), policy));
    let service = IntakeService::new(IntakeStore::new(db.pool().clone()), uploads, importer);

    let github = match (&config.github_app, private_key) {
        (Some(settings), Some(key)) => {
            let installations: Arc<dyn AppInstallations> = match app_installations {
                Some(custom) => custom,
                None => Arc::new(HttpAppInstallations::new(&endpoints)?),
            };
            let runtime = GithubRuntime::new(settings, key, installations);
            tracing::info!(app_id = %settings.app_id, "GitHub import is available");
            Some(runtime)
        }
        _ => None,
    };
    Ok(IntakeState {
        service,
        github,
        limits,
        upload_max_bytes: config.intake.upload_max_bytes,
        incoming_dir,
    })
}

/// The App id and the PEM, when the App is configured and the key file is usable. Logs what is
/// wrong without ever printing the key.
fn load_github_credentials(
    settings: Option<&GithubAppSettings>,
) -> (Option<String>, Option<SecretString>) {
    let Some(settings) = settings else {
        tracing::info!("GitHub import is unavailable: the GitHub App is not configured");
        return (None, None);
    };
    let pem = match std::fs::read_to_string(&settings.private_key_file) {
        Ok(text) if text.trim().is_empty() => {
            tracing::warn!("GitHub import is unavailable: the App private key file is empty");
            return (None, None);
        }
        Ok(text) => SecretString::new(text),
        Err(err) => {
            tracing::error!(
                kind = ?err.kind(),
                "GitHub import is unavailable: the App private key file cannot be read"
            );
            return (None, None);
        }
    };
    if factory_intake::app_jwt(&settings.app_id, &pem, 0).is_err() {
        tracing::error!("GitHub import is unavailable: the App private key is not a valid RSA PEM");
        return (None, None);
    }
    (Some(settings.app_id.clone()), Some(pem))
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
