//! Upload intake: a ZIP archive or a folder becomes a fresh server workspace.
//!
//! Order of work, so that a hostile upload is refused as early as possible:
//!
//! 1. Check the request (explicit consent before anything is written).
//! 2. Receive the bytes. A ZIP is stored as received under the **uploads area**
//!    (`<data_root>/uploads/<account>/<project>/`), outside the workspace, under a
//!    generated name with a neutral extension and mode `0640`. It is never served and
//!    never executed. SHA-256 and size are recorded.
//! 3. Plan: read the central directory (bounded) or the folder entry list and apply every
//!    name, kind, type and limit rule **without writing a byte** to a workspace.
//! 4. Create the workspace and stream the entries into `source/`, counting every byte.
//! 5. Decide Git: an uploaded `.git` is used only if its configuration passes
//!    `factory-workspace`'s checks. Otherwise a repository is created only with explicit
//!    consent.
//!
//! A failure after step 4 removes the workspace this call created. A pre-existing
//! workspace is never touched: creating over one fails with `workspace_exists`.
//!
//! The functions here block. Call them from `spawn_blocking`.

use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use factory_workspace::{AccountId, ProjectId, StorageRoot, Workspace, valid_branch_name};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::{FileTypePolicy, IntakeLimits};
use crate::error::{IntakeError, shorten};
use crate::extract::{EntrySource, ExtractReport, FileRecord, ModePolicy, extract};
use crate::gitfinal::{
    finalize_git, force_remove_dir_all, inventory_summary, name_findings, scan_gitattributes,
};
use crate::model::{
    ExtractionSummary, Finding, StoredOriginal, UploadFailure, UploadKind, UploadMeta,
    UploadOptions, UploadOutcome,
};
use crate::plan::{Candidate, Plan, plan};
use crate::util::{Copied, copy_hashed, hex};
use crate::zip::{self, DeclaredKind, ZipIndex};

const UPLOADS_DIR: &str = "uploads";

/// Where the upload goes: the owning account and the new project.
#[derive(Debug, Clone, Copy)]
pub struct UploadTarget<'a> {
    pub account: &'a AccountId,
    pub project: &'a ProjectId,
}

/// Bytes of one folder-upload file.
#[derive(Debug)]
pub enum EntryData {
    /// Bytes held in memory.
    Bytes(Vec<u8>),
    /// A file the server spooled from the request. It is checked with `lstat`: a link, a
    /// device or a file with more than one name is refused.
    Path(PathBuf),
}

/// One entry of a folder upload: a client-relative path and its bytes.
#[derive(Debug)]
pub struct FolderEntry {
    /// Relative path with `/` separators, exactly as the client sent it.
    pub path: String,
    pub data: EntryData,
    /// Unix permission bits, when the client knows them.
    pub mode: Option<u32>,
    /// What the client says this entry is. Browsers send only files. An adapter that walks
    /// a directory can mark links and devices so they are refused by name.
    pub kind: DeclaredKind,
}

impl FolderEntry {
    pub fn file(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        FolderEntry {
            path: path.into(),
            data: EntryData::Bytes(bytes.into()),
            mode: None,
            kind: DeclaredKind::File,
        }
    }
}

/// Upload intake for one storage root.
#[derive(Debug, Clone)]
pub struct UploadIntake {
    storage: StorageRoot,
    limits: IntakeLimits,
    policy: FileTypePolicy,
}

/// Removes a project directory the call created unless disarmed.
struct WorkspaceGuard {
    project_dir: Option<PathBuf>,
}

impl WorkspaceGuard {
    fn disarm(mut self) {
        self.project_dir = None;
    }
}

impl Drop for WorkspaceGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.project_dir.take()
            && let Err(err) = force_remove_dir_all(&dir)
        {
            tracing::error!(error = %err, "could not remove the workspace of a refused upload");
        }
    }
}

/// Removes a path (file or directory) unless disarmed.
struct PathGuard {
    path: Option<PathBuf>,
}

impl PathGuard {
    fn new(path: PathBuf) -> Self {
        PathGuard { path: Some(path) }
    }
    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = force_remove_dir_all(&path);
            let _ = fs::remove_file(&path);
        }
    }
}

