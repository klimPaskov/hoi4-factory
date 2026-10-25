//! `SetupAdapter`: the C-09 operations on server workspaces.
//!
//! One adapter serves the whole server. It opens one facade session per
//! (account, project, onboarding session), keeps the in-memory Setup gates there, and persists
//! everything the factory must remember (sessions, scans, analyses, plans, approvals, transaction
//! projections, C-04 style receipts) in PostgreSQL. The Setup journal stays in the project's
//! state root on the workspace volume and is the only authority for recovery.
//!
//! The operations are in `ops_*.rs`; this file holds construction, authorization, session
//! management, receipts and the helpers they share.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use factory_accounts::{Database, Permission, RetryClass};
use factory_workspace::{
    AccountId as WsAccountId, ProjectId as WsProjectId, ResolveMode, SnapshotInventory,
    StorageRoot, Workspace,
};
use hoi4_mod_setup::AppError;
use hoi4_mod_setup::credentials::{CredentialStore, DeniedCredentialStore};
use hoi4_mod_setup::service::{
    NodeRuntime, PlanBuilder, SessionConfig, SetupService, SetupSession,
};
use hoi4_mod_setup::source::SourceFetcher;
use hoi4_mod_setup::transaction::PostInstallActionRunner;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{SetupError, native_kind};
use crate::ids::{AccountId, OnboardingSessionId, OpContext, ProjectId};
use crate::model::ModelClient;
use crate::pin::SETUP_PIN;
use crate::recovery::hex;
use crate::redact::Redactor;
use crate::store::{ReceiptIntent, ReceiptOutcome, ReceiptStatus, Scope, SetupOperation, Store};
use crate::views::{OperationCapability, SessionInfo, SetupCapabilities};

/// Directory name of the Setup state root, a sibling of `source/` in the project workspace.
pub const STATE_DIR_NAME: &str = "setup-state";

/// What the server supplies.
pub struct SetupAdapterDeps {
    pub database: Database,
    pub storage: StorageRoot,
    /// Credential store handed to Setup. [`DeniedCredentialStore`] by default: Setup then can read
    /// no credential, and analysis is host-run through the [`ModelClient`].
    pub credentials: Arc<dyn CredentialStore + Send + Sync>,
    /// Provider access for `analyze_setup`. Without it analysis is unavailable.
    pub model: Option<Arc<dyn ModelClient>>,
    /// Plan builder. Until the default builder is extracted upstream and this crate is bumped to
    /// that revision, none exists, and `plan_setup` reports `capability_unavailable`.
    pub plan_builder: Option<Arc<dyn PlanBuilder>>,
    pub node: Option<NodeRuntime>,
    /// Runs reviewed post-install actions. None runs without it.
    pub post_install_runner: Option<PostInstallActionRunner>,
    /// Where the workflow package comes from. Analysis resolves the source revision through it
    /// and the default plan builder fetches the files through the same client, so both agree on
    /// one revision. Without it, [`SetupAdapter::analyze_session`] is unavailable.
    pub source: Option<Arc<dyn SourceFetcher>>,
    /// Which provider, model and endpoint the server's [`ModelClient`] stands for. The analysis
    /// record, the plan and the receipts all carry exactly this identity.
    pub analysis_route: Option<AnalysisRoute>,
    /// Compare workspace inventories around read-only operations. On by default.
    pub verify_read_only: bool,
}

/// The provider route the server's model client implements. Recorded in the analysis and checked
/// again when the plan is built, so the model named at confirmation is the model that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRoute {
    pub provider: String,
    pub model: String,
    pub reasoning_effort: String,
    pub endpoint: String,
}

impl AnalysisRoute {
    /// The Anthropic Messages API route (Setup's `claude` provider).
    pub fn anthropic(model: impl Into<String>) -> Self {
        Self {
            provider: "claude".into(),
            model: model.into(),
            reasoning_effort: "high".into(),
            endpoint: "https://api.anthropic.com/v1/messages".into(),
        }
    }
}

