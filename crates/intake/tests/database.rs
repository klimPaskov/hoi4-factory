//! PostgreSQL behaviour: migrations, row-level security, the service records.
//!
//! Needs a PostgreSQL server reachable with a SUPERUSER URL in `FACTORY_TEST_ADMIN_URL`
//! (or `DATABASE_URL`), the same convention as the accounts crate. Every test creates its own
//! database, migrates it as a non-superuser owner role and then talks to it as a
//! non-superuser application role that is a member of `factory_app`, as in production.

mod common;

use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use common::fake_github::*;
use common::*;
use factory_intake::{
    FileTypePolicy, GithubConfig, GithubEndpoints, GithubImportRequest, GithubImporter,
    HttpGithubApi, InstallationRef, IntakeError, IntakeLimits, IntakeService, IntakeStore,
    LinkState, StaticSecrets, UploadIntake, UploadKind, UploadMeta, UploadOptions,
};
use factory_workspace::StorageRoot;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool, Row};
use uuid::Uuid;

const OWNER_ROLE: &str = "hf_intake_owner";
const APP_ROLE: &str = "hf_intake_app";

fn admin_options() -> PgConnectOptions {
    let url = std::env::var("FACTORY_TEST_ADMIN_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set FACTORY_TEST_ADMIN_URL (a superuser URL) to run the database tests");
    PgConnectOptions::from_str(&url).expect("valid PostgreSQL URL")
}

struct Db {
    name: String,
    admin: PgPool,
    owner: PgPool,
    app: PgPool,
}

async fn migrator() -> sqlx::migrate::Migrator {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    sqlx::migrate::Migrator::new(dir)
        .await
        .expect("load migrations")
}

impl Db {
    async fn new() -> Db {
        let mut conn = PgConnection::connect_with(&admin_options())
            .await
            .expect("connect as superuser");
        let is_super: bool =
            sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert!(is_super, "the admin URL must be a superuser");
        sqlx::query("SELECT pg_advisory_lock(7302)")
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
        sqlx::query("SELECT pg_advisory_unlock(7302)")
            .execute(&mut conn)
            .await
            .unwrap();
        let name = format!("intake_test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {name} OWNER {OWNER_ROLE}"))
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.ok();

        let pool = |user: Option<&str>, database: &str| {
            let mut options = admin_options().database(database);
            if let Some(user) = user {
                options = options.username(user);
            }
            PgPoolOptions::new()
                .max_connections(5)
                .connect_lazy_with(options)
        };
        let db = Db {
            admin: pool(None, &name),
            owner: pool(Some(OWNER_ROLE), &name),
            app: pool(Some(APP_ROLE), &name),
            name,
        };
        migrator()
            .await
            .run(&db.owner)
            .await
            .expect("migrations apply as a non-superuser owner");
        db
    }

    async fn finish(self) {
        self.app.close().await;
        self.owner.close().await;
        self.admin.close().await;
        let mut conn = PgConnection::connect_with(&admin_options()).await.unwrap();
        sqlx::query(&format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.name
        ))
        .execute(&mut conn)
        .await
        .ok();
    }

    async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.admin)
            .await
            .unwrap()
    }
}

// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn migrations_are_reversible_and_stay_in_their_range() {
    let db = Db::new().await;
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.admin)
            .await
            .unwrap();
    assert_eq!(versions, vec![300, 301, 302]);
    assert!(
        versions
            .iter()
            .all(|v| factory_intake::MIGRATION_VERSIONS.contains(&(*v as u32)))
    );

    for table in [
        "intake_uploads",
        "repository_links",
        "github_repository_allowlist",
    ] {
        let flags = sqlx::query(
            "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = $1",
        )
        .bind(table)
        .fetch_one(&db.admin)
        .await
        .unwrap();
        assert!(
            flags.get::<bool, _>(0) && flags.get::<bool, _>(1),
            "{table}: RLS enabled and forced"
        );
        // The tenant and project columns are indexed (no foreign keys yet).
        let indexed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_indexes WHERE tablename = $1 AND indexdef ~ 'account_id'",
        )
        .bind(table)
        .fetch_one(&db.admin)
        .await
        .unwrap();
        assert!(indexed >= 1, "{table}: account_id is indexed");
    }
    // timestamptz everywhere.
    let naive = db
        .count("SELECT count(*) FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('intake_uploads','repository_links','github_repository_allowlist') AND data_type = 'timestamp without time zone'")
        .await;
    assert_eq!(naive, 0);

    // Down all the way, then up again.
    migrator()
        .await
        .undo(&db.owner, 0)
        .await
        .expect("down migrations apply");
    assert_eq!(
        db.count("SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('intake_uploads','repository_links','github_repository_allowlist')")
            .await,
        0
    );
    migrator()
        .await
        .run(&db.owner)
        .await
        .expect("migrations apply again");
    assert_eq!(db.count("SELECT count(*) FROM intake_uploads").await, 0);
    db.finish().await;
}

