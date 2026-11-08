//! T-ENG-01 commit-check cases on the linked engine.
//!
//! The cases that need the game's documentation run when `HOI4_GAME_ROOT_FOR_TEST` names the game
//! folder (read only). Without it they print a note and pass, as the other game tests do.

use std::path::{Path, PathBuf};

use factory_engine_check::{
    CheckRequest, CommitActor, CommitDecision, ENGINE_REV, Verdict, baseline_from, commit_gate,
    finding_delta, run_check,
};

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn focus_mod(root: &Path, triggers: &[&str]) {
    write(
        root,
        "descriptor.mod",
        "name=\"engine check fixture\"\nsupported_version=\"1.19.*\"\n",
    );
    let mut focuses = String::new();
    for (i, t) in triggers.iter().enumerate() {
        focuses.push_str(&format!(
            "\tfocus = {{\n\t\tid = ECF_focus_{i}\n\t\tx = {i}\n\t\ty = 0\n\t\tcost = 10\n\t\tavailable = {{ {t} = yes }}\n\t\tcompletion_reward = {{ add_political_power = 10 }}\n\t}}\n"
        ));
    }
    write(
        root,
        "common/national_focus/ecf.txt",
        &format!("focus_tree = {{\n\tid = ecf_tree\n\tcountry = {{ factor = 0 }}\n{focuses}}}\n"),
    );
}

fn game_root() -> Option<PathBuf> {
    match std::env::var_os("HOI4_GAME_ROOT_FOR_TEST").map(PathBuf::from) {
        Some(p) if p.is_dir() => Some(p),
        _ => {
            eprintln!("SKIPPED: set HOI4_GAME_ROOT_FOR_TEST to the game folder to run this case");
            None
        }
    }
}

#[test]
fn without_the_game_the_verdict_is_incomplete_never_pass() {
    let dir = tempfile::tempdir().unwrap();
    focus_mod(dir.path(), &["ecf_unknown_trigger"]);
    let report = run_check(&CheckRequest::new(dir.path(), None)).unwrap();
    assert_eq!(report.verdict, Verdict::Incomplete, "{:?}", report.blocking);
    assert!(!report.unknown.vocabulary_available);
    assert!(!report.unknown.game_layer);
    assert!(report.unknown.commands_unknown > 0, "{:?}", report.unknown);
    assert_eq!(report.report["schema"], "hoi4-engine-report/1");
    assert_eq!(report.report["verdict"], "incomplete");
    // Incomplete is recorded as is and never turned into a pass.
    assert_eq!(
        commit_gate(&report, CommitActor::Agent),
        CommitDecision::Allowed {
            verdict: Verdict::Incomplete
        }
    );
}

#[test]
fn a_missing_mod_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_check(&CheckRequest::new(dir.path().join("absent"), None)).is_err());
}

#[test]
fn an_unknown_scripted_trigger_blocks_an_agent_commit_but_not_a_person() {
    let Some(game) = game_root() else { return };
    let dir = tempfile::tempdir().unwrap();
    focus_mod(dir.path(), &["ecf_unknown_trigger"]);
    let report = run_check(&CheckRequest::new(dir.path(), Some(game))).unwrap();
    assert_eq!(report.verdict, Verdict::Fail);
    assert!(report.unknown.vocabulary_available && report.unknown.game_layer);
    assert_eq!(report.blocking.len(), 1, "{:?}", report.blocking);
    assert_eq!(report.blocking[0].code, "REF_UNKNOWN_TRIGGER");
    match commit_gate(&report, CommitActor::Agent) {
        CommitDecision::Blocked { findings } => {
            assert!(findings[0].contains("ecf_unknown_trigger"), "{findings:?}")
        }
        other => panic!("agent commit not blocked: {other:?}"),
    }
    assert_eq!(
        commit_gate(&report, CommitActor::Person),
        CommitDecision::Allowed {
            verdict: Verdict::Fail
        }
    );
}

#[test]
fn only_findings_outside_the_baseline_block() {
    let Some(game) = game_root() else { return };
    let dir = tempfile::tempdir().unwrap();
    focus_mod(dir.path(), &["ecf_old_trigger"]);
    let before = run_check(&CheckRequest::new(dir.path(), Some(game.clone()))).unwrap();
    assert_eq!(before.verdict, Verdict::Fail);
    let baseline = baseline_from(&before.findings);

    // The same file with its baseline finding still present passes the gate.
    let mut req = CheckRequest::new(dir.path(), Some(game));
    req.baseline = Some(baseline);
    let same = run_check(&req).unwrap();
    assert_ne!(same.verdict, Verdict::Fail, "{:?}", same.blocking);
    assert_eq!(same.baselined_count, 1);
    assert!(matches!(
        commit_gate(&same, CommitActor::Agent),
        CommitDecision::Allowed { .. }
    ));

    // A proposal that adds a second unknown trigger fails on the new finding only.
    focus_mod(dir.path(), &["ecf_old_trigger", "ecf_new_trigger"]);
    let after = run_check(&req).unwrap();
    assert_eq!(after.verdict, Verdict::Fail);
    assert_eq!(after.blocking.len(), 1);
    assert_eq!(
        after.blocking[0].subject.as_deref(),
        Some("ecf_new_trigger")
    );
    let delta = finding_delta(&before.findings, &after.findings);
    assert_eq!(delta.summary["introduced"], 1, "{}", delta.summary);
}

#[test]
fn the_linked_engine_matches_the_image_and_the_tool_profile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains(&format!("hoi4-agent-tools\", rev = \"{ENGINE_REV}\"")),
        "Cargo.toml links hoi4-engine from another commit than ENGINE_REV"
    );
    let profile = std::fs::read_to_string(root.join("crates/hoi4-tools/src/profile.rs")).unwrap();
    assert!(
        profile.contains(&format!("AGENT_TOOLS_REV: &str = \"{ENGINE_REV}\"")),
        "the tool profile pins another Agent Tools commit"
    );
    let version = profile
        .split("AGENT_TOOLS_VERSION: &str = \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("AGENT_TOOLS_VERSION");
    let dockerfile = std::fs::read_to_string(root.join("docker/server.Dockerfile")).unwrap();
    assert!(
        dockerfile.contains(&format!("ARG AGENT_TOOLS_VERSION={version}")),
        "the image installs another Agent Tools release than the profile"
    );
}
