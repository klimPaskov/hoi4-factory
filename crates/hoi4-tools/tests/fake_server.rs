//! Fault tests against `fake-mcp-server`, a test double (see its docs), not a real MCP server.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use factory_hoi4_tools::profile::{
    EffectClass, ProfileMode, ProfileTool, RetryPolicy, ServerIdentity, ToolProfile,
};
use factory_hoi4_tools::{
    Health, McpClient, McpError, McpServerSpec, ToolError, ToolSession, ToolState, discover,
};
use serde_json::{Value, json};

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }
    fn log_path(&self) -> PathBuf {
        self.dir.path().join("fake.log")
    }
    fn log(&self) -> String {
        std::fs::read_to_string(self.log_path()).unwrap_or_default()
    }
    fn count(&self, prefix: &str) -> usize {
        self.log().lines().filter(|l| l.starts_with(prefix)).count()
    }
    fn spec(&self, mode: &str) -> McpServerSpec {
        let mut spec = McpServerSpec::new(env!("CARGO_BIN_EXE_fake-mcp-server"), self.dir.path());
        spec.args = vec![mode.into()];
        spec.env = vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            (
                "HOME".into(),
                self.dir.path().to_string_lossy().into_owned(),
            ),
            (
                "FAKE_MCP_LOG".into(),
                self.log_path().to_string_lossy().into_owned(),
            ),
        ];
        spec.startup_timeout = Duration::from_secs(10);
        spec.default_call_timeout = Duration::from_secs(10);
        spec.max_frame_bytes = 64 * 1024;
        spec
    }
}

fn tool(mcp: &str, exposed: &str, class: EffectClass, max_output: usize) -> ProfileTool {
    ProfileTool {
        mcp_name: mcp.into(),
        exposed_name: exposed.into(),
        description: "test".into(),
        effect_class: class,
        host_schema: json!({
            "type": "object",
            "properties": {"text": {"type": "string"}, "repeat": {"type": "integer", "maximum": 1000000}},
            "additionalProperties": false
        }),
        timeout: Duration::from_secs(5),
        max_output_bytes: max_output,
        retry: RetryPolicy { max_attempts: 2 },
        inject_workspace: false,
        expected_schema_sha256: None,
    }
}

fn profile(version: &str, max_output: usize) -> ToolProfile {
    ToolProfile {
        id: "fake.v1".into(),
        server: ServerIdentity {
            name: "fake-mcp".into(),
            version_req: version.into(),
        },
        mode: ProfileMode::ReadOnly,
        tools: vec![
            tool("fake.read", "fake_read", EffectClass::PureRead, max_output),
            tool("fake.job", "fake_job", EffectClass::DerivedJob, max_output),
            tool("fake.write", "fake_write", EffectClass::Write, max_output),
        ],
    }
}

fn soon() -> Instant {
    Instant::now() + Duration::from_secs(20)
}

