//! Server workspace storage, snapshot inventory and Git baseline for HOI4 Factory.
//!
//! * [`storage`]: the `<data_root>/workspaces/<account>/<project>/source` layout, reached
//!   only through typed ids
//! * [`paths`]: safe resolution of client-supplied relative paths
//! * [`inventory`]: per-file size, mode, SHA-256 and Git classification (C-01
//!   `source_snapshot`) and inventory comparison
//! * [`git`]: Git baseline through the `git` CLI, owned branches, task worktrees,
//!   restore points, recorded remote, never a push or a fetch
//! * [`descriptor`]: the C-01 `WorkspaceDescriptor` types
//!
//! The crate opens no network connection and holds no credentials.

pub mod descriptor;
mod error;
pub mod git;
mod ids;
pub mod inventory;
pub mod paths;
pub mod storage;

pub use descriptor::{
    Access, Capability, CapabilityStatus, ComponentClosure, DESCRIPTOR_SCHEMA_VERSION,
    DescriptorInputs, DirtyFileHash, GameProfile, GeneratedFile, GitState, MemberRole, Role,
    RootHandle, RootKind, SetupReceiptSummary, SourceSnapshot, Tenant, UnresolvedCheck,
    WorkspaceDescriptor,
};
pub use error::{GitError, WorkspaceError};
pub use git::{
    Baseline, DirtyState, GitRepo, Identity, InitOptions, OWNED_BRANCH_PREFIX, Oid, OwnedBranch,
    RESTORE_REF_PREFIX, RemoteLink, RestorePoint, TaskWorktree, task_branch_name,
    valid_branch_name,
};
pub use ids::{AccountId, ProjectId, RestoreLabel, TaskId, WorkspaceId};
pub use inventory::{
    EntryKind, FileEntry, FileState, INVENTORY_SCHEMA_VERSION, InventoryDiff, SnapshotInventory,
    sha256_file,
};
pub use paths::{PathError, ResolveMode, fold_key, names_collide, resolve, validate_relative};
pub use storage::{LayoutIssue, ROOT_DIR_MODE, StorageRoot, WORKSPACE_DIR_MODE, Workspace};
