//! Git baseline through the `git` command line (SPEC-005, "Workspace baseline and
//! isolation"). No libgit2.
//!
//! Every invocation is controlled:
//!
//! * `-c core.autocrlf=false -c core.safecrlf=false` always, so a system default of
//!   `core.autocrlf=true` (Git for Windows) cannot rewrite CRLF or BOM bytes (S01-L03)
//! * the system and global configuration files are not read (`GIT_CONFIG_NOSYSTEM`,
//!   `GIT_CONFIG_GLOBAL`). Nothing here ever writes a global or system configuration
//! * hooks are disabled (`core.hooksPath`), `core.fsmonitor` is off and every `GIT_*`
//!   variable of the parent process is dropped. Repository-local configuration that would
//!   run programs (filters, diff drivers, credential helpers, ...) makes [`GitRepo::open`]
//!   fail with [`GitError::UnsafeConfig`] instead of running them (GIT-10)
//! * optional locks are off, so read-only commands never refresh the user's real index
//! * `GIT_CEILING_DIRECTORIES` stops discovery from walking up into a parent repository
//! * nothing here fetches, pulls or pushes. Remote handling only records a URL
//!
//! Known limit: `.gitattributes` files inside an imported repository (`text`, `eol`,
//! `filter`) still apply to `git add` and checkout. The intake stage should reject or
//! report `eol` and `filter` attributes before the first baseline commit.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::{GitError, WorkspaceError};
use crate::ids::{RestoreLabel, TaskId};
use crate::inventory::sha256_file;
use crate::paths::{self, ResolveMode};

/// Branches created by the factory live under this prefix. Operations refuse any other.
pub const OWNED_BRANCH_PREFIX: &str = "hoi4factory/";
/// Restore points live under `refs/hoi4factory/restore/<task>/<label>`.
pub const RESTORE_REF_PREFIX: &str = "refs/hoi4factory/restore/";

const REMOTE_NAME: &str = "origin";
const DEFAULT_BRANCH_KEY: &str = "hoi4factory.defaultbranch";

/// A Git object id in lowercase hexadecimal (SHA-1 or SHA-256 repositories).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Oid(String);

impl Oid {
    pub fn new(value: impl Into<String>) -> Result<Self, GitError> {
        let value = value.into();
        let ok_len = value.len() == 40 || value.len() == 64;
        if !ok_len
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(GitError::InvalidOid);
        }
        Ok(Oid(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Oid {
    type Error = GitError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Oid::new(value)
    }
}

impl From<Oid> for String {
    fn from(oid: Oid) -> String {
        oid.0
    }
}

/// Name and email used as committer (the factory) or author (the acting member).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

impl Identity {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Result<Self, GitError> {
        let (name, email) = (name.into(), email.into());
        let bad = |s: &str| {
            s.trim().is_empty() || s.chars().any(|c| c.is_control() || c == '<' || c == '>')
        };
        if bad(&name) || bad(&email) || name.len() > 200 || email.len() > 200 {
            return Err(GitError::InvalidIdentity);
        }
        Ok(Identity { name, email })
    }

    /// The identity used for commits made by the factory itself.
    pub fn factory() -> Self {
        Identity {
            name: "HOI4 Factory".into(),
            email: "factory@hoi4-factory.invalid".into(),
        }
    }
}

/// Options for [`GitRepo::init`].
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Must be true. Initializing Git in a user's uploaded mod needs explicit consent.
    pub consent: bool,
    pub initial_branch: String,
    /// Committer identity stored in the returned handle.
    pub identity: Identity,
}

impl InitOptions {
    pub fn with_consent(initial_branch: &str) -> Self {
        InitOptions {
            consent: true,
            initial_branch: initial_branch.to_owned(),
            identity: Identity::factory(),
        }
    }
}

/// Repository state recorded at baseline time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// `None` for a repository without commits.
    pub head: Option<Oid>,
    /// `None` for a detached HEAD.
    pub branch: Option<String>,
    /// SHA-256 of `.git/index` as found, `None` when no index file exists.
    pub index_sha256: Option<String>,
}

/// Staged, unstaged and untracked paths, sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyState {
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

/// A linked remote. Recording it never contacts the remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteLink {
    pub name: String,
    pub url: String,
    pub default_branch: Option<String>,
}

