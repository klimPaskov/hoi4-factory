//! The worker side of the host/worker protocol.
//!
//! The worker is the process that talks to model providers. It holds a
//! credential only for the length of one turn, receives it inside the
//! `RunTurn` frame and never writes it anywhere. It makes exactly one provider
//! call per `RunTurn`, assembles the stream and returns the turn. The host
//! validates and executes tool calls, persists state and decides retries.
//!
//! Frames carry the worker generation. A frame addressed to another
//! generation is ignored with a stderr diagnostic. Diagnostics never go to
//! stdout, which is the frame channel.
//!
//! In debug builds only, the environment variable
//! `FACTORY_AGENT_WORKER_TEST_FAULT` injects worker faults for supervision
//! tests: `ignore_cancel` (never answer a cancel, hang), `exit_mid_turn` (exit
//! with code 3 once the provider accepted the request) and `stale_generation`
//! (stamp results with the previous generation) and `old_protocol` (report the
//! previous protocol version in `Hello`). Release builds do not read it.

use std::time::Instant;

use serde_json::json;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, watch};

use crate::client::ProviderClient;
use crate::protocol::*;
use crate::types::TurnError;

/// Env var read by debug builds for test faults.
pub const TEST_FAULT_ENV: &str = "FACTORY_AGENT_WORKER_TEST_FAULT";

#[cfg(debug_assertions)]
fn test_fault() -> Option<String> {
    std::env::var(TEST_FAULT_ENV).ok().filter(|v| !v.is_empty())
}

#[cfg(not(debug_assertions))]
fn test_fault() -> Option<String> {
    None
}

fn diag(msg: &str) {
    eprintln!("factory-agent-worker: {msg}");
}

type Inbound = Result<Frame, FrameError>;

async fn send_result<W: AsyncWrite + Unpin>(
    out: &mut W,
    generation: u64,
    f: &Frame,
    payload: serde_json::Value,
) -> Result<(), FrameError> {
    let stamp = if test_fault().as_deref() == Some("stale_generation") {
        generation.wrapping_sub(1)
    } else {
        generation
    };
    let frame = Frame {
        schema_version: SCHEMA_VERSION,
        worker_generation: stamp,
        request_id: f.request_id.clone(),
        attempt_id: f.attempt_id.clone(),
        body: Body::TurnResult { payload },
    };
    match write_frame(out, &frame).await {
        Err(FrameError::TooLarge(n)) => {
            diag(&format!(
                "result of {n} bytes exceeds the frame limit; sending a worker error"
            ));
            let frame = Frame {
                body: Body::TurnResult {
                    payload: json!({"worker_error": format!("result frame of {n} bytes exceeds the {MAX_FRAME} byte limit")}),
                },
                ..frame
            };
            write_frame(out, &frame).await.map(|_| ())
        }
        other => other.map(|_| ()),
    }
}

