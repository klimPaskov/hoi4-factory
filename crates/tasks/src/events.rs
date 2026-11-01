//! C-03 run events: append-only, ordered per task by a gapless host sequence.
//!
//! `append_event` takes the next number by updating `agent_tasks.last_seq` in the same
//! transaction as the insert. The row lock serializes concurrent appenders, and a rolled-back
//! transaction gives its number back, so a reader never sees a hole that later fills in. Every
//! state-changing operation in this crate calls it inside the transaction of the change itself
//! (ORCH-08), so neither half is visible alone.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};

use crate::canon::canonical_sha256;
use crate::enums::Actor;
use crate::error::TaskError;
use crate::ids::{AttemptId, EventId, TaskId};
use crate::rows::parse_col;

pub const EVENT_SCHEMA_VERSION: i32 = 1;
/// Policy version recorded when the caller names none.
pub const DEFAULT_POLICY_VERSION: &str = "policy-0";
pub const MAX_EVENTS_PER_READ: i64 = 1000;

/// What the caller decides about an event. The sequence, ID, hash and time are assigned here.
#[derive(Debug, Clone)]
pub struct EventDraft {
    pub attempt: Option<AttemptId>,
    pub actor: Actor,
    pub event_type: String,
    pub payload: Value,
    pub evidence_refs: Value,
    pub workspace_snapshot: Option<String>,
    pub policy_version: String,
}

impl EventDraft {
    pub fn new(actor: Actor, event_type: impl Into<String>, payload: Value) -> Self {
        Self {
            attempt: None,
            actor,
            event_type: event_type.into(),
            payload,
            evidence_refs: json!([]),
            workspace_snapshot: None,
            policy_version: DEFAULT_POLICY_VERSION.to_owned(),
        }
    }

    pub fn for_attempt(mut self, attempt: AttemptId) -> Self {
        self.attempt = Some(attempt);
        self
    }

    pub fn with_evidence(mut self, refs: Value) -> Self {
        self.evidence_refs = refs;
        self
    }

    pub fn with_snapshot(mut self, snapshot: impl Into<String>) -> Self {
        self.workspace_snapshot = Some(snapshot.into());
        self
    }

    pub fn with_policy(mut self, version: impl Into<String>) -> Self {
        self.policy_version = version.into();
        self
    }
}

/// A committed C-03 event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    pub task_id: TaskId,
    pub seq: i64,
    pub event_id: EventId,
    pub attempt_id: Option<AttemptId>,
    pub schema_version: i32,
    pub actor: Actor,
    pub event_type: String,
    pub payload: Value,
    pub payload_sha256: String,
    pub workspace_snapshot: Option<String>,
    pub policy_version: String,
    pub evidence_refs: Value,
    pub created_at: DateTime<Utc>,
}

impl RunEvent {
    /// True when the stored payload still matches its recorded hash.
    pub fn payload_intact(&self) -> bool {
        canonical_sha256(&self.payload) == self.payload_sha256
    }
}

/// A hole or reordering in a stream: the consumer should refresh from a snapshot (C-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("event stream gap: expected sequence {expected}, found {found}")]
pub struct Gap {
    pub expected: i64,
    pub found: i64,
}

/// Checks that `events` continue directly after `after_seq` with no gap, duplicate or reorder.
pub fn check_contiguous(after_seq: i64, events: &[RunEvent]) -> Result<(), Gap> {
    for (expected, event) in (after_seq + 1..).zip(events) {
        if event.seq != expected {
            return Err(Gap {
                expected,
                found: event.seq,
            });
        }
    }
    Ok(())
}

fn valid_event_type(t: &str) -> bool {
    let mut chars = t.chars();
    matches!(chars.next(), Some('a'..='z'))
        && t.len() <= 64
        && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.'))
}

pub(crate) const EVENT_COLUMNS: &str = "task_id, seq, event_id, attempt_id, schema_version, actor, event_type, \
     payload, payload_sha256, workspace_snapshot, policy_version, evidence_refs, created_at";

