//! C-08 approval grants.
//!
//! A grant records what an agent run may do: operation classes, path roots inside the task's own
//! worktree, provider destinations, limits, an expiry and a revocation epoch. The grant never
//! exceeds the grantor's project role (EXT-16): [`issue_grant`] refuses a class whose required
//! [`Permission`] the role lacks, and the insert policy of `agent_grants` checks that the claimed
//! role matches the grantor's membership.
//!
//! Every function takes a transaction the caller opened with `TenantRepo::begin`, so the host can
//! combine a grant check, a fence check and an intent insert in one transaction.
//! [`check_for_effect`] reads the row `FOR SHARE`: a concurrent revocation (an `UPDATE`) waits for
//! the effect's transaction to end, and an effect that starts after the revocation commits sees
//! the new epoch and is refused. Revocation increments the epoch durably, so a stale worker
//! cannot regain authority by replaying an earlier request.
//!
//! Publication classes are named and refused by role here, but S03 never issues them.

use chrono::{DateTime, Utc};
use factory_accounts::{AccountId, Permission, ProjectId, Role};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::ids::GrantId;

/// What an agent may be allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    ReadSource,
    RunReadOnlyTool,
    /// Writes inside the task's own Git worktree.
    WriteTaskWorktree,
    ModelInference,
    // Publication classes: named and refused. S03 never issues them.
    LocalCommit,
    PushOwnedBranch,
    OpenPullRequest,
    CreateRepository,
    IntegrateDefaultBranch,
}

impl OperationClass {
    pub const ALL: [OperationClass; 9] = [
        OperationClass::ReadSource,
        OperationClass::RunReadOnlyTool,
        OperationClass::WriteTaskWorktree,
        OperationClass::ModelInference,
        OperationClass::LocalCommit,
        OperationClass::PushOwnedBranch,
        OperationClass::OpenPullRequest,
        OperationClass::CreateRepository,
        OperationClass::IntegrateDefaultBranch,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            OperationClass::ReadSource => "read_source",
            OperationClass::RunReadOnlyTool => "run_read_only_tool",
            OperationClass::WriteTaskWorktree => "write_task_worktree",
            OperationClass::ModelInference => "model_inference",
            OperationClass::LocalCommit => "local_commit",
            OperationClass::PushOwnedBranch => "push_owned_branch",
            OperationClass::OpenPullRequest => "open_pull_request",
            OperationClass::CreateRepository => "create_repository",
            OperationClass::IntegrateDefaultBranch => "integrate_default_branch",
        }
    }

    pub fn parse(value: &str) -> Option<OperationClass> {
        OperationClass::ALL
            .into_iter()
            .find(|c| c.as_str() == value)
    }

    /// True for the classes that publish work beyond the task worktree.
    pub fn is_publication(self) -> bool {
        matches!(
            self,
            OperationClass::LocalCommit
                | OperationClass::PushOwnedBranch
                | OperationClass::OpenPullRequest
                | OperationClass::CreateRepository
                | OperationClass::IntegrateDefaultBranch
        )
    }

    /// The project permission a grantor needs to hand out this class.
    pub fn required_permission(self) -> Permission {
        match self {
            OperationClass::ReadSource | OperationClass::RunReadOnlyTool => Permission::ReadProject,
            OperationClass::WriteTaskWorktree | OperationClass::ModelInference => {
                Permission::CreateTask
            }
            OperationClass::LocalCommit
            | OperationClass::PushOwnedBranch
            | OperationClass::OpenPullRequest
            | OperationClass::CreateRepository => Permission::Publish,
            OperationClass::IntegrateDefaultBranch => Permission::ApproveProposal,
        }
    }
}

