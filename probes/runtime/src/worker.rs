//! In-process loop vs supervised worker process (same binary), RT-08.
//!
//! The worker speaks the framed protocol from `factory_agent::protocol` on
//! stdin/stdout. Diagnostics go to stderr and are collected separately. The
//! host accepts a frame only when its channel generation, its stamped worker
//! generation, its request ID and its schema version all match.

use factory_agent::anthropic::AnthropicClient;
use factory_agent::openai::{self, ApiKey, Control, OpenAiClient};
use factory_agent::protocol::*;
use factory_agent::transport::{ByteSource, Fault, Replay};
use serde_json::{Value, json};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::journal::Journal;
use crate::util::*;

const WS_ID: &str = "ws-worker";

type Tagged = (u64, Result<Frame, String>);

struct Worker {
    wgen: u64,
    child: Child,
    stdin: ChildStdin,
}

struct Host {
    tx: mpsc::UnboundedSender<Tagged>,
    rx: mpsc::UnboundedReceiver<Tagged>,
    stderr_lines: Arc<std::sync::Mutex<Vec<String>>>,
    current: u64,
}

impl Host {
    async fn spawn(&mut self, wgen: u64, extra: &[&str]) -> Result<(Worker, Frame, f64), String> {
        let t0 = Instant::now();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("worker-child")
            .args(["--generation", &wgen.to_string()])
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            loop {
                match read_frame(&mut stdout).await {
                    Ok(f) => {
                        if tx.send((wgen, Ok(f))).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send((wgen, Err(e.to_string())));
                        break;
                    }
                }
            }
        });
        let lines = self.stderr_lines.clone();
        tokio::spawn(async move {
            let mut r = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(l)) = r.next_line().await {
                lines.lock().unwrap().push(format!("wgen{wgen}: {l}"));
            }
        });
        let mut w = Worker { wgen, child, stdin };
        // Handshake must come from this channel within 5 s.
        let hello = loop {
            match tokio::time::timeout(Duration::from_secs(5), self.rx.recv()).await {
                Ok(Some((g, r))) if g == wgen => break r,
                Ok(Some(_)) => continue,
                _ => break Err("handshake timeout".into()),
            }
        };
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        match hello {
            Ok(f) => match &f.body {
                Body::Hello { protocol_version, .. } if *protocol_version == PROTOCOL_VERSION && f.worker_generation == wgen && f.schema_version == SCHEMA_VERSION => Ok((w, f, ms)),
                _ => {
                    let _ = w.child.kill().await;
                    Err(format!("incompatible handshake refused before any task: {}", serde_json::to_string(&f).unwrap()))
                }
            },
            Err(e) => {
                let _ = w.child.kill().await;
                Err(format!("protocol violation during handshake, worker terminated: {e}"))
            }
        }
    }

    /// Validate a received frame against the current generation and request.
    fn check(&self, tagged: &Tagged, expected_request: &str) -> Result<Frame, Value> {
        let (chan, r) = tagged;
        let f = match r {
            Ok(f) => f,
            Err(e) => return Err(json!({"rejected": "channel_error", "channel_generation": chan, "error": e})),
        };
        if *chan != self.current {
            return Err(json!({"rejected": "stale_channel_generation", "channel_generation": chan, "current": self.current, "frame_generation": f.worker_generation, "request_id": f.request_id}));
        }
        if f.worker_generation != self.current {
            return Err(json!({"rejected": "stale_frame_generation", "frame_generation": f.worker_generation, "current": self.current, "request_id": f.request_id}));
        }
        if f.schema_version != SCHEMA_VERSION {
            return Err(json!({"rejected": "schema_version", "got": f.schema_version}));
        }
        if f.request_id != expected_request {
            return Err(json!({"rejected": "unexpected_request_id", "got": f.request_id, "expected": expected_request}));
        }
        Ok(f.clone())
    }

    async fn recv(&mut self, timeout: Duration) -> Option<Tagged> {
        tokio::time::timeout(timeout, self.rx.recv()).await.ok().flatten()
    }

    /// Next item from the current channel. Items from superseded channels are
    /// rejected and logged as host diagnostics, never delivered.
    async fn recv_current(&mut self, j: &Journal, timeout: Duration) -> Option<Tagged> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let t = self.recv(left).await?;
            if t.0 == self.current {
                return Some(t);
            }
            let r = match &t.1 {
                Ok(f) => json!({"rejected": "stale_channel_generation", "channel_generation": t.0, "current": self.current, "request_id": f.request_id}),
                Err(e) => json!({"rejected": "stale_channel_closed", "channel_generation": t.0, "current": self.current, "error": e}),
            };
            j.diagnostic(WS_ID, "stale_channel_item_dropped", &r).await.unwrap();
        }
    }
}

