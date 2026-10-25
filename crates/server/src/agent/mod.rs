//! The native agent host (S03): runs attempts of agent tasks inside the server.
//!
//! The host owns every authority the agent crate does not have. It builds the C-06 context
//! bundle, issues the C-08 grant, holds the workspace lease, and drives the loop turn by turn: a
//! supervised worker process of this same binary (`factory-server agent-worker`) makes one
//! provider call per turn and returns the assembled turn, and the host validates every tool call
//! against its closed schema, checks the fence and the grant, persists the C-04 intent, runs the
//! effect and records the receipt ([`runner`]). Model usage is reserved before each call and
//! settled after it, apart for the operator allowance and the user's own key.
//!
//! * [`workspace_tools`]: the built-in file tools, inside the task's own worktree.
//! * [`tools`]: the tool set of one attempt (built-in tools plus the HOI4 Agent Tools profile).
//! * [`runner`]: one attempt's loop, its pause, interrupt and revocation handling.
//! * [`recovery`]: what startup does with attempts an earlier process left behind.
//! * [`profiles`]: the model profiles a task can use and who pays for them.
//!
//! Every database step runs in a transaction scoped to the account that started the attempt (the
//! grantor), so row-level security applies to the background work exactly as to a request.

pub mod profiles;
pub mod recovery;
pub mod runner;
pub mod tools;
pub mod workspace_tools;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use factory_accounts::{AccountId, ApiError, Clock, Database, SecretStore};
use factory_agent::destination::DestinationPolicy;
use factory_broker::KeyStore;
use factory_hoi4_tools::AgentToolsInstall;
use factory_tasks::{AttemptId, IncarnationId, TaskId};
use factory_workspace::StorageRoot;
use sqlx::{Postgres, Transaction};
use tokio::sync::{Semaphore, broadcast, watch};

/// What the host asks of a running attempt. The runner reads it at every boundary; only
/// [`Control::Interrupt`] also stops a model turn in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Run,
    /// Finish the current step, then suspend. Survives restart through the task's pause flag.
    Pause,
    /// Stop now: cancel the model turn in flight and settle what was started.
    Interrupt,
}

/// How the host starts its worker process.
#[derive(Debug, Clone)]
pub struct WorkerLauncher {
    pub program: PathBuf,
    /// Arguments before `--generation N`.
    pub args: Vec<String>,
}

impl WorkerLauncher {
    /// This binary, `agent-worker` subcommand.
    pub fn current_exe() -> Option<Self> {
        Some(Self {
            program: std::env::current_exe().ok()?,
            args: vec!["agent-worker".into()],
        })
    }
}

/// Timing of the loop. Tests shorten them.
#[derive(Debug, Clone)]
pub struct RunnerSettings {
    pub lease_ttl: Duration,
    /// Upper bound of one model turn, on top of the profile's own timeout.
    pub turn_timeout: Duration,
    /// How long a worker may take to answer a cancel before its process group is killed.
    pub cancel_grace: Duration,
    pub handshake_timeout: Duration,
    /// Attempts a fresh worker gets to retry a turn whose request was known not to be accepted.
    pub safe_retries: u32,
}

impl Default for RunnerSettings {
    fn default() -> Self {
        Self {
            lease_ttl: Duration::from_secs(60),
            turn_timeout: Duration::from_secs(180),
            cancel_grace: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(10),
            safe_retries: 2,
        }
    }
}

/// Where the HOI4 Agent Tools server comes from, when it is installed.
#[derive(Debug, Clone)]
pub struct AgentToolsSettings {
    pub install: AgentToolsInstall,
    /// The read-only game reference, passed to the tools as `HOI4_GAME_ROOT` when available.
    pub game_root: Option<PathBuf>,
}

