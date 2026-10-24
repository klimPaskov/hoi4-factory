//! GitHub import: list the repositories an installation can see, then import one.
//!
//! The import is a read-only clone of one branch into a fresh workspace, authorised by a
//! short-lived installation token minted for that one repository. Order of work:
//!
//! 1. Everything that needs no network: the repository name, the branch name and the
//!    project's allowlist.
//! 2. The App JWT from the secret store, then `GET /app/installations/{id}`. Anything but
//!    `repository_selection = selected` is refused, and so is a suspended installation.
//! 3. `POST .../access_tokens` with `repositories: [name]`. The response must list exactly
//!    that repository.
//! 4. `GET /repos/{owner}/{name}` for the node id, default branch and size.
//! 5. `git ls-remote` for the refs and the branch tip, then init, fetch and checkout in a
//!    new workspace. The credential reaches `git` only as an `http.<base>/.extraheader`
//!    environment entry for that one process.
//!
//! HTTP 401 and 404 anywhere mean the connection is gone: [`IntakeError::AccessRevoked`].
//! A workspace this call created is removed on failure. A workspace that already existed is
//! never touched. There is no push, no fetch beyond this import, and no pull request.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use factory_workspace::{
    AccountId, GitError, GitRepo, Identity, InitOptions, Oid, ProjectId, SnapshotInventory,
    StorageRoot, valid_branch_name,
};

use super::api::{
    GithubApi, GithubEndpoints, GithubError, InstallationInfo, InstallationToken, RepositoryInfo,
};
use super::jwt::app_jwt;
use super::link::{InstallationRef, ObservedRef, RepositoryAllowlist, parse_full_name};
use crate::config::{FileTypePolicy, IntakeLimits, TypeVerdict};
use crate::error::IntakeError;
use crate::extract::FileRecord;
use crate::gitcmd::{self, GitAuth, GitRunError};
use crate::gitfinal::{NEUTRAL_ATTRIBUTES, force_remove_dir_all, scan_gitattributes};
use crate::model::{Finding, InventorySummary};
use crate::secrets::{SecretSource, SecretString};

/// Server-side settings of the GitHub import.
#[derive(Debug, Clone)]
pub struct GithubConfig {
    pub endpoints: GithubEndpoints,
    /// Secret-store key of the GitHub App id.
    pub app_id_secret: String,
    /// Secret-store key of the App private key (PEM).
    pub private_key_secret: String,
    /// Host recorded in the workspace's `origin` URL. Always `https`.
    pub canonical_remote_host: String,
    /// Largest repository GitHub may report, in KiB.
    pub max_repository_kib: u64,
    /// Most refs recorded on the link.
    pub max_recorded_refs: usize,
    pub ls_remote_timeout: Duration,
    pub fetch_timeout: Duration,
}

impl GithubConfig {
    pub fn new(endpoints: GithubEndpoints) -> Self {
        GithubConfig {
            endpoints,
            app_id_secret: "github_app.id".into(),
            private_key_secret: "github_app.private_key".into(),
            canonical_remote_host: "github.com".into(),
            max_repository_kib: 1024 * 1024,
            max_recorded_refs: 200,
            ls_remote_timeout: Duration::from_secs(60),
            fetch_timeout: Duration::from_secs(900),
        }
    }
}

/// What an import is asked to do.
#[derive(Debug, Clone, Copy)]
pub struct ImportRequest<'a> {
    pub account: &'a AccountId,
    pub project: &'a ProjectId,
    pub installation: &'a InstallationRef,
    /// `owner/name`.
    pub repository: &'a str,
    /// Branch to import. `None` imports the default branch.
    pub branch: Option<&'a str>,
    pub allowlist: &'a RepositoryAllowlist,
}

/// A repository the installation can see, and whether this project may import it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VisibleRepository {
    pub repository: RepositoryInfo,
    pub allowed: bool,
}

/// The checks passed and a token exists. Holds the token, so it is not `Clone` and prints
/// no secret.
pub struct PreparedImport {
    pub repository: RepositoryInfo,
    pub branch: String,
    pub installation_id: u64,
    token: InstallationToken,
}

impl std::fmt::Debug for PreparedImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedImport")
            .field("repository", &self.repository.full_name)
            .field("branch", &self.branch)
            .finish_non_exhaustive()
    }
}

