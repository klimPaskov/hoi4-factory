//! Stream faults injected into recorded real OpenAI streams, and host broker
//! boundary checks on a real recorded tool call. Evidence kind for every case
//! here is `recorded-real-replay`.

use factory_agent::openai::Control;
use factory_agent::transport::Fault;
use factory_agent::types::{Turn, TurnError};
use serde_json::{Value, json};

use crate::broker::{self, Broker, Workspace};
use crate::journal::Journal;
use crate::util::*;

pub async fn assemble(rec: &Recording, fault: &Fault) -> Result<Turn, TurnError> {
    crate::provider::assemble(crate::provider::of(rec), rec.source(fault), |_| Control::Continue).await
}

pub(crate) use crate::provider::{content_events, finish_event, tool_arg_events};

struct Ctx {
    j: Journal,
    ws: Workspace,
    ws_id: String,
    fence: i64,
    epoch: i64,
}

impl Ctx {
    async fn effect_state(&self) -> (String, usize) {
        let ops = self.j.ops(&self.ws_id).await.unwrap();
        (tree_snapshot(self.ws.root.parent().unwrap()), ops.len())
    }
}

/// Drive one turn through the loop and the host. Executes only a complete,
/// valid turn. Persists a failed turn's error as the attempt outcome.
async fn drive(ctx: &mut Ctx, case: &str, rec: &Recording, fault: Fault, expect_effect: bool) -> Value {
    let (snap0, ops0) = ctx.effect_state().await;
    let attempt = new_id("att");
    let res = assemble(rec, &fault).await;
    let mut host = json!(null);
    let persisted;
    match &res {
        Ok(turn) => {
            if let Some(call) = turn.tool_calls.first() {
                let b = Broker::new(ctx.ws.clone());
                let rep = b.prepare(&call.name, &call.arguments);
                host = match &rep.result {
                    Ok(p) => {
                        let before = broker::file_hash(&broker::resolve_in_root(&ctx.ws.root, &p.rel_path).unwrap());
                        let op = ctx.j.begin_intent(&ctx.ws_id, "faults-host", ctx.fence, ctx.epoch, &new_id("req"), &attempt, p, &before).await.unwrap();
                        match op {
                            Ok(op_id) => {
                                let st = broker::stage(&ctx.ws.root, p, &op_id).unwrap();
                                let after = broker::commit_stage(&ctx.ws.root, p, &st).unwrap();
                                let c = ctx.j.complete(&ctx.ws_id, "faults-host", ctx.fence, &op_id, &after).await.unwrap();
                                json!({"op_id": op_id, "published": c.is_ok(), "after_hash": after})
                            }
                            Err(r) => json!({"intent_refused": r}),
                        }
                    }
                    Err(r) => json!({"broker_refused": r}),
                };
            }
            let seq = ctx.j.record_attempt(&ctx.ws_id, &attempt, "succeeded", 0, &json!({"finish_reason": turn.finish_reason, "host": host})).await.unwrap();
            persisted = json!({"event_seq": seq, "row": ctx.j.attempt(&attempt).await.unwrap()});
        }
        Err(e) => {
            let state = e.class();
            let seq = ctx
                .j
                .record_attempt(&ctx.ws_id, &attempt, state, 0, &json!({"error": e, "fault": fault, "recording_sha256": rec.meta["sse_sha256"], "auto_retry_allowed": e.auto_retry_allowed()}))
                .await
                .unwrap();
            persisted = json!({"event_seq": seq, "row_state": ctx.j.attempt(&attempt).await.unwrap().map(|r| r["state"].clone())});
        }
    }
    let (snap1, ops1) = ctx.effect_state().await;
    let effect = snap0 != snap1 || ops1 != ops0;
    let verdict = if expect_effect {
        effect && res.is_ok() && host["published"] == true
    } else {
        !effect && res.is_err() && !res.as_ref().err().unwrap().auto_retry_allowed()
    };
    json!({
        "case": case,
        "evidence_kind": "recorded-real-replay",
        "recording": rec.reference(),
        "fault": fault,
        "loop_result": match &res { Ok(t) => json!({"ok": true, "finish_reason": t.finish_reason, "tool_calls": t.tool_calls.len()}), Err(e) => json!({"ok": false, "class": e.class(), "display": e.to_string(), "auto_retry_allowed": e.auto_retry_allowed()}) },
        "host": host,
        "persisted_attempt": persisted,
        "effect_observed": effect,
        "workspace_snapshot_before": snap0, "workspace_snapshot_after": snap1,
        "op_intents_before": ops0, "op_intents_after": ops1,
        "verdict": if verdict { "pass" } else { "fail" },
    })
}

