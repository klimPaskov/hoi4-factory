//! Rows of migration 0800: focus projections and C-02 change proposals.

use std::collections::BTreeMap;

use factory_accounts::{AccountId, ProjectId};
use factory_tasks::{AttemptId, Task, TaskId};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use super::projection::Built;

/// The task a focus operation belongs to, and the account whose row-level security scope its
/// transactions use (the person, or the account that started the agent's attempt).
#[derive(Debug, Clone, Copy)]
pub struct TaskKeys {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub owner_account_id: AccountId,
    pub acting: AccountId,
}

impl TaskKeys {
    pub fn of(task: &Task, acting: AccountId) -> Self {
        Self { id: task.id, project_id: task.project_id, owner_account_id: task.owner_account_id, acting }
    }
}

pub async fn insert_projection(conn: &mut PgConnection, task: &TaskKeys, built: &Built) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO focus_projections (id, task_id, project_id, owner_account_id, tree_id, relative_path,
             tool_name, tool_version, source_hashes, projection_hash, projection, plan)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(id)
    .bind(task.id)
    .bind(task.project_id.0)
    .bind(task.owner_account_id.0)
    .bind(&built.tree_id)
    .bind(&built.relative_path)
    .bind(built.projection["tool"]["name"].as_str().unwrap_or(""))
    .bind(built.projection["tool"]["version"].as_str().unwrap_or(""))
    .bind(json!(built.source_hashes))
    .bind(&built.projection_hash)
    .bind(&built.projection)
    .bind(&built.plan)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}

/// The newest projection of a tree in a task.
pub async fn latest_projection(conn: &mut PgConnection, task: TaskId, tree_id: &str) -> Result<Option<(Uuid, Built)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, relative_path, source_hashes, projection_hash, projection, plan FROM focus_projections
         WHERE task_id = $1 AND tree_id = $2 ORDER BY created_at DESC, id LIMIT 1",
    )
    .bind(task)
    .bind(tree_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let hashes: Value = row.try_get("source_hashes")?;
    let source_hashes: BTreeMap<String, String> = serde_json::from_value(hashes).unwrap_or_default();
    Ok(Some((
        row.try_get("id")?,
        Built {
            tree_id: tree_id.to_owned(),
            relative_path: row.try_get("relative_path")?,
            projection: row.try_get("projection")?,
            projection_hash: row.try_get("projection_hash")?,
            plan: row.try_get("plan")?,
            source_hashes,
        },
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Person(factory_accounts::AccountId),
    NativeAgent(AttemptId),
    CodingClient,
}

impl ActorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorKind::Person(_) => "person",
            ActorKind::NativeAgent(_) => "native_agent",
            ActorKind::CodingClient => "coding_client",
        }
    }
}

pub struct NewProposal<'a> {
    pub task: &'a TaskKeys,
    pub actor: ActorKind,
    pub tree_id: &'a str,
    pub tool_operation: &'a str,
    pub tool_version: &'a str,
    pub base_projection_hash: &'a str,
    pub operations: Value,
    pub request_sha256: &'a str,
    pub before: &'a BTreeMap<String, String>,
    pub after: &'a BTreeMap<String, String>,
    pub changed_paths: &'a [String],
    pub changed_ids: &'a [String],
    pub semantic_summary: Value,
    pub diffs: Value,
    pub after_projection_id: Option<Uuid>,
}

const PROPOSAL_COLUMNS: &str = "id, task_id, actor_kind, actor_account_id, attempt_id, tree_id, tool_operation, tool_version,
    base_projection_hash, operations, request_sha256, before_hashes, after_hashes, changed_paths, changed_ids,
    semantic_summary, diffs, proposal_hash, state, after_projection_id, engine_report, finding_delta, created_at, updated_at";

/// The canonical hash approval binds to: what was asked, on which base, and what it changed.
pub fn proposal_hash(p: &NewProposal<'_>) -> String {
    factory_hoi4_tools::mcp::canonical_sha256(&json!({
        "schema": "hoi4factory.change-proposal/1",
        "task": p.task.id,
        "tree_id": p.tree_id,
        "actor_kind": p.actor.as_str(),
        "tool_operation": p.tool_operation,
        "tool_version": p.tool_version,
        "base_projection_hash": p.base_projection_hash,
        "operations": p.operations,
        "request_sha256": p.request_sha256,
        "before": p.before,
        "after": p.after,
        "changed_paths": p.changed_paths,
    }))
}

