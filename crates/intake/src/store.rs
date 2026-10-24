//! PostgreSQL persistence for uploads, repository links and the project allowlist.
//!
//! Every method opens a transaction, sets `app.account_id` as its first statement and then
//! queries. The row-level security policies of migrations 0300 to 0302 key on that setting
//! and fail closed, so a query cannot see or change another account's rows even if the SQL
//! forgot a filter. The application must connect as a role that is neither a superuser nor
//! `BYPASSRLS` (the accounts crate checks this at startup).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::IntakeError;
use crate::github::{
    ImportOutcome, LinkState, ObservedRef, RepositoryAllowlist, RepositoryInfo, RepositoryLink,
    SELECTED_SCOPE,
};
use crate::model::{Finding, GitOutcome, UploadFailure, UploadKind, UploadOutcome};

/// Row of `intake_uploads`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UploadRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub status: String,
    pub original_name: Option<String>,
    pub original_sha256: Option<String>,
    pub original_bytes: Option<i64>,
    pub original_locator: Option<String>,
    pub file_count: Option<i32>,
    pub git_state: Option<String>,
    pub git_head: Option<String>,
    pub inventory_hash: Option<String>,
    pub findings: Vec<Finding>,
    pub rejection_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Tenant-scoped access to the intake tables.
#[derive(Debug, Clone)]
pub struct IntakeStore {
    pool: PgPool,
}

fn json_error(err: serde_json::Error) -> IntakeError {
    IntakeError::Database(sqlx::Error::Decode(Box::new(err)))
}

