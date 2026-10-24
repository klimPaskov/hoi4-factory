//! Shared state of the running server.

use std::sync::Arc;

use axum::extract::FromRef;
use factory_accounts::{AccountsState, Database};
use factory_workspace::StorageRoot;

use crate::game_reference::GameReferenceHandle;
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

/// Everything a handler can reach. Cloning shares the underlying pools and handles.
///
/// Extension point: the intake crate and the Setup adapter add their own fields here and are
/// wired in [`crate::routes::api_routes`]. A handler that needs only one part takes it with
/// `State<AccountsState>` (via [`FromRef`]) instead of the whole struct.
#[derive(Clone)]
pub struct ServerState {
    pub accounts: AccountsState,
    pub db: Database,
    pub storage: StorageRoot,
    pub game_reference: GameReferenceHandle,
    pub migrations: Arc<MigrationReport>,
    pub settings: Arc<RuntimeSettings>,
    pub web: Option<StaticSite>,
    // pub intake: factory_intake::IntakeState,
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