/// A branch created by the factory for one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedBranch {
    pub task: TaskId,
    pub name: String,
    pub tip: Oid,
}

/// A recorded restore point: a ref that keeps a commit reachable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePoint {
    pub ref_name: String,
    pub oid: Oid,
}

/// Name of the owned branch for a task.
pub fn task_branch_name(task: &TaskId) -> String {
    format!("{OWNED_BRANCH_PREFIX}{task}")
}

fn restore_ref_name(task: &TaskId, label: &RestoreLabel) -> String {
    format!("{RESTORE_REF_PREFIX}{task}/{label}")
}

/// Conservative branch-name check: ASCII letters, digits and `._-/`, no `..`, no leading
/// or trailing `/`, no `.lock` suffix, no component starting with `.` or `-`.
pub fn valid_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && !name.contains("..")
        && !name.ends_with(".lock")
        && !name.ends_with('.')
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.starts_with('-'))
}

#[derive(Debug, Clone)]
enum RepoKind {
    Primary,
    Linked,
}

/// Handle on a repository working directory. Cheap to clone.
#[derive(Debug, Clone)]
pub struct GitRepo {
    work_dir: PathBuf,
    kind: RepoKind,
    identity: Identity,
}

/// Parsed `git status` and `ls-files` state used by the inventory.
#[derive(Debug, Default)]
pub(crate) struct RepoScan {
    pub tracked: BTreeSet<String>,
    pub gitlinks: BTreeSet<String>,
    /// path -> (index status, worktree status), `.` meaning unchanged
    pub status: BTreeMap<String, (u8, u8)>,
    pub untracked: BTreeSet<String>,
    pub ignored: BTreeSet<String>,
}

impl GitRepo {
    /// True when `dir/.git` exists (file or directory), without following links.
    pub fn exists_in(dir: &Path) -> bool {
        fs::symlink_metadata(dir.join(".git")).is_ok()
    }

    /// Initialize a repository in `dir`. Requires explicit consent and refuses a
    /// directory that already has a `.git`. The new repository stores
    /// `core.autocrlf=false` and `core.safecrlf=false` in its own local configuration.
    pub fn init(dir: &Path, options: InitOptions) -> Result<GitRepo, GitError> {
        if !options.consent {
            return Err(GitError::ConsentRequired);
        }
        if Self::exists_in(dir) {
            return Err(GitError::AlreadyRepository);
        }
        if !valid_branch_name(&options.initial_branch) {
            return Err(GitError::Parse("invalid initial branch name".into()));
        }
        let repo = GitRepo {
            work_dir: dir.to_path_buf(),
            kind: RepoKind::Primary,
            identity: options.identity,
        };
        repo.run([
            "init".to_owned(),
            "--quiet".to_owned(),
            "--template=".to_owned(),
            format!("--initial-branch={}", options.initial_branch),
        ])?;
        repo.run(["config", "--local", "core.autocrlf", "false"])?;
        repo.run(["config", "--local", "core.safecrlf", "false"])?;
        Ok(repo)
    }

    /// Open the repository in `dir` as the primary repository of a workspace. `dir/.git`
    /// must be a real directory. Fails when the repository configuration enables
    /// automation (see [`GitRepo::config_findings`]).
    pub fn open(dir: &Path) -> Result<GitRepo, GitError> {
        Self::open_with_identity(dir, Identity::factory())
    }

    /// As [`GitRepo::open`], with an explicit committer identity.
    pub fn open_with_identity(dir: &Path, identity: Identity) -> Result<GitRepo, GitError> {
        let meta = fs::symlink_metadata(dir.join(".git")).map_err(|_| GitError::NotARepository)?;
        if paths::is_link_like(&meta) || !meta.is_dir() {
            return Err(GitError::NotARepository);
        }
        let repo = GitRepo {
            work_dir: dir.to_path_buf(),
            kind: RepoKind::Primary,
            identity,
        };
        // Configuration first: it needs no working tree, so a hostile `core.worktree` is
        // reported as a finding instead of breaking the later commands.
        let findings = repo.config_findings().map_err(|err| match err {
            GitError::Failed { .. } => GitError::NotARepository,
            other => other,
        })?;
        if !findings.is_empty() {
            return Err(GitError::UnsafeConfig(findings));
        }
        let git_dir = repo
            .run_text(["rev-parse", "--git-dir"])
            .map_err(|_| GitError::NotARepository)?;
        if git_dir.trim() != ".git" {
            return Err(GitError::NotARepository);
        }
        Ok(repo)
    }

