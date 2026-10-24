//! Git handling after an upload is extracted, and `.gitattributes` inspection.

use std::fs;
use std::io;
use std::path::Path;

use factory_workspace::{
    GitError, GitRepo, Identity, InitOptions, SnapshotInventory, valid_branch_name,
};

use crate::error::IntakeError;
use crate::extract::FileRecord;
use crate::model::{Finding, GitInitRequest, GitOutcome, InventorySummary};

/// Attributes written to `.git/info/attributes` of a repository that intake creates.
///
/// `info/attributes` has the highest precedence, so these unset the conversion attributes
/// for every path whatever the uploaded `.gitattributes` says. Without it, a line such as
/// `* text=auto` would make `git add` rewrite CRLF to LF even with `core.autocrlf=false`,
/// and the baseline commit would no longer hold the uploaded bytes.
pub(crate) const NEUTRAL_ATTRIBUTES: &str =
    "* -text -eol -crlf -filter -ident -working-tree-encoding\n";

const MAX_ATTRIBUTES_FILE: u64 = 1024 * 1024;

/// Attribute tokens that make Git convert content.
fn conversion_attribute(token: &str) -> bool {
    if token.starts_with('-') || token.starts_with('!') {
        return false;
    }
    let name = token.split('=').next().unwrap_or("");
    matches!(
        name,
        "text" | "eol" | "filter" | "crlf" | "working-tree-encoding" | "ident"
    )
}

/// Report every `.gitattributes` line that would convert content.
pub(crate) fn scan_gitattributes(source: &Path, files: &[FileRecord]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for record in files.iter().filter(|r| !r.in_git) {
        let name = record.path.rsplit('/').next().unwrap_or("");
        if name != ".gitattributes" || record.size > MAX_ATTRIBUTES_FILE {
            continue;
        }
        let Ok(bytes) = fs::read(source.join(&record.path)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Skip the pattern. A quoted pattern may contain spaces.
            let rest = if let Some(stripped) = line.strip_prefix('"') {
                let mut end = None;
                let mut escaped = false;
                for (i, c) in stripped.char_indices() {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        end = Some(i + 1);
                        break;
                    }
                }
                end.map(|i| &stripped[i..]).unwrap_or("")
            } else {
                line.split_once(char::is_whitespace)
                    .map(|(_, rest)| rest)
                    .unwrap_or("")
            };
            for token in rest.split_whitespace() {
                if conversion_attribute(token) {
                    findings.push(Finding::GitattributesConversion {
                        path: record.path.clone(),
                        line: (index + 1) as u32,
                        attribute: crate::error::shorten(token),
                    });
                }
            }
        }
    }
    findings
}

/// Findings that come from file names alone.
pub(crate) fn name_findings(files: &[FileRecord]) -> Vec<Finding> {
    files
        .iter()
        .filter(|r| !r.in_git && r.path.rsplit('/').next() == Some(".gitmodules"))
        .map(|r| Finding::Submodules {
            path: r.path.clone(),
        })
        .collect()
}

pub(crate) fn inventory_summary(
    source: &Path,
    repo: Option<&GitRepo>,
) -> Result<(SnapshotInventory, InventorySummary), IntakeError> {
    let inventory = SnapshotInventory::capture(source, repo)?;
    let summary = InventorySummary {
        entries: inventory.entries.len() as u64,
        content_hash: inventory.content_hash(),
    };
    Ok((inventory, summary))
}

fn map_open_error(err: GitError) -> IntakeError {
    match err {
        GitError::UnsafeConfig(keys) => IntakeError::GitUnsafeConfig(keys),
        GitError::NotARepository | GitError::GitDirOutsideWorkspace => {
            IntakeError::GitNotRepository
        }
        other => IntakeError::Git(other),
    }
}

