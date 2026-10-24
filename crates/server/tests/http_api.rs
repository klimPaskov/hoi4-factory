//! The assembled server against a real PostgreSQL: accounts over HTTP, tenant isolation,
//! hardening, CORS, the game reference endpoint, readiness and static serving.
//!
//! Needs a disposable PostgreSQL reachable with a SUPERUSER URL in `FACTORY_TEST_ADMIN_URL`
//! (for example a `postgres` container with `POSTGRES_HOST_AUTH_METHOD=trust` on a random
//! local port). Each test makes its own database, migrates it as a non-superuser owner role
//! and serves it as a non-superuser application role, as production does.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, header};
use factory_accounts::config::Argon2Params;
use factory_accounts::{
    AccountsDeps, Clock, InMemoryRateLimiter, MemoryMailer, OutgoingMail, Rule, SystemClock,
};
use factory_server::{
    Overrides, ServerConfig, ServerState, StartupError, build_app, initialize_with,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

const OWNER_ROLE: &str = "hf_srv_owner";
const APP_ROLE: &str = "hf_srv_app";
const PASSWORD: &str = "correct horse battery";
const VITE_ORIGIN: &str = "http://localhost:5173";

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

/// A game folder with the metadata the reader wants, and a canary file it must never expose.
fn fixture_game_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("launcher-settings.json"),
        r#"{"version":"1.19.3","rawVersion":"1.19.3.0","gameId":"hoi4"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("hoi4_rev.txt"), "c01a\n").unwrap();
    std::fs::write(dir.path().join("checksum_manifest.txt"), "abc\n").unwrap();
    std::fs::write(dir.path().join("canary-secret-file.txt"), "CANARY-CONTENT").unwrap();
    for dlc in ["dlc001_sky_high", "dlc002_by_blood_alone"] {
        std::fs::create_dir_all(dir.path().join("dlc").join(dlc)).unwrap();
    }
    dir
}

#[derive(Default)]
struct Options {
    production: bool,
    missing_game: bool,
    web_dist: Option<std::path::PathBuf>,
    max_body_bytes: Option<usize>,
}

struct Harness {
    state: ServerState,
    app: Router,
    mailer: Arc<MemoryMailer>,
    game_dir: TempDir,
    data_dir: TempDir,
    database: String,
}

fn lookup_config(options: &Options, database: &str, game: &str, data: &str) -> ServerConfig {
    let mut vars: BTreeMap<&str, String> = BTreeMap::new();
    vars.insert("DATABASE_URL", url_for(APP_ROLE, database));
    vars.insert("DATABASE_MIGRATION_URL", url_for(OWNER_ROLE, database));
    vars.insert("FACTORY_DATA_ROOT", data.to_owned());
    vars.insert("FACTORY_GAME_REF", game.to_owned());
    vars.insert("FACTORY_ALLOWED_ORIGINS", VITE_ORIGIN.to_owned());
    if options.production {
        vars.insert("FACTORY_ENV", "production".into());
        vars.insert("FACTORY_PUBLIC_URL", "https://factory.test".into());
    } else {
        vars.insert("FACTORY_ENV", "development".into());
        vars.insert("FACTORY_PUBLIC_URL", "http://localhost:8080".into());
    }
    if let Some(dist) = &options.web_dist {
        vars.insert("FACTORY_WEB_DIST", dist.to_string_lossy().into_owned());
    }
    if let Some(limit) = options.max_body_bytes {
        vars.insert("FACTORY_MAX_BODY_BYTES", limit.to_string());
    }
    let mut config = ServerConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
    // Cheap hashing and generous limits: the tests make many sign-ups from one address.
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
        global_log();
        let database = format!("hf_s_{}", Uuid::new_v4().simple());
        create_database(&database).await;
        let game_dir = fixture_game_dir();
        let data_dir = tempfile::tempdir().unwrap();
        let game_path = if options.missing_game {
            game_dir.path().join("not-mounted")
        } else {
            game_dir.path().to_path_buf()
        };
        let config = lookup_config(
            &options,
            &database,
            &game_path.to_string_lossy(),
            &data_dir.path().to_string_lossy(),
        );
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
        let state = initialize_with(
            config,
            Overrides {
                accounts_deps: Some(deps),
                clock: Some(clock),
                ..Overrides::default()
            },
        )
        .await
        .expect("the server starts");
        let app = build_app(state.clone());
        Self {
            state,
            app,
            mailer,
            game_dir,
            data_dir,
            database,
        }
    }

    fn client(&self) -> Client {
        Client {
            router: self.app.clone(),
            jar: Mutex::new(BTreeMap::new()),
        }
    }

    /// Signs up through HTTP, redeems the verification mail and returns the signed-in client.
    async fn verified_user(&self, email: &str) -> Client {
        let client = self.client();
        let resp = client
            .post(
                "/api/v1/auth/signup",
                json!({"email": email, "password": PASSWORD}),
            )
            .await;
        assert_eq!(resp.status, 201, "{}", resp.text);
        let mail = self.mailer.last_to(email).expect("verification mail sent");
        let token = token_from_mail(&mail);
        let resp = client
            .post("/api/v1/auth/verify-email", json!({ "token": token }))
            .await;
        assert_eq!(resp.status, 200, "{}", resp.text);
        assert_eq!(resp.body["account"]["email_verified"], true);
        client
    }
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
    headers: HeaderMap,
    body: Value,
    text: String,
}

