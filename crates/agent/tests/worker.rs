//! Supervision tests through the real `factory-agent-worker` binary.
//!
//! The worker faults used here are debug-build test hooks (see
//! `factory_agent::worker`). Provider traffic is recorded real Anthropic bytes
//! served by the local replay server.

mod common;

use std::time::Duration;

use common::*;
use factory_agent::errors::RetryClass;
use factory_agent::protocol::*;
use factory_agent::supervisor::{CancelOutcome, SupervisorError, WorkerProcess};
use factory_agent::testing::{Recorded, ReplayServer, Reply};
use factory_agent::transport::Fault;
use factory_agent::types::TurnError;
use tokio::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_factory-agent-worker");
const SECRET: &str = "sk-test-secret-not-real-0123456789";

fn command(generation: u64, fault: Option<&str>) -> Command {
    let mut c = Command::new(BIN);
    c.arg("--generation").arg(generation.to_string());
    match fault {
        Some(f) => c.env(factory_agent::worker::TEST_FAULT_ENV, f),
        None => c.env_remove(factory_agent::worker::TEST_FAULT_ENV),
    };
    c
}

async fn spawn(generation: u64, fault: Option<&str>) -> WorkerProcess {
    WorkerProcess::spawn(
        command(generation, fault),
        generation,
        Duration::from_secs(10),
    )
    .await
    .expect("worker spawns")
}

fn payload(base_url: &str) -> RunTurnPayload {
    RunTurnPayload {
        profile: anthropic_profile(base_url),
        credential: Credential::new(SECRET),
        request: tool_request(),
        policy: loopback_policy(),
        max_response_bytes: 8 * 1024 * 1024,
    }
}

async fn tool_server(stall: bool) -> ReplayServer {
    let rec = Recorded::load("anthropic-tool-turn");
    let reply = if stall {
        Reply::Sse {
            bytes: rec.bytes.clone(),
            chunk_sizes: Some(rec.chunk_sizes.clone()),
            fault: Fault::None,
            stall_after_bytes: Some(1),
        }
    } else {
        rec.reply(Fault::None)
    };
    ReplayServer::start(vec![("/v1/messages", reply)]).await
}

#[tokio::test]
async fn handshake_then_orderly_shutdown() {
    let mut w = spawn(7, None).await;
    assert_eq!(w.generation(), 7);
    assert!(w.pid().is_some());
    let (exit, signal) = w.shutdown().await;
    assert_eq!((exit, signal), (Some(0), None));
}

#[tokio::test]
async fn an_incompatible_protocol_version_is_refused_before_any_task() {
    let e = WorkerProcess::spawn(command(1, Some("old_protocol")), 1, Duration::from_secs(10))
        .await
        .err()
        .expect("refused");
    assert!(
        matches!(e, SupervisorError::IncompatibleWorker { expected: PROTOCOL_VERSION, got } if got == PROTOCOL_VERSION - 1),
        "{e}"
    );
}

#[tokio::test]
async fn a_hello_for_another_generation_is_refused() {
    // A generation mismatch makes the worker's Hello not match what the host expects.
    let e = WorkerProcess::spawn(command(1, None), 2, Duration::from_secs(10))
        .await
        .err()
        .expect("refused");
    assert!(matches!(e, SupervisorError::Protocol(_)), "{e}");
}

#[tokio::test]
async fn a_turn_against_the_replay_server() {
    let server = tool_server(false).await;
    let mut w = spawn(1, None).await;
    let outcome = w
        .run_turn(
            "req-1",
            "att-1",
            &payload(&server.base_url),
            Duration::from_secs(20),
        )
        .await
        .expect("outcome");
    let turn = outcome.result.expect("turn");
    assert_eq!(turn.tool_calls[0].name, "write_file");
    assert_eq!(turn.usage.as_ref().map(|u| u.output_tokens), Some(84));
    assert_eq!(server.request_count("/v1/messages"), 1);
    assert_eq!(w.stale_frames_rejected(), 0);
    // The worker can serve another turn on the same process.
    let again = w
        .run_turn(
            "req-2",
            "att-2",
            &payload(&server.base_url),
            Duration::from_secs(20),
        )
        .await
        .unwrap();
    assert!(again.result.is_ok());
    assert!(
        !w.stderr_tail().contains(SECRET),
        "the credential never reaches diagnostics"
    );
    w.shutdown().await;
}

#[tokio::test]
async fn a_refused_destination_comes_back_as_a_normalized_error() {
    let server = tool_server(false).await;
    let mut p = payload(&server.base_url);
    p.policy = factory_agent::destination::DestinationPolicy::default();
    let mut w = spawn(1, None).await;
    let o = w
        .run_turn("r", "a", &p, Duration::from_secs(20))
        .await
        .unwrap();
    let rep = o.result.unwrap_err();
    assert_eq!(rep.normalized.code, "destination_blocked");
    assert_eq!(server.request_count("/v1/messages"), 0);
    w.shutdown().await;
}

