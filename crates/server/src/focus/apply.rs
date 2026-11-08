//! The one path that applies focus operations, for the editor routes and the agent's tool.
//!
//! Each task has one slot in [`FocusHost`]: the Agent Tools session on its worktree, behind an
//! async lock. Every focus operation of the task, whoever sends it, takes that lock, so a person's
//! gesture and the agent's edit never interleave and both go through the same request builder,
//! the same hash guards and the same proposal record.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use factory_tasks::{Actor, TaskId};
use serde_json::{Value, json};
use tokio::sync::Mutex as AsyncMutex;

use super::ops::FocusOp;
use super::projection::Built;
use super::session::{FocusError, FocusSession};
use super::store::{self, ActorKind, NewProposal, TaskKeys};
use crate::agent::AgentRuntime;
use crate::agent::runner::event;

pub const REWRITE_OPERATION: &str = "hoi4.focus_rewrite";

type Slot = Arc<AsyncMutex<Option<FocusSession>>>;

/// The Agent Tools sessions of the focus editor, one per task.
#[derive(Default)]
pub struct FocusHost {
    slots: Mutex<HashMap<TaskId, Slot>>,
}

impl FocusHost {
    pub fn slot(&self, task: TaskId) -> Slot {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(task)
            .or_default()
            .clone()
    }

    /// Stops the session of a task, for example before its worktree is removed.
    pub async fn close(&self, task: TaskId) {
        let slot = self.slots.lock().unwrap_or_else(|e| e.into_inner()).remove(&task);
        if let Some(slot) = slot
            && let Some(session) = slot.lock().await.take()
        {
            session.shutdown().await;
        }
    }
}

async fn open<'a>(rt: &AgentRuntime, task: &TaskKeys, root: &Path, guard: &'a mut Option<FocusSession>) -> Result<&'a mut FocusSession, FocusError> {
    let stale = guard.as_ref().is_some_and(|s| s.root() != root);
    if stale && let Some(old) = guard.take() {
        old.shutdown().await;
    }
    if guard.is_none() {
        let settings = rt
            .agent_tools_settings()
            .ok_or_else(|| FocusError::ToolsUnavailable { message: "HOI4 Agent Tools are not installed on this server".into() })?;
        let state = rt
            .storage()
            .data_root()
            .join("tool-state")
            .join(task.owner_account_id.to_string())
            .join(task.project_id.to_string())
            .join("focus-editor");
        *guard = Some(FocusSession::start(settings, root, &state).await?);
    }
    Ok(guard.as_mut().expect("session started"))
}

fn db(e: impl std::fmt::Display) -> FocusError {
    FocusError::Internal { message: format!("database: {e}") }
}

async fn store_if_new(rt: &AgentRuntime, task: &TaskKeys, built: &Built) -> Result<uuid::Uuid, FocusError> {
    let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
    if let Some((id, latest)) = store::latest_projection(&mut tx, task.id, &built.tree_id).await.map_err(db)?
        && latest.projection_hash == built.projection_hash
        && latest.source_hashes == built.source_hashes
    {
        tx.commit().await.map_err(db)?;
        return Ok(id);
    }
    let id = store::insert_projection(&mut tx, task, built).await.map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(id)
}

/// Inspects a tree in the task's worktree and records its projection.
pub async fn inspect_tree(rt: &AgentRuntime, task: &TaskKeys, root: &Path, tree_id: &str) -> Result<Value, FocusError> {
    let slot = rt.focus().slot(task.id);
    let mut guard = slot.lock().await;
    let session = open(rt, task, root, &mut guard).await?;
    let built = session.inspect(tree_id).await?;
    store_if_new(rt, task, &built).await?;
    Ok(built.projection)
}

