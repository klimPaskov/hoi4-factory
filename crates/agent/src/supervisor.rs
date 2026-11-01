//! Host-side supervision of one worker process.
//!
//! The supervisor spawns the worker in its own process group with piped stdio
//! and `kill_on_drop`, reads the `Hello` handshake and refuses an incompatible
//! protocol version before any task is sent. It accepts a result only when the
//! worker generation, request ID and attempt ID all match the turn in flight;
//! every other frame is dropped and counted. A worker that ignores a cancel is
//! killed together with its process group and the exit status is returned as
//! termination evidence. The worker's stderr is kept in a bounded ring for
//! diagnostics and is never parsed.

use std::collections::VecDeque;
use std::process::ExitStatus;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::protocol::*;

/// Bytes of worker stderr kept for diagnostics.
pub const STDERR_RING_BYTES: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("worker could not be started: {0}")]
    Spawn(String),
    #[error("incompatible worker: protocol version {got}, host speaks {expected}")]
    IncompatibleWorker { expected: u32, got: u32 },
    #[error("worker did not answer in time")]
    Timeout,
    #[error("protocol violation: {0}")]
    Protocol(String),
    #[error("worker exited (code {exit:?}, signal {signal:?}); stderr tail: {stderr_tail}")]
    WorkerExited {
        exit: Option<i32>,
        signal: Option<i32>,
        stderr_tail: String,
    },
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CancelOutcome {
    /// The worker answered the cancel with a result in time.
    Cooperative { outcome: TurnOutcome },
    /// The worker did not answer within the grace period and was killed.
    Killed {
        exit: Option<i32>,
        signal: Option<i32>,
        after_ms: u64,
    },
    /// The worker was already gone.
    AlreadyExited {
        exit: Option<i32>,
        signal: Option<i32>,
    },
}

type Inbound = Result<Frame, String>;

pub struct WorkerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: mpsc::UnboundedReceiver<Inbound>,
    stderr: Arc<Mutex<VecDeque<u8>>>,
    generation: u64,
    stale_rejected: u64,
    in_flight: Option<(String, String)>,
    exited: bool,
}

fn status_parts(s: &ExitStatus) -> (Option<i32>, Option<i32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        (s.code(), s.signal())
    }
    #[cfg(not(unix))]
    {
        (s.code(), None)
    }
}