impl SetupAdapterDeps {
    pub fn new(database: Database, storage: StorageRoot) -> Self {
        Self {
            database,
            storage,
            credentials: Arc::new(DeniedCredentialStore),
            model: None,
            plan_builder: None,
            node: None,
            post_install_runner: None,
            source: None,
            analysis_route: None,
            verify_read_only: true,
        }
    }

    pub fn with_credentials(mut self, store: Arc<dyn CredentialStore + Send + Sync>) -> Self {
        self.credentials = store;
        self
    }

    pub fn with_model(mut self, model: Arc<dyn ModelClient>) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_plan_builder(mut self, builder: Arc<dyn PlanBuilder>) -> Self {
        self.plan_builder = Some(builder);
        self
    }

    pub fn with_node_runtime(mut self, node: NodeRuntime) -> Self {
        self.node = Some(node);
        self
    }

    pub fn with_post_install_runner(mut self, runner: PostInstallActionRunner) -> Self {
        self.post_install_runner = Some(runner);
        self
    }

    pub fn with_source(mut self, source: Arc<dyn SourceFetcher>) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_analysis_route(mut self, route: AnalysisRoute) -> Self {
        self.analysis_route = Some(route);
        self
    }
}

type SessionKey = (AccountId, ProjectId, OnboardingSessionId);

pub(crate) struct ActiveSession {
    pub id: OnboardingSessionId,
    pub scope: Scope,
    pub workspace: Workspace,
    pub facade: Arc<SetupSession>,
    pub mod_root_rel: String,
    pub mod_root: PathBuf,
    pub redactor: Redactor,
    /// What the review steps of this session have established so far (see `ops_review.rs`).
    pub review: Mutex<crate::ops_review::ReviewState>,
}

/// Authorization result: who acts on which workspace.
pub(crate) struct Access {
    pub scope: Scope,
    pub workspace: Workspace,
    pub state_dir: PathBuf,
    pub redactor: Redactor,
}

pub(crate) struct Inner {
    pub store: Store,
    pub storage: StorageRoot,
    pub service: SetupService,
    pub model: Option<Arc<dyn ModelClient>>,
    pub has_plan_builder: bool,
    pub source: Option<Arc<dyn SourceFetcher>>,
    pub analysis_route: Option<AnalysisRoute>,
    pub sessions: Mutex<HashMap<SessionKey, Arc<ActiveSession>>>,
    /// Identifies this adapter instance in transaction rows and logs.
    pub holder: String,
    pub verify_read_only: bool,
}

/// The C-09 Setup adapter. Cheap to clone.
#[derive(Clone)]
pub struct SetupAdapter {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct ReceiptHandle {
    pub id: Uuid,
    pub scope: Scope,
    pub operation: SetupOperation,
}

impl SetupAdapter {
    pub fn new(deps: SetupAdapterDeps) -> Self {
        let has_plan_builder = deps.plan_builder.is_some();
        let mut service = SetupService::new(deps.credentials);
        if let Some(builder) = deps.plan_builder {
            service = service.with_plan_builder(builder);
        }
        if let Some(node) = deps.node {
            service = service.with_node_runtime(node);
        }
        if let Some(runner) = deps.post_install_runner {
            service = service.with_post_install_runner(runner);
        }
        Self {
            inner: Arc::new(Inner {
                store: Store::new(deps.database),
                storage: deps.storage,
                service,
                model: deps.model,
                has_plan_builder,
                source: deps.source,
                analysis_route: deps.analysis_route,
                sessions: Mutex::new(HashMap::new()),
                holder: format!("{}:{}", std::process::id(), Uuid::new_v4().simple()),
                verify_read_only: deps.verify_read_only,
            }),
        }
    }

