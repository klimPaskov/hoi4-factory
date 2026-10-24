//! Bakes the HOI4-Mod-Setup pin from the workspace manifest into the crate.
//!
//! The pin lives in one place, `[workspace.dependencies] hoi4-mod-setup` and
//! `[workspace.metadata.hoi4-mod-setup-pin]` of the root Cargo.toml. Reading it here means the
//! revision the adapter reports can never differ from the revision Cargo builds.

use std::fs;
use std::path::PathBuf;

fn main() {
    let root: PathBuf = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join("..")
        .join("Cargo.toml");
    println!("cargo:rerun-if-changed={}", root.display());
    println!("cargo:rerun-if-changed=build.rs");
    let text = fs::read_to_string(&root).expect("read the workspace Cargo.toml");
    let doc: toml::Table = text.parse().expect("workspace Cargo.toml parses");
    let workspace = doc
        .get("workspace")
        .and_then(|v| v.as_table())
        .expect("[workspace]");
    let dep = workspace
        .get("dependencies")
        .and_then(|v| v.get("hoi4-mod-setup"))
        .and_then(|v| v.as_table())
        .expect("[workspace.dependencies] hoi4-mod-setup");
    let rev = dep
        .get("rev")
        .and_then(|v| v.as_str())
        .expect("hoi4-mod-setup must be pinned with `rev`");
    assert!(
        rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()),
        "the Setup pin must be a full 40 character commit id"
    );
    assert_eq!(
        dep.get("default-features").and_then(|v| v.as_bool()),
        Some(false),
        "the Setup dependency must build with default features off (no keyring, no desktop)"
    );
    let git = dep.get("git").and_then(|v| v.as_str()).expect("git url");
    let meta = workspace
        .get("metadata")
        .and_then(|v| v.get("hoi4-mod-setup-pin"))
        .and_then(|v| v.as_table())
        .expect("[workspace.metadata.hoi4-mod-setup-pin]");
    let text = |key: &str| -> String {
        meta.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("pin metadata `{key}` (string)"))
            .to_owned()
    };
    let flag = |key: &str| -> bool {
        meta.get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| panic!("pin metadata `{key}` (bool)"))
    };
    println!("cargo:rustc-env=SETUP_PIN_GIT={git}");
    println!("cargo:rustc-env=SETUP_PIN_REV={rev}");
    println!(
        "cargo:rustc-env=SETUP_PIN_REPOSITORY={}",
        text("repository")
    );
    println!("cargo:rustc-env=SETUP_PIN_BRANCH={}", text("branch"));
    println!(
        "cargo:rustc-env=SETUP_PIN_PULL_REQUEST={}",
        text("pull_request")
    );
    println!(
        "cargo:rustc-env=SETUP_PIN_PR105_HEAD={}",
        text("pr_105_head")
    );
    println!(
        "cargo:rustc-env=SETUP_PIN_HAS_PR105_FIX={}",
        flag("contains_pr_105_fix")
    );
    println!(
        "cargo:rustc-env=SETUP_PIN_DEFAULT_PLAN_BUILDER={}",
        flag("default_plan_builder")
    );
}