/// Everything the agent host needs. Cloning shares it.
#[derive(Clone)]
pub struct AgentRuntime {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub db: Database,
    pub storage: StorageRoot,
    pub clock: Arc<dyn Clock>,
    pub incarnation: IncarnationId,
    pub keys: Option<KeyStore>,
    pub policy: DestinationPolicy,
    pub allowance_micros: i64,
    pub operator_keys: profiles::OperatorKeys,
    pub agent_tools: Option<AgentToolsSettings>,
    pub worker: Option<WorkerLauncher>,
    pub settings: RunnerSettings,
    pub running: Mutex<HashMap<AttemptId, Running>>,
    pub notifier: broadcast::Sender<TaskId>,
    pub slots: Arc<Semaphore>,
}

/// A runner of this process.
pub(crate) struct Running {
    pub control: watch::Sender<Control>,
    pub abort: Option<tokio::task::AbortHandle>,
}

/// The parts of the runtime that startup assembles.
pub struct AgentRuntimeParts {
    pub db: Database,
    pub storage: StorageRoot,
    pub clock: Arc<dyn Clock>,
    pub secrets: Option<Arc<dyn SecretStore>>,
    pub policy: DestinationPolicy,
    pub allowance_micros: i64,
    pub operator_keys: profiles::OperatorKeys,
    pub agent_tools: Option<AgentToolsSettings>,
    pub worker: Option<WorkerLauncher>,
    pub settings: RunnerSettings,
    pub max_running: usize,
}

impl AgentRuntime {
    /// Registers this process as a new host incarnation. Every lease it takes and every
    /// effect it records is fenced by that identity.
    pub async fn start(parts: AgentRuntimeParts) -> Result<Self, factory_tasks::TaskError> {
        let mut conn = parts.db.pool().acquire().await?;
        let incarnation = factory_tasks::register_incarnation(&mut conn, "factory-server").await?;
        drop(conn);
        let (notifier, _) = broadcast::channel(1024);
        Ok(Self {
            inner: Arc::new(Inner {
                db: parts.db,
                storage: parts.storage,
                clock: parts.clock,
                incarnation,
                keys: parts.secrets.map(KeyStore::new),
                policy: parts.policy,
                allowance_micros: parts.allowance_micros,
                operator_keys: parts.operator_keys,
                agent_tools: parts.agent_tools,
                worker: parts.worker,
                settings: parts.settings,
                running: Mutex::new(HashMap::new()),
                notifier,
                slots: Arc::new(Semaphore::new(parts.max_running.max(1))),
            }),
        })
    }

    pub fn incarnation(&self) -> IncarnationId {
        self.inner.incarnation
    }

    pub fn keys_available(&self) -> bool {
        self.inner.keys.is_some()
    }

    pub fn agent_tools_installed(&self) -> bool {
        self.inner.agent_tools.is_some()
    }

    pub fn policy(&self) -> &DestinationPolicy {
        &self.inner.policy
    }

    pub fn allowance_micros(&self) -> i64 {
        self.inner.allowance_micros
    }

    /// A transaction with row-level security scoped to `account`.
    pub async fn tx(&self, account: AccountId) -> Result<Transaction<'static, Postgres>, ApiError> {
        self.inner.db.scoped(account).begin().await
    }

    /// Wakes the event streams of `task`.
    pub fn notify(&self, task: TaskId) {
        let _ = self.inner.notifier.send(task);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TaskId> {
        self.inner.notifier.subscribe()
    }

    /// Sends `control` to the runner of `attempt`. False when no runner of this process holds it.
    pub fn signal(&self, attempt: AttemptId, control: Control) -> bool {
        let running = self.inner.running.lock().unwrap_or_else(|e| e.into_inner());
        match running.get(&attempt) {
            Some(r) => {
                let _ = r.control.send(control);
                true
            }
            None => false,
        }
    }

    pub fn is_running(&self, attempt: AttemptId) -> bool {
        let running = self.inner.running.lock().unwrap_or_else(|e| e.into_inner());
        running.contains_key(&attempt)
    }

    /// Drops every runner of this process mid-step without settling anything, as a process
    /// death would (their workers are killed with them). Tests use it; recovery then runs in a
    /// new incarnation.
    pub fn abort_all(&self) {
        let mut running = self.inner.running.lock().unwrap_or_else(|e| e.into_inner());
        for (_, r) in running.drain() {
            if let Some(abort) = r.abort {
                abort.abort();
            }
        }
    }
}
