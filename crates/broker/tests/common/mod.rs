//! Shared harness: a disposable PostgreSQL database migrated as production would be (accounts
//! and broker ranges), reached as the non-superuser application role so row-level security
//! applies.
//!
//! Needs a PostgreSQL SUPERUSER URL in `FACTORY_TEST_ADMIN_URL` (or `DATABASE_URL`).
#![allow(dead_code)]

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use factory_accounts::{AccountId, Database, ProjectId, Role, SystemClock, Visibility};
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

pub struct Env {
    pub name: String,
    /// Superuser: ground truth only.
    pub admin: PgPool,
    /// Non-superuser table owner (ran the migrations).
    pub owner: PgPool,
    /// Non-superuser application role.
    pub app: PgPool,
    pub database: Database,
}

impl Env {
    pub async fn new() -> Self {
        ensure_roles().await;
        let name = format!("hf_b_{}", Uuid::new_v4().simple());
        let mut conn = PgConnection::connect_with(&admin_options()).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{name}\" OWNER {OWNER_ROLE}"))
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        let admin = lazy_pool(admin_options().database(&name), 2);
        let owner = lazy_pool(admin_options().username(OWNER_ROLE).database(&name), 2);
        let app = lazy_pool(admin_options().username(APP_ROLE).database(&name), 16);
        factory_accounts::migrator()
            .run(&owner)
            .await
            .expect("accounts migrations apply");
        factory_broker::migrator()
            .run(&owner)
            .await
            .expect("broker migrations apply");
        let database = Database::new(app.clone(), Arc::new(SystemClock));
        Self {
            name,
            admin,
            owner,
            app,
            database,
        }
    }

    /// Insert an account row as the superuser (sign-in is the accounts crate's concern).
    pub async fn account(&self, name: &str) -> AccountId {
        let id = AccountId::new();
        sqlx::query(
            "INSERT INTO accounts (id, email, email_verified_at, password_hash, display_name)
             VALUES ($1, $2, now(), 'not-a-real-hash', $3)",
        )
        .bind(id.0)
        .bind(format!("{name}-{}@example.invalid", id.0.simple()))
        .bind(name)
        .execute(&self.admin)
        .await
        .unwrap();
        id
    }

    pub async fn project(&self, owner: AccountId) -> ProjectId {
        self.database
            .scoped(owner)
            .create_project("Broker fixture", Visibility::Private)
            .await
            .unwrap()
            .id
    }

    pub async fn member(&self, owner: AccountId, project: ProjectId, who: AccountId, role: Role) {
        self.database
            .scoped(owner)
            .add_member(project, who, role)
            .await
            .unwrap();
    }

    /// A transaction with `app.account_id` set, as every request handler gets.
    pub async fn tx(&self, account: AccountId) -> Transaction<'static, Postgres> {
        self.database.scoped(account).begin().await.unwrap()
    }

    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.admin)
            .await
            .unwrap()
    }
}
