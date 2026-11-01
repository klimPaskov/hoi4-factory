//! Storing and loading C-06 bundles. A stored bundle is immutable (no UPDATE or DELETE grant and
//! a trigger). `load_bundle` recomputes the hashes, so a bundle altered behind the application's
//! back is reported, never used.

use factory_accounts::{AccountId, ProjectId};
use sqlx::{PgConnection, Row};

use crate::context::{ContextBundle, content_hash, manifest_hash};
use crate::ids::{BundleId, TaskId};

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("context bundle not found")]
    NotFound,
    /// The stored bundle no longer hashes to what was recorded.
    #[error("context bundle integrity failure: expected {expected}, found {actual}")]
    IntegrityFailure { expected: String, actual: String },
    #[error("the bundle to store does not match its own hashes")]
    InconsistentBundle,
    /// The task does not exist in this scope.
    #[error("task not found")]
    TaskNotFound,
    #[error("stored bundle is unreadable: {0}")]
    Unreadable(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Stores a bundle for a task. Refuses a bundle whose hashes do not verify.
pub async fn store_bundle(
    conn: &mut PgConnection,
    project: ProjectId,
    owner: AccountId,
    task: TaskId,
    bundle: &ContextBundle,
) -> Result<(), BundleError> {
    if bundle.verify().is_err() {
        return Err(BundleError::InconsistentBundle);
    }
    let value = serde_json::to_value(bundle).map_err(|e| BundleError::Unreadable(e.to_string()))?;
    sqlx::query(
        "INSERT INTO context_bundles (id, task_id, project_id, owner_account_id, manifest_sha256, bundle)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(bundle.id)
    .bind(task)
    .bind(project.0)
    .bind(owner.0)
    .bind(&bundle.manifest_sha256)
    .bind(value)
    .execute(&mut *conn)
    .await
    .map_err(|e| {
        if crate::error::is_scope_violation(&e) {
            BundleError::TaskNotFound
        } else {
            BundleError::Database(e)
        }
    })?;
    Ok(())
}

/// Loads a bundle and verifies the manifest hash and the content hash.
pub async fn load_bundle(
    conn: &mut PgConnection,
    id: BundleId,
) -> Result<ContextBundle, BundleError> {
    let row = sqlx::query("SELECT manifest_sha256, bundle FROM context_bundles WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(BundleError::NotFound)?;
    let recorded: String = row.try_get("manifest_sha256")?;
    let value: serde_json::Value = row.try_get("bundle")?;
    let bundle: ContextBundle =
        serde_json::from_value(value).map_err(|e| BundleError::Unreadable(e.to_string()))?;
    let manifest = manifest_hash(&bundle.manifest);
    if manifest != recorded || bundle.manifest_sha256 != recorded {
        return Err(BundleError::IntegrityFailure {
            expected: recorded,
            actual: manifest,
        });
    }
    let content = content_hash(&bundle.content);
    if content != bundle.manifest.content_sha256 {
        return Err(BundleError::IntegrityFailure {
            expected: bundle.manifest.content_sha256,
            actual: content,
        });
    }
    if bundle.id != id {
        return Err(BundleError::IntegrityFailure {
            expected: id.to_string(),
            actual: bundle.id.to_string(),
        });
    }
    Ok(bundle)
}
