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
use factory_setup_adapter::setup::{
    DefaultPlanBuilder, HttpSourceClient, NodeRuntime, SourceFetcher,
};
use factory_setup_adapter::{AnalysisRoute, ModelClient, SetupAdapter, SetupAdapterDeps};
use factory_workspace::{StorageRoot, WorkspaceError};
use tokio::net::TcpListener;

use crate::agent::{AgentRuntime, AgentRuntimeParts, AgentToolsSettings, RunnerSettings, WorkerLauncher};
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
    /// Replaces the model client of Setup analysis. The route (provider, model) still comes from
    /// the configuration, so the analysis record names the model the configuration names.
    pub model_client: Option<Arc<dyn ModelClient>>,
    /// Replaces where the Setup workflow package comes from (the public repository by default).
    pub setup_source: Option<Arc<dyn SourceFetcher>>,
    /// Replaces how the agent's worker process is started (tests run the built server binary).
    pub agent_worker: Option<WorkerLauncher>,
    /// Replaces the agent loop's timings.
    pub agent_settings: Option<RunnerSettings>,
    /// Replaces the Agent Tools install found in the configuration (`Some(None)` disables it).
    pub agent_tools: Option<Option<AgentToolsSettings>>,
    /// Skips the startup recovery pass, so a test can run it itself.
    pub agent_skip_recovery: bool,
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
    let setup = build_setup(
        &config,
        &db,
        &storage,
        overrides.model_client,
        overrides.setup_source,
    )
    .await;

    let agent = build_agent(
        &config,
        &db,
        &storage,
        &clock,
        game_reference.status().is_healthy(),
        AgentOverrides {
            worker: overrides.agent_worker,
            settings: overrides.agent_settings,
            tools: overrides.agent_tools,
            skip_recovery: overrides.agent_skip_recovery,
        },
    )
    .await;

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
        agent,
    })
}

struct AgentOverrides {
    worker: Option<WorkerLauncher>,
    settings: Option<RunnerSettings>,
    tools: Option<Option<AgentToolsSettings>>,
    skip_recovery: bool,
}

/// The agent host: key encryption, the outbound destination policy, operator keys, the worker
/// program and the Agent Tools install. Whatever is missing makes its one feature unavailable
/// and is logged; only a bad destination range in the configuration stops the agent entirely.
async fn build_agent(
    config: &ServerConfig,
    db: &Database,
    storage: &StorageRoot,
    clock: &Arc<dyn Clock>,
    game_available: bool,
    overrides: AgentOverrides,
) -> Option<AgentRuntime> {
    let settings = &config.agent;
    let secrets: Option<Arc<dyn factory_accounts::SecretStore>> = match &settings.secret_key_file {
        None => {
            tracing::info!("saving model keys is unavailable: FACTORY_SECRET_KEY_FILE is not set");
            None
        }
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => match factory_accounts::AesGcmSecretStore::from_base64(&text) {
                Ok(store) => Some(Arc::new(store)),
                Err(_) => {
                    tracing::error!("saving model keys is unavailable: the key file is not 32 bytes of base64");
                    None
                }
            },
            Err(err) => {
                tracing::error!(kind = ?err.kind(), "saving model keys is unavailable: the key file cannot be read");
                None
            }
        },
    };
    let ranges: Vec<&str> = settings.outbound_allow_cidrs.iter().map(String::as_str).collect();
    let policy = match factory_agent::destination::DestinationPolicy::allowing(&ranges) {
        Ok(p) => p,
        Err(err) => {
            tracing::error!(error = %err, "the agent is unavailable: FACTORY_OUTBOUND_ALLOW_CIDRS is invalid");
            return None;
        }
    };
    if !ranges.is_empty() {
        tracing::warn!(ranges = ?ranges, "custom model endpoints may use these private ranges");
    }
    let tools = match overrides.tools {
        Some(t) => t,
        None => match (&settings.agent_tools_node, &settings.agent_tools_entry) {
            (Some(node), Some(entry)) if node.exists() && entry.exists() => Some(AgentToolsSettings {
                install: factory_hoi4_tools::AgentToolsInstall { node: node.clone(), entry: entry.clone() },
                game_root: game_available.then(|| config.game_ref_dir.clone()),
            }),
            _ => {
                tracing::info!("HOI4 Agent Tools are unavailable: FACTORY_AGENT_TOOLS_NODE or _ENTRY is not installed");
                None
            }
        },
    };
    let operator_keys = crate::agent::profiles::OperatorKeys {
        anthropic: config.anthropic.as_ref().map(|a| a.api_key.clone()),
        openai: settings.openai_api_key.clone(),
        anthropic_base: config.anthropic.as_ref().map(|a| a.base_url.clone()),
        openai_base: settings.openai_base_url.clone(),
    };
    let runtime = AgentRuntime::start(AgentRuntimeParts {
        db: db.clone(),
        storage: storage.clone(),
        clock: clock.clone(),
        secrets,
        policy,
        allowance_micros: i64::try_from(settings.allowance_micros).unwrap_or(i64::MAX),
        operator_keys,
        agent_tools: tools,
        worker: overrides.worker.or_else(WorkerLauncher::current_exe),
        settings: overrides.settings.unwrap_or_default(),
        max_running: settings.max_running,
    })
    .await;
    match runtime {
        Ok(rt) => {
            tracing::info!(incarnation = %rt.incarnation(), "agent host started");
            if !overrides.skip_recovery {
                crate::agent::recovery::spawn(rt.clone());
            }
            Some(rt)
        }
        Err(err) => {
            tracing::error!(error = %err, "the agent host could not start");
            None
        }
    }
}

