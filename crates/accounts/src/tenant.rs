//! The tenant-scoped repository: the only API for projects and members.
//!
//! Every method opens a transaction and runs `set_config('app.account_id', <account>, true)`
//! first (the parameterisable form of `SET LOCAL`). PostgreSQL's row-level security policies on
//! `projects` and `project_members` key on that setting, so even a query written without a
//! filter cannot see another account's rows. The queries below also filter by account
//! explicitly, so they stay correct if a deployment connects with a role that bypasses RLS.
//!
//! A request for another account's private project is `not_found`. A member whose role lacks
//! the permission gets `forbidden`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::clock::Clock;
use crate::error::ApiError;
use crate::ids::{AccountId, ProjectId};
use crate::model::{Member, Permission, Project, Role, Visibility};
use crate::text::normalize_project_name;

const PROJECT_COLUMNS: &str = "p.id, p.owner_account_id, p.name, p.visibility, p.created_at, \
                               p.deleted_at, p.delete_after, m.role";

/// Repository bound to one account. Obtain it from `Database::scoped`.
#[derive(Clone)]
pub struct TenantRepo {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    account: AccountId,
    deletion_grace: Duration,
}

fn map_project(row: &PgRow) -> Result<Project, sqlx::Error> {
    let visibility: String = row.try_get("visibility")?;
    let role: Option<String> = row.try_get("role")?;
    Ok(Project {
        id: ProjectId(row.try_get::<Uuid, _>("id")?),
        owner_account_id: AccountId(row.try_get::<Uuid, _>("owner_account_id")?),
        name: row.try_get("name")?,
        visibility: Visibility::parse(&visibility)
            .ok_or_else(|| sqlx::Error::Decode("unknown visibility".into()))?,
        created_at: row.try_get("created_at")?,
        deleted_at: row.try_get("deleted_at")?,
        delete_after: row.try_get("delete_after")?,
        role: match role {
            Some(r) => {
                Some(Role::parse(&r).ok_or_else(|| sqlx::Error::Decode("unknown role".into()))?)
            }
            None => None,
        },
    })
}

fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23503"))
}

impl TenantRepo {
    pub(crate) fn new(pool: PgPool, clock: Arc<dyn Clock>, account: AccountId) -> Self {
        Self {
            pool,
            clock,
            account,
            deletion_grace: Duration::from_secs(30 * 24 * 60 * 60),
        }
    }

    /// Overrides the grace period applied by [`TenantRepo::mark_for_deletion`] (default 30 days).
    pub fn with_deletion_grace(mut self, grace: Duration) -> Self {
        self.deletion_grace = grace;
        self
    }

    pub fn account_id(&self) -> AccountId {
        self.account
    }

    /// Starts a transaction with `app.account_id` set for its whole life. Other crates use this
    /// to run queries on their own row-level-security tables under the same scope.
    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>, ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.account_id', $1, true)")
            .bind(self.account.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    async fn fetch(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        id: ProjectId,
    ) -> Result<Option<Project>, ApiError> {
        let sql = format!(
            "SELECT {PROJECT_COLUMNS}
             FROM projects p
             LEFT JOIN project_members m ON m.project_id = p.id AND m.account_id = $1
             WHERE p.id = $2 AND (m.account_id IS NOT NULL OR p.visibility = 'public')"
        );
        let row = sqlx::query(&sql)
            .bind(self.account.0)
            .bind(id.0)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(row.as_ref().map(map_project).transpose()?)
    }

    /// Loads a project the account can see and checks `permission` on it (when given).
    async fn load(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        id: ProjectId,
        permission: Option<Permission>,
    ) -> Result<Project, ApiError> {
        let project = self.fetch(tx, id).await?.ok_or_else(ApiError::not_found)?;
        if let Some(permission) = permission {
            project.require(permission)?;
        }
        Ok(project)
    }

    /// Creates a project owned by this account. The caller becomes its `owner` member.
    pub async fn create_project(
        &self,
        name: &str,
        visibility: Visibility,
    ) -> Result<Project, ApiError> {
        let name = normalize_project_name(name)?;
        let id = ProjectId::new();
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        sqlx::query(
            "INSERT INTO projects (id, owner_account_id, name, visibility, created_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id.0)
        .bind(self.account.0)
        .bind(&name)
        .bind(visibility.as_str())
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO project_members (project_id, account_id, project_owner_id, role, created_at)
             VALUES ($1, $2, $2, 'owner', $3)",
        )
        .bind(id.0)
        .bind(self.account.0)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Project {
            id,
            owner_account_id: self.account,
            name,
            visibility,
            created_at: now,
            deleted_at: None,
            delete_after: None,
            role: Some(Role::Owner),
        })
    }

