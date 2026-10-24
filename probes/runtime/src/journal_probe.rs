//! T-RUN-03 / T-RUN-04 probe against the Compose PostgreSQL service.
//!
//! Phase a: fencing, stale result, replay identity, real process crashes at
//! three points, pause and revocation. The driver script then restarts the
//! database container. Phase b runs in a new process: persisted state, recovery
//! to unknown_effect without replay, and explicit reconciliation.

use serde_json::{Value, json};
use std::time::Duration;

use crate::broker::{self, Broker, Prepared, Workspace};
use crate::journal::Journal;
use crate::util::*;

const WS_ID: &str = "ws-journal";

async fn real_call() -> (String, Value) {
    let r = tool_request().await;
    (r.name, r.args)
}

/// Host-side scoping hook: place the model's relative path under a per-case
/// directory. It runs before final validation like any other hook.
fn scoped(b: &mut Broker, case: &str) {
    let case = case.to_owned();
    b.hooks.push((
        "host_scope_case_dir".into(),
        Box::new(move |_, a| {
            let p = a["path"].as_str().unwrap_or("").to_owned();
            a["path"] = json!(format!("{case}/{p}"));
        }),
    ));
}

fn prepare(ws: &Workspace, case: &str, name: &str, args: &Value) -> Prepared {
    let mut b = Broker::new(ws.clone());
    scoped(&mut b, case);
    b.prepare(name, args).result.expect("real call prepares")
}

fn base() -> std::path::PathBuf {
    probe_base().join("journal")
}

