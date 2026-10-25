//! Startup recovery discovery. The restarting server does not know which accounts have work, and
//! every row-level policy needs a known account, so it calls the narrow SECURITY DEFINER function
//! `tasks_recovery_scan()` once (ids and states only), then opens one scoped transaction per
//! owner (`TenantRepo::begin`) for the actual reconciliation.

use factory_accounts::{AccountId, ProjectId};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Row};

use crate::enums::AttemptState;
use crate::error::TaskError;
use crate::ids::{AttemptId, TaskId};
use crate::rows::parse_col;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryItem {
    pub account_id: AccountId,
    pub project_id: ProjectId,
    pub task_id: TaskId,
    pub attempt_id: AttemptId,
    pub attempt_state: AttemptState,
}

/// Every nonterminal attempt across accounts, oldest first. Run on a plain connection: it needs
/// no tenant scope and returns no content.
pub async fn recovery_scan(conn: &mut PgConnection) -> Result<Vec<RecoveryItem>, TaskError> {
    let rows = sqlx::query(
        "SELECT account_id, project_id, task_id, attempt_id, attempt_state FROM tasks_recovery_scan()",
    )
    .fetch_all(&mut *conn)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(RecoveryItem {
                account_id: AccountId(r.try_get("account_id")?),
                project_id: ProjectId(r.try_get("project_id")?),
                task_id: r.try_get("task_id")?,
                attempt_id: r.try_get("attempt_id")?,
                attempt_state: parse_col(r, "attempt_state")?,
            })
        })
        .collect()
}