/// What the grantor asks for. The grantor's role comes from the caller (the host read it from
/// the membership); the database re-checks that it matches.
#[derive(Debug, Clone)]
pub struct NewGrant {
    pub project_id: ProjectId,
    pub owner_account_id: AccountId,
    pub grantor_account_id: AccountId,
    pub task_id: Uuid,
    pub operation_classes: Vec<OperationClass>,
    pub allowed_roots: Vec<String>,
    pub provider_destinations: Vec<String>,
    pub max_cost_micros: i64,
    pub max_turns: i32,
    pub max_changed_files: i32,
    pub expires_at: DateTime<Utc>,
    pub policy_version: String,
    /// `Some` binds the grant to one proposal hash; `None` authorizes a plan class.
    pub proposal_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Grant {
    pub id: GrantId,
    pub project_id: ProjectId,
    pub owner_account_id: AccountId,
    pub grantor_account_id: AccountId,
    pub grantor_role: Role,
    pub task_id: Uuid,
    pub operation_classes: Vec<OperationClass>,
    pub allowed_roots: Vec<String>,
    pub provider_destinations: Vec<String>,
    pub max_cost_micros: i64,
    pub max_turns: i32,
    pub max_changed_files: i32,
    pub expires_at: DateTime<Utc>,
    pub policy_version: String,
    pub proposal_sha256: Option<String>,
    pub revocation_epoch: i64,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<AccountId>,
    pub created_at: DateTime<Utc>,
}

impl Grant {
    /// True when the grant authorizes one proposal hash rather than a plan class.
    pub fn is_proposal_bound(&self) -> bool {
        self.proposal_sha256.is_some()
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GrantError {
    #[error("role {role:?} does not allow granting {class:?}")]
    ExceedsRole { class: OperationClass, role: Role },
    #[error("invalid grant: {0}")]
    Invalid(String),
    #[error("grant not found")]
    NotFound,
    #[error("only the grantor or the project owner may revoke a grant")]
    NotPermitted,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

const MAX_ROOT_COMPONENTS_BYTES: usize = 1024;
const MAX_DESTINATION_BYTES: usize = 512;

fn normalize_roots(roots: &[String]) -> Result<Vec<String>, GrantError> {
    if roots.len() > 64 {
        return Err(GrantError::Invalid("too many allowed roots".into()));
    }
    let mut out = Vec::with_capacity(roots.len());
    for root in roots {
        let trimmed = root.trim_end_matches('/');
        if trimmed.len() > MAX_ROOT_COMPONENTS_BYTES {
            return Err(GrantError::Invalid("allowed root is too long".into()));
        }
        factory_workspace::validate_relative(trimmed)
            .map_err(|e| GrantError::Invalid(format!("allowed root {root:?}: {e}")))?;
        out.push(trimmed.to_owned());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn normalize_destinations(destinations: &[String]) -> Result<Vec<String>, GrantError> {
    if destinations.len() > 16 {
        return Err(GrantError::Invalid("too many provider destinations".into()));
    }
    let mut out = Vec::with_capacity(destinations.len());
    for d in destinations {
        let ok = !d.is_empty()
            && d.len() <= MAX_DESTINATION_BYTES
            && d.contains(':')
            && !d.starts_with(':')
            && !d.chars().any(|c| c.is_control() || c.is_whitespace());
        if !ok {
            return Err(GrantError::Invalid(format!(
                "provider destination {d:?} must look like provider:url"
            )));
        }
        out.push(d.clone());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn map_grant(row: &PgRow) -> Result<Grant, sqlx::Error> {
    let decode = |what: &str| sqlx::Error::Decode(format!("unknown {what}").into());
    let role: String = row.try_get("grantor_role")?;
    let classes: Vec<String> = row.try_get("operation_classes")?;
    Ok(Grant {
        id: GrantId(row.try_get("id")?),
        project_id: ProjectId(row.try_get("project_id")?),
        owner_account_id: AccountId(row.try_get("owner_account_id")?),
        grantor_account_id: AccountId(row.try_get("grantor_account_id")?),
        grantor_role: Role::parse(&role).ok_or_else(|| decode("role"))?,
        task_id: row.try_get("task_id")?,
        operation_classes: classes
            .iter()
            .map(|c| OperationClass::parse(c).ok_or_else(|| decode("operation class")))
            .collect::<Result<_, _>>()?,
        allowed_roots: row.try_get("allowed_roots")?,
        provider_destinations: row.try_get("provider_destinations")?,
        max_cost_micros: row.try_get("max_cost_micros")?,
        max_turns: row.try_get("max_turns")?,
        max_changed_files: row.try_get("max_changed_files")?,
        expires_at: row.try_get("expires_at")?,
        policy_version: row.try_get("policy_version")?,
        proposal_sha256: row.try_get("proposal_sha256")?,
        revocation_epoch: row.try_get("revocation_epoch")?,
        revoked_at: row.try_get("revoked_at")?,
        revoked_by: row.try_get::<Option<Uuid>, _>("revoked_by")?.map(AccountId),
        created_at: row.try_get("created_at")?,
    })
}

const COLUMNS: &str = "id, project_id, owner_account_id, grantor_account_id, grantor_role, \
    task_id, operation_classes, allowed_roots, provider_destinations, max_cost_micros, \
    max_turns, max_changed_files, expires_at, policy_version, proposal_sha256, \
    revocation_epoch, revoked_at, revoked_by, created_at";

/// Issues a grant. Refuses a class the grantor's role does not allow, an expiry in the past and
/// malformed roots, destinations or proposal hash.
pub async fn issue_grant(
    conn: &mut PgConnection,
    new: NewGrant,
    grantor_role: Role,
    now: DateTime<Utc>,
) -> Result<Grant, GrantError> {
    if new.operation_classes.is_empty() {
        return Err(GrantError::Invalid("no operation class".into()));
    }
    for class in &new.operation_classes {
        if !grantor_role.allows(class.required_permission()) {
            return Err(GrantError::ExceedsRole {
                class: *class,
                role: grantor_role,
            });
        }
    }
    if new.expires_at <= now {
        return Err(GrantError::Invalid("the grant is already expired".into()));
    }
    if new.max_cost_micros < 0 || new.max_turns < 0 || new.max_changed_files < 0 {
        return Err(GrantError::Invalid("limits cannot be negative".into()));
    }
    let policy_version = new.policy_version.trim();
    if policy_version.is_empty() || policy_version.len() > 64 {
        return Err(GrantError::Invalid(
            "policy version must be 1 to 64 characters".into(),
        ));
    }
    if let Some(hash) = &new.proposal_sha256
        && !(hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
    {
        return Err(GrantError::Invalid(
            "proposal hash must be 64 lowercase hex characters".into(),
        ));
    }
    let roots = normalize_roots(&new.allowed_roots)?;
    let destinations = normalize_destinations(&new.provider_destinations)?;
    let mut classes: Vec<&'static str> = new.operation_classes.iter().map(|c| c.as_str()).collect();
    classes.sort_unstable();
    classes.dedup();

    let id = GrantId::new();
    let sql = format!(
        "INSERT INTO agent_grants (id, project_id, owner_account_id, grantor_account_id, \
         grantor_role, task_id, operation_classes, allowed_roots, provider_destinations, \
         max_cost_micros, max_turns, max_changed_files, expires_at, policy_version, \
         proposal_sha256, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING {COLUMNS}"
    );
    let row = sqlx::query(&sql)
        .bind(id.0)
        .bind(new.project_id.0)
        .bind(new.owner_account_id.0)
        .bind(new.grantor_account_id.0)
        .bind(grantor_role.as_str())
        .bind(new.task_id)
        .bind(&classes)
        .bind(&roots)
        .bind(&destinations)
        .bind(new.max_cost_micros)
        .bind(new.max_turns)
        .bind(new.max_changed_files)
        .bind(new.expires_at)
        .bind(policy_version)
        .bind(&new.proposal_sha256)
        .bind(now)
        .fetch_one(&mut *conn)
        .await?;
    Ok(map_grant(&row)?)
}

pub async fn get_grant(conn: &mut PgConnection, id: GrantId) -> Result<Option<Grant>, GrantError> {
    let sql = format!("SELECT {COLUMNS} FROM agent_grants WHERE id = $1");
    let row = sqlx::query(&sql)
        .bind(id.0)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(row.as_ref().map(map_grant).transpose()?)
}

/// Revokes a grant and increments its epoch. Only the grantor or the project owner may revoke.
/// Idempotent: revoking a revoked grant returns it unchanged.
pub async fn revoke_grant(
    conn: &mut PgConnection,
    id: GrantId,
    by: AccountId,
    now: DateTime<Utc>,
) -> Result<Grant, GrantError> {
    let current = get_grant(conn, id).await?.ok_or(GrantError::NotFound)?;
    if current.is_revoked() {
        return Ok(current);
    }
    if by != current.grantor_account_id && by != current.owner_account_id {
        return Err(GrantError::NotPermitted);
    }
    let sql = format!(
        "UPDATE agent_grants SET revocation_epoch = revocation_epoch + 1, revoked_at = $2, \
         revoked_by = $3 WHERE id = $1 AND revoked_at IS NULL RETURNING {COLUMNS}"
    );
    let row = sqlx::query(&sql)
        .bind(id.0)
        .bind(now)
        .bind(by.0)
        .fetch_optional(&mut *conn)
        .await?;
    match row {
        Some(row) => Ok(map_grant(&row)?),
        // Another transaction revoked it while this one waited for the row lock.
        None => get_grant(conn, id).await?.ok_or(GrantError::NotFound),
    }
}

/// The grant as it stood when an effect was checked.
#[derive(Debug, Clone)]
pub struct GrantSnapshot {
    pub grant: Grant,
    pub class: OperationClass,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum GrantRefusal {
    #[error("grant not found")]
    NotFound,
    #[error("grant was revoked")]
    Revoked,
    #[error("stale revocation epoch: presented {presented}, current {current}")]
    StaleEpoch { presented: i64, current: i64 },
    #[error("grant expired")]
    Expired,
    #[error("operation class {0:?} is not granted")]
    ClassNotGranted(OperationClass),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The check made immediately before an effect. Reads the grant row `FOR SHARE` so that, inside
/// the caller's transaction, a concurrent revocation cannot slip between the check and the
/// effect's own writes: the revoker waits for this transaction, or this check sees the revoked
/// row.
pub async fn check_for_effect(
    conn: &mut PgConnection,
    id: GrantId,
    presented_epoch: i64,
    class: OperationClass,
    now: DateTime<Utc>,
) -> Result<GrantSnapshot, GrantRefusal> {
    let sql = format!("SELECT {COLUMNS} FROM agent_grants WHERE id = $1 FOR SHARE");
    let row = sqlx::query(&sql)
        .bind(id.0)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(GrantRefusal::NotFound)?;
    let grant = map_grant(&row)?;
    if grant.is_revoked() {
        return Err(GrantRefusal::Revoked);
    }
    if presented_epoch != grant.revocation_epoch {
        return Err(GrantRefusal::StaleEpoch {
            presented: presented_epoch,
            current: grant.revocation_epoch,
        });
    }
    if grant.expires_at <= now {
        return Err(GrantRefusal::Expired);
    }
    if !grant.operation_classes.contains(&class) {
        return Err(GrantRefusal::ClassNotGranted(class));
    }
    Ok(GrantSnapshot {
        grant,
        class,
        checked_at: now,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathRefusal {
    #[error("path {path:?} is not acceptable: {reason}")]
    Invalid { path: String, reason: String },
    #[error("path {path:?} is outside the granted roots")]
    OutsideRoots { path: String },
}

/// Pure check on a snapshot: every path must be a valid workspace-relative path and must equal
/// or sit under an allowed root on a component boundary (`common` does not authorize
/// `common2/x`). Comparison is exact, so case variations do not match. No roots means no path.
pub fn authorize_paths(snapshot: &GrantSnapshot, paths: &[&str]) -> Result<(), PathRefusal> {
    let roots: Vec<Vec<&str>> = snapshot
        .grant
        .allowed_roots
        .iter()
        .filter_map(|r| factory_workspace::validate_relative(r).ok())
        .collect();
    for path in paths {
        let components =
            factory_workspace::validate_relative(path).map_err(|e| PathRefusal::Invalid {
                path: (*path).to_owned(),
                reason: e.to_string(),
            })?;
        let inside = roots
            .iter()
            .any(|root| components.len() >= root.len() && components[..root.len()] == root[..]);
        if !inside {
            return Err(PathRefusal::OutsideRoots {
                path: (*path).to_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("destination {destination:?} is not granted")]
pub struct DestinationRefusal {
    pub destination: String,
}

/// Pure check on a snapshot: the destination string must be listed exactly.
pub fn authorize_destination(
    snapshot: &GrantSnapshot,
    destination: &str,
) -> Result<(), DestinationRefusal> {
    if snapshot
        .grant
        .provider_destinations
        .iter()
        .any(|d| d == destination)
    {
        Ok(())
    } else {
        Err(DestinationRefusal {
            destination: destination.to_owned(),
        })
    }
}