/// The Setup adapter: analysis through the server's model, planning through Setup's
/// `DefaultPlanBuilder`.
///
/// * Analysis needs the model key; without it that one operation reports itself unavailable.
/// * Planning needs a cache directory for the verified workflow package, outside every
///   workspace. The package is fetched by the server process only, from the public Setup source
///   repository, and every file is checked against the manifest's size and SHA-256 before it is
///   planned. When the cache cannot be set up, planning reports `capability_unavailable` with
///   the reason in the log instead of failing the server.
/// * A Node runtime is registered when `FACTORY_NODE_EXECUTABLE` names one Setup accepts.
async fn build_setup(
    config: &ServerConfig,
    db: &Database,
    storage: &StorageRoot,
    model_client: Option<Arc<dyn ModelClient>>,
    source_override: Option<Arc<dyn SourceFetcher>>,
) -> SetupState {
    let mut deps = SetupAdapterDeps::new(db.clone(), storage.clone());
    match (&config.anthropic, model_client) {
        (Some(settings), client) => {
            tracing::info!(model = %settings.model, "setup analysis model configured");
            deps = deps
                .with_model(client.unwrap_or_else(|| Arc::new(AnthropicModel::new(settings))))
                .with_analysis_route(AnalysisRoute::anthropic(settings.model.clone()));
        }
        (None, _) => {
            tracing::info!("setup analysis is unavailable: FACTORY_ANTHROPIC_API_KEY is not set")
        }
    }

    let source: Option<Arc<dyn SourceFetcher>> = match source_override {
        Some(source) => Some(source),
        None => {
            let cache = config.setup.cache_dir.clone();
            // Setup's source client is built on a blocking HTTP client, which must not be created
            // (or dropped) on the async runtime.
            let built = tokio::task::spawn_blocking(move || {
                prepare_setup_cache(&cache).and_then(|dir| {
                    HttpSourceClient::for_cache_root(dir).map_err(|e| e.to_string())
                })
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            match built {
                Ok(client) => Some(Arc::new(DetachedSource::new(client))),
                Err(why) => {
                    tracing::error!(
                        reason = %why,
                        "setup planning is unavailable: the package cache cannot be used"
                    );
                    None
                }
            }
        }
    };
    if let Some(source) = source {
        deps = deps
            .with_source(source.clone())
            .with_plan_builder(Arc::new(DefaultPlanBuilder::with_source(source)));
        tracing::info!("setup planning is available");
    }

    if let Some(executable) = &config.setup.node_executable {
        match NodeRuntime::new(executable, &config.setup.node_prefix) {
            Ok(node) => deps = deps.with_node_runtime(node),
            Err(err) => tracing::warn!(
                error = %err,
                "the configured Node runtime was not accepted; plans that register an MCP server will be refused"
            ),
        }
    }
    SetupState {
        adapter: SetupAdapter::new(deps),
    }
}

/// Drops its value on a plain thread. Setup's source client owns a blocking HTTP client whose
/// internal runtime panics when it is dropped inside an async context, such as the server's own
/// shutdown or a test's end.
pub struct DetachedDrop<T: Send + 'static>(Option<T>);

impl<T: Send + 'static> DetachedDrop<T> {
    pub fn new(value: T) -> Self {
        Self(Some(value))
    }
}

impl<T: Send + 'static> std::ops::Deref for DetachedDrop<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref().expect("value is present until drop")
    }
}

impl<T: Send + 'static> Drop for DetachedDrop<T> {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            std::thread::spawn(move || drop(value));
        }
    }
}

/// Setup's HTTP source client, safe to hold and drop inside the server.
pub struct DetachedSource(DetachedDrop<HttpSourceClient>);

impl DetachedSource {
    pub fn new(client: HttpSourceClient) -> Self {
        Self(DetachedDrop::new(client))
    }
}

impl SourceFetcher for DetachedSource {
    fn resolve_commit(
        &self,
        request: &factory_setup_adapter::setup::SourceRequest,
    ) -> Result<(String, Option<String>, Option<String>), factory_setup_adapter::setup::AppError>
    {
        SourceFetcher::resolve_commit(&*self.0, request)
    }

    fn fetch_manifest(
        &self,
        revision: &str,
    ) -> Result<Vec<u8>, factory_setup_adapter::setup::AppError> {
        SourceFetcher::fetch_manifest(&*self.0, revision)
    }

    fn fetch_verified_file(
        &self,
        revision: &str,
        path: &str,
        expected_sha256: &str,
        expected_size: Option<u64>,
    ) -> Result<Vec<u8>, factory_setup_adapter::setup::AppError> {
        SourceFetcher::fetch_verified_file(&*self.0, revision, path, expected_sha256, expected_size)
    }

    fn prefetch_verified_files(&self, revision: &str, files: &[(String, String, Option<u64>)]) {
        SourceFetcher::prefetch_verified_files(&*self.0, revision, files)
    }

    fn shares_process_cache(&self) -> bool {
        SourceFetcher::shares_process_cache(&*self.0)
    }
}

/// Creates the package cache directory (0755 on Unix) and returns its absolute path. Setup
/// itself checks that it is outside every workspace and free of links.
fn prepare_setup_cache(dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create the cache directory: {e}"))?;
    std::fs::canonicalize(dir).map_err(|e| format!("cannot resolve the cache directory: {e}"))
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
