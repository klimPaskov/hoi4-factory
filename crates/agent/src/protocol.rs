//! Versioned framed protocol between the host and a supervised worker process.
//!
//! Each frame is a 4-byte big-endian length followed by that many bytes of
//! UTF-8 JSON. Frames above [`MAX_FRAME`] are a protocol violation, which also
//! catches unsolicited non-frame output on the structured channel. Diagnostics
//! go to stderr, never to the frame channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::destination::DestinationPolicy;
use crate::errors::NormalizedError;
use crate::profile::ModelProfile;
use crate::request::ModelRequest;
use crate::types::{Turn, TurnError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Version 2 adds `Cancel` and the typed `RunTurnPayload` / `TurnOutcome`.
pub const PROTOCOL_VERSION: u32 = 2;
pub const SCHEMA_VERSION: u32 = 1;
/// 16 MiB: a long conversation with large tool results fits (a 100k-token context is about
/// 400 KiB of text). Context compaction (S05) keeps requests far below this.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    pub schema_version: u32,
    pub worker_generation: u64,
    pub request_id: String,
    pub attempt_id: String,
    #[serde(flatten)]
    pub body: Body,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Hello {
        protocol_version: u32,
        runtime_build: String,
        adapters: Vec<String>,
    },
    /// Host asks the worker to run one model turn.
    RunTurn {
        payload: Value,
    },
    /// Worker returns the turn outcome. The host still validates any tool call.
    TurnResult {
        payload: Value,
    },
    /// Host asks the worker to stop the turn named by the frame's request and
    /// attempt IDs. A cooperative worker answers with a `TurnResult` whose
    /// error is `Cancelled`; the host kills a worker that does not.
    Cancel,
    Shutdown,
}

/// The API credential for one turn. It travels inside the `RunTurn` frame
/// (never in argv or the environment of the worker), so it serializes, but it
/// never prints.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credential(String);

impl Credential {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }
    pub fn into_api_key(self) -> crate::openai::ApiKey {
        crate::openai::ApiKey::new(self.0)
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Payload of `Body::RunTurn`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunTurnPayload {
    pub profile: ModelProfile,
    pub credential: Credential,
    pub request: ModelRequest,
    pub policy: DestinationPolicy,
    pub max_response_bytes: u64,
}

/// A failed turn with both the detailed error and its envelope view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnErrorReport {
    pub error: TurnError,
    pub normalized: NormalizedError,
}

impl From<TurnError> for TurnErrorReport {
    fn from(error: TurnError) -> Self {
        let normalized = error.normalized();
        Self { error, normalized }
    }
}

/// Payload of `Body::TurnResult`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnOutcome {
    pub result: Result<Turn, TurnErrorReport>,
    pub elapsed_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("channel closed")]
    Eof,
    #[error("frame length {0} exceeds maximum {MAX_FRAME}")]
    TooLarge(usize),
    #[error("frame is not valid protocol JSON: {0}")]
    BadJson(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, f: &Frame) -> Result<usize, FrameError> {
    let bytes = serde_json::to_vec(f).map_err(|e| FrameError::BadJson(e.to_string()))?;
    if bytes.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(bytes.len()));
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(bytes.len() + 4)
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Frame, FrameError> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(FrameError::Eof),
        Err(e) => return Err(e.into()),
    }
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(FrameError::TooLarge(n));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            FrameError::Eof
        } else {
            FrameError::Io(e)
        }
    })?;
    serde_json::from_slice(&buf).map_err(|e| FrameError::BadJson(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_and_oversize() {
        let f = Frame {
            schema_version: SCHEMA_VERSION,
            worker_generation: 3,
            request_id: "r".into(),
            attempt_id: "a".into(),
            body: Body::Shutdown,
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &f).await.unwrap();
        let back = read_frame(&mut buf.as_slice()).await.unwrap();
        assert_eq!(back, f);
        let junk = b"hello world\n".to_vec();
        assert!(matches!(
            read_frame(&mut junk.as_slice()).await,
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn credential_serializes_but_never_prints() {
        let p = RunTurnPayload {
            profile: ModelProfile::anthropic_haiku(),
            credential: Credential::new("sk-test-secret-value"),
            request: ModelRequest {
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_output_tokens: 1,
            },
            policy: DestinationPolicy::default(),
            max_response_bytes: 1,
        };
        assert!(!format!("{p:?}").contains("sk-test-secret-value"));
        assert!(format!("{p:?}").contains("[REDACTED]"));
        let wire = serde_json::to_value(&p).unwrap();
        assert_eq!(wire["credential"], "sk-test-secret-value");
        let back: RunTurnPayload = serde_json::from_value(wire).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn outcome_roundtrips_both_arms() {
        let err = TurnOutcome {
            result: Err(TurnError::Cancelled {
                partial: Default::default(),
            }
            .into()),
            elapsed_ms: 5,
        };
        let back: TurnOutcome =
            serde_json::from_value(serde_json::to_value(&err).unwrap()).unwrap();
        assert_eq!(back, err);
        assert_eq!(back.result.unwrap_err().normalized.code, "cancelled");
    }
}