#[tokio::test]
async fn row_level_security_isolates_accounts_and_fails_closed() {
    let db = Db::new().await;
    let store = IntakeStore::new(db.app.clone());
    let (a, b, project) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (upload, link) = (Uuid::new_v4(), Uuid::new_v4());

    store
        .begin_upload(a, project, upload, UploadKind::Zip, Some("a.zip"))
        .await
        .unwrap();
    store
        .allow_repository(a, project, "Owner/Repo")
        .await
        .unwrap();
    store
        .insert_pending_link(a, project, link, "proj-x", "Owner/Repo", "installations/1")
        .await
        .unwrap();

    // Account B sees nothing of account A, through every accessor.
    assert!(store.get_upload(b, upload).await.unwrap().is_none());
    assert!(store.get_link(b, link).await.unwrap().is_none());
    assert!(
        store
            .links_for_project(b, project)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.allowlist(b, project).await.unwrap().names().count(),
        0
    );
    assert!(
        !store
            .set_link_state(b, link, LinkState::Disconnected, None)
            .await
            .unwrap(),
        "B cannot update A's link"
    );
    assert!(
        !store
            .disallow_repository(b, project, "owner/repo")
            .await
            .unwrap()
    );
    // And A still sees its own rows, unchanged.
    assert_eq!(
        store.get_link(a, link).await.unwrap().unwrap().state,
        LinkState::Pending
    );
    assert!(
        store
            .allowlist(a, project)
            .await
            .unwrap()
            .contains("owner/repo")
    );
    assert!(store.get_upload(a, upload).await.unwrap().is_some());

    // Fail closed without the setting: the application role sees no rows and cannot insert.
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM intake_uploads")
        .fetch_one(&db.app)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    let unscoped = sqlx::query(
        "INSERT INTO intake_uploads (id, account_id, project_id, kind, status) VALUES ($1, $2, $3, 'zip', 'receiving')",
    )
    .bind(Uuid::new_v4())
    .bind(a)
    .bind(project)
    .execute(&db.app)
    .await;
    assert!(
        unscoped.is_err(),
        "an unscoped insert must be refused by row-level security"
    );

    // A row for another account cannot be written even with a valid scope.
    let mut tx = db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(a.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let forged = sqlx::query(
        "INSERT INTO intake_uploads (id, account_id, project_id, kind, status) VALUES ($1, $2, $3, 'zip', 'receiving')",
    )
    .bind(Uuid::new_v4())
    .bind(b)
    .bind(project)
    .execute(&mut *tx)
    .await;
    assert!(
        forged.is_err(),
        "writing a row for another account must be refused"
    );
    drop(tx);

    // The application role cannot hard-delete uploads or links.
    let mut tx = db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(a.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM intake_uploads")
            .execute(&mut *tx)
            .await
            .is_err()
    );
    drop(tx);
    db.finish().await;
}

#[tokio::test]
async fn constraints_keep_credentials_and_duplicate_links_out() {
    let db = Db::new().await;
    let store = IntakeStore::new(db.app.clone());
    let (account, project) = (Uuid::new_v4(), Uuid::new_v4());
    for bad in [
        "ghs_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
        "ghp_abc",
        "github_pat_abc",
        "has space",
        "",
    ] {
        let result = store
            .insert_pending_link(
                account,
                project,
                Uuid::new_v4(),
                "proj-x",
                "Owner/Repo",
                bad,
            )
            .await;
        assert!(
            result.is_err(),
            "installation_ref {bad:?} must be refused by the database"
        );
    }
    store
        .insert_pending_link(
            account,
            project,
            Uuid::new_v4(),
            "proj-x",
            "Owner/Repo",
            "installations/1",
        )
        .await
        .unwrap();
    let second = store
        .insert_pending_link(
            account,
            project,
            Uuid::new_v4(),
            "proj-x",
            "Owner/Other",
            "installations/1",
        )
        .await;
    assert!(second.is_err(), "one live link per project");
    db.finish().await;
}

// ---------------------------------------------------------------------------------------------
// The service

struct Svc {
    _tmp: tempfile::TempDir,
    storage: StorageRoot,
    service: IntakeService,
    db: Db,
    fake: FakeGithub,
    remote: RemoteFixture,
    account: Uuid,
    project: Uuid,
}

async fn service() -> Svc {
    let db = Db::new().await;
    let tmp = tempfile::tempdir().unwrap();
    let git_root = tmp.path().join("remote");
    fs::create_dir_all(&git_root).unwrap();
    let remote = make_remote(&git_root, "klimPaskov", "hoi4-factory-test-mod");
    let fake = FakeGithub::start(
        &git_root,
        vec![FakeRepo::new("klimPaskov/hoi4-factory-test-mod", 77)],
    )
    .await;
    let storage = StorageRoot::new(tmp.path().join("data"));
    let endpoints = GithubEndpoints::custom(&fake.api_base(), &fake.clone_base()).unwrap();
    let secrets = StaticSecrets::new()
        .with("github_app.id", APP_ID)
        .with("github_app.private_key", &test_key().private_pem)
        .with("installations/test", &INSTALLATION_ID.to_string());
    let github = Arc::new(GithubImporter::new(
        Arc::new(HttpGithubApi::new(&endpoints).unwrap()),
        Arc::new(secrets),
        storage.clone(),
        GithubConfig::new(endpoints),
        IntakeLimits::default(),
        FileTypePolicy::default(),
    ));
    let uploads = Arc::new(UploadIntake::new(
        storage.clone(),
        IntakeLimits::default(),
        FileTypePolicy::default(),
    ));
    let service = IntakeService::new(IntakeStore::new(db.app.clone()), uploads, github);
    Svc {
        _tmp: tmp,
        storage,
        service,
        db,
        fake,
        remote,
        account: Uuid::new_v4(),
        project: Uuid::new_v4(),
    }
}

impl Svc {
    fn project_dir(&self) -> std::path::PathBuf {
        self.storage
            .data_root()
            .join("workspaces")
            .join(self.account.to_string())
            .join(self.project.to_string())
    }

    fn import_request(&self, link: Uuid) -> GithubImportRequest {
        GithubImportRequest {
            account: self.account,
            project: self.project,
            link_id: link,
            installation: InstallationRef::new("installations/test").unwrap(),
            repository: "klimPaskov/hoi4-factory-test-mod".into(),
            branch: None,
        }
    }

    async fn allow(&self) {
        self.service
            .store()
            .allow_repository(
                self.account,
                self.project,
                "klimPaskov/hoi4-factory-test-mod",
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn uploads_are_recorded_as_accepted_or_rejected() {
    let svc = service().await;
    let archive = build_zip(&[ZEntry::file("descriptor.mod", b"name=\"x\"\r\n".to_vec())]);
    let meta = UploadMeta {
        upload_id: Uuid::new_v4(),
        original_name: Some("my mod.zip".into()),
    };
    let outcome = svc
        .service
        .upload_zip(
            svc.account,
            svc.project,
            meta.clone(),
            Box::new(std::io::Cursor::new(archive.clone())),
            UploadOptions::default(),
        )
        .await
        .unwrap();
    let record = svc
        .service
        .store()
        .get_upload(svc.account, meta.upload_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, "accepted");
    assert_eq!(record.kind, "zip");
    assert_eq!(record.original_name.as_deref(), Some("my mod.zip"));
    assert_eq!(
        record.original_sha256.as_deref(),
        Some(sha256_hex(&archive).as_str())
    );
    assert_eq!(record.original_bytes, Some(archive.len() as i64));
    assert_eq!(record.git_state.as_deref(), Some("not_initialised"));
    assert_eq!(
        record.inventory_hash.as_deref(),
        Some(outcome.inventory.content_hash.as_str())
    );
    assert!(record.completed_at.is_some());
    assert!(
        !record
            .original_locator
            .unwrap()
            .contains(&*svc.storage.data_root().to_string_lossy())
    );

    // A hostile upload into another project is rejected, recorded with its code and hash.
    let hostile = build_zip(&[ZEntry::file("../evil.txt", b"x".to_vec())]);
    let project2 = Uuid::new_v4();
    let meta2 = UploadMeta {
        upload_id: Uuid::new_v4(),
        original_name: None,
    };
    let failure = svc
        .service
        .upload_zip(
            svc.account,
            project2,
            meta2.clone(),
            Box::new(std::io::Cursor::new(hostile.clone())),
            UploadOptions::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code(), "path_rejected");
    let record = svc
        .service
        .store()
        .get_upload(svc.account, meta2.upload_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, "rejected");
    assert_eq!(record.rejection_code.as_deref(), Some("path_rejected"));
    assert_eq!(
        record.original_sha256.as_deref(),
        Some(sha256_hex(&hostile).as_str())
    );
    assert!(record.completed_at.is_some());
    assert!(
        !svc.storage
            .data_root()
            .join("workspaces")
            .join(svc.account.to_string())
            .join(project2.to_string())
            .exists()
    );

    // Another account cannot see either record.
    assert!(
        svc.service
            .store()
            .get_upload(Uuid::new_v4(), meta.upload_id)
            .await
            .unwrap()
            .is_none()
    );
    svc.db.finish().await;
}

#[tokio::test]
async fn a_github_import_records_the_c10_link() {
    let svc = service().await;
    svc.allow().await;
    let link_id = Uuid::new_v4();
    let (link, outcome) = svc
        .service
        .import_github(svc.import_request(link_id))
        .await
        .unwrap();

    assert_eq!(link.id, link_id);
    assert_eq!(link.state, LinkState::Active);
    assert_eq!(link.provider, "github");
    assert_eq!(
        link.repository_full_name,
        "klimPaskov/hoi4-factory-test-mod"
    );
    assert_eq!(link.repository_node_id.as_deref(), Some("R_node77"));
    assert_eq!(link.repository_id, Some(77));
    assert_eq!(
        link.installation_ref, "installations/test",
        "a secret-store key name, never a token"
    );
    assert_eq!(link.selected_scope, "selected_repositories");
    assert_eq!(link.default_branch.as_deref(), Some("main"));
    assert_eq!(link.imported_branch.as_deref(), Some("main"));
    assert_eq!(
        link.imported_commit.as_deref(),
        Some(svc.remote.main_tip.as_str())
    );
    assert_eq!(link.workspace_id, svc.project.to_string());
    assert!(link.observed_at.is_some());
    let names: Vec<(&str, &str)> = link
        .observed_refs
        .iter()
        .map(|r| (r.name.as_str(), r.sha.as_str()))
        .collect();
    assert!(names.contains(&("refs/heads/main", svc.remote.main_tip.as_str())));
    assert!(names.contains(&("refs/heads/feature", svc.remote.feature_tip.as_str())));
    assert_eq!(outcome.head.to_string(), svc.remote.main_tip);
    assert_eq!(outcome.dirty_paths, 0);

    // No credential is stored anywhere in the database.
    let tokens: Vec<String> = svc.fake.with(|s| s.tokens.keys().cloned().collect());
    for table in [
        "repository_links",
        "intake_uploads",
        "github_repository_allowlist",
    ] {
        let text: String = sqlx::query_scalar(&format!(
            "SELECT coalesce(string_agg(t::text, ' '), '') FROM {table} t"
        ))
        .fetch_one(&svc.db.admin)
        .await
        .unwrap();
        for token in &tokens {
            assert!(!text.contains(token), "{table} holds a token");
        }
    }

    // A second link for the same project is refused while one is live.
    let again = svc
        .service
        .import_github(svc.import_request(Uuid::new_v4()))
        .await
        .unwrap_err();
    assert_eq!(again.code(), "invalid_request");
    svc.db.finish().await;
}

#[tokio::test]
async fn revocation_after_import_marks_the_link_and_keeps_the_workspace() {
    let svc = service().await;
    svc.allow().await;
    let link_id = Uuid::new_v4();
    svc.service
        .import_github(svc.import_request(link_id))
        .await
        .unwrap();
    let before = fs::read(svc.project_dir().join("source/notes.txt")).unwrap();

    // Still connected: the check changes nothing.
    assert_eq!(
        svc.service
            .check_link(svc.account, link_id)
            .await
            .unwrap()
            .state,
        LinkState::Active
    );

    // A rate limit is not a revocation.
    svc.fake.with(|s| s.rate_limit_installation = true);
    let err = svc
        .service
        .check_link(svc.account, link_id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "github_rate_limited");
    svc.fake.with(|s| s.rate_limit_installation = false);
    assert_eq!(
        svc.service
            .check_link(svc.account, link_id)
            .await
            .unwrap()
            .state,
        LinkState::Active
    );

    // The installation is deleted.
    svc.fake.with(|s| s.revoked = true);
    let link = svc.service.check_link(svc.account, link_id).await.unwrap();
    assert_eq!(link.state, LinkState::AccessRevoked);
    assert_eq!(
        link.state_reason.as_deref(),
        Some("installation_or_repository_gone")
    );
    assert_eq!(
        fs::read(svc.project_dir().join("source/notes.txt")).unwrap(),
        before,
        "the workspace is untouched"
    );
    assert!(svc.project_dir().join("source/.git").is_dir());
    svc.db.finish().await;
}

#[tokio::test]
async fn a_revoked_installation_during_import_leaves_an_access_revoked_link_and_no_workspace() {
    let svc = service().await;
    svc.allow().await;
    svc.fake.with(|s| s.revoked = true);
    let link_id = Uuid::new_v4();
    let err = svc
        .service
        .import_github(svc.import_request(link_id))
        .await
        .unwrap_err();
    assert!(matches!(err, IntakeError::AccessRevoked));
    let link = svc
        .service
        .store()
        .get_link(svc.account, link_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.state, LinkState::AccessRevoked);
    assert!(!svc.project_dir().exists());

    // A rejected token mid-clone ends the same way and discards the partial clone.
    let svc2 = service().await;
    svc2.allow().await;
    svc2.fake.with(|s| s.git_reject_after = Some(2));
    let link_id = Uuid::new_v4();
    let err = svc2
        .service
        .import_github(svc2.import_request(link_id))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "access_revoked");
    let link = svc2
        .service
        .store()
        .get_link(svc2.account, link_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.state, LinkState::AccessRevoked);
    assert!(!svc2.project_dir().exists());
    svc.db.finish().await;
    svc2.db.finish().await;
}

#[tokio::test]
async fn refusals_before_the_network_leave_no_link_and_selection_changes_revoke() {
    let svc = service().await;
    // Not on the allowlist: refused before any row or network call.
    let err = svc
        .service
        .import_github(svc.import_request(Uuid::new_v4()))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "repository_not_allowed");
    assert!(
        svc.service
            .store()
            .links_for_project(svc.account, svc.project)
            .await
            .unwrap()
            .is_empty()
    );
    svc.fake.with(|s| assert_eq!(s.api_calls, 0));

    // An installation switched to all repositories is refused and the link marked.
    svc.allow().await;
    svc.fake.with(|s| s.selection = "all".into());
    let link_id = Uuid::new_v4();
    let err = svc
        .service
        .import_github(svc.import_request(link_id))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "installation_not_selected");
    let link = svc
        .service
        .store()
        .get_link(svc.account, link_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.state, LinkState::AccessRevoked);
    assert_eq!(
        link.state_reason.as_deref(),
        Some("installation_not_selected")
    );
    assert!(!svc.project_dir().exists());

    // The failed link no longer blocks a new attempt once the installation is fixed.
    svc.fake.with(|s| s.selection = "selected".into());
    let (link, _) = svc
        .service
        .import_github(svc.import_request(Uuid::new_v4()))
        .await
        .unwrap();
    assert_eq!(link.state, LinkState::Active);
    svc.db.finish().await;
}

#[tokio::test]
async fn listing_through_the_service_applies_the_stored_allowlist() {
    let svc = service().await;
    let installation = InstallationRef::new("installations/test").unwrap();
    let listed = svc
        .service
        .list_github_repositories(svc.account, svc.project, &installation)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].allowed);
    svc.allow().await;
    let listed = svc
        .service
        .list_github_repositories(svc.account, svc.project, &installation)
        .await
        .unwrap();
    assert!(listed[0].allowed);
    svc.db.finish().await;
}
