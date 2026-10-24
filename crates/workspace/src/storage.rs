//! Server workspace storage layout.
//!
//! ```text
//! <data_root>/                                  0755
//!   workspaces/                                 0755
//!     <account_id>/                             0750
//!       <project_id>/                           0750
//!         source/                               0750   working tree of the mod (the Git repository)
//!         tasks/<task_id>/                      0750   linked worktree of one task
//! ```
//!
//! Directories are reached only through typed ids ([`AccountId`], [`ProjectId`],
//! [`TaskId`]). A client string never becomes a directory name. File paths inside a
//! workspace go through [`crate::paths::resolve`].
//!
//! Ancestors stay traversable and readable: the Setup library opens every ancestor of its
//! root for reading, and an ancestor at 0711 fails with `EACCES` (S01-L22). Modes apply on
//! Unix only. On Windows the functions record the intended modes as documented no-ops and
//! rely on the directory ACLs of the data root.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::WorkspaceError;
use crate::git::GitRepo;
use crate::ids::{AccountId, ProjectId, TaskId};
use crate::paths::{self, ResolveMode};

/// Mode of `data_root` and `workspaces/`.
pub const ROOT_DIR_MODE: u32 = 0o755;
/// Mode of account, project, `source/` and `tasks/` directories.
pub const WORKSPACE_DIR_MODE: u32 = 0o750;

const WORKSPACES_DIR: &str = "workspaces";
const SOURCE_DIR: &str = "source";
const TASKS_DIR: &str = "tasks";

/// One deviation of the on-disk layout from the required layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutIssue {
    /// Logical name of the directory, never a host path.
    pub directory: String,
    pub problem: String,
}

/// Root of all server workspaces.
#[derive(Debug, Clone)]
pub struct StorageRoot {
    data_root: PathBuf,
}

impl StorageRoot {
    /// Use `data_root` as the storage root. Nothing is created yet.
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
        }
    }

    /// Host path of the data root. For server-side use and logs only.
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    fn workspaces_dir(&self) -> PathBuf {
        self.data_root.join(WORKSPACES_DIR)
    }

    fn project_dir(&self, account: &AccountId, project: &ProjectId) -> PathBuf {
        self.workspaces_dir()
            .join(account.as_str())
            .join(project.as_str())
    }

    /// Create `data_root` and `workspaces/` when missing, with mode 0755 on Unix.
    /// Existing directories are left as they are. Use [`StorageRoot::verify_layout`] to
    /// inspect them.
    pub fn ensure_layout(&self) -> Result<(), WorkspaceError> {
        create_dir_with_mode(&self.data_root, ROOT_DIR_MODE, false)?;
        create_dir_with_mode(&self.workspaces_dir(), ROOT_DIR_MODE, false)?;
        Ok(())
    }

    /// Create the directories of a new project workspace. Fails with
    /// [`WorkspaceError::AlreadyExists`] when the project directory exists. The account
    /// directory is created on first use and its mode is repaired when it is too narrow.
    pub fn create_workspace(
        &self,
        account: &AccountId,
        project: &ProjectId,
    ) -> Result<Workspace, WorkspaceError> {
        self.ensure_layout()?;
        let account_dir = self.workspaces_dir().join(account.as_str());
        create_dir_with_mode(&account_dir, WORKSPACE_DIR_MODE, true)?;
        let project_dir = self.project_dir(account, project);
        match fs::symlink_metadata(&project_dir) {
            Ok(_) => return Err(WorkspaceError::AlreadyExists),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(WorkspaceError::Io {
                    operation: "inspect project directory",
                    source: err,
                });
            }
        }
        create_dir_with_mode(&project_dir, WORKSPACE_DIR_MODE, true)?;
        create_dir_with_mode(&project_dir.join(SOURCE_DIR), WORKSPACE_DIR_MODE, true)?;
        create_dir_with_mode(&project_dir.join(TASKS_DIR), WORKSPACE_DIR_MODE, true)?;
        Ok(Workspace {
            account: account.clone(),
            project: project.clone(),
            dir: project_dir,
        })
    }

    /// Open an existing workspace. Refuses account or project directories that are
    /// symbolic links or reparse points.
    pub fn open_workspace(
        &self,
        account: &AccountId,
        project: &ProjectId,
    ) -> Result<Workspace, WorkspaceError> {
        let account_dir = self.workspaces_dir().join(account.as_str());
        let project_dir = self.project_dir(account, project);
        for dir in [&account_dir, &project_dir, &project_dir.join(SOURCE_DIR)] {
            match fs::symlink_metadata(dir) {
                Ok(meta) if paths::is_link_like(&meta) => {
                    return Err(WorkspaceError::Layout(
                        "workspace directory is a link".into(),
                    ));
                }
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => {
                    return Err(WorkspaceError::Layout(
                        "workspace path is not a directory".into(),
                    ));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    return Err(WorkspaceError::NotFound);
                }
                Err(err) => {
                    return Err(WorkspaceError::Io {
                        operation: "open workspace",
                        source: err,
                    });
                }
            }
        }
        Ok(Workspace {
            account: account.clone(),
            project: project.clone(),
            dir: project_dir,
        })
    }

    /// Inspect the modes of the data root, `workspaces/` and the directories of one
    /// workspace. Returns the deviations. An empty list means the layout is as required.
    /// Always empty on non-Unix platforms.
    pub fn verify_layout(&self, workspace: Option<&Workspace>) -> Vec<LayoutIssue> {
        let mut issues = Vec::new();
        let mut check = |name: &str, path: &Path, required: u32| {
            if let Some(problem) = mode_problem(path, required) {
                issues.push(LayoutIssue {
                    directory: name.to_owned(),
                    problem,
                });
            }
        };
        check("data_root", &self.data_root, ROOT_DIR_MODE);
        check("workspaces", &self.workspaces_dir(), ROOT_DIR_MODE);
        if let Some(workspace) = workspace {
            let account_dir = workspace
                .dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            check("account", &account_dir, WORKSPACE_DIR_MODE);
            check("project", &workspace.dir, WORKSPACE_DIR_MODE);
            check("source", &workspace.source_dir(), WORKSPACE_DIR_MODE);
            check("tasks", &workspace.tasks_dir(), WORKSPACE_DIR_MODE);
        }
        issues
    }
}

