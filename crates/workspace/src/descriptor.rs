//! C-01 `WorkspaceDescriptor` (see `docs/02-architecture/contracts-and-interfaces.md`).
//!
//! The descriptor gives the runtime logical locators and authorized content, never host
//! paths, credentials or raw game files. The browser client receives display names and
//! these locators only.

use std::collections::BTreeMap;

use factory_game_ref::{Compatibility, GameReference, check_compatibility};
use serde::{Deserialize, Serialize};

use crate::error::WorkspaceError;
use crate::git::{Oid, RemoteLink, RestorePoint, TaskWorktree};
use crate::ids::{AccountId, ProjectId, WorkspaceId};
use crate::inventory::{FileState, SnapshotInventory};
use crate::storage::Workspace;

/// Version of the serialized descriptor shape. A consumer rejects versions it does not
/// know.
pub const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Maintainer,
    Contributor,
    Viewer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRole {
    pub account_id: AccountId,
    pub role: Role,
}

/// Owning account, project and member roles at descriptor creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tenant {
    pub account_id: AccountId,
    pub project_id: ProjectId,
    pub member_roles: Vec<MemberRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    ModSource,
    TaskWorktree,
    GameReference,
    Tools,
    Artifacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

/// Host-owned handle for one root. `locator` is logical, never a host path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootHandle {
    pub kind: RootKind,
    pub locator: String,
    pub access: Access,
}

/// A file whose bytes differ from the repository, or that is not in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyFileHash {
    pub path: String,
    pub state: FileState,
    pub sha256: Option<String>,
}

/// Git revision plus the full file inventory and the dirty or untracked file hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSnapshot {
    /// `None` without a repository or without commits.
    pub git_revision: Option<Oid>,
    pub inventory_hash: String,
    pub dirty_files: Vec<DirtyFileHash>,
    pub inventory: SnapshotInventory,
}

/// Branch, worktree, expected HEAD and the staged/unstaged inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitState {
    pub is_repository: bool,
    pub branch: Option<String>,
    pub head: Option<Oid>,
    /// Logical locator of the task worktree this descriptor describes, if any.
    pub worktree_locator: Option<String>,
    /// HEAD the host expects. A different HEAD means something moved the branch.
    pub expected_head: Option<Oid>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
    pub git_index_sha256: Option<String>,
    pub restore_points: Vec<RestorePoint>,
    pub nested_repositories: Vec<String>,
}

/// Server reference build and DLC set, content order, the mod's declared supported
/// version, any compatibility warning and explicit unknowns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameProfile {
    pub reference: Option<GameReference>,
    pub content_order: Vec<String>,
    pub declared_supported_version: Option<String>,
    pub compatibility: Compatibility,
    pub unknowns: Vec<String>,
}

impl GameProfile {
    /// Build a profile and compute the compatibility of the declared version.
    pub fn new(
        reference: Option<GameReference>,
        declared_supported_version: Option<String>,
        content_order: Vec<String>,
    ) -> Self {
        let mut unknowns = Vec::new();
        let compatibility = match &reference {
            Some(reference) => {
                check_compatibility(reference, declared_supported_version.as_deref())
            }
            None => {
                unknowns.push("no server game reference is available".to_owned());
                Compatibility::Unknown {
                    reason: "no server game reference is available".into(),
                }
            }
        };
        if let Compatibility::Unknown { reason } = &compatibility
            && reference.is_some()
        {
            unknowns.push(reason.clone());
        }
        GameProfile {
            reference,
            content_order,
            declared_supported_version,
            compatibility,
            unknowns,
        }
    }

    /// A profile for a workspace whose game reference has not been resolved.
    pub fn unresolved() -> Self {
        GameProfile::new(None, None, Vec::new())
    }
}

/// Setup revision, manifest hash, resolved assets and tool versions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentClosure {
    pub setup_revision: Option<String>,
    pub manifest_hash: Option<String>,
    pub resolved_assets: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Available,
    Unavailable,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub status: CapabilityStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub sha256: String,
}

/// Plan identity, final transaction state and generated-file manifest of a Setup run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupReceiptSummary {
    pub plan_id: String,
    pub transaction_state: String,
    pub generated_files: Vec<GeneratedFile>,
}

/// A precise remaining check with its owner and impact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedCheck {
    pub check: String,
    pub owner: String,
    pub impact: String,
}

/// C-01.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDescriptor {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub tenant: Tenant,
    pub root_handles: Vec<RootHandle>,
    pub source_snapshot: SourceSnapshot,
    pub git_state: GitState,
    pub game_profile: GameProfile,
    /// Optional C-10 link, `None` for an uploaded or new mod.
    pub remote_link: Option<RemoteLink>,
    pub component_closure: ComponentClosure,
    pub capabilities: Vec<Capability>,
    pub setup_receipt: Option<SetupReceiptSummary>,
    pub unresolved_checks: Vec<UnresolvedCheck>,
}