#[tokio::test]
async fn cooperative_cancel_returns_a_cancelled_result() {
    let server = tool_server(true).await;
    let mut w = spawn(1, None).await;
    let p = payload(&server.base_url);
    // The host-side signal drops the pending call, then asks the worker to stop.
    let waited = tokio::time::timeout(
        Duration::from_millis(500),
        w.run_turn("req-c", "att-c", &p, Duration::from_secs(30)),
    )
    .await;
    assert!(waited.is_err(), "the stalled stream keeps the turn open");
    match w.cancel(Duration::from_secs(10)).await {
        CancelOutcome::Cooperative { outcome } => {
            let rep = outcome.result.unwrap_err();
            assert!(matches!(rep.error, TurnError::Cancelled { .. }));
            assert_eq!(rep.normalized.retry_class, RetryClass::ReconcileFirst);
        }
        other => panic!("expected a cooperative cancel, got {other:?}"),
    }
    assert_eq!(server.request_count("/v1/messages"), 1);
    // The worker is still usable.
    let fresh = tool_server(false).await;
    assert!(
        w.run_turn(
            "req-d",
            "att-d",
            &payload(&fresh.base_url),
            Duration::from_secs(20)
        )
        .await
        .unwrap()
        .result
        .is_ok()
    );
    w.shutdown().await;
}

#[tokio::test]
async fn a_worker_that_ignores_cancel_is_killed_with_exit_evidence() {
    let server = tool_server(true).await;
    let mut w = spawn(1, Some("ignore_cancel")).await;
    let p = payload(&server.base_url);
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        w.run_turn("req-k", "att-k", &p, Duration::from_secs(30)),
    )
    .await;
    let grace = Duration::from_millis(400);
    match w.cancel(grace).await {
        CancelOutcome::Killed {
            exit,
            signal,
            after_ms,
        } => {
            assert!(
                after_ms >= 400,
                "the grace period was honored: {after_ms} ms"
            );
            assert!(
                exit.is_some() || signal.is_some(),
                "termination evidence is returned"
            );
            #[cfg(unix)]
            assert_eq!(signal, Some(9));
        }
        other => panic!("expected Killed, got {other:?}"),
    }
    // The dead worker is reported, not silently reused.
    let e = w
        .run_turn(
            "req-k2",
            "att-k2",
            &payload(&server.base_url),
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            e,
            SupervisorError::WorkerExited { .. } | SupervisorError::Protocol(_)
        ),
        "{e}"
    );
    assert_eq!(server.request_count("/v1/messages"), 1);
}

#[tokio::test]
async fn a_worker_that_exits_mid_turn_is_reported_with_its_exit_code() {
    let server = tool_server(false).await;
    let mut w = spawn(1, Some("exit_mid_turn")).await;
    let e = w
        .run_turn(
            "req-x",
            "att-x",
            &payload(&server.base_url),
            Duration::from_secs(20),
        )
        .await
        .unwrap_err();
    match e {
        SupervisorError::WorkerExited {
            exit, stderr_tail, ..
        } => {
            assert_eq!(exit, Some(3));
            assert!(stderr_tail.contains("exit_mid_turn"), "{stderr_tail}");
            assert!(!stderr_tail.contains(SECRET));
        }
        other => panic!("expected WorkerExited, got {other}"),
    }
    assert_eq!(
        server.request_count("/v1/messages"),
        1,
        "the accepted request was not repeated"
    );
}

#[tokio::test]
async fn a_stale_generation_result_is_rejected_and_counted_never_accepted() {
    let server = tool_server(false).await;
    let mut w = spawn(5, Some("stale_generation")).await;
    let e = w
        .run_turn(
            "req-s",
            "att-s",
            &payload(&server.base_url),
            Duration::from_secs(3),
        )
        .await
        .unwrap_err();
    assert!(matches!(e, SupervisorError::Timeout), "{e}");
    assert_eq!(w.stale_frames_rejected(), 1);
    w.shutdown().await;
}