impl Resp {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    fn error_code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }
}

struct Client {
    router: Router,
    jar: Mutex<BTreeMap<String, String>>,
}

impl Client {
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
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
        for cookie in headers.get_all(header::SET_COOKIE) {
            let first = cookie.to_str().unwrap().split(';').next().unwrap();
            let (name, value) = first.split_once('=').unwrap();
            self.jar
                .lock()
                .unwrap()
                .insert(name.to_owned(), value.to_owned());
        }
        Resp {
            status,
            headers,
            body,
            text,
        }
    }

    async fn get(&self, path: &str) -> Resp {
        self.send("GET", path, None, &[]).await
    }

    async fn get_with(&self, path: &str, extra: &[(&str, &str)]) -> Resp {
        self.send("GET", path, None, extra).await
    }

    async fn post(&self, path: &str, body: Value) -> Resp {
        self.send("POST", path, Some(body), &[]).await
    }

    async fn post_with(&self, path: &str, body: Value, extra: &[(&str, &str)]) -> Resp {
        self.send("POST", path, Some(body), extra).await
    }
}

// ----- accounts and tenant isolation over HTTP -----------------------------------------------

#[tokio::test]
async fn two_accounts_sign_up_verify_and_cannot_reach_each_others_projects() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let bob = h.verified_user("bob+mods@example.org").await;

    let created = alice
        .post("/api/v1/projects", json!({"name": "Alice's Mod"}))
        .await;
    assert_eq!(created.status, 201, "{}", created.text);
    let alice_project = created.body["project"]["id"].as_str().unwrap().to_owned();
    let bobs = bob
        .post("/api/v1/projects", json!({"name": "Bob's Mod"}))
        .await;
    assert_eq!(bobs.status, 201, "{}", bobs.text);
    let bob_project = bobs.body["project"]["id"].as_str().unwrap().to_owned();

    // Each sees exactly their own project.
    let alice_list = alice.get("/api/v1/projects").await;
    let ids: Vec<&str> = alice_list.body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [alice_project.as_str()]);
    let bob_list = bob.get("/api/v1/projects").await;
    assert_eq!(bob_list.body["projects"].as_array().unwrap().len(), 1);
    assert_eq!(bob_list.body["projects"][0]["id"], bob_project);

    // Bob cannot read, rename, delete or restore Alice's project, and cannot tell it exists.
    let path = format!("/api/v1/projects/{alice_project}");
    let missing = format!("/api/v1/projects/{}", Uuid::new_v4());
    let read = bob.get(&path).await;
    assert_eq!((read.status, read.error_code()), (404, "not_found"));
    let absent = bob.get(&missing).await;
    assert_eq!(
        (read.status, read.body.clone()),
        (absent.status, absent.body.clone()),
        "an existing foreign project looks exactly like a missing one"
    );
    for (method, body) in [("PATCH", Some(json!({"name": "stolen"}))), ("DELETE", None)] {
        let resp = bob.send(method, &path, body, &[]).await;
        assert_eq!(resp.status, 404, "{method}: {}", resp.text);
    }
    let restore = bob
        .send("POST", &format!("{path}/restore"), None, &[])
        .await;
    assert_eq!(restore.status, 404);

    // Alice's project is untouched and still hers.
    let still = alice.get(&path).await;
    assert_eq!(still.status, 200, "{}", still.text);
    assert_eq!(still.body["project"]["name"], "Alice's Mod");

    // Nobody signed in sees nothing.
    let anonymous = h.client();
    let denied = anonymous.get(&path).await;
    assert_eq!(
        (denied.status, denied.error_code()),
        (401, "unauthenticated")
    );
}