/// An imported repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ImportOutcome {
    pub repository: RepositoryInfo,
    pub branch: String,
    pub head: Oid,
    pub observed_refs: Vec<ObservedRef>,
    /// The `origin` URL recorded in the workspace. No credential.
    pub remote_url: String,
    pub source_locator: String,
    pub findings: Vec<Finding>,
    pub inventory: InventorySummary,
    /// Paths that differ from the commit after checkout. Zero means the working tree
    /// matches the remote ref byte for byte.
    pub dirty_paths: u64,
}

/// Imports from GitHub into server workspaces.
#[derive(Clone)]
pub struct GithubImporter {
    api: Arc<dyn GithubApi>,
    secrets: Arc<dyn SecretSource>,
    storage: StorageRoot,
    config: GithubConfig,
    limits: IntakeLimits,
    policy: FileTypePolicy,
}

impl std::fmt::Debug for GithubImporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubImporter").finish_non_exhaustive()
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, IntakeError> + Send + 'static,
) -> Result<T, IntakeError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| IntakeError::Aborted)?
}

fn map_git(err: GitRunError) -> IntakeError {
    if err.looks_unauthorized() {
        IntakeError::AccessRevoked
    } else {
        err.into()
    }
}

/// Parse `git ls-remote --heads --tags` output.
fn parse_refs(listing: &[u8], max: usize, keep: &str) -> Result<Vec<ObservedRef>, IntakeError> {
    let text = String::from_utf8_lossy(listing);
    let mut refs = Vec::new();
    for line in text.lines() {
        let Some((sha, name)) = line.split_once('\t') else {
            continue;
        };
        if name.ends_with("^{}")
            || !(name.starts_with("refs/heads/") || name.starts_with("refs/tags/"))
        {
            continue;
        }
        let sha_ok = (sha.len() == 40 || sha.len() == 64)
            && sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !sha_ok || name.chars().any(char::is_control) || name.len() > 300 {
            return Err(IntakeError::GithubProtocol(
                "ls-remote line is malformed".into(),
            ));
        }
        refs.push(ObservedRef {
            name: name.to_owned(),
            sha: sha.to_owned(),
        });
    }
    refs.sort_by(|a, b| a.name.cmp(&b.name));
    if refs.len() > max {
        let kept = refs.iter().position(|r| r.name == keep);
        let mut cut: Vec<ObservedRef> = refs.iter().take(max).cloned().collect();
        if let Some(i) = kept
            && i >= max
        {
            cut.pop();
            cut.push(refs[i].clone());
        }
        return Ok(cut);
    }
    Ok(refs)
}

impl GithubImporter {
    pub fn new(
        api: Arc<dyn GithubApi>,
        secrets: Arc<dyn SecretSource>,
        storage: StorageRoot,
        config: GithubConfig,
        limits: IntakeLimits,
        policy: FileTypePolicy,
    ) -> Self {
        GithubImporter {
            api,
            secrets,
            storage,
            config,
            limits,
            policy,
        }
    }

    pub fn config(&self) -> &GithubConfig {
        &self.config
    }

    fn secret(&self, name: &str, what: &str) -> Result<SecretString, IntakeError> {
        self.secrets
            .get(name)
            .map_err(|e| IntakeError::Credentials(e.0))?
            .ok_or_else(|| IntakeError::Credentials(format!("{what} is not in the secret store")))
    }

    /// Installation id and a fresh App JWT.
    fn credentials(
        &self,
        installation: &InstallationRef,
    ) -> Result<(u64, SecretString), IntakeError> {
        let id_text = self.secret(installation.as_str(), "the installation reference")?;
        let installation_id: u64 = id_text.expose().trim().parse().map_err(|_| {
            IntakeError::Credentials("the installation reference is not an id".into())
        })?;
        let app_id = self.secret(&self.config.app_id_secret, "the App id")?;
        let pem = self.secret(&self.config.private_key_secret, "the App private key")?;
        let jwt = app_jwt(app_id.expose().trim(), &pem, now_unix())?;
        Ok((installation_id, jwt))
    }

