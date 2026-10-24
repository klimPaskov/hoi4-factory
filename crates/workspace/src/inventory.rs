//! Snapshot inventory of a workspace (C-01 `source_snapshot`).
//!
//! The inventory lists every file under a working tree with its relative path, size,
//! mode and SHA-256, and classifies it through the Git CLI as tracked-clean,
//! tracked-modified, staged, untracked or ignored. It is read-only: files are only read,
//! and Git runs with optional locks off so the real index is not refreshed. `.git/` is
//! excluded, but the SHA-256 of the index file is recorded so a change to the staging
//! area shows up in a comparison.
//!
//! Entries are sorted by path in byte order, so the same tree always yields the same
//! inventory and the same [`SnapshotInventory::content_hash`].
//!
//! Modes come from the file system on Unix (`st_mode & 0o7777`). Windows has no
//! executable bit, so files report 0o644, or 0o444 when read-only, and a mode comparison
//! is not meaningful there.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::WorkspaceError;
use crate::git::GitRepo;
use crate::paths::is_link_like;

/// Version of the serialized inventory shape.
pub const INVENTORY_SCHEMA_VERSION: u32 = 1;

/// Lowercase hexadecimal encoding.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 of a file's bytes, streamed.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

/// How Git sees a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// In the index and equal to it and to HEAD.
    TrackedClean,
    /// In the index, with unstaged changes in the working tree.
    TrackedModified,
    /// Has changes staged in the index. `FileEntry::also_modified` tells whether the
    /// working tree differs from the index as well.
    Staged,
    Untracked,
    Ignored,
    /// There is no repository, so Git has no opinion.
    Unmanaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    /// A symbolic link or reparse point. Never followed. The hash is of the link target.
    Symlink,
    /// A socket, device or similar. Not read, no hash.
    Other,
}

/// One file in the inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path with `/` separators, exactly as stored (never normalized).
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    pub mode: u32,
    /// SHA-256 of the contents. `None` for `EntryKind::Other`.
    pub sha256: Option<String>,
    pub state: FileState,
    /// With `Staged`: the working tree also differs from the index.
    pub also_modified: bool,
    /// The on-disk name is not valid UTF-8 and `path` is a lossy rendering.
    pub path_not_utf8: bool,
}

/// Inventory of one working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInventory {
    pub schema_version: u32,
    /// Files sorted by `path`.
    pub entries: Vec<FileEntry>,
    /// Paths in the index that are absent from the working tree.
    pub missing_tracked: Vec<String>,
    /// Directories that contain their own `.git` (nested repositories and submodule
    /// working trees). They are recorded and not descended into.
    pub nested_repositories: Vec<String>,
    /// SHA-256 of the repository's index file. `None` without a repository or index.
    pub git_index_sha256: Option<String>,
}

/// Difference between two inventories: `before.diff(&after)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryDiff {
    /// Present only in `after`.
    pub added: Vec<String>,
    /// Present only in `before`.
    pub removed: Vec<String>,
    /// In both, with different content (hash, size or kind).
    pub changed: Vec<String>,
    /// In both, with a different mode.
    pub mode_changed: Vec<String>,
    /// The Git index file differs.
    pub index_changed: bool,
}

impl InventoryDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.mode_changed.is_empty()
            && !self.index_changed
    }
}

