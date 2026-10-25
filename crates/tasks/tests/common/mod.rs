//! Shared harness: a disposable PostgreSQL database migrated as production would be (accounts
//! 0100 range, then this crate's 0600 range, both as the table-owner role), with the application
//! talking to it as the non-superuser role `hf_test_app`, a member of `factory_app`, so
//! row-level security applies. Two accounts own one project each.
//!
//! Needs a PostgreSQL SUPERUSER URL in `FACTORY_TEST_ADMIN_URL` (or `DATABASE_URL`).
#![allow(dead_code)]

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use factory_accounts::{AccountId, Database, ProjectId, SystemClock, TenantRepo, Visibility};
use factory_tasks::context::{
    ContextInputs, MessageRole, SourceLocator, TaskScope, TokenBudget, VisibleMessage,
    build_context,
};
use factory_tasks::*;
use serde_json::json;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const OWNER_ROLE: &str = "hf_test_owner";
pub const APP_ROLE: &str = "hf_test_app";

fn admin_options() -> PgConnectOptions {
    let url = std::env::var("FACTORY_TEST_ADMIN_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set FACTORY_TEST_ADMIN_URL (a superuser URL) to run the integration tests against PostgreSQL");
    PgConnectOptions::from_str(&url).expect("valid PostgreSQL URL")
}

static ROLES: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn ensure_roles() {
    ROLES
        .get_or_init(|| async {
            let mut conn = PgConnection::connect_with(&admin_options())
                .await
                .expect("connect as superuser");
            let is_super: bool =
                sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
                    .fetch_one(&mut conn)
                    .await
                    .unwrap();
            assert!(is_super, "the admin URL must be a superuser");
            sqlx::query("SELECT pg_advisory_lock(7101)")
                .execute(&mut conn)
                .await
                .unwrap();
            for (role, extra) in [
                (OWNER_ROLE, "LOGIN NOSUPERUSER NOBYPASSRLS"),
                (APP_ROLE, "LOGIN NOSUPERUSER NOBYPASSRLS"),
                ("factory_app", "NOLOGIN NOSUPERUSER NOBYPASSRLS"),
            ] {
                let exists: bool =
                    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
                        .bind(role)
                        .fetch_one(&mut conn)
                        .await
                        .unwrap();
                if !exists {
                    sqlx::query(&format!("CREATE ROLE {role} {extra}"))
                        .execute(&mut conn)
                        .await
                        .unwrap();
                }
            }
            sqlx::query(&format!("GRANT factory_app TO {APP_ROLE}"))
                .execute(&mut conn)
                .await
                .unwrap();
            sqlx::query("SELECT pg_advisory_unlock(7101)")
                .execute(&mut conn)
                .await
                .unwrap();
            conn.close().await.unwrap();
        })
        .await;
}

fn lazy_pool(options: PgConnectOptions, max: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(5))
        .connect_lazy_with(options)
}

pub struct TestDb {
    pub name: String,
    /// Superuser: ground truth only.
    pub admin: PgPool,
    /// Non-superuser table owner (ran the migrations).
    pub owner: PgPool,
    /// Non-superuser application role.
    pub app: PgPool,
}

impl TestDb {
    pub async fn create() -> Self {
        ensure_roles().await;
        let name = format!("hf_t_{}", Uuid::new_v4().simple());
        let mut conn = PgConnection::connect_with(&admin_options()).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{name}\" OWNER {OWNER_ROLE}"))
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        let admin = lazy_pool(admin_options().database(&name), 4);
        let owner = lazy_pool(admin_options().username(OWNER_ROLE).database(&name), 2);
        let app = lazy_pool(admin_options().username(APP_ROLE).database(&name), 16);
        factory_accounts::migrator()
            .run(&owner)
            .await
            .expect("accounts migrations apply");
        factory_tasks::migrator()
            .run(&owner)
            .await
            .expect("tasks migrations apply");
        Self {
            name,
            admin,
            owner,
            app,
        }
    }

    /// A second application pool, as a freshly started process would have.
    pub fn fresh_app_pool(&self) -> PgPool {
        lazy_pool(admin_options().username(APP_ROLE).database(&self.name), 4)
    }
}

