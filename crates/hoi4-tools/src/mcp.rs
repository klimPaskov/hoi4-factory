//! A thin MCP client over newline-delimited JSON-RPC on a child's stdio.
//!
//! The factory writes this itself instead of using an MCP SDK (S03 scope default 6) because it
//! needs bounded frames, an explicit child environment, process-group termination and its own
//! durable wrapper around every call. Invariants:
//!
//! - The child gets `env_clear()` plus only the variables in the [`McpServerSpec`]; the host
//!   environment is never inherited.
//! - The child runs in its own process group on Unix, so `shutdown` and `Drop` can kill helpers
//!   it spawned, not just the leader.
//! - stdout carries protocol only. A line over `max_frame_bytes`, invalid UTF-8 or invalid JSON
//!   ends the session with [`McpError::Protocol`]. stderr is kept in a bounded ring for
//!   diagnostics and never parsed.
//! - A timed-out call is cancelled with `notifications/cancelled`, but its outcome stays unknown:
//!   only a pure read may be repeated after [`McpError::Timeout`].

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, oneshot};

/// The only protocol version this client offers and accepts.
pub const OFFERED_PROTOCOL_VERSION: &str = "2025-06-18";
/// Versions the client accepts in the server's `initialize` answer.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[OFFERED_PROTOCOL_VERSION];
/// Default bound for one stdout line.
pub const DEFAULT_MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const STDERR_RING_BYTES: usize = 16 * 1024;
const SHUTDOWN_GRACE: Duration = Duration::from_millis(750);

/// How to launch one MCP server.
#[derive(Debug, Clone)]
pub struct McpServerSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Working directory; also answered to `roots/list` as a `file://` URI.
    pub cwd: PathBuf,
    /// The complete child environment. Nothing else is inherited.
    pub env: Vec<(String, String)>,
    pub startup_timeout: Duration,
    pub default_call_timeout: Duration,
    pub max_frame_bytes: usize,
}

impl McpServerSpec {
    pub fn new(program: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            env: Vec::new(),
            startup_timeout: Duration::from_secs(30),
            default_call_timeout: Duration::from_secs(60),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }
}

/// What a server said about itself in `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    pub protocol_version: String,
    pub server_name: String,
    pub server_version: String,
    pub capabilities: Value,
    /// SHA-256 of the server's `instructions` text, when it sent any. The text itself is never
    /// stored: it is untrusted input for the model, not for the host.
    pub instructions_sha256: Option<String>,
}

/// One tool from `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: Value,
    /// SHA-256 of the canonical JSON of `input_schema`.
    pub schema_sha256: String,
}

/// The result of one `tools/call`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: Vec<Value>,
    pub structured: Option<Value>,
    pub is_error: bool,
    /// Serialized size of the result as received.
    pub bytes: usize,
    /// Set by the caller that bounded the output (the session), never by the client.
    pub truncated: bool,
}

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpError {
    #[error("could not spawn the server: {message}")]
    Spawn { message: String },
    #[error("handshake failed: {message}")]
    Handshake { message: String },
    /// `sent` is true when `notifications/cancelled` was written for the request. The outcome of
    /// the call is unknown either way.
    #[error("the call timed out (cancellation sent: {sent})")]
    Timeout { sent: bool },
    #[error("protocol violation: {message}")]
    Protocol { message: String },
    #[error("server error {code}: {message}")]
    ServerError { code: i64, message: String },
    #[error("the server process exited (status {status:?})")]
    ProcessExited {
        status: Option<i32>,
        stderr_tail: String,
    },
    #[error("transport failure: {message}")]
    Transport { message: String },
}

impl McpError {
    /// A short stable class name for receipts and metrics.
    pub fn class(&self) -> &'static str {
        match self {
            McpError::Spawn { .. } => "spawn",
            McpError::Handshake { .. } => "handshake",
            McpError::Timeout { .. } => "timeout",
            McpError::Protocol { .. } => "protocol",
            McpError::ServerError { .. } => "server_error",
            McpError::ProcessExited { .. } => "process_exited",
            McpError::Transport { .. } => "transport",
        }
    }
}

/// What happened when the process was stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShutdownReport {
    pub exit_status: Option<i32>,
    /// True when the process did not leave within the grace period and was killed.
    pub killed: bool,
}

type Pending = HashMap<u64, oneshot::Sender<Result<Value, McpError>>>;

struct State {
    pending: Pending,
    ended: Option<McpError>,
}

struct Shared {
    state: StdMutex<State>,
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Option<Child>>,
    stderr: StdMutex<VecDeque<u8>>,
    root_uri: String,
    pid: Option<u32>,
    unknown_responses: AtomicU64,
    notifications: AtomicU64,
}

