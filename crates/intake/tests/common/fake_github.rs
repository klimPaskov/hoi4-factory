//! A local stand-in for GitHub: the REST calls intake makes, plus the Git smart-HTTP
//! transport served by the real `git http-backend` over bare repositories.
//!
//! The REST side verifies what a real GitHub would: the App JWT must be a valid RS256
//! signature of the test App key, an installation token must cover the repository being
//! asked for, and Git requests must carry the exact `Authorization: basic` header of a token
//! that was minted for that repository.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub const APP_ID: &str = "1234567";
pub const INSTALLATION_ID: u64 = 424242;

/// One RSA key pair for the whole test binary. Generated at run time and never written
/// anywhere, so no key material is committed.
pub struct TestKey {
    pub pkcs1_pem: String,
    pub private_pem: String,
    pub public_pem: String,
}

pub fn test_key() -> &'static TestKey {
    static KEY: OnceLock<TestKey> = OnceLock::new();
    KEY.get_or_init(|| {
        let mut rng = rand::rngs::OsRng;
        let private = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("generate a test key");
        TestKey {
            pkcs1_pem: rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_pem(&private, LineEnding::LF)
                .unwrap()
                .to_string(),
            private_pem: private.to_pkcs8_pem(LineEnding::LF).unwrap().to_string(),
            public_pem: private
                .to_public_key()
                .to_public_key_pem(LineEnding::LF)
                .unwrap(),
        }
    })
}

#[derive(Clone, Debug)]
pub struct FakeRepo {
    pub full_name: String,
    pub id: u64,
    pub node_id: String,
    pub default_branch: String,
    pub size_kib: u64,
}

impl FakeRepo {
    pub fn new(full_name: &str, id: u64) -> FakeRepo {
        FakeRepo {
            full_name: full_name.to_owned(),
            id,
            node_id: format!("R_node{id}"),
            default_branch: "main".into(),
            size_kib: 12,
        }
    }