/// Focuses added, and fields that changed with their before and after values.
pub fn semantic_summary(before: &Value, after: &Value, changed_ids: &[String]) -> Value {
    let index = |p: &Value| -> BTreeMap<String, Value> {
        p["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|n| Some((n["id"].as_str()?.to_owned(), n.clone())))
            .collect()
    };
    let (b, a) = (index(before), index(after));
    let mut added = Vec::new();
    let mut changed = Vec::new();
    for id in changed_ids {
        match (b.get(id), a.get(id)) {
            (None, Some(_)) => added.push(json!(id)),
            (Some(old), Some(new)) => {
                let mut fields = Vec::new();
                for (name, key) in [("cost", "cost"), ("x", "x"), ("y", "y"), ("prerequisites", "prerequisiteGroups"), ("title", "label")] {
                    if old[key] != new[key] {
                        fields.push(json!({"name": name, "before": old[key].to_string(), "after": new[key].to_string()}));
                    }
                }
                changed.push(json!({"id": id, "fields": fields}));
            }
            _ => {}
        }
    }
    let removed: Vec<Value> = b.keys().filter(|id| !a.contains_key(*id)).map(|id| json!(id)).collect();
    json!({"added": added, "changed": changed, "removed": removed})
}

pub struct Applied {
    pub proposal: Value,
    pub projection: Value,
}

/// Applies `ops` to `tree_id` in the task's worktree as one C-02 proposal.
///
/// `expected_projection_hash` is the projection the sender looked at. When it is not the latest,
/// or when a file it was built from changed on disk, nothing is written and the answer is
/// `stale_base` with the current state.
pub async fn apply_operations(
    rt: &AgentRuntime,
    task: &TaskKeys,
    root: &Path,
    actor: ActorKind,
    tree_id: &str,
    expected_projection_hash: Option<&str>,
    ops: Vec<FocusOp>,
) -> Result<Applied, FocusError> {
    let slot = rt.focus().slot(task.id);
    let mut guard = slot.lock().await;
    let session = open(rt, task, root, &mut guard).await?;

    let latest = {
        let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
        let l = store::latest_projection(&mut tx, task.id, tree_id).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        l.map(|(_, b)| b)
    };
    let base = match (latest, expected_projection_hash) {
        (Some(b), Some(expected)) if b.projection_hash != expected => {
            return Err(FocusError::StaleBase {
                current: BTreeMap::from([("projectionHash".to_owned(), b.projection_hash)]),
            });
        }
        (Some(b), _) => b,
        (None, Some(_)) => {
            let built = session.inspect(tree_id).await?;
            store_if_new(rt, task, &built).await?;
            return Err(FocusError::StaleBase {
                current: BTreeMap::from([("projectionHash".to_owned(), built.projection_hash)]),
            });
        }
        (None, None) => {
            let built = session.inspect(tree_id).await?;
            store_if_new(rt, task, &built).await?;
            built
        }
    };
    if session.stale_files(&base).is_some() {
        // Something else wrote since the projection. Record the current state so the sender can
        // refresh, and write nothing.
        let built = session.inspect(tree_id).await?;
        store_if_new(rt, task, &built).await?;
        let mut current = built.source_hashes.clone();
        current.insert("projectionHash".to_owned(), built.projection_hash);
        return Err(FocusError::StaleBase { current });
    }

    let operations = serde_json::to_value(&ops).unwrap_or(Value::Null);
    let rewritten = session.rewrite(&base, &ops).await?;
    let tool_version = session.tool_version().to_owned();
    // The rewrite is done. A failed re-inspection still leaves a proposal to review.
    let after = session.inspect(tree_id).await;

    let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
    let (after_id, after_projection) = match &after {
        Ok(built) => (Some(store::insert_projection(&mut tx, task, built).await.map_err(db)?), built.projection.clone()),
        Err(_) => (None, Value::Null),
    };
    let summary = semantic_summary(&base.projection, &after_projection, &rewritten.change.changed_ids);
    let diffs: Vec<Value> = rewritten.diffs.iter().map(|(p, d)| json!({"path": p, "diff": d})).collect();
    let proposal = store::insert_proposal(
        &mut tx,
        &NewProposal {
            task,
            actor,
            tree_id,
            tool_operation: REWRITE_OPERATION,
            tool_version: &tool_version,
            base_projection_hash: &base.projection_hash,
            operations,
            request_sha256: &rewritten.request_sha256,
            before: &rewritten.before,
            after: &rewritten.after,
            changed_paths: &rewritten.changed_paths,
            changed_ids: &rewritten.change.changed_ids,
            semantic_summary: summary,
            diffs: json!(diffs),
            after_projection_id: after_id,
        },
    )
    .await
    .map_err(db)?;
    let event_actor = match actor {
        ActorKind::Person(a) => Actor::user(a),
        _ => Actor::Host,
    };
    let mut draft = event(
        None,
        event_actor,
        "source_changed",
        "Focus tree changed",
        json!({
            "proposal_id": proposal["id"],
            "actor_kind": actor.as_str(),
            "tree_id": tree_id,
            "changed_ids": rewritten.change.changed_ids,
            "file_hashes": rewritten.after,
            "projection_hash": after_projection["projectionHash"],
        }),
    );
    if let ActorKind::NativeAgent(attempt) = actor {
        draft = draft.for_attempt(attempt);
    }
    if let Some(h) = after_projection["projectionHash"].as_str() {
        draft = draft.with_snapshot(h.to_owned());
    }
    factory_tasks::append_event(&mut tx, task.id, draft).await.map_err(|e| db(e.to_string()))?;
    tx.commit().await.map_err(db)?;
    rt.notify(task.id);
    if let Err(e) = after {
        tracing::warn!(error = ?e, "the projection after a rewrite could not be built");
    }
    Ok(Applied { proposal, projection: after_projection })
}