#[tokio::test]
async fn a_late_result_for_an_abandoned_request_is_rejected_and_counted() {
    let server = tool_server(false).await;
    let mut w = spawn(1, None).await;
    let p = payload(&server.base_url);
    // The host gives up on "old" (its future is dropped) but the worker still answers it.
    let _ = tokio::time::timeout(
        Duration::from_millis(1),
        w.run_turn("old", "old-att", &p, Duration::from_secs(30)),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let o = w
        .run_turn("new", "new-att", &p, Duration::from_secs(20))
        .await
        .unwrap();
    assert!(o.result.is_ok());
    assert_eq!(
        w.stale_frames_rejected(),
        1,
        "the answer to the abandoned request was dropped"
    );
    w.shutdown().await;
}

/// Issue #13: a caller that drops `run_turn` while a large frame is still being written must
/// never leave half a frame on the worker's stdin. Each round drops a multi-megabyte turn after
/// a different short delay, then follows the documented contract (cancel after a dropped
/// turn). The cancel must be answered cooperatively and the next turn on the same worker must
/// succeed. Before the fix the cut frame made the worker exit with "frame is not valid
/// protocol JSON".
#[tokio::test]
async fn dropping_run_turn_mid_write_never_corrupts_the_next_frame() {
    let server = tool_server(false).await;
    let mut w = spawn(1, None).await;
    let mut big = payload(&server.base_url);
    big.request.system = "x".repeat(4 * 1024 * 1024);
    let rounds = 24u64;
    for i in 0..rounds {
        let delay = Duration::from_micros((i * 397) % 3000);
        let _ = tokio::time::timeout(
            delay,
            w.run_turn(
                &format!("drop-{i}"),
                "drop-att",
                &big,
                Duration::from_secs(30),
            ),
        )
        .await;
        match w.cancel(Duration::from_secs(20)).await {
            CancelOutcome::Cooperative { .. } => {}
            other => panic!("round {i}: the worker did not answer the cancel: {other:?}"),
        }
        let o = w
            .run_turn(
                &format!("next-{i}"),
                "next-att",
                &payload(&server.base_url),
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|e| panic!("round {i}: the turn after a dropped write failed: {e}"));
        assert!(o.result.is_ok(), "round {i}");
    }
    assert!(w.stale_frames_rejected() <= rounds);
    let (exit, _) = w.shutdown().await;
    assert_eq!(exit, Some(0), "the worker never saw a corrupt frame");
}

#[tokio::test]
async fn an_oversize_outgoing_frame_is_refused_by_the_host_and_the_worker_survives() {
    let server = tool_server(false).await;
    let mut w = spawn(1, None).await;
    let mut p = payload(&server.base_url);
    p.request.system = "x".repeat(MAX_FRAME + 10);
    let e = w
        .run_turn("big", "big-att", &p, Duration::from_secs(5))
        .await
        .unwrap_err();
    assert!(matches!(e, SupervisorError::Protocol(_)), "{e}");
    assert_eq!(server.request_count("/v1/messages"), 0);
    let ok = w
        .run_turn(
            "small",
            "small-att",
            &payload(&server.base_url),
            Duration::from_secs(20),
        )
        .await
        .unwrap();
    assert!(ok.result.is_ok());
    w.shutdown().await;
}

#[tokio::test]
async fn an_oversize_incoming_frame_makes_the_worker_exit() {
    use tokio::io::AsyncWriteExt;
    let mut child = command(1, None)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&(u32::MAX).to_be_bytes()).await.unwrap();
    stdin.flush().await.unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("worker exits")
        .unwrap();
    assert!(!status.success());
    let mut err = String::new();
    use tokio::io::AsyncReadExt;
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .await
        .unwrap();
    assert!(err.contains("protocol violation"), "{err}");
}

#[tokio::test]
async fn junk_on_stdin_is_a_protocol_violation_not_a_frame() {
    use tokio::io::AsyncWriteExt;
    let mut child = command(1, None)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"hello world\n").await.unwrap();
    stdin.flush().await.unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("worker exits")
        .unwrap();
    assert!(!status.success());
}

/// Reads the helper pid the worker logged to stderr (`grandchild <pid>`).
async fn grandchild_pid(w: &WorkerProcess) -> u32 {
    for _ in 0..100 {
        if let Some(pid) = w
            .stderr_tail()
            .lines()
            .find_map(|l| l.split("grandchild ").nth(1))
            .and_then(|p| p.trim().parse().ok())
        {
            return pid;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the worker never logged a grandchild: {}", w.stderr_tail());
}

/// Whether `pid` is a live process (a zombie or an exited process counts as gone).
fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist");
        String::from_utf8_lossy(&out.stdout).contains(&format!("\",\"{pid}\",\""))
    }
    #[cfg(unix)]
    {
        // SAFETY: signal 0 only checks that the process exists.
        let exists = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        let zombie = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(')')
                    .map(|(_, rest)| rest.trim_start().starts_with('Z'))
            })
            .unwrap_or(false);
        exists && !zombie
    }
}

async fn assert_gone(pid: u32, what: &str) {
    for _ in 0..50 {
        if !process_alive(pid) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{what} (pid {pid}) is still running");
}

#[tokio::test]
async fn cancel_kills_the_workers_whole_process_tree() {
    let server = tool_server(true).await;
    let mut w = spawn(1, Some("ignore_cancel_grandchild")).await;
    let worker = w.pid().expect("worker pid");
    let helper = grandchild_pid(&w).await;
    assert!(process_alive(helper), "the helper starts alive");
    let p = payload(&server.base_url);
    let _ = tokio::time::timeout(
        Duration::from_millis(500),
        w.run_turn("req-t", "att-t", &p, Duration::from_secs(30)),
    )
    .await;
    assert!(matches!(
        w.cancel(Duration::from_millis(400)).await,
        CancelOutcome::Killed { .. }
    ));
    assert_gone(worker, "the worker").await;
    assert_gone(helper, "the worker's helper process").await;
}

#[tokio::test]
async fn shutdown_leaves_no_helper_process_behind() {
    let mut w = spawn(1, Some("grandchild")).await;
    let helper = grandchild_pid(&w).await;
    assert!(process_alive(helper), "the helper starts alive");
    let worker = w.pid().expect("worker pid");
    w.shutdown().await;
    assert_gone(worker, "the worker").await;
    assert_gone(helper, "the worker's helper process").await;
}
