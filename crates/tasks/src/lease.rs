//! Host incarnations, workspace leases and fencing.
//!
//! A project workspace has at most one writer. The writer is a host incarnation holding a lease
//! with a fencing generation. A takeover (the lease is absent, expired, or the same incarnation
//! re-acquiring after expiry) increments the generation, so every effect carries the generation
//! it was authorized under and the effect boundary refuses any other. A timeout may allow
//! replacement; it never authorizes the previous owner to continue.
//!
//! Time comes from the database clock (`now()`), so replicas with skewed clocks agree.
//! `check_fence` locks the lease row `FOR SHARE`: a takeover waits for the in-flight check's
//! transaction, so an effect that passed its check cannot be overtaken mid-transaction.

use std::time::Duration;

use chrono::{DateTime, Utc};
use factory_accounts::{AccountId, ProjectId};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Row};

use crate::error::TaskError;
use crate::ids::IncarnationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub generation: i64,
    pub expires_at: DateTime<Utc>,
}

/// Why the effect boundary refused a holder.
#[derive(Debug, thiserror::Error)]
pub enum FenceError {
    #[error("stale fencing generation: presented {presented}, current {current}")]
    StaleGeneration { presented: i64, current: i64 },
    #[error("the lease has expired")]
    LeaseExpired,
    #[error("no lease is held by this incarnation")]
    NotHeld,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Records one start of the server process. The label is a short build or process name, never a
/// host name or path.
pub async fn register_incarnation(
    conn: &mut PgConnection,
    label: &str,
) -> Result<IncarnationId, TaskError> {
    let valid = !label.is_empty()
        && label.len() <= 64
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
    if !valid {
        return Err(TaskError::Invalid(
            "an incarnation label is 1 to 64 characters of letters, digits and . _ : -".into(),
        ));
    }
    let id = IncarnationId::new();
    sqlx::query("INSERT INTO host_incarnations (id, label) VALUES ($1, $2)")
        .bind(id)
        .bind(label)
        .execute(&mut *conn)
        .await?;
    Ok(id)
}

/// Marks the incarnation alive.
pub async fn touch_incarnation(
    conn: &mut PgConnection,
    id: IncarnationId,
) -> Result<(), TaskError> {
    sqlx::query("UPDATE host_incarnations SET last_seen_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

fn secs(ttl: Duration) -> f64 {
    ttl.as_secs_f64()
}

/// Takes the project's lease for `incarnation` when it is absent, expired, or already held by
/// this incarnation. Returns `Ok(None)` when a live lease belongs to another incarnation.
/// Re-acquiring a live lease you hold keeps its generation; every other success increments it.
pub async fn acquire_lease(
    conn: &mut PgConnection,
    project: ProjectId,
    owner: AccountId,
    incarnation: IncarnationId,
    ttl: Duration,
) -> Result<Option<Lease>, TaskError> {
    let row = sqlx::query(
        "INSERT INTO workspace_leases (project_id, owner_account_id, holder_incarnation, generation,
             expires_at, updated_at)
         VALUES ($1, $2, $3, 1, now() + make_interval(secs => $4), now())
         ON CONFLICT (project_id) DO UPDATE SET
             holder_incarnation = EXCLUDED.holder_incarnation,
             generation = CASE
                 WHEN workspace_leases.holder_incarnation = EXCLUDED.holder_incarnation
                      AND workspace_leases.expires_at > now()
                 THEN workspace_leases.generation
                 ELSE workspace_leases.generation + 1 END,
             expires_at = EXCLUDED.expires_at,
             updated_at = now()
         WHERE workspace_leases.expires_at <= now()
            OR workspace_leases.holder_incarnation = EXCLUDED.holder_incarnation
         RETURNING generation, expires_at",
    )
    .bind(project.0)
    .bind(owner.0)
    .bind(incarnation)
    .bind(secs(ttl))
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|r| {
        Ok(Lease {
            generation: r.try_get("generation")?,
            expires_at: r.try_get("expires_at")?,
        })
    })
    .transpose()
}

/// Extends a live lease. Fails when the generation is stale, another incarnation holds it or it
/// has already expired (an expired lease must be re-acquired, which increments the generation).
pub async fn renew_lease(
    conn: &mut PgConnection,
    project: ProjectId,
    incarnation: IncarnationId,
    generation: i64,
    ttl: Duration,
) -> Result<Lease, FenceError> {
    let row = sqlx::query(
        "UPDATE workspace_leases SET expires_at = now() + make_interval(secs => $4), updated_at = now()
         WHERE project_id = $1 AND holder_incarnation = $2 AND generation = $3 AND expires_at > now()
         RETURNING generation, expires_at",
    )
    .bind(project.0)
    .bind(incarnation)
    .bind(generation)
    .bind(secs(ttl))
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(r) => Ok(Lease {
            generation: r.try_get("generation")?,
            expires_at: r.try_get("expires_at")?,
        }),
        None => {
            check_fence(conn, project, incarnation, generation).await?;
            Err(FenceError::NotHeld)
        }
    }
}

/// The effect-boundary check: is `(incarnation, generation)` still the live holder? Locks the
/// lease row `FOR SHARE` for the rest of the transaction, so run it in the transaction that
/// records the intent or the receipt.
pub async fn check_fence(
    conn: &mut PgConnection,
    project: ProjectId,
    incarnation: IncarnationId,
    generation: i64,
) -> Result<(), FenceError> {
    let row = sqlx::query(
        "SELECT holder_incarnation, generation, expires_at > now() AS live
         FROM workspace_leases WHERE project_id = $1 FOR SHARE",
    )
    .bind(project.0)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Err(FenceError::NotHeld);
    };
    let holder: IncarnationId = row.try_get("holder_incarnation")?;
    let current: i64 = row.try_get("generation")?;
    let live: bool = row.try_get("live")?;
    if current != generation {
        return Err(FenceError::StaleGeneration {
            presented: generation,
            current,
        });
    }
    if holder != incarnation {
        return Err(FenceError::NotHeld);
    }
    if !live {
        return Err(FenceError::LeaseExpired);
    }
    Ok(())
}

/// The current lease, if any.
pub async fn current_lease(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<Option<(IncarnationId, Lease)>, TaskError> {
    let row = sqlx::query(
        "SELECT holder_incarnation, generation, expires_at FROM workspace_leases WHERE project_id = $1",
    )
    .bind(project.0)
    .fetch_optional(&mut *conn)
    .await?;
    row.map(|r| {
        Ok((
            r.try_get("holder_incarnation")?,
            Lease {
                generation: r.try_get("generation")?,
                expires_at: r.try_get("expires_at")?,
            },
        ))
    })
    .transpose()
}
