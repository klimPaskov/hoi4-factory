use std::io;

use crate::paths::PathError;

/// Errors from workspace storage, snapshots and Git operations.
///
/// Display text never contains a host filesystem path or Git stderr, because errors can
/// reach the browser. Use the fields for logs.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("invalid {kind}: {reason}")]
    InvalidId { kind: &'static str, reason: String },
    #[error("path rejected: {0}")]
    Path(#[from] PathError),
    #[error("io error during {operation}: {source}")]
    Io {
        operation: &'static str,
        source: io::Error,
    },
    #[error("git: {0}")]
    Git(#[from] GitError),
    #[error("workspace not found")]
    NotFound,
    #[error("workspace already exists")]
    AlreadyExists,
    #[error("storage layout problem: {0}")]
    Layout(String),
}

impl WorkspaceError {
    pub(crate) fn io(operation: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| WorkspaceError::Io { operation, source }
    }

    /// Stable machine-readable code for the API error envelope.
    pub fn code(&self) -> &'static str {
        match self {
            WorkspaceError::InvalidId { .. } => "invalid_id",
            WorkspaceError::Path(_) => "path_rejected",
            WorkspaceError::Io { .. } => "io_error",
            WorkspaceError::Git(err) => err.code(),
            WorkspaceError::NotFound => "workspace_not_found",
            WorkspaceError::AlreadyExists => "workspace_exists",
            WorkspaceError::Layout(_) => "storage_layout",
        }
    }
}

/// Errors from the `git` CLI wrapper.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("the git executable could not be started: {0}")]
    Spawn(io::Error),
    /// A git command exited non-zero. `stderr` is for logs only.
    #[error("git {subcommand} failed with exit code {code:?}")]
    Failed {
        subcommand: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("io error reading git metadata: {0}")]
    Io(io::Error),
    #[error("git output could not be parsed: {0}")]
    Parse(String),
    #[error("initializing a repository needs explicit consent")]
    ConsentRequired,
    #[error("the directory is already a repository")]
    AlreadyRepository,
    #[error("the directory is not a repository owned by this workspace")]
    NotARepository,
    #[error("the repository's git directory lies outside the workspace")]
    GitDirOutsideWorkspace,
    /// The repository configuration enables automation that must not run on a server.
    #[error("repository configuration enables automation: {}", .0.join(", "))]
    UnsafeConfig(Vec<String>),
    #[error("HEAD moved: expected {expected}, found {actual}")]
    HeadMoved { expected: String, actual: String },
    #[error("the repository has no commits yet")]
    UnbornHead,
    #[error("the repository already has commits")]
    HeadAlreadyExists,
    #[error("reference {0} is not owned by HOI4 Factory")]
    NotOwned(String),
    #[error("branch for this task already exists")]
    BranchExists,
    #[error("branch for this task does not exist")]
    BranchMissing,
    #[error("the task worktree is on branch {actual:?}, expected {expected:?}")]
    WrongBranch {
        expected: String,
        actual: Option<String>,
    },
    #[error("worktree for this task is not registered at that location")]
    WorktreeMissing,
    #[error("worktree location already exists")]
    WorktreeExists,
    #[error("restore point already exists")]
    RestorePointExists,
    #[error("the index of the task worktree already has staged changes")]
    UnexpectedStagedChanges,
    #[error("nothing to commit for the requested paths")]
    NothingToCommit,
    #[error("invalid object id")]
    InvalidOid,
    #[error("invalid commit identity")]
    InvalidIdentity,
    #[error("invalid remote: {0}")]
    InvalidRemote(String),
    #[error("a different remote is already configured")]
    RemoteConflict,
}

impl GitError {
    pub fn code(&self) -> &'static str {
        match self {
            GitError::Spawn(_) => "git_unavailable",
            GitError::Failed { .. } => "git_failed",
            GitError::Io(_) => "git_io",
            GitError::Parse(_) => "git_parse",
            GitError::ConsentRequired => "git_consent_required",
            GitError::AlreadyRepository => "git_already_repository",
            GitError::NotARepository => "git_not_repository",
            GitError::GitDirOutsideWorkspace => "git_dir_outside_workspace",
            GitError::UnsafeConfig(_) => "git_unsafe_config",
            GitError::HeadMoved { .. } => "head_moved",
            GitError::UnbornHead => "git_unborn_head",
            GitError::HeadAlreadyExists => "git_head_exists",
            GitError::NotOwned(_) => "git_not_owned",
            GitError::BranchExists => "git_branch_exists",
            GitError::BranchMissing => "git_branch_missing",
            GitError::WrongBranch { .. } => "git_wrong_branch",
            GitError::WorktreeMissing => "git_worktree_missing",
            GitError::WorktreeExists => "git_worktree_exists",
            GitError::RestorePointExists => "git_restore_point_exists",
            GitError::UnexpectedStagedChanges => "git_unexpected_staged",
            GitError::NothingToCommit => "git_nothing_to_commit",
            GitError::InvalidOid => "git_invalid_oid",
            GitError::InvalidIdentity => "git_invalid_identity",
            GitError::InvalidRemote(_) => "git_invalid_remote",
            GitError::RemoteConflict => "git_remote_conflict",
        }
    }
}
