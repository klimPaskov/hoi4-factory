//! Tasks: creation, expected-version transitions and the durable pause condition.
//!
//! Every update names the version it read (`UPDATE ... WHERE id = $1 AND version = $2`), so two
//! competing commands succeed exactly once and the loser gets `VersionConflict` with the current
//! version. Each accepted change writes its C-03 event in the same transaction.

use chrono::{DateTime, Utc};
use factory_accounts::{AccountId, ProjectId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};

use crate::enums::{Actor, TaskMode, TaskState};
use crate::error::{TaskError, is_scope_violation};
use crate::events::{EventDraft, append_event};
use crate::ids::TaskId;
use crate::rows::parse_col;

pub const MAX_OBJECTIVE_CHARS: usize = 4000;
pub const MAX_PAUSE_REASON_CHARS: usize = 1000;

/// A task as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub owner_account_id: AccountId,
    pub created_by_account_id: AccountId,
    pub objective: String,
    pub requirement_ids: Vec<String>,
    pub mode: TaskMode,
    pub allowed_paths: Vec<String>,
    pub state: TaskState,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub budget_micros: i64,
    pub max_turns: i32,
    pub model_choice: Value,
    pub version: i64,
    pub last_seq: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for [`create_task`].
#[derive(Debug, Clone)]
pub struct NewTask {
    pub project_id: ProjectId,
    pub owner_account_id: AccountId,
    pub created_by: AccountId,
    pub objective: String,
    pub requirement_ids: Vec<String>,
    pub mode: TaskMode,
    pub allowed_paths: Vec<String>,
    pub budget_micros: i64,
    pub max_turns: i32,
    pub model_choice: Value,
}

impl NewTask {
    /// Rejects a missing objective and contradictory owned-path constraints (SPEC-003: create
    /// task).
    pub fn validate(&self) -> Result<(), TaskError> {
        let invalid = |m: &str| Err(TaskError::Invalid(m.to_owned()));
        if self.objective.trim().is_empty() {
            return invalid("the objective is empty");
        }
        if self.objective.chars().count() > MAX_OBJECTIVE_CHARS {
            return invalid("the objective is too long");
        }
        match self.mode {
            TaskMode::ReadOnly if !self.allowed_paths.is_empty() => {
                return invalid("a read-only task cannot own paths");
            }
            TaskMode::Edit if self.allowed_paths.is_empty() => {
                return invalid("an edit task must name the paths it may write");
            }
            _ => {}
        }
        for path in &self.allowed_paths {
            validate_allowed_path(path)?;
        }
        if self.requirement_ids.iter().any(|r| r.trim().is_empty()) {
            return invalid("a requirement ID is empty");
        }
        if self.budget_micros < 0 {
            return invalid("the budget is negative");
        }
        if !(1..=1000).contains(&self.max_turns) {
            return invalid("max_turns must be between 1 and 1000");
        }
        Ok(())
    }
}

/// An owned path is a relative, forward-slash prefix inside the workspace: no root, drive,
/// backslash, empty or `.`/`..` segment.
pub fn validate_allowed_path(path: &str) -> Result<(), TaskError> {
    let bad = |why: &str| Err(TaskError::Invalid(format!("allowed path {path:?}: {why}")));
    if path.is_empty() || path.len() > 512 {
        return bad("empty or too long");
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') || path.contains('\0') {
        return bad("must be relative with forward slashes");
    }
    for segment in path.trim_end_matches('/').split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return bad("has an empty, '.' or '..' segment");
        }
    }
    Ok(())
}

const TASK_COLUMNS: &str = "id, project_id, owner_account_id, created_by_account_id, objective, \
     requirement_ids, mode, allowed_paths, state, paused, pause_reason, budget_micros, max_turns, \
     model_choice, version, last_seq, created_at, updated_at";

