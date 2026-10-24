//! Shared helpers for the integration tests. Each test builds its own temp directory.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use factory_workspace::{AccountId, ProjectId, StorageRoot, TaskId, Workspace};
use sha2::{Digest, Sha256};

pub const BOM: &[u8] = b"\xEF\xBB\xBF";

/// A temp data root with one created workspace.
pub struct Fx {
    pub tmp: tempfile::TempDir,
    pub root: StorageRoot,
    pub ws: Workspace,
}

impl Fx {
    pub fn new() -> Fx {
        let tmp = tempfile::tempdir().unwrap();
        let root = StorageRoot::new(tmp.path().join("data"));
        let ws = root.create_workspace(&account(), &project()).unwrap();
        Fx { tmp, root, ws }
    }

    pub fn src(&self) -> PathBuf {
        self.ws.source_dir()
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        write_file(&self.src(), rel, bytes);
    }

    pub fn read(&self, rel: &str) -> Vec<u8> {
        fs::read(self.src().join(rel)).unwrap()
    }
}

pub fn account() -> AccountId {
    AccountId::new("acct-0001").unwrap()
}

pub fn project() -> ProjectId {
    ProjectId::new("proj-0001").unwrap()
}

pub fn task(name: &str) -> TaskId {
    TaskId::new(name).unwrap()
}

pub fn write_file(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// File with a BOM and CRLF line endings, the shape of real HOI4 script and localisation.
pub fn bom_crlf(body_lines: &[&str]) -> Vec<u8> {
    let mut out = BOM.to_vec();
    for line in body_lines {
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Run raw git for test setup and observation, isolated from the machine's
/// configuration. It deliberately does NOT force `core.autocrlf=false`; callers that want
/// the dangerous behavior pass `-c core.autocrlf=true`.
pub fn tgit(dir: &Path, args: &[&str]) -> Vec<u8> {
    let out = tgit_raw(dir, args);
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

pub fn tgit_raw(dir: &Path, args: &[&str]) -> std::process::Output {
    let ceiling = dir.parent().unwrap_or(dir).to_path_buf();
    Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CEILING_DIRECTORIES", ceiling)
        .args([
            "-c",
            "user.name=Test User",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap()
}

pub fn tgit_text(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(tgit(dir, args))
        .unwrap()
        .trim()
        .to_owned()
}

/// Recursively snapshot every file under `dir` except `.git` entries, as path -> bytes.
pub fn snapshot_bytes(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                walk(base, &path, out);
            } else if entry.file_type().unwrap().is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}