    async fn require_selected(
        &self,
        jwt: &SecretString,
        installation_id: u64,
    ) -> Result<InstallationInfo, IntakeError> {
        let info = self
            .api
            .get_installation(jwt.expose(), installation_id)
            .await?;
        if info.repository_selection != "selected" {
            return Err(IntakeError::InstallationNotSelected);
        }
        if info.suspended_at.is_some() {
            return Err(IntakeError::InstallationSuspended);
        }
        Ok(info)
    }

    /// Check that the installation still exists and is limited to selected repositories.
    /// A 401 or 404 is [`IntakeError::AccessRevoked`].
    pub async fn check_installation(
        &self,
        installation: &InstallationRef,
    ) -> Result<InstallationInfo, IntakeError> {
        let (installation_id, jwt) = self.credentials(installation)?;
        self.require_selected(&jwt, installation_id).await
    }

    /// Repositories the installation can see, with the project's allowlist applied as a
    /// flag. Only a `selected` installation is listed.
    pub async fn list_repositories(
        &self,
        installation: &InstallationRef,
        allowlist: &RepositoryAllowlist,
    ) -> Result<Vec<VisibleRepository>, IntakeError> {
        let (installation_id, jwt) = self.credentials(installation)?;
        self.require_selected(&jwt, installation_id).await?;
        let token = self
            .api
            .create_installation_token(jwt.expose(), installation_id, None)
            .await?;
        let mut repositories = self
            .api
            .list_installation_repositories(token.token.expose())
            .await?;
        repositories.sort_by_key(|r| r.full_name.to_lowercase());
        Ok(repositories
            .into_iter()
            .map(|repository| VisibleRepository {
                allowed: allowlist.contains(&repository.full_name),
                repository,
            })
            .collect())
    }

    /// Everything before the clone: allowlist, installation, token, repository, branch.
    pub async fn prepare_import(
        &self,
        request: &ImportRequest<'_>,
    ) -> Result<PreparedImport, IntakeError> {
        let Some((_, name)) = parse_full_name(request.repository) else {
            return Err(IntakeError::InvalidRequest(
                "repository must be owner/name".into(),
            ));
        };
        if !request.allowlist.contains(request.repository) {
            return Err(IntakeError::RepositoryNotAllowed);
        }
        if let Some(branch) = request.branch
            && !valid_branch_name(branch)
        {
            return Err(IntakeError::BranchNameUnsupported);
        }
        let (installation_id, jwt) = self.credentials(request.installation)?;
        self.require_selected(&jwt, installation_id).await?;
        let token = self
            .api
            .create_installation_token(jwt.expose(), installation_id, Some(&[name.to_owned()]))
            .await
            .map_err(|err| match err {
                // GitHub answers 422 when a named repository is not part of the installation.
                GithubError::Status { status: 422, .. } => IntakeError::RepositoryNotInInstallation,
                other => other.into(),
            })?;
        match token.repositories.as_slice() {
            [only] if only.eq_ignore_ascii_case(request.repository) => {}
            [] => {
                return Err(IntakeError::GithubProtocol(
                    "the token response did not name its repositories".into(),
                ));
            }
            _ => return Err(IntakeError::RepositoryNotInInstallation),
        }
        let repository = self
            .api
            .get_repository(token.token.expose(), request.repository)
            .await?;
        if !repository
            .full_name
            .eq_ignore_ascii_case(request.repository)
        {
            // Renamed or transferred since the allowlist entry was made.
            return Err(IntakeError::RepositoryNotAllowed);
        }
        if repository.size > self.config.max_repository_kib {
            return Err(IntakeError::RepositoryTooLarge {
                limit_kib: self.config.max_repository_kib,
            });
        }
        let branch = request
            .branch
            .map(str::to_owned)
            .unwrap_or_else(|| repository.default_branch.clone());
        if !valid_branch_name(&branch) {
            return Err(IntakeError::BranchNameUnsupported);
        }
        Ok(PreparedImport {
            repository,
            branch,
            installation_id,
            token,
        })
    }

    /// Prepare and clone.
    pub async fn import(&self, request: &ImportRequest<'_>) -> Result<ImportOutcome, IntakeError> {
        let prepared = self.prepare_import(request).await?;
        self.clone_into_workspace(request, &prepared).await
    }