fn ensure_private_dir(path: &Path) -> Result<(), IntakeError> {
    let mut missing: Vec<PathBuf> = Vec::new();
    let mut current = path;
    loop {
        match fs::symlink_metadata(current) {
            Ok(meta) => {
                if factory_workspace::paths::is_link_like(&meta) || !meta.is_dir() {
                    return Err(IntakeError::Workspace(
                        factory_workspace::WorkspaceError::Layout(
                            "uploads path is not a directory".into(),
                        ),
                    ));
                }
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
                match current.parent() {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            Err(err) => {
                return Err(IntakeError::Io {
                    operation: "inspect uploads directory",
                    source: err,
                });
            }
        }
    }
    for dir in missing.iter().rev() {
        fs::create_dir(dir).map_err(IntakeError::io("create uploads directory"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o750))
                .map_err(IntakeError::io("set uploads directory mode"))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o640)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

struct ZipSource {
    file: File,
    index: ZipIndex,
}

impl EntrySource for ZipSource {
    fn open(&mut self, origin: usize) -> Result<Box<dyn Read + '_>, IntakeError> {
        let entry = self
            .index
            .entries
            .get(origin)
            .ok_or_else(|| IntakeError::ArchiveMalformed("entry index out of range".into()))?
            .clone();
        zip::open_entry(&mut self.file, &self.index, &entry)
    }

    fn crc32(&self, origin: usize) -> Option<u32> {
        self.index.entries.get(origin).map(|e| e.crc32)
    }
}

struct FolderSource<'a> {
    entries: &'a [FolderEntry],
}

impl EntrySource for FolderSource<'_> {
    fn open(&mut self, origin: usize) -> Result<Box<dyn Read + '_>, IntakeError> {
        let entry = self
            .entries
            .get(origin)
            .ok_or_else(|| IntakeError::InvalidRequest("entry index out of range".into()))?;
        match &entry.data {
            EntryData::Bytes(bytes) => Ok(Box::new(Cursor::new(bytes.as_slice()))),
            EntryData::Path(path) => {
                check_spooled_file(path, &entry.path)?;
                let file = File::open(path).map_err(IntakeError::io("open spooled file"))?;
                Ok(Box::new(file))
            }
        }
    }

    fn crc32(&self, _: usize) -> Option<u32> {
        None
    }
}