impl Shared {
    fn stderr_tail(&self) -> String {
        let ring = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
        let bytes: Vec<u8> = ring.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Ends the session once; later calls keep the first error.
    fn end(&self, error: McpError) {
        let drained = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.ended.is_none() {
                state.ended = Some(error);
            }
            std::mem::take(&mut state.pending)
        };
        let error = self.ended_error();
        for (_, tx) in drained {
            let _ = tx.send(Err(error.clone().unwrap_or(McpError::Transport {
                message: "session ended".into(),
            })));
        }
    }

    fn ended_error(&self) -> Option<McpError> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ended
            .clone()
    }

    async fn write_message(&self, message: &Value) -> Result<(), McpError> {
        let mut line = serde_json::to_vec(message).map_err(|e| McpError::Transport {
            message: e.to_string(),
        })?;
        line.push(b'\n');
        let mut guard = self.stdin.lock().await;
        let Some(stdin) = guard.as_mut() else {
            return Err(McpError::Transport {
                message: "stdin is closed".into(),
            });
        };
        let written = async {
            stdin.write_all(&line).await?;
            stdin.flush().await
        }
        .await;
        written.map_err(|e| McpError::Transport {
            message: format!("write to server failed: {e}"),
        })
    }
}

/// A connected MCP session with one child process.
pub struct McpClient {
    shared: Arc<Shared>,
    next_id: AtomicU64,
    call_timeout: Duration,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("pid", &self.shared.pid)
            .finish_non_exhaustive()
    }
}