pub async fn insert_proposal(conn: &mut PgConnection, p: &NewProposal<'_>) -> Result<Value, sqlx::Error> {
    let id = Uuid::new_v4();
    let (account, attempt) = match p.actor {
        ActorKind::Person(a) => (Some(a.0), None),
        ActorKind::NativeAgent(at) => (None, Some(at.0)),
        ActorKind::CodingClient => (None, None),
    };
    let hash = proposal_hash(p);
    let row = sqlx::query(&format!(
        "INSERT INTO change_proposals (id, task_id, project_id, owner_account_id, actor_kind, actor_account_id,
             attempt_id, tree_id, tool_operation, tool_version, base_projection_hash, operations, request_sha256,
             before_hashes, after_hashes, changed_paths, changed_ids, semantic_summary, diffs, proposal_hash, state,
             after_projection_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, 'applied', $21)
         RETURNING {PROPOSAL_COLUMNS}"
    ))
    .bind(id)
    .bind(p.task.id)
    .bind(p.task.project_id.0)
    .bind(p.task.owner_account_id.0)
    .bind(p.actor.as_str())
    .bind(account)
    .bind(attempt)
    .bind(p.tree_id)
    .bind(p.tool_operation)
    .bind(p.tool_version)
    .bind(p.base_projection_hash)
    .bind(&p.operations)
    .bind(p.request_sha256)
    .bind(json!(p.before))
    .bind(json!(p.after))
    .bind(p.changed_paths)
    .bind(p.changed_ids)
    .bind(&p.semantic_summary)
    .bind(&p.diffs)
    .bind(&hash)
    .bind(p.after_projection_id)
    .fetch_one(&mut *conn)
    .await?;
    proposal_json(&row)
}

pub fn proposal_json(row: &sqlx::postgres::PgRow) -> Result<Value, sqlx::Error> {
    let created: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
    let updated: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;
    Ok(json!({
        "id": row.try_get::<Uuid, _>("id")?,
        "taskId": row.try_get::<Uuid, _>("task_id")?,
        "actorKind": row.try_get::<String, _>("actor_kind")?,
        "actorAccountId": row.try_get::<Option<Uuid>, _>("actor_account_id")?,
        "attemptId": row.try_get::<Option<Uuid>, _>("attempt_id")?,
        "treeId": row.try_get::<String, _>("tree_id")?,
        "toolOperation": row.try_get::<String, _>("tool_operation")?,
        "toolVersion": row.try_get::<String, _>("tool_version")?,
        "baseProjectionHash": row.try_get::<String, _>("base_projection_hash")?,
        "operations": row.try_get::<Value, _>("operations")?,
        "requestSha256": row.try_get::<String, _>("request_sha256")?,
        "before": row.try_get::<Value, _>("before_hashes")?,
        "after": row.try_get::<Value, _>("after_hashes")?,
        "changedPaths": row.try_get::<Vec<String>, _>("changed_paths")?,
        "changedIds": row.try_get::<Vec<String>, _>("changed_ids")?,
        "summary": row.try_get::<Value, _>("semantic_summary")?,
        "diffs": row.try_get::<Value, _>("diffs")?,
        "proposalHash": row.try_get::<String, _>("proposal_hash")?,
        "state": row.try_get::<String, _>("state")?,
        "afterProjectionId": row.try_get::<Option<Uuid>, _>("after_projection_id")?,
        "engineReport": row.try_get::<Option<Value>, _>("engine_report")?,
        "findingDelta": row.try_get::<Option<Value>, _>("finding_delta")?,
        "createdAt": created,
        "updatedAt": updated,
    }))
}

pub async fn list_proposals(conn: &mut PgConnection, task: TaskId) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "SELECT {PROPOSAL_COLUMNS} FROM change_proposals WHERE task_id = $1 ORDER BY created_at, id"
    ))
    .bind(task)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(proposal_json).collect()
}

pub async fn get_proposal(conn: &mut PgConnection, task: TaskId, id: Uuid) -> Result<Option<Value>, sqlx::Error> {
    let row = sqlx::query(&format!("SELECT {PROPOSAL_COLUMNS} FROM change_proposals WHERE task_id = $1 AND id = $2"))
        .bind(task)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    row.as_ref().map(proposal_json).transpose()
}

/// Person proposals of a task created after `after` (the agent's last read), oldest first.
pub async fn person_changes_since(
    conn: &mut PgConnection,
    task: TaskId,
    after: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<(String, Vec<String>)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT tree_id, changed_ids FROM change_proposals
         WHERE task_id = $1 AND actor_kind = 'person' AND created_at > $2 ORDER BY created_at",
    )
    .bind(task)
    .bind(after)
    .fetch_all(&mut *conn)
    .await?;
    rows.iter()
        .map(|r| Ok((r.try_get("tree_id")?, r.try_get("changed_ids")?)))
        .collect()
}

pub async fn set_check(conn: &mut PgConnection, id: Uuid, report: &Value, delta: &Value) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE change_proposals SET engine_report = $2, finding_delta = $3, updated_at = clock_timestamp() WHERE id = $1",
    )
    .bind(id)
    .bind(report)
    .bind(delta)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn set_state(conn: &mut PgConnection, task: TaskId, ids: &[Uuid], from: &str, to: &str) -> Result<u64, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE change_proposals SET state = $4, updated_at = clock_timestamp()
         WHERE task_id = $1 AND id = ANY($2) AND state = $3",
    )
    .bind(task)
    .bind(ids)
    .bind(from)
    .bind(to)
    .execute(&mut *conn)
    .await?;
    Ok(done.rows_affected())
}
