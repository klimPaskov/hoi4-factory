//! Attempts: one run of a task, admitted against a frozen context bundle and a grant.
//!
//! Single writer: a partial unique index allows one nonterminal attempt per task, and admission
//! also checks pause, task state, expected version and unresolved effects first so the caller
//! gets a typed reason instead of a constraint error.
//!
//! An attempt's success is not the task's completion. `transition_attempt(.., Succeeded, ..)`
//! never touches the task row; the host moves the task to `in_review`, and only acceptance
//! policy moves it on to `completed`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::canon::canonical_sha256;
use crate::enums::{Actor, AttemptState, Funding, TaskState};
use crate::error::{TaskError, is_unique_violation};
use crate::events::{EventDraft, append_event};
use crate::ids::{AttemptId, BundleId, IncarnationId, OperationId, TaskId};
use crate::rows::parse_col;
use crate::tasks::lock_task;

/// Largest conversation the ledger stores for one attempt.
pub const MAX_CONVERSATION_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub id: AttemptId,
    pub task_id: TaskId,
    pub attempt_no: i32,
    pub state: AttemptState,
    pub bundle_id: BundleId,
    pub grant_id: Uuid,
    pub grant_epoch: i64,
    pub host_incarnation: IncarnationId,
    pub fencing_generation: i64,
    pub model_profile: Value,
    pub funding: Funding,
    /// A reference to the user's stored key, never the key.
    pub user_key_id: Option<Uuid>,
    pub conversation: Value,
    pub outcome: Option<Value>,
    pub error_code: Option<String>,
    pub version: i64,
    pub admitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// What admission binds the attempt to.
#[derive(Debug, Clone)]
pub struct Admission {
    pub bundle_id: BundleId,
    pub grant_id: Uuid,
    pub grant_epoch: i64,
    pub incarnation: IncarnationId,
    pub generation: i64,
    pub model_profile: Value,
    pub funding: Funding,
    pub user_key_id: Option<Uuid>,
}

pub(crate) const ATTEMPT_COLUMNS: &str = "id, task_id, attempt_no, state, bundle_id, grant_id, grant_epoch, \
     host_incarnation, fencing_generation, model_profile, funding, user_key_id, conversation, \
     outcome, error_code, version, admitted_at, started_at, finished_at";

pub(crate) fn attempt_from_row(row: &PgRow) -> Result<Attempt, TaskError> {
    Ok(Attempt {
        id: row.try_get("id")?,
        task_id: row.try_get("task_id")?,
        attempt_no: row.try_get("attempt_no")?,
        state: parse_col(row, "state")?,
        bundle_id: row.try_get("bundle_id")?,
        grant_id: row.try_get("grant_id")?,
        grant_epoch: row.try_get("grant_epoch")?,
        host_incarnation: row.try_get("host_incarnation")?,
        fencing_generation: row.try_get("fencing_generation")?,
        model_profile: row.try_get("model_profile")?,
        funding: parse_col(row, "funding")?,
        user_key_id: row.try_get("user_key_id")?,
        conversation: row.try_get("conversation")?,
        outcome: row.try_get("outcome")?,
        error_code: row.try_get("error_code")?,
        version: row.try_get("version")?,
        admitted_at: row.try_get("admitted_at")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
    })
}