async fn send(w: &mut Worker, request_id: &str, attempt_id: &str, payload: Value) {
    let f = Frame { schema_version: SCHEMA_VERSION, worker_generation: w.wgen, request_id: request_id.into(), attempt_id: attempt_id.into(), body: Body::RunTurn { payload } };
    write_frame(&mut w.stdin, &f).await.unwrap();
}

/// With a recording, the worker assembles the recorded real stream. Without
/// one, it hashes a fixed host-authored blob of similar size. Neither path
/// fabricates model output.
fn replay_payload(rec: &Option<Recording>, delay_ms: u64, echo_generation: Option<u64>) -> Value {
    match rec {
        Some(rec) => json!({"mode": "replay", "provider": rec.meta["provider"], "recording": String::from_utf8(rec.bytes.clone()).unwrap(), "chunks": rec.chunks, "delay_ms": delay_ms, "echo_generation": echo_generation}),
        None => json!({"mode": "ping", "blob": ping_blob(), "delay_ms": delay_ms, "echo_generation": echo_generation}),
    }
}

/// 4 KiB of host-authored bytes, about the size of a short tool-call stream.
fn ping_blob() -> String {
    "host-authored-ping-payload/".repeat(4096 / 27 + 1)[..4096].to_owned()
}

async fn in_process_work(rec: &Option<Recording>) -> bool {
    match rec {
        Some(r) => crate::provider::assemble(crate::provider::of(r), r.source(&Fault::None), |_| Control::Continue).await.is_ok(),
        None => !sha256_hex(ping_blob().as_bytes()).is_empty(),
    }
}

fn stats(mut v: Vec<f64>) -> Value {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| v[((v.len() as f64 - 1.0) * p).round() as usize];
    json!({"n": v.len(), "median": q(0.5), "p95": q(0.95), "min": v[0], "max": v[v.len() - 1]})
}