pub struct Party {
    pub account: AccountId,
    pub project: ProjectId,
}

/// One test's world: two accounts, one private project each.
pub struct World {
    pub db: TestDb,
    pub database: Database,
    pub a: Party,
    pub b: Party,
}

impl World {
    pub async fn new() -> Self {
        let db = TestDb::create().await;
        let database = Database::new(db.app.clone(), Arc::new(SystemClock));
        let a = account(&db, &database, "alice").await;
        let b = account(&db, &database, "bob").await;
        Self { db, database, a, b }
    }

    pub fn repo(&self, party: &Party) -> TenantRepo {
        self.database.scoped(party.account)
    }

    /// A transaction scoped to `party`'s account (`app.account_id` set).
    pub async fn begin(&self, party: &Party) -> Transaction<'static, Postgres> {
        self.repo(party).begin().await.expect("scoped transaction")
    }

    pub fn ledger(&self, party: &Party) -> Ledger {
        Ledger::new(self.repo(party))
    }

    /// Scalar count as the superuser (ground truth, ignoring row-level security).
    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.admin)
            .await
            .unwrap()
    }

    /// A task for `party`, in `planned`.
    pub async fn task(&self, party: &Party) -> Task {
        let mut tx = self.begin(party).await;
        let task = create_task(&mut tx, new_task(party)).await.unwrap();
        tx.commit().await.unwrap();
        task
    }

    /// A task in `ready` with a stored bundle and a registered incarnation.
    pub async fn ready_task(&self, party: &Party) -> ReadyTask {
        let task = self.task(party).await;
        let mut tx = self.begin(party).await;
        let task = transition_task(
            &mut tx,
            task.id,
            task.version,
            TaskState::Ready,
            EventDraft::new(Actor::Host, "task_ready", json!({})),
        )
        .await
        .unwrap();
        let bundle = sample_bundle(&task);
        store_bundle(&mut tx, party.project, party.account, task.id, &bundle)
            .await
            .unwrap();
        let incarnation = register_incarnation(&mut tx, "test-host").await.unwrap();
        tx.commit().await.unwrap();
        ReadyTask {
            task,
            bundle,
            incarnation,
        }
    }

    /// A task with an admitted attempt, moved to `running`.
    pub async fn running_attempt(&self, party: &Party) -> (Task, Attempt) {
        let ready = self.ready_task(party).await;
        let mut tx = self.begin(party).await;
        let attempt = admit_attempt(
            &mut tx,
            ready.task.id,
            ready.task.version,
            ready.admission(),
        )
        .await
        .unwrap();
        let attempt = transition_attempt(
            &mut tx,
            attempt.id,
            attempt.version,
            AttemptState::Running,
            None,
            None,
            Actor::Host,
        )
        .await
        .unwrap();
        let task = get_task(&mut tx, ready.task.id).await.unwrap();
        tx.commit().await.unwrap();
        (task, attempt)
    }
}

pub struct ReadyTask {
    pub task: Task,
    pub bundle: ContextBundle,
    pub incarnation: IncarnationId,
}

impl ReadyTask {
    pub fn admission(&self) -> Admission {
        Admission {
            bundle_id: self.bundle.id,
            grant_id: Uuid::new_v4(),
            grant_epoch: 1,
            incarnation: self.incarnation,
            generation: 1,
            model_profile: json!({"provider": "anthropic", "model": "claude-haiku-5-5"}),
            funding: Funding::OperatorAllowance,
            user_key_id: None,
        }
    }
}

