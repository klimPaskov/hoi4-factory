//! Shared helpers for the integration tests.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use factory_intake::{
    FileTypePolicy, IntakeLimits, UploadFailure, UploadIntake, UploadMeta, UploadOptions,
    UploadOutcome, UploadTarget,
};
use factory_workspace::{AccountId, ProjectId, StorageRoot};
use uuid::Uuid;

pub mod fake_github;

pub const BOM: &[u8] = b"\xEF\xBB\xBF";

// ---------------------------------------------------------------------------------------------
// A raw ZIP writer. The `zip` crate would refuse to write most of the hostile archives the
// tests need, and the reader under test must be checked against bytes it did not produce.

#[derive(Clone)]
pub struct ZEntry {
    pub name: Vec<u8>,
    pub data: Vec<u8>,
    /// 0 stored, 8 deflate, anything else is written as given with stored bytes.
    pub method: u16,
    pub host: u8,
    pub external: u32,
    pub flags: u16,
    /// Override of the sizes and CRC written to the headers (lying headers).
    pub declared_uncompressed: Option<u64>,
    pub declared_crc: Option<u32>,
    /// Pre-compressed bytes to store instead of compressing `data` (for crafted streams).
    pub raw_compressed: Option<Vec<u8>>,
}

impl ZEntry {
    pub fn file(name: &str, data: impl Into<Vec<u8>>) -> ZEntry {
        ZEntry {
            name: name.as_bytes().to_vec(),
            data: data.into(),
            method: 8,
            host: 3,
            external: 0o100644 << 16,
            flags: 0,
            declared_uncompressed: None,
            declared_crc: None,
            raw_compressed: None,
        }
    }

    pub fn stored(mut self) -> ZEntry {
        self.method = 0;
        self
    }

    pub fn dir(name: &str) -> ZEntry {
        let mut entry = ZEntry::file(name, Vec::new());
        entry.method = 0;
        entry.external = (0o040755 << 16) | 0x10;
        entry
    }

    pub fn mode(mut self, mode: u32) -> ZEntry {
        self.external = mode << 16;
        self
    }

    pub fn exec(self) -> ZEntry {
        self.mode(0o100755)
    }

    /// An entry whose Unix mode field says it is a symbolic link to `target`.
    pub fn symlink(name: &str, target: &str) -> ZEntry {
        ZEntry::file(name, target.as_bytes())
            .stored()
            .mode(0o120777)
    }

    pub fn special(name: &str, mode: u32) -> ZEntry {
        ZEntry::file(name, Vec::new()).stored().mode(mode)
    }
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

pub fn build_zip(entries: &[ZEntry]) -> Vec<u8> {
    build_zip_with(entries, false, None)
}

/// `zip64` writes the zip64 end records and sentinel values in the classic record.
/// `claimed_entries` overrides the entry count written to the end record(s).
pub fn build_zip_with(entries: &[ZEntry], zip64: bool, claimed_entries: Option<u64>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for entry in entries {
        let compressed: Vec<u8> = match (&entry.raw_compressed, entry.method) {
            (Some(raw), _) => raw.clone(),
            (None, 8) => deflate(&entry.data),
            (None, _) => entry.data.clone(),
        };
        let crc = entry
            .declared_crc
            .unwrap_or_else(|| crc32fast::hash(&entry.data));
        let uncompressed = entry
            .declared_uncompressed
            .unwrap_or(entry.data.len() as u64);
        let offset = out.len() as u32;
        // Local header.
        put32(&mut out, 0x0403_4b50);
        put16(&mut out, 20);
        put16(&mut out, entry.flags);
        put16(&mut out, entry.method);
        put16(&mut out, 0);
        put16(&mut out, 0x21);
        put32(&mut out, crc);
        put32(&mut out, compressed.len() as u32);
        put32(&mut out, uncompressed as u32);
        put16(&mut out, entry.name.len() as u16);
        put16(&mut out, 0);
        out.extend_from_slice(&entry.name);
        out.extend_from_slice(&compressed);
        // Central record.
        put32(&mut central, 0x0201_4b50);
        put16(&mut central, (u16::from(entry.host) << 8) | 20);
        put16(&mut central, 20);
        put16(&mut central, entry.flags);
        put16(&mut central, entry.method);
        put16(&mut central, 0);
        put16(&mut central, 0x21);
        put32(&mut central, crc);
        put32(&mut central, compressed.len() as u32);
        put32(&mut central, uncompressed as u32);
        put16(&mut central, entry.name.len() as u16);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, entry.external);
        put32(&mut central, offset);
        central.extend_from_slice(&entry.name);
    }
    let cd_offset = out.len() as u64;
    let cd_size = central.len() as u64;
    out.extend_from_slice(&central);
    let count = claimed_entries.unwrap_or(entries.len() as u64);
    if zip64 {
        let record_offset = out.len() as u64;
        put32(&mut out, 0x0606_4b50);
        put64(&mut out, 44);
        put16(&mut out, 45);
        put16(&mut out, 45);
        put32(&mut out, 0);
        put32(&mut out, 0);
        put64(&mut out, count);
        put64(&mut out, count);
        put64(&mut out, cd_size);
        put64(&mut out, cd_offset);
        put32(&mut out, 0x0706_4b50);
        put32(&mut out, 0);
        put64(&mut out, record_offset);
        put32(&mut out, 1);
        put32(&mut out, 0x0605_4b50);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, 0xFFFF);
        put16(&mut out, 0xFFFF);
        put32(&mut out, 0xFFFF_FFFF);
        put32(&mut out, 0xFFFF_FFFF);
        put16(&mut out, 0);
    } else {
        put32(&mut out, 0x0605_4b50);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, count as u16);
        put16(&mut out, count as u16);
        put32(&mut out, cd_size as u32);
        put32(&mut out, cd_offset as u32);
        put16(&mut out, 0);
    }
    out
}