pub async fn run() -> Value {
    if !has_recordings() {
        return json!({"probe": "stream-faults", "verdict": "unavailable", "evidence_kind": "none",
            "limitation": "No recorded real OpenAI stream exists. Every live request on 2026-10-24 returned HTTP 429 insufficient_quota (credit_balance_exhausted). Faults are only injected into recorded real streams, so no case ran. Synthetic streams were not substituted."});
    }
    let (n1, n2, n3) = primary_set().unwrap();
    let r1 = Recording::load(n1);
    let r2 = Recording::load(n2);
    let r3 = Recording::load(n3);
    let base = probe_base().join("faults");
    let _ = std::fs::remove_dir_all(&base);
    let ws = Workspace::open(&base.join("root")).unwrap();
    let j = Journal::connect().await.expect("database");
    let ws_id = "ws-stream-faults".to_string();
    let lease = j.acquire_lease(&ws_id, "faults-host", 600_000).await.unwrap().expect("lease");
    let (_, epoch) = j.control(&ws_id).await.unwrap();
    let mut ctx = Ctx { j, ws, ws_id, fence: lease.fence, epoch };

    let args = tool_arg_events(&r1);
    let (mid_s, mid_e, _) = args[args.len() / 2].clone();
    let (_, last_args_end, last_args_data) = args.last().unwrap().clone();
    let (_, _, fin_data) = finish_event(&r1).expect("finish event");
    let mut cases = vec![];
    cases.push(drive(&mut ctx, "F-TRUNC-MID-ARGS (RT-03)", &r1, Fault::TruncateAt(mid_s + (mid_e - mid_s) / 2), false).await);
    cases.push(drive(&mut ctx, "F-TRUNC-EVENT-BOUNDARY (RT-03)", &r1, Fault::TruncateAt(last_args_end), false).await);
    cases.push(drive(&mut ctx, "F-MISSING-FINISH-AND-DONE (RT-03)", &r1, Fault::DropEvents(vec![fin_data.clone(), crate::provider::done_needle(&r1)]), false).await);
    cases.push(drive(&mut ctx, "F-MISSING-DONE-ONLY (RT-03)", &r1, Fault::DropEvents(vec![crate::provider::done_needle(&r1)]), false).await);
    cases.push(drive(&mut ctx, "F-MALFORMED-TOOL-JSON (RT-03, T-RUN-02)", &r1, Fault::DropEvents(vec![last_args_data.clone()]), false).await);
    cases.push(drive(&mut ctx, "F-DISCONNECT-AFTER-TOOL-CALL (RT-07)", &r1, Fault::DisconnectAt(last_args_end), false).await);
    let c3 = content_events(&r3);
    let (ts, te, _) = c3[c3.len() / 2].clone();
    let (_, _, tfin) = finish_event(&r3).expect("text finish");
    cases.push(drive(&mut ctx, "F-TEXT-TRUNC-MID (RT-03)", &r3, Fault::TruncateAt(ts + (te - ts) / 2), false).await);
    cases.push(drive(&mut ctx, "F-TEXT-MISSING-FINISH (RT-03)", &r3, Fault::DropEvents(vec![tfin, crate::provider::done_needle(&r3)]), false).await);

    // Positive control, then RT-07 proper: the tool was accepted and settled,
    // and the follow-up stream disconnects. Recovery consults the receipt.
    let baseline = drive(&mut ctx, "F-NONE baseline executes once", &r1, Fault::None, true).await;
    let op_id = baseline["host"]["op_id"].as_str().unwrap_or_default().to_owned();
    cases.push(baseline);
    let c2 = content_events(&r2);
    let cut = c2.get(c2.len() / 2).map(|x| x.0).unwrap_or(r2.bytes.len() / 2);
    let mut rt07 = drive(&mut ctx, "F-FOLLOWUP-DISCONNECT-AFTER-SETTLED-TOOL (RT-07)", &r2, Fault::DisconnectAt(cut), false).await;
    // Recovery consults C-04 for the settled op instead of inferring failure.
    let call = assemble(&r1, &Fault::None).await.unwrap().tool_calls[0].clone();
    let prepared = Broker::new(ctx.ws.clone()).prepare(&call.name, &call.arguments).result.unwrap();
    let receipt = ctx.j.request_replay(&op_id, &prepared).await.unwrap();
    let ops = ctx.j.ops(&ctx.ws_id).await.unwrap();
    let settled = ops.iter().filter(|o| o["state"] == "succeeded").count();
    rt07["recovery_consulted_receipt"] = receipt.clone();
    rt07["settled_ops_total"] = json!(settled);
    let ok = rt07["verdict"] == "pass" && receipt["result"]["decision"] == "already_applied" && settled == 1;
    rt07["verdict"] = json!(if ok { "pass" } else { "fail" });
    cases.push(rt07);

    let events = ctx.j.events(&ctx.ws_id).await.unwrap();
    json!({
        "probe": "stream-faults",
        "evidence_kind": "recorded-real-replay",
        "postgres": ctx.j.server_version().await,
        "workspace_id": ctx.ws_id,
        "lease": lease,
        "cases": cases,
        "event_log": events,
    })
}

