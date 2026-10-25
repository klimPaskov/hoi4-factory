//! The native agent through the assembled server (S03 gates G1 to G5 and the W6 restart and
//! intervention cases), against a real PostgreSQL with row-level security and the real worker
//! process (`factory-server agent-worker`).
//!
//! The model API is a local replay server that serves recorded real Anthropic streams
//! (`tests/fixtures/recorded/agent-*`, recorded by `agent_record.rs`, and the S01 recordings of the
//! agent crate) in order, with faults injected into the real bytes where a case needs one.
//! Evidence kind: recorded-real replay. Loopback is allowed by `FACTORY_OUTBOUND_ALLOW_CIDRS` for
//! that one purpose; the destination tests check that everything else private stays refused.

#![allow(dead_code, unused_imports)]

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");

use factory_agent::testing::{Reply, ReplayServer};
use factory_agent::transport::Fault;
use factory_server::agent::{RunnerSettings, WorkerLauncher};

const DESCRIPTOR_MOD: &[u8] = b"name=\"Agent fixture\"\r\nsupported_version=\"1.19.*\"\r\n";
const FOCUS_TXT: &[u8] = b"focus_tree = {\r\n\tid = x\r\n}\r\n";

fn mod_entries() -> Vec<ZEntry> {
    vec![
        ZEntry::file("descriptor.mod", DESCRIPTOR_MOD),
        ZEntry::file("common/national_focus/x.txt", FOCUS_TXT),
        ZEntry::file("notes.txt", b"one\ntwo\n"),
    ]
}

fn recorded(name: &str) -> Vec<u8> {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let own = here.join("tests/fixtures/recorded").join(format!("{name}.sse"));
    let s01 = here.join("../agent/tests/fixtures/recorded").join(format!("{name}.sse"));
    std::fs::read(&own).or_else(|_| std::fs::read(&s01)).unwrap_or_else(|e| panic!("recording {name}: {e}"))
}

fn rec(name: &str) -> Reply {
    Reply::sse(recorded(name), None, Fault::None)
}

fn delayed(ms: u64, reply: Reply) -> Reply {
    Reply::Delayed(Duration::from_millis(ms), Box::new(reply))
}

fn stalled(name: &str, after: usize) -> Reply {
    Reply::Sse { bytes: recorded(name), chunk_sizes: None, fault: Fault::None, stall_after_bytes: Some(after) }
}

fn launcher(env: &[(&str, &str)]) -> WorkerLauncher {
    WorkerLauncher {
        program: PathBuf::from(env!("CARGO_BIN_EXE_factory-server")),
        args: vec!["agent-worker".into()],
        env: env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
    }
}

fn fast_settings() -> RunnerSettings {
    RunnerSettings {
        lease_ttl: Duration::from_secs(1),
        turn_timeout: Duration::from_secs(20),
        cancel_grace: Duration::from_secs(1),
        handshake_timeout: Duration::from_secs(10),
        safe_retries: 0,
    }
}

struct Agent {
    h: Harness,
    replay: ReplayServer,
    alice: User,
    project: String,
    _key: TempDir,
}

