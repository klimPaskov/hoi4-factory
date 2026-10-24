//! Shared state of the running server.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::FromRef;
use factory_accounts::{AccountsState, Database};
use factory_intake::{IntakeLimits, IntakeService};
use factory_workspace::StorageRoot;

use crate::game_reference::GameReferenceHandle;
use crate::github::GithubRuntime;
use crate::migrations::MigrationReport;
use crate::web::StaticSite;

/// Settings the request path needs after startup.
#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub development: bool,
    pub max_body_bytes: usize,
    /// Normalised origins CORS may echo.
    pub cors_origins: Vec<String>,
}

/// Upload and GitHub intake as the routes use them.
#[derive(Clone)]
pub struct IntakeState {
    pub service: IntakeService,
    /// `None` when the GitHub App is not configured or its key could not be used.
    pub github: Option<GithubRuntime>,
    pub limits: IntakeLimits,
    /// Body limit of the upload route.
    pub upload_max_bytes: usize,
    /// Where received bytes are spooled before intake reads them. Under the data root so a
    /// large upload does not fill the memory-backed `/tmp` of the container.
    pub incoming_dir: PathBuf,
}

/// Everything a handler can reach. Cloning shares the underlying pools and handles.
///
/// Extension point: the Setup adapter adds its own field here and is wired in
/// [`crate::routes::api_routes`]. A handler that needs only one part takes it with
/// `State<AccountsState>` (via [`FromRef`]) instead of the whole struct.
#[derive(Clone)]
pub struct ServerState {
    pub accounts: AccountsState,
    pub db: Database,
    pub storage: StorageRoot,
    pub game_reference: GameReferenceHandle,
    pub intake: IntakeState,
    pub migrations: Arc<MigrationReport>,
    pub settings: Arc<RuntimeSettings>,
    pub web: Option<StaticSite>,
    // pub setup: factory_setup_adapter::SetupState,
}

impl FromRef<ServerState> for AccountsState {
    fn from_ref(state: &ServerState) -> Self {
        state.accounts.clone()
    }
}

impl FromRef<ServerState> for Database {
    fn from_ref(state: &ServerState) -> Self {
        state.db.clone()
    }
}

impl FromRef<ServerState> for StorageRoot {
    fn from_ref(state: &ServerState) -> Self {
        state.storage.clone()
    }
}

impl FromRef<ServerState> for GameReferenceHandle {
    fn from_ref(state: &ServerState) -> Self {
        state.game_reference.clone()
    }
}

impl FromRef<ServerState> for IntakeState {
    fn from_ref(state: &ServerState) -> Self {
        state.intake.clone()
    }
}