    /// Open a linked worktree of this repository. `dir/.git` must be a file whose
    /// `gitdir:` target is inside this repository's `.git/worktrees/`.
    fn open_linked(&self, dir: &Path) -> Result<GitRepo, GitError> {
        let pointer = dir.join(".git");
        let meta = fs::symlink_metadata(&pointer).map_err(|_| GitError::WorktreeMissing)?;
        if paths::is_link_like(&meta) || !meta.is_file() {
            return Err(GitError::WorktreeMissing);
        }
        let text = fs::read_to_string(&pointer).map_err(|_| GitError::WorktreeMissing)?;
        let target = text
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("gitdir:"))
            .map(str::trim)
            .ok_or(GitError::WorktreeMissing)?;
        let target = {
            let raw = PathBuf::from(target);
            if raw.is_absolute() {
                raw
            } else {
                dir.join(raw)
            }
        };
        let allowed = fs::canonicalize(self.work_dir.join(".git").join("worktrees"))
            .map_err(|_| GitError::WorktreeMissing)?;
        let target = fs::canonicalize(target).map_err(|_| GitError::WorktreeMissing)?;
        if !target.starts_with(&allowed) {
            return Err(GitError::GitDirOutsideWorkspace);
        }
        Ok(GitRepo {
            work_dir: dir.to_path_buf(),
            kind: RepoKind::Linked,
            identity: self.identity.clone(),
        })
    }

    /// Working directory of this handle. Server-side use only.
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(&self.work_dir);
        for (key, _) in std::env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("GIT_")
            {
                command.env_remove(key);
            }
        }
        let ceiling = self
            .work_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.work_dir.clone());
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_CEILING_DIRECTORIES", ceiling)
            .env("LC_ALL", "C")
            .stdin(Stdio::null());
        command
            .arg("--no-optional-locks")
            .arg("--literal-pathspecs")
            .args(["-c", "core.autocrlf=false"])
            .args(["-c", "core.safecrlf=false"])
            .args(["-c", "core.hooksPath=/dev/null"])
            .args(["-c", "core.fsmonitor=false"])
            .args(["-c", "core.quotepath=false"])
            .args(["-c", "core.longpaths=true"])
            .args(["-c", "protocol.ext.allow=never"])
            .args(["-c", "gc.auto=0"])
            .args(["-c", "maintenance.auto=false"])
            .args(["-c", "commit.gpgsign=false"])
            .args(["-c", "core.editor=true"])
            .arg("-c")
            .arg(format!("user.name={}", self.identity.name))
            .arg("-c")
            .arg(format!("user.email={}", self.identity.email));
        command
    }

    fn exec<I, S>(&self, args: I, envs: &[(&str, &str)]) -> Result<Output, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.command();
        command.args(args);
        for (key, value) in envs {
            command.env(key, value);
        }
        command.output().map_err(GitError::Spawn)
    }

    /// Run git and require exit code 0. Returns stdout.
    fn run<I, S>(&self, args: I) -> Result<Vec<u8>, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_env(args, &[])
    }

    fn run_env<I, S>(&self, args: I, envs: &[(&str, &str)]) -> Result<Vec<u8>, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<S> = args.into_iter().collect();
        let subcommand = args
            .first()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .unwrap_or_default();
        let output = self.exec(&args, envs)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(GitError::Failed {
                subcommand,
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            })
        }
    }

    fn run_text<I, S>(&self, args: I) -> Result<String, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let bytes = self.run(args)?;
        String::from_utf8(bytes).map_err(|_| GitError::Parse("output is not UTF-8".into()))
    }

    /// Exit code 0 means true, 1 means false, anything else is an error.
    fn run_bool<I, S>(&self, args: I) -> Result<bool, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<S> = args.into_iter().collect();
        let subcommand = args
            .first()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .unwrap_or_default();
        let output = self.exec(&args, &[])?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            code => Err(GitError::Failed {
                subcommand,
                code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        }
    }

    // ---- inspection ----

    /// Current commit, `None` when the repository has no commits.
    pub fn head(&self) -> Result<Option<Oid>, GitError> {
        let output = self.exec(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"], &[])?;
        match output.status.code() {
            Some(0) => Ok(Some(Oid::new(
                String::from_utf8_lossy(&output.stdout).trim(),
            )?)),
            Some(1) => Ok(None),
            code => Err(GitError::Failed {
                subcommand: "rev-parse".into(),
                code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        }
    }

    /// Short name of the checked-out branch, `None` when HEAD is detached.
    pub fn current_branch(&self) -> Result<Option<String>, GitError> {
        let output = self.exec(["symbolic-ref", "--quiet", "--short", "HEAD"], &[])?;
        match output.status.code() {
            Some(0) => Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            )),
            Some(1) => Ok(None),
            code => Err(GitError::Failed {
                subcommand: "symbolic-ref".into(),
                code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        }
    }

    /// Absolute path of this repository's index file.
    pub fn index_path(&self) -> Result<PathBuf, GitError> {
        let text = self.run_text(["rev-parse", "--git-path", "index"])?;
        let path = PathBuf::from(text.trim());
        Ok(if path.is_absolute() {
            path
        } else {
            self.work_dir.join(path)
        })
    }

    /// SHA-256 of the index file as it is on disk, `None` when there is none.
    pub fn index_sha256(&self) -> Result<Option<String>, GitError> {
        let path = self.index_path()?;
        match sha256_file(&path) {
            Ok(hash) => Ok(Some(hash)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(GitError::Io(err)),
        }
    }

    /// Record HEAD, branch and the index hash. Read-only.
    pub fn baseline(&self) -> Result<Baseline, GitError> {
        Ok(Baseline {
            head: self.head()?,
            branch: self.current_branch()?,
            index_sha256: self.index_sha256()?,
        })
    }

    /// Configuration keys that would run programs or redirect the repository. The key
    /// names are returned, never the values.
    pub fn config_findings(&self) -> Result<Vec<String>, GitError> {
        let mut findings = Vec::new();
        let raw = match self.kind {
            // Read `.git/config` as a plain file without discovering the repository, because
            // discovery parses the configuration with its includes. A hostile `include.path`
            // would otherwise be read before this check, and one naming a file that is not
            // configuration (`/etc/passwd` exists on Linux and macOS, not on Windows) fails the
            // repository as unreadable instead of reporting the key. GIT_DIR names a path under
            // a regular file, which can never exist, so Git runs outside any repository.
            RepoKind::Primary => self.run_env(
                [
                    "config",
                    "--no-includes",
                    "--file",
                    ".git/config",
                    "--list",
                    "-z",
                ],
                &[("GIT_DIR", ".git/config/no-repository")],
            )?,
            RepoKind::Linked => self.run(["config", "--local", "--list", "-z"])?,
        };
        for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let text = String::from_utf8_lossy(record);
            let key = text.split('\n').next().unwrap_or("").to_ascii_lowercase();
            if config_key_is_dangerous(&key) {
                findings.push(key);
            }
        }
        let git_dir = self.work_dir.join(".git");
        if matches!(self.kind, RepoKind::Primary) {
            if git_dir.join("commondir").exists() {
                findings.push("commondir file".into());
            }
            if git_dir
                .join("objects")
                .join("info")
                .join("alternates")
                .exists()
            {
                findings.push("objects/info/alternates file".into());
            }
        }
        findings.sort();
        findings.dedup();
        Ok(findings)
    }

    pub(crate) fn scan(&self) -> Result<RepoScan, GitError> {
        let mut scan = RepoScan::default();

        let staged = self.run(["ls-files", "-z", "-s"])?;
        for record in staged.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let text = String::from_utf8_lossy(record);
            let (meta, path) = text
                .split_once('\t')
                .ok_or_else(|| GitError::Parse("ls-files -s record".into()))?;
            if meta.starts_with("160000") {
                scan.gitlinks.insert(path.to_owned());
            }
            scan.tracked.insert(path.to_owned());
        }

        let status = self.run([
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=no",
            "--no-renames",
            "--ignore-submodules=all",
        ])?;
        let mut records = status.split(|b| *b == 0).filter(|r| !r.is_empty());
        while let Some(record) = records.next() {
            let text = String::from_utf8_lossy(record);
            let kind = text.split(' ').next().unwrap_or("");
            let fields = match kind {
                "1" => 9,
                "2" => 10,
                "u" => 11,
                "#" | "?" | "!" => continue,
                other => return Err(GitError::Parse(format!("status record kind {other:?}"))),
            };
            let all: Vec<&str> = text.splitn(fields, ' ').collect();
            if all.len() != fields || all[1].len() != 2 {
                return Err(GitError::Parse("status record fields".into()));
            }
            let xy = all[1].as_bytes();
            let path = all[fields - 1].to_owned();
            if kind == "2" {
                // The original path follows as its own NUL-terminated record.
                let _ = records.next();
            }
            scan.status.insert(path, (xy[0], xy[1]));
        }

        let untracked = self.run(["ls-files", "-z", "-o", "--exclude-standard"])?;
        for record in untracked.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let path = String::from_utf8_lossy(record).into_owned();
            if !path.ends_with('/') {
                scan.untracked.insert(path);
            }
        }
        let ignored = self.run(["ls-files", "-z", "-o", "-i", "--exclude-standard"])?;
        for record in ignored.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let path = String::from_utf8_lossy(record).into_owned();
            if !path.ends_with('/') {
                scan.ignored.insert(path);
            }
        }
        Ok(scan)
    }

    /// Staged, unstaged and untracked paths. Read-only.
    pub fn dirty_state(&self) -> Result<DirtyState, GitError> {
        let scan = self.scan()?;
        let mut state = DirtyState::default();
        for (path, (x, y)) in &scan.status {
            if *x != b'.' {
                state.staged.push(path.clone());
            }
            if *y != b'.' {
                state.unstaged.push(path.clone());
            }
        }
        state.untracked = scan.untracked.into_iter().collect();
        Ok(state)
    }

    // ---- baseline commit ----

    /// Stage everything and create the first commit of a repository that has none, using
    /// the committer identity of this handle. Refuses when HEAD already exists, so it can
    /// never sweep files into a user's history or touch an existing index.
    pub fn baseline_commit(&self, message: &str) -> Result<Oid, GitError> {
        if !matches!(self.kind, RepoKind::Primary) {
            return Err(GitError::NotARepository);
        }
        if self.head()?.is_some() {
            return Err(GitError::HeadAlreadyExists);
        }
        self.run(["add", "-A"])?;
        self.run([
            "commit",
            "--quiet",
            "--allow-empty",
            "--no-verify",
            "--no-gpg-sign",
            "--cleanup=verbatim",
            "-m",
            message,
        ])?;
        self.head()?.ok_or(GitError::UnbornHead)
    }

    // ---- owned branches and worktrees ----

    fn require_commit(&self, oid: &Oid) -> Result<(), GitError> {
        let spec = format!("{oid}^{{commit}}");
        if self.run_bool(["rev-parse", "--verify", "--quiet", &spec])? {
            Ok(())
        } else {
            Err(GitError::InvalidOid)
        }
    }

    fn ref_oid(&self, full_ref: &str) -> Result<Option<Oid>, GitError> {
        let output = self.exec(["rev-parse", "--verify", "--quiet", full_ref], &[])?;
        match output.status.code() {
            Some(0) => Ok(Some(Oid::new(
                String::from_utf8_lossy(&output.stdout).trim(),
            )?)),
            Some(1) => Ok(None),
            code => Err(GitError::Failed {
                subcommand: "rev-parse".into(),
                code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        }
    }

    /// Create the owned branch `hoi4factory/<task>` at `base` without touching the
    /// working tree or the index. Fails when the branch exists.
    pub fn create_task_branch(&self, task: &TaskId, base: &Oid) -> Result<OwnedBranch, GitError> {
        self.require_commit(base)?;
        let name = task_branch_name(task);
        let full = format!("refs/heads/{name}");
        if self.ref_oid(&full)?.is_some() {
            return Err(GitError::BranchExists);
        }
        // An empty old value makes update-ref create the ref only if it is absent.
        self.run([
            "update-ref",
            "-m",
            "hoi4factory: create task branch",
            &full,
            base.as_str(),
            "",
        ])?;
        Ok(OwnedBranch {
            task: task.clone(),
            name,
            tip: base.clone(),
        })
    }

    /// Reopen the owned branch of a task and report its current tip.
    pub fn open_task_branch(&self, task: &TaskId) -> Result<OwnedBranch, GitError> {
        let name = task_branch_name(task);
        let tip = self
            .ref_oid(&format!("refs/heads/{name}"))?
            .ok_or(GitError::BranchMissing)?;
        Ok(OwnedBranch {
            task: task.clone(),
            name,
            tip,
        })
    }

    /// Create a linked worktree for a task at `dir` on a new owned branch starting at
    /// `base`. `dir` must not exist yet and its parent must. The primary working tree,
    /// its index and every unrelated file stay untouched.
    pub fn add_task_worktree(
        &self,
        task: &TaskId,
        dir: &Path,
        base: &Oid,
    ) -> Result<TaskWorktree, GitError> {
        if !matches!(self.kind, RepoKind::Primary) {
            return Err(GitError::NotARepository);
        }
        self.require_commit(base)?;
        let name = task_branch_name(task);
        if self.ref_oid(&format!("refs/heads/{name}"))?.is_some() {
            return Err(GitError::BranchExists);
        }
        if fs::symlink_metadata(dir).is_ok() {
            return Err(GitError::WorktreeExists);
        }
        self.run([
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--quiet"),
            OsStr::new("--no-track"),
            OsStr::new("-b"),
            OsStr::new(&name),
            OsStr::new("--"),
            dir.as_os_str(),
            OsStr::new(base.as_str()),
        ])?;
        self.open_task_worktree(task, dir, Some(base))
    }

    /// Reopen the worktree of a task. Verifies that the directory is a linked worktree of
    /// this repository, that it is on the task's owned branch and, when `expected_head`
    /// is given, that HEAD has not moved.
    pub fn open_task_worktree(
        &self,
        task: &TaskId,
        dir: &Path,
        expected_head: Option<&Oid>,
    ) -> Result<TaskWorktree, GitError> {
        let linked = self.open_linked(dir)?;
        let branch = task_branch_name(task);
        if !self.worktree_registered(dir)? {
            return Err(GitError::WorktreeMissing);
        }
        let actual_branch = linked.current_branch()?;
        if actual_branch.as_deref() != Some(branch.as_str()) {
            return Err(GitError::WrongBranch {
                expected: branch,
                actual: actual_branch,
            });
        }
        let head = linked.head()?.ok_or(GitError::UnbornHead)?;
        if let Some(expected) = expected_head
            && *expected != head
        {
            return Err(GitError::HeadMoved {
                expected: expected.to_string(),
                actual: head.to_string(),
            });
        }
        Ok(TaskWorktree {
            task: task.clone(),
            branch,
            expected_head: head,
            repo: linked,
        })
    }

    /// True when `dir` is listed by `git worktree list` as a worktree of this repository.
    fn worktree_registered(&self, dir: &Path) -> Result<bool, GitError> {
        let wanted_dir = fs::canonicalize(dir).map_err(|_| GitError::WorktreeMissing)?;
        let raw = self.run(["worktree", "list", "--porcelain", "-z"])?;
        for field in raw.split(|b| *b == 0) {
            let text = String::from_utf8_lossy(field);
            if let Some(value) = text.strip_prefix("worktree ")
                && fs::canonicalize(value)
                    .map(|c| c == wanted_dir)
                    .unwrap_or(false)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Remove a task worktree without force. Git refuses when the worktree has
    /// uncommitted or untracked files, which are never discarded here. The owned branch
    /// and restore points stay.
    pub fn remove_task_worktree(&self, worktree: &TaskWorktree) -> Result<(), GitError> {
        worktree.verify_head()?;
        self.run([
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--"),
            worktree.repo.work_dir.as_os_str(),
        ])?;
        Ok(())
    }

    // ---- remote ----

    /// Record the linked remote URL and default branch in the repository's local
    /// configuration (`remote.origin.url` and `hoi4factory.defaultBranch`). It never
    /// fetches and never pushes. Only `https://` URLs without credentials are accepted.
    /// A different URL already configured under `origin` is a conflict.
    pub fn link_remote(&self, url: &str, default_branch: &str) -> Result<RemoteLink, GitError> {
        validate_remote_url(url)?;
        if !valid_branch_name(default_branch) {
            return Err(GitError::InvalidRemote("default branch name".into()));
        }
        match self.remote_url()? {
            Some(existing) if existing == url => {}
            Some(_) => return Err(GitError::RemoteConflict),
            None => {
                self.run(["remote", "add", REMOTE_NAME, url])?;
            }
        }
        self.run(["config", "--local", DEFAULT_BRANCH_KEY, default_branch])?;
        Ok(RemoteLink {
            name: REMOTE_NAME.into(),
            url: url.to_owned(),
            default_branch: Some(default_branch.to_owned()),
        })
    }

    fn remote_url(&self) -> Result<Option<String>, GitError> {
        let output = self.exec(["config", "--local", "--get", "remote.origin.url"], &[])?;
        match output.status.code() {
            Some(0) => Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            )),
            Some(1) => Ok(None),
            code => Err(GitError::Failed {
                subcommand: "config".into(),
                code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            }),
        }
    }

    /// The recorded remote, if any. Read-only, no network.
    pub fn remote_link(&self) -> Result<Option<RemoteLink>, GitError> {
        let Some(url) = self.remote_url()? else {
            return Ok(None);
        };
        let output = self.exec(["config", "--local", "--get", DEFAULT_BRANCH_KEY], &[])?;
        let default_branch = match output.status.code() {
            Some(0) => Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
            _ => None,
        };
        Ok(Some(RemoteLink {
            name: REMOTE_NAME.into(),
            url,
            default_branch,
        }))
    }
}

fn config_key_is_dangerous(key: &str) -> bool {
    let tail = key.rsplit('.').next().unwrap_or("");
    if key.starts_with("filter.") && matches!(tail, "clean" | "smudge" | "process") {
        return true;
    }
    if key.starts_with("diff.") && matches!(tail, "command" | "textconv") {
        return true;
    }
    if key.starts_with("merge.") && tail == "driver" {
        return true;
    }
    if key.starts_with("credential.") && tail == "helper" {
        return true;
    }
    if key.starts_with("gpg.") && tail == "program" {
        return true;
    }
    if key.starts_with("includeif.") && tail == "path" {
        return true;
    }
    matches!(
        key,
        "diff.external"
            | "core.sshcommand"
            | "core.gitproxy"
            | "core.askpass"
            | "core.pager"
            | "core.worktree"
            | "core.attributesfile"
            | "core.excludesfile"
            | "include.path"
            | "uploadpack.packobjectshook"
            | "credential.helper"
            | "sequence.editor"
    )
}

fn validate_remote_url(url: &str) -> Result<(), GitError> {
    let bad = |what: &str| Err(GitError::InvalidRemote(what.to_owned()));
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return bad("url contains whitespace or control characters or is too long");
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return bad("only https remotes are accepted");
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => return bad("url has no path"),
    };
    if authority.is_empty() || authority.contains('@') {
        return bad("url has no host or carries credentials");
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || host.starts_with('.')
        || host.starts_with('-')
    {
        return bad("url host");
    }
    if let Some(port) = port
        && (port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()))
    {
        return bad("url port");
    }
    if path.is_empty() || path.contains('?') || path.contains('#') || path.contains('\\') {
        return bad("url path");
    }
    Ok(())
}

