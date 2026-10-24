//! Writing a validated [`Plan`] to disk.
//!
//! Every byte is counted while it streams. A source that delivers more than it declared,
//! fewer than it declared, or bytes whose CRC-32 differs is refused, so a lying header
//! cannot expand past the limits that were checked against the declared sizes. Files are
//! created with `create_new`, directories one level at a time, and nothing is created
//! outside `dest` because every name already passed `validate_relative`.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::config::IntakeLimits;
use crate::error::{IntakeError, shorten};
use crate::model::Finding;
use crate::plan::{Plan, PlannedEntry};
use crate::util::hex;

/// Where entry bytes come from.
pub(crate) trait EntrySource {
    fn open(&mut self, origin: usize) -> Result<Box<dyn Read + '_>, IntakeError>;
    /// Declared CRC-32 of the entry, when the source has one.
    fn crc32(&self, origin: usize) -> Option<u32>;
}

/// One extracted file.
#[derive(Debug, Clone)]
pub(crate) struct FileRecord {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub in_git: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ExtractReport {
    pub files: Vec<FileRecord>,
    pub findings: Vec<Finding>,
    pub bytes: u64,
}

/// How file modes are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModePolicy {
    /// Keep the owner-execute bit of the source, normalising everything else
    /// (`0644` or `0755`).
    Preserve,
    /// Never executable (`0640`). For the retained copy of a folder upload.
    Retained,
}

const SNIFF_BYTES: usize = 4096;
const LFS_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1";

/// True when the leading bytes of a file identify a native executable or a script
/// interpreter line. Applied to every file outside `.git`.
pub(crate) fn looks_executable(head: &[u8]) -> bool {
    if head.starts_with(b"#!") || head.starts_with(b"\x7fELF") {
        return true;
    }
    const MACHO: [[u8; 4]; 6] = [
        [0xfe, 0xed, 0xfa, 0xce],
        [0xfe, 0xed, 0xfa, 0xcf],
        [0xce, 0xfa, 0xed, 0xfe],
        [0xcf, 0xfa, 0xed, 0xfe],
        [0xca, 0xfe, 0xba, 0xbe],
        [0xbe, 0xba, 0xfe, 0xca],
    ];
    if head.len() >= 4 && MACHO.iter().any(|magic| head[..4] == magic[..]) {
        return true;
    }
    if head.starts_with(b"MZ") && head.len() >= 0x40 {
        let offset = u32::from_le_bytes([head[0x3c], head[0x3d], head[0x3e], head[0x3f]]) as usize;
        if offset
            .checked_add(4)
            .is_some_and(|end| end <= head.len() && &head[offset..end] == b"PE\0\0")
        {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn create_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o750).create(path)
}

#[cfg(not(unix))]
fn create_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn create_file(path: &Path, mode: u32) -> std::io::Result<File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    // The umask may have narrowed the mode. Set it exactly.
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    Ok(file)
}

#[cfg(not(unix))]
fn create_file(path: &Path, _mode: u32) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// Create `rel` (and its ancestors) under `root`, one level at a time. Refuses a level
/// that already exists as anything but a real directory.
fn ensure_dir(root: &Path, rel: &str, made: &mut HashSet<String>) -> Result<(), IntakeError> {
    let mut prefix = String::new();
    let mut path = root.to_path_buf();
    for component in rel.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        path.push(component);
        if made.contains(&prefix) {
            continue;
        }
        match create_dir(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta =
                    fs::symlink_metadata(&path).map_err(IntakeError::io("inspect directory"))?;
                if factory_workspace::paths::is_link_like(&meta) || !meta.is_dir() {
                    return Err(IntakeError::PathConflict {
                        path: shorten(&prefix),
                    });
                }
            }
            Err(err) => {
                return Err(IntakeError::Io {
                    operation: "create directory",
                    source: err,
                });
            }
        }
        made.insert(prefix.clone());
    }
    Ok(())
}