fn task_from_row(row: &PgRow) -> Result<Task, TaskError> {
    Ok(Task {
        id: row.try_get("id")?,
        project_id: ProjectId(row.try_get("project_id")?),
        owner_account_id: AccountId(row.try_get("owner_account_id")?),
        created_by_account_id: AccountId(row.try_get("created_by_account_id")?),
        objective: row.try_get("objective")?,
        requirement_ids: row.try_get("requirement_ids")?,
        mode: parse_col(row, "mode")?,
        allowed_paths: row.try_get("allowed_paths")?,
        state: parse_col(row, "state")?,
        paused: row.try_get("paused")?,
        pause_reason: row.try_get("pause_reason")?,
        budget_micros: row.try_get("budget_micros")?,
        max_turns: row.try_get("max_turns")?,
        model_choice: row.try_get("model_choice")?,
        version: row.try_get("version")?,
        last_seq: row.try_get("last_seq")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Creates a task in `planned` and writes its `task_created` event (sequence 1).
pub async fn create_task(conn: &mut PgConnection, new: NewTask) -> Result<Task, TaskError> {
    new.validate()?;
    let id = TaskId::new();
    let inserted = sqlx::query(&format!(
        "INSERT INTO agent_tasks (id, project_id, owner_account_id, created_by_account_id, objective,
             requirement_ids, mode, allowed_paths, budget_micros, max_turns, model_choice)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING {TASK_COLUMNS}"
    ))
    .bind(id)
    .bind(new.project_id.0)
    .bind(new.owner_account_id.0)
    .bind(new.created_by.0)
    .bind(&new.objective)
    .bind(&new.requirement_ids)
    .bind(new.mode.as_str())
    .bind(&new.allowed_paths)
    .bind(new.budget_micros)
    .bind(new.max_turns)
    .bind(&new.model_choice)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        if is_scope_violation(&e) {
            TaskError::NotFound
        } else {
            TaskError::Database(e)
        }
    })?;
    let task = task_from_row(&inserted)?;
    append_event(
        conn,
        id,
        EventDraft::new(
            Actor::user(new.created_by),
            "task_created",
            json!({
                "objective": new.objective,
                "requirement_ids": new.requirement_ids,
                "mode": new.mode,
                "allowed_paths": new.allowed_paths,
                "budget_micros": new.budget_micros,
                "max_turns": new.max_turns,
            }),
        ),
    )
    .await?;
    get_task(conn, task.id).await
}