#[cfg(unix)]
fn pid_gone(pid: i32) -> bool {
    for _ in 0..40 {
        // SAFETY: signal 0 only checks that the process exists.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

async fn wait_for_log(fx: &Fixture, prefix: &str) -> bool {
    for _ in 0..40 {
        if fx.count(prefix) > 0 {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn handshake_is_recorded_and_tools_listed() {
    let fx = Fixture::new();
    let (client, hs) = McpClient::start(&fx.spec("normal")).await.expect("start");
    assert_eq!(hs.protocol_version, "2025-06-18");
    assert_eq!(hs.server_name, "fake-mcp");
    assert_eq!(hs.server_version, "1.0.0");
    assert!(
        hs.instructions_sha256
            .as_deref()
            .is_some_and(|h| h.len() == 64)
    );
    let tools = client.list_tools().await.expect("list");
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0].schema_sha256.len(), 64);
    let report = client.shutdown().await;
    assert!(
        !report.killed,
        "a well-behaved server exits when stdin closes"
    );
}

#[tokio::test]
async fn only_the_spec_environment_reaches_the_server() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("normal")).await.expect("start");
    assert!(wait_for_log(&fx, "env ").await);
    let line = fx
        .log()
        .lines()
        .find(|l| l.starts_with("env "))
        .unwrap()
        .to_owned();
    let keys: Vec<&str> = line["env ".len()..].split(',').collect();
    for key in &keys {
        assert!(
            ["FAKE_MCP_LOG", "HOME", "PATH"].contains(key),
            "unexpected variable {key}"
        );
    }
    client.shutdown().await;
}

#[tokio::test]
async fn wrong_protocol_version_is_refused() {
    let fx = Fixture::new();
    let err = McpClient::start(&fx.spec("wrong_protocol_version"))
        .await
        .expect_err("refused");
    assert!(matches!(err, McpError::Handshake { .. }), "{err:?}");
    let inv = discover(&fx.spec("wrong_protocol_version"), &profile("1.0.0", 1000)).await;
    assert!(matches!(inv.health, Health::Unavailable { .. }));
    assert_eq!(inv.available_count(), 0);
}

#[tokio::test]
async fn a_missing_server_is_unavailable_not_an_error() {
    let fx = Fixture::new();
    let mut spec = fx.spec("normal");
    spec.program = Path::new("/nonexistent/node").into();
    let inv = discover(&spec, &profile("1.0.0", 1000)).await;
    assert!(matches!(inv.health, Health::Unavailable { .. }));
    let mut session = ToolSession::start(spec, profile("1.0.0", 1000)).await;
    assert!(session.exposed_tools().is_empty());
    let err = session
        .call("fake_read", &json!({"text":"a"}), soon())
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotAvailable { .. }), "{err:?}");
}

#[tokio::test]
async fn call_returns_content_and_inventory_is_healthy() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("normal"), profile("1.0.0", 100_000)).await;
    assert!(session.inventory().is_healthy());
    assert!(session.inventory().version_matches);
    // fake.write exists on the server and is Available in the inventory; it is the call-time
    // read-only check that refuses it.
    assert_eq!(session.exposed_tools().len(), 3);
    let r = session
        .call("fake_read", &json!({"text":"hi"}), soon())
        .await
        .expect("call");
    assert_eq!(r.attempts, 1);
    assert!(!r.output.is_error);
    assert_eq!(r.output.content[0]["text"], "hi");
    assert_eq!(session.records().len(), 1);
    assert_eq!(session.records()[0].attempts[0].outcome, "ok");
    session.shutdown().await;
}

#[tokio::test]
async fn timeout_sends_cancellation_and_reports_unknown_outcome() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("slow")).await.expect("start");
    let err = client
        .call_tool(
            "fake.read",
            json!({"text":"x"}),
            Some(Duration::from_millis(300)),
        )
        .await
        .unwrap_err();
    assert_eq!(err, McpError::Timeout { sent: true });
    assert!(
        wait_for_log(&fx, "cancelled").await,
        "server saw no cancellation"
    );
    // The session is still usable for other requests.
    assert!(client.is_alive());
    client.shutdown().await;
}

#[tokio::test]
async fn pure_read_retries_once_after_a_crash_by_restarting() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("crash_once"), profile("1.0.0", 100_000)).await;
    let first_pid = session.client_pid();
    let r = session
        .call("fake_read", &json!({"text":"again"}), soon())
        .await
        .expect("retry succeeds");
    assert_eq!(r.attempts, 2);
    assert_eq!(fx.count("spawn "), 2, "the server was restarted once");
    assert_eq!(fx.count("call "), 2);
    assert_ne!(first_pid, session.client_pid());
    let outcomes: Vec<_> = session.records()[0]
        .attempts
        .iter()
        .map(|a| a.outcome.as_str())
        .collect();
    assert_eq!(outcomes, ["process_exited", "ok"]);
    session.shutdown().await;
}

