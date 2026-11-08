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
            before_contents: &rewritten.before_contents,
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

fn git_head_bytes(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = std::process::Command::new("git")
        .args(["-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Records files an agent wrote with `write_file` as one reviewable C-02 proposal, compared with
/// the worktree's HEAD, so every agent change goes through the same review, commit check and
/// commit as a focus edit. Returns `None` when none of the files differs from HEAD.
pub async fn record_file_proposal(
    rt: &AgentRuntime,
    task: &TaskKeys,
    root: &Path,
    actor: ActorKind,
    paths: &[String],
) -> Result<Option<Value>, FocusError> {
    let root_buf = root.to_path_buf();
    let wanted = paths.to_vec();
    let (head, files) = tokio::task::spawn_blocking(move || {
        let head = git_head_bytes(&root_buf, &["rev-parse", "HEAD"]).map(|b| String::from_utf8_lossy(&b).trim().to_owned());
        let mut files = Vec::new();
        for path in wanted {
            if factory_workspace::validate_relative(&path).is_err() {
                continue;
            }
            let before = git_head_bytes(&root_buf, &["cat-file", "blob", &format!("HEAD:{path}")]);
            let after = std::fs::read(root_buf.join(&path)).ok();
            if before != after {
                files.push((path, before, after));
            }
        }
        (head, files)
    })
    .await
    .map_err(|e| FocusError::Internal { message: e.to_string() })?;
    if files.is_empty() {
        return Ok(None);
    }
    let hash = |b: &Option<Vec<u8>>| b.as_deref().map(super::session::sha256_hex).unwrap_or_else(|| super::session::ABSENT.to_owned());
    let before: BTreeMap<String, String> = files.iter().map(|(p, b, _)| (p.clone(), hash(b))).collect();
    let after: BTreeMap<String, String> = files.iter().map(|(p, _, a)| (p.clone(), hash(a))).collect();
    let changed_paths: Vec<String> = files.iter().map(|(p, _, _)| p.clone()).collect();
    let before_contents: BTreeMap<String, Option<Vec<u8>>> = files.iter().map(|(p, b, _)| (p.clone(), b.clone())).collect();
    let diffs: Vec<Value> = files
        .iter()
        .map(|(p, b, a)| json!({"path": p, "diff": super::session::unified_diff(p, b.as_deref(), a.as_deref())}))
        .collect();
    let operations = json!(changed_paths.iter().map(|p| json!({"op": "write_file", "path": p, "after_sha256": after[p]})).collect::<Vec<_>>());
    let base = super::session::sha256_hex(head.unwrap_or_default().as_bytes());
    let request_sha256 = factory_hoi4_tools::mcp::canonical_sha256(&operations);
    let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
    let proposal = store::insert_proposal(
        &mut tx,
        &NewProposal {
            task,
            actor,
            tree_id: "files",
            tool_operation: "write_file",
            tool_version: crate::agent::workspace_tools::TOOL_VERSION,
            base_projection_hash: &base,
            operations,
            request_sha256: &request_sha256,
            before: &before,
            after: &after,
            changed_paths: &changed_paths,
            changed_ids: &[],
            semantic_summary: json!({"added": [], "changed": [], "removed": [], "files": changed_paths}),
            diffs: json!(diffs),
            after_projection_id: None,
            before_contents: &before_contents,
        },
    )
    .await
    .map_err(db)?;
    let mut draft = event(
        None,
        match actor {
            ActorKind::Person(a) => Actor::user(a),
            _ => Actor::Host,
        },
        "source_changed",
        "Files changed",
        json!({"proposal_id": proposal["id"], "actor_kind": actor.as_str(), "changed_paths": changed_paths, "file_hashes": after}),
    );
    if let ActorKind::NativeAgent(attempt) = actor {
        draft = draft.for_attempt(attempt);
    }
    factory_tasks::append_event(&mut tx, task.id, draft).await.map_err(|e| db(e.to_string()))?;
    tx.commit().await.map_err(db)?;
    rt.notify(task.id);
    Ok(Some(proposal))
}

/// Discards an uncommitted proposal: each of its files goes back to the bytes it had before the
/// proposal, but only while the file still holds what the proposal wrote. A file changed since
/// (by a person, the agent or a later proposal) refuses the whole discard and nothing is written.
pub async fn discard_proposal(rt: &AgentRuntime, task: &TaskKeys, root: &Path, actor: ActorKind, id: uuid::Uuid) -> Result<Value, FocusError> {
    let slot = rt.focus().slot(task.id);
    let _guard = slot.lock().await;
    let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
    let proposal = store::get_proposal(&mut tx, task.id, id).await.map_err(db)?.ok_or(FocusError::Internal { message: "no such proposal".into() })?;
    let contents = store::before_contents(&mut tx, task.id, id).await.map_err(db)?;
    tx.commit().await.map_err(db)?;
    if proposal["state"] != "applied" {
        return Err(FocusError::OperationRefused {
            error: super::ops::OpError { code: "not_open", focus_id: String::new(), message: "Only an uncommitted proposal can be discarded.".into() },
        });
    }
    let paths: Vec<String> = proposal["changedPaths"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    let mut changed_since = BTreeMap::new();
    for p in &paths {
        let now = std::fs::read(root.join(p)).ok().map(|b| super::session::sha256_hex(&b)).unwrap_or_else(|| super::session::ABSENT.to_owned());
        if Some(now.as_str()) != proposal["after"][p].as_str() {
            changed_since.insert(p.clone(), now);
        }
    }
    if !changed_since.is_empty() || paths.iter().any(|p| !contents.contains_key(p)) {
        return Err(FocusError::StaleBase { current: changed_since });
    }
    for p in &paths {
        match &contents[p] {
            Some(bytes) => std::fs::write(root.join(p), bytes).map_err(|e| FocusError::Internal { message: e.to_string() })?,
            None => std::fs::remove_file(root.join(p)).map_err(|e| FocusError::Internal { message: e.to_string() })?,
        }
    }
    let mut tx = rt.tx(task.acting).await.map_err(|e| db(e.code))?;
    store::set_state(&mut tx, task.id, &[id], "applied", "discarded").await.map_err(db)?;
    let draft = event(
        None,
        match actor {
            ActorKind::Person(a) => Actor::user(a),
            _ => Actor::Host,
        },
        "source_changed",
        "A proposal was discarded",
        json!({"discarded_proposal_id": id, "actor_kind": actor.as_str(), "tree_id": proposal["treeId"], "changed_ids": proposal["changedIds"], "changed_paths": paths, "file_hashes": proposal["before"]}),
    );
    factory_tasks::append_event(&mut tx, task.id, draft).await.map_err(|e| db(e.to_string()))?;
    tx.commit().await.map_err(db)?;
    rt.notify(task.id);
    Ok(json!({"discarded": id, "restored_paths": paths}))
}