/// Fill the head buffer to `SNIFF_BYTES` or end of input.
fn fill_head(reader: &mut impl Read, head: &mut Vec<u8>, cap: u64) -> std::io::Result<()> {
    let want = (SNIFF_BYTES as u64).min(cap) as usize;
    let mut buf = vec![0u8; want];
    let mut filled = 0;
    while filled < want {
        let read = reader.read(&mut buf[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    buf.truncate(filled);
    head.extend_from_slice(&buf);
    Ok(())
}

pub(crate) fn extract(
    plan: &Plan,
    source: &mut dyn EntrySource,
    dest: &Path,
    limits: &IntakeLimits,
    modes: ModePolicy,
) -> Result<ExtractReport, IntakeError> {
    let mut report = ExtractReport::default();
    let mut made: HashSet<String> = HashSet::new();
    if plan.has_git {
        ensure_dir(dest, ".git", &mut made)?;
    }
    for entry in plan.entries.iter().filter(|e| e.is_dir) {
        ensure_dir(dest, &entry.rel, &mut made)?;
    }
    for entry in plan.entries.iter().filter(|e| !e.is_dir) {
        extract_file(entry, source, dest, limits, modes, &mut made, &mut report)?;
    }
    Ok(report)
}

fn extract_file(
    entry: &PlannedEntry,
    source: &mut dyn EntrySource,
    dest: &Path,
    limits: &IntakeLimits,
    modes: ModePolicy,
    made: &mut HashSet<String>,
    report: &mut ExtractReport,
) -> Result<(), IntakeError> {
    if let Some((parent, _)) = entry.rel.rsplit_once('/') {
        ensure_dir(dest, parent, made)?;
    }
    let target = dest.join(&entry.rel);
    let declared_exec = entry.mode.is_some_and(|m| m & 0o100 != 0);
    let mode = match modes {
        ModePolicy::Preserve if declared_exec => 0o755,
        ModePolicy::Preserve => 0o644,
        ModePolicy::Retained => 0o640,
    };
    let crc_expected = source.crc32(entry.origin);
    let mut reader = source.open(entry.origin)?;
    let mut file = create_file(&target, mode).map_err(|err| {
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            IntakeError::DuplicateEntry {
                path: shorten(&entry.rel),
            }
        } else {
            IntakeError::Io {
                operation: "create file",
                source: err,
            }
        }
    })?;

    let mismatch = || IntakeError::SizeMismatch {
        path: shorten(&entry.rel),
    };
    // One byte past the declared size is enough to prove the source lied.
    let mut limited = reader.by_ref().take(entry.size.saturating_add(1));
    let mut sha = Sha256::new();
    let mut crc = crc32fast::Hasher::new();
    let mut head = Vec::new();
    fill_head(&mut limited, &mut head, entry.size.saturating_add(1))
        .map_err(IntakeError::io("read upload entry"))?;
    if !entry.in_git {
        if looks_executable(&head) {
            return Err(IntakeError::ExecutableRefused {
                path: shorten(&entry.rel),
            });
        }
        if head.starts_with(LFS_PREFIX) && entry.size < 1024 {
            report.findings.push(Finding::LfsPointer {
                path: entry.rel.clone(),
            });
        }
    }
    let mut written: u64 = head.len() as u64;
    let mut chunk = head;
    loop {
        if written > entry.size {
            return Err(mismatch());
        }
        report.bytes = report.bytes.saturating_add(chunk.len() as u64);
        if report.bytes > limits.max_total_bytes {
            return Err(IntakeError::TotalTooLarge {
                limit: limits.max_total_bytes,
            });
        }
        sha.update(&chunk);
        crc.update(&chunk);
        file.write_all(&chunk)
            .map_err(IntakeError::io("write file"))?;
        chunk.clear();
        chunk.resize(64 * 1024, 0);
        let read = limited.read(&mut chunk).map_err(|err| match err.kind() {
            // A damaged deflate stream or a CRC failure inside the decoder.
            std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof => mismatch(),
            _ => IntakeError::Io {
                operation: "read upload entry",
                source: err,
            },
        })?;
        if read == 0 {
            break;
        }
        chunk.truncate(read);
        written += read as u64;
    }
    if written != entry.size {
        return Err(mismatch());
    }
    if let Some(expected) = crc_expected
        && crc.finalize() != expected
    {
        return Err(mismatch());
    }
    file.flush().map_err(IntakeError::io("write file"))?;
    if !entry.in_git && declared_exec && modes == ModePolicy::Preserve {
        report.findings.push(Finding::ExecutableMode {
            path: entry.rel.clone(),
        });
    }
    report.files.push(FileRecord {
        path: entry.rel.clone(),
        size: written,
        sha256: hex(&sha.finalize()),
        in_git: entry.in_git,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_magic_is_recognised() {
        assert!(looks_executable(b"#!/bin/sh\n"));
        assert!(looks_executable(b"\x7fELF\x02"));
        assert!(looks_executable(&[0xcf, 0xfa, 0xed, 0xfe, 0, 0]));
        assert!(looks_executable(&[0xca, 0xfe, 0xba, 0xbe, 0, 0]));
        let mut pe = vec![0u8; 0x100];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c] = 0x80;
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        assert!(looks_executable(&pe));
    }

    #[test]
    fn ordinary_content_is_not() {
        assert!(!looks_executable(b""));
        assert!(!looks_executable(b"MZ Division\n"));
        assert!(!looks_executable(b"focus = { id = x }\r\n"));
        assert!(!looks_executable(b"DDS |\0\0\0"));
        let mut mz = vec![b'x'; 0x100];
        mz[..2].copy_from_slice(b"MZ");
        assert!(!looks_executable(&mz), "MZ without a PE header is text");
        assert!(!looks_executable(&[0x7f, b'E']));
    }
}