/// Zip a directory tree (including `.git`) the way an archiver would.
pub fn zip_directory(root: &Path) -> Vec<u8> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<ZEntry>) {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        for path in names {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                out.push(ZEntry::dir(&format!("{rel}/")));
                walk(base, &path, out);
            } else {
                out.push(ZEntry::file(&rel, fs::read(&path).unwrap()));
            }
        }
    }
    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    build_zip(&entries)
}

// ---------------------------------------------------------------------------------------------
// Environment

pub struct Env {
    pub tmp: tempfile::TempDir,
    pub storage: StorageRoot,
    pub intake: UploadIntake,
    pub account: AccountId,
    pub project: ProjectId,
}

impl Env {
    pub fn new() -> Env {
        Env::with(IntakeLimits::default(), FileTypePolicy::default())
    }

    pub fn with_limits(limits: IntakeLimits) -> Env {
        Env::with(limits, FileTypePolicy::default())
    }

    pub fn with(limits: IntakeLimits, policy: FileTypePolicy) -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let storage = StorageRoot::new(tmp.path().join("data"));
        let intake = UploadIntake::new(storage.clone(), limits, policy);
        Env {
            tmp,
            storage,
            intake,
            account: AccountId::new("acct-0001").unwrap(),
            project: ProjectId::new("proj-0001").unwrap(),
        }
    }

    pub fn target(&self) -> UploadTarget<'_> {
        UploadTarget {
            account: &self.account,
            project: &self.project,
        }
    }

    pub fn meta(&self) -> UploadMeta {
        UploadMeta {
            upload_id: Uuid::new_v4(),
            original_name: Some("mod.zip".into()),
        }
    }

    pub fn project_dir(&self) -> PathBuf {
        self.storage
            .data_root()
            .join("workspaces")
            .join(self.account.as_str())
            .join(self.project.as_str())
    }

    pub fn source(&self) -> PathBuf {
        self.project_dir().join("source")
    }

    pub fn uploads_dir(&self) -> PathBuf {
        self.storage
            .data_root()
            .join("uploads")
            .join(self.account.as_str())
            .join(self.project.as_str())
    }

    pub fn zip(
        &self,
        bytes: &[u8],
        options: &UploadOptions,
    ) -> Result<UploadOutcome, UploadFailure> {
        let mut reader = std::io::Cursor::new(bytes.to_vec());
        self.intake
            .ingest_zip(&self.target(), &self.meta(), &mut reader, options)
    }

    pub fn zip_ok(&self, bytes: &[u8]) -> UploadOutcome {
        self.zip(bytes, &UploadOptions::default())
            .unwrap_or_else(|f| panic!("upload refused: {} ({})", f.code(), f.error))
    }

    /// Assert the upload was refused with `code` and left no workspace and no stored
    /// original behind.
    pub fn assert_refused(&self, result: Result<UploadOutcome, UploadFailure>, code: &str) {
        let failure = result.expect_err("the upload should have been refused");
        assert_eq!(failure.code(), code, "wrong code: {}", failure.error);
        assert!(
            !self.project_dir().exists(),
            "a refused upload left a workspace behind"
        );
        assert!(
            self.stored_originals().is_empty(),
            "a refused upload left its original behind: {:?}",
            self.stored_originals()
        );
    }

    pub fn stored_originals(&self) -> Vec<PathBuf> {
        match fs::read_dir(self.uploads_dir()) {
            Ok(read) => read.map(|e| e.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Git helpers (plain `git`, with conversions off)

pub fn git(dir: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["-c", "core.autocrlf=false", "-c", "core.safecrlf=false"])
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

pub fn git_text(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(git(dir, args)).unwrap().trim().to_owned()
}

/// Bytes of `path` at `rev`, exactly as stored in the object database.
pub fn blob(dir: &Path, rev: &str, path: &str) -> Vec<u8> {
    git(dir, &["cat-file", "blob", &format!("{rev}:{path}")])
}

pub fn write_file(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// All 256 byte values, twice, with a NUL in the middle: a stand-in for a .dds file.
pub fn binary_blob() -> Vec<u8> {
    let mut v: Vec<u8> = (0..=255u8).collect();
    v.extend((0..=255u8).rev());
    v
}