    /// Clone the prepared repository and branch into a new workspace.
    pub async fn clone_into_workspace(
        &self,
        request: &ImportRequest<'_>,
        prepared: &PreparedImport,
    ) -> Result<ImportOutcome, IntakeError> {
        let info = &prepared.repository;
        let branch = prepared.branch.clone();
        let transport_url = self.config.endpoints.clone_url(&info.full_name);
        let auth = GitAuth::new(
            &self.config.endpoints.clone_base,
            prepared.token.token.expose(),
        );
        self.storage.ensure_layout()?;

        let listing = gitcmd::run(
            self.storage.data_root(),
            &["ls-remote", "--heads", "--tags", "--", &transport_url],
            Some(&auth),
            self.config.ls_remote_timeout,
        )
        .await
        .map_err(map_git)?;
        let head_ref = format!("refs/heads/{branch}");
        let mut observed = parse_refs(&listing.stdout, self.config.max_recorded_refs, &head_ref)?;
        if !observed.iter().any(|r| r.name == head_ref) {
            return Err(IntakeError::BranchNotFound);
        }

        let workspace = self
            .storage
            .create_workspace(request.account, request.project)?;
        let project_dir = workspace
            .source_dir()
            .parent()
            .map(std::path::Path::to_path_buf)
            .ok_or(IntakeError::Workspace(
                factory_workspace::WorkspaceError::NotFound,
            ))?;
        let guard = DirGuard(Some(project_dir));
        let source = workspace.source_dir();

        let canonical = format!(
            "https://{}/{}.git",
            self.config.canonical_remote_host, info.full_name
        );
        {
            let source = source.clone();
            let branch = branch.clone();
            let default_branch = info.default_branch.clone();
            let canonical = canonical.clone();
            blocking(move || {
                let repo = GitRepo::init(
                    &source,
                    InitOptions {
                        consent: true,
                        initial_branch: branch,
                        identity: Identity::factory(),
                    },
                )?;
                // Highest-precedence attributes: no checkout conversion, whatever the
                // repository's own .gitattributes says, so the tree equals the commit.
                let info_dir = source.join(".git").join("info");
                std::fs::create_dir_all(&info_dir)
                    .map_err(IntakeError::io("create git info directory"))?;
                std::fs::write(info_dir.join("attributes"), NEUTRAL_ATTRIBUTES)
                    .map_err(IntakeError::io("write git attributes"))?;
                repo.link_remote(&canonical, &default_branch)
                    .map_err(|err| match err {
                        GitError::InvalidRemote(_) => IntakeError::BranchNameUnsupported,
                        other => IntakeError::Git(other),
                    })?;
                Ok(())
            })
            .await?;
        }

        let tracking = format!("refs/remotes/origin/{branch}");
        let refspec = format!("+{head_ref}:{tracking}");
        gitcmd::run(
            &source,
            &["fetch", "--no-tags", "--", &transport_url, &refspec],
            Some(&auth),
            self.config.fetch_timeout,
        )
        .await
        .map_err(map_git)?;
        let verify_spec = format!("{tracking}^{{commit}}");
        let tip = gitcmd::run(
            &source,
            &["rev-parse", "--verify", "--quiet", &verify_spec],
            None,
            Duration::from_secs(60),
        )
        .await
        .map_err(map_git)?;
        let tip = Oid::new(String::from_utf8_lossy(&tip.stdout).trim())?;
        gitcmd::run(
            &source,
            &["checkout", "-q", "-B", &branch, &tracking],
            None,
            self.config.fetch_timeout,
        )
        .await
        .map_err(map_git)?;
        let upstream = format!("--set-upstream-to=origin/{branch}");
        gitcmd::run(
            &source,
            &["branch", &upstream, &branch],
            None,
            Duration::from_secs(60),
        )
        .await
        .map_err(map_git)?;

        // The branch moved between ls-remote and fetch: record what was actually fetched.
        for entry in observed.iter_mut().filter(|r| r.name == head_ref) {
            entry.sha = tip.as_str().to_owned();
        }

        let (findings, inventory, dirty_paths, head) = {
            let source = source.clone();
            let limits = self.limits.clone();
            let policy = self.policy.clone();
            blocking(move || inspect_clone(&source, &limits, &policy)).await?
        };
        if head != tip {
            return Err(IntakeError::Git(GitError::HeadMoved {
                expected: tip.to_string(),
                actual: head.to_string(),
            }));
        }
        guard.disarm();
        Ok(ImportOutcome {
            repository: info.clone(),
            branch,
            head,
            observed_refs: observed,
            remote_url: canonical,
            source_locator: workspace.source_locator(),
            findings,
            inventory,
            dirty_paths,
        })
    }
}

