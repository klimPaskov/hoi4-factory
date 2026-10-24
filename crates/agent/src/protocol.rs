//! Versioned framed protocol between the host and a supervised worker process.
//!
//! Each frame is a 4-byte big-endian length followed by that many bytes of
//! UTF-8 JSON. Frames above [`MAX_FRAME`] are a protocol violation, which also
//! catches unsolicited non-frame output on the structured channel. Diagnostics
//! go to stderr, never to the frame channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u32 = 1;
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_FRAME: usize = 1024 * 1024;

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
    RunTurn { payload: Value },
    /// Worker returns the turn outcome. The host still validates any tool call.
    TurnResult { payload: Value },
    Shutdown,
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
        assert!(matches!(read_frame(&mut junk.as_slice()).await, Err(FrameError::TooLarge(_))));
    }
}