    /// The Setup revision this build runs and what it is known to contain.
    pub fn setup_revision(&self) -> crate::pin::SetupPin {
        SETUP_PIN
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn holder(&self) -> &str {
        &self.inner.holder
    }

    /// What this deployment can do, with reasons for what it cannot.
    pub fn capabilities(&self) -> SetupCapabilities {
        let analysis = if self.inner.model.is_none() {
            (
                false,
                Some("no ModelClient was supplied by the server".to_owned()),
            )
        } else if self.inner.analysis_route.is_none() || self.inner.source.is_none() {
            (
                false,
                Some("the server named no analysis route or Setup source".to_owned()),
            )
        } else {
            (true, None)
        };
        let plan = if self.inner.has_plan_builder {
            (true, None)
        } else if SETUP_PIN.default_plan_builder {
            (false, Some("the host injected no PlanBuilder".to_owned()))
        } else {
            (
                false,
                Some(format!(
                    "Setup revision {} ships no default PlanBuilder (the extraction pull request is not merged) and the host injected none",
                    SETUP_PIN.short()
                )),
            )
        };
        let always = |name: &'static str| OperationCapability {
            operation: name,
            available: true,
            reason: None,
        };
        let op =
            |name: &'static str, (available, reason): (bool, Option<String>)| OperationCapability {
                operation: name,
                available,
                reason,
            };
        let after_plan = |name: &'static str| OperationCapability {
            operation: name,
            available: self.inner.has_plan_builder,
            reason: (!self.inner.has_plan_builder)
                .then(|| "needs a plan, and no plan can be built".to_owned()),
        };
        SetupCapabilities {
            setup: SETUP_PIN,
            operations: vec![
                always("inspectProject"),
                always("confirmSetupEvidence"),
                op("analyzeSetup", analysis),
                always("confirmSetupAnalysis"),
                op("planSetup", plan),
                after_plan("resolveSetupConflicts"),
                after_plan("approveSetupPlan"),
                after_plan("applySetupPlan"),
                always("inspectRecovery"),
                always("recoverSetup"),
                always("openWorkspace"),
            ],
        }
    }

    // ----- authorization ------------------------------------------------------------------

    /// Checks the account's role on the project and opens its workspace. Another account's
    /// project is `not_found`; a role without the permission is `forbidden`.
    pub(crate) async fn access(
        &self,
        account: AccountId,
        project: ProjectId,
        permission: Permission,
    ) -> Result<Access, SetupError> {
        let found = self
            .inner
            .store
            .database()
            .scoped(account)
            .authorize(project, permission)
            .await?;
        if found.pending_deletion() {
            return Err(SetupError::precondition(
                "project_pending_deletion",
                "The project is marked for deletion.",
            ));
        }
        let ws_account = WsAccountId::new(found.owner_account_id.to_string())?;
        let ws_project = WsProjectId::new(found.id.to_string())?;
        let workspace = self
            .inner
            .storage
            .open_workspace(&ws_account, &ws_project)?;
        let state_dir = ensure_state_dir(&workspace)?;
        let redactor = redactor_for(&workspace, &state_dir, project);
        Ok(Access {
            scope: Scope {
                actor: account,
                owner: found.owner_account_id,
                project,
            },
            workspace,
            state_dir,
            redactor,
        })
    }

    // ----- sessions ----------------------------------------------------------------------

    /// Opens an onboarding session: one facade session bound to the project's source directory
    /// and its state root. `mod_root` is a path relative to `source/` (default: `source/`
    /// itself). Anything that would leave `source/` is refused.
    pub async fn open_session(
        &self,
        account: AccountId,
        project: ProjectId,
        mod_root: Option<&str>,
    ) -> Result<SessionInfo, SetupError> {
        let access = self
            .access(account, project, Permission::ReadProject)
            .await?;
        let (rel, root) = resolve_mod_root(&access.workspace, mod_root)?;
        let config = SessionConfig::new(access.workspace.source_dir(), &access.state_dir)
            .map_err(|e| map_native(e, &access.redactor))?;
        let facade = Arc::new(self.inner.service.open_session(config));
        // Host paths as the facade resolved them, for redaction.
        let redactor = Redactor::new(
            access
                .redactor
                .pairs()
                .chain(std::iter::once((
                    facade.workspace_root().display().to_string(),
                    source_locator(project),
                )))
                .chain(std::iter::once((
                    facade.state_root().display().to_string(),
                    state_locator(project),
                ))),
        );
        let id = OnboardingSessionId::new();
        self.inner
            .store
            .insert_session(&access.scope, id.0, &rel)
            .await?;
        let root_locator = mod_root_locator(project, &rel);
        let active = Arc::new(ActiveSession {
            id,
            scope: access.scope,
            workspace: access.workspace,
            facade,
            mod_root_rel: rel.clone(),
            mod_root: root,
            redactor,
            review: Mutex::default(),
        });
        self.inner
            .sessions
            .lock()
            .map_err(|_| SetupError::Internal("session registry poisoned".into()))?
            .insert((account, project, id), active);
        Ok(SessionInfo {
            session_id: id,
            account_id: account,
            project_id: project,
            mod_root: rel,
            root_locator,
            setup: SETUP_PIN,
            capabilities: self.capabilities(),
        })
    }

    /// Closes the session. Its in-memory analyses, plans and approvals are gone; the persisted
    /// rows and receipts stay.
    pub async fn close_session(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> Result<(), SetupError> {
        let active = self.session(account, project, session)?;
        self.inner
            .sessions
            .lock()
            .map_err(|_| SetupError::Internal("session registry poisoned".into()))?
            .remove(&(account, project, session));
        self.inner
            .store
            .close_session(&active.scope, session.0)
            .await
    }

    /// The live session for exactly this account, project and session ID.
    pub(crate) fn session(
        &self,
        account: AccountId,
        project: ProjectId,
        session: OnboardingSessionId,
    ) -> Result<Arc<ActiveSession>, SetupError> {
        self.inner
            .sessions
            .lock()
            .map_err(|_| SetupError::Internal("session registry poisoned".into()))?
            .get(&(account, project, session))
            .cloned()
            .ok_or(SetupError::NotFound)
    }

    /// A facade session without onboarding state, for recovery: it reads the native journal in
    /// the state root and needs no model, scan or plan.
    pub(crate) fn recovery_facade(&self, access: &Access) -> Result<SetupSession, SetupError> {
        let config = SessionConfig::new(access.workspace.source_dir(), &access.state_dir)
            .map_err(|e| map_native(e, &access.redactor))?;
        Ok(self.inner.service.open_session(config))
    }

    // ----- receipts ----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn open_receipt(
        &self,
        scope: &Scope,
        session: Option<Uuid>,
        operation: SetupOperation,
        ctx: &OpContext,
        request: Value,
        subject: Option<Uuid>,
        root_identity: Option<String>,
        fence: Option<i64>,
        idempotency_key: Option<String>,
    ) -> Result<ReceiptHandle, SetupError> {
        let id = Uuid::new_v4();
        let request_sha256 = sha256_json(&request);
        self.inner
            .store
            .insert_receipt(
                scope,
                &ReceiptIntent {
                    id,
                    session_id: session,
                    operation,
                    request_id: ctx.request_id.unwrap_or_else(Uuid::new_v4),
                    idempotency_key,
                    request,
                    request_sha256,
                    subject_id: subject,
                    root_identity,
                    grant_id: ctx.grant_id.clone(),
                    revocation_epoch: ctx.revocation_epoch,
                    fence_generation: fence,
                    retry_class: if operation.effect_class() == "project_write" {
                        RetryClass::ReconcileFirst
                    } else {
                        RetryClass::SafeSameInputs
                    },
                    started_at: self.inner.store.now(),
                },
            )
            .await?;
        Ok(ReceiptHandle {
            id,
            scope: *scope,
            operation,
        })
    }

    /// Settles the receipt from the result and passes the result through. A failure to settle is
    /// logged and leaves the intent visible to the reconciler; it never changes the result.
    pub(crate) async fn close_receipt<T>(
        &self,
        handle: ReceiptHandle,
        result: Result<(T, ReceiptOutcome), SetupError>,
    ) -> Result<T, SetupError> {
        let (status, outcome, out) = match result {
            Ok((value, mut outcome)) => {
                if let Some(r) = &outcome.result {
                    outcome.result_sha256 = Some(sha256_json(r));
                }
                (ReceiptStatus::Succeeded, outcome, Ok(value))
            }
            Err(err) => {
                let status = receipt_status_for(&err, handle.operation);
                let outcome = ReceiptOutcome {
                    error_code: Some(err.code().to_owned()),
                    error_message: Some(err.to_string()),
                    ..ReceiptOutcome::default()
                };
                (status, outcome, Err(err))
            }
        };
        if let Err(e) = self
            .inner
            .store
            .settle_receipt(&handle.scope, handle.id, status, &outcome)
            .await
        {
            tracing::error!(receipt = %handle.id, error = %e, "could not settle a setup receipt");
        }
        out
    }

    /// A refusal before any intent worth keeping: write and settle a receipt at once, with no
    /// idempotency key, so the refusal is on record and a retry is not blocked.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn record_refusal(
        &self,
        scope: &Scope,
        session: Option<Uuid>,
        operation: SetupOperation,
        ctx: &OpContext,
        request: Value,
        subject: Option<Uuid>,
        err: &SetupError,
    ) {
        let handle = match self
            .open_receipt(
                scope, session, operation, ctx, request, subject, None, None, None,
            )
            .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::error!(error = %e, "could not record a refused setup operation");
                return;
            }
        };
        let outcome = ReceiptOutcome {
            error_code: Some(err.code().to_owned()),
            error_message: Some(err.to_string()),
            ..ReceiptOutcome::default()
        };
        if let Err(e) = self
            .inner
            .store
            .settle_receipt(&handle.scope, handle.id, ReceiptStatus::Refused, &outcome)
            .await
        {
            tracing::error!(error = %e, "could not settle a refusal receipt");
        }
    }
}