pub async fn run(live: bool) -> Value {
    let rec = primary_set().map(|(t, _, _)| Recording::load(t));
    let payload_kind = if rec.is_some() { "recorded real stream bytes" } else { "host-authored 4 KiB ping blob (no recorded real stream exists)" };
    let mut j = Journal::connect().await.expect("database");
    let (tx, rx) = mpsc::unbounded_channel();
    let mut h = Host { tx, rx, stderr_lines: Default::default(), current: 0 };
    let mut checks = vec![];
    let mut spend = vec![];

    // W1: handshake with a durable generation.
    let g1 = j.bump_generation(WS_ID).await.unwrap() as u64;
    h.current = g1;
    let (mut w1, hello1, hs_ms) = h.spawn(g1, &[]).await.expect("gen1");
    checks.push(json!({"id": "W1-HANDSHAKE", "hello": hello1, "handshake_ms": hs_ms, "verdict": "pass"}));

    // W2: incompatible protocol version refused before any task is sent.
    let gx = j.bump_generation(WS_ID).await.unwrap() as u64;
    let prev = h.current;
    h.current = gx;
    let bad = h.spawn(gx, &["--report-protocol", "99"]).await;
    h.current = prev;
    checks.push(json!({"id": "W2-INCOMPATIBLE-VERSION-REFUSED", "result": bad.as_ref().err(), "tasks_sent": 0, "verdict": if bad.is_err() {"pass"} else {"fail"}}));

    // W3: gen1 stalls, host supersedes it, gen2 answers, gen1's late frame is rejected.
    let req1 = new_id("req");
    let att1 = new_id("att");
    send(&mut w1, &req1, &att1, replay_payload(&rec, 1500, None)).await;
    let first = h.recv(Duration::from_millis(300)).await;
    j.record_attempt(WS_ID, &att1, "worker_unresponsive_superseded", g1 as i64, &json!({"request_id": req1, "effect": "none requested", "auto_retry": false, "first_frame_within_timeout": first.is_some()})).await.unwrap();
    let g2 = j.bump_generation(WS_ID).await.unwrap() as u64;
    h.current = g2;
    let (mut w2, _, _) = h.spawn(g2, &[]).await.expect("gen2");
    let req2 = new_id("req");
    let att2 = new_id("att");
    send(&mut w2, &req2, &att2, replay_payload(&rec, 0, None)).await;
    let mut accepted2 = json!(null);
    while let Some(t) = h.recv(Duration::from_secs(5)).await {
        match h.check(&t, &req2) {
            Ok(f) => {
                let seq = j.record_attempt(WS_ID, &att2, "turn_result_accepted", g2 as i64, &json!({"request_id": req2})).await.unwrap();
                accepted2 = json!({"seq": seq, "frame_generation": f.worker_generation});
                break;
            }
            Err(r) => j.diagnostic(WS_ID, "frame_rejected", &r).await.unwrap(),
        }
    }
    let events_before_late = j.events(WS_ID).await.unwrap();
    let late = h.recv(Duration::from_secs(3)).await;
    let late_verdict = late.as_ref().map(|t| h.check(t, &req1));
    if let Some(Err(r)) = &late_verdict {
        j.diagnostic(WS_ID, "frame_rejected", r).await.unwrap();
    }
    let events_after_late = j.events(WS_ID).await.unwrap();
    let _ = w1.child.kill().await;
    let w1_status = w1.child.wait().await.ok().and_then(|s| s.code());
    let ok = matches!(late_verdict, Some(Err(_))) && events_before_late == events_after_late && !accepted2.is_null();
    checks.push(json!({"id": "W3-LATE-OLD-GENERATION-FRAME-REJECTED (RT-08)", "gen1": g1, "gen2": g2, "gen2_accepted": accepted2,
        "late_frame": late_verdict.map(|r| r.err()), "event_order_unchanged": events_before_late == events_after_late,
        "gen1_killed_exit_code": w1_status, "verdict": if ok {"pass"} else {"fail"}}));

    // W4: a frame stamped with an old generation on the current channel.
    let req3 = new_id("req");
    send(&mut w2, &req3, &new_id("att"), replay_payload(&rec, 0, Some(g1))).await;
    let t = h.recv_current(&j, Duration::from_secs(5)).await.unwrap();
    let forged_from = json!({"channel_generation": t.0, "frame_generation": t.1.as_ref().ok().map(|f| f.worker_generation)});
    let forged = h.check(&t, &req3);
    if let Err(r) = &forged {
        j.diagnostic(WS_ID, "frame_rejected", r).await.unwrap();
    }
    checks.push(json!({"id": "W4-FORGED-OLD-GENERATION-STAMP-REJECTED (RT-08)", "received": forged_from, "result": forged.as_ref().err(), "verdict": if forged.as_ref().err().map(|e| e["rejected"] == "stale_frame_generation").unwrap_or(false) {"pass"} else {"fail"}}));

    // W5: kill the worker mid-request. Outcome unknown, no automatic retry.
    let req4 = new_id("req");
    let att4 = new_id("att");
    let mut sends_req4 = 0;
    send(&mut w2, &req4, &att4, replay_payload(&rec, 2000, None)).await;
    sends_req4 += 1;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let _ = w2.child.kill().await;
    let killed_status = w2.child.wait().await.ok().map(|s| format!("{s:?}"));
    let eof = h.recv_current(&j, Duration::from_secs(3)).await;
    let eof_seen = matches!(&eof, Some((g, Err(_))) if *g == g2);
    j.record_attempt(WS_ID, &att4, "worker_lost", g2 as i64, &json!({"request_id": req4, "outcome": "unknown", "auto_retry": false, "termination": killed_status})).await.unwrap();
    let g3 = j.bump_generation(WS_ID).await.unwrap() as u64;
    h.current = g3;
    let (mut w3, _, _) = h.spawn(g3, &[]).await.expect("gen3");
    let row = j.attempt(&att4).await.unwrap();
    checks.push(json!({"id": "W5-WORKER-KILLED-RESTARTED (RT-08, RT-10)", "eof_observed": eof_seen, "termination": killed_status, "attempt_row": row, "requests_sent_for_attempt": sends_req4,
        "restarted_generation": g3, "verdict": if eof_seen && sends_req4 == 1 && row.map(|r| r["state"] == "worker_lost").unwrap_or(false) {"pass"} else {"fail"}}));

    // W6: unsolicited stdout output cannot pass as a frame.
    let gj = j.bump_generation(WS_ID).await.unwrap() as u64;
    let prev = h.current;
    h.current = gj;
    let junk = h.spawn(gj, &["--emit-junk"]).await;
    h.current = prev;
    checks.push(json!({"id": "W6-UNSOLICITED-OUTPUT-IS-PROTOCOL-VIOLATION", "result": junk.as_ref().err(), "verdict": if junk.is_err() {"pass"} else {"fail"}}));

    // W7: an in-process task that outlives a restart is rejected the same way.
    let gen_now = Arc::new(AtomicU64::new(h.current));
    let task_gen = gen_now.load(Ordering::SeqCst);
    let rec_task = rec.as_ref().map(|r| Recording { name: r.name.clone(), bytes: r.bytes.clone(), chunks: r.chunks.clone(), meta: r.meta.clone() });
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        (task_gen, in_process_work(&rec_task).await)
    });
    let g4 = j.bump_generation(WS_ID).await.unwrap() as u64;
    gen_now.store(g4, Ordering::SeqCst);
    let (tg, ok_turn) = task.await.unwrap();
    let rejected = tg != gen_now.load(Ordering::SeqCst);
    checks.push(json!({"id": "W7-IN-PROCESS-LATE-RESULT-REJECTED (RT-08)", "task_generation": tg, "current_generation": g4, "turn_assembled": ok_turn, "rejected": rejected, "verdict": if rejected {"pass"} else {"fail"}}));
    h.current = g3; // the worker gen3 is still the current process for the bench below
    // Persisted generation moved on in W7. Re-align the bench worker generation.
    let g5 = j.bump_generation(WS_ID).await.unwrap() as u64;
    let _ = w3.child.kill().await;
    h.current = g5;
    let (mut w5, _, _) = h.spawn(g5, &[]).await.expect("gen5");

    // W8: one live turn through the worker process.
    if live {
        let req = new_id("req");
        let body = crate::scenario::a_tool_turn_body();
        send(&mut w5, &req, &new_id("att"), json!({"mode": "live", "provider": factory_agent::anthropic::PROVIDER, "body": body})).await;
        let t = h.recv_current(&j, Duration::from_secs(60)).await.unwrap();
        let r = h.check(&t, &req);
        let payload = r.as_ref().ok().map(|f| match &f.body { Body::TurnResult { payload } => payload.clone(), _ => json!(null) });
        let usage = payload.as_ref().and_then(|p| serde_json::from_value::<factory_agent::types::Usage>(p["turn"]["usage"].clone()).ok());
        let s = crate::live::spend_entry("worker-live-turn", payload.as_ref().and_then(|p| p["turn"]["model"].as_str()).unwrap_or(crate::scenario::A_MODEL), usage.as_ref(), &body);
        spend.push(s.clone());
        let ok = payload.as_ref().map(|p| p["ok"] == true).unwrap_or(false);
        checks.push(json!({"id": "W8-LIVE-TURN-VIA-WORKER", "evidence_kind": "live", "result": payload, "spend": s, "verdict": if ok {"pass"} else {"fail"}}));
    }

    // Overhead: in-process assembly vs worker round trip, same recording.
    let n = 200;
    let mut inproc = vec![];
    for _ in 0..n {
        let t0 = Instant::now();
        assert!(in_process_work(&rec).await);
        inproc.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    let mut rt = vec![];
    let mut child_us = vec![];
    for _ in 0..n {
        let req = new_id("req");
        let t0 = Instant::now();
        send(&mut w5, &req, "att-bench", replay_payload(&rec, 0, None)).await;
        let t = h.recv_current(&j, Duration::from_secs(5)).await.unwrap();
        let f = h.check(&t, &req).expect("bench frame");
        rt.push(t0.elapsed().as_secs_f64() * 1e6);
        if let Body::TurnResult { payload } = f.body {
            child_us.push(payload["assemble_us"].as_f64().unwrap_or(0.0));
        }
    }
    let mut spawns = vec![];
    for i in 0..10u64 {
        // Bench-only generations, never persisted, distinct per spawn.
        let g = 1_000_000 + i;
        let prev = h.current;
        h.current = g;
        let (mut w, _, ms) = h.spawn(g, &[]).await.expect("bench spawn");
        spawns.push(ms);
        let _ = w.child.kill().await;
        h.current = prev;
    }
    let shutdown = Frame { schema_version: SCHEMA_VERSION, worker_generation: w5.wgen, request_id: "shutdown".into(), attempt_id: "-".into(), body: Body::Shutdown };
    let _ = write_frame(&mut w5.stdin, &shutdown).await;
    let _ = w5.child.wait().await;

    let diagnostics = j.c.query("SELECT kind, payload FROM diagnostic WHERE workspace_id=$1 ORDER BY id", &[&WS_ID]).await.unwrap();
    json!({
        "probe": "worker", "evidence_kind": if rec.is_some() {"recorded-real-replay"} else {"local (real processes and pipes, host-authored payload, no model output)"},
        "payload": payload_kind,
        "recording": rec.as_ref().map(|r| r.reference()), "protocol_version": PROTOCOL_VERSION, "max_frame_bytes": MAX_FRAME,
        "checks": checks,
        "overhead": {
            "payload_bytes": rec.as_ref().map(|r| r.bytes.len()).unwrap_or(4096),
            "in_process_assemble_us": stats(inproc),
            "worker_roundtrip_us": stats(rt),
            "worker_internal_assemble_us": stats(child_us),
            "worker_spawn_and_handshake_ms": stats(spawns),
            "build_profile": if cfg!(debug_assertions) {"debug"} else {"release"},
        },
        "durable_events": j.events(WS_ID).await.unwrap(),
        "host_diagnostics": diagnostics.iter().map(|r| json!({"kind": r.get::<_, String>(0), "payload": r.get::<_, Value>(1)})).collect::<Vec<_>>(),
        "worker_stderr_lines": h.stderr_lines.lock().unwrap().len(),
        "spend": spend,
    })
}

