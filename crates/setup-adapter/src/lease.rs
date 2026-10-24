//! The per-project lease: at most one owner drives a setup transaction.
//!
//! A lease is a PostgreSQL transaction-level advisory lock held by an open transaction on a
//! dedicated connection. It is taken without waiting (`pg_try_advisory_xact_lock`), so a second
//! owner is refused at once with `project_busy` instead of queueing behind a long apply. The
//! lock disappears with the transaction: a normal release commits it, and a crash of this
//! process, a lost connection or a dropped guard ends the transaction and frees the lock, so a
//! dead owner cannot hold a project forever. Every lease also draws a fencing generation from a
//! database sequence; receipts record it, and a later owner always has a larger one.
//!
//! The lease is the cross-process guard. Two further guards back it: a partial unique index on
//! `setup_transactions` (one unfinished transaction per project) and Setup's own refusal to
//! start a transaction while a journal for the same root is incomplete.

use factory_accounts::Database;
use sqlx::{Postgres, Transaction};

use crate::error::SetupError;
use crate::ids::{AccountId, ProjectId};

/// Exclusive right to drive setup effects on one project.
pub struct ProjectLease {
    tx: Option<Transaction<'static, Postgres>>,
    project: ProjectId,
    generation: i64,
}

impl ProjectLease {
    /// Take the lease or fail at once with [`SetupError::ProjectBusy`].
    pub async fn try_acquire(
        db: &Database,
        account: AccountId,
        project: ProjectId,
    ) -> Result<Self, SetupError> {
        let mut tx = db.scoped(account).begin().await?;
        let key = format!("factory.setup.project:{project}");
        let got: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(&key)
                .fetch_one(&mut *tx)
                .await?;
        if !got {
            tx.rollback().await?;
            return Err(SetupError::ProjectBusy { project });
        }
        let generation: i64 = sqlx::query_scalar("SELECT nextval('setup_fence_seq')")
            .fetch_one(&mut *tx)
            .await?;
        Ok(Self {
            tx: Some(tx),
            project,
            generation,
        })
    }

    /// Fencing generation drawn when the lease was taken.
    pub fn generation(&self) -> i64 {
        self.generation
    }

    pub fn project(&self) -> ProjectId {
        self.project
    }

    /// Give the lease up.
    pub async fn release(mut self) -> Result<(), SetupError> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await?;
        }
        Ok(())
    }
}

impl Drop for ProjectLease {
    fn drop(&mut self) {
        // A guard dropped without `release` (a cancelled request, a panic) must not leave the
        // lock to whenever the connection is next used.
        if let Some(tx) = self.tx.take() {
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(async move {
                        let _ = tx.rollback().await;
                    });
                }
                Err(_) => drop(tx),
            }
        }
    }
}