impl WorkerProcess {
    /// Spawn the worker, read its `Hello` within `handshake_timeout` and check
    /// the protocol version. `cmd` should run the worker with `--generation N`
    /// for the same `generation`. The credential never goes on its command line.
    pub async fn spawn(
        mut cmd: Command,
        generation: u64,
        handshake_timeout: Duration,
    ) -> Result<Self, SupervisorError> {
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        let (Some(mut stdout), Some(mut stderr_pipe), Some(stdin)) =
            (child.stdout.take(), child.stderr.take(), child.stdin.take())
        else {
            return Err(SupervisorError::Spawn("stdio pipes unavailable".into()));
        };
        let (tx, frames) = mpsc::unbounded_channel::<Inbound>();
        tokio::spawn(async move {
            loop {
                let item = read_frame(&mut stdout).await.map_err(|e| e.to_string());
                let stop = item.is_err();
                if tx.send(item).is_err() || stop {
                    break;
                }
            }
        });
        let ring: Arc<Mutex<VecDeque<u8>>> = Default::default();
        let ring2 = ring.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stderr_pipe.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut r) = ring2.lock() {
                            r.extend(&buf[..n]);
                            while r.len() > STDERR_RING_BYTES {
                                r.pop_front();
                            }
                        }
                    }
                }
            }
        });
        let mut me = Self {
            child,
            stdin: Some(stdin),
            frames,
            stderr: ring,
            generation,
            stale_rejected: 0,
            in_flight: None,
            exited: false,
        };
        let first = match tokio::time::timeout(handshake_timeout, me.frames.recv()).await {
            Err(_) => {
                me.kill_group().await;
                return Err(SupervisorError::Timeout);
            }
            Ok(None) | Ok(Some(Err(_))) => {
                let (exit, signal) = me.wait_exit(Duration::from_secs(2)).await;
                return Err(SupervisorError::WorkerExited {
                    exit,
                    signal,
                    stderr_tail: me.stderr_tail(),
                });
            }
            Ok(Some(Ok(f))) => f,
        };
        match first.body {
            Body::Hello {
                protocol_version, ..
            } if protocol_version != PROTOCOL_VERSION => {
                me.kill_group().await;
                Err(SupervisorError::IncompatibleWorker {
                    expected: PROTOCOL_VERSION,
                    got: protocol_version,
                })
            }
            Body::Hello { .. }
                if first.worker_generation == generation
                    && first.schema_version == SCHEMA_VERSION =>
            {
                Ok(me)
            }
            _ => {
                me.kill_group().await;
                Err(SupervisorError::Protocol(
                    "first frame was not a Hello for this generation and schema".into(),
                ))
            }
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Frames dropped because their generation, request or attempt did not
    /// match the turn in flight.
    pub fn stale_frames_rejected(&self) -> u64 {
        self.stale_rejected
    }

    /// The last bytes the worker wrote to stderr, for diagnostics only.
    pub fn stderr_tail(&self) -> String {
        let r = self
            .stderr
            .lock()
            .map(|r| r.iter().copied().collect::<Vec<u8>>())
            .unwrap_or_default();
        String::from_utf8_lossy(&r).into_owned()
    }

    async fn send(
        &mut self,
        request_id: &str,
        attempt_id: &str,
        body: Body,
    ) -> Result<(), SupervisorError> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(SupervisorError::Protocol("worker input is closed".into()));
        };
        let frame = Frame {
            schema_version: SCHEMA_VERSION,
            worker_generation: self.generation,
            request_id: request_id.into(),
            attempt_id: attempt_id.into(),
            body,
        };
        match write_frame(stdin, &frame).await {
            Ok(_) => Ok(()),
            Err(FrameError::TooLarge(n)) => Err(SupervisorError::Protocol(format!(
                "frame of {n} bytes exceeds the {MAX_FRAME} byte limit"
            ))),
            Err(FrameError::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                let (exit, signal) = self.wait_exit(Duration::from_secs(2)).await;
                Err(SupervisorError::WorkerExited {
                    exit,
                    signal,
                    stderr_tail: self.stderr_tail(),
                })
            }
            Err(e) => Err(SupervisorError::Protocol(e.to_string())),
        }
    }

    /// Send one turn and wait for its result. A frame is accepted only if its
    /// generation, request ID and attempt ID all match. If the future is
    /// dropped (for example by a host-side cancel signal), call
    /// [`WorkerProcess::cancel`] next.
    pub async fn run_turn(
        &mut self,
        request_id: &str,
        attempt_id: &str,
        payload: &RunTurnPayload,
        timeout: Duration,
    ) -> Result<TurnOutcome, SupervisorError> {
        let value =
            serde_json::to_value(payload).map_err(|e| SupervisorError::Protocol(e.to_string()))?;
        self.in_flight = Some((request_id.to_owned(), attempt_id.to_owned()));
        self.send(request_id, attempt_id, Body::RunTurn { payload: value })
            .await?;
        let deadline = Instant::now() + timeout;
        match self.await_result(request_id, attempt_id, deadline).await? {
            Some(o) => {
                self.in_flight = None;
                Ok(o)
            }
            None => Err(SupervisorError::Timeout),
        }
    }

    /// Wait until `deadline` for the matching result. `Ok(None)` is a timeout.
    async fn await_result(
        &mut self,
        request_id: &str,
        attempt_id: &str,
        deadline: Instant,
    ) -> Result<Option<TurnOutcome>, SupervisorError> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let item = match tokio::time::timeout(left, self.frames.recv()).await {
                Err(_) => return Ok(None),
                Ok(i) => i,
            };
            let frame = match item {
                None | Some(Err(_)) => {
                    let (exit, signal) = self.wait_exit(Duration::from_secs(2)).await;
                    return Err(SupervisorError::WorkerExited {
                        exit,
                        signal,
                        stderr_tail: self.stderr_tail(),
                    });
                }
                Some(Ok(f)) => f,
            };
            let matches = frame.worker_generation == self.generation
                && frame.schema_version == SCHEMA_VERSION
                && frame.request_id == request_id
                && frame.attempt_id == attempt_id;
            match frame.body {
                Body::TurnResult { payload } if matches => {
                    return match serde_json::from_value::<TurnOutcome>(payload.clone()) {
                        Ok(o) => Ok(Some(o)),
                        Err(e) => {
                            let detail = payload
                                .get("worker_error")
                                .and_then(|v| v.as_str())
                                .map(str::to_owned)
                                .unwrap_or_else(|| {
                                    format!("result payload is not a TurnOutcome: {e}")
                                });
                            Err(SupervisorError::Protocol(detail))
                        }
                    };
                }
                _ => self.stale_rejected += 1,
            }
        }
    }

    /// Ask the worker to stop the turn in flight. Waits `grace` for the
    /// cancelled result; otherwise kills the worker's whole process group and
    /// returns the exit status as termination evidence.
    pub async fn cancel(&mut self, grace: Duration) -> CancelOutcome {
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            let (exit, signal) = status_parts(&status);
            return CancelOutcome::AlreadyExited { exit, signal };
        }
        let (req, att) = self
            .in_flight
            .clone()
            .unwrap_or_else(|| ("-".into(), "-".into()));
        let started = Instant::now();
        if self.send(&req, &att, Body::Cancel).await.is_ok()
            && self.in_flight.is_some()
            && let Ok(Some(outcome)) = self.await_result(&req, &att, started + grace).await
        {
            self.in_flight = None;
            return CancelOutcome::Cooperative { outcome };
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            let (exit, signal) = status_parts(&status);
            return CancelOutcome::AlreadyExited { exit, signal };
        }
        let (exit, signal) = self.kill_group().await;
        self.in_flight = None;
        CancelOutcome::Killed {
            exit,
            signal,
            after_ms: started.elapsed().as_millis() as u64,
        }
    }

    /// Ask the worker to exit, then kill it if it has not within a short wait.
    pub async fn shutdown(&mut self) -> (Option<i32>, Option<i32>) {
        let _ = self.send("shutdown", "-", Body::Shutdown).await;
        self.stdin = None;
        match tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await {
            Ok(Ok(status)) => {
                self.exited = true;
                status_parts(&status)
            }
            _ => self.kill_group().await,
        }
    }

    async fn wait_exit(&mut self, within: Duration) -> (Option<i32>, Option<i32>) {
        match tokio::time::timeout(within, self.child.wait()).await {
            Ok(Ok(status)) => {
                self.exited = true;
                status_parts(&status)
            }
            _ => (None, None),
        }
    }

    /// Kill the worker and everything in its process group, then reap it.
    async fn kill_group(&mut self) -> (Option<i32>, Option<i32>) {
        if self.exited {
            return (None, None);
        }
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            // SAFETY: killpg only sends a signal to the group the worker leads.
            unsafe {
                libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
        let r = match tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(status)) => status_parts(&status),
            _ => (None, None),
        };
        self.exited = true;
        r
    }
}