/// Run the worker on the given channel until `Shutdown`, end of input or a
/// protocol violation. `generation` is the durable worker generation the host
/// started this process with.
pub async fn serve<R, W>(stdin: R, mut stdout: W, generation: u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let to_io = |e: FrameError| std::io::Error::other(e.to_string());
    let hello = Frame {
        schema_version: SCHEMA_VERSION,
        worker_generation: generation,
        request_id: "hello".into(),
        attempt_id: "-".into(),
        body: Body::Hello {
            protocol_version: if test_fault().as_deref() == Some("old_protocol") {
                PROTOCOL_VERSION - 1
            } else {
                PROTOCOL_VERSION
            },
            runtime_build: crate::RUNTIME_BUILD.into(),
            adapters: vec![
                crate::anthropic::PROVIDER.into(),
                crate::openai::PROVIDER.into(),
            ],
        },
    };
    write_frame(&mut stdout, &hello).await.map_err(to_io)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<Inbound>();
    tokio::spawn(async move {
        let mut stdin = stdin;
        loop {
            let item = read_frame(&mut stdin).await;
            let stop = item.is_err();
            if tx.send(item).is_err() || stop {
                break;
            }
        }
    });

    loop {
        let frame = match rx.recv().await {
            None | Some(Err(FrameError::Eof)) => return Ok(()),
            Some(Err(e)) => {
                diag(&format!("protocol violation: {e}"));
                return Err(to_io(e));
            }
            Some(Ok(f)) => f,
        };
        if frame.worker_generation != generation {
            diag(&format!(
                "ignored frame for generation {} (this worker is generation {generation})",
                frame.worker_generation
            ));
            continue;
        }
        match &frame.body {
            Body::Shutdown => return Ok(()),
            Body::Cancel => diag("cancel received while idle; ignored"),
            Body::Hello { .. } | Body::TurnResult { .. } => {
                diag("unexpected frame kind from host; ignored")
            }
            Body::RunTurn { payload } => {
                let payload: RunTurnPayload = match serde_json::from_value(payload.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        diag(&format!("RunTurn payload rejected: {e}"));
                        send_result(
                            &mut stdout,
                            generation,
                            &frame,
                            json!({"worker_error": format!("RunTurn payload rejected: {e}")}),
                        )
                        .await
                        .map_err(to_io)?;
                        continue;
                    }
                };
                let (outcome, keep_running) = run_one(&frame, payload, generation, &mut rx).await;
                send_result(
                    &mut stdout,
                    generation,
                    &frame,
                    serde_json::to_value(&outcome)
                        .unwrap_or(json!({"worker_error": "outcome not serializable"})),
                )
                .await
                .map_err(to_io)?;
                if !keep_running {
                    return Ok(());
                }
            }
        }
    }
}

/// Run one turn while watching the channel for `Cancel`. Returns the outcome
/// and whether the worker should keep serving.
async fn run_one(
    frame: &Frame,
    payload: RunTurnPayload,
    generation: u64,
    rx: &mut mpsc::UnboundedReceiver<Inbound>,
) -> (TurnOutcome, bool) {
    let started = Instant::now();
    let done = |result, keep| {
        (
            TurnOutcome {
                result,
                elapsed_ms: started.elapsed().as_millis() as u64,
            },
            keep,
        )
    };
    let client = match ProviderClient::with_max_response_bytes(
        payload.profile.clone(),
        payload.credential.clone().into_api_key(),
        &payload.policy,
        payload.max_response_bytes,
    ) {
        Ok(c) => c,
        Err(r) => return done(Err(TurnError::Destination(r).into()), true),
    };
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let on_accepted = || {
        if test_fault().as_deref() == Some("exit_mid_turn") {
            diag("test fault exit_mid_turn: exiting after the provider accepted the request");
            std::process::exit(3);
        }
    };
    let fut = client.run_turn_with(&payload.request, cancel_rx, None, on_accepted);
    tokio::pin!(fut);
    let mut keep_running = true;
    let result = loop {
        tokio::select! {
            r = &mut fut => break r,
            item = rx.recv() => match item {
                Some(Ok(f)) if f.worker_generation != generation => {
                    diag(&format!("ignored frame for generation {} during a turn", f.worker_generation));
                }
                Some(Ok(f)) => match f.body {
                    Body::Cancel if f.request_id == frame.request_id && f.attempt_id == frame.attempt_id => {
                        if test_fault().as_deref() == Some("ignore_cancel") {
                            diag("test fault ignore_cancel: hanging after cancel");
                            std::future::pending::<()>().await;
                        }
                        let _ = cancel_tx.send(true);
                    }
                    Body::Cancel => diag("cancel for another request ignored"),
                    Body::Shutdown => {
                        keep_running = false;
                        let _ = cancel_tx.send(true);
                    }
                    _ => diag("frame ignored during a turn"),
                },
                Some(Err(e)) => {
                    diag(&format!("channel failed during a turn: {e}"));
                    keep_running = false;
                    let _ = cancel_tx.send(true);
                }
                None => {
                    keep_running = false;
                    let _ = cancel_tx.send(true);
                }
            }
        }
    };
    done(result.map_err(TurnErrorReport::from), keep_running)
}
