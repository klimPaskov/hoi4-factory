//! Kept in its own test binary with a single test so setting a parent variable cannot race
//! with other threads reading the environment.

use std::time::Duration;

use factory_hoi4_tools::McpServerSpec;

#[test]
fn host_secrets_do_not_reach_the_child() {
    // SAFETY: this binary has one test and no other threads exist yet.
    unsafe { std::env::set_var("SECRET_SHOULD_NOT_LEAK", "hunter2") };
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("log");
    let mut spec = McpServerSpec::new(env!("CARGO_BIN_EXE_fake-mcp-server"), dir.path());
    spec.args = vec!["normal".into()];
    spec.env = vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("FAKE_MCP_LOG".into(), log.to_string_lossy().into_owned()),
    ];
    spec.startup_timeout = Duration::from_secs(10);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    rt.block_on(async {
        let (client, _) = factory_hoi4_tools::McpClient::start(&spec)
            .await
            .expect("start");
        tokio::time::sleep(Duration::from_millis(200)).await;
        client.shutdown().await;
    });
    let text = std::fs::read_to_string(&log).expect("log");
    let line = text
        .lines()
        .find(|l| l.starts_with("env "))
        .expect("env line");
    assert!(!line.contains("SECRET_SHOULD_NOT_LEAK"), "{line}");
    assert_eq!(line, "env FAKE_MCP_LOG,PATH");
}
