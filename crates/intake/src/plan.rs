//! Validation of an upload's entry list before anything is written.
//!
//! Both upload kinds reduce to a list of [`Candidate`]s (name, kind, declared size). The
//! planner applies every name, kind, type and limit rule to that list and either returns a
//! [`Plan`] or the first reason to refuse. Path rules come from `factory-workspace`
//! ([`validate_relative`], [`fold_key`]), so the upload and the workspace agree on what a
//! safe name is.
//!
//! A `.git` directory at the top of the upload is split out as the repository the user
//! brought. Its `hooks/` subtree is dropped, because the server never runs hooks and a
//! script has no business in the workspace. Every other `.git` component is refused.

use std::collections::{HashMap, HashSet};

use factory_workspace::{PathError, fold_key, validate_relative};

use crate::config::{FileTypePolicy, IntakeLimits, TypeVerdict};
use crate::error::{IntakeError, shorten};
use crate::zip::DeclaredKind;

/// One entry of an upload as its source declares it.
#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub path: String,
    pub kind: DeclaredKind,
    /// Uncompressed bytes the source declares.
    pub size: u64,
    /// Compressed bytes, for archives.
    pub compressed: Option<u64>,
    /// Unix permission bits, when the source carries them.
    pub mode: Option<u32>,
    /// Index of the entry in the source.
    pub origin: usize,
}

/// One accepted entry.
#[derive(Debug, Clone)]
pub(crate) struct PlannedEntry {
    /// Path relative to the workspace `source/` directory, `/` separated. Entries of an
    /// uploaded repository start with `.git/`.
    pub rel: String,
    pub is_dir: bool,
    pub origin: usize,
    pub size: u64,
    pub mode: Option<u32>,
    pub in_git: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub entries: Vec<PlannedEntry>,
    pub has_git: bool,
    pub dropped_hook_files: u64,
    pub total_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
}

fn path_error(path: &str, err: PathError) -> IntakeError {
    IntakeError::PathRejected {
        path: shorten(path),
        reason: err.to_string(),
    }
}

