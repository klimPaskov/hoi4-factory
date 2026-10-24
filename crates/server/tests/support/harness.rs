// Shared harness of the server's HTTP tests: a disposable database, the assembled application
// with a test GitHub, signed-in clients and multipart bodies. Included (not a module) by
// `intake_api.rs` and `setup_gates.rs`, each after `#[path = "../../intake/tests/common/mod.rs"]
// mod common;`, so both see the same private items.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, header};
use common::fake_github::{
    APP_ID, FakeGithub, FakeRepo, INSTALLATION_ID, RemoteFixture, make_remote, test_key,
};
use common::{ZEntry, build_zip, sha256_hex};
use factory_accounts::config::Argon2Params;
use factory_accounts::{
    AccountsDeps, Clock, InMemoryRateLimiter, MemoryMailer, OutgoingMail, Rule, SystemClock,
};
use factory_intake::GithubError;
use factory_server::github::{
    AppInstallation, AppInstallationAccount, AppInstallations, HttpAppInstallations,
};
use factory_server::{Overrides, ServerConfig, ServerState, build_app, initialize_with};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

const OWNER_ROLE: &str = "hf_srv_owner";
const APP_ROLE: &str = "hf_srv_app";
const PASSWORD: &str = "correct horse battery";
const REPO: &str = "klimPaskov/hoi4-factory-test-mod";
const INSTALL_URL: &str = "https://github.com/apps/hoi4-factory-dev/installations/new";

fn admin_url() -> url::Url {
    let raw = std::env::var("FACTORY_TEST_ADMIN_URL").expect(
        "set FACTORY_TEST_ADMIN_URL (a superuser URL of a disposable PostgreSQL) to run these tests",
    );
    url::Url::parse(&raw).expect("valid PostgreSQL URL")
}

fn url_for(role: &str, database: &str) -> String {
    let mut url = admin_url();
    url.set_username(role).unwrap();
    url.set_path(&format!("/{database}"));
    url.to_string()
}

fn admin_url_for(database: &str) -> String {
    let mut url = admin_url();
    url.set_path(&format!("/{database}"));
    url.to_string()
}

static ROLES: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn ensure_roles(admin: &PgPool) {
    ROLES
        .get_or_init(|| async {
            let mut conn = admin.acquire().await.unwrap();
            sqlx::query("SELECT pg_advisory_lock(7102)")
                .execute(&mut *conn)
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
                        .fetch_one(&mut *conn)
                        .await
                        .unwrap();
                if !exists {
                    sqlx::query(&format!("CREATE ROLE {role} {extra}"))
                        .execute(&mut *conn)
                        .await
                        .unwrap();
                }
            }
            sqlx::query(&format!("GRANT factory_app TO {APP_ROLE}"))
                .execute(&mut *conn)
                .await
                .unwrap();
            sqlx::query("SELECT pg_advisory_unlock(7102)")
                .execute(&mut *conn)
                .await
                .unwrap();
        })
        .await;
}