    /// Projects this account is a member of, oldest first. Projects that are only public are
    /// not listed. Projects pending deletion are listed only when asked for.
    pub async fn list_projects(
        &self,
        include_pending_deletion: bool,
    ) -> Result<Vec<Project>, ApiError> {
        let mut tx = self.begin().await?;
        let sql = format!(
            "SELECT {PROJECT_COLUMNS}
             FROM projects p
             JOIN project_members m ON m.project_id = p.id AND m.account_id = $1
             WHERE ($2 OR p.deleted_at IS NULL)
             ORDER BY p.created_at, p.id"
        );
        let rows = sqlx::query(&sql)
            .bind(self.account.0)
            .bind(include_pending_deletion)
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(rows.iter().map(map_project).collect::<Result<_, _>>()?)
    }

    /// A project the account is a member of, or any public project (then `role` is `None`).
    /// Everything else, including projects that do not exist, is `not_found`.
    pub async fn get_project(&self, id: ProjectId) -> Result<Project, ApiError> {
        let mut tx = self.begin().await?;
        let project = self.load(&mut tx, id, None).await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Loads a project and checks that the account's role grants `permission`. This is the check
    /// later stages call before acting on a project's workspace.
    pub async fn authorize(
        &self,
        id: ProjectId,
        permission: Permission,
    ) -> Result<Project, ApiError> {
        let mut tx = self.begin().await?;
        let project = self.load(&mut tx, id, Some(permission)).await?;
        tx.commit().await?;
        Ok(project)
    }

    pub async fn rename_project(&self, id: ProjectId, name: &str) -> Result<Project, ApiError> {
        let name = normalize_project_name(name)?;
        let mut tx = self.begin().await?;
        let project = self
            .load(&mut tx, id, Some(Permission::RenameProject))
            .await?;
        self.ensure_not_pending(&project)?;
        let updated =
            sqlx::query("UPDATE projects SET name = $2 WHERE id = $1 AND deleted_at IS NULL")
                .bind(id.0)
                .bind(&name)
                .execute(&mut *tx)
                .await?;
        self.ensure_updated(updated.rows_affected(), id)?;
        let project = self.load(&mut tx, id, None).await?;
        tx.commit().await?;
        Ok(project)
    }

    pub async fn set_visibility(
        &self,
        id: ProjectId,
        visibility: Visibility,
    ) -> Result<Project, ApiError> {
        let mut tx = self.begin().await?;
        let project = self
            .load(&mut tx, id, Some(Permission::ChangeVisibility))
            .await?;
        self.ensure_not_pending(&project)?;
        let updated =
            sqlx::query("UPDATE projects SET visibility = $2 WHERE id = $1 AND deleted_at IS NULL")
                .bind(id.0)
                .bind(visibility.as_str())
                .execute(&mut *tx)
                .await?;
        self.ensure_updated(updated.rows_affected(), id)?;
        let project = self.load(&mut tx, id, None).await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Hides the project and schedules permanent removal after the grace period. Owner only.
    /// Marking an already marked project changes nothing.
    pub async fn mark_for_deletion(&self, id: ProjectId) -> Result<Project, ApiError> {
        let now = self.clock.now();
        let delete_after = now
            + chrono::Duration::from_std(self.deletion_grace).unwrap_or(chrono::Duration::days(30));
        let mut tx = self.begin().await?;
        let project = self
            .load(&mut tx, id, Some(Permission::MarkForDeletion))
            .await?;
        if project.pending_deletion() {
            tx.commit().await?;
            return Ok(project);
        }
        let updated = sqlx::query(
            "UPDATE projects SET deleted_at = $2, delete_after = $3
             WHERE id = $1 AND owner_account_id = $4 AND deleted_at IS NULL",
        )
        .bind(id.0)
        .bind(now)
        .bind(delete_after)
        .bind(self.account.0)
        .execute(&mut *tx)
        .await?;
        self.ensure_updated(updated.rows_affected(), id)?;
        let project = self.load(&mut tx, id, None).await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Cancels a pending deletion while the grace period lasts. Owner only.
    pub async fn restore_project(&self, id: ProjectId) -> Result<Project, ApiError> {
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        let project = self
            .load(&mut tx, id, Some(Permission::MarkForDeletion))
            .await?;
        if !project.pending_deletion() {
            tx.commit().await?;
            return Ok(project);
        }
        if project.delete_after.is_some_and(|t| t <= now) {
            return Err(ApiError::conflict(
                "grace_period_over",
                "The grace period for this project has ended.",
            )
            .with_id("project_id", id));
        }
        let updated = sqlx::query(
            "UPDATE projects SET deleted_at = NULL, delete_after = NULL
             WHERE id = $1 AND owner_account_id = $2",
        )
        .bind(id.0)
        .bind(self.account.0)
        .execute(&mut *tx)
        .await?;
        self.ensure_updated(updated.rows_affected(), id)?;
        let project = self.load(&mut tx, id, None).await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Adds a member, or changes an existing member's role. Owner only; the owner role itself
    /// cannot be assigned. The invitation flow that will call this arrives with collaboration.
    pub async fn add_member(
        &self,
        id: ProjectId,
        account: AccountId,
        role: Role,
    ) -> Result<Member, ApiError> {
        if role == Role::Owner {
            return Err(ApiError::invalid_request(
                "The owner role cannot be assigned.",
            ));
        }
        let now = self.clock.now();
        let mut tx = self.begin().await?;
        let project = self
            .load(&mut tx, id, Some(Permission::ManageMembers))
            .await?;
        if account == project.owner_account_id {
            return Err(ApiError::invalid_request(
                "The owner's role cannot be changed.",
            ));
        }
        let result = sqlx::query(
            "INSERT INTO project_members (project_id, account_id, project_owner_id, role, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (project_id, account_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(id.0)
        .bind(account.0)
        .bind(project.owner_account_id.0)
        .bind(role.as_str())
        .bind(now)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            Err(e) if is_foreign_key_violation(&e) => return Err(ApiError::not_found()),
            Err(e) => return Err(e.into()),
        }
        tx.commit().await?;
        Ok(Member {
            account_id: account,
            role,
            added_at: now,
        })
    }

    /// All members of a project. Owner only.
    pub async fn list_members(&self, id: ProjectId) -> Result<Vec<Member>, ApiError> {
        let mut tx = self.begin().await?;
        self.load(&mut tx, id, Some(Permission::ManageMembers))
            .await?;
        let rows = sqlx::query(
            "SELECT account_id, role, created_at FROM project_members WHERE project_id = $1 ORDER BY created_at, account_id",
        )
        .bind(id.0)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.iter()
            .map(|row| {
                let role: String = row.try_get("role")?;
                Ok(Member {
                    account_id: AccountId(row.try_get::<Uuid, _>("account_id")?),
                    role: Role::parse(&role)
                        .ok_or_else(|| sqlx::Error::Decode("unknown role".into()))?,
                    added_at: row.try_get::<DateTime<Utc>, _>("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(ApiError::from)
    }

    fn ensure_not_pending(&self, project: &Project) -> Result<(), ApiError> {
        if project.pending_deletion() {
            return Err(ApiError::conflict(
                "project_pending_deletion",
                "This project is marked for deletion. Restore it first.",
            )
            .with_id("project_id", project.id));
        }
        Ok(())
    }

    /// An UPDATE that matched nothing after the role check passed means the row policy refused
    /// it (the role changed in between) or the project changed state.
    fn ensure_updated(&self, rows: u64, id: ProjectId) -> Result<(), ApiError> {
        if rows == 0 {
            return Err(ApiError::forbidden(
                "Your role on this project does not allow that action.",
            )
            .with_id("project_id", id));
        }
        Ok(())
    }
}