// ----- hardening -----------------------------------------------------------------------------

fn assert_security_headers(resp: &Resp, hsts: bool, what: &str) {
    let csp = resp.header("content-security-policy").unwrap_or_default();
    for directive in [
        "default-src 'self'",
        "frame-ancestors 'none'",
        "object-src 'none'",
    ] {
        assert!(
            csp.contains(directive),
            "{what}: CSP lacks {directive}: {csp}"
        );
    }
    assert_eq!(
        resp.header("x-content-type-options"),
        Some("nosniff"),
        "{what}"
    );
    assert_eq!(
        resp.header("referrer-policy"),
        Some("strict-origin-when-cross-origin"),
        "{what}"
    );
    assert_eq!(
        resp.header("strict-transport-security").is_some(),
        hsts,
        "{what}: HSTS"
    );
    assert!(resp.header("x-request-id").is_some(), "{what}: request id");
}

#[tokio::test]
async fn every_response_carries_the_security_headers() {
    let h = Harness::start(Options::default()).await;
    let client = h.client();
    let user = h.verified_user("headers@example.org").await;
    for (what, resp) in [
        ("healthz", client.get("/healthz").await),
        ("readyz", client.get("/readyz").await),
        ("401", client.get("/api/v1/auth/me").await),
        ("api 404", client.get("/api/v1/nowhere").await),
        ("root 404", client.get("/nowhere").await),
        ("200 json", user.get("/api/v1/projects").await),
        (
            "preflight",
            client
                .send(
                    "OPTIONS",
                    "/api/v1/projects",
                    None,
                    &[
                        ("origin", VITE_ORIGIN),
                        ("access-control-request-method", "POST"),
                    ],
                )
                .await,
        ),
    ] {
        assert_security_headers(&resp, false, what);
    }
    // API responses are not cacheable.
    assert_eq!(
        user.get("/api/v1/projects").await.header("cache-control"),
        Some("no-store")
    );
}

#[tokio::test]
async fn production_adds_strict_transport_security() {
    let h = Harness::start(Options {
        production: true,
        ..Options::default()
    })
    .await;
    let resp = h.client().get("/healthz").await;
    assert_security_headers(&resp, true, "production healthz");
    assert!(
        resp.header("strict-transport-security")
            .unwrap()
            .starts_with("max-age=")
    );
}

#[tokio::test]
async fn cors_admits_only_configured_origins_and_never_everyone() {
    let h = Harness::start(Options::default()).await;
    let client = h.client();
    let preflight = |origin: &'static str| {
        let client = &client;
        async move {
            client
                .send(
                    "OPTIONS",
                    "/api/v1/auth/signin",
                    None,
                    &[
                        ("origin", origin),
                        ("access-control-request-method", "POST"),
                        ("access-control-request-headers", "content-type"),
                    ],
                )
                .await
        }
    };

    let foreign = preflight("https://evil.example").await;
    assert!(
        foreign.header("access-control-allow-origin").is_none(),
        "a foreign origin must not be echoed"
    );

    let allowed = preflight(VITE_ORIGIN).await;
    assert_eq!(
        allowed.header("access-control-allow-origin"),
        Some(VITE_ORIGIN)
    );
    assert_eq!(
        allowed.header("access-control-allow-credentials"),
        Some("true")
    );

    // Ordinary requests: the foreign origin gets no CORS headers, the configured one does.
    let simple = client
        .get_with("/healthz", &[("origin", "https://evil.example")])
        .await;
    assert!(simple.header("access-control-allow-origin").is_none());
    let simple = client
        .get_with("/healthz", &[("origin", VITE_ORIGIN)])
        .await;
    assert_eq!(
        simple.header("access-control-allow-origin"),
        Some(VITE_ORIGIN)
    );

    // Nothing ever answers with the wildcard.
    for resp in [&foreign, &allowed, &simple] {
        assert_ne!(resp.header("access-control-allow-origin"), Some("*"));
    }

    // And the accounts guard still refuses a state-changing request from a foreign origin.
    let forged = client
        .post_with(
            "/api/v1/auth/signup",
            json!({"email": "forged@example.org", "password": PASSWORD}),
            &[("origin", "https://evil.example")],
        )
        .await;
    assert_eq!(
        (forged.status, forged.error_code()),
        (403, "origin_not_allowed")
    );
    assert!(h.mailer.last_to("forged@example.org").is_none());
}