fn receipt_status_for(err: &SetupError, op: SetupOperation) -> ReceiptStatus {
    match err {
        SetupError::UnknownEffect(_) | SetupError::ApplyInterrupted { .. } => {
            ReceiptStatus::UnknownEffect
        }
        SetupError::CapabilityUnavailable { .. }
        | SetupError::OutsideWorkspace(_)
        | SetupError::NotFound
        | SetupError::Forbidden
        | SetupError::ProjectBusy { .. }
        | SetupError::RecoveryRequired { .. }
        | SetupError::Precondition { .. }
        | SetupError::Stale { .. }
        | SetupError::InvalidValues { .. }
        | SetupError::IdempotencyReplay { .. } => ReceiptStatus::Refused,
        SetupError::Native { .. } if op.effect_class() != "project_write" => ReceiptStatus::Refused,
        _ => ReceiptStatus::Failed,
    }
}

// ----- shared helpers ---------------------------------------------------------------------

pub(crate) fn sha256_json(value: &Value) -> String {
    hex(&Sha256::digest(
        serde_json::to_vec(value).unwrap_or_default(),
    ))
}

pub(crate) fn source_locator(project: ProjectId) -> String {
    format!("workspace:{project}/source")
}

pub(crate) fn state_locator(project: ProjectId) -> String {
    format!("workspace:{project}/{STATE_DIR_NAME}")
}