/// What the caller supplies. The descriptor fills in everything it can read from disk.
#[derive(Debug, Clone)]
pub struct DescriptorInputs {
    pub workspace_id: WorkspaceId,
    pub member_roles: Vec<MemberRole>,
    /// Roots beyond the mod source and task worktree, for example the game reference,
    /// tools and artifact roots. Locators only.
    pub extra_roots: Vec<RootHandle>,
    pub game_profile: GameProfile,
    /// Overrides the remote read from the repository's configuration.
    pub remote_link: Option<RemoteLink>,
    pub component_closure: ComponentClosure,
    pub capabilities: Vec<Capability>,
    pub setup_receipt: Option<SetupReceiptSummary>,
    pub unresolved_checks: Vec<UnresolvedCheck>,
    /// Describe this task worktree instead of the primary source tree.
    pub task: Option<TaskWorktree>,
}

impl DescriptorInputs {
    pub fn new(workspace_id: WorkspaceId) -> Self {
        DescriptorInputs {
            workspace_id,
            member_roles: Vec::new(),
            extra_roots: Vec::new(),
            game_profile: GameProfile::unresolved(),
            remote_link: None,
            component_closure: ComponentClosure::default(),
            capabilities: Vec::new(),
            setup_receipt: None,
            unresolved_checks: Vec::new(),
            task: None,
        }
    }
}

impl Workspace {
    /// Assemble the C-01 descriptor from the files and Git state on disk plus the
    /// caller's inputs. Read-only: it hashes files and runs read-only Git commands.
    pub fn describe(
        &self,
        inputs: DescriptorInputs,
    ) -> Result<WorkspaceDescriptor, WorkspaceError> {
        let (root_dir, repo, mut root_handles) = match &inputs.task {
            Some(task) => (
                task.work_dir().to_path_buf(),
                Some(task.repo().clone()),
                vec![
                    RootHandle {
                        kind: RootKind::ModSource,
                        locator: self.source_locator(),
                        access: Access::ReadOnly,
                    },
                    RootHandle {
                        kind: RootKind::TaskWorktree,
                        locator: self.task_locator(task.task()),
                        access: Access::ReadWrite,
                    },
                ],
            ),
            None => (
                self.source_dir(),
                self.git()?,
                vec![RootHandle {
                    kind: RootKind::ModSource,
                    locator: self.source_locator(),
                    access: Access::ReadWrite,
                }],
            ),
        };
        root_handles.extend(inputs.extra_roots.iter().cloned());

        let inventory = SnapshotInventory::capture(&root_dir, repo.as_ref())?;
        let dirty_files = inventory
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e.state,
                    FileState::TrackedModified | FileState::Staged | FileState::Untracked
                )
            })
            .map(|e| DirtyFileHash {
                path: e.path.clone(),
                state: e.state,
                sha256: e.sha256.clone(),
            })
            .collect();

        let (git_state, git_revision, remote_from_repo) = match &repo {
            Some(repo) => {
                let baseline = repo.baseline()?;
                let dirty = repo.dirty_state()?;
                let restore_points = match &inputs.task {
                    Some(task) => task.restore_points()?,
                    None => Vec::new(),
                };
                let remote = repo.remote_link()?;
                (
                    GitState {
                        is_repository: true,
                        branch: baseline.branch,
                        head: baseline.head.clone(),
                        worktree_locator: inputs.task.as_ref().map(|t| self.task_locator(t.task())),
                        expected_head: inputs.task.as_ref().map(|t| t.expected_head().clone()),
                        staged: dirty.staged,
                        unstaged: dirty.unstaged,
                        untracked: dirty.untracked,
                        git_index_sha256: baseline.index_sha256,
                        restore_points,
                        nested_repositories: inventory.nested_repositories.clone(),
                    },
                    baseline.head,
                    remote,
                )
            }
            None => (
                GitState {
                    is_repository: false,
                    branch: None,
                    head: None,
                    worktree_locator: None,
                    expected_head: None,
                    staged: Vec::new(),
                    unstaged: Vec::new(),
                    untracked: Vec::new(),
                    git_index_sha256: None,
                    restore_points: Vec::new(),
                    nested_repositories: inventory.nested_repositories.clone(),
                },
                None,
                None,
            ),
        };

        Ok(WorkspaceDescriptor {
            schema_version: DESCRIPTOR_SCHEMA_VERSION,
            workspace_id: inputs.workspace_id,
            tenant: Tenant {
                account_id: self.account().clone(),
                project_id: self.project().clone(),
                member_roles: inputs.member_roles,
            },
            root_handles,
            source_snapshot: SourceSnapshot {
                git_revision,
                inventory_hash: inventory.content_hash(),
                dirty_files,
                inventory,
            },
            git_state,
            game_profile: inputs.game_profile,
            remote_link: inputs.remote_link.or(remote_from_repo),
            component_closure: inputs.component_closure,
            capabilities: inputs.capabilities,
            setup_receipt: inputs.setup_receipt,
            unresolved_checks: inputs.unresolved_checks,
        })
    }
}
