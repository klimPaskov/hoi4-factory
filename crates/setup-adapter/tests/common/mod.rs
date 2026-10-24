//! Shared harness: a disposable PostgreSQL database migrated as production would be, two
//! accounts, server workspaces on a temporary data root, and a synthetic import fixture.
//!
//! Needs a PostgreSQL SUPERUSER URL in `FACTORY_TEST_ADMIN_URL` (or `DATABASE_URL`), the same
//! as the accounts crate's tests. The application talks to the database as the non-superuser
//! role `hf_test_app`, a member of `factory_app`, so row-level security applies.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use factory_accounts::{AccountId, Database, ProjectId, SystemClock, Visibility};
use factory_setup_adapter::{SetupAdapter, SetupAdapterDeps};
use factory_workspace::{AccountId as WsAccount, ProjectId as WsProject, StorageRoot};
use hoi4_mod_setup::models::{SourceIdentity, SourceMode};
use hoi4_mod_setup::security::sha256_bytes;
use hoi4_mod_setup::source::{SOURCE_NAME, SOURCE_OWNER, SourceResolution, parse_manifest};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool};
use tempfile::TempDir;
use uuid::Uuid;

pub mod native;

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
        let name = format!("hf_s_{}", Uuid::new_v4().simple());
        let mut conn = PgConnection::connect_with(&admin_options()).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{name}\" OWNER {OWNER_ROLE}"))
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        let admin = lazy_pool(admin_options().database(&name), 2);
        let owner = lazy_pool(admin_options().username(OWNER_ROLE).database(&name), 2);
        let app = lazy_pool(admin_options().username(APP_ROLE).database(&name), 12);
        factory_accounts::migrator()
            .run(&owner)
            .await
            .expect("accounts migrations apply");
        factory_setup_adapter::migrator()
            .run(&owner)
            .await
            .expect("setup migrations apply");
        Self {
            name,
            admin,
            owner,
            app,
        }
    }
}

/// One test's world.
pub struct Env {
    pub db: TestDb,
    pub database: Database,
    pub storage: StorageRoot,
    pub tmp: TempDir,
}

pub struct Fixture {
    pub account: AccountId,
    pub project: ProjectId,
    pub source: PathBuf,
    pub state: PathBuf,
}

impl Env {
    pub async fn new() -> Self {
        let db = TestDb::create().await;
        let database = Database::new(db.app.clone(), Arc::new(SystemClock));
        let tmp = tempfile::tempdir().unwrap();
        let data = std::fs::canonicalize(tmp.path()).unwrap().join("data");
        let storage = StorageRoot::new(data);
        storage.ensure_layout().unwrap();
        Self {
            db,
            database,
            storage,
            tmp,
        }
    }

    pub fn adapter(&self) -> SetupAdapter {
        SetupAdapter::new(SetupAdapterDeps::new(
            self.database.clone(),
            self.storage.clone(),
        ))
    }

    pub fn adapter_with(
        &self,
        f: impl FnOnce(SetupAdapterDeps) -> SetupAdapterDeps,
    ) -> SetupAdapter {
        SetupAdapter::new(f(SetupAdapterDeps::new(
            self.database.clone(),
            self.storage.clone(),
        )))
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
        .execute(&self.db.admin)
        .await
        .unwrap();
        id
    }

    /// A project owned by `owner` with its workspace holding the synthetic import fixture.
    pub async fn project_with_import(&self, owner: AccountId) -> Fixture {
        let project = self
            .database
            .scoped(owner)
            .create_project("Fixture mod", Visibility::Private)
            .await
            .unwrap();
        let ws = self
            .storage
            .create_workspace(
                &WsAccount::new(owner.to_string()).unwrap(),
                &WsProject::new(project.id.to_string()).unwrap(),
            )
            .unwrap();
        let source = ws.source_dir();
        build_import_fixture(&source, self.tmp.path());
        let state = source
            .parent()
            .unwrap()
            .join(factory_setup_adapter::STATE_DIR_NAME);
        Fixture {
            account: owner,
            project: project.id,
            source,
            state,
        }
    }