async fn account(db: &TestDb, database: &Database, name: &str) -> Party {
    let id = AccountId::new();
    sqlx::query(
        "INSERT INTO accounts (id, email, email_verified_at, password_hash, display_name)
         VALUES ($1, $2, now(), 'not-a-real-hash', $3)",
    )
    .bind(id.0)
    .bind(format!("{name}-{}@example.invalid", id.0.simple()))
    .bind(name)
    .execute(&db.admin)
    .await
    .unwrap();
    let project = database
        .scoped(id)
        .create_project(&format!("{name} mod"), Visibility::Private)
        .await
        .unwrap();
    Party {
        account: id,
        project: project.id,
    }
}

pub fn new_task(party: &Party) -> NewTask {
    NewTask {
        project_id: party.project,
        owner_account_id: party.account,
        created_by: party.account,
        objective: "Add one national focus".into(),
        requirement_ids: vec!["REQ-014".into()],
        mode: TaskMode::Edit,
        allowed_paths: vec!["common/national_focus/".into()],
        budget_micros: 200_000,
        max_turns: 20,
        model_choice: json!({"profile": "default"}),
    }
}

pub fn scope_of(task: &Task) -> TaskScope {
    TaskScope {
        objective: task.objective.clone(),
        mode: task.mode,
        allowed_paths: task.allowed_paths.clone(),
        requirement_ids: task.requirement_ids.clone(),
    }
}

pub fn locator(path: &str, text: &str, required: bool) -> SourceLocator {
    SourceLocator {
        path: path.into(),
        sha256: sha256_hex(text.as_bytes()),
        bytes: text.len() as u64,
        region: None,
        reason: "named by the objective".into(),
        required,
        priority: 10,
        content: Some(text.into()),
    }
}

pub fn inputs_for(task: &Task) -> ContextInputs {
    ContextInputs {
        scope: scope_of(task),
        source_locators: vec![locator(
            "common/national_focus/a.txt",
            "focus_tree = {}",
            true,
        )],
        approved_references: vec![],
        completed_receipts: vec![],
        unresolved_effects: vec![],
        recent_messages: vec![VisibleMessage {
            role: MessageRole::User,
            text: "please add a focus".into(),
        }],
        capability_profile: json!({"tools": ["read_file"]}),
        budget: TokenBudget {
            context_tokens: 100_000,
            reserved_output: 4_000,
            reserved_tools: 2_000,
        },
    }
}

pub fn sample_bundle(task: &Task) -> ContextBundle {
    build_context(inputs_for(task)).expect("sample bundle fits")
}

pub fn hash(n: u8) -> String {
    sha256_hex(&[n])
}

pub fn replay_identity() -> ReplayIdentity {
    ReplayIdentity {
        workspace_id: "ws-1".into(),
        root_identity: "root-1".into(),
        source_snapshot: "snap-1".into(),
        tool: "write_file".into(),
        tool_version: "1.0.0".into(),
    }
}

pub fn new_intent(task: &Task, attempt: &Attempt, key: &str, class: EffectClass) -> NewIntent {
    NewIntent {
        task_id: task.id,
        attempt_id: attempt.id,
        request_id: format!("req-{key}"),
        idempotency_key: key.into(),
        replay: replay_identity(),
        schema_version: "1".into(),
        effect_class: class,
        request_artifact: json!({"path": "common/national_focus/a.txt", "content_sha256": hash(1), "credential_ref": "key_ref_1"}),
        grant_id: attempt.grant_id,
        grant_epoch: attempt.grant_epoch,
        fencing_generation: attempt.fencing_generation,
        retry_class: RetryClass::ReconcileFirst,
        reconciliation_method: "observe_target_hash".into(),
        observation_inputs: json!({"path": "common/national_focus/a.txt", "expected_before": hash(1), "expected_after": hash(2)}),
    }
}

/// Error text of a failed statement, or panics when it succeeded.
pub async fn must_fail<T: std::fmt::Debug>(r: Result<T, sqlx::Error>) -> String {
    match r {
        Ok(v) => panic!("expected the statement to fail, got {v:?}"),
        Err(e) => e.to_string(),
    }
}