pub async fn get_attempt(conn: &mut PgConnection, id: AttemptId) -> Result<Attempt, TaskError> {
    let row = sqlx::query(&format!(
        "SELECT {ATTEMPT_COLUMNS} FROM agent_attempts WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(TaskError::NotFound)?;
    attempt_from_row(&row)
}

/// Attempts of a task, oldest first.
pub async fn list_attempts(
    conn: &mut PgConnection,
    task: TaskId,
) -> Result<Vec<Attempt>, TaskError> {
    let rows = sqlx::query(&format!(
        "SELECT {ATTEMPT_COLUMNS} FROM agent_attempts WHERE task_id = $1 ORDER BY attempt_no"
    ))
    .bind(task)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(attempt_from_row).collect()
}

/// Operations of the task whose effect is not established (receipt `started` or
/// `unknown_effect`), oldest first.
pub async fn unresolved_operations(
    conn: &mut PgConnection,
    task: TaskId,
) -> Result<Vec<OperationId>, TaskError> {
    let ids = sqlx::query_scalar::<_, OperationId>(
        "SELECT operation_id FROM tool_receipts
         WHERE task_id = $1 AND status IN ('started', 'unknown_effect')
         ORDER BY started_at, operation_id",
    )
    .bind(task)
    .fetch_all(&mut *conn)
    .await?;
    Ok(ids)
}

/// Admits a new attempt and moves the task to `running`, in one transaction with the
/// `attempt_admitted` event.
///
/// Refused, with a typed error, when: the expected task version is stale, the task is paused,
/// terminal or not in `ready`/`running`, it already has a nonterminal attempt, or any of its
/// operations has an unresolved effect (receipt `started` or `unknown_effect`).
pub async fn admit_attempt(
    conn: &mut PgConnection,
    task_id: TaskId,
    expected_task_version: i64,
    admission: Admission,
) -> Result<Attempt, TaskError> {
    if (admission.funding == Funding::UserKey) != admission.user_key_id.is_some() {
        return Err(TaskError::Invalid(
            "a user key ID is required exactly when the user's key pays".into(),
        ));
    }
    let task = lock_task(conn, task_id, expected_task_version).await?;
    if task.state.is_terminal() {
        return Err(TaskError::TaskTerminal { state: task.state });
    }
    if task.paused {
        return Err(TaskError::Paused);
    }
    if !matches!(task.state, TaskState::Ready | TaskState::Running) {
        return Err(TaskError::NotAdmissible { state: task.state });
    }
    let active = sqlx::query_scalar::<_, AttemptId>(
        "SELECT id FROM agent_attempts
         WHERE task_id = $1 AND state NOT IN ('succeeded', 'failed', 'cancelled')",
    )
    .bind(task_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(attempt) = active {
        return Err(TaskError::AttemptActive { attempt });
    }
    let operations = unresolved_operations(conn, task_id).await?;
    if !operations.is_empty() {
        return Err(TaskError::UnresolvedEffects { operations });
    }
    let bundle_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM context_bundles WHERE id = $1 AND task_id = $2)",
    )
    .bind(admission.bundle_id)
    .bind(task_id)
    .fetch_one(&mut *conn)
    .await?;
    if !bundle_ok {
        return Err(TaskError::BundleMismatch);
    }

    let id = AttemptId::new();
    let row = sqlx::query(&format!(
        "INSERT INTO agent_attempts (id, task_id, project_id, owner_account_id, attempt_no, state,
             bundle_id, grant_id, grant_epoch, host_incarnation, fencing_generation, model_profile,
             funding, user_key_id)
         SELECT $1, t.id, t.project_id, t.owner_account_id,
                COALESCE((SELECT max(attempt_no) FROM agent_attempts WHERE task_id = t.id), 0) + 1,
                'admitted', $2, $3, $4, $5, $6, $7, $8, $9
         FROM agent_tasks t WHERE t.id = $10
         RETURNING {ATTEMPT_COLUMNS}"
    ))
    .bind(id)
    .bind(admission.bundle_id)
    .bind(admission.grant_id)
    .bind(admission.grant_epoch)
    .bind(admission.incarnation)
    .bind(admission.generation)
    .bind(&admission.model_profile)
    .bind(admission.funding.as_str())
    .bind(admission.user_key_id)
    .bind(task_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            TaskError::Invalid("a competing attempt was admitted".into())
        } else {
            TaskError::Database(e)
        }
    })?;
    let attempt = attempt_from_row(&row)?;

    sqlx::query(
        "UPDATE agent_tasks SET state = 'running', version = version + 1, updated_at = now()
         WHERE id = $1 AND version = $2",
    )
    .bind(task_id)
    .bind(expected_task_version)
    .execute(&mut *conn)
    .await?;
    append_event(
        conn,
        task_id,
        EventDraft::new(
            Actor::Host,
            "attempt_admitted",
            json!({
                "attempt_no": attempt.attempt_no,
                "bundle_id": attempt.bundle_id,
                "grant_id": attempt.grant_id,
                "grant_epoch": attempt.grant_epoch,
                "fencing_generation": attempt.fencing_generation,
                "funding": attempt.funding,
                "task_state_from": task.state,
            }),
        )
        .for_attempt(id),
    )
    .await?;
    Ok(attempt)
}