#[tokio::test]
async fn oversized_bodies_are_refused() {
    let h = Harness::start(Options {
        max_body_bytes: Some(2048),
        ..Options::default()
    })
    .await;
    let client = h.client();
    let big = "x".repeat(10_000);
    let resp = client
        .post(
            "/api/v1/auth/signup",
            json!({"email": "big@example.org", "password": PASSWORD, "display_name": big}),
        )
        .await;
    assert!(
        (400..500).contains(&resp.status),
        "{} {}",
        resp.status,
        resp.text
    );
    assert!(h.mailer.last_to("big@example.org").is_none());
    // A normal request still works.
    let ok = client
        .post(
            "/api/v1/auth/signup",
            json!({"email": "small@example.org", "password": PASSWORD}),
        )
        .await;
    assert_eq!(ok.status, 201, "{}", ok.text);
}

// ----- request ids and logs ------------------------------------------------------------------

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

struct LogHandle(Arc<Mutex<Vec<u8>>>);

impl Write for LogHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = LogHandle;

    fn make_writer(&'a self) -> LogHandle {
        LogHandle(self.0.clone())
    }
}

/// One process-wide log capture. A thread-local default subscriber is unreliable when tests
/// run in parallel (cached callsite interest), so every test shares this global one and looks
/// for its own unique markers.
fn global_log() -> LogBuffer {
    use tracing_subscriber::layer::SubscriberExt;
    static LOG: std::sync::OnceLock<LogBuffer> = std::sync::OnceLock::new();
    LOG.get_or_init(|| {
        let buffer = LogBuffer::default();
        let subscriber = tracing_subscriber::registry()
            .with(factory_server::logging::JsonLogLayer::new(buffer.clone()));
        tracing::subscriber::set_global_default(subscriber).expect("one global subscriber");
        buffer
    })
    .clone()
}

#[tokio::test]
async fn request_ids_are_echoed_and_logs_hold_no_secrets() {
    let buffer = global_log();
    let h = Harness::start(Options::default()).await;
    let client = h.client();

    // A well-formed inbound id is kept; a malformed one is replaced.
    let kept = client
        .get_with("/healthz", &[("x-request-id", "trace-42.abc_DEF")])
        .await;
    assert_eq!(kept.header("x-request-id"), Some("trace-42.abc_DEF"));
    let replaced = client
        .get_with("/healthz", &[("x-request-id", "bad id with spaces")])
        .await;
    let generated = replaced.header("x-request-id").unwrap();
    assert!(Uuid::parse_str(generated).is_ok(), "{generated}");

    let secret_password = "S3cret-Passphrase-For-Log-Test";
    let resp = client
        .send(
            "POST",
            "/api/v1/auth/signin?code=ONE-TIME-CODE&page=2",
            Some(json!({"email": "nobody@example.org", "password": secret_password})),
            &[
                ("cookie", "hf_session=COOKIE-VALUE-SHOULD-NOT-LOG"),
                ("authorization", "Bearer BEARER-VALUE-SHOULD-NOT-LOG"),
                ("x-request-id", "login-attempt-1"),
            ],
        )
        .await;
    assert_eq!(resp.status, 401, "{}", resp.text);

    let logs = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    for secret in [
        secret_password,
        "COOKIE-VALUE-SHOULD-NOT-LOG",
        "BEARER-VALUE-SHOULD-NOT-LOG",
        "ONE-TIME-CODE",
    ] {
        assert!(
            !logs.contains(secret),
            "{secret} leaked into the logs:\n{logs}"
        );
    }
    let line: Value = logs
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["fields"]["request_id"] == "login-attempt-1")
        .unwrap_or_else(|| {
            panic!(
                "the sign-in request was logged:
{logs}"
            )
        });
    assert_eq!(line["fields"]["path"], "/api/v1/auth/signin");
    assert_eq!(line["fields"]["status"], 401);
    assert_eq!(line["fields"]["method"], "POST");
    assert_eq!(line["fields"]["query"], "code=[redacted]&page=2");
}

// ----- game reference ------------------------------------------------------------------------

