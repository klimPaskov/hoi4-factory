//! The agent's built-in workspace tools, run by the host inside one task worktree.
//!
//! `list_files` and `read_file` are pure reads. `write_file` is an idempotent write: its key is
//! the operation, its precondition is the expected hash of the file before the write (or that
//! the file is absent) and its postcondition is the hash of the new content, which is also how
//! an interrupted write is reconciled ([`observe`]). The precondition is checked again
//! immediately before the rename that publishes the new bytes (S01 EXP-02: a stale base must
//! never be overwritten).
//!
//! Every path is relative to the worktree root and goes through
//! [`factory_workspace::resolve`], which refuses traversal, links, Git metadata and names that
//! collide on case-insensitive file systems. Which paths may be written is the grant's
//! decision, checked by the caller before an intent is recorded. The functions here block and
//! run on the blocking pool.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use factory_workspace::{PathError, ResolveMode, resolve, sha256_file};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Largest file `read_file` returns whole. Larger files come back truncated, and say so.
pub const MAX_READ_BYTES: usize = 256 * 1024;
/// Largest content `write_file` accepts.
pub const MAX_WRITE_BYTES: usize = 1024 * 1024;
/// Most entries `list_files` returns.
pub const MAX_LIST_ENTRIES: usize = 2000;

/// The model-facing names.
pub const LIST_FILES: &str = "list_files";
pub const READ_FILE: &str = "read_file";
pub const WRITE_FILE: &str = "write_file";
/// The structured final report. It has no effect; the host validates it and ends the attempt.
pub const FINISH: &str = "finish";
pub const TOOL_VERSION: &str = "factory-workspace-tools/1";

#[derive(Debug, thiserror::Error)]
pub enum ToolFailure {
    #[error("the path is not allowed: {0}")]
    Path(#[from] PathError),
    #[error("the path is a directory")]
    IsDirectory,
    #[error("the path is not a directory")]
    NotDirectory,
    #[error("the content is larger than {MAX_WRITE_BYTES} bytes")]
    TooLarge,
    #[error("the file changed: expected {expected}, found {found}")]
    Stale { expected: String, found: String },
    #[error("file system error: {0}")]
    Io(#[from] io::Error),
}

impl ToolFailure {
    pub fn code(&self) -> &'static str {
        match self {
            ToolFailure::Path(_) => "path_refused",
            ToolFailure::IsDirectory => "is_directory",
            ToolFailure::NotDirectory => "not_directory",
            ToolFailure::TooLarge => "content_too_large",
            ToolFailure::Stale { .. } => "stale_base",
            ToolFailure::Io(_) => "io_error",
        }
    }
}

/// The state of one path, as the hash of its bytes or absent.
pub const ABSENT: &str = "absent";

/// Closed input schemas of the built-in tools. The broker refuses any schema that is not closed.
pub fn schemas() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            LIST_FILES,
            "List files and folders under a folder of the mod (relative path, empty for the root).",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "maxLength": 1024},
                    "recursive": {"type": "boolean"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        (
            READ_FILE,
            "Read a text file of the mod. Returns its content and SHA-256. Large files are truncated.",
            json!({
                "type": "object",
                "properties": {"path": {"type": "string", "minLength": 1, "maxLength": 1024}},
                "required": ["path"],
                "additionalProperties": false
            }),
        ),
        (
            WRITE_FILE,
            "Create or replace a whole file on the task's own branch. expected_sha256 is the SHA-256 \
             read_file returned, or \"absent\" for a new file; the write is refused if the file changed.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "minLength": 1, "maxLength": 1024},
                    "content": {"type": "string", "maxLength": MAX_WRITE_BYTES},
                    "expected_sha256": {"type": "string", "pattern": "^(absent|[0-9a-f]{64})$"}
                },
                "required": ["path", "content", "expected_sha256"],
                "additionalProperties": false
            }),
        ),
        (
            FINISH,
            "Finish the task with a short report. Call it exactly once, when the work is done or cannot continue.",
            json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "minLength": 1, "maxLength": 4000},
                    "changed_files": {
                        "type": "array",
                        "items": {"type": "string", "maxLength": 1024},
                        "maxItems": 200
                    },
                    "open_questions": {
                        "type": "array",
                        "items": {"type": "string", "maxLength": 1000},
                        "maxItems": 20
                    }
                },
                "required": ["summary", "changed_files", "open_questions"],
                "additionalProperties": false
            }),
        ),
    ]
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The current hash of `rel` (or [`ABSENT`]). Used for before and after hashes and to
/// reconcile an interrupted write.
pub fn observe(root: &Path, rel: &str) -> Result<String, ToolFailure> {
    let path = resolve(root, rel, ResolveMode::MayCreate)?;
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => Err(ToolFailure::IsDirectory),
        Ok(_) => Ok(sha256_file(&path)?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ABSENT.to_owned()),
        Err(err) => Err(err.into()),
    }
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub path: String,
    pub kind: &'static str,
    pub bytes: Option<u64>,
}

