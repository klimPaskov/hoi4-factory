//! S01 EXP-03 / EXP-04 runtime probe driver.
//!
//! Subcommands:
//!   record                 live: capture real OpenAI streams, auth refusal, live cancellation
//!   record-anthropic       live: the same captures against Anthropic Messages
//!   faults                 recorded-real-replay: stream faults through the loop and host
//!   broker                 recorded-real-replay: hook mutation and root policy (RT-04, T-RUN-02)
//!   journal --phase a|b    PostgreSQL effect journal, fencing, crash and restart (T-RUN-03/04)
//!   journal-crash-child    internal: crashes at a chosen point
//!   worker                 supervised worker process vs in-process (RT-08), overhead
//!   worker-child           internal: the worker process
//!   serve-replay --port P  local HTTP server replaying recordings with faults
//!   live-once [--provider openai]  live: one native model request (Anthropic by default), JSON to stdout (EXP-04)
//!   models-check           live: list models only (no inference, no spend), JSON to stdout

mod broker;
mod faults;
mod journal;
mod journal_probe;
mod live;
mod provider;
mod replay_server;
mod scenario;
mod util;
mod worker;

use serde_json::json;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("");
    let out_dir = util::evidence_dir("exp03").join("runs");
    match cmd {
        "record" => {
            let v = record().await;
            util::write_json(&out_dir.join("live-record.json"), &v);
        }
        "record-anthropic" => {
            let v = record_anthropic().await;
            util::write_json(&out_dir.join("live-record-anthropic.json"), &v);
        }
        "faults" => {
            let v = faults::run().await;
            util::write_json(&out_dir.join("stream-faults.json"), &v);
        }
        "broker" => {
            let v = faults::broker_probe().await;
            util::write_json(&out_dir.join("broker.json"), &v);
        }
        "journal" => {
            let phase = util::arg(&args, "--phase").unwrap_or_else(|| "a".into());
            let v = journal_probe::run(&phase).await;
            util::write_json(&out_dir.join(format!("journal-phase-{phase}.json")), &v);
        }
        "journal-crash-child" => journal_probe::crash_child(&args).await,
        "worker" => {
            let live = util::flag(&args, "--live");
            let v = worker::run(live).await;
            util::write_json(&out_dir.join("worker.json"), &v);
        }
        "worker-child" => worker::child(&args).await,
        "serve-replay" => {
            let port: u16 = util::arg(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(18431);
            replay_server::serve(port).await;
        }
        "models-check" => {
            let v = live::models_check().await;
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
        "live-once" => {
            let v = if util::arg(&args, "--provider").as_deref() == Some("openai") { live::live_once().await } else { live::live_once_anthropic().await };
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
        _ => {
            eprintln!("usage: runtime-probe <record|faults|broker|journal|worker|serve-replay|live-once>");
            std::process::exit(2);
        }
    }
}

async fn record() -> serde_json::Value {
    let mut spend = vec![];
    let r1 = live::record_one("openai-tool-turn", &scenario::tool_turn_body(), None).await;
    spend.push(r1["spend"].clone());
    let call = &r1["outcome"]["turn"]["tool_calls"][0];
    let mut r2 = json!(null);
    let mut tool_exec = json!(null);
    if call.is_object() {
        // The host executes the real tool call in a scratch workspace, then the
        // real tool result is sent back for the follow-up turn.
        let ws = broker::Workspace::open(&util::probe_base().join("record-ws")).unwrap();
        let b = broker::Broker::new(ws.clone());
        let rep = b.prepare(call["name"].as_str().unwrap(), &call["arguments"]);
        tool_exec = match &rep.result {
            Ok(p) => {
                let op = util::new_id("op");
                let st = broker::stage(&ws.root, p, &op).unwrap();
                let after = broker::commit_stage(&ws.root, p, &st).unwrap();
                json!({"ok": true, "path": p.rel_path, "sha256": after, "bytes": p.args["content"].as_str().unwrap().len()})
            }
            Err(r) => json!({"ok": false, "refusal": r}),
        };
        let body = scenario::followup_body(
            call["id"].as_str().unwrap(),
            call["raw_arguments"].as_str().unwrap(),
            &tool_exec,
        );
        r2 = live::record_one("openai-followup-turn", &body, None).await;
        spend.push(r2["spend"].clone());
    }
    let r3 = live::record_one("openai-text-turn", &scenario::text_body(), None).await;
    spend.push(r3["spend"].clone());
    let cancel = live::record_one("openai-cancelled-turn", &scenario::long_body(), Some(5)).await;
    spend.push(cancel["spend"].clone());
    let auth = live::auth_probe().await;
    json!({
        "evidence_kind": "live",
        "model": scenario::MODEL,
        "tool_turn": r1, "host_tool_execution": tool_exec, "followup_turn": r2, "text_turn": r3,
        "cancelled_turn": cancel, "auth_refusal": auth, "spend": spend,
    })
}

async fn record_anthropic() -> serde_json::Value {
    let mut spend = vec![];
    let r1 = live::record_anthropic("anthropic-tool-turn", &scenario::a_tool_turn_body(), None).await;
    spend.push(r1["spend"].clone());
    let call = &r1["outcome"]["turn"]["tool_calls"][0];
    let mut r2 = json!(null);
    let mut tool_exec = json!(null);
    if call.is_object() {
        // The host executes the real tool call in a scratch workspace through
        // the broker, then sends its real result back for the follow-up turn.
        let ws = broker::Workspace::open(&util::probe_base().join("record-anthropic-ws")).unwrap();
        let b = broker::Broker::new(ws.clone());
        let rep = b.prepare(call["name"].as_str().unwrap(), &call["arguments"]);
        tool_exec = match &rep.result {
            Ok(p) => {
                let op = util::new_id("op");
                let st = broker::stage(&ws.root, p, &op).unwrap();
                let after = broker::commit_stage(&ws.root, p, &st).unwrap();
                json!({"ok": true, "path": p.rel_path, "sha256": after, "bytes": p.args["content"].as_str().unwrap().len()})
            }
            Err(r) => json!({"ok": false, "refusal": r}),
        };
        let body = scenario::a_followup_body(call["id"].as_str().unwrap(), &call["arguments"], &tool_exec);
        r2 = live::record_anthropic("anthropic-followup-turn", &body, None).await;
        spend.push(r2["spend"].clone());
    }
    let r3 = live::record_anthropic("anthropic-text-turn", &scenario::a_text_body(), None).await;
    spend.push(r3["spend"].clone());
    let cancel = live::record_anthropic("anthropic-cancelled-turn", &scenario::a_long_body(), Some(8)).await;
    spend.push(cancel["spend"].clone());
    let auth = live::auth_probe_anthropic().await;
    json!({
        "evidence_kind": "live", "provider": factory_agent::anthropic::PROVIDER, "model_requested": scenario::A_MODEL,
        "tool_turn": r1, "host_tool_execution": tool_exec, "followup_turn": r2, "text_turn": r3,
        "cancelled_turn": cancel, "auth_refusal": auth, "spend": spend,
    })
}