/// `lstat` a spooled file and refuse anything that is not a plain single-name file.
fn check_spooled_file(path: &Path, label: &str) -> Result<u64, IntakeError> {
    let meta = fs::symlink_metadata(path).map_err(IntakeError::io("inspect spooled file"))?;
    if factory_workspace::paths::is_link_like(&meta) {
        return Err(IntakeError::SymlinkEntry {
            path: shorten(label),
        });
    }
    if !meta.is_file() {
        return Err(IntakeError::DeviceEntry {
            path: shorten(label),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return Err(IntakeError::HardlinkEntry {
                path: shorten(label),
            });
        }
    }
    Ok(meta.len())
}

impl UploadIntake {
    pub fn new(storage: StorageRoot, limits: IntakeLimits, policy: FileTypePolicy) -> Self {
        UploadIntake {
            storage,
            limits,
            policy,
        }
    }

    pub fn storage(&self) -> &StorageRoot {
        &self.storage
    }

    pub fn limits(&self) -> &IntakeLimits {
        &self.limits
    }

    fn uploads_dir(&self, target: &UploadTarget<'_>) -> PathBuf {
        self.storage
            .data_root()
            .join(UPLOADS_DIR)
            .join(target.account.as_str())
            .join(target.project.as_str())
    }

    /// Host path of the stored original of a ZIP upload. For server-side maintenance only.
    /// The path is never given to a client and nothing serves it.
    pub fn original_path(&self, target: &UploadTarget<'_>, upload_id: Uuid) -> PathBuf {
        self.uploads_dir(target)
            .join(format!("{upload_id}.zip.upload"))
    }

    /// Remove the stored original of an upload (a ZIP file or a folder copy).
    pub fn discard_original(
        &self,
        target: &UploadTarget<'_>,
        upload_id: Uuid,
    ) -> Result<(), IntakeError> {
        let dir = self.uploads_dir(target);
        force_remove_dir_all(&dir.join(upload_id.to_string()))
            .map_err(IntakeError::io("remove folder original"))?;
        match fs::remove_file(self.original_path(target, upload_id)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(IntakeError::Io {
                operation: "remove archive original",
                source: err,
            }),
        }
    }

    fn precheck(&self, options: &UploadOptions) -> Result<(), IntakeError> {
        if let Some(request) = &options.init_git {
            if !request.consent {
                return Err(IntakeError::GitConsentRequired);
            }
            if !valid_branch_name(&request.initial_branch) {
                return Err(IntakeError::InvalidRequest(
                    "initial branch name is not valid".into(),
                ));
            }
        }
        Ok(())
    }

    /// Accept a ZIP archive. `archive` is read to its end or until the upload limit.
    pub fn ingest_zip(
        &self,
        target: &UploadTarget<'_>,
        meta: &UploadMeta,
        archive: &mut dyn Read,
        options: &UploadOptions,
    ) -> Result<UploadOutcome, UploadFailure> {
        self.precheck(options).map_err(UploadFailure::plain)?;
        let dir = self.uploads_dir(target);
        ensure_private_dir(&dir).map_err(UploadFailure::plain)?;
        let stored = self.original_path(target, meta.upload_id);
        let mut file = create_private_file(&stored).map_err(|err| {
            UploadFailure::plain(if err.kind() == std::io::ErrorKind::AlreadyExists {
                IntakeError::InvalidRequest("upload id was already used".into())
            } else {
                IntakeError::Io {
                    operation: "store upload",
                    source: err,
                }
            })
        })?;
        let mut stored_guard = PathGuard::new(stored.clone());
        let (bytes, sha256) = match copy_hashed(archive, &mut file, self.limits.max_upload_bytes) {
            Ok(Copied::Done { bytes, sha256 }) => (bytes, sha256),
            Ok(Copied::LimitExceeded) => {
                return Err(UploadFailure::plain(IntakeError::UploadTooLarge {
                    limit: self.limits.max_upload_bytes,
                }));
            }
            Err(err) => {
                return Err(UploadFailure::plain(IntakeError::Io {
                    operation: "store upload",
                    source: err,
                }));
            }
        };
        drop(file);
        let fail = |error: IntakeError| UploadFailure {
            error,
            received_sha256: Some(sha256.clone()),
            received_bytes: Some(bytes),
        };

        let mut reader = File::open(&stored).map_err(|e| {
            fail(IntakeError::Io {
                operation: "open stored upload",
                source: e,
            })
        })?;
        let index = zip::read_index(&mut reader, self.limits.max_entries).map_err(&fail)?;
        let candidates: Vec<Candidate> = index
            .entries
            .iter()
            .enumerate()
            .map(|(origin, e)| Candidate {
                path: e.name.clone(),
                kind: e.kind,
                size: e.uncompressed_size,
                compressed: Some(e.compressed_size),
                mode: e.unix_mode,
                origin,
            })
            .collect();
        let plan = plan(&candidates, Some(bytes), &self.limits, &self.policy).map_err(&fail)?;
        let mut source = ZipSource {
            file: reader,
            index,
        };
        let original = StoredOriginal {
            locator: format!("upload:{}/{}", target.project, meta.upload_id),
            sha256: sha256.clone(),
            bytes,
        };
        let outcome = self
            .build_workspace(
                target,
                meta,
                UploadKind::Zip,
                &plan,
                &mut source,
                original,
                options,
            )
            .map_err(&fail)?;
        stored_guard.disarm();
        Ok(outcome)
    }

    /// Accept a folder upload: a list of relative paths and their bytes.
    pub fn ingest_folder(
        &self,
        target: &UploadTarget<'_>,
        meta: &UploadMeta,
        entries: Vec<FolderEntry>,
        options: &UploadOptions,
    ) -> Result<UploadOutcome, UploadFailure> {
        self.precheck(options).map_err(UploadFailure::plain)?;
        let mut candidates = Vec::with_capacity(entries.len());
        let mut received: u64 = 0;
        for (origin, entry) in entries.iter().enumerate() {
            let size = match (&entry.data, entry.kind) {
                (_, DeclaredKind::Directory) => 0,
                (EntryData::Bytes(bytes), _) => bytes.len() as u64,
                (EntryData::Path(path), DeclaredKind::File) => {
                    check_spooled_file(path, &entry.path).map_err(UploadFailure::plain)?
                }
                (EntryData::Path(_), _) => 0,
            };
            received = received.saturating_add(size);
            if received > self.limits.max_upload_bytes {
                return Err(UploadFailure::plain(IntakeError::UploadTooLarge {
                    limit: self.limits.max_upload_bytes,
                }));
            }
            candidates.push(Candidate {
                path: entry.path.clone(),
                kind: entry.kind,
                size,
                compressed: None,
                mode: entry.mode.map(|m| m & 0o7777),
                origin,
            });
        }
        let plan =
            plan(&candidates, None, &self.limits, &self.policy).map_err(UploadFailure::plain)?;
        let mut source = FolderSource { entries: &entries };

        // Keep the folder as received, in the uploads area, before the workspace exists.
        let retained_dir = self.uploads_dir(target).join(meta.upload_id.to_string());
        ensure_private_dir(&self.uploads_dir(target)).map_err(UploadFailure::plain)?;
        fs::create_dir(&retained_dir).map_err(|err| {
            UploadFailure::plain(if err.kind() == std::io::ErrorKind::AlreadyExists {
                IntakeError::InvalidRequest("upload id was already used".into())
            } else {
                IntakeError::Io {
                    operation: "store upload",
                    source: err,
                }
            })
        })?;
        let mut retained_guard = PathGuard::new(retained_dir.clone());
        let retained = extract(
            &plan,
            &mut source,
            &retained_dir,
            &self.limits,
            ModePolicy::Retained,
        )
        .map_err(UploadFailure::plain)?;
        let sha256 = manifest_hash(&retained.files);
        let original = StoredOriginal {
            locator: format!("upload:{}/{}", target.project, meta.upload_id),
            sha256: sha256.clone(),
            bytes: retained.bytes,
        };
        let fail = |error: IntakeError| UploadFailure {
            error,
            received_sha256: Some(sha256.clone()),
            received_bytes: Some(retained.bytes),
        };
        let outcome = self
            .build_workspace(
                target,
                meta,
                UploadKind::Folder,
                &plan,
                &mut source,
                original,
                options,
            )
            .map_err(&fail)?;
        retained_guard.disarm();
        Ok(outcome)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_workspace(
        &self,
        target: &UploadTarget<'_>,
        meta: &UploadMeta,
        kind: UploadKind,
        plan: &Plan,
        source: &mut dyn EntrySource,
        original: StoredOriginal,
        options: &UploadOptions,
    ) -> Result<UploadOutcome, IntakeError> {
        let workspace: Workspace = self
            .storage
            .create_workspace(target.account, target.project)?;
        let project_dir = workspace
            .source_dir()
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| IntakeError::Workspace(factory_workspace::WorkspaceError::NotFound))?;
        // From here the directory is ours. Any failure removes it.
        let guard = WorkspaceGuard {
            project_dir: Some(project_dir),
        };
        let source_dir = workspace.source_dir();
        let report: ExtractReport = extract(
            plan,
            source,
            &source_dir,
            &self.limits,
            ModePolicy::Preserve,
        )?;
        let mut findings: Vec<Finding> = report.findings.clone();
        findings.extend(scan_gitattributes(&source_dir, &report.files));
        findings.extend(name_findings(&report.files));
        if plan.dropped_hook_files > 0 {
            findings.push(Finding::DroppedGitHooks {
                files: plan.dropped_hook_files,
            });
        }
        let label = match kind {
            UploadKind::Zip => "ZIP",
            UploadKind::Folder => "folder",
        };
        let git = finalize_git(
            &source_dir,
            plan.has_git,
            options.init_git.as_ref(),
            &original.sha256,
            label,
            &mut findings,
        )?;
        let repo = workspace.git()?;
        let (_, inventory) = inventory_summary(&source_dir, repo.as_ref())?;
        let outcome = UploadOutcome {
            upload_id: meta.upload_id,
            kind,
            source_locator: workspace.source_locator(),
            original,
            extracted: ExtractionSummary {
                files: report.files.len() as u64,
                directories: plan.dir_count,
                bytes: report.bytes,
            },
            git,
            findings,
            inventory,
        };
        guard.disarm();
        Ok(outcome)
    }
}

/// SHA-256 of the sorted manifest `"<sha256> <size> <path>\n"`.
fn manifest_hash(files: &[FileRecord]) -> String {
    let mut sorted: Vec<&FileRecord> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut hasher = Sha256::new();
    for record in sorted {
        hasher.update(format!("{} {} {}\n", record.sha256, record.size, record.path).as_bytes());
    }
    hex(&hasher.finalize())
}