/// Apply all rules. `archive_len` is the archive size for the whole-archive ratio check.
pub(crate) fn plan(
    candidates: &[Candidate],
    archive_len: Option<u64>,
    limits: &IntakeLimits,
    policy: &FileTypePolicy,
) -> Result<Plan, IntakeError> {
    if candidates.len() as u64 > limits.max_entries {
        return Err(IntakeError::TooManyEntries {
            limit: limits.max_entries,
        });
    }
    let mut plan = Plan::default();
    // Folded prefix -> the spelling first seen. Detects collisions at every level.
    let mut prefixes: HashMap<String, String> = HashMap::new();
    let mut exact: HashMap<String, bool> = HashMap::new();
    let mut file_keys: HashSet<String> = HashSet::new();
    let mut dir_keys: HashSet<String> = HashSet::new();

    for candidate in candidates {
        match candidate.kind {
            DeclaredKind::Symlink => {
                return Err(IntakeError::SymlinkEntry {
                    path: shorten(&candidate.path),
                });
            }
            DeclaredKind::Hardlink => {
                return Err(IntakeError::HardlinkEntry {
                    path: shorten(&candidate.path),
                });
            }
            DeclaredKind::Device => {
                return Err(IntakeError::DeviceEntry {
                    path: shorten(&candidate.path),
                });
            }
            DeclaredKind::File | DeclaredKind::Directory => {}
        }
        let is_dir = candidate.kind == DeclaredKind::Directory;
        let mut path = candidate.path.as_str();
        if is_dir {
            path = path.strip_suffix('/').unwrap_or(path);
        } else if path.ends_with('/') {
            return Err(IntakeError::PathRejected {
                path: shorten(path),
                reason: "a file name ends with a separator".into(),
            });
        }
        if path.len() > limits.max_path_bytes {
            return Err(IntakeError::PathTooLong {
                path: shorten(path),
                limit: limits.max_path_bytes,
            });
        }

        // Split the repository the user brought from the mod content.
        let (in_git, git_inner) = match path.strip_prefix(".git/") {
            Some(inner) => (true, Some(inner)),
            None if path == ".git" => {
                if is_dir {
                    plan.has_git = true;
                    continue;
                }
                return Err(IntakeError::GitPointerFile);
            }
            None => (false, None),
        };
        let components = match git_inner {
            Some(inner) => {
                plan.has_git = true;
                let parts = validate_relative(inner).map_err(|e| path_error(path, e))?;
                if parts[0] == "hooks" {
                    if !is_dir {
                        plan.dropped_hook_files += 1;
                    }
                    continue;
                }
                let mut all = vec![".git"];
                all.extend(parts);
                all
            }
            None => validate_relative(path).map_err(|e| {
                // A `.git` component below the top level is a nested repository.
                if path.split('/').skip(1).any(|c| c == ".git") {
                    return IntakeError::NestedGit {
                        path: shorten(path),
                    };
                }
                path_error(path, e)
            })?,
        };
        if components.len() > limits.max_path_depth {
            return Err(IntakeError::PathTooDeep {
                path: shorten(path),
                limit: limits.max_path_depth,
            });
        }
        let full: String = components.join("/");

        match exact.insert(full.clone(), is_dir) {
            None => {}
            // The same name as a file and as a directory is a conflict, not a repeat.
            Some(was_dir) if was_dir != is_dir => {
                return Err(IntakeError::PathConflict {
                    path: shorten(&full),
                });
            }
            Some(_) => {
                return Err(IntakeError::DuplicateEntry {
                    path: shorten(&full),
                });
            }
        }
        // Collisions at every ancestor and at the entry itself.
        let mut key = String::new();
        let mut raw = String::new();
        for (index, component) in components.iter().enumerate() {
            if index > 0 {
                key.push('/');
                raw.push('/');
            }
            key.push_str(&fold_key(component));
            raw.push_str(component);
            match prefixes.get(&key) {
                Some(seen) if *seen != raw => {
                    return Err(IntakeError::NameCollision {
                        a: shorten(seen),
                        b: shorten(&raw),
                    });
                }
                Some(_) => {}
                None => {
                    prefixes.insert(key.clone(), raw.clone());
                }
            }
            let last = index + 1 == components.len();
            if !last {
                if file_keys.contains(&key) {
                    return Err(IntakeError::PathConflict {
                        path: shorten(&raw),
                    });
                }
                dir_keys.insert(key.clone());
            }
        }
        if is_dir {
            if file_keys.contains(&key) {
                return Err(IntakeError::PathConflict {
                    path: shorten(&full),
                });
            }
            dir_keys.insert(key);
            plan.dir_count += 1;
            plan.entries.push(PlannedEntry {
                rel: full,
                is_dir: true,
                origin: candidate.origin,
                size: 0,
                mode: None,
                in_git,
            });
            continue;
        }
        if dir_keys.contains(&key) {
            return Err(IntakeError::PathConflict {
                path: shorten(&full),
            });
        }
        file_keys.insert(key);

        if !in_git {
            let name = components.last().copied().unwrap_or("");
            match policy.judge(name) {
                TypeVerdict::Allowed => {}
                TypeVerdict::Executable => {
                    return Err(IntakeError::ExecutableRefused {
                        path: shorten(&full),
                    });
                }
                TypeVerdict::NotAllowed => {
                    return Err(IntakeError::FileTypeNotAllowed {
                        path: shorten(&full),
                    });
                }
            }
        }

        plan.file_count += 1;
        if plan.file_count > limits.max_files {
            return Err(IntakeError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        if candidate.size > limits.max_file_bytes {
            return Err(IntakeError::FileTooLarge {
                path: shorten(&full),
                limit: limits.max_file_bytes,
            });
        }
        if let Some(compressed) = candidate.compressed
            && candidate.size > limits.ratio_floor_bytes
            && candidate.size / compressed.max(1) > limits.max_compression_ratio
        {
            return Err(IntakeError::CompressionRatio {
                path: shorten(&full),
            });
        }
        plan.total_bytes = plan.total_bytes.saturating_add(candidate.size);
        if plan.total_bytes > limits.max_total_bytes {
            return Err(IntakeError::TotalTooLarge {
                limit: limits.max_total_bytes,
            });
        }
        plan.entries.push(PlannedEntry {
            rel: full,
            is_dir: false,
            origin: candidate.origin,
            size: candidate.size,
            mode: candidate.mode,
            in_git,
        });
    }
    if let Some(len) = archive_len
        && plan.total_bytes > limits.ratio_floor_bytes
        && plan.total_bytes / len.max(1) > limits.max_compression_ratio
    {
        return Err(IntakeError::CompressionRatio {
            path: "(whole archive)".into(),
        });
    }
    Ok(plan)
}
