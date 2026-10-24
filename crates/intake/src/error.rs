//! Error type with stable codes and contract retry classes.
//!
//! Display text never contains a host filesystem path, a token or Git stderr, because
//! errors can reach the browser. Paths in messages are archive-relative names, shortened.

use std::time::Duration;

use factory_workspace::{GitError, WorkspaceError};

/// Retry classes of the contracts (`contracts-and-interfaces.md`, error envelope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    SafeSameInputs,
    RefreshPreconditions,
    ReconcileFirst,
    RequiresAuthorization,
    Terminal,
}

/// Why intake refused or failed.
#[derive(Debug, thiserror::Error)]
pub enum IntakeError {
    // ---- request level ----
    #[error("initializing Git in an uploaded mod needs explicit consent")]
    GitConsentRequired,
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    // ---- limits ----
    #[error("the upload is larger than the {limit} byte limit")]
    UploadTooLarge { limit: u64 },
    #[error("the upload has more than {limit} entries")]
    TooManyEntries { limit: u64 },
    #[error("the upload has more than {limit} files")]
    TooManyFiles { limit: u64 },
    #[error("{path:?} is larger than the {limit} byte file limit")]
    FileTooLarge { path: String, limit: u64 },
    #[error("the upload expands to more than {limit} bytes")]
    TotalTooLarge { limit: u64 },
    #[error("{path:?} is nested deeper than {limit} levels")]
    PathTooDeep { path: String, limit: usize },
    #[error("{path:?} is longer than {limit} bytes")]
    PathTooLong { path: String, limit: usize },
    #[error("{path:?} expands by a suspicious compression ratio")]
    CompressionRatio { path: String },

    // ---- entry names and kinds ----
    #[error("path {path:?} was rejected: {reason}")]
    PathRejected { path: String, reason: String },
    #[error("{path:?} appears twice")]
    DuplicateEntry { path: String },
    #[error("{a:?} and {b:?} would be the same file on some file systems")]
    NameCollision { a: String, b: String },
    #[error("{path:?} is used both as a file and as a directory")]
    PathConflict { path: String },
    #[error("{path:?} is a symbolic link")]
    SymlinkEntry { path: String },
    #[error("{path:?} is a hard link")]
    HardlinkEntry { path: String },
    #[error("{path:?} is a device, pipe or socket")]
    DeviceEntry { path: String },
    #[error("{path:?} is encrypted")]
    EncryptedEntry { path: String },
    #[error("{path:?} is an executable or script, which uploads may not contain")]
    ExecutableRefused { path: String },
    #[error("{path:?} has a file type that mod uploads may not contain")]
    FileTypeNotAllowed { path: String },
    #[error("{path:?} is a nested Git directory")]
    NestedGit { path: String },
    #[error("the upload has a .git file that points elsewhere")]
    GitPointerFile,

    // ---- archive structure ----
    #[error("the file is not a ZIP archive")]
    NotAZipArchive,
    #[error("the ZIP archive is malformed: {0}")]
    ArchiveMalformed(String),
    #[error("{path:?} uses a compression method that is not supported")]
    UnsupportedCompression { path: String },
    #[error("{path:?} does not match its declared size or checksum")]
    SizeMismatch { path: String },

    // ---- Git inside the upload ----
    #[error("the uploaded .git directory is not a usable repository")]
    GitNotRepository,
    #[error("the uploaded repository configuration enables automation: {}", .0.join(", "))]
    GitUnsafeConfig(Vec<String>),

    // ---- lower layers ----
    #[error("workspace: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("git: {0}")]
    Git(#[from] GitError),
    #[error("io error during {operation}: {source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("the intake task was cancelled or panicked")]
    Aborted,
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),

    // ---- GitHub ----
    #[error("the GitHub connection was revoked or the installation no longer exists")]
    AccessRevoked,
    #[error("the GitHub App installation is not limited to selected repositories")]
    InstallationNotSelected,
    #[error("the GitHub App installation is suspended")]
    InstallationSuspended,
    #[error("the repository is not on this project's allowlist")]
    RepositoryNotAllowed,
    #[error("the repository is not part of the GitHub App installation")]
    RepositoryNotInInstallation,
    #[error("the branch does not exist on the remote")]
    BranchNotFound,
    #[error("the branch name is not supported")]
    BranchNameUnsupported,
    #[error("the repository is larger than the {limit_kib} KiB import limit")]
    RepositoryTooLarge { limit_kib: u64 },
    #[error("GitHub rate limit reached")]
    GithubRateLimited { retry_after: Option<Duration> },
    #[error("GitHub is unavailable: {0}")]
    GithubUnavailable(String),
    #[error("unexpected GitHub response: {0}")]
    GithubProtocol(String),
    #[error("GitHub App credentials are not available: {0}")]
    Credentials(String),
    #[error("the remote clone failed")]
    CloneFailed,
}

