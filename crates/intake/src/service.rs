//! The intake service: uploads and GitHub imports, with their database records.
//!
//! Each operation records its intent before it acts, so a crash leaves a row that says what
//! was attempted: an upload in `receiving`, a link in `pending`. The row is completed or
//! rejected after the work. A lost completion write is a database error with the retry
//! class `reconcile_first`, never a silent success.

use std::io::Read;
use std::sync::Arc;

use factory_workspace::{AccountId, ProjectId};
use uuid::Uuid;

use crate::error::IntakeError;
use crate::github::{
    GithubImporter, ImportOutcome, ImportRequest, InstallationRef, LinkState, RepositoryLink,
    VisibleRepository, parse_full_name,
};
use crate::model::{UploadFailure, UploadKind, UploadMeta, UploadOptions, UploadOutcome};
use crate::store::IntakeStore;
use crate::upload::{FolderEntry, UploadIntake, UploadTarget};

/// Request to import a GitHub repository into a new workspace of `project`.
#[derive(Debug, Clone)]
pub struct GithubImportRequest {
    pub account: Uuid,
    pub project: Uuid,
    /// Identity of the new link, chosen by the caller so a retry is recognisable.
    pub link_id: Uuid,
    pub installation: InstallationRef,
    /// `owner/name`.
    pub repository: String,
    pub branch: Option<String>,
}

#[derive(Clone)]
pub struct IntakeService {
    store: IntakeStore,
    uploads: Arc<UploadIntake>,
    github: Arc<GithubImporter>,
}

fn ids(account: Uuid, project: Uuid) -> Result<(AccountId, ProjectId), IntakeError> {
    Ok((
        AccountId::new(account.to_string())?,
        ProjectId::new(project.to_string())?,
    ))
}