/// Removes the project directory this call created unless disarmed.
struct DirGuard(Option<std::path::PathBuf>);

impl DirGuard {
    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take()
            && let Err(err) = force_remove_dir_all(&dir)
        {
            tracing::error!(error = %err, "could not remove the workspace of a failed import");
        }
    }
}

/// Facts about a fresh clone, and the limit checks that apply to any workspace.
fn inspect_clone(
    source: &std::path::Path,
    limits: &IntakeLimits,
    policy: &FileTypePolicy,
) -> Result<(Vec<Finding>, InventorySummary, u64, Oid), IntakeError> {
    let repo = GitRepo::open_with_identity(source, Identity::factory())?;
    let head = repo.head()?.ok_or(GitError::UnbornHead)?;
    let dirty = repo.dirty_state()?;
    let inventory = SnapshotInventory::capture(source, Some(&repo))?;
    if inventory.entries.len() as u64 > limits.max_files {
        return Err(IntakeError::TooManyFiles {
            limit: limits.max_files,
        });
    }
    let total: u64 = inventory.entries.iter().map(|e| e.size).sum();
    if total > limits.max_total_bytes {
        return Err(IntakeError::TotalTooLarge {
            limit: limits.max_total_bytes,
        });
    }
    let mut findings = Vec::new();
    let mut records = Vec::new();
    for entry in &inventory.entries {
        let name = entry.path.rsplit('/').next().unwrap_or("");
        if entry.kind == factory_workspace::EntryKind::Symlink {
            findings.push(Finding::SymlinkInRepository {
                path: entry.path.clone(),
            });
            continue;
        }
        if policy.judge(name) == TypeVerdict::Executable {
            findings.push(Finding::ExecutableFile {
                path: entry.path.clone(),
            });
        }
        if name == ".gitmodules" {
            findings.push(Finding::Submodules {
                path: entry.path.clone(),
            });
        }
        if entry.size < 1024
            && entry.kind == factory_workspace::EntryKind::File
            && let Ok(bytes) = std::fs::read(source.join(&entry.path))
            && bytes.starts_with(b"version https://git-lfs.github.com/spec/v1")
        {
            findings.push(Finding::LfsPointer {
                path: entry.path.clone(),
            });
        }
        records.push(FileRecord {
            path: entry.path.clone(),
            size: entry.size,
            sha256: String::new(),
            in_git: false,
        });
    }
    findings.extend(scan_gitattributes(source, &records));
    let summary = InventorySummary {
        entries: inventory.entries.len() as u64,
        content_hash: inventory.content_hash(),
    };
    let dirty_paths = (dirty.staged.len() + dirty.unstaged.len() + dirty.untracked.len()) as u64;
    Ok((findings, summary, dirty_paths, head))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(sha_char: char, name: &str) -> String {
        format!("{}\t{name}\n", sha_char.to_string().repeat(40))
    }

    #[test]
    fn refs_are_parsed_sorted_and_peeled_tags_skipped() {
        let listing = [
            line('b', "refs/tags/v1"),
            line('c', "refs/tags/v1^{}"),
            line('a', "refs/heads/main"),
            line('d', "refs/pull/1/head"),
        ]
        .concat();
        let refs = parse_refs(listing.as_bytes(), 10, "refs/heads/main").unwrap();
        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["refs/heads/main", "refs/tags/v1"]);
        assert_eq!(refs[1].sha, "b".repeat(40));
    }

    #[test]
    fn the_imported_branch_survives_the_cap() {
        let mut listing = String::new();
        for i in 0..10 {
            listing.push_str(&line('a', &format!("refs/heads/b{i:02}")));
        }
        let refs = parse_refs(listing.as_bytes(), 3, "refs/heads/b09").unwrap();
        assert_eq!(refs.len(), 3);
        assert!(refs.iter().any(|r| r.name == "refs/heads/b09"));
    }

    #[test]
    fn malformed_lines_are_a_protocol_error() {
        let bad = "nothex\trefs/heads/main\n";
        assert!(parse_refs(bad.as_bytes(), 10, "x").is_err());
    }
}