fn file_uri(path: &std::path::Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    if !text.starts_with('/') {
        text.insert(0, '/');
    }
    let mut out = String::from("file://");
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// SHA-256 over canonical JSON (sorted keys, no whitespace), as lowercase hex.
pub fn canonical_sha256(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    hex(&Sha256::digest(out.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                if let Some(v) = map.get(key) {
                    write_canonical(v, out);
                }
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

enum FrameError {
    TooLong,
    Io(std::io::Error),
}

/// Reads one newline-terminated frame without ever buffering more than `max` bytes of it.
async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Option<Vec<u8>>, FrameError> {
    let mut line: Vec<u8> = Vec::new();
    loop {
        let buf = reader.fill_buf().await.map_err(FrameError::Io)?;
        if buf.is_empty() {
            return Ok(if line.is_empty() { None } else { Some(line) });
        }
        match buf.iter().position(|b| *b == b'\n') {
            Some(pos) => {
                if line.len() + pos > max {
                    return Err(FrameError::TooLong);
                }
                line.extend_from_slice(&buf[..pos]);
                reader.consume(pos + 1);
                return Ok(Some(line));
            }
            None => {
                let len = buf.len();
                if line.len() + len > max {
                    return Err(FrameError::TooLong);
                }
                line.extend_from_slice(buf);
                reader.consume(len);
            }
        }
    }
}

async fn reader_task(shared: Arc<Shared>, stdout: tokio::process::ChildStdout, max: usize) {
    let mut reader = BufReader::with_capacity(8192, stdout);
    loop {
        let frame = match read_frame(&mut reader, max).await {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                let status = {
                    let mut guard = shared.child.lock().await;
                    match guard.as_mut() {
                        Some(child) => {
                            tokio::time::timeout(Duration::from_millis(750), child.wait())
                                .await
                                .ok()
                                .and_then(|r| r.ok())
                                .and_then(|s| s.code())
                        }
                        None => None,
                    }
                };
                shared.end(McpError::ProcessExited {
                    status,
                    stderr_tail: shared.stderr_tail(),
                });
                return;
            }
            Err(FrameError::TooLong) => {
                protocol_violation(
                    &shared,
                    format!("a stdout line exceeded {max} bytes without a newline"),
                );
                return;
            }
            Err(FrameError::Io(e)) => {
                shared.end(McpError::Transport {
                    message: format!("read from server failed: {e}"),
                });
                return;
            }
        };
        if frame.is_empty() || frame == b"\r" {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&frame) else {
            protocol_violation(&shared, "stdout line is not valid UTF-8".into());
            return;
        };
        let message: Value = match serde_json::from_str(text.trim_end_matches('\r')) {
            Ok(v) => v,
            Err(e) => {
                protocol_violation(&shared, format!("stdout line is not JSON: {e}"));
                return;
            }
        };
        handle_message(&shared, message).await;
    }
}

fn protocol_violation(shared: &Shared, message: String) {
    kill_group(shared.pid);
    shared.end(McpError::Protocol { message });
}

async fn handle_message(shared: &Arc<Shared>, message: Value) {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").cloned();
    match (method, id) {
        (Some(method), Some(id)) => {
            // A server-initiated request. Only roots/list is answered.
            let reply = if method == "roots/list" {
                json!({"jsonrpc":"2.0","id":id,"result":{"roots":[{"uri":shared.root_uri,"name":"mod"}]}})
            } else {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not supported by the host"}})
            };
            let _ = shared.write_message(&reply).await;
        }
        (Some(_), None) => {
            shared.notifications.fetch_add(1, Ordering::Relaxed);
        }
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else {
                shared.unknown_responses.fetch_add(1, Ordering::Relaxed);
                return;
            };
            let tx = shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pending
                .remove(&id);
            let Some(tx) = tx else {
                shared.unknown_responses.fetch_add(1, Ordering::Relaxed);
                return;
            };
            let outcome = if let Some(error) = message.get("error") {
                Err(McpError::ServerError {
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(2000)
                        .collect(),
                })
            } else {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            };
            let _ = tx.send(outcome);
        }
        (None, None) => {
            shared.unknown_responses.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(unix)]
fn kill_group(pid: Option<u32>) {
    if let Some(pid) = pid.and_then(|p| i32::try_from(p).ok()) {
        // SAFETY: killpg only sends a signal; the group id is the pid of a child we created with
        // process_group(0), and a group that is already gone yields ESRCH, which we ignore.
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: Option<u32>) {}

impl McpClient {
    /// Spawns the server and completes the `initialize` handshake.
    pub async fn start(spec: &McpServerSpec) -> Result<(McpClient, Handshake), McpError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .env_clear()
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|e| McpError::Spawn {
            message: e.to_string(),
        })?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(McpError::Spawn {
                message: "child stdio was not piped".into(),
            });
        };
        let shared = Arc::new(Shared {
            state: StdMutex::new(State {
                pending: HashMap::new(),
                ended: None,
            }),
            stdin: Mutex::new(Some(stdin)),
            pid: child.id(),
            child: Mutex::new(Some(child)),
            stderr: StdMutex::new(VecDeque::new()),
            root_uri: file_uri(&spec.cwd),
            unknown_responses: AtomicU64::new(0),
            notifications: AtomicU64::new(0),
        });
        tokio::spawn(stderr_task(shared.clone(), stderr));
        tokio::spawn(reader_task(shared.clone(), stdout, spec.max_frame_bytes));
        let client = McpClient {
            shared,
            next_id: AtomicU64::new(1),
            call_timeout: spec.default_call_timeout,
        };
        match client.handshake(spec.startup_timeout).await {
            Ok(handshake) => Ok((client, handshake)),
            Err(error) => {
                let _ = client.shutdown().await;
                Err(error)
            }
        }
    }

    async fn handshake(&self, timeout: Duration) -> Result<Handshake, McpError> {
        let params = json!({
            "protocolVersion": OFFERED_PROTOCOL_VERSION,
            "capabilities": {"roots": {}},
            "clientInfo": {"name": "hoi4-factory", "version": env!("CARGO_PKG_VERSION")},
        });
        let result = self
            .request("initialize", params, timeout)
            .await
            .map_err(|e| match e {
                McpError::Timeout { .. } => McpError::Handshake {
                    message: "initialize timed out".into(),
                },
                McpError::ServerError { code, message } => McpError::Handshake {
                    message: format!("initialize refused ({code}): {message}"),
                },
                other => other,
            })?;
        let version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&version) {
            return Err(McpError::Handshake {
                message: format!("unsupported protocol version {version:?}"),
            });
        }
        let info = result.get("serverInfo");
        let text = |key: &str| {
            info.and_then(|i| i.get(key))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        let handshake = Handshake {
            protocol_version: version.to_owned(),
            server_name: text("name"),
            server_version: text("version"),
            capabilities: result.get("capabilities").cloned().unwrap_or(Value::Null),
            instructions_sha256: result
                .get("instructions")
                .and_then(Value::as_str)
                .map(|s| hex(&Sha256::digest(s.as_bytes()))),
        };
        self.shared
            .write_message(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        Ok(handshake)
    }

    async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(error) = &state.ended {
                return Err(error.clone());
            }
            state.pending.insert(id, tx);
        }
        let message = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        if let Err(error) = self.shared.write_message(&message).await {
            self.remove_pending(id);
            // A write failure usually means the process died; prefer the reader's explanation.
            return Err(self.shared.ended_error().unwrap_or(error));
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => Err(self.shared.ended_error().unwrap_or(McpError::Transport {
                message: "response channel dropped".into(),
            })),
            Err(_) => {
                self.remove_pending(id);
                let sent = self
                    .shared
                    .write_message(&json!({
                        "jsonrpc":"2.0",
                        "method":"notifications/cancelled",
                        "params":{"requestId":id,"reason":"host timeout"}
                    }))
                    .await
                    .is_ok();
                Err(McpError::Timeout { sent })
            }
        }
    }

    fn remove_pending(&self, id: u64) {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .remove(&id);
    }

    /// Lists every tool, following `nextCursor`.
    pub async fn list_tools(&self) -> Result<Vec<DiscoveredTool>, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..64 {
            let params = match &cursor {
                Some(c) => json!({"cursor": c}),
                None => json!({}),
            };
            let result = self
                .request("tools/list", params, self.call_timeout)
                .await?;
            let items = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::Protocol {
                    message: "tools/list result has no tools array".into(),
                })?;
            for item in items {
                let name =
                    item.get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| McpError::Protocol {
                            message: "a listed tool has no name".into(),
                        })?;
                let schema = item.get("inputSchema").cloned().unwrap_or(Value::Null);
                tools.push(DiscoveredTool {
                    name: name.to_owned(),
                    description: item
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    schema_sha256: canonical_sha256(&schema),
                    input_schema: schema,
                    annotations: item.get("annotations").cloned().unwrap_or(Value::Null),
                });
            }
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        Err(McpError::Protocol {
            message: "tools/list pagination did not end".into(),
        })
    }

    /// Calls a tool. On timeout the request is cancelled and the outcome is unknown.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        timeout: Option<Duration>,
    ) -> Result<ToolOutput, McpError> {
        let result = self
            .request(
                "tools/call",
                json!({"name": name, "arguments": arguments}),
                timeout.unwrap_or(self.call_timeout),
            )
            .await?;
        let bytes = serde_json::to_vec(&result).map(|v| v.len()).unwrap_or(0);
        let content = match result.get("content") {
            Some(Value::Array(items)) => items.clone(),
            None | Some(Value::Null) => Vec::new(),
            Some(_) => {
                return Err(McpError::Protocol {
                    message: "tools/call content is not an array".into(),
                });
            }
        };
        Ok(ToolOutput {
            content,
            structured: result.get("structuredContent").cloned(),
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            bytes,
            truncated: false,
        })
    }

    /// True while the session has not ended.
    pub fn is_alive(&self) -> bool {
        self.shared.ended_error().is_none()
    }

    /// Why the session ended, if it did.
    pub fn ended(&self) -> Option<McpError> {
        self.shared.ended_error()
    }

    pub fn pid(&self) -> Option<u32> {
        self.shared.pid
    }

    pub fn stderr_tail(&self) -> String {
        self.shared.stderr_tail()
    }

    /// Server messages that matched no pending request, for diagnostics.
    pub fn unknown_response_count(&self) -> u64 {
        self.shared.unknown_responses.load(Ordering::Relaxed)
    }

    /// Closes stdin, waits briefly, then kills the whole process group.
    pub async fn shutdown(self) -> ShutdownReport {
        self.shared.stdin.lock().await.take();
        let mut child = self.shared.child.lock().await.take();
        let mut killed = false;
        let mut exit_status = None;
        if let Some(child) = child.as_mut() {
            match tokio::time::timeout(SHUTDOWN_GRACE, child.wait()).await {
                Ok(Ok(status)) => exit_status = status.code(),
                _ => {
                    killed = true;
                    kill_group(self.shared.pid);
                    let _ = child.start_kill();
                    exit_status = child.wait().await.ok().and_then(|s| s.code());
                }
            }
        }
        // The leader may be gone while helpers it spawned still run in its group.
        kill_group(self.shared.pid);
        self.shared.end(McpError::Transport {
            message: "client shut down".into(),
        });
        ShutdownReport {
            exit_status,
            killed,
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        kill_group(self.shared.pid);
    }
}