#[tokio::test]
async fn pure_read_retries_are_bounded() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("crash_on_call"), profile("1.0.0", 100_000)).await;
    let err = session
        .call("fake_read", &json!({"text":"x"}), soon())
        .await
        .unwrap_err();
    match err {
        ToolError::Mcp { error, attempts } => {
            assert_eq!(attempts, 2);
            assert!(matches!(error, McpError::ProcessExited { .. }), "{error:?}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fx.count("call "), 2);
    session.shutdown().await;
}

#[tokio::test]
async fn derived_job_is_never_retried() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("crash_on_call"), profile("1.0.0", 100_000)).await;
    let err = session
        .call("fake_job", &json!({"text":"x"}), soon())
        .await
        .unwrap_err();
    assert!(err.outcome_unknown());
    assert!(matches!(err, ToolError::Mcp { attempts: 1, .. }), "{err:?}");
    assert_eq!(fx.count("call "), 1);
    session.shutdown().await;
}

#[tokio::test]
async fn read_only_mode_refuses_a_write_before_the_server_sees_it() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("normal"), profile("1.0.0", 100_000)).await;
    let err = session
        .call("fake_write", &json!({"text":"x"}), soon())
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::ReadOnlyRefused { .. }), "{err:?}");
    assert_eq!(fx.count("call "), 0);
    session.shutdown().await;
}

#[tokio::test]
async fn invalid_and_unknown_requests_never_reach_the_server() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("normal"), profile("1.0.0", 100_000)).await;
    for bad in [
        json!({"text":"x","workspaceId":"other"}),
        json!({"text": 5}),
        json!([]),
    ] {
        let err = session.call("fake_read", &bad, soon()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments { .. }),
            "{bad}: {err:?}"
        );
    }
    let err = session
        .call("hoi4.source_lookup", &json!({}), soon())
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::UnknownTool(_)));
    assert_eq!(fx.count("call "), 0);
    session.shutdown().await;
}

#[tokio::test]
async fn schema_change_withholds_the_tool_until_rediscovery() {
    let fx = Fixture::new();
    let mut session =
        ToolSession::start(fx.spec("catalog_changes"), profile("1.0.0", 100_000)).await;
    assert!(
        session
            .call("fake_read", &json!({"text":"a"}), soon())
            .await
            .is_ok()
    );
    let refresh = session.refresh_catalog().await.expect("refresh");
    assert_eq!(refresh.withheld, ["fake_read"]);
    assert!(
        session
            .exposed_tools()
            .iter()
            .all(|t| t.name != "fake_read")
    );
    let err = session
        .call("fake_read", &json!({"text":"a"}), soon())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, ToolError::NotAvailable { reason, .. } if reason == "schema_changed"),
        "{err:?}"
    );
    // Rediscovery restarts the server, whose first listing is the baseline again.
    session.rediscover().await;
    assert!(
        session
            .call("fake_read", &json!({"text":"a"}), soon())
            .await
            .is_ok()
    );
    session.shutdown().await;
}

#[tokio::test]
async fn a_version_other_than_the_pin_withholds_every_tool() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("other_version"), profile("1.0.0", 1000)).await;
    let inv = session.inventory();
    assert!(inv.is_healthy());
    assert!(!inv.version_matches);
    assert_eq!(inv.available_count(), 0);
    assert!(inv.tools.iter().all(
        |t| matches!(&t.state, ToolState::Withheld { reason } if reason == "version_mismatch")
    ));
    assert!(session.exposed_tools().is_empty());
    let err = session
        .call("fake_read", &json!({"text":"a"}), soon())
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotAvailable { .. }));
    assert_eq!(fx.count("call "), 0);
    session.shutdown().await;
}

