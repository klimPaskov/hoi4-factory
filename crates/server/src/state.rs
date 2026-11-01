//! Shared state of the running server.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::FromRef;
use factory_accounts::{AccountsState, Database, InMemoryRateLimiter};
use factory_intake::{IntakeLimits, IntakeService};
use factory_setup_adapter::SetupAdapter;
use factory_workspace::StorageRoot;

use crate::agent::AgentRuntime;
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

/// The Setup adapter. It keeps the Setup gates of every onboarding session in memory and the
/// session rows in PostgreSQL, so a restart ends the in-memory sessions and the routes answer
/// `setup_session_expired` for them (see `setup_api`).
#[derive(Clone)]
pub struct SetupState {
    pub adapter: SetupAdapter,
}

/// Everything a handler can reach. Cloning shares the underlying pools and handles.
///
/// A handler that needs only one part takes it with
/// `State<AccountsState>` (via [`FromRef`]) instead of the whole struct.
#[derive(Clone)]
pub struct ServerState {
    pub accounts: AccountsState,
    pub db: Database,
    pub storage: StorageRoot,
    pub game_reference: GameReferenceHandle,
    pub intake: IntakeState,
    pub setup: SetupState,
    pub migrations: Arc<MigrationReport>,
    pub settings: Arc<RuntimeSettings>,
    pub web: Option<StaticSite>,
    /// The native agent host. `None` only when it could not start; the task routes then
    /// answer `agent_unavailable`.
    pub agent: Option<AgentRuntime>,
    /// Per-account limits of the expensive intake and Setup routes (see [`crate::access::limit`]).
    pub route_limits: Arc<InMemoryRateLimiter>,
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

impl FromRef<ServerState> for SetupState {
    fn from_ref(state: &ServerState) -> Self {
        state.setup.clone()
    }
}

impl FromRef<ServerState> for IntakeState {
    fn from_ref(state: &ServerState) -> Self {
        state.intake.clone()
    }
}