pub fn list_files(root: &Path, rel: &str, recursive: bool) -> Result<Value, ToolFailure> {
    let rel = rel.trim_matches('/');
    let dir = if rel.is_empty() {
        root.to_path_buf()
    } else {
        resolve(root, rel, ResolveMode::MustExist)?
    };
    if !fs::symlink_metadata(&dir)?.is_dir() {
        return Err(ToolFailure::NotDirectory);
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, String)> = vec![(dir, rel.to_owned())];
    while let Some((dir, prefix)) = stack.pop() {
        let mut names: Vec<_> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        names.sort();
        for name in names {
            let Some(name) = name.to_str().map(str::to_owned) else {
                continue;
            };
            if name == ".git" {
                continue;
            }
            let child_rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let meta = fs::symlink_metadata(dir.join(&name))?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if entries.len() >= MAX_LIST_ENTRIES {
                truncated = true;
                break;
            }
            if meta.is_dir() {
                entries.push(Entry {
                    path: child_rel.clone(),
                    kind: "dir",
                    bytes: None,
                });
                if recursive {
                    stack.push((dir.join(&name), child_rel));
                }
            } else {
                entries.push(Entry {
                    path: child_rel,
                    kind: "file",
                    bytes: Some(meta.len()),
                });
            }
        }
    }
    Ok(json!({ "entries": entries, "truncated": truncated }))
}

pub fn read_file(root: &Path, rel: &str) -> Result<Value, ToolFailure> {
    let path = resolve(root, rel, ResolveMode::MustExist)?;
    let meta = fs::symlink_metadata(&path)?;
    if meta.is_dir() {
        return Err(ToolFailure::IsDirectory);
    }
    let bytes = fs::read(&path)?;
    let sha256 = sha256_hex(&bytes);
    let truncated = bytes.len() > MAX_READ_BYTES;
    let shown = &bytes[..bytes.len().min(MAX_READ_BYTES)];
    // HOI4 source is UTF-8 (often with a BOM) or Windows-1252. Lossy decoding is labelled, and
    // the hash always covers the real bytes, so a write based on this read stays exact.
    let (content, encoding) = match std::str::from_utf8(shown) {
        Ok(text) => (text.to_owned(), "utf-8"),
        Err(_) => (String::from_utf8_lossy(shown).into_owned(), "not_utf8_lossy"),
    };
    Ok(json!({
        "path": rel,
        "sha256": sha256,
        "bytes": bytes.len(),
        "truncated": truncated,
        "encoding": encoding,
        "content": content,
    }))
}

/// What a write did.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WriteOutcome {
    pub before_sha256: String,
    pub after_sha256: String,
    /// False when the file already had exactly this content: nothing was written.
    pub changed: bool,
}

/// Writes `content` to `rel` when its current hash is `expected` (or [`ABSENT`]).
pub fn write_file(
    root: &Path,
    rel: &str,
    content: &str,
    expected: &str,
) -> Result<WriteOutcome, ToolFailure> {
    if content.len() > MAX_WRITE_BYTES {
        return Err(ToolFailure::TooLarge);
    }
    let path = resolve(root, rel, ResolveMode::MayCreate)?;
    let after = sha256_hex(content.as_bytes());
    let before = observe(root, rel)?;
    if before == after {
        return Ok(WriteOutcome {
            before_sha256: before,
            after_sha256: after,
            changed: false,
        });
    }
    if before != expected {
        return Err(ToolFailure::Stale {
            expected: expected.to_owned(),
            found: before,
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("the path has no parent"))?;
    fs::create_dir_all(parent)?;
    // The new parents were created by us; resolve again so a link swapped in meanwhile is caught.
    resolve(root, rel, ResolveMode::MayCreate)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    let now = observe(root, rel)?;
    if now != expected {
        return Err(ToolFailure::Stale {
            expected: expected.to_owned(),
            found: now,
        });
    }
    temp.persist(&path).map_err(|e| ToolFailure::Io(e.error))?;
    Ok(WriteOutcome {
        before_sha256: before,
        after_sha256: after,
        changed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_is_conditional_and_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let first = write_file(root, "common/a.txt", "one", ABSENT).unwrap();
        assert!(first.changed);
        assert_eq!(first.before_sha256, ABSENT);
        // Same content again: no write, same postcondition.
        let again = write_file(root, "common/a.txt", "one", ABSENT).unwrap();
        assert!(!again.changed);
        assert_eq!(again.after_sha256, first.after_sha256);
        // A stale base is refused and the file keeps its bytes.
        let stale = write_file(root, "common/a.txt", "two", ABSENT).unwrap_err();
        assert_eq!(stale.code(), "stale_base");
        assert_eq!(fs::read_to_string(root.join("common/a.txt")).unwrap(), "one");
        let ok = write_file(root, "common/a.txt", "two", &first.after_sha256).unwrap();
        assert!(ok.changed);
        assert_eq!(observe(root, "common/a.txt").unwrap(), sha256_hex(b"two"));
    }

    #[test]
    fn paths_outside_the_root_or_into_git_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["../x", "/etc/passwd", ".git/config", "a/../../b", "C:/x", "a\\b"] {
            assert!(write_file(dir.path(), bad, "x", ABSENT).is_err(), "{bad}");
            assert!(read_file(dir.path(), bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn read_reports_hash_and_lossy_encoding() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("w.txt"), b"caf\xe9").unwrap();
        let v = read_file(dir.path(), "w.txt").unwrap();
        assert_eq!(v["encoding"], "not_utf8_lossy");
        assert_eq!(v["sha256"], sha256_hex(b"caf\xe9"));
    }

    #[test]
    fn list_skips_git_and_links() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("events")).unwrap();
        fs::write(dir.path().join("events/e.txt"), "x").unwrap();
        let v = list_files(dir.path(), "", true).unwrap();
        let paths: Vec<_> = v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["path"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(paths, vec!["events", "events/e.txt"]);
    }
}