/// Decide what happens to Git for an extracted upload.
pub(crate) fn finalize_git(
    source: &Path,
    has_git: bool,
    init: Option<&GitInitRequest>,
    upload_sha256: &str,
    kind_label: &str,
    inventory_findings: &mut Vec<Finding>,
) -> Result<GitOutcome, IntakeError> {
    if has_git {
        // The user brought a repository. It is used only if its configuration cannot run
        // programs on this server. History and the index stay exactly as uploaded.
        let repo =
            GitRepo::open_with_identity(source, Identity::factory()).map_err(map_open_error)?;
        let dirty = repo.dirty_state()?;
        return Ok(GitOutcome::ExistingRepository {
            head: repo.head()?,
            branch: repo.current_branch()?,
            staged: dirty.staged.len() as u64,
            unstaged: dirty.unstaged.len() as u64,
            untracked: dirty.untracked.len() as u64,
        });
    }
    let Some(request) = init else {
        return Ok(GitOutcome::NotInitialised);
    };
    if !request.consent {
        return Err(IntakeError::GitConsentRequired);
    }
    if !valid_branch_name(&request.initial_branch) {
        return Err(IntakeError::InvalidRequest(
            "initial branch name is not valid".into(),
        ));
    }
    let repo = GitRepo::init(
        source,
        InitOptions {
            consent: true,
            initial_branch: request.initial_branch.clone(),
            identity: request.identity.clone(),
        },
    )?;
    let info = source.join(".git").join("info");
    fs::create_dir_all(&info).map_err(IntakeError::io("create git info directory"))?;
    fs::write(info.join("attributes"), NEUTRAL_ATTRIBUTES)
        .map_err(IntakeError::io("write git attributes"))?;
    let message = format!(
        "Import {kind_label} upload\n\nUpload sha256: {upload_sha256}\nThe files are stored exactly as received.\n"
    );
    let head = repo.baseline_commit(&message)?;
    let state = repo.dirty_state()?;
    if !state.staged.is_empty() || !state.unstaged.is_empty() {
        // Git changed bytes while committing. The baseline would not be the upload.
        return Err(IntakeError::Git(GitError::Parse(
            "baseline commit differs from the working tree".into(),
        )));
    }
    let scan = SnapshotInventory::capture(source, Some(&repo))?;
    let ignored = scan
        .entries
        .iter()
        .filter(|e| e.state == factory_workspace::FileState::Ignored)
        .count() as u64;
    if ignored > 0 {
        inventory_findings.push(Finding::IgnoredByGitignore { files: ignored });
    }
    Ok(GitOutcome::Initialised {
        head,
        branch: request.initial_branch.clone(),
        ignored_files: ignored,
    })
}

/// Remove a directory tree, clearing read-only attributes first. Git marks object files
/// read-only on Windows, which makes a plain `remove_dir_all` fail there.
pub(crate) fn force_remove_dir_all(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => {
            clear_readonly(path);
            match fs::remove_dir_all(path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        }
    }
}

fn clear_readonly(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.file_type().is_symlink() {
        return;
    }
    let mut perms = meta.permissions();
    if perms.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
    if meta.is_dir()
        && let Ok(read) = fs::read_dir(path)
    {
        for entry in read.flatten() {
            clear_readonly(&entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_attributes_are_recognised() {
        for token in [
            "text",
            "text=auto",
            "eol=lf",
            "eol=crlf",
            "filter=lfs",
            "crlf",
            "crlf=input",
            "ident",
            "working-tree-encoding=UTF-16",
        ] {
            assert!(conversion_attribute(token), "{token}");
        }
        for token in [
            "-text",
            "!text",
            "-eol",
            "binary",
            "diff=lfs",
            "merge=ours",
            "linguist-vendored",
            "export-ignore",
            "-filter",
        ] {
            assert!(!conversion_attribute(token), "{token}");
        }
    }

    #[test]
    fn neutral_attributes_unset_every_conversion() {
        for name in [
            "text",
            "eol",
            "crlf",
            "filter",
            "ident",
            "working-tree-encoding",
        ] {
            assert!(NEUTRAL_ATTRIBUTES.contains(&format!("-{name}")), "{name}");
        }
    }
}
