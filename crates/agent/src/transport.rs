//! Byte sources for provider streams: a live HTTP response (optionally teed to
//! a caller-supplied recording sink) or a replay of recorded real bytes with
//! injected faults.
//!
//! The crate never opens files. The caller hands in the sink or the bytes.

use bytes::Bytes;
use std::io::Write;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum TransportError {
    /// The connection failed before the request could have been accepted.
    #[error("connect failed before request was sent: {0}")]
    ConnectFailed(String),
    /// The request may have reached the provider. Acceptance is uncertain.
    #[error("transport failed after request may have been accepted: {0}")]
    AfterSend(String),
    /// The body stream broke after the provider accepted the request.
    #[error("body stream broke after acceptance: {0}")]
    BodyBroken(String),
    #[error("recording sink failed: {0}")]
    Recording(String),
    /// The response body passed the configured size cap. The connection is
    /// closed; the provider may already have charged for the request.
    #[error("response body exceeded the {0} byte cap")]
    ResponseTooLarge(u64),
}

/// Default cap on a provider response body (RT-11).
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// Records chunk boundaries and timing so replay can reproduce them.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ChunkLog {
    /// (byte length, microseconds since response headers)
    pub chunks: Vec<(usize, u64)>,
}

pub struct Tee {
    pub sink: Box<dyn Write + Send>,
    pub log: ChunkLog,
    started: Instant,
}

impl Tee {
    pub fn new(sink: Box<dyn Write + Send>) -> Self {
        Self {
            sink,
            log: ChunkLog::default(),
            started: Instant::now(),
        }
    }
    fn record(&mut self, b: &[u8]) -> Result<(), TransportError> {
        self.sink
            .write_all(b)
            .and_then(|_| self.sink.flush())
            .map_err(|e| TransportError::Recording(e.to_string()))?;
        self.log
            .chunks
            .push((b.len(), self.started.elapsed().as_micros() as u64));
        Ok(())
    }
}

/// A fault applied to recorded real bytes during replay.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Fault {
    None,
    /// End the stream cleanly after this many bytes.
    TruncateAt(usize),
    /// Return a transport error after this many bytes.
    DisconnectAt(usize),
    /// Remove every SSE event (data line block) whose text contains any needle.
    DropEvents(Vec<String>),
}

pub struct Replay {
    data: Bytes,
    boundaries: Vec<usize>,
    pos: usize,
    idx: usize,
    end: usize,
    error_at_end: bool,
}

impl Replay {
    /// `chunk_sizes` reproduces the recorded network chunking when given.
    pub fn new(recorded: Bytes, chunk_sizes: Option<Vec<usize>>, fault: &Fault) -> Self {
        let (data, sizes) = match fault {
            Fault::DropEvents(needles) => (drop_events(&recorded, needles), None),
            _ => (recorded, chunk_sizes),
        };
        let sizes = sizes.unwrap_or_else(|| vec![64; data.len().div_ceil(64)]);
        let (end, error_at_end) = match fault {
            Fault::TruncateAt(n) => ((*n).min(data.len()), false),
            Fault::DisconnectAt(n) => ((*n).min(data.len()), true),
            _ => (data.len(), false),
        };
        Self {
            data,
            boundaries: sizes,
            pos: 0,
            idx: 0,
            end,
            error_at_end,
        }
    }

    pub fn total_len(&self) -> usize {
        self.end
    }

    fn next(&mut self) -> Option<Result<Bytes, TransportError>> {
        if self.pos >= self.end {
            if self.error_at_end {
                self.error_at_end = false;
                return Some(Err(TransportError::BodyBroken(
                    "injected disconnect (connection reset) during replay".into(),
                )));
            }
            return None;
        }
        let want = self.boundaries.get(self.idx).copied().unwrap_or(64).max(1);
        self.idx += 1;
        let stop = (self.pos + want).min(self.end);
        let b = self.data.slice(self.pos..stop);
        self.pos = stop;
        Some(Ok(b))
    }
}