    /// Row count in `table` as the superuser (ground truth, ignoring row-level security).
    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.admin)
            .await
            .unwrap()
    }
}

// ----- synthetic fixture -------------------------------------------------------------------

fn git(dir: &Path, home: &Path, args: &[&str]) -> String {
    let empty = home.join("empty.gitconfig");
    let _ = std::fs::write(&empty, "");
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &empty)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_AUTHOR_DATE", "2026-10-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-01T00:00:00Z")
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "core.autocrlf=false",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Same shape as probes/setup/build_fixture.sh: CRLF focus file, BOM localisation, a binary asset,
/// an executable script, committed history, a staged edit, an unstaged edit and untracked files.
pub fn build_import_fixture(dst: &Path, scratch: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import");
    copy_tree(&src, dst);
    let focus = dst.join("common/national_focus/exp01_synthetic_focus.txt");
    let text = std::fs::read(&focus).unwrap();
    let text = String::from_utf8(text)
        .unwrap()
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    std::fs::write(&focus, text).unwrap();
    let loc = dst.join("localisation/english/exp01_synthetic_l_english.yml");
    let mut bytes = std::fs::read(&loc).unwrap();
    if !bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.splice(0..0, [0xEF, 0xBB, 0xBF]);
    }
    std::fs::write(&loc, bytes).unwrap();
    // 4096 deterministic binary bytes.
    use sha2::{Digest, Sha256};
    let mut state = b"EXP01 synthetic asset".to_vec();
    let mut out = Vec::new();
    while out.len() < 4096 {
        state = Sha256::digest(&state).to_vec();
        out.extend_from_slice(&state);
    }
    out.truncate(4096);
    std::fs::create_dir_all(dst.join("gfx")).unwrap();
    std::fs::write(dst.join("gfx/exp01_custom_asset.bin"), out).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dst.join("tools/exp01_check.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let home = scratch.join("githome");
    std::fs::create_dir_all(&home).unwrap();
    git(dst, &home, &["init", "-q", "."]);
    git(
        dst,
        &home,
        &[
            "add",
            "descriptor.mod",
            "common",
            "localisation",
            "AGENTS.md",
            ".agents",
            "README.md",
            "tools",
            "exp01_setup",
            "gfx",
        ],
    );
    git(dst, &home, &["commit", "-q", "-m", "synthetic baseline"]);
    // Staged localisation edit.
    let mut loc_bytes = std::fs::read(&loc).unwrap();
    loc_bytes.extend_from_slice(b" EXP01_staged_line:0 \"Staged but not committed\"\r\n");
    std::fs::write(&loc, loc_bytes).unwrap();
    git(
        dst,
        &home,
        &["add", "localisation/english/exp01_synthetic_l_english.yml"],
    );
    // Unstaged edit and untracked scratch.
    let readme = dst.join("README.md");
    let mut r = std::fs::read(&readme).unwrap();
    r.extend_from_slice(b"\nLocal unstaged edit by an author.\n");
    std::fs::write(&readme, r).unwrap();
    std::fs::write(dst.join("gfx/exp01_scratch.tmp"), b"scratch bytes\n").unwrap();
}

// ----- source resolution (the manifest is Setup's own data, copied for tests) -------------

pub fn source_resolution() -> Arc<SourceResolution> {
    let bytes = include_bytes!("../fixtures/source/hoi4-mod-setup.manifest.json");
    Arc::new(SourceResolution {
        identity: SourceIdentity {
            repository: format!("{SOURCE_OWNER}/{SOURCE_NAME}"),
            mode: SourceMode::Latest,
            resolved_revision: "a".repeat(40),
            requested_ref: None,
            release: None,
            manifest_sha256: sha256_bytes(bytes),
            manifest_origin: "remote".into(),
        },
        manifest: parse_manifest(bytes, None).unwrap(),
        manifest_bytes: bytes.to_vec(),
        branch: Some("main".into()),
    })
}

pub fn native_endpoint() -> &'static str {
    native::LOCAL_ENDPOINT
}