pub(crate) fn event_from_row(row: &sqlx::postgres::PgRow) -> Result<RunEvent, TaskError> {
    Ok(RunEvent {
        task_id: row.try_get("task_id")?,
        seq: row.try_get("seq")?,
        event_id: row.try_get("event_id")?,
        attempt_id: row.try_get("attempt_id")?,
        schema_version: row.try_get("schema_version")?,
        actor: parse_col(row, "actor")?,
        event_type: row.try_get("event_type")?,
        payload: row.try_get("payload")?,
        payload_sha256: row.try_get("payload_sha256")?,
        workspace_snapshot: row.try_get("workspace_snapshot")?,
        policy_version: row.try_get("policy_version")?,
        evidence_refs: row.try_get("evidence_refs")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Appends one event to the task's stream. Run it inside the transaction of the state change it
/// describes.
pub async fn append_event(
    conn: &mut PgConnection,
    task: TaskId,
    draft: EventDraft,
) -> Result<RunEvent, TaskError> {
    if !valid_event_type(&draft.event_type) {
        return Err(TaskError::Invalid(format!(
            "event type {:?} is not lowercase words with _ or .",
            draft.event_type
        )));
    }
    let slot = sqlx::query(
        "UPDATE agent_tasks SET last_seq = last_seq + 1 WHERE id = $1
         RETURNING project_id, owner_account_id, last_seq",
    )
    .bind(task)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(TaskError::NotFound)?;
    let project: uuid::Uuid = slot.try_get("project_id")?;
    let owner: uuid::Uuid = slot.try_get("owner_account_id")?;
    let seq: i64 = slot.try_get("last_seq")?;

    let event_id = EventId::new();
    let payload_sha256 = canonical_sha256(&draft.payload);
    let row = sqlx::query(&format!(
        "INSERT INTO agent_events (task_id, seq, project_id, owner_account_id, event_id, attempt_id,
             schema_version, actor, event_type, payload, payload_sha256, workspace_snapshot,
             policy_version, evidence_refs)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         RETURNING {EVENT_COLUMNS}"
    ))
    .bind(task)
    .bind(seq)
    .bind(project)
    .bind(owner)
    .bind(event_id)
    .bind(draft.attempt)
    .bind(EVENT_SCHEMA_VERSION)
    .bind(draft.actor.to_string())
    .bind(&draft.event_type)
    .bind(&draft.payload)
    .bind(&payload_sha256)
    .bind(&draft.workspace_snapshot)
    .bind(&draft.policy_version)
    .bind(&draft.evidence_refs)
    .fetch_one(&mut *conn)
    .await?;
    event_from_row(&row)
}

/// Events with `seq > after_seq`, oldest first, at most `limit` (1 to 1000).
pub async fn events_after(
    conn: &mut PgConnection,
    task: TaskId,
    after_seq: i64,
    limit: i64,
) -> Result<Vec<RunEvent>, TaskError> {
    let limit = limit.clamp(1, MAX_EVENTS_PER_READ);
    let rows = sqlx::query(&format!(
        "SELECT {EVENT_COLUMNS} FROM agent_events WHERE task_id = $1 AND seq > $2
         ORDER BY seq LIMIT $3"
    ))
    .bind(task)
    .bind(after_seq.max(0))
    .bind(limit)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(event_from_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: i64) -> RunEvent {
        RunEvent {
            task_id: TaskId::new(),
            seq,
            event_id: EventId::new(),
            attempt_id: None,
            schema_version: 1,
            actor: Actor::Host,
            event_type: "x".into(),
            payload: json!({}),
            payload_sha256: canonical_sha256(&json!({})),
            workspace_snapshot: None,
            policy_version: "p".into(),
            evidence_refs: json!([]),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn contiguous_and_gaps() {
        assert!(check_contiguous(0, &[]).is_ok());
        assert!(check_contiguous(0, &[event(1), event(2), event(3)]).is_ok());
        assert!(check_contiguous(5, &[event(6), event(7)]).is_ok());
        assert_eq!(
            check_contiguous(0, &[event(1), event(3)]),
            Err(Gap {
                expected: 2,
                found: 3
            })
        );
        assert_eq!(
            check_contiguous(2, &[event(4)]),
            Err(Gap {
                expected: 3,
                found: 4
            })
        );
        assert!(check_contiguous(0, &[event(1), event(1)]).is_err());
    }

    #[test]
    fn event_type_shape() {
        assert!(valid_event_type("task_created"));
        assert!(valid_event_type("tool.settled"));
        assert!(!valid_event_type("Task"));
        assert!(!valid_event_type(""));
        assert!(!valid_event_type("a b"));
    }
}
