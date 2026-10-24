//! Minimal local HTTP/1.1 server that replays a recorded real OpenAI stream
//! with an injected fault, so the thin adapter and a third-party crate can be
//! compared through their real HTTP stacks. Binds 127.0.0.1 only.
//!
//! Path selects the fault: /<fault>/v1/chat/completions where fault is one of
//! none, truncate-mid-args, drop-finish-done, malformed-args, disconnect-after-tool-call.

use factory_agent::transport::{ByteSource, Fault, Replay};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::util::Recording;

pub fn fault_for(name: &str, rec: &Recording) -> Option<Fault> {
    let args = crate::provider::tool_arg_events(rec);
    let (ms, me, _) = args[args.len() / 2].clone();
    let (_, last_end, last_data) = args.last().unwrap().clone();
    let (_, _, fin) = crate::provider::finish_event(rec).unwrap();
    Some(match name {
        "none" => Fault::None,
        "truncate-mid-args" => Fault::TruncateAt(ms + (me - ms) / 2),
        "drop-finish-done" => Fault::DropEvents(vec![fin, crate::provider::done_needle(rec)]),
        "malformed-args" => Fault::DropEvents(vec![last_data]),
        "disconnect-after-tool-call" => Fault::DisconnectAt(last_end),
        _ => return None,
    })
}

pub async fn serve(port: u16) {
    let rec = std::sync::Arc::new(Recording::load("openai-tool-turn"));
    let l = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    eprintln!("replay server on 127.0.0.1:{port}");
    loop {
        let (mut s, _) = l.accept().await.unwrap();
        let rec = rec.clone();
        tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let header_end = loop {
                let n = s.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let len: usize = head
                .lines()
                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                .unwrap_or(0);
            while buf.len() < header_end + len {
                let n = s.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let fault_name = path.trim_start_matches('/').split('/').next().unwrap_or("none").to_owned();
            let Some(fault) = fault_for(&fault_name, &rec) else {
                let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
                return;
            };
            eprintln!("serve {path} fault={fault:?}");
            let _ = s
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream; charset=utf-8\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n")
                .await;
            let mut src = ByteSource::Replay(Replay::new(rec.bytes.clone().into(), Some(rec.chunks.clone()), &fault));
            loop {
                match src.next_chunk().await {
                    Some(Ok(b)) => {
                        let mut frame = format!("{:x}\r\n", b.len()).into_bytes();
                        frame.extend_from_slice(&b);
                        frame.extend_from_slice(b"\r\n");
                        if s.write_all(&frame).await.is_err() {
                            return;
                        }
                        let _ = s.flush().await;
                    }
                    Some(Err(_)) => {
                        // Abortive close: RST instead of a clean chunked end.
                        #[allow(deprecated)]
                        let _ = s.set_linger(Some(std::time::Duration::ZERO));
                        drop(s);
                        return;
                    }
                    None => {
                        let _ = s.write_all(b"0\r\n\r\n").await;
                        let _ = s.shutdown().await;
                        return;
                    }
                }
            }
        });
    }
}
