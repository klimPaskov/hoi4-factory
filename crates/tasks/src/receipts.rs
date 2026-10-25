//! C-04 tool intents and receipts: intent before the effect, the observed outcome after.
//!
//! Protocol, in the order the host runs it (one transaction each step):
//! 1. `record_intent` with the final normalized arguments, after the grant and fence checks in
//!    the same transaction. A duplicate idempotency key returns the existing intent.
//! 2. `mark_started` commits BEFORE the effect begins. An intent with no receipt is therefore
//!    known never to have started.
//! 3. The effect runs.
//! 4. `settle` records the observed outcome. If the process dies between 3 and 4 the receipt
//!    stays `started`; recovery calls `mark_unknown` with what it observed, and only an explicit
//!    `reconcile` (never a retry) moves it on.
//!
//! Receipt status moves forward only, enforced by an expected-status `UPDATE` here and by a
//! trigger in the database. Unknown effects default to non-replayable ([`replay_decision`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::canon::{canonical_sha256, is_sha256_hex};
use crate::context::ReceiptSummary;
use crate::enums::{Actor, AttemptState, EffectClass, ReceiptStatus, RetryClass};
use crate::error::{TaskError, is_scope_violation};
use crate::events::{EventDraft, append_event};
use crate::ids::{AttemptId, OperationId, TaskId};
use crate::rows::parse_col;

/// What an operation was authorized and recorded under. Any change invalidates an old replay
/// decision (ORCH-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayIdentity {
    pub workspace_id: String,
    pub root_identity: String,
    pub source_snapshot: String,
    pub tool: String,
    pub tool_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub operation_id: OperationId,
    pub task_id: TaskId,
    pub attempt_id: AttemptId,
    pub request_id: String,
    pub idempotency_key: String,
    pub replay: ReplayIdentity,
    pub schema_version: String,
    pub effect_class: EffectClass,
    pub args_sha256: String,
    pub request_artifact: Value,
    pub grant_id: Uuid,
    pub grant_epoch: i64,
    pub fencing_generation: i64,
    pub retry_class: RetryClass,
    pub reconciliation_method: String,
    pub observation_inputs: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub operation_id: OperationId,
    pub task_id: TaskId,
    pub status: ReceiptStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub result_artifact: Option<Value>,
    pub result_sha256: Option<String>,
    pub partial: bool,
    /// What recovery saw when it marked the effect unknown.
    pub observation: Option<Value>,
    /// How an unknown effect was decided.
    pub reconciliation: Option<Value>,
}