async fn wait_lease(j: &Journal, owner: &str, ttl: i64) -> crate::journal::Lease {
    for _ in 0..100 {
        if let Some(l) = j.acquire_lease(WS_ID, owner, ttl).await.unwrap() {
            return l;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("lease never became free");
}

pub async fn run(phase: &str) -> Value {
    match phase {
        "reset" => {
            let j = Journal::connect().await.expect("database");
            j.reset().await.unwrap();
            json!({"reset": crate::journal::SCHEMA, "postgres": j.server_version().await})
        }
        "a" => phase_a().await,
        "b" => phase_b().await,
        _ => json!({"error": "unknown phase"}),
    }
}

async fn phase_a() -> Value {
    let _ = std::fs::remove_dir_all(base());
    let ws = Workspace::open(&base().join("root")).unwrap();
    let mut j = Journal::connect().await.expect("database");
    let (name, args) = real_call().await;
    let mut checks = vec![];

    // J1: intent before effect, outcome after, under fence A.
    let a = j.acquire_lease(WS_ID, "owner-a", 5000).await.unwrap().unwrap();
    let (_, epoch) = j.control(WS_ID).await.unwrap();
    let p1 = prepare(&ws, "j1", &name, &args);
    let before = broker::file_hash(&broker::resolve_in_root(&ws.root, &p1.rel_path).unwrap());
    let op1 = j.begin_intent(WS_ID, "owner-a", a.fence, epoch, &new_id("req"), "att-j1", &p1, &before).await.unwrap().unwrap();
    let ops_after_intent = j.ops(WS_ID).await.unwrap();
    let st = broker::stage(&ws.root, &p1, &op1).unwrap();
    let after1 = broker::commit_stage(&ws.root, &p1, &st).unwrap();
    let pub1 = j.complete(WS_ID, "owner-a", a.fence, &op1, &after1).await.unwrap();
    checks.push(json!({"id": "J1-INTENT-BEFORE-EFFECT", "op_id": op1, "state_after_intent_before_effect": ops_after_intent[0]["state"], "published": pub1.is_ok(), "verdict": if pub1.is_ok() && ops_after_intent[0]["state"] == "in_flight" {"pass"} else {"fail"}}));

    // J2: owner A starts op2 and performs the effect, then loses the lease.
    let p2 = prepare(&ws, "j2-stale", &name, &args);
    let before2 = broker::file_hash(&broker::resolve_in_root(&ws.root, &p2.rel_path).unwrap());
    let op2 = j.begin_intent(WS_ID, "owner-a", a.fence, epoch, &new_id("req"), "att-j2", &p2, &before2).await.unwrap().unwrap();
    let st2 = broker::stage(&ws.root, &p2, &op2).unwrap();
    let after2 = broker::commit_stage(&ws.root, &p2, &st2).unwrap();
    tokio::time::sleep(Duration::from_millis(5200)).await;
    let b = j.acquire_lease(WS_ID, "owner-b", 3000).await.unwrap().expect("takeover after expiry");
    let stale_pub = j.complete(WS_ID, "owner-a", a.fence, &op2, &after2).await.unwrap();
    checks.push(json!({"id": "J2-STALE-FENCE-RESULT-CANNOT-PUBLISH (T-RUN-03)", "fence_a": a.fence, "fence_b": b.fence, "publish_attempt": stale_pub.as_ref().err(), "op2_state": j.ops(WS_ID).await.unwrap().iter().find(|o| o["op_id"] == op2.as_str()).map(|o| o["state"].clone()),
        "verdict": if stale_pub.is_err() && b.fence > a.fence {"pass"} else {"fail"}}));

    // J2b: owner A cannot start a new effect with its stale fence.
    let snap0 = tree_snapshot(&base());
    let p3 = prepare(&ws, "j2b-stale-start", &name, &args);
    let r3 = j.begin_intent(WS_ID, "owner-a", a.fence, epoch, &new_id("req"), "att-j2b", &p3, "absent").await.unwrap();
    let snap1 = tree_snapshot(&base());
    checks.push(json!({"id": "J2B-STALE-FENCE-CANNOT-START (T-RUN-03)", "refusal": r3.as_ref().err(), "tree_unchanged": snap0 == snap1, "verdict": if r3.is_err() && snap0 == snap1 {"pass"} else {"fail"}}));

    // J3: replay identity. Same identity is already applied. Changed root or tool version is refused.
    let same = j.request_replay(&op1, &p1).await.unwrap();
    let moved = Workspace::open(&base().join("root-moved")).unwrap();
    let p_moved = prepare(&moved, "j1", &name, &args);
    let changed_root = j.request_replay(&op1, &p_moved).await.unwrap();
    let mut p_v2 = p1.clone();
    p_v2.tool_version = "write_file@2".into();
    p_v2.replay_identity = broker::replay_identity(&p_v2.tool, &p_v2.tool_version, 1, &p_v2.root_identity, &p_v2.args_hash);
    let changed_tool = j.request_replay(&op1, &p_v2).await.unwrap();
    let ok = same["result"]["decision"] == "already_applied" && changed_root["result"]["decision"] == "refused" && changed_tool["result"]["decision"] == "refused";
    checks.push(json!({"id": "J3-REPLAY-IDENTITY (T-RUN-04, ORCH-09)", "same_identity": same, "changed_root": changed_root, "changed_tool_version": changed_tool, "verdict": if ok {"pass"} else {"fail"}}));

    // J4: real process crashes at three points.
    tokio::time::sleep(Duration::from_millis(3200)).await;
    let exe = std::env::current_exe().unwrap();
    let mut crashes = vec![];
    for mode in ["before-intent", "during-effect", "after-effect"] {
        let st = tokio::process::Command::new(&exe)
            .args(["journal-crash-child", "--mode", mode, "--ws", base().join("root").to_str().unwrap()])
            .status()
            .await
            .unwrap();
        crashes.push(json!({"mode": mode, "exit_code": st.code(), "success": st.success()}));
        tokio::time::sleep(Duration::from_millis(3200)).await;
    }
    checks.push(json!({"id": "J4-CRASH-CHILDREN", "children": crashes, "verdict": if crashes.iter().all(|c| c["success"] == false) {"pass"} else {"fail"}}));

    // J5: pause and revoke, durable before the database restart.
    let d = wait_lease(&j, "owner-d", 5000).await;
    j.set_paused(WS_ID, true).await.unwrap();
    let new_epoch = j.revoke(WS_ID).await.unwrap();
    let p5 = prepare(&ws, "j5-paused", &name, &args);
    let r5 = j.begin_intent(WS_ID, "owner-d", d.fence, epoch, &new_id("req"), "att-j5", &p5, "absent").await.unwrap();
    checks.push(json!({"id": "J5-PAUSE-AND-REVOCATION-DISPATCH-STOP (T-RUN-03)", "refusal": r5.as_ref().err(), "revocation_epoch": new_epoch, "verdict": if r5.is_err() {"pass"} else {"fail"}}));

    json!({
        "probe": "journal", "phase": "a", "evidence_kind": "live-local (PostgreSQL 17 in Compose, real process crashes)", "input_source": tool_request().await.input_source,
        "postgres": j.server_version().await,
        "workspace": ws, "checks": checks,
        "events": j.events(WS_ID).await.unwrap(), "ops": j.ops(WS_ID).await.unwrap(),
        "control": j.control(WS_ID).await.unwrap(),
        "last_fence": d.fence,
        "op_ids": {"op1": op1, "op2_stale": op2},
    })
}

pub async fn crash_child(args: &[String]) {
    let mode = arg(args, "--mode").unwrap();
    let ws = Workspace::open(std::path::Path::new(&arg(args, "--ws").unwrap())).unwrap();
    let mut j = Journal::connect().await.unwrap();
    let owner = format!("crash-{mode}");
    let l = wait_lease(&j, &owner, 3000).await;
    let (_, epoch) = j.control(WS_ID).await.unwrap();
    let (name, a) = real_call().await;
    let p = prepare(&ws, &format!("j4-{mode}"), &name, &a);
    eprintln!("crash child {mode}: fence {}", l.fence);
    if mode == "before-intent" {
        std::process::abort();
    }
    let op = j.begin_intent(WS_ID, &owner, l.fence, epoch, &new_id("req"), &format!("att-j4-{mode}"), &p, "absent").await.unwrap().unwrap();
    let st = broker::stage(&ws.root, &p, &op).unwrap();
    if mode == "during-effect" {
        std::process::abort();
    }
    broker::commit_stage(&ws.root, &p, &st).unwrap();
    // after-effect: crash before the outcome is recorded or a reply is sent.
    std::process::abort();
}

async fn phase_b() -> Value {
    let a_path = evidence_dir("exp03").join("runs/journal-phase-a.json");
    let a: Value = serde_json::from_slice(&std::fs::read(&a_path).unwrap()).unwrap();
    let ws = Workspace::open(&base().join("root")).unwrap();
    let mut j = Journal::connect().await.expect("database after restart");
    let mut checks = vec![];

    // R1: persisted state after a database container restart and a new process.
    let ev = j.events(WS_ID).await.unwrap();
    let prefix_same = a["events"].as_array().map(|old| ev.len() >= old.len() && old.iter().zip(&ev).all(|(o, n)| o == n)).unwrap_or(false);
    let (paused, epoch) = j.control(WS_ID).await.unwrap();
    checks.push(json!({"id": "R1-PERSISTENT-RESTART (T-RUN-03)", "phase_a_events": a["events"].as_array().map(|x| x.len()), "events_now": ev.len(), "event_prefix_identical": prefix_same,
        "paused_persisted": paused, "revocation_epoch_persisted": epoch, "workspace_identity_same": ws.identity == a["workspace"]["identity"].as_str().unwrap_or(""),
        "verdict": if prefix_same && paused && epoch == a["control"][1].as_i64().unwrap_or(-1) {"pass"} else {"fail"}}));

    // R2: dispatch still refused while paused, then by the revoked epoch.
    let (name, args) = real_call().await;
    let l = wait_lease(&j, "owner-e", 30_000).await;
    let p = prepare(&ws, "r2", &name, &args);
    let paused_ref = j.begin_intent(WS_ID, "owner-e", l.fence, epoch, &new_id("req"), "att-r2", &p, "absent").await.unwrap();
    j.set_paused(WS_ID, false).await.unwrap();
    let old_epoch_ref = j.begin_intent(WS_ID, "owner-e", l.fence, epoch - 1, &new_id("req"), "att-r2b", &p, "absent").await.unwrap();
    checks.push(json!({"id": "R2-PAUSE-SURVIVES-RESTART-AND-REVOKED-EPOCH (T-RUN-03)", "while_paused": paused_ref.as_ref().err(), "old_epoch_after_resume": old_epoch_ref.as_ref().err(),
        "verdict": if paused_ref.is_err() && old_epoch_ref.is_err() {"pass"} else {"fail"}}));

    // R3: recovery marks in-flight ops unknown_effect with observations, executes nothing.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snap0 = tree_snapshot(&base());
    // owner-e holds a 30 s lease. Recovery is a different owner, so let it lapse.
    j.c.execute("UPDATE lease SET expires_at = now() WHERE workspace_id=$1", &[&WS_ID]).await.unwrap();
    let rec = j.recover(WS_ID, "recovery-host", &ws.root).await.unwrap();
    let snap1 = tree_snapshot(&base());
    let unknown = rec["unknown_effects"].as_array().cloned().unwrap_or_default();
    let fence_increasing = rec["lease"]["fence"].as_i64().unwrap_or(0) > a["last_fence"].as_i64().unwrap_or(i64::MAX);
    let classes: Vec<Value> = unknown.iter().map(|u| u["observation"]["classification"].clone()).collect();
    let before_intent_absent = !ws.root.join("j4-before-intent").exists();
    let ok = unknown.len() == 3 && snap0 == snap1 && fence_increasing && before_intent_absent;
    checks.push(json!({"id": "R3-CRASH-RECOVERY-UNKNOWN-EFFECT (T-RUN-04, ORCH-02..04)", "recovery": rec, "classifications": classes, "tree_unchanged_by_recovery": snap0 == snap1,
        "fence_increasing_across_restart": fence_increasing, "before_intent_crash_left_no_op_and_no_file": before_intent_absent, "verdict": if ok {"pass"} else {"fail"}}));

    // R4: replay of every unknown effect is blocked.
    let mut decisions = vec![];
    for u in &unknown {
        let op = u["op_id"].as_str().unwrap();
        decisions.push(j.request_replay(op, &p).await.unwrap());
    }
    let blocked = decisions.iter().all(|d| d["result"]["decision"] == "blocked");
    checks.push(json!({"id": "R4-UNKNOWN-EFFECT-NOT-REPLAYED (T-RUN-04)", "decisions": decisions, "verdict": if blocked && !decisions.is_empty() {"pass"} else {"fail"}}));

    // R5: explicit reconciliation of the after-effect op, then replay says already applied.
    let after_op = unknown.iter().find(|u| u["observation"]["classification"] == "target_matches_intended_after_hash" && u["op_id"] != a["op_ids"]["op2_stale"]);
    let r5 = if let Some(u) = after_op {
        let op = u["op_id"].as_str().unwrap();
        let obs = u["observation"]["observed_hash"].as_str().unwrap();
        j.reconcile(WS_ID, op, "probe-reconciler (stands in for an explicit adjudication)", "reconciled_applied", obs).await.unwrap();
        let p4 = prepare(&ws, "j4-after-effect", &name, &args);
        let d = j.request_replay(op, &p4).await.unwrap();
        json!({"op_id": op, "after_reconcile": d, "verdict": if d["result"]["decision"] == "already_applied" {"pass"} else {"fail"}})
    } else {
        json!({"verdict": "fail", "reason": "after-effect op not found"})
    };
    checks.push(json!({"id": "R5-EXPLICIT-RECONCILIATION", "result": r5, "verdict": r5["verdict"]}));

    json!({"probe": "journal", "phase": "b", "evidence_kind": "live-local (PostgreSQL 17 in Compose after container restart, new process)", "input_source": tool_request().await.input_source,
        "postgres": j.server_version().await, "checks": checks, "events": j.events(WS_ID).await.unwrap(), "ops": j.ops(WS_ID).await.unwrap()})
}