    fn json(&self) -> Value {
        json!({
            "id": self.id, "node_id": self.node_id, "full_name": self.full_name,
            "default_branch": self.default_branch, "private": true, "archived": false,
            "size": self.size_kib,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GitCall {
    pub path: String,
    pub query: String,
    pub authorization: Option<String>,
    pub status: u16,
}

pub struct State_ {
    pub selection: String,
    pub suspended: bool,
    /// The installation is gone: every call about it answers 404.
    pub revoked: bool,
    pub repos: Vec<FakeRepo>,
    pub token_requests: Vec<Value>,
    /// token -> lowercase full names it covers
    pub tokens: HashMap<String, Vec<String>>,
    pub api_calls: u32,
    pub jwt_ok: u32,
    pub jwt_bad: u32,
    pub git_calls: Vec<GitCall>,
    pub git_reject_all: bool,
    /// Reject Git requests once this many have been served.
    pub git_reject_after: Option<usize>,
    pub rate_limit_installation: bool,
    pub installation_status_override: Option<u16>,
    pub repo_status_override: Option<u16>,
    pub minted: u32,
    pub git_root: PathBuf,
}

pub struct FakeGithub {
    pub addr: SocketAddr,
    pub state: Arc<Mutex<State_>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeGithub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeGithub {
    pub async fn start(git_root: &Path, repos: Vec<FakeRepo>) -> FakeGithub {
        let state = Arc::new(Mutex::new(State_ {
            selection: "selected".into(),
            suspended: false,
            revoked: false,
            repos,
            token_requests: Vec::new(),
            tokens: HashMap::new(),
            api_calls: 0,
            jwt_ok: 0,
            jwt_bad: 0,
            git_calls: Vec::new(),
            git_reject_all: false,
            git_reject_after: None,
            rate_limit_installation: false,
            installation_status_override: None,
            repo_status_override: None,
            minted: 0,
            git_root: git_root.to_path_buf(),
        }));
        let app = Router::new().fallback(handle).with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        FakeGithub { addr, state, task }
    }

    pub fn api_base(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    pub fn clone_base(&self) -> String {
        format!("http://127.0.0.1:{}/git", self.addr.port())
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State_) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }
}

fn json_response(status: u16, body: Value) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn not_found() -> Response {
    json_response(404, json!({"message": "Not Found"}))
}

#[derive(serde::Deserialize)]
struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
}

fn verify_jwt(state: &Arc<Mutex<State_>>, headers: &HeaderMap) -> bool {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let ok = token.is_some_and(|token| {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[APP_ID]);
        validation.set_required_spec_claims(&["exp", "iss"]);
        let key = DecodingKey::from_rsa_pem(test_key().public_pem.as_bytes()).unwrap();
        match decode::<Claims>(token, &key, &validation) {
            Ok(data) => data.claims.exp - data.claims.iat <= 600 && data.claims.iss == APP_ID,
            Err(_) => false,
        }
    });
    let mut guard = state.lock().unwrap();
    if ok {
        guard.jwt_ok += 1;
    } else {
        guard.jwt_bad += 1;
    }
    ok
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
}

type Shared = Arc<Mutex<State_>>;

async fn handle(State(state): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 256 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let path = parts.uri.path().to_owned();
    let query = parts.uri.query().unwrap_or("").to_owned();
    if let Some(rest) = path.strip_prefix("/git/") {
        return git_backend(&state, &parts.method, rest, &query, &parts.headers, body).await;
    }
    state.lock().unwrap().api_calls += 1;
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (&parts.method, segments.as_slice()) {
        (&Method::GET, ["app", "installations", id]) => {
            if let Some(early) = app_precheck(&state, &parts.headers, id) {
                return early;
            }
            let (selection, suspended) = {
                let s = state.lock().unwrap();
                (s.selection.clone(), s.suspended)
            };
            json_response(
                200,
                json!({
                    "id": INSTALLATION_ID, "repository_selection": selection,
                    "suspended_at": if suspended { json!("2026-10-01T00:00:00Z") } else { Value::Null },
                    "account": {"login": "klimPaskov"},
                }),
            )
        }
        (&Method::POST, ["app", "installations", id, "access_tokens"]) => {
            if let Some(early) = app_precheck(&state, &parts.headers, id) {
                return early;
            }
            access_tokens(&state, &body)
        }
        (&Method::GET, ["installation", "repositories"]) => {
            let Some(covered) = covered_by(&state, &parts.headers) else {
                return json_response(401, json!({"message": "Bad credentials"}));
            };
            let s = state.lock().unwrap();
            let repos: Vec<Value> = s
                .repos
                .iter()
                .filter(|r| covered.contains(&r.full_name.to_lowercase()))
                .map(FakeRepo::json)
                .collect();
            json_response(
                200,
                json!({"total_count": repos.len(), "repositories": repos}),
            )
        }
        (&Method::GET, ["repos", owner, name]) => {
            let full = format!("{owner}/{name}").to_lowercase();
            let status_override = state.lock().unwrap().repo_status_override;
            if let Some(status) = status_override {
                return json_response(status, json!({"message": "overridden"}));
            }
            let Some(covered) = covered_by(&state, &parts.headers) else {
                return json_response(401, json!({"message": "Bad credentials"}));
            };
            let s = state.lock().unwrap();
            match s.repos.iter().find(|r| r.full_name.to_lowercase() == full) {
                Some(repo) if covered.contains(&full) => json_response(200, repo.json()),
                _ => not_found(),
            }
        }
        _ => not_found(),
    }
}

/// JWT, rate limit, status override and installation existence.
fn app_precheck(state: &Shared, headers: &HeaderMap, id: &str) -> Option<Response> {
    if !verify_jwt(state, headers) {
        return Some(json_response(401, json!({"message": "Bad credentials"})));
    }
    let s = state.lock().unwrap();
    if s.rate_limit_installation {
        return Some(
            Response::builder()
                .status(403)
                .header("x-ratelimit-remaining", "0")
                .header("retry-after", "7")
                .body(Body::from("{\"message\":\"rate limit\"}"))
                .unwrap(),
        );
    }
    if let Some(status) = s.installation_status_override {
        return Some(json_response(status, json!({"message": "overridden"})));
    }
    if s.revoked || id.parse::<u64>().ok() != Some(INSTALLATION_ID) {
        return Some(not_found());
    }
    None
}

fn access_tokens(state: &Shared, body: &Bytes) -> Response {
    let request: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let mut s = state.lock().unwrap();
    s.token_requests.push(request.clone());
    let covered: Vec<FakeRepo> = match request.get("repositories").and_then(Value::as_array) {
        Some(names) => {
            let mut found = Vec::new();
            for name in names.iter().filter_map(Value::as_str) {
                match s.repos.iter().find(|r| {
                    r.full_name
                        .split('/')
                        .nth(1)
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                }) {
                    Some(repo) => found.push(repo.clone()),
                    None => {
                        return json_response(
                            422,
                            json!({"message": "There is at least one repository that does not exist or is not accessible to the parent installation."}),
                        );
                    }
                }
            }
            found
        }
        None => s.repos.clone(),
    };
    s.minted += 1;
    let token = format!("ghs_faketoken{:04}", s.minted);
    s.tokens.insert(
        token.clone(),
        covered.iter().map(|r| r.full_name.to_lowercase()).collect(),
    );
    json_response(
        201,
        json!({
            "token": token, "expires_at": "2099-01-01T00:00:00Z",
            "repositories": covered.iter().map(|r| json!({"full_name": r.full_name})).collect::<Vec<_>>(),
        }),
    )
}

/// Lowercase names covered by the bearer token, or `None` for an unknown token.
fn covered_by(state: &Shared, headers: &HeaderMap) -> Option<Vec<String>> {
    let token = bearer(headers)?;
    state.lock().unwrap().tokens.get(&token).cloned()
}

fn basic_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("authorization")?.to_str().ok()?;
    let encoded = raw
        .strip_prefix("basic ")
        .or_else(|| raw.strip_prefix("Basic "))?;
    let decoded = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
    decoded.strip_prefix("x-access-token:").map(str::to_owned)
}

async fn git_backend(
    state: &Shared,
    method: &Method,
    rest: &str,
    query: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    let mut parts = rest.splitn(3, '/');
    let owner = parts.next().unwrap_or("");
    let repo_git = parts.next().unwrap_or("");
    let full = format!("{owner}/{}", repo_git.trim_end_matches(".git")).to_lowercase();
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let allowed = {
        let s = state.lock().unwrap();
        !s.git_reject_all
            && s.git_reject_after.is_none_or(|n| s.git_calls.len() < n)
            && basic_token(headers)
                .and_then(|t| s.tokens.get(&t).cloned())
                .is_some_and(|covered| covered.contains(&full))
    };
    let record = |status: u16| {
        state.lock().unwrap().git_calls.push(GitCall {
            path: format!("/{rest}"),
            query: query.to_owned(),
            authorization: authorization.clone(),
            status,
        });
    };
    if !allowed {
        record(401);
        return Response::builder()
            .status(401)
            .header("www-authenticate", "Basic realm=\"fake\"")
            .body(Body::empty())
            .unwrap();
    }
    let git_root = state.lock().unwrap().git_root.clone();
    let mut command = Command::new("git");
    command
        .arg("http-backend")
        .env("GIT_PROJECT_ROOT", &git_root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", format!("/{rest}"))
        .env("REQUEST_METHOD", method.as_str())
        .env("QUERY_STRING", query)
        .env("REMOTE_USER", "x-access-token")
        .env("REMOTE_ADDR", "127.0.0.1")
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("CONTENT_LENGTH", body.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(value) = headers.get("content-type").and_then(|v| v.to_str().ok()) {
        command.env("CONTENT_TYPE", value);
    }
    if let Some(value) = headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
    {
        command.env("HTTP_CONTENT_ENCODING", value);
    }
    if let Some(value) = headers.get("git-protocol").and_then(|v| v.to_str().ok()) {
        command.env("HTTP_GIT_PROTOCOL", value);
    }
    let mut child = command.spawn().expect("git http-backend starts");
    let mut stdin = child.stdin.take().unwrap();
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(&body).await;
        drop(stdin);
    });
    let output = child
        .wait_with_output()
        .await
        .expect("git http-backend runs");
    let _ = writer.await;
    let raw = output.stdout;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4))
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2)));
    let Some((at, skip)) = split else {
        record(500);
        return json_response(500, json!({"message": "backend produced no headers"}));
    };
    let head = String::from_utf8_lossy(&raw[..at]).into_owned();
    let mut response = Response::builder();
    let mut status = 200u16;
    for line in head.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("status") {
            status = value
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(200);
        } else if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.trim().as_bytes()),
            HeaderValue::from_str(value),
        ) {
            response = response.header(name, value);
        }
    }
    record(status);
    response
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .body(Body::from(raw[at + skip..].to_vec()))
        .unwrap()
}