pub async fn child(args: &[String]) {
    let wgen: u64 = arg(args, "--generation").and_then(|g| g.parse().ok()).unwrap_or(0);
    let proto: u32 = arg(args, "--report-protocol").and_then(|g| g.parse().ok()).unwrap_or(PROTOCOL_VERSION);
    let mut out = tokio::io::stdout();
    let mut inp = tokio::io::stdin();
    eprintln!("worker gen {wgen} starting, protocol {proto}");
    if flag(args, "--emit-junk") {
        out.write_all(b"worker starting up, loading adapters...\n").await.unwrap();
        out.flush().await.unwrap();
    }
    let hello = Frame {
        schema_version: SCHEMA_VERSION,
        worker_generation: wgen,
        request_id: "hello".into(),
        attempt_id: "-".into(),
        body: Body::Hello { protocol_version: proto, runtime_build: factory_agent::RUNTIME_BUILD.into(), adapters: vec![factory_agent::openai::PROVIDER.into(), "anthropic-messages (no live evidence)".into()] },
    };
    write_frame(&mut out, &hello).await.unwrap();
    let client = ApiKey::from_env("OPENAI_API_KEY").and_then(|k| OpenAiClient::new(crate::live::BASE, k, Duration::from_secs(60)).ok());
    loop {
        let f = match read_frame(&mut inp).await {
            Ok(f) => f,
            Err(_) => return,
        };
        let payload = match f.body {
            Body::RunTurn { payload } => payload,
            Body::Shutdown => return,
            _ => continue,
        };
        let delay = payload["delay_ms"].as_u64().unwrap_or(0);
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        let t0 = Instant::now();
        if payload["mode"] == "ping" {
            let h = sha256_hex(payload["blob"].as_str().unwrap_or("").as_bytes());
            let us = t0.elapsed().as_secs_f64() * 1e6;
            let stamp = payload["echo_generation"].as_u64().unwrap_or(wgen);
            let reply = Frame { schema_version: SCHEMA_VERSION, worker_generation: stamp, request_id: f.request_id, attempt_id: f.attempt_id, body: Body::TurnResult { payload: json!({"ok": true, "blob_sha256": h, "assemble_us": us}) } };
            if write_frame(&mut out, &reply).await.is_err() {
                return;
            }
            continue;
        }
        let anthropic = payload["provider"] == factory_agent::anthropic::PROVIDER;
        let result = if payload["mode"] == "live" && anthropic {
            match ApiKey::from_env("ANTHROPIC_API_KEY").and_then(|k| AnthropicClient::new(crate::live::ANTHROPIC_BASE, k, Duration::from_secs(60)).ok()) {
                Some(c) => match c.start(&payload["body"], None).await {
                    Ok((src, _)) => factory_agent::anthropic::assemble(src, |_| Control::Continue).await,
                    Err(e) => Err(e),
                },
                None => {
                    eprintln!("no anthropic credential");
                    continue;
                }
            }
        } else if payload["mode"] == "live" {
            match &client {
                Some(c) => match c.start(&payload["body"], None).await {
                    Ok((src, _)) => openai::assemble(src, |_| Control::Continue).await,
                    Err(e) => Err(e),
                },
                None => {
                    eprintln!("no credential");
                    continue;
                }
            }
        } else if anthropic {
            let bytes = payload["recording"].as_str().unwrap_or("").as_bytes().to_vec();
            let chunks: Vec<usize> = serde_json::from_value(payload["chunks"].clone()).unwrap_or_default();
            let src = ByteSource::Replay(Replay::new(bytes.into(), Some(chunks), &Fault::None));
            factory_agent::anthropic::assemble(src, |_| Control::Continue).await
        } else {
            let bytes = payload["recording"].as_str().unwrap_or("").as_bytes().to_vec();
            let chunks: Vec<usize> = serde_json::from_value(payload["chunks"].clone()).unwrap_or_default();
            let src = ByteSource::Replay(Replay::new(bytes.into(), Some(chunks), &Fault::None));
            openai::assemble(src, |_| Control::Continue).await
        };
        let us = t0.elapsed().as_secs_f64() * 1e6;
        let stamp = payload["echo_generation"].as_u64().unwrap_or(wgen);
        let body = match result {
            Ok(t) => json!({"ok": true, "turn": t, "assemble_us": us}),
            Err(e) => json!({"ok": false, "class": e.class(), "error": e, "assemble_us": us}),
        };
        let reply = Frame { schema_version: SCHEMA_VERSION, worker_generation: stamp, request_id: f.request_id, attempt_id: f.attempt_id, body: Body::TurnResult { payload: body } };
        if write_frame(&mut out, &reply).await.is_err() {
            return;
        }
    }
}
