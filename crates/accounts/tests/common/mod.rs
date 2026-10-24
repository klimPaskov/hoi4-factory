//! Shared harness for the integration tests.
//!
//! Needs a PostgreSQL server reachable with a SUPERUSER URL in `FACTORY_TEST_ADMIN_URL` (or
//! `DATABASE_URL`). Each test creates its own database, migrates it as a non-superuser owner
//! role (`hf_test_owner`) and then talks to it as the non-superuser application role
//! (`hf_test_app`, a member of `factory_app`), exactly like production. The superuser pool is
//! used only to inspect or modify ground truth.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, header};
use factory_accounts::{
    AccountsConfig, AccountsDeps, AccountsState, Database, Environment, GitHubClient,
    GitHubOAuthConfig, HttpGitHubClient, InMemoryRateLimiter, ManualClock, MemoryMailer,
    OutgoingMail, Rule,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool};
use tower::ServiceExt;
use uuid::Uuid;

pub const OWNER_ROLE: &str = "hf_test_owner";
pub const APP_ROLE: &str = "hf_test_app";
pub const GOOD_PASSWORD: &str = "correct horse battery";

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
            assert!(
                is_super,
                "the admin URL must be a superuser: it creates databases and roles"
            );
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

/// A freshly migrated database with the three connection kinds.
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
    pub async fn create(clock: Arc<ManualClock>) -> Self {
        ensure_roles().await;
        let name = format!("hf_t_{}", Uuid::new_v4().simple());
        let mut conn = PgConnection::connect_with(&admin_options()).await.unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{name}\" OWNER {OWNER_ROLE}"))
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        // Small pools, opened on first use: many tests run in parallel against one server.
        let admin = lazy_pool(admin_options().database(&name), 2);
        let owner = lazy_pool(admin_options().username(OWNER_ROLE).database(&name), 1);
        let app = lazy_pool(admin_options().username(APP_ROLE).database(&name), 4);
        Database::new(owner.clone(), clock)
            .migrate()
            .await
            .expect("migrations apply");
        owner.close().await;
        let owner = lazy_pool(admin_options().username(OWNER_ROLE).database(&name), 1);
        Self {
            name,
            admin,
            owner,
            app,
        }
    }
}

// ----- fake GitHub --------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct FakeUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    /// `None` answers 403 on /user/emails, like an App without the email permission.
    pub emails: Option<Vec<(String, bool, bool)>>,
}

impl FakeUser {
    pub fn verified(id: i64, login: &str, email: &str) -> Self {
        Self {
            id,
            login: login.to_owned(),
            name: Some(format!("{login} (GitHub)")),
            emails: Some(vec![(email.to_owned(), true, true)]),
        }
    }
}

#[derive(Default)]
struct FakeGitHubState {
    codes: HashMap<String, FakeUser>,
    tokens: HashMap<String, FakeUser>,
    issued_tokens: Vec<String>,
    token_requests: usize,
    last_secret_seen: Option<String>,
}

pub const GITHUB_CLIENT_ID: &str = "Iv1.testclient";

pub struct FakeGitHub {
    pub base: String,
    pub client_secret: String,
    state: Arc<Mutex<FakeGitHubState>>,
}