impl IntakeError {
    pub(crate) fn io(operation: &'static str) -> impl FnOnce(std::io::Error) -> Self {
        move |source| IntakeError::Io { operation, source }
    }

    /// Stable machine-readable code for the API error envelope.
    pub fn code(&self) -> &'static str {
        match self {
            IntakeError::GitConsentRequired => "git_consent_required",
            IntakeError::InvalidRequest(_) => "invalid_request",
            IntakeError::UploadTooLarge { .. } => "upload_too_large",
            IntakeError::TooManyEntries { .. } => "too_many_entries",
            IntakeError::TooManyFiles { .. } => "too_many_files",
            IntakeError::FileTooLarge { .. } => "file_too_large",
            IntakeError::TotalTooLarge { .. } => "total_too_large",
            IntakeError::PathTooDeep { .. } => "path_too_deep",
            IntakeError::PathTooLong { .. } => "path_too_long",
            IntakeError::CompressionRatio { .. } => "compression_ratio_exceeded",
            IntakeError::PathRejected { .. } => "path_rejected",
            IntakeError::DuplicateEntry { .. } => "duplicate_entry",
            IntakeError::NameCollision { .. } => "name_collision",
            IntakeError::PathConflict { .. } => "path_conflict",
            IntakeError::SymlinkEntry { .. } => "symlink_entry",
            IntakeError::HardlinkEntry { .. } => "hardlink_entry",
            IntakeError::DeviceEntry { .. } => "device_entry",
            IntakeError::EncryptedEntry { .. } => "encrypted_entry",
            IntakeError::ExecutableRefused { .. } => "executable_refused",
            IntakeError::FileTypeNotAllowed { .. } => "file_type_not_allowed",
            IntakeError::NestedGit { .. } => "nested_git_directory",
            IntakeError::GitPointerFile => "git_pointer_file",
            IntakeError::NotAZipArchive => "not_a_zip_archive",
            IntakeError::ArchiveMalformed(_) => "archive_malformed",
            IntakeError::UnsupportedCompression { .. } => "unsupported_compression",
            IntakeError::SizeMismatch { .. } => "size_mismatch",
            IntakeError::GitNotRepository => "git_not_repository",
            IntakeError::GitUnsafeConfig(_) => "git_unsafe_config",
            IntakeError::Workspace(err) => err.code(),
            IntakeError::Git(err) => err.code(),
            IntakeError::Io { .. } => "io_error",
            IntakeError::Aborted => "intake_aborted",
            IntakeError::Database(_) => "database_error",
            IntakeError::AccessRevoked => "access_revoked",
            IntakeError::InstallationNotSelected => "installation_not_selected",
            IntakeError::InstallationSuspended => "installation_suspended",
            IntakeError::RepositoryNotAllowed => "repository_not_allowed",
            IntakeError::RepositoryNotInInstallation => "repository_not_in_installation",
            IntakeError::BranchNotFound => "branch_not_found",
            IntakeError::BranchNameUnsupported => "branch_name_unsupported",
            IntakeError::RepositoryTooLarge { .. } => "repository_too_large",
            IntakeError::GithubRateLimited { .. } => "github_rate_limited",
            IntakeError::GithubUnavailable(_) => "github_unavailable",
            IntakeError::GithubProtocol(_) => "github_protocol",
            IntakeError::Credentials(_) => "github_credentials_unavailable",
            IntakeError::CloneFailed => "clone_failed",
        }
    }

    /// Contract retry class. A rejected upload cannot succeed with the same bytes, so most
    /// content errors are terminal. Transport trouble is retryable only because intake
    /// discards a partial workspace and repeats from the recorded ref (SPEC-005).
    pub fn retry_class(&self) -> RetryClass {
        match self {
            IntakeError::GitConsentRequired | IntakeError::AccessRevoked => {
                RetryClass::RequiresAuthorization
            }
            IntakeError::InstallationNotSelected
            | IntakeError::InstallationSuspended
            | IntakeError::RepositoryNotAllowed
            | IntakeError::RepositoryNotInInstallation => RetryClass::RequiresAuthorization,
            IntakeError::BranchNotFound | IntakeError::RepositoryTooLarge { .. } => {
                RetryClass::RefreshPreconditions
            }
            IntakeError::GithubRateLimited { .. }
            | IntakeError::GithubUnavailable(_)
            | IntakeError::CloneFailed => RetryClass::SafeSameInputs,
            IntakeError::Workspace(WorkspaceError::AlreadyExists) => {
                RetryClass::RefreshPreconditions
            }
            IntakeError::Io { .. } | IntakeError::Aborted | IntakeError::Database(_) => {
                RetryClass::ReconcileFirst
            }
            _ => RetryClass::Terminal,
        }
    }
}

/// Shorten a client-supplied name for an error message.
pub(crate) fn shorten(text: &str) -> String {
    const MAX: usize = 160;
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .take(MAX)
        .collect();
    if text.chars().count() > MAX {
        out.push('…');
    }
    out
}
