//! Stage S02 gates G1 to G4 and account isolation for Setup, through the assembled server over
//! HTTP, against a real PostgreSQL (`FACTORY_TEST_ADMIN_URL`, see `http_api.rs`).
//!
//! What is real here: the server, the Setup facade at the pinned revision, Setup's
//! `DefaultPlanBuilder` fetching and verifying the real workflow package (network on first run;
//! files are cached), Setup's transaction engine, Git and the file system. GitHub is the intake
//! crate's local double.
//!
//! The analysis step needs a model reply. Tests marked `#[ignore]` call the Anthropic API with
//! the key from the user environment and record the reply (`FACTORY_RECORD_REPLIES=1`). The other
//! tests replay such a recording: the reply is the provider's bytes, with only `input_sha256`
//! rebound to the current request (see `support/setup_support.rs`). No reply is ever written by
//! hand.
#![allow(dead_code, private_interfaces)]

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");
include!("support/setup_support.rs");

use std::process::Command;

// ----- the fixture mod ---------------------------------------------------------------------

const BOM: &[u8] = b"\xEF\xBB\xBF";

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn asset_bytes() -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut state = b"gate fixture asset".to_vec();
    // A PNG signature first, so the upload policy sees an image and not a binary of unknown kind.
    let mut out = PNG_SIGNATURE.to_vec();
    while out.len() < 4096 {
        state = Sha256::digest(&state).to_vec();
        out.extend_from_slice(&state);
    }
    out.truncate(4096);
    out
}

fn fixture_files() -> Vec<(&'static str, Vec<u8>, u32)> {
    let mut descriptor = BOM.to_vec();
    descriptor.extend_from_slice(
        b"name=\"Gate Fixture\"\r\nversion=\"0.3.1\"\r\nsupported_version=\"1.19.*\"\r\ntags={\r\n\t\"Alternative History\"\r\n}\r\n",
    );
    let mut loc = BOM.to_vec();
    loc.extend_from_slice(b"l_english:\r\n gate_focus_title:0 \"Gate focus\"\r\n");
    vec![
        ("descriptor.mod", descriptor, 0o644),
        (
            "common/national_focus/gate_focus.txt",
            b"focus_tree = {\r\n\tid = gate_tree\r\n\tfocus = {\r\n\t\tid = gate_focus\r\n\t}\r\n}\r\n".to_vec(),
            0o644,
        ),
        ("localisation/english/gate_l_english.yml", loc, 0o644),
        ("gfx/gate_asset.png", asset_bytes(), 0o644),
        (
            "AGENTS.md",
            b"# My own agent instructions\n\nKeep exactly these words.\n".to_vec(),
            0o644,
        ),
        (
            ".agents/skills/gate-custom/SKILL.md",
            b"---\nname: gate-custom\ndescription: a custom skill of the author\n---\nDo the custom thing.\n".to_vec(),
            0o644,
        ),
        ("README.md", b"# Gate Fixture\n\nMy readme.\n".to_vec(), 0o644),
        ("docs/author-notes.md", b"# Notes\n\nNot for the package.\n".to_vec(), 0o644),
        ("notes/idea.txt", "Ideas \u{2014} \u{00e9}\u{00e8} \u{4e2d}\u{6587}\n".as_bytes().to_vec(), 0o644),
    ]
}

fn fixture_zip() -> Vec<u8> {
    let entries: Vec<ZEntry> = fixture_files()
        .into_iter()
        .map(|(name, data, mode)| ZEntry::file(name, data).mode(0o100000 | mode))
        .collect();
    build_zip(&entries)
}

async fn upload_fixture(h: &Harness, user: &User, name: &str) -> String {
    let project = h.project(user, name).await;
    let resp = user
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::text("git_init_consent", "true"),
                Part::file("gate.zip", fixture_zip()),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    project
}

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let empty = dir.parent().unwrap().join("empty.gitconfig");
    let _ = std::fs::write(&empty, "");
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &empty)
        .env("GIT_AUTHOR_NAME", "gate")
        .env("GIT_AUTHOR_EMAIL", "gate@example.invalid")
        .env("GIT_COMMITTER_NAME", "gate")
        .env("GIT_COMMITTER_EMAIL", "gate@example.invalid")
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[tokio::test]
#[ignore = "calls the Anthropic API with the user's key (exploration)"]
async fn probe_the_real_flow() {
    let key = live_key();
    let model = RecordingModel::new(&key, LIVE_MODEL);
    let h = Harness::start(Options {
        anthropic_key: Some("probe"),
        model: Some(model.clone()),
        setup_source: Some(PinnedSource::new().await),
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = upload_fixture(&h, &alice, "Probe").await;
    let flow = Flow {
        client: &alice.client,
        project: project.clone(),
    };
    let analysis = flow.scan_and_analyze("existing", None).await;
    println!("ANALYSIS {} {}", analysis.status, analysis.text);
    assert_eq!(analysis.status, 200);
    let plan = flow.confirm_and_plan(&analysis).await;
    println!("PLAN {} {}", plan.status, &plan.text[..plan.text.len().min(6000)]);
    model.save("probe", "exploration");
}