impl FakeGitHub {
    pub async fn start() -> Self {
        use axum::Json;
        use axum::extract::{Form, State};
        use axum::http::StatusCode;
        use axum::routing::{get, post};

        let state: Arc<Mutex<FakeGitHubState>> = Arc::default();
        let client_secret = format!("secret-{}", Uuid::new_v4().simple());
        let secret_for_handler = client_secret.clone();

        type Shared = State<(Arc<Mutex<FakeGitHubState>>, String)>;

        async fn token(
            State((st, secret)): Shared,
            Form(form): Form<HashMap<String, String>>,
        ) -> Json<Value> {
            let mut st = st.lock().unwrap();
            st.token_requests += 1;
            st.last_secret_seen = form.get("client_secret").cloned();
            if form.get("client_id").map(String::as_str) != Some(GITHUB_CLIENT_ID)
                || form.get("client_secret") != Some(&secret)
            {
                return Json(json!({"error": "incorrect_client_credentials"}));
            }
            let Some(user) = form.get("code").and_then(|c| st.codes.remove(c)) else {
                return Json(json!({"error": "bad_verification_code"}));
            };
            let token = format!("ghu_{}", Uuid::new_v4().simple());
            st.tokens.insert(token.clone(), user);
            st.issued_tokens.push(token.clone());
            Json(json!({"access_token": token, "token_type": "bearer"}))
        }

        fn bearer(headers: &HeaderMap) -> Option<String> {
            headers
                .get(header::AUTHORIZATION)?
                .to_str()
                .ok()?
                .strip_prefix("Bearer ")
                .map(str::to_owned)
        }

        async fn user(State((st, _)): Shared, headers: HeaderMap) -> (StatusCode, Json<Value>) {
            let st = st.lock().unwrap();
            match bearer(&headers).and_then(|t| st.tokens.get(&t).cloned()) {
                Some(u) => (
                    StatusCode::OK,
                    Json(json!({"id": u.id, "login": u.login, "name": u.name})),
                ),
                None => (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"message": "Bad credentials"})),
                ),
            }
        }

        async fn emails(State((st, _)): Shared, headers: HeaderMap) -> (StatusCode, Json<Value>) {
            let st = st.lock().unwrap();
            match bearer(&headers).and_then(|t| st.tokens.get(&t).cloned()) {
                None => (StatusCode::UNAUTHORIZED, Json(json!({"message": "Bad credentials"}))),
                Some(u) => match u.emails {
                    None => (StatusCode::FORBIDDEN, Json(json!({"message": "Resource not accessible"}))),
                    Some(list) => (
                        StatusCode::OK,
                        Json(Value::Array(
                            list.into_iter()
                                .map(|(email, primary, verified)| json!({"email": email, "primary": primary, "verified": verified}))
                                .collect(),
                        )),
                    ),
                },
            }
        }

        let app = Router::new()
            .route("/login/oauth/access_token", post(token))
            .route("/user", get(user))
            .route("/user/emails", get(emails))
            .with_state((state.clone(), secret_for_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        Self {
            base: format!("http://{addr}"),
            client_secret,
            state,
        }
    }

    /// Registers a one-time authorization code that exchanges for `user`.
    pub fn grant_code(&self, user: FakeUser) -> String {
        let code = format!("code-{}", Uuid::new_v4().simple());
        self.state.lock().unwrap().codes.insert(code.clone(), user);
        code
    }

    pub fn token_requests(&self) -> usize {
        self.state.lock().unwrap().token_requests
    }

    pub fn issued_tokens(&self) -> Vec<String> {
        self.state.lock().unwrap().issued_tokens.clone()
    }

    pub fn last_secret_seen(&self) -> Option<String> {
        self.state.lock().unwrap().last_secret_seen.clone()
    }
}

// ----- the application under test -----------------------------------------------------------

pub struct TestApp {
    pub db: TestDb,
    pub clock: Arc<ManualClock>,
    pub mailer: Arc<MemoryMailer>,
    pub state: AccountsState,
    pub router: Router,
    pub github: Option<FakeGitHub>,
}

type ConfigTweak = Box<dyn FnOnce(&mut AccountsConfig)>;

#[derive(Default)]
pub struct Builder {
    production: bool,
    github: bool,
    tweak: Option<ConfigTweak>,
}

impl Builder {
    pub fn production(mut self) -> Self {
        self.production = true;
        self
    }

    pub fn github(mut self) -> Self {
        self.github = true;
        self
    }

    pub fn config(mut self, f: impl FnOnce(&mut AccountsConfig) + 'static) -> Self {
        self.tweak = Some(Box::new(f));
        self
    }

    pub async fn build(self) -> TestApp {
        let clock = Arc::new(ManualClock::starting_now());
        let db = TestDb::create(clock.clone()).await;
        let mut config = AccountsConfig::development();
        config.argon2 = factory_accounts::config::Argon2Params {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        };
        config.trust_forwarded_for = true;
        for rule in config.rate_limits.rules.values_mut() {
            *rule = Rule {
                capacity: 10_000,
                refill_every: Duration::from_secs(1),
            };
        }
        if self.production {
            config.environment = Environment::Production;
            config.public_url = "https://factory.test".to_owned();
            config.verify_link_base = "https://factory.test/verify-email".to_owned();
        }
        let fake = if self.github {
            let fake = FakeGitHub::start().await;
            let mut gh = GitHubOAuthConfig::new(
                GITHUB_CLIENT_ID,
                fake.client_secret.clone(),
                &config.public_url,
            );
            gh.authorize_url = format!("{}/login/oauth/authorize", fake.base);
            gh.token_url = format!("{}/login/oauth/access_token", fake.base);
            gh.api_base = fake.base.clone();
            config.github = Some(gh);
            Some(fake)
        } else {
            None
        };
        if let Some(tweak) = self.tweak {
            tweak(&mut config);
        }
        let database = Database::new(db.app.clone(), clock.clone());
        let mailer = Arc::new(MemoryMailer::new());
        let github: Option<Arc<dyn GitHubClient>> = match &config.github {
            Some(gh) => Some(Arc::new(HttpGitHubClient::new(gh.clone()).unwrap())),
            None => None,
        };
        let deps = AccountsDeps {
            mailer: mailer.clone(),
            github,
            limiter: Arc::new(InMemoryRateLimiter::new(clock.clone(), &config.rate_limits)),
        };
        let state = AccountsState::new(database, config, deps).unwrap();
        let router = Router::new().nest("/api/v1", factory_accounts::router(state.clone()));
        TestApp {
            db,
            clock,
            mailer,
            state,
            router,
            github: fake,
        }
    }
}