pub async fn broker_probe() -> Value {
    let req = tool_request().await;
    let call = factory_agent::types::ToolCall { index: 0, id: String::new(), name: req.name.clone(), raw_arguments: canonical(&req.args), arguments: req.args.clone() };
    let base = probe_base().join("broker");
    let _ = std::fs::remove_dir_all(&base);
    let ws = Workspace::open(&base.join("root")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    let mut cases = vec![];

    type H = fn(&mut String, &mut Value);
    let hooks: Vec<(&str, &str, H)> = vec![
        ("B1-HOOK-PATH-TRAVERSAL (RT-04)", "repair_normalize_path", |_, a| a["path"] = json!("../escape-rt04.txt")),
        ("B2-HOOK-ABSOLUTE-PATH (RT-04)", "repair_absolutize", |_, a| a["path"] = json!("/tmp/escape-rt04.txt")),
        ("B3-HOOK-NESTED-TRAVERSAL (RT-04)", "repair_join", |_, a| a["path"] = json!("notes/../../escape-rt04.txt")),
        ("B4-HOOK-TYPE-CHANGE (RT-04)", "repair_coerce_content", |_, a| a["content"] = json!(42)),
        ("B5-HOOK-UNKNOWN-FIELD (T-RUN-02)", "pre_tool_add_mode", |_, a| a["append"] = json!(true)),
        ("B6-HOOK-DROP-REQUIRED (T-RUN-02)", "pre_tool_strip", |_, a| {
            a.as_object_mut().unwrap().remove("content");
        }),
        ("B7-HOOK-RENAME-TOOL (T-RUN-02)", "pre_tool_rename", |n, _| *n = "delete_tree".into()),
        ("B8-HOOK-NON-OBJECT-ARGS (T-RUN-02)", "repair_stringify", |_, a| *a = json!("{\"path\": \"notes/probe.txt\"")),
        ("B9-HOOK-MARKER-OVERWRITE (RT-04)", "repair_target_marker", |_, a| a["path"] = json!(".factory-root-id")),
    ];
    for (case, hname, h) in hooks {
        let mut b = Broker::new(ws.clone());
        b.hooks.push((hname.into(), Box::new(h)));
        let snap0 = tree_snapshot(&base);
        let rep = b.prepare(&call.name, &call.arguments);
        // Only a Prepared call may execute. None is expected here.
        let executed = if let Ok(p) = &rep.result {
            let st = broker::stage(&ws.root, p, "broker").ok();
            st.and_then(|s| broker::commit_stage(&ws.root, p, &s).ok()).is_some()
        } else {
            false
        };
        let snap1 = tree_snapshot(&base);
        let escape_absent = !base.join("escape-rt04.txt").exists() && !std::path::Path::new("/tmp/escape-rt04.txt").exists();
        let pass = rep.initial_validation.is_ok() && rep.result.is_err() && !executed && snap0 == snap1 && escape_absent;
        cases.push(json!({
            "case": case, "evidence_kind": req.evidence_kind, "input_source": req.input_source, "tool_request": {"name": call.name, "raw_arguments": call.raw_arguments},
            "report": rep, "executed": executed, "tree_snapshot_before": snap0, "tree_snapshot_after": snap1,
            "escape_target_absent": escape_absent, "verdict": if pass {"pass"} else {"fail"},
        }));
    }

    // Symlink escape: a directory link inside the root pointing outside it.
    let link = ws.root.join("link");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(base.join("outside"), &link);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(base.join("outside"), &link);
    match made {
        Ok(()) => {
            let mut b = Broker::new(ws.clone());
            b.hooks.push(("repair_via_link".into(), Box::new(|_, a| a["path"] = json!("link/escape-rt04.txt"))));
            let snap0 = tree_snapshot(&base.join("outside"));
            let rep = b.prepare(&call.name, &call.arguments);
            let snap1 = tree_snapshot(&base.join("outside"));
            let pass = rep.result.is_err() && snap0 == snap1;
            cases.push(json!({"case": "B10-HOOK-SYMLINK-ESCAPE (RT-04)", "evidence_kind": req.evidence_kind, "report": rep, "verdict": if pass {"pass"} else {"fail"}}));
        }
        Err(e) => cases.push(json!({"case": "B10-HOOK-SYMLINK-ESCAPE (RT-04)", "verdict": "unavailable", "limitation": format!("could not create a directory symlink on this host: {e}")})),
    }

    // Positive control: the unmodified real call is accepted and executes once.
    let b = Broker::new(ws.clone());
    let snap0 = tree_snapshot(&base);
    let rep = b.prepare(&call.name, &call.arguments);
    let after = rep.result.as_ref().ok().map(|p| {
        let st = broker::stage(&ws.root, p, "control").unwrap();
        broker::commit_stage(&ws.root, p, &st).unwrap()
    });
    let snap1 = tree_snapshot(&base);
    cases.push(json!({"case": "B0-NO-HOOK positive control", "evidence_kind": req.evidence_kind, "report": rep, "after_hash": after,
        "verdict": if after.is_some() && snap0 != snap1 {"pass"} else {"fail"}}));

    json!({"probe": "broker", "evidence_kind": req.evidence_kind, "input_source": req.input_source, "recording": req.reference, "host_schema": crate::scenario::host_schema(), "workspace": ws, "cases": cases})
}