#[tokio::test]
async fn game_reference_endpoint_serves_the_manifest_summary_and_never_a_path() {
    let h = Harness::start(Options::default()).await;
    let anonymous = h.client();
    let denied = anonymous.get("/api/v1/game-reference").await;
    assert_eq!(
        (denied.status, denied.error_code()),
        (401, "unauthenticated")
    );

    // Even an unverified account may read the summary: it is identity metadata.
    let user = h.client();
    let signup = user
        .post(
            "/api/v1/auth/signup",
            json!({"email": "ref@example.org", "password": PASSWORD}),
        )
        .await;
    assert_eq!(signup.status, 201);
    let resp = user.get("/api/v1/game-reference").await;
    assert_eq!(resp.status, 200, "{}", resp.text);
    assert_eq!(resp.body["available"], true);
    assert_eq!(resp.body["version"], "1.19.3");
    assert_eq!(resp.body["raw_version"], "1.19.3.0");
    assert_eq!(resp.body["rev"], "c01a");
    assert_eq!(
        resp.body["dlc_ids"],
        json!(["dlc001_sky_high", "dlc002_by_blood_alone"])
    );
    assert_eq!(resp.body["manifest_hash"].as_str().unwrap().len(), 64);
    // A temp directory is writable, so the probe says so. A real `:ro` mount says true.
    assert_eq!(resp.body["read_only"], false);
    assert_eq!(resp.body["read_only_evidence"], "writable");

    // Only the allowlisted keys, nothing path-like, no file content.
    let mut keys: Vec<&str> = resp
        .body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "available",
            "dlc_ids",
            "manifest_hash",
            "raw_version",
            "read_only",
            "read_only_evidence",
            "rev",
            "version"
        ]
    );
    let game_path = h.game_dir.path().to_string_lossy().into_owned();
    let data_path = h.data_dir.path().to_string_lossy().into_owned();
    for needle in [
        game_path.as_str(),
        data_path.as_str(),
        "canary",
        "CANARY-CONTENT",
        "launcher-settings",
        "checksum_manifest",
        "mount",
    ] {
        assert!(
            !resp.text.contains(needle),
            "{needle} leaked: {}",
            resp.text
        );
    }
}

#[tokio::test]
async fn a_missing_game_mount_degrades_instead_of_crashing() {
    let h = Harness::start(Options {
        missing_game: true,
        ..Options::default()
    })
    .await;
    let user = h.client();
    user.post(
        "/api/v1/auth/signup",
        json!({"email": "nomount@example.org", "password": PASSWORD}),
    )
    .await;
    let resp = user.get("/api/v1/game-reference").await;
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body["available"], false);
    assert_eq!(resp.body["reason"], "not_mounted");
    assert!(resp.body.get("version").is_none());
    let game_path = h.game_dir.path().to_string_lossy().into_owned();
    assert!(!resp.text.contains(&game_path));

    let ready = h.client().get("/readyz").await;
    assert_eq!(ready.status, 200, "{}", ready.text);
    assert_eq!(ready.body["status"], "degraded");
    assert_eq!(ready.body["checks"]["game_reference"]["available"], false);
    assert_eq!(ready.body["checks"]["database"]["ok"], true);
    assert!(!ready.text.contains(&game_path));
}

// ----- readiness and startup -----------------------------------------------------------------

#[tokio::test]
async fn readyz_reports_each_dependency_and_fails_when_the_database_goes_away() {
    let h = Harness::start(Options::default()).await;
    let client = h.client();
    let live = client.get("/healthz").await;
    assert_eq!(
        (live.status, live.body["status"].as_str()),
        (200, Some("ok"))
    );

    let ready = client.get("/readyz").await;
    assert_eq!(ready.status, 200, "{}", ready.text);
    let checks = &ready.body["checks"];
    assert_eq!(checks["database"]["ok"], true);
    assert_eq!(checks["migrations"]["ok"], true);
    assert!(checks["migrations"]["latest_version"].as_i64().unwrap() >= 101);
    assert_eq!(checks["storage"]["ok"], true);
    assert_eq!(checks["storage"]["issues"], json!([]));
    // The fixture directory is writable, so readiness is degraded rather than ok.
    assert_eq!(ready.body["status"], "degraded");
    assert!(
        !ready
            .text
            .contains(&h.data_dir.path().to_string_lossy().into_owned())
    );

    // Migrations are recorded and re-running them is a no-op.
    let admin = PgPool::connect(&url_for(admin_url().username(), &h.database))
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!(before >= 2);
    factory_server::migrations::run_all(&admin).await.unwrap();
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(before, after);
    admin.close().await;

    // Lose the database: readiness fails, liveness keeps answering.
    h.state.db.pool().close().await;
    let down = client.get("/readyz").await;
    assert_eq!(down.status, 503, "{}", down.text);
    assert_eq!(down.body["status"], "unavailable");
    assert_eq!(down.body["checks"]["database"]["ok"], false);
    assert_eq!(client.get("/healthz").await.status, 200);
}