/// Split recorded bytes into blank-line separated SSE blocks and drop blocks
/// that contain a needle. Byte content of kept blocks is unchanged.
pub fn drop_events(data: &[u8], needles: &[String]) -> Bytes {
    let text = String::from_utf8_lossy(data);
    let norm = text.replace("\r\n", "\n");
    let mut out = String::new();
    for block in norm.split_inclusive("\n\n") {
        if needles.iter().any(|n| block.contains(n.as_str())) {
            continue;
        }
        out.push_str(block);
    }
    Bytes::from(out)
}

pub enum ByteSource {
    #[cfg(feature = "http")]
    Live {
        resp: reqwest::Response,
        tee: Option<Tee>,
        /// Body bytes read so far and the cap they may not pass.
        seen: u64,
        limit: u64,
    },
    Replay(Replay),
}

impl ByteSource {
    pub async fn next_chunk(&mut self) -> Option<Result<Bytes, TransportError>> {
        match self {
            #[cfg(feature = "http")]
            ByteSource::Live {
                resp,
                tee,
                seen,
                limit,
            } => match resp.chunk().await {
                Ok(Some(b)) => {
                    *seen += b.len() as u64;
                    if *seen > *limit {
                        return Some(Err(TransportError::ResponseTooLarge(*limit)));
                    }
                    if let Some(t) = tee.as_mut()
                        && let Err(e) = t.record(&b)
                    {
                        return Some(Err(e));
                    }
                    Some(Ok(b))
                }
                Ok(None) => None,
                Err(e) => Some(Err(TransportError::BodyBroken(e.to_string()))),
            },
            ByteSource::Replay(r) => r.next(),
        }
    }

    /// A live source for an accepted response. `limit` caps the body size.
    #[cfg(feature = "http")]
    pub fn live(resp: reqwest::Response, tee: Option<Tee>, limit: u64) -> Self {
        ByteSource::Live {
            resp,
            tee,
            seen: 0,
            limit,
        }
    }

    /// Take the chunk log out of a live teed source.
    pub fn take_chunk_log(&mut self) -> Option<ChunkLog> {
        match self {
            #[cfg(feature = "http")]
            ByteSource::Live { tee: Some(t), .. } => Some(std::mem::take(&mut t.log)),
            _ => None,
        }
    }
}

/// Read a whole (non-streaming) response body, refusing past `limit` bytes.
#[cfg(feature = "http")]
pub(crate) async fn read_capped(
    mut resp: reqwest::Response,
    limit: u64,
) -> Result<Vec<u8>, TransportError> {
    let mut out = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(b)) => {
                if (out.len() + b.len()) as u64 > limit {
                    return Err(TransportError::ResponseTooLarge(limit));
                }
                out.extend_from_slice(&b);
            }
            Ok(None) => return Ok(out),
            Err(e) => {
                return Err(TransportError::BodyBroken(crate::types::redact(
                    &e.to_string(),
                )));
            }
        }
    }
}

/// Map a failed `send()` to a turn error. A destination-policy refusal raised
/// by the connect-time resolver is found in the error's source chain.
#[cfg(feature = "http")]
pub(crate) fn map_send_error(e: reqwest::Error) -> crate::types::TurnError {
    use crate::types::{Acceptance, Partial, TurnError};
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(&e);
    while let Some(err) = cur {
        if let Some(r) = err.downcast_ref::<crate::destination::DestinationRefusal>() {
            return TurnError::Destination(r.clone());
        }
        cur = err.source();
    }
    let text = crate::types::redact(&e.to_string());
    let (error, acceptance) = if e.is_connect() {
        (TransportError::ConnectFailed(text), Acceptance::NotAccepted)
    } else {
        (TransportError::AfterSend(text), Acceptance::Uncertain)
    };
    TurnError::Transport {
        error,
        acceptance,
        partial: Partial::default(),
    }
}

/// A 3xx answer is never followed: a redirect could move the request to an
/// address the policy has not checked.
#[cfg(feature = "http")]
pub(crate) fn redirect_check(status: u16) -> Result<(), crate::types::TurnError> {
    if (300..400).contains(&status) {
        return Err(crate::types::TurnError::Destination(
            crate::destination::DestinationRefusal::new(
                "redirect_refused",
                format!("provider answered HTTP {status}; redirects are not followed"),
            ),
        ));
    }
    Ok(())
}