/// A task's linked worktree on its owned branch, with the HEAD the host expects.
#[derive(Debug, Clone)]
pub struct TaskWorktree {
    task: TaskId,
    branch: String,
    expected_head: Oid,
    repo: GitRepo,
}

impl TaskWorktree {
    pub fn task(&self) -> &TaskId {
        &self.task
    }

    /// Name of the owned branch.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// HEAD the host recorded. Persist it with the task so a later reopen can detect
    /// movement.
    pub fn expected_head(&self) -> &Oid {
        &self.expected_head
    }

    pub fn repo(&self) -> &GitRepo {
        &self.repo
    }

    pub fn work_dir(&self) -> &Path {
        self.repo.work_dir()
    }

    /// Re-read HEAD and the checked-out branch. Fails with [`GitError::HeadMoved`] or
    /// [`GitError::WrongBranch`] when either differs from what was recorded.
    pub fn verify_head(&self) -> Result<Oid, GitError> {
        let branch = self.repo.current_branch()?;
        if branch.as_deref() != Some(self.branch.as_str()) {
            return Err(GitError::WrongBranch {
                expected: self.branch.clone(),
                actual: branch,
            });
        }
        let actual = self.repo.head()?.ok_or(GitError::UnbornHead)?;
        if actual != self.expected_head {
            return Err(GitError::HeadMoved {
                expected: self.expected_head.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(actual)
    }

    /// Accept the current HEAD as the new expectation. Call only after reconciling why it
    /// moved.
    pub fn accept_current_head(&mut self) -> Result<Oid, GitError> {
        let head = self.repo.head()?.ok_or(GitError::UnbornHead)?;
        self.expected_head = head.clone();
        Ok(head)
    }

    /// Record a restore point for the current HEAD under
    /// `refs/hoi4factory/restore/<task>/<label>`. Fails when HEAD moved or the label is
    /// taken. The ref keeps the commit reachable.
    pub fn create_restore_point(&self, label: &RestoreLabel) -> Result<RestorePoint, GitError> {
        let head = self.verify_head()?;
        let ref_name = restore_ref_name(&self.task, label);
        if self.repo.ref_oid(&ref_name)?.is_some() {
            return Err(GitError::RestorePointExists);
        }
        self.repo.run([
            "update-ref",
            "-m",
            "hoi4factory: restore point",
            &ref_name,
            head.as_str(),
            "",
        ])?;
        Ok(RestorePoint {
            ref_name,
            oid: head,
        })
    }

    /// Restore points of this task, sorted by ref name.
    pub fn restore_points(&self) -> Result<Vec<RestorePoint>, GitError> {
        let prefix = format!("{RESTORE_REF_PREFIX}{}/", self.task);
        let text =
            self.repo
                .run_text(["for-each-ref", "--format=%(refname) %(objectname)", &prefix])?;
        let mut points = Vec::new();
        for line in text.lines() {
            let (name, oid) = line
                .split_once(' ')
                .ok_or_else(|| GitError::Parse("for-each-ref record".into()))?;
            points.push(RestorePoint {
                ref_name: name.to_owned(),
                oid: Oid::new(oid)?,
            });
        }
        points.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        Ok(points)
    }

    /// Stage exactly `paths` and commit them. Every path is checked with the workspace
    /// path rules first. Fails when HEAD moved, when the index already holds staged
    /// changes (so nothing unrelated can be swept in) or when the paths change nothing.
    /// `author`, when given, is the acting member. The committer stays the factory.
    /// Updates the expected HEAD to the new commit.
    pub fn commit_paths(
        &mut self,
        paths: &[String],
        message: &str,
        author: Option<&Identity>,
    ) -> Result<Oid, WorkspaceError> {
        self.verify_head()?;
        if paths.is_empty() {
            return Err(GitError::NothingToCommit.into());
        }
        for path in paths {
            paths::resolve(self.repo.work_dir(), path, ResolveMode::MayCreate)?;
        }
        let staged = self.repo.run(["diff", "--cached", "--name-only", "-z"])?;
        if !staged.is_empty() {
            return Err(GitError::UnexpectedStagedChanges.into());
        }
        let mut add: Vec<&str> = vec!["add", "-A", "--"];
        add.extend(paths.iter().map(String::as_str));
        self.repo.run(add)?;
        if !self.repo.run_bool(["diff", "--cached", "--quiet"])? {
            let envs: Vec<(&str, &str)> = match author {
                Some(a) => vec![
                    ("GIT_AUTHOR_NAME", a.name.as_str()),
                    ("GIT_AUTHOR_EMAIL", a.email.as_str()),
                ],
                None => Vec::new(),
            };
            self.repo.run_env(
                [
                    "commit",
                    "--quiet",
                    "--no-verify",
                    "--no-gpg-sign",
                    "--cleanup=verbatim",
                    "-m",
                    message,
                ],
                &envs,
            )?;
        } else {
            return Err(GitError::NothingToCommit.into());
        }
        let head = self.repo.head()?.ok_or(GitError::UnbornHead)?;
        self.expected_head = head.clone();
        Ok(head)
    }
}