struct Setup {
    replies: Vec<Reply>,
    worker_env: Vec<(&'static str, &'static str)>,
    vars: Vec<(&'static str, String)>,
    settings: RunnerSettings,
}

impl Default for Setup {
    fn default() -> Self {
        Self { replies: vec![], worker_env: vec![], vars: vec![], settings: fast_settings() }
    }
}

async fn agent(setup: Setup) -> Agent {
    let replay = ReplayServer::start(vec![
        ("/v1/messages", Reply::Sequence(setup.replies)),
        ("/v1/models", Reply::json(r#"{"data":[{"id":"claude-haiku-5-5","type":"model"}],"has_more":false}"#)),
        ("/models", Reply::json(r#"{"object":"list","data":[{"id":"local-model","object":"model"}]}"#)),
    ])
    .await;
    let key_dir = tempfile::tempdir().unwrap();
    let key_file = key_dir.path().join("secret.key");
    std::fs::write(&key_file, "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=").unwrap();
    let mut vars = vec![
        ("FACTORY_ANTHROPIC_BASE_URL", replay.base_url.clone()),
        ("FACTORY_OUTBOUND_ALLOW_CIDRS", "127.0.0.0/8".to_owned()),
        ("FACTORY_SECRET_KEY_FILE", key_file.to_string_lossy().into_owned()),
    ];
    vars.extend(setup.vars);
    let h = Harness::start(Options {
        anthropic_key: Some("sk-ant-replay-not-a-real-key"),
        extra_vars: vars,
        agent: AgentOptions {
            worker: Some(launcher(&setup.worker_env)),
            settings: Some(setup.settings),
            tools: Some(None),
            skip_recovery: true,
        },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Agent fixture").await;
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::text("git_init_consent", "true"),
                Part::file("fixture.zip", build_zip(&mod_entries())),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    Agent { h, replay, alice, project, _key: key_dir }
}

impl Agent {
    fn tasks(&self) -> String {
        format!("/api/v1/projects/{}/tasks", self.project)
    }

    async fn task(&self, user: &User, body: Value) -> Value {
        let resp = user.client.post(&self.tasks(), body).await;
        assert_eq!(resp.status, 201, "{}", resp.text);
        resp.body
    }

    async fn edit_task(&self, paths: &[&str]) -> Value {
        self.task(&self.alice, json!({ "objective": "Add an idea file", "mode": "edit", "allowed_paths": paths })).await
    }

    async fn start(&self, user: &User, task: &Value) -> Resp {
        let id = task["id"].as_str().unwrap();
        user.client
            .post(&format!("{}/{id}/attempts", self.tasks()), json!({ "expected_version": task["version"] }))
            .await
    }

    async fn detail(&self, task_id: &str) -> Value {
        let resp = self.alice.client.get(&format!("{}/{task_id}", self.tasks())).await;
        assert_eq!(resp.status, 200, "{}", resp.text);
        resp.body
    }

    async fn events(&self, task_id: &str) -> Vec<Value> {
        let resp = self.alice.client.get(&format!("{}/{task_id}/events?after=0&limit=500", self.tasks())).await;
        assert_eq!(resp.status, 200, "{}", resp.text);
        resp.body["events"].as_array().unwrap().clone()
    }

    /// Polls until `done` holds for the newest attempt, or fails after `secs`.
    async fn wait(&self, task_id: &str, secs: u64, done: impl Fn(&Value) -> bool) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let d = self.detail(task_id).await;
            if let Some(a) = d["attempts"].as_array().and_then(|a| a.last())
                && done(a)
            {
                return d;
            }
            assert!(std::time::Instant::now() < deadline, "timed out waiting; last detail: {d:#}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn wait_state(&self, task_id: &str, state: &str) -> Value {
        self.wait(task_id, 40, |a| a["state"] == state).await
    }

    async fn wait_event(&self, task_id: &str, kind: &str) -> Vec<Value> {
        let deadline = std::time::Instant::now() + Duration::from_secs(40);
        loop {
            let events = self.events(task_id).await;
            if events.iter().any(|e| e["type"] == kind) {
                return events;
            }
            assert!(std::time::Instant::now() < deadline, "no {kind} event; events: {events:#?}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn worktree(&self, task_id: &str) -> PathBuf {
        self.h
            .data_dir
            .path()
            .join("workspaces")
            .join(&self.alice.id)
            .join(&self.project)
            .join("tasks")
            .join(task_id)
    }

    fn source(&self) -> PathBuf {
        self.h.workspace_source(&self.alice, &self.project)
    }

    fn runtime(&self) -> factory_server::agent::AgentRuntime {
        self.h.state.agent.clone().expect("the agent host started")
    }
}

fn types(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap().to_owned()).collect()
}

fn assert_contiguous(events: &[Value]) {
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e["seq"].as_i64(), Some(i as i64 + 1), "events are gapless and ordered: {events:#?}");
    }
}

/// Every file under `dir` (excluding `.git`) with its SHA-256.
fn tree(dir: &std::path::Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, sha256_hex(&std::fs::read(&path).unwrap()));
            }
        }
    }
    out
}

async fn count(pool: &PgPool, sql: &str, task: &str) -> i64 {
    sqlx::query_scalar(sql).bind(Uuid::parse_str(task).unwrap()).fetch_one(pool).await.unwrap()
}

// ----- G1, G2: a native run through the persisted bundle, writes only inside the grant -------

#[tokio::test]
async fn a_native_run_consumes_its_bundle_and_writes_only_inside_the_grant() {
    let a = agent(Setup { replies: vec![rec("agent-write-turn"), rec("agent-finish-turn")], ..Setup::default() }).await;
    let source_before = tree(&a.source());
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    assert_eq!(start.status, 201, "{}", start.text);
    assert_eq!(start.body["attempt"]["grant"]["operation_classes"], json!(["model_inference", "read_source", "run_read_only_tool", "write_task_worktree"]));
    let d = a.wait_state(&id, "succeeded").await;
    let attempt = &d["attempts"][0];
    assert_eq!(d["task"]["state"], "in_review", "a successful attempt does not complete the task");
    assert_eq!(attempt["outcome"]["changed_files"], json!(["common/ideas/agent_test.txt"]));
    assert!(attempt["outcome"]["branch"].as_str().unwrap().starts_with("hoi4factory/"));

    // The write landed in the task's own worktree and nowhere else.
    let written = std::fs::read_to_string(a.worktree(&id).join("common/ideas/agent_test.txt")).unwrap();
    assert_eq!(written, "# written by the HOI4 Factory agent test\n");
    assert_eq!(tree(&a.source()), source_before, "the main mod tree is untouched");
    let mut expected = source_before.clone();
    expected.insert("common/ideas/agent_test.txt".into(), sha256_hex(written.as_bytes()));
    assert_eq!(tree(&a.worktree(&id)), expected, "exactly one file changed in the worktree");

    // C-03: ordered, gapless, intent before its settlement.
    let events = a.events(&id).await;
    assert_contiguous(&events);
    let kinds = types(&events);
    let intent = kinds.iter().position(|k| k == "tool_intent").unwrap_or_else(|| panic!("{kinds:?}"));
    let settled = kinds.iter().position(|k| k == "tool_settled").unwrap();
    assert!(intent < settled, "{kinds:?}");
    assert!(kinds.contains(&"context_bundle_attached".to_owned()));

    // C-06: the stored bundle is the one the model received, and it verifies on load.
    let admin = a.h.admin().await;
    let bundle_id: Uuid = sqlx::query_scalar("SELECT bundle_id FROM agent_attempts WHERE task_id = $1")
        .bind(Uuid::parse_str(&id).unwrap())
        .fetch_one(&admin)
        .await
        .unwrap();
    let mut conn = admin.acquire().await.unwrap();
    let bundle = factory_tasks::load_bundle(&mut conn, factory_tasks::BundleId(bundle_id)).await.expect("the bundle verifies");
    drop(conn);
    assert_eq!(attempt["bundle"]["manifest_sha256"], json!(bundle.manifest_sha256));
    let bodies = a.replay.request_bodies("/v1/messages");
    assert_eq!(bodies.len(), 2, "two model turns, no repeats");
    assert!(bodies[0].contains(&bundle.id.to_string()), "the first request renders the persisted bundle");
    assert!(bodies[0].contains(&bundle.manifest_sha256));
    assert!(!bodies[0].contains("sk-ant-replay"), "no credential in the request body");

    // C-04 and metering: one settled receipt for the write; both turns metered on the allowance.
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_receipts r JOIN tool_intents i USING (operation_id) WHERE i.task_id = $1 AND i.tool = 'write_file' AND r.status = 'succeeded'", &id).await, 1);
    assert_eq!(count(&admin, "SELECT count(*) FROM model_usage WHERE task_id = $1 AND status = 'settled' AND funding = 'operator_allowance' AND actual_micros > 0", &id).await, 2);
    let usage = a.alice.client.get("/api/v1/account/usage").await;
    assert!(usage.body["operator_spent_micros"].as_i64().unwrap() > 0);
    assert_eq!(usage.body["user_key_spent_micros"], 0);
    admin.close().await;
}

#[tokio::test]
async fn writes_outside_the_grant_and_off_schema_calls_change_nothing() {
    // Turn 1 writes events/outside.txt (not granted); turn 2 is the S01 recording whose
    // write_file call lacks expected_sha256 (fails the closed schema); turn 3 finishes.
    let a = agent(Setup {
        replies: vec![rec("agent-write-outside-turn"), rec("anthropic-tool-turn"), rec("agent-finish-turn")],
        ..Setup::default()
    })
    .await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    assert_eq!(a.start(&a.alice, &task).await.status, 201);
    let d = a.wait_state(&id, "succeeded").await;
    assert_eq!(d["attempts"][0]["outcome"]["changed_files"], json!([]), "nothing was written");
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()), "zero writes in the worktree");
    let events = a.events(&id).await;
    let refused: Vec<&Value> = events.iter().filter(|e| e["type"] == "tool_refused").collect();
    assert_eq!(refused.len(), 2, "{events:#?}");
    assert_eq!(refused[0]["payload"]["code"], "path_not_granted");
    assert!(refused[1]["payload"]["reason"].as_str().unwrap().contains("schema"), "{:#}", refused[1]);
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_intents WHERE task_id = $1", &id).await, 0, "no intent for a refused call");
    admin.close().await;
}