fn is_unique_violation(err: &IntakeError) -> bool {
    matches!(
        err,
        IntakeError::Database(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505")
    )
}

impl IntakeService {
    pub fn new(
        store: IntakeStore,
        uploads: Arc<UploadIntake>,
        github: Arc<GithubImporter>,
    ) -> Self {
        IntakeService {
            store,
            uploads,
            github,
        }
    }

    pub fn store(&self) -> &IntakeStore {
        &self.store
    }

    // ---- uploads ----

    /// Receive a ZIP archive into a new workspace of `project`.
    pub async fn upload_zip(
        &self,
        account: Uuid,
        project: Uuid,
        meta: UploadMeta,
        archive: Box<dyn Read + Send>,
        options: UploadOptions,
    ) -> Result<UploadOutcome, UploadFailure> {
        let (account_id, project_id) = ids(account, project).map_err(UploadFailure::plain)?;
        self.store
            .begin_upload(
                account,
                project,
                meta.upload_id,
                UploadKind::Zip,
                meta.original_name.as_deref(),
            )
            .await
            .map_err(UploadFailure::plain)?;
        let uploads = self.uploads.clone();
        let upload_id = meta.upload_id;
        let result = tokio::task::spawn_blocking(move || {
            let mut archive = archive;
            let target = UploadTarget {
                account: &account_id,
                project: &project_id,
            };
            uploads.ingest_zip(&target, &meta, &mut *archive, &options)
        })
        .await
        .unwrap_or_else(|_| Err(UploadFailure::plain(IntakeError::Aborted)));
        self.finish_upload(account, upload_id, result).await
    }

    /// Receive a folder upload into a new workspace of `project`.
    pub async fn upload_folder(
        &self,
        account: Uuid,
        project: Uuid,
        meta: UploadMeta,
        entries: Vec<FolderEntry>,
        options: UploadOptions,
    ) -> Result<UploadOutcome, UploadFailure> {
        let (account_id, project_id) = ids(account, project).map_err(UploadFailure::plain)?;
        self.store
            .begin_upload(
                account,
                project,
                meta.upload_id,
                UploadKind::Folder,
                meta.original_name.as_deref(),
            )
            .await
            .map_err(UploadFailure::plain)?;
        let uploads = self.uploads.clone();
        let upload_id = meta.upload_id;
        let result = tokio::task::spawn_blocking(move || {
            let target = UploadTarget {
                account: &account_id,
                project: &project_id,
            };
            uploads.ingest_folder(&target, &meta, entries, &options)
        })
        .await
        .unwrap_or_else(|_| Err(UploadFailure::plain(IntakeError::Aborted)));
        self.finish_upload(account, upload_id, result).await
    }

    async fn finish_upload(
        &self,
        account: Uuid,
        upload_id: Uuid,
        result: Result<UploadOutcome, UploadFailure>,
    ) -> Result<UploadOutcome, UploadFailure> {
        match result {
            Ok(outcome) => {
                self.store
                    .complete_upload(account, &outcome)
                    .await
                    .map_err(UploadFailure::plain)?;
                Ok(outcome)
            }
            Err(failure) => {
                if let Err(err) = self.store.reject_upload(account, upload_id, &failure).await {
                    tracing::error!(error = %err, "could not record a rejected upload");
                }
                Err(failure)
            }
        }
    }

    // ---- GitHub ----

    /// Repositories the project's installation can see, flagged by the project allowlist.
    pub async fn list_github_repositories(
        &self,
        account: Uuid,
        project: Uuid,
        installation: &InstallationRef,
    ) -> Result<Vec<VisibleRepository>, IntakeError> {
        let allowlist = self.store.allowlist(account, project).await?;
        self.github
            .list_repositories(installation, &allowlist)
            .await
    }

    /// Import a repository and branch into a new workspace and record its C-10 link.
    pub async fn import_github(
        &self,
        request: GithubImportRequest,
    ) -> Result<(RepositoryLink, ImportOutcome), IntakeError> {
        let (account_id, project_id) = ids(request.account, request.project)?;
        if parse_full_name(&request.repository).is_none() {
            return Err(IntakeError::InvalidRequest(
                "repository must be owner/name".into(),
            ));
        }
        let allowlist = self
            .store
            .allowlist(request.account, request.project)
            .await?;
        if !allowlist.contains(&request.repository) {
            return Err(IntakeError::RepositoryNotAllowed);
        }
        self.store
            .insert_pending_link(
                request.account,
                request.project,
                request.link_id,
                project_id.as_str(),
                &request.repository,
                request.installation.as_str(),
            )
            .await
            .map_err(|err| {
                if is_unique_violation(&err) {
                    IntakeError::InvalidRequest("the project already has a repository link".into())
                } else {
                    err
                }
            })?;

        let import = ImportRequest {
            account: &account_id,
            project: &project_id,
            installation: &request.installation,
            repository: &request.repository,
            branch: request.branch.as_deref(),
            allowlist: &allowlist,
        };
        let prepared = match self.github.prepare_import(&import).await {
            Ok(prepared) => prepared,
            Err(err) => return Err(self.fail_link(&request, err).await),
        };
        self.store
            .record_repository(
                request.account,
                request.link_id,
                &prepared.repository,
                &prepared.branch,
            )
            .await?;
        let outcome = match self.github.clone_into_workspace(&import, &prepared).await {
            Ok(outcome) => outcome,
            Err(err) => return Err(self.fail_link(&request, err).await),
        };
        self.store
            .activate_link(request.account, request.link_id, &outcome)
            .await?;
        let link = self
            .store
            .get_link(request.account, request.link_id)
            .await?
            .ok_or_else(|| IntakeError::InvalidRequest("link vanished".into()))?;
        Ok((link, outcome))
    }

    /// Record the failure on the link and hand the error back. A revoked connection becomes
    /// `access_revoked`, anything else `failed`. The workspace of a failed import was removed
    /// by the importer. A workspace that existed before is never touched.
    async fn fail_link(&self, request: &GithubImportRequest, err: IntakeError) -> IntakeError {
        let (state, reason) = match &err {
            IntakeError::AccessRevoked => {
                (LinkState::AccessRevoked, "installation_or_repository_gone")
            }
            IntakeError::InstallationNotSelected => {
                (LinkState::AccessRevoked, "installation_not_selected")
            }
            other => (LinkState::Failed, other.code()),
        };
        if let Err(db) = self
            .store
            .set_link_state(request.account, request.link_id, state, Some(reason))
            .await
        {
            tracing::error!(error = %db, "could not record the link state after a failed import");
        }
        err
    }

    /// Re-check the installation behind a link. A 401 or 404, or an installation that is no
    /// longer limited to selected repositories, turns an active link to `access_revoked`.
    /// The server workspace is left as it is. Rate limits and outages do not change the state.
    pub async fn check_link(
        &self,
        account: Uuid,
        link_id: Uuid,
    ) -> Result<RepositoryLink, IntakeError> {
        let link = self
            .store
            .get_link(account, link_id)
            .await?
            .ok_or_else(|| IntakeError::InvalidRequest("no such link".into()))?;
        if link.state != LinkState::Active {
            return Ok(link);
        }
        let installation = InstallationRef::new(link.installation_ref.clone())?;
        match self.github.check_installation(&installation).await {
            Ok(_) => Ok(link),
            Err(IntakeError::AccessRevoked) => {
                self.store
                    .set_link_state(
                        account,
                        link_id,
                        LinkState::AccessRevoked,
                        Some("installation_or_repository_gone"),
                    )
                    .await?;
                self.reload(account, link_id).await
            }
            Err(IntakeError::InstallationNotSelected) => {
                self.store
                    .set_link_state(
                        account,
                        link_id,
                        LinkState::AccessRevoked,
                        Some("installation_not_selected"),
                    )
                    .await?;
                self.reload(account, link_id).await
            }
            Err(other) => Err(other),
        }
    }

    async fn reload(&self, account: Uuid, link_id: Uuid) -> Result<RepositoryLink, IntakeError> {
        self.store
            .get_link(account, link_id)
            .await?
            .ok_or_else(|| IntakeError::InvalidRequest("link vanished".into()))
    }
}