pub async fn get_task(conn: &mut PgConnection, id: TaskId) -> Result<Task, TaskError> {
    let row = sqlx::query(&format!("SELECT {TASK_COLUMNS} FROM agent_tasks WHERE id = $1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(TaskError::NotFound)?;
    task_from_row(&row)
}

/// Tasks of a project the scope can see, newest first.
pub async fn list_tasks(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<Vec<Task>, TaskError> {
    let rows = sqlx::query(&format!(
        "SELECT {TASK_COLUMNS} FROM agent_tasks WHERE project_id = $1
         ORDER BY created_at DESC, id"
    ))
    .bind(project.0)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(task_from_row).collect()
}

/// Locks the task row and checks the caller's expected version.
pub(crate) async fn lock_task(
    conn: &mut PgConnection,
    id: TaskId,
    expected_version: i64,
) -> Result<Task, TaskError> {
    let row = sqlx::query(&format!(
        "SELECT {TASK_COLUMNS} FROM agent_tasks WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(TaskError::NotFound)?;
    let task = task_from_row(&row)?;
    if task.version != expected_version {
        return Err(TaskError::VersionConflict {
            current: task.version,
        });
    }
    Ok(task)
}

/// Moves a task along the explicit transition table. The draft's actor and event type describe
/// the cause; the event payload records `from`, `to` and the draft's own payload under `detail`.
///
/// Rules beyond the table: a paused task cannot enter `running`, and `completed` cannot be
/// decided by a worker actor (completion is the acceptance policy's call, not the model's).
pub async fn transition_task(
    conn: &mut PgConnection,
    id: TaskId,
    expected_version: i64,
    to: TaskState,
    draft: EventDraft,
) -> Result<Task, TaskError> {
    let task = lock_task(conn, id, expected_version).await?;
    if !task.state.can_transition_to(to) {
        return Err(TaskError::InvalidTransition {
            from: task.state,
            to,
        });
    }
    if to == TaskState::Running && task.paused {
        return Err(TaskError::Paused);
    }
    if to == TaskState::Completed && draft.actor.is_worker() {
        return Err(TaskError::CompletionNotByWorker);
    }
    let updated = sqlx::query(
        "UPDATE agent_tasks SET state = $3, version = version + 1, updated_at = now()
         WHERE id = $1 AND version = $2",
    )
    .bind(id)
    .bind(expected_version)
    .bind(to.as_str())
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(TaskError::VersionConflict {
            current: task.version + 1,
        });
    }
    let mut draft = draft;
    if draft.event_type.is_empty() {
        draft.event_type = format!("task_{to}");
    }
    draft.payload = json!({"from": task.state, "to": to, "detail": draft.payload});
    append_event(conn, id, draft).await?;
    get_task(conn, id).await
}

/// Sets or clears the durable pause condition with its own event. Pause survives restart, does
/// not cancel the task and does not change its state. A terminal task cannot be paused.
pub async fn set_paused(
    conn: &mut PgConnection,
    id: TaskId,
    expected_version: i64,
    paused: bool,
    reason: Option<&str>,
    actor: Actor,
) -> Result<Task, TaskError> {
    let task = lock_task(conn, id, expected_version).await?;
    if task.state.is_terminal() {
        return Err(TaskError::TaskTerminal { state: task.state });
    }
    if task.paused == paused {
        return Err(TaskError::Invalid(
            if paused {
                "the task is already paused"
            } else {
                "the task is not paused"
            }
            .to_owned(),
        ));
    }
    if reason.is_some_and(|r| r.chars().count() > MAX_PAUSE_REASON_CHARS) {
        return Err(TaskError::Invalid("the pause reason is too long".into()));
    }
    let reason = if paused { reason } else { None };
    let updated = sqlx::query(
        "UPDATE agent_tasks SET paused = $3, pause_reason = $4, version = version + 1,
             updated_at = now()
         WHERE id = $1 AND version = $2",
    )
    .bind(id)
    .bind(expected_version)
    .bind(paused)
    .bind(reason)
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(TaskError::VersionConflict {
            current: task.version + 1,
        });
    }
    append_event(
        conn,
        id,
        EventDraft::new(
            actor,
            if paused { "task_paused" } else { "task_resumed" },
            json!({"reason": reason, "state": task.state}),
        ),
    )
    .await?;
    get_task(conn, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> NewTask {
        NewTask {
            project_id: ProjectId::new(),
            owner_account_id: AccountId::new(),
            created_by: AccountId::new(),
            objective: "add a focus".into(),
            requirement_ids: vec!["REQ-001".into()],
            mode: TaskMode::Edit,
            allowed_paths: vec!["common/national_focus/".into()],
            budget_micros: 200_000,
            max_turns: 20,
            model_choice: json!({}),
        }
    }

    #[test]
    fn validation() {
        assert!(base().validate().is_ok());
        let mut t = base();
        t.objective = "  ".into();
        assert!(t.validate().is_err());
        let mut t = base();
        t.allowed_paths.clear();
        assert!(t.validate().is_err(), "edit without paths");
        let mut t = base();
        t.mode = TaskMode::ReadOnly;
        assert!(t.validate().is_err(), "read-only with paths");
        t.allowed_paths.clear();
        assert!(t.validate().is_ok());
        for bad in ["/abs", "a/../b", "a\\b", "C:/x", "", "a//b", "./a"] {
            let mut t = base();
            t.allowed_paths = vec![bad.into()];
            assert!(t.validate().is_err(), "{bad}");
        }
    }
}