// ---------------------------------------------------------------------------------------------
// A real repository to serve

/// What the remote repository holds, for assertions.
pub struct RemoteFixture {
    pub root: PathBuf,
    /// Bare repository served as `<owner>/<name>.git`.
    pub bare: PathBuf,
    pub work: PathBuf,
    pub main_tip: String,
    pub feature_tip: String,
    pub tag_object: String,
}

/// LF in the blob, while `.gitattributes` says `eol=crlf` for `*.txt`.
pub const NOTES_TXT: &[u8] = b"line one\nline two\n";
pub const LEGACY_CSV: &[u8] = b"a;b;c\r\n1;2;3\r\n";
pub const BOM_GUI: &[u8] = b"\xEF\xBB\xBFguiTypes = {\r\n}\r\n";

/// Create `<git_root>/<owner>/<name>.git` with two branches and a tag. The tree has a
/// `.gitattributes` that would rewrite line endings on checkout, an LF file, a CRLF file, a
/// BOM file and a binary file.
pub fn make_remote(git_root: &Path, owner: &str, name: &str) -> RemoteFixture {
    use super::{binary_blob, git, git_text, write_file};
    let work = git_root.join("_work").join(name);
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write_file(&work, "descriptor.mod", b"name=\"Test\"\nversion=\"1\"\n");
    write_file(&work, "notes.txt", NOTES_TXT);
    write_file(&work, "data/legacy.csv", LEGACY_CSV);
    write_file(&work, "interface/bom.gui", BOM_GUI);
    write_file(&work, "gfx/art.dds", &binary_blob());
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "initial"]);
    // Added after the data, so the blobs above keep their bytes. On checkout these
    // attributes would rewrite LF to CRLF in *.txt if they were honoured.
    write_file(
        &work,
        ".gitattributes",
        b"*.txt text eol=crlf\n*.gui text=auto\n",
    );
    git(&work, &["add", ".gitattributes"]);
    git(&work, &["commit", "-q", "-m", "attributes"]);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    write_file(&work, "feature.txt", b"only on the feature branch\n");
    git(&work, &["add", "feature.txt"]);
    git(&work, &["commit", "-q", "-m", "feature"]);
    git(&work, &["tag", "-a", "-m", "release", "v1", "main"]);
    git(&work, &["checkout", "-q", "main"]);
    let bare = git_root.join(owner).join(format!("{name}.git"));
    std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
    git(
        git_root,
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    RemoteFixture {
        root: git_root.to_path_buf(),
        main_tip: git_text(&bare, &["rev-parse", "refs/heads/main"]),
        feature_tip: git_text(&bare, &["rev-parse", "refs/heads/feature"]),
        tag_object: git_text(&bare, &["rev-parse", "refs/tags/v1"]),
        bare,
        work,
    }
}