/// Input for [`record_intent`].
#[derive(Debug, Clone)]
pub struct NewIntent {
    pub task_id: TaskId,
    pub attempt_id: AttemptId,
    pub request_id: String,
    pub idempotency_key: String,
    pub replay: ReplayIdentity,
    pub schema_version: String,
    pub effect_class: EffectClass,
    /// Final normalized nonsecret arguments plus credential REFERENCES (never values).
    pub request_artifact: Value,
    pub grant_id: Uuid,
    pub grant_epoch: i64,
    pub fencing_generation: i64,
    pub retry_class: RetryClass,
    pub reconciliation_method: String,
    pub observation_inputs: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    /// The idempotency key was already recorded for this task. The existing operation is
    /// returned; no second operation exists. `same_args` says whether the new request's
    /// arguments hash to the same value.
    #[error("idempotency key already recorded as operation {}", existing.operation_id)]
    Duplicate {
        existing: Box<Intent>,
        same_args: bool,
    },
    #[error("the task is paused")]
    Paused,
    #[error("attempt is {state} and cannot start new effects")]
    AttemptNotActive { state: AttemptState },
    #[error("task or attempt not found")]
    NotFound,
    #[error("the request artifact holds what looks like a credential value at {path}")]
    SecretInArtifact { path: String },
    #[error("invalid intent: {0}")]
    Invalid(String),
    #[error("stored data is not understood: {0}")]
    Corrupt(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<TaskError> for IntentError {
    fn from(e: TaskError) -> Self {
        match e {
            TaskError::NotFound => IntentError::NotFound,
            TaskError::Database(e) => IntentError::Database(e),
            other => IntentError::Corrupt(other.to_string()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptError {
    #[error("operation not found")]
    NotFound,
    #[error("the operation was already started (status {current})")]
    AlreadyStarted { current: ReceiptStatus },
    /// The receipt is no longer in the status the caller expected: a stale or late result.
    #[error("receipt status is {current}, expected {expected}")]
    StatusConflict {
        current: ReceiptStatus,
        expected: ReceiptStatus,
    },
    #[error("receipt cannot move from {from} to {to}")]
    InvalidStatus {
        from: ReceiptStatus,
        to: ReceiptStatus,
    },
    #[error("the task is paused")]
    Paused,
    #[error("attempt is {state} and cannot start new effects")]
    AttemptNotActive { state: AttemptState },
    #[error("worker output cannot decide a reconciliation")]
    ReconcileByWorker,
    #[error("invalid receipt data: {0}")]
    Invalid(String),
    #[error("stored data is not understood: {0}")]
    Corrupt(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<TaskError> for ReceiptError {
    fn from(e: TaskError) -> Self {
        match e {
            TaskError::NotFound => ReceiptError::NotFound,
            TaskError::Database(e) => ReceiptError::Database(e),
            other => ReceiptError::Corrupt(other.to_string()),
        }
    }
}

pub(crate) const INTENT_COLUMNS: &str = "operation_id, task_id, attempt_id, request_id, idempotency_key, \
     tool, tool_version, schema_version, effect_class, args_sha256, request_artifact, workspace_id, \
     root_identity, source_snapshot, grant_id, grant_epoch, fencing_generation, retry_class, \
     reconciliation_method, observation_inputs, created_at";

pub(crate) const RECEIPT_COLUMNS: &str = "operation_id, task_id, status, started_at, finished_at, \
     before_sha256, after_sha256, result_artifact, result_sha256, partial, observation, reconciliation";

pub(crate) fn intent_from_row(row: &PgRow) -> Result<Intent, TaskError> {
    Ok(Intent {
        operation_id: row.try_get("operation_id")?,
        task_id: row.try_get("task_id")?,
        attempt_id: row.try_get("attempt_id")?,
        request_id: row.try_get("request_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        replay: ReplayIdentity {
            workspace_id: row.try_get("workspace_id")?,
            root_identity: row.try_get("root_identity")?,
            source_snapshot: row.try_get("source_snapshot")?,
            tool: row.try_get("tool")?,
            tool_version: row.try_get("tool_version")?,
        },
        schema_version: row.try_get("schema_version")?,
        effect_class: parse_col(row, "effect_class")?,
        args_sha256: row.try_get("args_sha256")?,
        request_artifact: row.try_get("request_artifact")?,
        grant_id: row.try_get("grant_id")?,
        grant_epoch: row.try_get("grant_epoch")?,
        fencing_generation: row.try_get("fencing_generation")?,
        retry_class: parse_col(row, "retry_class")?,
        reconciliation_method: row.try_get("reconciliation_method")?,
        observation_inputs: row.try_get("observation_inputs")?,
        created_at: row.try_get("created_at")?,
    })
}

pub(crate) fn receipt_from_row(row: &PgRow) -> Result<Receipt, TaskError> {
    Ok(Receipt {
        operation_id: row.try_get("operation_id")?,
        task_id: row.try_get("task_id")?,
        status: parse_col(row, "status")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
        before_sha256: row.try_get("before_sha256")?,
        after_sha256: row.try_get("after_sha256")?,
        result_artifact: row.try_get("result_artifact")?,
        result_sha256: row.try_get("result_sha256")?,
        partial: row.try_get("partial")?,
        observation: row.try_get("observation")?,
        reconciliation: row.try_get("reconciliation")?,
    })
}

/// Key names that must never hold a value in a request artifact. A name ending in `_ref` or
/// `_id` is a reference to a stored credential and is allowed. Defense in depth: the broker
/// builds the artifact from the tool schema, this catches a mistake before it is persisted.
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "client_secret",
    "api_key",
    "apikey",
    "authorization",
    "access_token",
    "refresh_token",
    "bearer",
    "private_key",
];

fn find_secret(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let lower = key.to_ascii_lowercase();
                let reference = lower.ends_with("_ref") || lower.ends_with("_id");
                let child = format!("{path}/{key}");
                if !reference && SECRET_KEYS.contains(&lower.as_str()) && !inner.is_null() {
                    return Some(child);
                }
                if let Some(found) = find_secret(inner, &child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| find_secret(v, &format!("{path}/{i}"))),
        _ => None,
    }
}

fn require_text(name: &str, value: &str, max: usize) -> Result<(), IntentError> {
    if value.is_empty() || value.len() > max {
        return Err(IntentError::Invalid(format!(
            "{name} must be 1 to {max} bytes"
        )));
    }
    Ok(())
}

/// Records the intent of one operation, before its effect. Returns `Duplicate` with the existing
/// intent when the task already recorded this idempotency key. Refused when the task is paused
/// or the attempt is not `admitted`/`running`.
pub async fn record_intent(
    conn: &mut PgConnection,
    new: NewIntent,
) -> Result<Intent, IntentError> {
    require_text("request_id", &new.request_id, 200)?;
    require_text("idempotency_key", &new.idempotency_key, 200)?;
    require_text("tool", &new.replay.tool, 200)?;
    require_text("tool_version", &new.replay.tool_version, 200)?;
    require_text("schema_version", &new.schema_version, 100)?;
    require_text("reconciliation_method", &new.reconciliation_method, 200)?;
    if let Some(path) = find_secret(&new.request_artifact, "") {
        return Err(IntentError::SecretInArtifact { path });
    }
    let args_sha256 = canonical_sha256(&new.request_artifact);

    // A retried request gets its original operation back, even while paused.
    let existing = sqlx::query(&format!(
        "SELECT {INTENT_COLUMNS} FROM tool_intents WHERE task_id = $1 AND idempotency_key = $2"
    ))
    .bind(new.task_id)
    .bind(&new.idempotency_key)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = existing {
        let existing = intent_from_row(&row)?;
        let same_args = existing.args_sha256 == args_sha256;
        return Err(IntentError::Duplicate {
            existing: Box::new(existing),
            same_args,
        });
    }

    let gate = sqlx::query(
        "SELECT a.state AS attempt_state, t.paused
         FROM agent_attempts a JOIN agent_tasks t ON t.id = a.task_id
         WHERE a.id = $1 AND a.task_id = $2",
    )
    .bind(new.attempt_id)
    .bind(new.task_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(IntentError::NotFound)?;
    let state: AttemptState = parse_col(&gate, "attempt_state")?;
    let paused: bool = gate.try_get("paused")?;
    if paused {
        return Err(IntentError::Paused);
    }
    if !matches!(state, AttemptState::Admitted | AttemptState::Running) {
        return Err(IntentError::AttemptNotActive { state });
    }

    let operation_id = OperationId::new();
    let inserted = sqlx::query(&format!(
        "INSERT INTO tool_intents (operation_id, task_id, attempt_id, project_id, owner_account_id,
             request_id, idempotency_key, tool, tool_version, schema_version, effect_class,
             args_sha256, request_artifact, workspace_id, root_identity, source_snapshot, grant_id,
             grant_epoch, fencing_generation, retry_class, reconciliation_method, observation_inputs)
         SELECT $1, t.id, $2, t.project_id, t.owner_account_id, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19
         FROM agent_tasks t WHERE t.id = $20
         ON CONFLICT (task_id, idempotency_key) DO NOTHING
         RETURNING {INTENT_COLUMNS}"
    ))
    .bind(operation_id)
    .bind(new.attempt_id)
    .bind(&new.request_id)
    .bind(&new.idempotency_key)
    .bind(&new.replay.tool)
    .bind(&new.replay.tool_version)
    .bind(&new.schema_version)
    .bind(new.effect_class.as_str())
    .bind(&args_sha256)
    .bind(&new.request_artifact)
    .bind(&new.replay.workspace_id)
    .bind(&new.replay.root_identity)
    .bind(&new.replay.source_snapshot)
    .bind(new.grant_id)
    .bind(new.grant_epoch)
    .bind(new.fencing_generation)
    .bind(new.retry_class.as_str())
    .bind(&new.reconciliation_method)
    .bind(&new.observation_inputs)
    .bind(new.task_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| {
        if is_scope_violation(&e) {
            IntentError::NotFound
        } else {
            IntentError::Database(e)
        }
    })?;
    let Some(row) = inserted else {
        // A concurrent request with the same key won between our check and our insert.
        let row = sqlx::query(&format!(
            "SELECT {INTENT_COLUMNS} FROM tool_intents WHERE task_id = $1 AND idempotency_key = $2"
        ))
        .bind(new.task_id)
        .bind(&new.idempotency_key)
        .fetch_one(&mut *conn)
        .await?;
        let existing = intent_from_row(&row)?;
        let same_args = existing.args_sha256 == args_sha256;
        return Err(IntentError::Duplicate {
            existing: Box::new(existing),
            same_args,
        });
    };
    let intent = intent_from_row(&row)?;
    append_event(
        conn,
        new.task_id,
        EventDraft::new(
            Actor::Host,
            "tool_intent",
            json!({
                "operation_id": intent.operation_id,
                "tool": intent.replay.tool,
                "tool_version": intent.replay.tool_version,
                "effect_class": intent.effect_class,
                "args_sha256": intent.args_sha256,
                "idempotency_key": intent.idempotency_key,
                "grant_id": intent.grant_id,
                "grant_epoch": intent.grant_epoch,
                "fencing_generation": intent.fencing_generation,
            }),
        )
        .for_attempt(new.attempt_id)
        .with_snapshot(intent.replay.source_snapshot.clone()),
    )
    .await?;
    Ok(intent)
}

pub async fn get_intent(conn: &mut PgConnection, op: OperationId) -> Result<Intent, TaskError> {
    let row = sqlx::query(&format!(
        "SELECT {INTENT_COLUMNS} FROM tool_intents WHERE operation_id = $1"
    ))
    .bind(op)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(TaskError::NotFound)?;
    intent_from_row(&row)
}

/// The receipt, or `None` when the operation never started.
pub async fn get_receipt(
    conn: &mut PgConnection,
    op: OperationId,
) -> Result<Option<Receipt>, TaskError> {
    let row = sqlx::query(&format!(
        "SELECT {RECEIPT_COLUMNS} FROM tool_receipts WHERE operation_id = $1"
    ))
    .bind(op)
    .fetch_optional(&mut *conn)
    .await?;
    row.as_ref().map(receipt_from_row).transpose()
}

/// Opens the receipt as `started`. Commit this BEFORE the effect begins.
pub async fn mark_started(
    conn: &mut PgConnection,
    op: OperationId,
) -> Result<Receipt, ReceiptError> {
    let gate = sqlx::query(
        "SELECT i.attempt_id, a.state AS attempt_state, t.paused
         FROM tool_intents i
         JOIN agent_attempts a ON a.id = i.attempt_id
         JOIN agent_tasks t ON t.id = i.task_id
         WHERE i.operation_id = $1",
    )
    .bind(op)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ReceiptError::NotFound)?;
    if let Some(current) = get_receipt(conn, op).await? {
        return Err(ReceiptError::AlreadyStarted {
            current: current.status,
        });
    }
    let state: AttemptState = parse_col(&gate, "attempt_state").map_err(ReceiptError::from)?;
    let attempt: AttemptId = gate.try_get("attempt_id")?;
    if gate.try_get::<bool, _>("paused")? {
        return Err(ReceiptError::Paused);
    }
    if !matches!(state, AttemptState::Admitted | AttemptState::Running) {
        return Err(ReceiptError::AttemptNotActive { state });
    }
    let row = sqlx::query(&format!(
        "INSERT INTO tool_receipts (operation_id, task_id, project_id, owner_account_id, status)
         SELECT i.operation_id, i.task_id, i.project_id, i.owner_account_id, 'started'
         FROM tool_intents i WHERE i.operation_id = $1
         RETURNING {RECEIPT_COLUMNS}"
    ))
    .bind(op)
    .fetch_one(&mut *conn)
    .await?;
    let receipt = receipt_from_row(&row)?;
    append_event(
        conn,
        receipt.task_id,
        EventDraft::new(
            Actor::Host,
            "tool_started",
            json!({"operation_id": op}),
        )
        .for_attempt(attempt),
    )
    .await?;
    Ok(receipt)
}

/// The observed outcome of an effect.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// One of `succeeded`, `unchanged`, `refused`, `failed`.
    pub status: ReceiptStatus,
    pub before: Option<String>,
    pub after: Option<String>,
    pub result_artifact: Option<Value>,
    /// The output was truncated or the effect only partly happened.
    pub partial: bool,
}

fn check_hash(name: &str, value: &Option<String>) -> Result<(), ReceiptError> {
    match value {
        Some(h) if !is_sha256_hex(h) => Err(ReceiptError::Invalid(format!(
            "{name} must be 64 lowercase hex digits"
        ))),
        _ => Ok(()),
    }
}

async fn attempt_of(conn: &mut PgConnection, op: OperationId) -> Result<AttemptId, ReceiptError> {
    sqlx::query_scalar::<_, AttemptId>("SELECT attempt_id FROM tool_intents WHERE operation_id = $1")
        .bind(op)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(ReceiptError::NotFound)
}

async fn status_conflict(
    conn: &mut PgConnection,
    op: OperationId,
    expected: ReceiptStatus,
) -> ReceiptError {
    match get_receipt(conn, op).await {
        Ok(Some(r)) => ReceiptError::StatusConflict {
            current: r.status,
            expected,
        },
        Ok(None) => ReceiptError::NotFound,
        Err(e) => e.into(),
    }
}

/// Records the observed outcome. `expected_status` is the status the caller last saw (normally
/// `started`); a receipt that moved on (recovery marked it unknown, say) refuses a late result
/// with `StatusConflict` instead of overwriting it.
pub async fn settle(
    conn: &mut PgConnection,
    op: OperationId,
    expected_status: ReceiptStatus,
    outcome: Outcome,
) -> Result<Receipt, ReceiptError> {
    if !matches!(
        outcome.status,
        ReceiptStatus::Succeeded
            | ReceiptStatus::Unchanged
            | ReceiptStatus::Refused
            | ReceiptStatus::Failed
    ) {
        return Err(ReceiptError::Invalid(format!(
            "settle records succeeded, unchanged, refused or failed, not {}",
            outcome.status
        )));
    }
    if !expected_status.can_transition_to(outcome.status) {
        return Err(ReceiptError::InvalidStatus {
            from: expected_status,
            to: outcome.status,
        });
    }
    check_hash("before", &outcome.before)?;
    check_hash("after", &outcome.after)?;
    let result_sha256 = outcome.result_artifact.as_ref().map(canonical_sha256);
    let attempt = attempt_of(conn, op).await?;
    let row = sqlx::query(&format!(
        "UPDATE tool_receipts SET status = $3, finished_at = now(), before_sha256 = $4,
             after_sha256 = $5, result_artifact = $6, result_sha256 = $7, partial = $8
         WHERE operation_id = $1 AND status = $2
         RETURNING {RECEIPT_COLUMNS}"
    ))
    .bind(op)
    .bind(expected_status.as_str())
    .bind(outcome.status.as_str())
    .bind(&outcome.before)
    .bind(&outcome.after)
    .bind(&outcome.result_artifact)
    .bind(&result_sha256)
    .bind(outcome.partial)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Err(status_conflict(conn, op, expected_status).await);
    };
    let receipt = receipt_from_row(&row)?;
    append_event(
        conn,
        receipt.task_id,
        EventDraft::new(
            Actor::Host,
            "tool_settled",
            json!({
                "operation_id": op,
                "status": receipt.status,
                "before_sha256": receipt.before_sha256,
                "after_sha256": receipt.after_sha256,
                "result_sha256": receipt.result_sha256,
                "partial": receipt.partial,
            }),
        )
        .for_attempt(attempt),
    )
    .await?;
    Ok(receipt)
}

/// Records that the effect may or may not have happened (process died, transport closed). The
/// observation is whatever recovery could see. Replay stays blocked until `reconcile`.
pub async fn mark_unknown(
    conn: &mut PgConnection,
    op: OperationId,
    observation: Value,
) -> Result<Receipt, ReceiptError> {
    let attempt = attempt_of(conn, op).await?;
    let row = sqlx::query(&format!(
        "UPDATE tool_receipts SET status = 'unknown_effect', observation = $2
         WHERE operation_id = $1 AND status = 'started'
         RETURNING {RECEIPT_COLUMNS}"
    ))
    .bind(op)
    .bind(&observation)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Err(status_conflict(conn, op, ReceiptStatus::Started).await);
    };
    let receipt = receipt_from_row(&row)?;
    append_event(
        conn,
        receipt.task_id,
        EventDraft::new(
            Actor::Host,
            "tool_unknown_effect",
            json!({"operation_id": op, "observation_sha256": canonical_sha256(&observation)}),
        )
        .for_attempt(attempt),
    )
    .await?;
    Ok(receipt)
}

/// Operations that started but have no established outcome (`started` or `unknown_effect`),
/// with their intents, oldest first.
pub async fn pending_for_task(
    conn: &mut PgConnection,
    task: TaskId,
) -> Result<Vec<(Intent, Receipt)>, TaskError> {
    let intents = sqlx::query(
        "SELECT i.operation_id FROM tool_intents i
         JOIN tool_receipts r ON r.operation_id = i.operation_id
         WHERE i.task_id = $1 AND r.status IN ('started', 'unknown_effect')
         ORDER BY r.started_at, i.operation_id",
    )
    .bind(task)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(intents.len());
    for row in intents {
        let op: OperationId = row.try_get("operation_id")?;
        let intent = get_intent(conn, op).await?;
        let receipt = get_receipt(conn, op).await?.ok_or(TaskError::NotFound)?;
        out.push((intent, receipt));
    }
    Ok(out)
}

/// Intents whose receipt was never opened: known not to have started. Oldest first.
pub async fn never_started_for_task(
    conn: &mut PgConnection,
    task: TaskId,
) -> Result<Vec<Intent>, TaskError> {
    let rows = sqlx::query(&format!(
        "SELECT {INTENT_COLUMNS} FROM tool_intents i
         WHERE i.task_id = $1
           AND NOT EXISTS (SELECT 1 FROM tool_receipts r WHERE r.operation_id = i.operation_id)
         ORDER BY i.created_at, i.operation_id"
    ))
    .bind(task)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(intent_from_row).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationDecision {
    /// The effect happened as intended.
    Applied,
    /// The effect did not happen.
    NotApplied,
    /// The target differs from both the before state and the intended after state.
    Conflict,
}

impl ReconciliationDecision {
    fn status(self) -> ReceiptStatus {
        match self {
            ReconciliationDecision::Applied => ReceiptStatus::ReconciledApplied,
            ReconciliationDecision::NotApplied => ReceiptStatus::ReconciledNotApplied,
            ReconciliationDecision::Conflict => ReceiptStatus::ReconciledConflict,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reconciliation {
    pub method: String,
    pub observation: Value,
    pub decision: ReconciliationDecision,
    /// The host or a user. Worker output cannot decide.
    pub decided_by: Actor,
    /// The target's hash as observed, when the method produced one.
    pub observed_after_sha256: Option<String>,
}

/// Decides an `unknown_effect` operation from independent observation. `reconciled_conflict`
/// counts as resolved for admission (a person decided what to do), and it is surfaced to the
/// next bundle through the receipt summaries.
pub async fn reconcile(
    conn: &mut PgConnection,
    op: OperationId,
    reconciliation: Reconciliation,
) -> Result<Receipt, ReceiptError> {
    if reconciliation.decided_by.is_worker() {
        return Err(ReceiptError::ReconcileByWorker);
    }
    if reconciliation.method.is_empty() || reconciliation.method.len() > 200 {
        return Err(ReceiptError::Invalid("method must be 1 to 200 bytes".into()));
    }
    check_hash("observed_after_sha256", &reconciliation.observed_after_sha256)?;
    let attempt = attempt_of(conn, op).await?;
    let to = reconciliation.decision.status();
    let record = json!({
        "method": reconciliation.method,
        "observation": reconciliation.observation,
        "decided_by": reconciliation.decided_by.to_string(),
    });
    let row = sqlx::query(&format!(
        "UPDATE tool_receipts SET status = $2, finished_at = now(),
             after_sha256 = COALESCE($3, after_sha256),
             reconciliation = $4::jsonb || jsonb_build_object('at', now())
         WHERE operation_id = $1 AND status = 'unknown_effect'
         RETURNING {RECEIPT_COLUMNS}"
    ))
    .bind(op)
    .bind(to.as_str())
    .bind(&reconciliation.observed_after_sha256)
    .bind(&record)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Err(status_conflict(conn, op, ReceiptStatus::UnknownEffect).await);
    };
    let receipt = receipt_from_row(&row)?;
    append_event(
        conn,
        receipt.task_id,
        EventDraft::new(
            reconciliation.decided_by,
            "tool_reconciled",
            json!({
                "operation_id": op,
                "status": receipt.status,
                "method": reconciliation.method,
                "observed_after_sha256": reconciliation.observed_after_sha256,
            }),
        )
        .for_attempt(attempt),
    )
    .await?;
    Ok(receipt)
}

/// Factual summaries for the next context bundle: operations with an established outcome
/// (`unresolved == false`) or operations still unresolved (`true`).
pub async fn receipt_summaries(
    conn: &mut PgConnection,
    task: TaskId,
    unresolved: bool,
) -> Result<Vec<ReceiptSummary>, TaskError> {
    let rows = sqlx::query(
        "SELECT i.operation_id, i.tool, i.args_sha256, r.status, r.before_sha256, r.after_sha256, r.partial
         FROM tool_intents i JOIN tool_receipts r ON r.operation_id = i.operation_id
         WHERE i.task_id = $1 AND (r.status IN ('started', 'unknown_effect')) = $2
         ORDER BY r.started_at, i.operation_id",
    )
    .bind(task)
    .bind(unresolved)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter()
        .map(|row| {
            let status: ReceiptStatus = parse_col(row, "status")?;
            let tool: String = row.try_get("tool")?;
            Ok(ReceiptSummary {
                operation_id: row.try_get("operation_id")?,
                summary: format!("{tool}: {status}"),
                tool,
                status,
                args_sha256: row.try_get("args_sha256")?,
                before_sha256: row.try_get("before_sha256")?,
                after_sha256: row.try_get("after_sha256")?,
                partial: row.try_get("partial")?,
            })
        })
        .collect()
}

/// What a recovery or retry may do with a recorded operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayDecision {
    /// The operation settled under this identity: use the recorded result, do not run it again.
    ReuseResult,
    /// Running it again cannot do harm: it has no effect, or its effect is idempotent and known
    /// not to have happened or to be observable.
    SafeToRepeat,
    /// The effect may have happened. Reconcile before any repeat.
    MustReconcile,
    /// The identity changed since the intent: the old decision no longer applies and the work
    /// needs a new attempt (ORCH-09). Names what changed.
    Rejected { changed: Vec<&'static str> },
}

impl Intent {
    pub fn replay_identity(&self) -> &ReplayIdentity {
        &self.replay
    }
}

/// The names of the identity fields that differ.
pub fn identity_changes(stored: &ReplayIdentity, current: &ReplayIdentity) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if stored.workspace_id != current.workspace_id {
        changed.push("workspace_id");
    }
    if stored.root_identity != current.root_identity {
        changed.push("root_identity");
    }
    if stored.source_snapshot != current.source_snapshot {
        changed.push("source_snapshot");
    }
    if stored.tool != current.tool {
        changed.push("tool");
    }
    if stored.tool_version != current.tool_version {
        changed.push("tool_version");
    }
    changed
}

/// Decides what may happen to a recorded operation. Pure. `receipt` is `None` when the
/// operation never started.
///
/// 1. Any changed workspace, root, source snapshot or tool version rejects the old decision.
/// 2. A settled operation's result is reused (`succeeded`, `unchanged`, `reconciled_applied`).
/// 3. A write whose effect is not established (`started`, `unknown_effect`, partial failure,
///    `reconciled_conflict`, or any failure of an unknown-effect tool) must be reconciled.
///    Pure reads and derived jobs have no effect to protect and may be repeated.
/// 4. A refusal or a reconciled non-application means nothing happened: safe to repeat.
pub fn replay_decision(
    intent: &Intent,
    receipt: Option<&Receipt>,
    current: &ReplayIdentity,
) -> ReplayDecision {
    let changed = identity_changes(&intent.replay, current);
    if !changed.is_empty() {
        return ReplayDecision::Rejected { changed };
    }
    let repeatable_by_nature = matches!(
        intent.effect_class,
        EffectClass::PureRead | EffectClass::DerivedJob
    );
    let Some(receipt) = receipt else {
        // Never started: the first execution is not a replay.
        return ReplayDecision::SafeToRepeat;
    };
    match receipt.status {
        ReceiptStatus::Succeeded | ReceiptStatus::Unchanged | ReceiptStatus::ReconciledApplied => {
            ReplayDecision::ReuseResult
        }
        ReceiptStatus::Refused | ReceiptStatus::ReconciledNotApplied => {
            ReplayDecision::SafeToRepeat
        }
        ReceiptStatus::Started | ReceiptStatus::UnknownEffect => {
            if repeatable_by_nature {
                ReplayDecision::SafeToRepeat
            } else {
                ReplayDecision::MustReconcile
            }
        }
        ReceiptStatus::Failed => {
            if repeatable_by_nature
                || (intent.effect_class == EffectClass::IdempotentWrite && !receipt.partial)
            {
                ReplayDecision::SafeToRepeat
            } else {
                ReplayDecision::MustReconcile
            }
        }
        ReceiptStatus::ReconciledConflict => ReplayDecision::MustReconcile,
    }
}