#[tokio::test]
async fn a_read_only_task_has_no_write_tool() {
    let a = agent(Setup { replies: vec![rec("agent-write-turn"), rec("agent-read-turn"), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.task(&a.alice, json!({ "objective": "Look at the descriptor", "mode": "read_only" })).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    assert_eq!(start.body["attempt"]["grant"]["operation_classes"], json!(["model_inference", "read_source", "run_read_only_tool"]));
    a.wait_state(&id, "succeeded").await;
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()));
    let events = a.events(&id).await;
    let refused = events.iter().find(|e| e["type"] == "tool_refused").expect("the write is refused");
    assert!(refused["summary"].as_str().unwrap().contains("unknown tool"), "{refused:#}");
    // The read went through as a pure read with a receipt.
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_intents WHERE task_id = $1 AND tool = 'read_file' AND effect_class = 'pure_read'", &id).await, 1);
    admin.close().await;
}

// ----- G4: pause and revocation persist across restart --------------------------------------

#[tokio::test]
async fn a_pause_survives_a_restart_and_resume_does_not_repeat_the_turn() {
    let mut a = agent(Setup { replies: vec![delayed(1500, rec("agent-write-turn")), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    assert_eq!(start.status, 201, "{}", start.text);
    // Pause while the first model turn is in flight.
    a.wait_event(&id, "model_request").await;
    let version = a.detail(&id).await["task"]["version"].clone();
    let pause = a.alice.client.post(&format!("{}/{id}/pause", a.tasks()), json!({ "expected_version": version, "reason": "lunch" })).await;
    assert_eq!(pause.status, 200, "{}", pause.text);
    assert_eq!(pause.body["paused"], true);
    // The turn in flight settles; the next step does not start.
    a.wait_event(&id, "attempt_suspended").await;
    assert_eq!(a.replay.request_count("/v1/messages"), 1);
    let old = a.runtime();
    let old_generation = a.detail(&id).await["attempts"][0]["fencing_generation"].as_i64().unwrap();

    a.h.restart().await;
    let alice = a.h.rebind(&a.alice);
    a.alice = alice;
    factory_server::agent::recovery::recover_all(&a.runtime()).await.unwrap();
    let d = a.detail(&id).await;
    assert_eq!(d["task"]["paused"], true, "pause is durable");
    assert_eq!(d["attempts"][0]["state"], "running", "the suspended attempt is kept");
    assert_eq!(a.replay.request_count("/v1/messages"), 1, "a restart repeats nothing");

    // Resume: the new incarnation takes the lease (once the old one expires) and continues
    // from the saved conversation.
    let version = d["task"]["version"].clone();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let r = a.alice.client.post(&format!("{}/{id}/resume", a.tasks()), json!({ "expected_version": version })).await;
        if r.status == 200 {
            break;
        }
        assert_eq!(r.error_code(), "workspace_busy", "{}", r.text);
        assert!(std::time::Instant::now() < deadline);
        // The pause flag was already cleared by the refused attempt only if it succeeded.
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    a.wait_state(&id, "succeeded").await;
    assert_eq!(a.replay.request_count("/v1/messages"), 2, "turn 1 was not sent again");
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_intents WHERE task_id = $1 AND tool = 'write_file'", &id).await, 1, "one write, never repeated");
    admin.close().await;

    // ORCH-05: the old incarnation is fenced off the workspace now.
    let mut tx = old.tx(a.alice_account()).await.unwrap();
    let fenced = factory_tasks::check_fence(&mut tx, factory_accounts::ProjectId(Uuid::parse_str(&a.project).unwrap()), old.incarnation(), old_generation).await;
    assert!(fenced.is_err(), "the old host's generation is refused");
}

impl Agent {
    fn alice_account(&self) -> factory_accounts::AccountId {
        factory_accounts::AccountId(Uuid::parse_str(&self.alice.id).unwrap())
    }
}

#[tokio::test]
async fn revocation_stops_effects_and_persists_across_restart() {
    let mut a = agent(Setup { replies: vec![delayed(1500, rec("agent-write-turn")), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    let grant = start.body["attempt"]["grant"]["id"].as_str().unwrap().to_owned();
    // Revoke while the model turn is in flight, so the refusal lands on the effect.
    a.wait_event(&id, "model_request").await;
    let revoke = a.alice.client.post(&format!("{}/{id}/grants/{grant}/revoke", a.tasks()), json!({})).await;
    assert_eq!(revoke.status, 200, "{}", revoke.text);
    assert_eq!(revoke.body["revocation_epoch"], 2);
    let d = a.wait(&id, 40, |at| at["state"] == "failed" || at["state"] == "succeeded").await;
    assert_eq!(d["attempts"][0]["state"], "failed");
    assert_eq!(d["attempts"][0]["error_code"], "grant_revoked");
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()), "no effect after revocation");
    // The revocation is seen at the next check (the boundary before the turn's tool calls, or
    // the effect check itself); either way no intent was recorded and nothing was written.
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_intents WHERE task_id = $1", &id).await, 0);
    admin.close().await;
    assert!(a.events(&id).await.iter().any(|e| e["type"] == "grant_revoked"));

    a.h.restart().await;
    a.alice = a.h.rebind(&a.alice);
    let d = a.detail(&id).await;
    assert_eq!(d["attempts"][0]["grant"]["revoked"], true);
    assert_eq!(d["attempts"][0]["grant"]["revocation_epoch"], 2);
    // An effect presented with the old epoch is refused after the restart too.
    let rt = a.runtime();
    let mut tx = rt.tx(a.alice_account()).await.unwrap();
    let check = factory_broker::check_for_effect(&mut tx, factory_broker::GrantId(Uuid::parse_str(&grant).unwrap()), 1, factory_broker::OperationClass::WriteTaskWorktree, chrono::Utc::now()).await;
    assert!(matches!(check, Err(factory_broker::GrantRefusal::Revoked)), "{check:?}");
}

// ----- G3 and W6: process death, reconciliation, reopening ----------------------------------

#[tokio::test]
async fn an_interrupted_write_is_reconciled_after_a_restart_and_never_rerun() {
    let mut a = agent(Setup { replies: vec![delayed(4000, rec("agent-write-turn")), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    assert_eq!(start.status, 201, "{}", start.text);
    a.wait_event(&id, "model_request").await;

    // The process dies mid-turn: its runner and worker are gone without settling anything.
    let rt = a.runtime();
    rt.abort_all();
    let attempt = a.detail(&id).await["attempts"][0].clone();
    let attempt_id = factory_tasks::AttemptId(Uuid::parse_str(attempt["id"].as_str().unwrap()).unwrap());
    let task_id = factory_tasks::TaskId(Uuid::parse_str(&id).unwrap());
    let worktree = a.worktree(&id);

    // Its last acts, as the dead host left them: three write intents marked started. A's bytes
    // reached the disk, B's did not, and C's file was changed by someone else meanwhile.
    let content = "# written by the HOI4 Factory agent test\n";
    let intended = factory_server::agent::workspace_tools::sha256_hex(content.as_bytes());
    let mut ops = Vec::new();
    for (n, path) in ["common/ideas/a.txt", "common/ideas/b.txt", "common/ideas/c.txt"].iter().enumerate() {
        let mut tx = rt.tx(a.alice_account()).await.unwrap();
        let intent = factory_tasks::record_intent(&mut tx, factory_tasks::NewIntent {
            task_id,
            attempt_id,
            request_id: format!("toolu_dead_{n}"),
            idempotency_key: format!("{}:toolu_dead_{n}", attempt_id),
            replay: factory_tasks::ReplayIdentity {
                workspace_id: format!("workspace:{}", a.project),
                root_identity: format!("workspace:{}/tasks/{id}", a.project),
                source_snapshot: "dead".into(),
                tool: "write_file".into(),
                tool_version: factory_server::agent::workspace_tools::TOOL_VERSION.into(),
            },
            schema_version: "1".into(),
            effect_class: factory_tasks::EffectClass::IdempotentWrite,
            request_artifact: json!({ "path": path, "content": content, "expected_sha256": "absent" }),
            grant_id: Uuid::parse_str(attempt["grant"]["id"].as_str().unwrap()).unwrap(),
            grant_epoch: 1,
            fencing_generation: attempt["fencing_generation"].as_i64().unwrap(),
            retry_class: factory_tasks::RetryClass::ReconcileFirst,
            reconciliation_method: "observe_sha256".into(),
            observation_inputs: json!({ "path": path, "expected_before": "absent", "intended_after": intended }),
        })
        .await
        .unwrap();
        factory_tasks::mark_started(&mut tx, intent.operation_id).await.unwrap();
        tx.commit().await.unwrap();
        ops.push(intent.operation_id);
    }
    std::fs::create_dir_all(worktree.join("common/ideas")).unwrap();
    std::fs::write(worktree.join("common/ideas/a.txt"), content).unwrap();
    std::fs::write(worktree.join("common/ideas/c.txt"), "someone else\n").unwrap();
    let before_recovery = tree(&worktree);

    a.h.restart().await;
    a.alice = a.h.rebind(&a.alice);
    let left = factory_server::agent::recovery::recover_all(&a.runtime()).await.unwrap();
    // The dead incarnation's lease may still be live for a moment: recovery waits it out.
    if left > 0 {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(factory_server::agent::recovery::recover_all(&a.runtime()).await.unwrap(), 0);
    }
    let d = a.detail(&id).await;
    assert_eq!(d["attempts"][0]["state"], "failed");
    assert_eq!(d["attempts"][0]["error_code"], "host_restarted");
    assert_eq!(d["task"]["state"], "ready");
    assert_eq!(d["pending_effects"], json!([]), "everything is reconciled");
    assert_eq!(tree(&worktree), before_recovery, "recovery observes, it never writes");
    let admin = a.h.admin().await;
    let statuses: Vec<(Uuid, String)> = sqlx::query_as("SELECT operation_id, status FROM tool_receipts WHERE task_id = $1 ORDER BY started_at")
        .bind(task_id.0)
        .fetch_all(&admin)
        .await
        .unwrap();
    let status_of = |op: factory_tasks::OperationId| statuses.iter().find(|(o, _)| *o == op.0).unwrap().1.clone();
    assert_eq!(status_of(ops[0]), "reconciled_applied");
    assert_eq!(status_of(ops[1]), "reconciled_not_applied");
    assert_eq!(status_of(ops[2]), "reconciled_conflict");
    assert_eq!(count(&admin, "SELECT count(*) FROM model_usage WHERE task_id = $1 AND status = 'usage_unknown'", &id).await, 1, "the turn in flight stays unknown, never zero");
    assert!(a.events(&id).await.iter().any(|e| e["type"] == "recovery_report"));

    // Reopen: a new attempt receives the receipts and the conflict in its bundle, and nothing
    // is written again.
    let task_now = d["task"].clone();
    let start = a.start(&a.alice, &task_now).await;
    assert_eq!(start.status, 201, "{}", start.text);
    let d = a.wait(&id, 40, |at| at["attempt_no"] == 2 && at["state"] == "succeeded").await;
    let bundle_id = d["attempts"][1]["bundle"]["id"].as_str().unwrap();
    let content: Value = sqlx::query_scalar("SELECT bundle->'content' FROM context_bundles WHERE id = $1")
        .bind(Uuid::parse_str(bundle_id).unwrap())
        .fetch_one(&admin)
        .await
        .unwrap();
    let receipts = content["completed_receipts"].to_string();
    assert!(receipts.contains("reconciled_applied") && receipts.contains("reconciled_conflict"), "{content:#}");
    assert_eq!(a.replay.request_count("/v1/messages"), 2, "the dead turn was not repeated automatically");
    admin.close().await;
}

#[tokio::test]
async fn a_worker_that_ignores_cancellation_is_killed_and_the_stop_is_recorded() {
    let a = agent(Setup {
        replies: vec![stalled("agent-write-turn", 120)],
        worker_env: vec![("FACTORY_AGENT_WORKER_TEST_FAULT", "ignore_cancel")],
        ..Setup::default()
    })
    .await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    let attempt = start.body["attempt"]["id"].as_str().unwrap().to_owned();
    a.wait_event(&id, "model_request").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let resp = a.alice.client.post(&format!("{}/{id}/attempts/{attempt}/interrupt", a.tasks()), json!({})).await;
    assert_eq!(resp.status, 200, "{}", resp.text);
    assert_eq!(resp.body["state"], "interrupt_requested", "an accepted cancel is not a confirmed stop");
    let d = a.wait_state(&id, "cancelled").await;
    let stop = &d["attempts"][0]["outcome"]["cancel"];
    assert_eq!(stop["stop"], "killed", "{d:#}");
    if cfg!(unix) {
        assert_eq!(stop["signal"], 9);
    }
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM model_usage WHERE task_id = $1 AND status = 'usage_unknown'", &id).await, 1);
    admin.close().await;
}

#[tokio::test]
async fn a_cooperative_worker_stops_when_asked() {
    let a = agent(Setup { replies: vec![stalled("agent-write-turn", 120)], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let start = a.start(&a.alice, &task).await;
    let attempt = start.body["attempt"]["id"].as_str().unwrap().to_owned();
    a.wait_event(&id, "model_request").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    a.alice.client.post(&format!("{}/{id}/attempts/{attempt}/interrupt", a.tasks()), json!({})).await;
    let d = a.wait_state(&id, "cancelled").await;
    assert_eq!(d["attempts"][0]["outcome"]["cancel"]["stop"], "cooperative", "{d:#}");
    assert_eq!(d["task"]["state"], "ready");
}

#[tokio::test]
async fn a_result_from_a_stale_worker_generation_is_rejected() {
    let mut settings = fast_settings();
    settings.turn_timeout = Duration::from_secs(3);
    let a = agent(Setup {
        replies: vec![rec("agent-write-turn"), rec("agent-finish-turn")],
        worker_env: vec![("FACTORY_AGENT_WORKER_TEST_FAULT", "stale_generation")],
        settings,
        ..Setup::default()
    })
    .await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    let d = a.wait(&id, 40, |at| at["state"] == "failed").await;
    assert_eq!(d["attempts"][0]["error_code"], "worker_failed");
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()), "the stale result's tool call never ran");
    let admin = a.h.admin().await;
    assert_eq!(count(&admin, "SELECT count(*) FROM tool_intents WHERE task_id = $1", &id).await, 0);
    assert_eq!(count(&admin, "SELECT count(*) FROM model_usage WHERE task_id = $1 AND status = 'usage_unknown'", &id).await, 1);
    admin.close().await;
    assert_eq!(a.replay.request_count("/v1/messages"), 1, "no automatic repeat");
}

#[tokio::test]
async fn a_worker_that_dies_mid_turn_is_not_retried_and_its_cost_stays_unknown() {
    let a = agent(Setup {
        replies: vec![rec("agent-write-turn"), rec("agent-finish-turn")],
        worker_env: vec![("FACTORY_AGENT_WORKER_TEST_FAULT", "exit_mid_turn")],
        ..Setup::default()
    })
    .await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    let d = a.wait(&id, 40, |at| at["state"] == "failed").await;
    assert_eq!(d["attempts"][0]["error_code"], "worker_failed");
    let events = a.events(&id).await;
    let died = events.iter().find(|e| e["type"] == "worker_failed").unwrap();
    assert_eq!(died["payload"]["exit"], 3, "{died:#}");
    assert_eq!(a.replay.request_count("/v1/messages"), 1);
    let usage = a.alice.client.get("/api/v1/account/usage").await;
    assert_eq!(usage.body["unknown_count"], 1, "{}", usage.text);
}

#[tokio::test]
async fn a_stream_lost_after_acceptance_is_not_repeated() {
    let bytes = recorded("agent-write-turn");
    let a = agent(Setup { replies: vec![Reply::sse(bytes.clone(), None, Fault::DisconnectAt(bytes.len() / 2)), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    let d = a.wait(&id, 40, |at| at["state"] == "failed").await;
    assert_eq!(d["attempts"][0]["error_code"], "model_error");
    let events = a.events(&id).await;
    let err = events.iter().find(|e| e["type"] == "model_error").unwrap();
    assert_eq!(err["payload"]["retry_class"], "reconcile_first");
    assert_eq!(a.replay.request_count("/v1/messages"), 1, "an accepted request is never sent twice");
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()));
}

// ----- G5: budgets per task and per account, payers kept apart ------------------------------

#[tokio::test]
async fn an_exhausted_allowance_stops_dispatch_before_any_request() {
    let a = agent(Setup { replies: vec![rec("agent-write-turn")], vars: vec![("FACTORY_ALLOWANCE_MICROS", "1000".into())], ..Setup::default() }).await;
    let task = a.edit_task(&["common/ideas"]).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    let d = a.wait(&id, 40, |at| at["state"] == "failed").await;
    assert_eq!(d["attempts"][0]["error_code"], "budget_allowance_exhausted");
    assert_eq!(a.replay.request_count("/v1/messages"), 0);
    let usage = a.alice.client.get("/api/v1/account/usage").await;
    assert_eq!(usage.body["allowance_micros"], 1000);
    assert_eq!(usage.body["remaining_allowance_micros"], 1000);
}

#[tokio::test]
async fn a_task_budget_stops_dispatch_and_user_keys_are_metered_apart() {
    let a = agent(Setup { replies: vec![rec("agent-read-turn"), rec("agent-finish-turn"), rec("agent-read-turn"), rec("agent-finish-turn")], ..Setup::default() }).await;
    // A task whose budget is below one turn's reservation never dispatches.
    let tiny = a.task(&a.alice, json!({ "objective": "Look", "mode": "read_only", "budget_usd": 0.001 })).await;
    let tiny_id = tiny["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &tiny).await;
    let d = a.wait(&tiny_id, 40, |at| at["state"] == "failed").await;
    assert_eq!(d["attempts"][0]["error_code"], "budget_task_exhausted");
    assert_eq!(a.replay.request_count("/v1/messages"), 0);

    // The user's own key pays for this run; the allowance is untouched.
    let key = a.alice.client.post("/api/v1/account/api-keys", json!({ "provider": "anthropic", "label": "mine", "api_key": "sk-ant-user-test-key-0000" })).await;
    assert_eq!(key.status, 201, "{}", key.text);
    assert_eq!(key.body["key_hint"], "0000");
    assert!(!key.text.contains("sk-ant-user"), "the key is never returned");
    let key_id = key.body["id"].as_str().unwrap().to_owned();
    let own = a.task(&a.alice, json!({ "objective": "Look", "mode": "read_only", "model": { "profile_id": "anthropic:claude-haiku-5-5", "key_id": key_id } })).await;
    assert_eq!(own["model"]["funding"], "user_key");
    let own_id = own["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &own).await;
    a.wait_state(&own_id, "succeeded").await;
    let usage = a.alice.client.get("/api/v1/account/usage").await;
    assert!(usage.body["user_key_spent_micros"].as_i64().unwrap() > 0, "{}", usage.text);
    assert_eq!(usage.body["operator_spent_micros"], 0);

    // The allowance pays for the next one; user-key spend is unchanged.
    let user_spent = usage.body["user_key_spent_micros"].as_i64().unwrap();
    let op = a.task(&a.alice, json!({ "objective": "Look", "mode": "read_only" })).await;
    let op_id = op["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &op).await;
    a.wait_state(&op_id, "succeeded").await;
    let usage = a.alice.client.get("/api/v1/account/usage").await;
    assert!(usage.body["operator_spent_micros"].as_i64().unwrap() > 0);
    assert_eq!(usage.body["user_key_spent_micros"].as_i64().unwrap(), user_spent);

    // A deleted key can no longer fund a task.
    let del = a.alice.client.send_raw("DELETE", &format!("/api/v1/account/api-keys/{key_id}"), None, vec![], &[]).await;
    assert_eq!(del.status, 204, "{}", del.text);
    let again = a.alice.client.post(&a.tasks(), json!({ "objective": "Look", "mode": "read_only", "model": { "profile_id": "anthropic:claude-haiku-5-5", "key_id": key_id } })).await;
    assert_eq!(again.error_code(), "provider_key_unavailable", "{}", again.text);
    // The key text is nowhere in the database in clear.
    let admin = a.h.admin().await;
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM user_api_keys WHERE position('sk-ant-user' in encode(sealed, 'escape')) > 0").fetch_one(&admin).await.unwrap();
    assert_eq!(leaked, 0);
    admin.close().await;
}

// ----- isolation, destinations, capabilities, events ----------------------------------------

#[tokio::test]
async fn another_account_cannot_reach_tasks_keys_or_events() {
    let a = agent(Setup { replies: vec![rec("agent-read-turn"), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.task(&a.alice, json!({ "objective": "Look", "mode": "read_only" })).await;
    let id = task["id"].as_str().unwrap().to_owned();
    let key = a.alice.client.post("/api/v1/account/api-keys", json!({ "provider": "anthropic", "label": "mine", "api_key": "sk-ant-alice-1234" })).await;
    let key_id = key.body["id"].as_str().unwrap().to_owned();
    let bob = a.h.verified_user("bob@example.org").await;
    for path in [a.tasks(), format!("{}/{id}", a.tasks()), format!("{}/{id}/events", a.tasks())] {
        let r = bob.client.get(&path).await;
        assert_eq!(r.status, 404, "{path}: {}", r.text);
    }
    let r = bob.client.post(&format!("{}/{id}/attempts", a.tasks()), json!({ "expected_version": task["version"] })).await;
    assert_eq!(r.status, 404, "{}", r.text);
    let bob_project = a.h.project(&bob, "Bob").await;
    let r = bob.client.post(&format!("/api/v1/projects/{bob_project}/tasks"), json!({ "objective": "x", "mode": "read_only", "model": { "profile_id": "anthropic:claude-haiku-5-5", "key_id": key_id } })).await;
    assert_eq!(r.error_code(), "provider_key_unavailable", "{}", r.text);
    let r = bob.client.send_raw("DELETE", &format!("/api/v1/account/api-keys/{key_id}"), None, vec![], &[]).await;
    assert_eq!(r.status, 404, "{}", r.text);
    let keys = bob.client.get("/api/v1/account/api-keys").await;
    assert_eq!(keys.body["keys"], json!([]));
}

#[tokio::test]
async fn custom_endpoints_to_blocked_destinations_are_refused() {
    let a = agent(Setup::default()).await;
    for url in ["http://10.1.2.3:11434/v1", "http://169.254.169.254/v1", "http://[fd00::1]/v1", "http://user:pw@example.org/v1", "ftp://example.org/v1"] {
        let r = a.alice.client.post("/api/v1/account/api-keys", json!({ "provider": "openai_compatible", "label": "gpu", "api_key": "local-key-0001", "base_url": url })).await;
        assert_eq!(r.error_code(), "destination_blocked", "{url}: {}", r.text);
    }
    // Loopback is allowed in this test configuration only (FACTORY_OUTBOUND_ALLOW_CIDRS), which
    // stands in for an operator who lists a private range. The probe lists models and leaves
    // tool calls unknown: no inference is run on a self-hosted endpoint.
    let ok = a.alice.client.post("/api/v1/account/api-keys", json!({ "provider": "openai_compatible", "label": "local", "api_key": "local-key-0001", "base_url": format!("{}", a.replay.base_url) })).await;
    assert_eq!(ok.status, 201, "{}", ok.text);
    let probe = a.alice.client.post(&format!("/api/v1/account/api-keys/{}/probe", ok.body["id"].as_str().unwrap()), json!({})).await;
    assert_eq!(probe.status, 200, "{}", probe.text);
    assert_eq!(probe.body["models"], json!(["local-model"]));
    assert_eq!(probe.body["capabilities"]["model_listing"], "supported");
    assert_eq!(probe.body["capabilities"]["tool_calls"], "unknown");
    assert_eq!(a.replay.request_count("/chat/completions"), 0, "no inference");

    let profiles = a.alice.client.get("/api/v1/model-profiles").await;
    let list = profiles.body["profiles"].as_array().unwrap();
    let haiku = list.iter().find(|p| p["id"] == "anthropic:claude-haiku-5-5").unwrap();
    assert_eq!(haiku["default"], true);
    assert_eq!(haiku["operator_allowance"], true);
    assert_eq!(haiku["capabilities"]["tool_calls"], "supported");
    let openai = list.iter().find(|p| p["provider"] == "openai").unwrap();
    assert_eq!(openai["operator_allowance"], false, "no operator OpenAI key: user keys only");
    // A task on a profile without an operator key needs the user's own key.
    let r = a.alice.client.post(&a.tasks(), json!({ "objective": "x", "mode": "read_only", "model": { "profile_id": "openai:gpt-5.4-nano", "key_id": null } })).await;
    assert_eq!(r.error_code(), "provider_unavailable", "{}", r.text);
}

#[tokio::test]
async fn events_replay_in_order_from_a_cursor_and_the_stream_resumes() {
    let a = agent(Setup { replies: vec![rec("agent-read-turn"), rec("agent-finish-turn")], ..Setup::default() }).await;
    let task = a.task(&a.alice, json!({ "objective": "Look", "mode": "read_only" })).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    a.wait_state(&id, "succeeded").await;
    let all = a.events(&id).await;
    assert_contiguous(&all);
    let after = a.alice.client.get(&format!("{}/{id}/events?after=3", a.tasks())).await;
    let tail = after.body["events"].as_array().unwrap();
    assert_eq!(tail[0]["seq"], 4);
    assert_eq!(tail.len(), all.len() - 3);
    assert_eq!(after.body["last_seq"], all.len());

    // The stream resumes after Last-Event-ID with the next sequence number.
    let cookies: Vec<String> = a.alice.client.jar.lock().unwrap().iter().map(|(k, v)| format!("{k}={v}")).collect();
    let request = Request::builder()
        .uri(format!("{}/{id}/events/stream", a.tasks()))
        .header(header::COOKIE, cookies.join("; "))
        .header("last-event-id", "2")
        .body(Body::empty())
        .unwrap();
    let response = a.h.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    let mut body = response.into_body();
    let mut text = String::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !text.contains("id: 4") {
        assert!(std::time::Instant::now() < deadline, "stream so far: {text}");
        let frame = tokio::time::timeout(Duration::from_secs(5), body.frame()).await.unwrap().unwrap().unwrap();
        if let Ok(data) = frame.into_data() {
            text.push_str(&String::from_utf8_lossy(&data));
        }
    }
    let first_id = text.lines().find_map(|l| l.strip_prefix("id: ")).unwrap();
    assert_eq!(first_id, "3", "the stream starts right after the cursor: {text}");
}

// ----- the real HOI4 Agent Tools profile through the host ------------------------------------

/// Runs only where a local Agent Tools build is configured (`FACTORY_AGENT_TOOLS_NODE` and
/// `FACTORY_AGENT_TOOLS_ENTRY`, as for the hoi4-tools tests); prints a note and passes otherwise.
#[tokio::test]
async fn a_read_only_agent_tools_call_runs_through_the_broker_and_changes_no_source() {
    let Some(install) = factory_hoi4_tools::AgentToolsInstall::from_env() else {
        eprintln!("SKIPPED: FACTORY_AGENT_TOOLS_NODE / FACTORY_AGENT_TOOLS_ENTRY are not set");
        return;
    };
    let replay = vec![rec("agent-tools-call-turn"), rec("agent-tools-finish-turn")];
    let replay_server = ReplayServer::start(vec![("/v1/messages", Reply::Sequence(replay))]).await;
    let key_dir = tempfile::tempdir().unwrap();
    let key_file = key_dir.path().join("secret.key");
    std::fs::write(&key_file, "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=").unwrap();
    let h = Harness::start(Options {
        anthropic_key: Some("sk-ant-replay-not-a-real-key"),
        extra_vars: vec![
            ("FACTORY_ANTHROPIC_BASE_URL", replay_server.base_url.clone()),
            ("FACTORY_OUTBOUND_ALLOW_CIDRS", "127.0.0.0/8".to_owned()),
            ("FACTORY_SECRET_KEY_FILE", key_file.to_string_lossy().into_owned()),
        ],
        agent: AgentOptions {
            worker: Some(launcher(&[])),
            settings: Some(fast_settings()),
            tools: Some(Some(factory_server::agent::AgentToolsSettings { install, game_root: None })),
            skip_recovery: true,
        },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Agent tools").await;
    let up = alice
        .client
        .upload(&project, &[Part::text("kind", "zip"), Part::text("git_init_consent", "true"), Part::file("m.zip", build_zip(&mod_entries()))])
        .await;
    assert_eq!(up.status, 201, "{}", up.text);
    let a = Agent { h, replay: replay_server, alice, project, _key: key_dir };
    let task = a.task(&a.alice, json!({ "objective": "Summarize the mod", "mode": "read_only" })).await;
    let id = task["id"].as_str().unwrap().to_owned();
    a.start(&a.alice, &task).await;
    let d = a.wait(&id, 120, |at| matches!(at["state"].as_str(), Some("succeeded" | "failed"))).await;
    assert_eq!(d["attempts"][0]["state"], "succeeded", "{d:#}");
    let events = a.events(&id).await;
    let ready = events.iter().find(|e| e["type"] == "tools_ready").unwrap();
    let names: Vec<&str> = ready["payload"]["tools"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(names.contains(&"agent_tools_mod_index"), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("rewrite")), "no rewrite tool is exposed: {names:?}");
    let admin = a.h.admin().await;
    assert_eq!(
        count(&admin, "SELECT count(*) FROM tool_receipts r JOIN tool_intents i USING (operation_id) WHERE i.task_id = $1 AND i.tool = 'agent_tools_mod_index' AND r.status = 'succeeded' AND i.effect_class = 'pure_read'", &id).await,
        1
    );
    admin.close().await;
    // The tool kept its index outside the mod: the worktree is byte-identical to the source.
    assert_eq!(tree(&a.worktree(&id)), tree(&a.source()));
}