impl SnapshotInventory {
    /// Capture the inventory of `root`. Pass the repository handle for that directory to
    /// get Git classifications, or `None` for an unmanaged tree.
    pub fn capture(root: &Path, repo: Option<&GitRepo>) -> Result<Self, WorkspaceError> {
        let mut walked = Vec::new();
        let mut nested = Vec::new();
        walk(root, "", &mut walked, &mut nested)?;

        let scan = match repo {
            Some(repo) => Some(repo.scan()?),
            None => None,
        };
        let git_index_sha256 = match repo {
            Some(repo) => repo.index_sha256()?,
            None => None,
        };

        let mut present: BTreeSet<&str> = BTreeSet::new();
        for entry in &walked {
            present.insert(entry.path.as_str());
        }
        let mut missing_tracked = Vec::new();
        let mut entries = Vec::with_capacity(walked.len());
        if let Some(scan) = &scan {
            for path in &scan.tracked {
                if !present.contains(path.as_str()) && !scan.gitlinks.contains(path) {
                    missing_tracked.push(path.clone());
                }
            }
        }
        for mut entry in walked {
            let (state, also_modified) = match &scan {
                None => (FileState::Unmanaged, false),
                Some(scan) => classify(scan, &entry.path),
            };
            entry.state = state;
            entry.also_modified = also_modified;
            entries.push(entry);
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        missing_tracked.sort();
        nested.sort();
        Ok(SnapshotInventory {
            schema_version: INVENTORY_SCHEMA_VERSION,
            entries,
            missing_tracked,
            nested_repositories: nested,
            git_index_sha256,
        })
    }

    pub fn get(&self, path: &str) -> Option<&FileEntry> {
        self.entries
            .binary_search_by(|e| e.path.as_str().cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }

    /// SHA-256 over path, kind, size, mode and content hash of every entry in order.
    /// Git state is not part of it, so the same bytes give the same hash whether or not
    /// they are committed. It identifies a file set exactly.
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for entry in &self.entries {
            let line = format!(
                "{}\0{:?}\0{}\0{:o}\0{}\n",
                entry.path,
                entry.kind,
                entry.size,
                entry.mode,
                entry.sha256.as_deref().unwrap_or("-")
            );
            hasher.update(line.as_bytes());
        }
        hex(&hasher.finalize())
    }

    /// Paths with the given state.
    pub fn paths_in_state(&self, state: FileState) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.state == state)
            .map(|e| e.path.as_str())
            .collect()
    }

    /// Compare with a later inventory. All lists are sorted.
    pub fn diff(&self, after: &SnapshotInventory) -> InventoryDiff {
        let before: BTreeMap<&str, &FileEntry> =
            self.entries.iter().map(|e| (e.path.as_str(), e)).collect();
        let later: BTreeMap<&str, &FileEntry> =
            after.entries.iter().map(|e| (e.path.as_str(), e)).collect();
        let mut diff = InventoryDiff {
            index_changed: self.git_index_sha256 != after.git_index_sha256,
            ..InventoryDiff::default()
        };
        for (path, old) in &before {
            match later.get(path) {
                None => diff.removed.push((*path).to_owned()),
                Some(new) => {
                    if old.sha256 != new.sha256 || old.size != new.size || old.kind != new.kind {
                        diff.changed.push((*path).to_owned());
                    }
                    if old.mode != new.mode {
                        diff.mode_changed.push((*path).to_owned());
                    }
                }
            }
        }
        for path in later.keys() {
            if !before.contains_key(path) {
                diff.added.push((*path).to_owned());
            }
        }
        diff
    }
}

fn classify(scan: &crate::git::RepoScan, path: &str) -> (FileState, bool) {
    if let Some((x, y)) = scan.status.get(path) {
        if *x != b'.' {
            return (FileState::Staged, *y != b'.');
        }
        if *y != b'.' {
            return (FileState::TrackedModified, false);
        }
    }
    if scan.tracked.contains(path) {
        return (FileState::TrackedClean, false);
    }
    if scan.untracked.contains(path) {
        return (FileState::Untracked, false);
    }
    if scan.ignored.contains(path) {
        return (FileState::Ignored, false);
    }
    // Not reported anywhere: treat as untracked rather than guessing it is clean.
    (FileState::Untracked, false)
}

#[cfg(unix)]
fn mode_of(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(meta: &fs::Metadata) -> u32 {
    if meta.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

fn walk(
    dir: &Path,
    prefix: &str,
    out: &mut Vec<FileEntry>,
    nested: &mut Vec<String>,
) -> Result<(), WorkspaceError> {
    let io = WorkspaceError::io("read workspace directory");
    let mut children = Vec::new();
    for entry in fs::read_dir(dir).map_err(io)? {
        children.push(entry.map_err(WorkspaceError::io("read workspace directory entry"))?);
    }
    let mut has_git = false;
    for child in &children {
        if child.file_name() == ".git" {
            has_git = true;
        }
    }
    if has_git && !prefix.is_empty() {
        nested.push(prefix.trim_end_matches('/').to_owned());
        return Ok(());
    }
    for child in children {
        let name_os = child.file_name();
        if name_os == ".git" {
            continue;
        }
        let name_lossy = name_os.to_string_lossy().into_owned();
        let not_utf8 = name_os.to_str().is_none();
        let rel = format!("{prefix}{name_lossy}");
        let meta = child
            .metadata()
            .map_err(WorkspaceError::io("read workspace entry metadata"))?;
        if is_link_like(&meta) {
            let target = fs::read_link(child.path()).unwrap_or_default();
            let mut hasher = Sha256::new();
            hasher.update(target.to_string_lossy().as_bytes());
            out.push(FileEntry {
                path: rel,
                kind: EntryKind::Symlink,
                size: target.to_string_lossy().len() as u64,
                mode: mode_of(&meta),
                sha256: Some(hex(&hasher.finalize())),
                state: FileState::Unmanaged,
                also_modified: false,
                path_not_utf8: not_utf8,
            });
        } else if meta.is_dir() {
            walk(&child.path(), &format!("{rel}/"), out, nested)?;
        } else if meta.is_file() {
            let sha256 =
                sha256_file(&child.path()).map_err(WorkspaceError::io("hash workspace file"))?;
            out.push(FileEntry {
                path: rel,
                kind: EntryKind::File,
                size: meta.len(),
                mode: mode_of(&meta),
                sha256: Some(sha256),
                state: FileState::Unmanaged,
                also_modified: false,
                path_not_utf8: not_utf8,
            });
        } else {
            out.push(FileEntry {
                path: rel,
                kind: EntryKind::Other,
                size: 0,
                mode: mode_of(&meta),
                sha256: None,
                state: FileState::Unmanaged,
                also_modified: false,
                path_not_utf8: not_utf8,
            });
        }
    }
    Ok(())
}