#[tokio::test]
async fn tools_outside_the_profile_are_counted_not_exposed() {
    let fx = Fixture::new();
    let mut p = profile("1.0.0", 1000);
    p.tools.retain(|t| t.mcp_name == "fake.read");
    p.tools.push(tool(
        "fake.absent",
        "fake_absent",
        EffectClass::PureRead,
        1000,
    ));
    let inv = discover(&fx.spec("normal"), &p).await;
    assert_eq!(inv.ignored_tool_count, 2);
    assert_eq!(
        inv.tool("fake_absent").map(|t| &t.state),
        Some(&ToolState::Missing)
    );
    assert_eq!(inv.available_count(), 1);
}

#[tokio::test]
async fn garbage_stdout_ends_the_session() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("garbage_stdout"))
        .await
        .expect("start");
    let pid = client.pid().expect("pid");
    let err = client
        .call_tool("fake.read", json!({}), None)
        .await
        .unwrap_err();
    assert!(matches!(err, McpError::Protocol { .. }), "{err:?}");
    assert!(!client.is_alive());
    let again = client
        .call_tool("fake.read", json!({}), None)
        .await
        .unwrap_err();
    assert!(matches!(again, McpError::Protocol { .. }));
    // Shutdown reaps the killed process; until then it is a zombie that still answers kill(0).
    client.shutdown().await;
    #[cfg(unix)]
    assert!(pid_gone(pid as i32), "the violating server was killed");
}

#[tokio::test]
async fn an_oversized_line_ends_the_session() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("oversized_line"))
        .await
        .expect("start");
    let err = client
        .call_tool("fake.read", json!({}), None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, McpError::Protocol { ref message } if message.contains("exceeded")),
        "{err:?}"
    );
    client.shutdown().await;
}

#[tokio::test]
async fn large_output_is_truncated_and_flagged() {
    let fx = Fixture::new();
    let mut session = ToolSession::start(fx.spec("normal"), profile("1.0.0", 2_000)).await;
    let r = session
        .call(
            "fake_read",
            &json!({"text":"0123456789","repeat":5000}),
            soon(),
        )
        .await
        .expect("call");
    assert!(r.output.truncated);
    assert!(r.output.bytes > 50_000);
    let kept: usize = r.output.content.iter().map(|c| c.to_string().len()).sum();
    assert!(kept <= 2_000, "{kept}");
    assert!(session.records()[0].truncated);
    session.shutdown().await;
}

#[tokio::test]
async fn server_requests_are_answered_or_refused_without_confusing_results() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("server_request"))
        .await
        .expect("start");
    let out = client
        .call_tool("fake.read", json!({"text":"ok"}), None)
        .await
        .expect("call");
    assert_eq!(out.content[0]["text"], "ok");
    let log = fx.log();
    let roots = log
        .lines()
        .find(|l| l.contains("\"srv-1\""))
        .expect("roots reply");
    let v: Value = serde_json::from_str(roots.trim_start_matches("roots_reply ")).unwrap();
    let uri = v["result"]["roots"][0]["uri"].as_str().unwrap();
    assert!(uri.starts_with("file://"), "{uri}");
    let refused = log
        .lines()
        .find(|l| l.contains("\"srv-2\""))
        .expect("refusal");
    assert!(refused.contains("-32601"), "{refused}");
    client.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_kills_the_whole_process_group() {
    let fx = Fixture::new();
    let (client, _) = McpClient::start(&fx.spec("stubborn")).await.expect("start");
    assert!(wait_for_log(&fx, "helper ").await);
    let log = fx.log();
    let pid_of = |prefix: &str| -> i32 {
        log.lines()
            .find_map(|l| l.strip_prefix(prefix))
            .and_then(|p| p.trim().parse().ok())
            .expect(prefix)
    };
    let server = pid_of("spawn ");
    let helper = pid_of("helper ");
    let report = client.shutdown().await;
    assert!(
        report.killed,
        "a server that ignores stdin EOF must be killed"
    );
    assert!(pid_gone(server), "server still running");
    assert!(pid_gone(helper), "orphaned helper still running");
}