pub(crate) fn mod_root_locator(project: ProjectId, rel: &str) -> String {
    if rel.is_empty() {
        source_locator(project)
    } else {
        format!("{}/{rel}", source_locator(project))
    }
}

fn redactor_for(workspace: &Workspace, state_dir: &Path, project: ProjectId) -> Redactor {
    Redactor::new([
        (
            workspace.source_dir().display().to_string(),
            source_locator(project),
        ),
        (state_dir.display().to_string(), state_locator(project)),
        (
            workspace
                .source_dir()
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            format!("workspace:{project}"),
        ),
    ])
}

/// Creates the Setup state root next to `source/`, outside every tree it protects.
fn ensure_state_dir(workspace: &Workspace) -> Result<PathBuf, SetupError> {
    let project_dir = workspace
        .source_dir()
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| SetupError::Workspace("workspace layout has no project directory".into()))?;
    let dir = project_dir.join(STATE_DIR_NAME);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(dir),
        Ok(_) => Err(SetupError::Workspace(
            "the Setup state directory is not a plain directory".into(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&dir).map_err(|e| {
                SetupError::Workspace(format!("cannot create the state directory: {e}"))
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750));
            }
            Ok(dir)
        }
        Err(e) => Err(SetupError::Workspace(format!(
            "cannot inspect the state directory: {e}"
        ))),
    }
}