async fn stderr_task(shared: Arc<Shared>, mut stderr: tokio::process::ChildStderr) {
    let mut chunk = [0u8; 2048];
    loop {
        match stderr.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                let mut ring = shared.stderr.lock().unwrap_or_else(|e| e.into_inner());
                ring.extend(&chunk[..n]);
                while ring.len() > STDERR_RING_BYTES {
                    ring.pop_front();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hash_ignores_key_order() {
        let a = json!({"b":1,"a":{"y":[1,2],"x":null}});
        let b: Value = serde_json::from_str(r#"{"a":{"x":null,"y":[1,2]},"b":1}"#).unwrap();
        assert_eq!(canonical_sha256(&a), canonical_sha256(&b));
        assert_ne!(canonical_sha256(&a), canonical_sha256(&json!({"a":1})));
    }

    #[test]
    fn file_uri_escapes() {
        assert_eq!(file_uri(std::path::Path::new("/a b/c")), "file:///a%20b/c");
    }

    #[tokio::test]
    async fn frame_reader_bounds_lines() {
        let data = b"abc\nabcdefghij\n";
        let mut r = BufReader::new(&data[..]);
        assert_eq!(read_frame(&mut r, 5).await.ok().flatten().unwrap(), b"abc");
        assert!(matches!(
            read_frame(&mut r, 5).await,
            Err(FrameError::TooLong)
        ));
    }
}