async fn create_database(name: &str) {
    let admin = PgPool::connect(admin_url().as_str()).await.unwrap();
    ensure_roles(&admin).await;
    sqlx::query(&format!("CREATE DATABASE \"{name}\" OWNER {OWNER_ROLE}"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

fn fixture_game_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("launcher-settings.json"),
        r#"{"version":"1.19.3","rawVersion":"1.19.3.0","gameId":"hoi4"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("hoi4_rev.txt"), "c01a\n").unwrap();
    std::fs::write(dir.path().join("checksum_manifest.txt"), "abc\n").unwrap();
    std::fs::create_dir_all(dir.path().join("dlc").join("dlc001_sky_high")).unwrap();
    dir
}

/// A GitHub App installation list that tests edit.
struct FixedInstallations {
    list: Mutex<Vec<AppInstallation>>,
}

#[async_trait]
impl AppInstallations for FixedInstallations {
    async fn list(&self, jwt: &str) -> Result<Vec<AppInstallation>, GithubError> {
        // Same bar as GitHub: an RS256 JWT of the test App.
        assert_eq!(jwt.split('.').count(), 3, "an App JWT is sent");
        Ok(self.list.lock().unwrap().clone())
    }
}

fn installation(github_user_id: u64, selection: &str) -> AppInstallation {
    AppInstallation {
        id: INSTALLATION_ID,
        repository_selection: selection.to_owned(),
        suspended_at: None,
        account: AppInstallationAccount {
            id: github_user_id,
            login: "alice-gh".to_owned(),
        },
    }
}

struct GithubFixture {
    fake: FakeGithub,
    remote: RemoteFixture,
    key_file: PathBuf,
    _dir: TempDir,
}

async fn github_fixture() -> GithubFixture {
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("remote");
    std::fs::create_dir_all(&git_root).unwrap();
    let remote = make_remote(&git_root, "klimPaskov", "hoi4-factory-test-mod");
    let fake = FakeGithub::start(
        &git_root,
        vec![
            FakeRepo::new(REPO, 1001),
            FakeRepo::new("klimPaskov/other-repo", 1002),
        ],
    )
    .await;
    let key_file = dir.path().join("app-key.pem");
    std::fs::write(&key_file, &test_key().private_pem).unwrap();
    GithubFixture {
        fake,
        remote,
        key_file,
        _dir: dir,
    }
}

#[derive(Default)]
struct Options {
    github: Option<GithubFixture>,
    upload_max_bytes: Option<usize>,
    anthropic_key: Option<&'static str>,
    /// Replaces the model client of Setup analysis (the route still comes from the configuration).
    model: Option<Arc<dyn factory_setup_adapter::ModelClient>>,
    /// Replaces where Setup fetches its workflow package from.
    setup_source: Option<Arc<dyn factory_setup_adapter::setup::SourceFetcher>>,
}

struct Harness {
    state: ServerState,
    app: Router,
    mailer: Arc<MemoryMailer>,
    installations: Arc<FixedInstallations>,
    game_dir: TempDir,
    data_dir: TempDir,
    database: String,
    github: Option<GithubFixture>,
    vars: BTreeMap<&'static str, String>,
    model: Option<Arc<dyn factory_setup_adapter::ModelClient>>,
    setup_source: Option<Arc<dyn factory_setup_adapter::setup::SourceFetcher>>,
}

fn test_config(vars: &BTreeMap<&'static str, String>) -> ServerConfig {
    let mut config = ServerConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
    config.accounts.argon2 = Argon2Params {
        memory_kib: 8,
        iterations: 1,
        parallelism: 1,
    };
    for rule in config.accounts.rate_limits.rules.values_mut() {
        *rule = Rule {
            capacity: 10_000,
            refill_every: Duration::from_secs(1),
        };
    }
    config
}

impl Harness {
    async fn start(options: Options) -> Self {
        let database = format!("hf_i_{}", Uuid::new_v4().simple());
        create_database(&database).await;
        let game_dir = fixture_game_dir();
        let data_dir = tempfile::tempdir().unwrap();

        let mut vars: BTreeMap<&'static str, String> = BTreeMap::new();
        vars.insert("DATABASE_URL", url_for(APP_ROLE, &database));
        vars.insert("DATABASE_MIGRATION_URL", url_for(OWNER_ROLE, &database));
        vars.insert(
            "FACTORY_DATA_ROOT",
            data_dir.path().to_string_lossy().into_owned(),
        );
        vars.insert(
            "FACTORY_GAME_REF",
            game_dir.path().to_string_lossy().into_owned(),
        );
        vars.insert("FACTORY_ENV", "development".into());
        vars.insert("FACTORY_PUBLIC_URL", "http://localhost:8080".into());
        vars.insert("FACTORY_ALLOWED_ORIGINS", "http://localhost:5173".into());
        // Setup registers a Node runtime in generated client configuration on Linux, and accepts
        // only the distribution's own /usr/bin/node. Where there is none (Windows hosts) the
        // tests run without one, and Setup then plans for its own platform.
        if cfg!(unix) && std::path::Path::new("/usr/bin/node").exists() {
            vars.insert("FACTORY_NODE_EXECUTABLE", "/usr/bin/node".into());
        }
        if let Some(key) = options.anthropic_key {
            vars.insert("FACTORY_ANTHROPIC_API_KEY", key.to_owned());
        }
        if let Some(limit) = options.upload_max_bytes {
            vars.insert("FACTORY_UPLOAD_MAX_BYTES", limit.to_string());
        }
        if let Some(gh) = &options.github {
            vars.insert("FACTORY_GITHUB_APP_ID", APP_ID.to_owned());
            vars.insert(
                "FACTORY_GITHUB_APP_PRIVATE_KEY_FILE",
                gh.key_file.to_string_lossy().into_owned(),
            );
            vars.insert("FACTORY_GITHUB_APP_API_BASE", gh.fake.api_base());
            vars.insert("FACTORY_GITHUB_CLONE_BASE", gh.fake.clone_base());
        }
        let config = test_config(&vars);
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let mailer = Arc::new(MemoryMailer::new());
        let deps = AccountsDeps {
            mailer: mailer.clone(),
            github: None,
            limiter: Arc::new(InMemoryRateLimiter::new(
                clock.clone(),
                &config.accounts.rate_limits,
            )),
        };
        let installations = Arc::new(FixedInstallations {
            list: Mutex::new(Vec::new()),
        });
        let state = initialize_with(
            config,
            Overrides {
                accounts_deps: Some(deps),
                clock: Some(clock),
                app_installations: Some(installations.clone()),
                model_client: options.model.clone(),
                setup_source: options.setup_source.clone(),
            },
        )
        .await
        .expect("the server starts");
        let app = build_app(state.clone());
        Self {
            state,
            app,
            mailer,
            installations,
            game_dir,
            data_dir,
            database,
            github: options.github,
            vars,
            model: options.model,
            setup_source: options.setup_source,
        }
    }

    /// A new server process over the same database and data directory: nothing in memory
    /// survives, the way a restart behaves. Clients made before keep talking to the old router;
    /// use [`Harness::rebind`] for a signed-in client of the new one.
    async fn restart(&mut self) {
        let config = test_config(&self.vars);
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let deps = AccountsDeps {
            mailer: self.mailer.clone(),
            github: None,
            limiter: Arc::new(InMemoryRateLimiter::new(
                clock.clone(),
                &config.accounts.rate_limits,
            )),
        };
        let state = initialize_with(
            config,
            Overrides {
                accounts_deps: Some(deps),
                clock: Some(clock),
                app_installations: Some(self.installations.clone()),
                model_client: self.model.clone(),
                setup_source: self.setup_source.clone(),
            },
        )
        .await
        .expect("the server restarts");
        self.app = build_app(state.clone());
        self.state = state;
    }

    /// The same signed-in user (same session cookie) talking to the current router.
    fn rebind(&self, user: &User) -> User {
        User {
            client: Client {
                router: self.app.clone(),
                jar: Mutex::new(user.client.jar.lock().unwrap().clone()),
            },
            id: user.id.clone(),
        }
    }

    fn client(&self) -> Client {
        Client {
            router: self.app.clone(),
            jar: Mutex::new(BTreeMap::new()),
        }
    }

    async fn admin(&self) -> PgPool {
        PgPool::connect(&admin_url_for(&self.database))
            .await
            .unwrap()
    }

    async fn verified_user(&self, email: &str) -> User {
        let client = self.client();
        let resp = client
            .post(
                "/api/v1/auth/signup",
                json!({"email": email, "password": PASSWORD}),
            )
            .await;
        assert_eq!(resp.status, 201, "{}", resp.text);
        let id = resp.body["account"]["id"].as_str().unwrap().to_owned();
        let mail = self.mailer.last_to(email).expect("verification mail sent");
        let resp = client
            .post(
                "/api/v1/auth/verify-email",
                json!({ "token": token_from_mail(&mail) }),
            )
            .await;
        assert_eq!(resp.status, 200, "{}", resp.text);
        User { client, id }
    }

    /// Marks the account as signed in with GitHub account `github_id`.
    async fn link_github(&self, user: &User, github_id: i64) {
        let admin = self.admin().await;
        sqlx::query("UPDATE accounts SET github_user_id = $1 WHERE id = $2::uuid")
            .bind(github_id)
            .bind(&user.id)
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }

    async fn project(&self, user: &User, name: &str) -> String {
        let resp = user
            .client
            .post("/api/v1/projects", json!({ "name": name }))
            .await;
        assert_eq!(resp.status, 201, "{}", resp.text);
        resp.body["project"]["id"].as_str().unwrap().to_owned()
    }

    fn workspace_source(&self, user: &User, project: &str) -> PathBuf {
        self.data_dir
            .path()
            .join("workspaces")
            .join(&user.id)
            .join(project)
            .join("source")
    }
}

struct User {
    client: Client,
    id: String,
}

fn token_from_mail(mail: &OutgoingMail) -> String {
    mail.text
        .split("token=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("mail carries a token link")
        .to_owned()
}

struct Resp {
    status: u16,
    body: Value,
    text: String,
}

impl Resp {
    fn error_code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }
}

struct Client {
    router: Router,
    jar: Mutex<BTreeMap<String, String>>,
}

impl Client {
    async fn send_raw(
        &self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: Vec<u8>,
        extra: &[(&str, &str)],
    ) -> Resp {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::USER_AGENT, "integration-test");
        let cookies: Vec<String> = self
            .jar
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        if !cookies.is_empty() {
            builder = builder.header(header::COOKIE, cookies.join("; "));
        }
        if let Some(ct) = content_type {
            builder = builder.header(header::CONTENT_TYPE, ct);
        }
        for (k, v) in extra {
            builder = builder.header(*k, *v);
        }
        let request = builder.body(Body::from(body)).unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        for cookie in headers.get_all(header::SET_COOKIE) {
            let first = cookie.to_str().unwrap().split(';').next().unwrap();
            let (name, value) = first.split_once('=').unwrap();
            self.jar
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_owned());
        }
        Resp { status, body, text }
    }

    async fn get(&self, path: &str) -> Resp {
        self.send_raw("GET", path, None, Vec::new(), &[]).await
    }

    async fn post(&self, path: &str, body: Value) -> Resp {
        self.send_raw(
            "POST",
            path,
            Some("application/json"),
            body.to_string().into_bytes(),
            &[],
        )
        .await
    }

    async fn upload(&self, project: &str, parts: &[Part]) -> Resp {
        self.upload_with(project, parts, &[]).await
    }

    async fn upload_with(&self, project: &str, parts: &[Part], extra: &[(&str, &str)]) -> Resp {
        let (content_type, body) = multipart(parts);
        self.send_raw(
            "POST",
            &format!("/api/v1/projects/{project}/uploads"),
            Some(&content_type),
            body,
            extra,
        )
        .await
    }
}

// ----- multipart bodies ----------------------------------------------------------------------

struct Part {
    name: &'static str,
    filename: Option<String>,
    data: Vec<u8>,
}

impl Part {
    fn text(name: &'static str, value: &str) -> Part {
        Part {
            name,
            filename: None,
            data: value.as_bytes().to_vec(),
        }
    }

    fn file(path: &str, data: impl Into<Vec<u8>>) -> Part {
        Part {
            name: "files",
            filename: Some(path.to_owned()),
            data: data.into(),
        }
    }
}

fn multipart(parts: &[Part]) -> (String, Vec<u8>) {
    let boundary = "----hf-test-boundary-7f3a";
    let mut body = Vec::new();
    for part in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match &part.filename {
            Some(f) => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                    part.name
                )
                .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                    part.name
                )
                .as_bytes(),
            ),
        }
        body.extend_from_slice(&part.data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