/// Locks an attempt and checks the expected version.
async fn lock_attempt(
    conn: &mut PgConnection,
    id: AttemptId,
    expected_version: i64,
) -> Result<Attempt, TaskError> {
    let row = sqlx::query(&format!(
        "SELECT {ATTEMPT_COLUMNS} FROM agent_attempts WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(TaskError::NotFound)?;
    let attempt = attempt_from_row(&row)?;
    if attempt.version != expected_version {
        return Err(TaskError::VersionConflict {
            current: attempt.version,
        });
    }
    Ok(attempt)
}

/// Moves an attempt along the allowed-transition table and writes `attempt_<state>`. A terminal
/// attempt never changes. This does not change the task.
pub async fn transition_attempt(
    conn: &mut PgConnection,
    id: AttemptId,
    expected_version: i64,
    to: AttemptState,
    outcome: Option<Value>,
    error_code: Option<&str>,
    actor: Actor,
) -> Result<Attempt, TaskError> {
    let attempt = lock_attempt(conn, id, expected_version).await?;
    if !attempt.state.can_transition_to(to) {
        return Err(TaskError::InvalidAttemptTransition {
            from: attempt.state,
            to,
        });
    }
    let outcome = outcome.or(attempt.outcome.clone());
    let updated = sqlx::query(
        "UPDATE agent_attempts SET state = $3, outcome = $4, error_code = $5, version = version + 1,
             started_at = CASE WHEN $3 = 'running' THEN now() ELSE started_at END,
             finished_at = CASE WHEN $3 IN ('succeeded', 'failed', 'cancelled') THEN now()
                                ELSE finished_at END
         WHERE id = $1 AND version = $2",
    )
    .bind(id)
    .bind(expected_version)
    .bind(to.as_str())
    .bind(&outcome)
    .bind(error_code)
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(TaskError::VersionConflict {
            current: attempt.version + 1,
        });
    }
    append_event(
        conn,
        attempt.task_id,
        EventDraft::new(
            actor,
            format!("attempt_{to}"),
            json!({
                "from": attempt.state,
                "to": to,
                "attempt_no": attempt.attempt_no,
                "error_code": error_code,
                "outcome_sha256": outcome.as_ref().map(canonical_sha256),
            }),
        )
        .for_attempt(id),
    )
    .await?;
    get_attempt(conn, id).await
}

/// Persists the agent loop's conversation. Subordinate state: it is stored with the journal but
/// never approves or repeats an effect. Writes no event (it changes every turn); bumps the
/// attempt version, so a stale writer (an old worker generation) loses.
pub async fn save_conversation(
    conn: &mut PgConnection,
    id: AttemptId,
    expected_version: i64,
    conversation: &Value,
) -> Result<Attempt, TaskError> {
    if conversation.to_string().len() > MAX_CONVERSATION_BYTES {
        return Err(TaskError::Invalid("the conversation is too large".into()));
    }
    let updated = sqlx::query(
        "UPDATE agent_attempts SET conversation = $3, version = version + 1
         WHERE id = $1 AND version = $2
           AND state NOT IN ('succeeded', 'failed', 'cancelled')",
    )
    .bind(id)
    .bind(expected_version)
    .bind(conversation)
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        // Distinguish a missing attempt, a finished one and a stale version.
        let attempt = get_attempt(conn, id).await?;
        if attempt.state.is_terminal() {
            return Err(TaskError::InvalidAttemptTransition {
                from: attempt.state,
                to: attempt.state,
            });
        }
        return Err(TaskError::VersionConflict {
            current: attempt.version,
        });
    }
    get_attempt(conn, id).await
}
