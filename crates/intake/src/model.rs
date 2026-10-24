//! Result types of upload intake. All serialize for the API envelope and the database.

use factory_workspace::{Identity, Oid};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A fact about the upload that the user or a later stage should see. None of these blocks
/// the upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Finding {
    /// A `.gitattributes` line that would make Git rewrite line endings or run a filter on
    /// `git add` or checkout. Intake neutralises it in the repository it creates (see
    /// [`crate::UploadOutcome`]), so the baseline commit keeps the uploaded bytes.
    GitattributesConversion {
        path: String,
        line: u32,
        attribute: String,
    },
    /// A file carries an owner-execute bit. The bit is kept, the file is never run.
    ExecutableMode { path: String },
    /// `.git/hooks/` files were not extracted.
    DroppedGitHooks { files: u64 },
    /// A Git LFS pointer file. The large file itself is not part of the upload.
    LfsPointer { path: String },
    /// `.gitignore` rules kept files out of the baseline commit. They are in the working
    /// tree and untracked.
    IgnoredByGitignore { files: u64 },
    /// The upload has a `.gitmodules` file. Submodules are recorded and never fetched.
    Submodules { path: String },
    /// An imported repository tracks a symbolic link. It is kept and never followed.
    SymlinkInRepository { path: String },
    /// An imported repository tracks a file with an executable or script extension. It is
    /// kept as history and never run.
    ExecutableFile { path: String },
}

/// Kind of upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadKind {
    Zip,
    Folder,
}

impl UploadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            UploadKind::Zip => "zip",
            UploadKind::Folder => "folder",
        }
    }
}

/// Explicit consent to create a Git repository in the uploaded mod.
#[derive(Debug, Clone)]
pub struct GitInitRequest {
    /// Must be true. The caller sets it only after the user agreed.
    pub consent: bool,
    pub initial_branch: String,
    /// Committer of the baseline commit.
    pub identity: Identity,
}

impl GitInitRequest {
    /// Consent granted, factory committer identity.
    pub fn with_consent(initial_branch: &str) -> Self {
        GitInitRequest {
            consent: true,
            initial_branch: initial_branch.to_owned(),
            identity: Identity::factory(),
        }
    }
}

/// Per-upload choices.
#[derive(Debug, Clone, Default)]
pub struct UploadOptions {
    /// `None` leaves the workspace without Git. `Some` with `consent: false` is refused
    /// before anything is written.
    pub init_git: Option<GitInitRequest>,
}

/// Identity and display name of one upload.
#[derive(Debug, Clone)]
pub struct UploadMeta {
    pub upload_id: Uuid,
    /// The file or folder name the client sent. Display only: it never becomes a path.
    pub original_name: Option<String>,
}

/// What happened to Git for this upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GitOutcome {
    /// No repository. Either the caller did not ask, or the user did not consent.
    NotInitialised,
    /// A repository was created with consent and one baseline commit holds the upload.
    Initialised {
        head: Oid,
        branch: String,
        /// Files that `.gitignore` rules kept out of the baseline commit.
        ignored_files: u64,
    },
    /// The upload brought its own repository and it passed the configuration checks.
    /// History is untouched and no commit was made.
    ExistingRepository {
        head: Option<Oid>,
        branch: Option<String>,
        staged: u64,
        unstaged: u64,
        untracked: u64,
    },
}

/// The upload as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOriginal {
    /// Logical locator under the uploads area. Never a host path.
    pub locator: String,
    /// SHA-256 of the archive bytes. For a folder, SHA-256 of the sorted manifest
    /// `"<sha256> <size> <path>\n"` of the files as received.
    pub sha256: String,
    /// Bytes as received.
    pub bytes: u64,
}

/// Counts of what was extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionSummary {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
}

/// Compact inventory facts. The full inventory is a call to `SnapshotInventory::capture`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySummary {
    pub entries: u64,
    pub content_hash: String,
}

/// Result of an accepted upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadOutcome {
    pub upload_id: Uuid,
    pub kind: UploadKind,
    /// Logical locator of the mod source root (`workspace:<project>/source`).
    pub source_locator: String,
    pub original: StoredOriginal,
    pub extracted: ExtractionSummary,
    pub git: GitOutcome,
    pub findings: Vec<Finding>,
    pub inventory: InventorySummary,
}

/// A refused upload, with whatever was learned about the bytes before the refusal.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct UploadFailure {
    #[source]
    pub error: crate::IntakeError,
    pub received_sha256: Option<String>,
    pub received_bytes: Option<u64>,
}

impl UploadFailure {
    pub(crate) fn plain(error: crate::IntakeError) -> Self {
        UploadFailure {
            error,
            received_sha256: None,
            received_bytes: None,
        }
    }

    pub fn code(&self) -> &'static str {
        self.error.code()
    }
}
