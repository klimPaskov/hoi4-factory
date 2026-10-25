//! Test support: a local HTTP server that replays recorded real provider
//! bytes with injected faults, and refusals, listings and redirects.
//!
//! Evidence kind for anything served from recorded bytes is
//! `recorded-real-replay`. Faults are injected into real bytes, never into
//! invented model output. The server binds `127.0.0.1` only and counts the
//! requests it receives per path, so a test can assert that a failed turn was
//! not sent twice. Tests that use it build a [`DestinationPolicy`] that
//! explicitly allows `127.0.0.0/8`, which is the operator allowance path.
//!
//! [`DestinationPolicy`]: crate::destination::DestinationPolicy

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::transport::{ByteSource, Fault, Replay};

/// What a route answers.
#[derive(Debug, Clone)]
pub enum Reply {
    /// A 200 event stream of recorded bytes, with the recorded chunk sizes and
    /// a fault. `stall_after_bytes` keeps the connection open without sending
    /// more once that many bytes were sent (for cancellation tests).
    Sse {
        bytes: Vec<u8>,
        chunk_sizes: Option<Vec<usize>>,
        fault: Fault,
        stall_after_bytes: Option<usize>,
    },
    /// A refusal or any other JSON answer with a status.
    Json {
        status: u16,
        body: String,
    },
    Redirect {
        status: u16,
        location: String,
    },
}

impl Reply {
    pub fn sse(bytes: Vec<u8>, chunk_sizes: Option<Vec<usize>>, fault: Fault) -> Self {
        Reply::Sse {
            bytes,
            chunk_sizes,
            fault,
            stall_after_bytes: None,
        }
    }
    pub fn refusal(status: u16, body: &str) -> Self {
        Reply::Json {
            status,
            body: body.to_owned(),
        }
    }
    pub fn json(body: &str) -> Self {
        Reply::Json {
            status: 200,
            body: body.to_owned(),
        }
    }
}

/// A recorded real stream and its meta file.
pub struct Recorded {
    pub bytes: Vec<u8>,
    pub chunk_sizes: Vec<usize>,
    pub meta: serde_json::Value,
}

impl Recorded {
    /// Load `tests/fixtures/recorded/<name>.sse` and `.meta.json`.
    pub fn load(name: &str) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorded");
        let bytes = std::fs::read(dir.join(format!("{name}.sse")))
            .unwrap_or_else(|e| panic!("recording {name}: {e}"));
        let meta: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{name}.meta.json")))
                .unwrap_or_else(|e| panic!("meta {name}: {e}")),
        )
        .unwrap_or_else(|e| panic!("meta {name}: {e}"));
        let chunk_sizes = meta["chunk_sizes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            bytes,
            chunk_sizes,
            meta,
        }
    }

    pub fn reply(&self, fault: Fault) -> Reply {
        Reply::sse(self.bytes.clone(), Some(self.chunk_sizes.clone()), fault)
    }

    /// Byte offset just after the first SSE block that contains `needle`.
    pub fn end_of_event_containing(&self, needle: &str) -> usize {
        let text = String::from_utf8_lossy(&self.bytes);
        let mut off = 0;
        for block in text.split_inclusive("\n\n") {
            off += block.len();
            if block.contains(needle) {
                return off;
            }
        }
        panic!("no event contains {needle}");
    }
}

type Counts = Arc<Mutex<HashMap<String, u64>>>;
type Bodies = Arc<Mutex<HashMap<String, Vec<String>>>>;

pub struct ReplayServer {
    /// `http://127.0.0.1:<port>`
    pub base_url: String,
    counts: Counts,
    bodies: Bodies,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ReplayServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ReplayServer {
    /// Serve `routes` (exact path, reply) on an ephemeral loopback port.
    pub async fn start(routes: Vec<(&str, Reply)>) -> ReplayServer {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();
        let routes: Arc<HashMap<String, Reply>> =
            Arc::new(routes.into_iter().map(|(p, r)| (p.to_owned(), r)).collect());
        let counts: Counts = Default::default();
        let bodies: Bodies = Default::default();
        let (c, b) = (counts.clone(), bodies.clone());
        let task = tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    return;
                };
                let (routes, c, b) = (routes.clone(), c.clone(), b.clone());
                tokio::spawn(async move {
                    let _ = handle(sock, routes, c, b).await;
                });
            }
        });
        ReplayServer {
            base_url: format!("http://127.0.0.1:{port}"),
            counts,
            bodies,
            task,
        }
    }

    /// Requests received for `path` (query string ignored).
    pub fn request_count(&self, path: &str) -> u64 {
        self.counts
            .lock()
            .map(|m| m.get(path).copied().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Request bodies received for `path`, in order.
    pub fn request_bodies(&self, path: &str) -> Vec<String> {
        self.bodies
            .lock()
            .map(|m| m.get(path).cloned().unwrap_or_default())
            .unwrap_or_default()
    }
}

async fn handle(
    mut s: TcpStream,
    routes: Arc<HashMap<String, Reply>>,
    counts: Counts,
    bodies: Bodies,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = s.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let len: usize = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    while buf.len() < header_end + len {
        let n = s.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let target = head.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/").to_owned();
    if let (Ok(mut c), Ok(mut b)) = (counts.lock(), bodies.lock()) {
        *c.entry(path.clone()).or_default() += 1;
        b.entry(path.clone())
            .or_default()
            .push(String::from_utf8_lossy(&buf[header_end..]).into_owned());
    }
    let Some(reply) = routes.get(&path) else {
        return s
            .write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .await;
    };
    match reply {
        Reply::Json { status, body } => {
            let head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            s.write_all(head.as_bytes()).await?;
            s.write_all(body.as_bytes()).await?;
            s.shutdown().await
        }
        Reply::Redirect { status, location } => {
            let head = format!(
                "HTTP/1.1 {status} X\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            s.write_all(head.as_bytes()).await?;
            s.shutdown().await
        }
        Reply::Sse {
            bytes,
            chunk_sizes,
            fault,
            stall_after_bytes,
        } => {
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream; charset=utf-8\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n").await?;
            let mut src = ByteSource::Replay(Replay::new(
                Bytes::from(bytes.clone()),
                chunk_sizes.clone(),
                fault,
            ));
            let mut sent = 0usize;
            loop {
                match src.next_chunk().await {
                    Some(Ok(b)) => {
                        let mut frame = format!("{:x}\r\n", b.len()).into_bytes();
                        frame.extend_from_slice(&b);
                        frame.extend_from_slice(b"\r\n");
                        s.write_all(&frame).await?;
                        s.flush().await?;
                        sent += b.len();
                        if stall_after_bytes.is_some_and(|n| sent >= n) {
                            // Hold the connection open until the client goes away.
                            let mut sink = [0u8; 64];
                            loop {
                                match s.read(&mut sink).await {
                                    Ok(0) | Err(_) => return Ok(()),
                                    Ok(_) => {}
                                }
                            }
                        }
                    }
                    Some(Err(_)) => {
                        // Abortive close: RST instead of a clean chunked end.
                        #[allow(deprecated)]
                        let _ = s.set_linger(Some(std::time::Duration::ZERO));
                        return Ok(());
                    }
                    None => {
                        s.write_all(b"0\r\n\r\n").await?;
                        return s.shutdown().await;
                    }
                }
            }
        }
    }
}