impl TestApp {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub async fn new() -> Self {
        Builder::default().build().await
    }

    /// A browser: its own cookie jar and source address.
    pub fn client(&self) -> Client {
        Client {
            router: self.router.clone(),
            jar: Mutex::new(BTreeMap::new()),
            ip: "198.51.100.1".to_owned(),
            origin: None,
        }
    }

    /// Signs up a verified-by-default test user and returns the signed-in client.
    pub async fn user(&self, email: &str) -> User {
        let client = self.client();
        let resp = client
            .post(
                "/api/v1/auth/signup",
                json!({"email": email, "password": GOOD_PASSWORD}),
            )
            .await;
        assert_eq!(resp.status, 201, "signup failed: {}", resp.text);
        let id = resp.body["account"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        User {
            client,
            id,
            email: email.to_owned(),
        }
    }

    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.admin)
            .await
            .unwrap()
    }
}

pub struct User {
    pub client: Client,
    pub id: factory_accounts::AccountId,
    pub email: String,
}

impl User {
    pub fn repo(&self, app: &TestApp) -> factory_accounts::TenantRepo {
        app.state.scoped(self.id)
    }

    pub async fn create_project(&self, name: &str) -> String {
        let resp = self
            .client
            .post("/api/v1/projects", json!({"name": name}))
            .await;
        assert_eq!(resp.status, 201, "{}", resp.text);
        resp.body["project"]["id"].as_str().unwrap().to_owned()
    }
}

pub fn token_from_mail(mail: &OutgoingMail) -> String {
    mail.text
        .split("token=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("mail carries a token link")
        .to_owned()
}

// ----- a minimal browser --------------------------------------------------------------------

pub struct Resp {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Value,
    pub text: String,
}

impl Resp {
    pub fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect()
    }

    pub fn set_cookie(&self, name: &str) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .find(|c| c.starts_with(&format!("{name}=")))
    }

    pub fn error_code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }

    pub fn location(&self) -> Option<String> {
        self.headers
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_owned())
    }
}

pub struct Client {
    router: Router,
    jar: Mutex<BTreeMap<String, String>>,
    pub ip: String,
    pub origin: Option<String>,
}

impl Client {
    pub fn with_ip(mut self, ip: &str) -> Self {
        self.ip = ip.to_owned();
        self
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.jar.lock().unwrap().get(name).cloned()
    }

    pub fn set_cookie(&self, name: &str, value: &str) {
        self.jar
            .lock()
            .unwrap()
            .insert(name.to_owned(), value.to_owned());
    }

    pub fn clear_jar(&self) {
        self.jar.lock().unwrap().clear();
    }

    pub async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        extra: &[(&str, &str)],
    ) -> Resp {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("x-forwarded-for", &self.ip)
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
        if let Some(origin) = &self.origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        for (k, v) in extra {
            builder = builder.header(*k, *v);
        }
        let request = match body {
            Some(b) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        let resp = Resp {
            status,
            headers,
            body,
            text,
        };
        self.absorb(&resp);
        resp
    }

    fn absorb(&self, resp: &Resp) {
        let mut jar = self.jar.lock().unwrap();
        for cookie in resp.set_cookies() {
            let first = cookie.split(';').next().unwrap();
            let (name, value) = first.split_once('=').unwrap();
            let expired = cookie.to_ascii_lowercase().contains("max-age=0");
            if expired || value.is_empty() {
                jar.remove(name);
            } else {
                jar.insert(name.to_owned(), value.to_owned());
            }
        }
    }

    pub async fn get(&self, path: &str) -> Resp {
        self.send("GET", path, None, &[]).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Resp {
        self.send("POST", path, Some(body), &[]).await
    }

    pub async fn post_empty(&self, path: &str) -> Resp {
        self.send("POST", path, None, &[]).await
    }

    pub async fn patch(&self, path: &str, body: Value) -> Resp {
        self.send("PATCH", path, Some(body), &[]).await
    }

    pub async fn delete(&self, path: &str) -> Resp {
        self.send("DELETE", path, None, &[]).await
    }
}