/// Resolve the mod root below `source/`. Returns the normalized relative path and the host path.
pub(crate) fn resolve_mod_root(
    workspace: &Workspace,
    requested: Option<&str>,
) -> Result<(String, PathBuf), SetupError> {
    let rel = requested.map(str::trim).unwrap_or("");
    if rel.is_empty() || rel == "." {
        return Ok((String::new(), workspace.source_dir()));
    }
    let path = workspace.resolve_source(rel, ResolveMode::MustExist)?;
    if !path.is_dir() {
        return Err(SetupError::OutsideWorkspace(
            "the mod root is not a directory".into(),
        ));
    }
    // Containment, independent of the path rules above.
    let source = std::fs::canonicalize(workspace.source_dir())
        .map_err(|e| SetupError::Workspace(e.to_string()))?;
    let canonical =
        std::fs::canonicalize(&path).map_err(|e| SetupError::Workspace(e.to_string()))?;
    if !canonical.starts_with(&source) {
        return Err(SetupError::OutsideWorkspace(
            "the mod root is not inside the project's source directory".into(),
        ));
    }
    Ok((rel.replace('\\', "/"), path))
}

/// Runs blocking Setup work off the async runtime.
pub(crate) async fn blocking<T, F>(f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Process(format!("a Setup task did not finish: {e}")))?
}

/// Capture the inventory of the source tree.
pub(crate) async fn capture_inventory(
    workspace: &Workspace,
) -> Result<SnapshotInventory, SetupError> {
    let workspace = workspace.clone();
    tokio::task::spawn_blocking(move || {
        let repo = workspace.git()?;
        SnapshotInventory::capture(&workspace.source_dir(), repo.as_ref())
    })
    .await
    .map_err(|e| SetupError::Internal(format!("inventory task: {e}")))?
    .map_err(SetupError::from)
}

/// Maps a native error to an adapter error. Gate refusals get their own stable codes; the rest
/// keep the native family. Host paths in the message are replaced first.
pub(crate) fn map_native(error: AppError, redactor: &Redactor) -> SetupError {
    let kind = native_kind(&error);
    let message = redactor.text(&error.to_string());
    let lower = message.to_ascii_lowercase();
    let has = |needle: &str| lower.contains(needle);
    if has("no plan builder") {
        return SetupError::CapabilityUnavailable {
            capability: "plan_builder",
            reason: "this service has no plan builder configured".into(),
        };
    }
    if has("outside the session workspace")
        || has("may not contain traversal")
        || has("outside the project root")
        || has("outside the workspace")
        || has("must be inside")
    {
        return SetupError::OutsideWorkspace(message);
    }
    if has("no longer present in the core session")
        || has("no longer available in the core session")
    {
        return SetupError::stale("analysis_not_in_session", message);
    }
    if has("not bound to the latest scan") {
        return SetupError::stale("scan_stale", message);
    }
    if has("analysis")
        && (has("must be confirmed")
            || has("confirmed selected-provider")
            || has("confirmed analysis"))
    {
        return SetupError::precondition("analysis_not_confirmed", message);
    }
    if has("scan evidence")
        || has("codex evidence")
        || has("analysis requires approved read-only evidence")
    {
        return SetupError::precondition("evidence_not_approved", message);
    }
    if has("approval is required") {
        return SetupError::precondition("plan_not_approved", message);
    }
    if has("not available in this session") {
        return SetupError::stale("plan_not_in_session", message);
    }
    if has("changed after it was reviewed") || has("changed after it was approved") {
        return SetupError::stale("plan_changed", message);
    }
    if has("root changed") {
        return SetupError::stale("root_changed", message);
    }
    SetupError::Native { kind, message }
}

pub(crate) fn json_request(parts: Value) -> Value {
    // Receipts hold the final normalized non-secret arguments.
    json!({ "schema": 1, "args": parts })
}