/// Mode problem of an existing directory: it must grant at least the `required` bits and
/// must not be world-writable.
#[cfg(unix)]
fn mode_problem(path: &Path, required: u32) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(meta) => {
            let mode = meta.permissions().mode() & 0o7777;
            if mode & required != required {
                Some(format!("mode {mode:04o} grants less than {required:04o}"))
            } else if mode & 0o002 != 0 {
                Some(format!("mode {mode:04o} is world-writable"))
            } else {
                None
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Some("missing".into()),
        Err(err) => Some(format!("cannot inspect: {:?}", err.kind())),
    }
}

#[cfg(not(unix))]
fn mode_problem(_: &Path, _: u32) -> Option<String> {
    None
}

/// Create one directory (not its parents) with `mode`, ignoring the umask. An existing
/// directory is accepted when it is a real directory. When `repair` is set an existing
/// directory whose mode grants less than `mode` is widened to `mode`.
fn create_dir_with_mode(path: &Path, mode: u32, repair: bool) -> Result<(), WorkspaceError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if paths::is_link_like(&meta) || !meta.is_dir() {
                return Err(WorkspaceError::Layout("expected a real directory".into()));
            }
            if repair {
                apply_mode_if_narrow(path, mode)?;
            }
            return Ok(());
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(WorkspaceError::Io {
                operation: "inspect directory",
                source: err,
            });
        }
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(WorkspaceError::io("create parent directories"))?;
    }
    make_dir(path, mode).map_err(WorkspaceError::io("create directory"))?;
    apply_mode_exact(path, mode)
}

#[cfg(unix)]
fn make_dir(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(mode).create(path)
}

#[cfg(not(unix))]
fn make_dir(path: &Path, _mode: u32) -> io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn apply_mode_exact(path: &Path, mode: u32) -> Result<(), WorkspaceError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(WorkspaceError::io("set directory mode"))
}

#[cfg(not(unix))]
fn apply_mode_exact(_: &Path, _: u32) -> Result<(), WorkspaceError> {
    Ok(())
}

#[cfg(unix)]
fn apply_mode_if_narrow(path: &Path, mode: u32) -> Result<(), WorkspaceError> {
    use std::os::unix::fs::PermissionsExt;
    let current = fs::metadata(path)
        .map_err(WorkspaceError::io("inspect directory mode"))?
        .permissions()
        .mode()
        & 0o7777;
    if current & mode != mode {
        fs::set_permissions(path, fs::Permissions::from_mode(current | mode))
            .map_err(WorkspaceError::io("repair directory mode"))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn apply_mode_if_narrow(_: &Path, _: u32) -> Result<(), WorkspaceError> {
    Ok(())
}

/// One project's server workspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    account: AccountId,
    project: ProjectId,
    dir: PathBuf,
}

impl Workspace {
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    pub fn project(&self) -> &ProjectId {
        &self.project
    }

    /// Logical locator of the workspace for C-01 `root_handles`. It names the project,
    /// not a host path.
    pub fn locator(&self) -> String {
        format!("workspace:{}", self.project)
    }

    /// Logical locator of the mod source root.
    pub fn source_locator(&self) -> String {
        format!("workspace:{}/source", self.project)
    }

    /// Logical locator of one task worktree.
    pub fn task_locator(&self, task: &TaskId) -> String {
        format!("workspace:{}/tasks/{}", self.project, task)
    }

    /// Host path of the mod working tree. For server-side use only.
    pub fn source_dir(&self) -> PathBuf {
        self.dir.join(SOURCE_DIR)
    }

    /// Host path of the directory holding task worktrees.
    pub fn tasks_dir(&self) -> PathBuf {
        self.dir.join(TASKS_DIR)
    }

    /// Host path of one task worktree.
    pub fn task_dir(&self, task: &TaskId) -> PathBuf {
        self.tasks_dir().join(task.as_str())
    }

    /// Resolve a client-supplied relative path inside the mod source tree.
    pub fn resolve_source(&self, rel: &str, mode: ResolveMode) -> Result<PathBuf, WorkspaceError> {
        Ok(paths::resolve(&self.source_dir(), rel, mode)?)
    }

    /// Resolve a client-supplied relative path inside one task worktree.
    pub fn resolve_in_task(
        &self,
        task: &TaskId,
        rel: &str,
        mode: ResolveMode,
    ) -> Result<PathBuf, WorkspaceError> {
        Ok(paths::resolve(&self.task_dir(task), rel, mode)?)
    }

    /// Open the Git repository of the mod source, if one exists.
    pub fn git(&self) -> Result<Option<GitRepo>, WorkspaceError> {
        if !GitRepo::exists_in(&self.source_dir()) {
            return Ok(None);
        }
        Ok(Some(GitRepo::open(&self.source_dir())?))
    }
}