impl IntakeStore {
    pub fn new(pool: PgPool) -> Self {
        IntakeStore { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Begin a transaction scoped to `account`.
    async fn scoped(&self, account: Uuid) -> Result<Transaction<'static, Postgres>, IntakeError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.account_id', $1, true)")
            .bind(account.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    // ---- uploads ----

    /// Record the intent to receive an upload.
    pub async fn begin_upload(
        &self,
        account: Uuid,
        project: Uuid,
        upload: Uuid,
        kind: UploadKind,
        original_name: Option<&str>,
    ) -> Result<(), IntakeError> {
        let mut tx = self.scoped(account).await?;
        sqlx::query(
            "INSERT INTO intake_uploads (id, account_id, project_id, kind, status, original_name)
             VALUES ($1, $2, $3, $4, 'receiving', $5)",
        )
        .bind(upload)
        .bind(account)
        .bind(project)
        .bind(kind.as_str())
        .bind(original_name.map(crate::error::shorten))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn complete_upload(
        &self,
        account: Uuid,
        outcome: &UploadOutcome,
    ) -> Result<(), IntakeError> {
        let (git_state, git_head) = match &outcome.git {
            GitOutcome::NotInitialised => ("not_initialised", None),
            GitOutcome::Initialised { head, .. } => ("initialised", Some(head.to_string())),
            GitOutcome::ExistingRepository { head, .. } => (
                "existing_repository",
                head.as_ref().map(ToString::to_string),
            ),
        };
        let mut tx = self.scoped(account).await?;
        let updated = sqlx::query(
            "UPDATE intake_uploads SET status = 'accepted', original_sha256 = $2, original_bytes = $3,
                    original_locator = $4, file_count = $5, directory_count = $6, extracted_bytes = $7,
                    git_state = $8, git_head = $9, inventory_hash = $10, findings = $11,
                    completed_at = now()
             WHERE id = $1 AND status = 'receiving'",
        )
        .bind(outcome.upload_id)
        .bind(&outcome.original.sha256)
        .bind(outcome.original.bytes as i64)
        .bind(&outcome.original.locator)
        .bind(outcome.extracted.files as i32)
        .bind(outcome.extracted.directories as i32)
        .bind(outcome.extracted.bytes as i64)
        .bind(git_state)
        .bind(git_head)
        .bind(&outcome.inventory.content_hash)
        .bind(Json(&outcome.findings))
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(IntakeError::InvalidRequest(
                "upload is not in the receiving state".into(),
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn reject_upload(
        &self,
        account: Uuid,
        upload: Uuid,
        failure: &UploadFailure,
    ) -> Result<(), IntakeError> {
        let mut tx = self.scoped(account).await?;
        sqlx::query(
            "UPDATE intake_uploads SET status = 'rejected', rejection_code = $2, rejection_message = $3,
                    original_sha256 = $4, original_bytes = $5, completed_at = now()
             WHERE id = $1 AND status = 'receiving'",
        )
        .bind(upload)
        .bind(failure.code())
        .bind(failure.error.to_string())
        .bind(failure.received_sha256.as_deref())
        .bind(failure.received_bytes.map(|b| b as i64))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_upload(
        &self,
        account: Uuid,
        upload: Uuid,
    ) -> Result<Option<UploadRecord>, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let row = sqlx::query(
            "SELECT id, account_id, project_id, kind, status, original_name, original_sha256,
                    original_bytes, original_locator, file_count, git_state, git_head,
                    inventory_hash, findings, rejection_code, created_at, completed_at
             FROM intake_uploads WHERE id = $1",
        )
        .bind(upload)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let findings: Json<Vec<Finding>> = row.try_get("findings")?;
        Ok(Some(UploadRecord {
            id: row.try_get("id")?,
            account_id: row.try_get("account_id")?,
            project_id: row.try_get("project_id")?,
            kind: row.try_get("kind")?,
            status: row.try_get("status")?,
            original_name: row.try_get("original_name")?,
            original_sha256: row.try_get("original_sha256")?,
            original_bytes: row.try_get("original_bytes")?,
            original_locator: row.try_get("original_locator")?,
            file_count: row.try_get("file_count")?,
            git_state: row.try_get("git_state")?,
            git_head: row.try_get("git_head")?,
            inventory_hash: row.try_get("inventory_hash")?,
            findings: findings.0,
            rejection_code: row.try_get("rejection_code")?,
            created_at: row.try_get("created_at")?,
            completed_at: row.try_get("completed_at")?,
        }))
    }

    // ---- allowlist ----

    pub async fn allow_repository(
        &self,
        account: Uuid,
        project: Uuid,
        full_name: &str,
    ) -> Result<(), IntakeError> {
        if crate::github::parse_full_name(full_name).is_none() {
            return Err(IntakeError::InvalidRequest(
                "repository must be owner/name".into(),
            ));
        }
        let mut tx = self.scoped(account).await?;
        sqlx::query(
            "INSERT INTO github_repository_allowlist (account_id, project_id, repository_full_name)
             VALUES ($1, $2, lower($3)) ON CONFLICT DO NOTHING",
        )
        .bind(account)
        .bind(project)
        .bind(full_name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn disallow_repository(
        &self,
        account: Uuid,
        project: Uuid,
        full_name: &str,
    ) -> Result<bool, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let result = sqlx::query(
            "DELETE FROM github_repository_allowlist WHERE project_id = $1 AND repository_full_name = lower($2)",
        )
        .bind(project)
        .bind(full_name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn allowlist(
        &self,
        account: Uuid,
        project: Uuid,
    ) -> Result<RepositoryAllowlist, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let rows = sqlx::query(
            "SELECT repository_full_name FROM github_repository_allowlist
             WHERE project_id = $1 ORDER BY repository_full_name",
        )
        .bind(project)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let names: Vec<String> = rows
            .iter()
            .map(|r| r.try_get("repository_full_name"))
            .collect::<Result<_, _>>()?;
        RepositoryAllowlist::new(names)
    }

    // ---- repository links ----

    /// Record the intent to link and import, before any GitHub call that produces facts.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_pending_link(
        &self,
        account: Uuid,
        project: Uuid,
        link: Uuid,
        workspace_id: &str,
        full_name: &str,
        installation_ref: &str,
    ) -> Result<(), IntakeError> {
        let mut tx = self.scoped(account).await?;
        sqlx::query(
            "INSERT INTO repository_links
                 (id, account_id, project_id, workspace_id, repository_full_name, installation_ref,
                  selected_scope, state)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')",
        )
        .bind(link)
        .bind(account)
        .bind(project)
        .bind(workspace_id)
        .bind(full_name)
        .bind(installation_ref)
        .bind(SELECTED_SCOPE)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Record what GitHub said about the repository, before the clone starts.
    pub async fn record_repository(
        &self,
        account: Uuid,
        link: Uuid,
        repository: &RepositoryInfo,
        branch: &str,
    ) -> Result<(), IntakeError> {
        let mut tx = self.scoped(account).await?;
        sqlx::query(
            "UPDATE repository_links SET repository_full_name = $2, repository_node_id = $3,
                    repository_id = $4, default_branch = $5, imported_branch = $6, updated_at = now()
             WHERE id = $1 AND state = 'pending'",
        )
        .bind(link)
        .bind(&repository.full_name)
        .bind(&repository.node_id)
        .bind(repository.id as i64)
        .bind(&repository.default_branch)
        .bind(branch)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn activate_link(
        &self,
        account: Uuid,
        link: Uuid,
        outcome: &ImportOutcome,
    ) -> Result<(), IntakeError> {
        let mut tx = self.scoped(account).await?;
        let updated = sqlx::query(
            "UPDATE repository_links SET state = 'active', state_reason = NULL,
                    repository_full_name = $2, repository_node_id = $3, repository_id = $4,
                    default_branch = $5, imported_branch = $6, imported_commit = $7,
                    observed_refs = $8, observed_at = now(), updated_at = now()
             WHERE id = $1 AND state = 'pending'",
        )
        .bind(link)
        .bind(&outcome.repository.full_name)
        .bind(&outcome.repository.node_id)
        .bind(outcome.repository.id as i64)
        .bind(&outcome.repository.default_branch)
        .bind(&outcome.branch)
        .bind(outcome.head.to_string())
        .bind(Json(&outcome.observed_refs))
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(IntakeError::InvalidRequest("link is not pending".into()));
        }
        tx.commit().await?;
        Ok(())
    }

    /// Move a link to `state`. Returns false when no such link is visible to the account.
    pub async fn set_link_state(
        &self,
        account: Uuid,
        link: Uuid,
        state: LinkState,
        reason: Option<&str>,
    ) -> Result<bool, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let result = sqlx::query(
            "UPDATE repository_links SET state = $2, state_reason = $3, updated_at = now() WHERE id = $1",
        )
        .bind(link)
        .bind(state.as_str())
        .bind(reason)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_link(
        &self,
        account: Uuid,
        link: Uuid,
    ) -> Result<Option<RepositoryLink>, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let row = sqlx::query(LINK_SELECT_BY_ID)
            .bind(link)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;
        row.map(link_from_row).transpose()
    }

    /// The links of a project, newest first.
    pub async fn links_for_project(
        &self,
        account: Uuid,
        project: Uuid,
    ) -> Result<Vec<RepositoryLink>, IntakeError> {
        let mut tx = self.scoped(account).await?;
        let rows = sqlx::query(LINK_SELECT_BY_PROJECT)
            .bind(project)
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        rows.into_iter().map(link_from_row).collect()
    }
}

const LINK_SELECT_BY_ID: &str = "SELECT id, account_id, project_id, workspace_id, provider,
    repository_full_name, repository_node_id, repository_id, installation_ref, selected_scope,
    default_branch, imported_branch, imported_commit, observed_refs, observed_at, state,
    state_reason, created_at, updated_at FROM repository_links WHERE id = $1";

const LINK_SELECT_BY_PROJECT: &str = "SELECT id, account_id, project_id, workspace_id, provider,
    repository_full_name, repository_node_id, repository_id, installation_ref, selected_scope,
    default_branch, imported_branch, imported_commit, observed_refs, observed_at, state,
    state_reason, created_at, updated_at FROM repository_links WHERE project_id = $1
    ORDER BY created_at DESC";

fn link_from_row(row: sqlx::postgres::PgRow) -> Result<RepositoryLink, IntakeError> {
    let refs: Json<Vec<ObservedRef>> = row.try_get("observed_refs")?;
    let state_text: String = row.try_get("state")?;
    let state = LinkState::parse(&state_text)
        .ok_or_else(|| json_error(serde::de::Error::custom("unknown link state in database")))?;
    Ok(RepositoryLink {
        id: row.try_get("id")?,
        account_id: row.try_get("account_id")?,
        project_id: row.try_get("project_id")?,
        workspace_id: row.try_get("workspace_id")?,
        provider: row.try_get("provider")?,
        repository_full_name: row.try_get("repository_full_name")?,
        repository_node_id: row.try_get("repository_node_id")?,
        repository_id: row.try_get("repository_id")?,
        installation_ref: row.try_get("installation_ref")?,
        selected_scope: row.try_get("selected_scope")?,
        default_branch: row.try_get("default_branch")?,
        imported_branch: row.try_get("imported_branch")?,
        imported_commit: row.try_get("imported_commit")?,
        observed_refs: refs.0,
        observed_at: row.try_get("observed_at")?,
        state,
        state_reason: row.try_get("state_reason")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