#[tokio::test]
async fn production_refuses_a_database_role_that_bypasses_row_level_security() {
    let database = format!("hf_s_{}", Uuid::new_v4().simple());
    create_database(&database).await;
    let superuser = admin_url().username().to_owned();
    let data = tempfile::tempdir().unwrap();
    let game = fixture_game_dir();
    let vars: BTreeMap<&str, String> = BTreeMap::from([
        ("FACTORY_ENV", "production".to_owned()),
        ("FACTORY_PUBLIC_URL", "https://factory.test".to_owned()),
        ("DATABASE_URL", url_for(&superuser, &database)),
        (
            "FACTORY_DATA_ROOT",
            data.path().to_string_lossy().into_owned(),
        ),
        (
            "FACTORY_GAME_REF",
            game.path().to_string_lossy().into_owned(),
        ),
    ]);
    let config = ServerConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
    let result = initialize_with(config, Overrides::default()).await;
    assert!(
        matches!(result, Err(StartupError::Posture(_))),
        "a superuser connection must stop a production start"
    );
}

// ----- the browser client --------------------------------------------------------------------

#[tokio::test]
async fn the_built_client_is_served_with_a_single_page_fallback() {
    let root = tempfile::tempdir().unwrap();
    let dist = root.path().join("dist");
    std::fs::create_dir_all(dist.join("assets")).unwrap();
    std::fs::write(
        dist.join("index.html"),
        "<!doctype html><title>HOI4 Factory</title>",
    )
    .unwrap();
    std::fs::write(dist.join("assets").join("app-abc123.js"), "console.log(1)").unwrap();
    std::fs::write(root.path().join("outside-secret.txt"), "OUTSIDE-SECRET").unwrap();

    let h = Harness::start(Options {
        web_dist: Some(dist),
        ..Options::default()
    })
    .await;
    let client = h.client();

    let index = client.get("/").await;
    assert_eq!(index.status, 200);
    assert!(index.text.contains("HOI4 Factory"));
    assert_eq!(index.header("cache-control"), Some("no-cache"));
    assert_security_headers(&index, false, "index");

    // An unknown client route falls back to the app.
    let route = client.get("/projects/123/scan").await;
    assert_eq!(route.status, 200);
    assert!(route.text.contains("HOI4 Factory"));

    let asset = client.get("/assets/app-abc123.js").await;
    assert_eq!(asset.status, 200);
    assert_eq!(
        asset.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );

    // API and probe paths never fall back to the app.
    for path in [
        "/api/v1/unknown",
        "/api/unknown",
        "/api",
        "/api/v1/projects/x/y/z",
    ] {
        let resp = client.get(path).await;
        assert_eq!(resp.status, 404, "{path}");
        assert_eq!(resp.error_code(), "not_found", "{path}: {}", resp.text);
        assert!(!resp.text.contains("<title>"), "{path}");
    }
    assert_eq!(client.get("/healthz").await.body["status"], "ok");

    // No escaping the directory.
    for path in [
        "/../outside-secret.txt",
        "/%2e%2e/outside-secret.txt",
        "/..%2foutside-secret.txt",
    ] {
        let resp = client.get(path).await;
        assert!(!resp.text.contains("OUTSIDE-SECRET"), "{path}");
    }
}

// ----- the real socket -----------------------------------------------------------------------

#[tokio::test]
async fn serves_over_tcp_with_connect_info() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let h = Harness::start(Options::default()).await;
    let app = build_app(h.state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(factory_server::serve(listener, app, async move {
        let _ = stop_rx.await;
    }));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let text = String::from_utf8_lossy(&raw).to_lowercase();
    assert!(text.starts_with("http/1.1 200"), "{text}");
    assert!(text.contains("content-security-policy"));
    assert!(text.contains("x-request-id"));

    // The container health probe works against the same socket.
    assert!(
        tokio::task::spawn_blocking(move || factory_server::healthcheck::probe(addr))
            .await
            .unwrap()
    );

    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("graceful shutdown")
        .unwrap()
        .unwrap();
}
