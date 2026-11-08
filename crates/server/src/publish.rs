//! S04-W5: the engine commit check, the commit of reviewed proposals, and restore.
//!
//! * The commit check (contract "Commit check", gate 8) runs the linked `hoi4-engine` on the
//!   task's worktree against the project's findings baseline at the worktree's HEAD. The baseline
//!   is computed once per commit from the committed files and stored in `engine_baselines`.
//! * A commit stages exactly the reviewed proposals' paths under the expected HEAD. Its intent is
//!   a `publications` row written before the effect, and the commit message carries a
//!   `Factory-Publication: <id>` line, so a lost reply is reconciled by reading the branch.
//! * Restore undoes one factory commit path by path: a path still holding the committed bytes
//!   returns to its earlier bytes, and a path someone changed since is kept and reported.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use factory_engine_check::{Baseline, CheckReport, CheckRequest, Finding, finding_delta, run_check};
use factory_workspace::{GitRepo, Identity, Oid};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::agent::AgentRuntime;
use crate::focus::store::TaskKeys;

pub const TRAILER: &str = "Factory-Publication";

#[derive(Debug)]
pub struct Checked {
    pub report: CheckReport,
    pub baseline_commit: String,
    /// Finding delta between the committed state and the worktree.
    pub delta: Value,
}

fn tmp_dir(rt: &AgentRuntime) -> PathBuf {
    rt.storage().data_root().join("tmp").join(format!("engine-{}", Uuid::new_v4()))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// The project's baseline at `head`: stored, or computed now from the committed files.
async fn baseline_at(
    rt: &AgentRuntime,
    keys: &TaskKeys,
    repo: &GitRepo,
    head: &Oid,
) -> Result<(Baseline, Vec<Finding>), String> {
    let mut tx = rt.tx(keys.acting).await.map_err(|e| e.code.to_owned())?;
    let row = sqlx::query(
        "SELECT baseline, report_summary FROM engine_baselines WHERE project_id = $1 AND commit_oid = $2 AND engine_rev = $3",
    )
    .bind(keys.project_id.0)
    .bind(head.as_str())
    .bind(factory_engine_check::ENGINE_REV)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    if let Some(row) = row {
        let baseline: Value = row.try_get("baseline").map_err(|e| e.to_string())?;
        let summary: Value = row.try_get("report_summary").map_err(|e| e.to_string())?;
        let findings: Vec<Finding> = serde_json::from_value(summary["findings"].clone()).unwrap_or_default();
        let parsed = factory_engine_check::parse_baseline(&baseline.to_string())?;
        return Ok((parsed, findings));
    }
    let dir = tmp_dir(rt);
    let (repo2, head2, dir2) = (repo.clone(), head.clone(), dir.clone());
    let game = rt.agent_tools_settings().and_then(|s| s.game_root.clone());
    let report = blocking(move || {
        repo2.export_commit(&head2, &dir2).map_err(|e| e.to_string())?;
        let report = run_check(&CheckRequest::new(&dir2, game)).map_err(|e| e.to_string());
        let _ = std::fs::remove_dir_all(&dir2);
        report
    })
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    let report = report?;
    let baseline = factory_engine_check::baseline_from(&report.findings);
    let baseline_json: Value = serde_json::from_str(&baseline.to_json()).map_err(|e| e.to_string())?;
    let summary = json!({
        "verdict": report.verdict,
        "unknown": report.unknown,
        "findings": report.findings,
        "game_version": report.game_version,
    });
    let mut tx = rt.tx(keys.acting).await.map_err(|e| e.code.to_owned())?;
    sqlx::query(
        "INSERT INTO engine_baselines (id, project_id, owner_account_id, commit_oid, engine_version, engine_rev, baseline, report_summary)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (project_id, commit_oid, engine_rev) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(keys.project_id.0)
    .bind(keys.owner_account_id.0)
    .bind(head.as_str())
    .bind(factory_engine_check::ENGINE_VERSION)
    .bind(factory_engine_check::ENGINE_REV)
    .bind(&baseline_json)
    .bind(&summary)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((baseline, report.findings))
}

/// Runs the commit check on the task's worktree.
pub async fn check_worktree(rt: &AgentRuntime, keys: &TaskKeys, repo: &GitRepo, worktree: &Path, head: &Oid) -> Result<Checked, String> {
    let (baseline, before) = baseline_at(rt, keys, repo, head).await?;
    let game = rt.agent_tools_settings().and_then(|s| s.game_root.clone());
    let root = worktree.to_path_buf();
    let report = blocking(move || {
        let mut req = CheckRequest::new(root, game);
        req.baseline = Some(baseline);
        run_check(&req).map_err(|e| e.to_string())
    })
    .await?;
    let delta = finding_delta(&before, &report.findings);
    Ok(Checked { delta: json!({"summary": delta.summary, "entries": delta.entries}), report, baseline_commit: head.to_string() })
}

/// A short form of the check for proposals, events and the review card.
pub fn check_summary(c: &Checked) -> Value {
    json!({
        "verdict": c.report.verdict,
        "engine_version": c.report.engine_version,
        "engine_rev": c.report.engine_rev,
        "game_version": c.report.game_version,
        "baseline_commit": c.baseline_commit,
        "new_findings": c.report.new_count,
        "baselined_findings": c.report.baselined_count,
        "blocking": c.report.blocking.iter().map(|f| json!({"code": f.code, "path": f.path, "line": f.line, "message": f.message})).collect::<Vec<_>>(),
        "unknown": c.report.unknown,
        "elapsed_ms": c.report.elapsed_ms,
    })
}

/// The commit author for a person: their display name and an address that names no real inbox,
/// because commits are later pushed to public repositories.
pub fn author_for(account: &factory_accounts::Account) -> Identity {
    let name: String = account.display_name.chars().filter(|c| !c.is_control() && *c != '<' && *c != '>').take(100).collect();
    let name = if name.trim().is_empty() { "HOI4 Factory member".to_owned() } else { name };
    Identity::new(name, format!("{}@users.hoi4-factory.invalid", account.id)).unwrap_or_else(|_| Identity::factory())
}

// ---- publications ----

pub struct Publication {
    pub id: Uuid,
    pub state: String,
    pub intent: Value,
    pub result: Option<Value>,
}

pub async fn find_publication(conn: &mut PgConnection, task: factory_tasks::TaskId, kind: &str, key: &str) -> Result<Option<Publication>, sqlx::Error> {
    let row = sqlx::query("SELECT id, state, intent, result FROM publications WHERE task_id = $1 AND kind = $2 AND idempotency_key = $3")
        .bind(task)
        .bind(kind)
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;
    row.map(|r| {
        Ok(Publication { id: r.try_get("id")?, state: r.try_get("state")?, intent: r.try_get("intent")?, result: r.try_get("result")? })
    })
    .transpose()
}

pub async fn get_publication(conn: &mut PgConnection, task: factory_tasks::TaskId, id: Uuid) -> Result<Option<(String, Publication)>, sqlx::Error> {
    let row = sqlx::query("SELECT id, kind, state, intent, result FROM publications WHERE task_id = $1 AND id = $2")
        .bind(task)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    row.map(|r| {
        Ok((
            r.try_get("kind")?,
            Publication { id: r.try_get("id")?, state: r.try_get("state")?, intent: r.try_get("intent")?, result: r.try_get("result")? },
        ))
    })
    .transpose()
}

pub async fn insert_intent(
    conn: &mut PgConnection,
    keys: &TaskKeys,
    kind: &str,
    key: &str,
    grant: Option<Uuid>,
    intent: &Value,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO publications (id, task_id, project_id, owner_account_id, kind, idempotency_key, requested_by, grant_id, intent)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(keys.id)
    .bind(keys.project_id.0)
    .bind(keys.owner_account_id.0)
    .bind(kind)
    .bind(key)
    .bind(keys.acting.0)
    .bind(grant)
    .bind(intent)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}

pub async fn settle(conn: &mut PgConnection, id: Uuid, state: &str, result: &Value) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE publications SET state = $2, result = $3, settled_at = clock_timestamp() WHERE id = $1 AND state IN ('intent', 'unknown')")
        .bind(id)
        .bind(state)
        .bind(result)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub fn trailer_line(id: Uuid) -> String {
    format!("{TRAILER}: {id}")
}

/// What restoring one commit does to each of its paths.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct RestorePlan {
    /// Paths that still hold the committed bytes: written back to the parent's bytes.
    pub restore: Vec<String>,
    /// Paths changed by someone since the commit: kept as they are.
    pub kept: Vec<String>,
}

/// Plans and writes a restore of `commit` (whose parent is `parent`) in the worktree.
pub fn restore_files(repo: &GitRepo, worktree: &Path, commit: &Oid, parent: &Oid, paths: &[String]) -> Result<RestorePlan, String> {
    let mut plan = RestorePlan::default();
    let unique: BTreeSet<&String> = paths.iter().collect();
    for path in unique {
        let committed = repo.read_at(commit, path).map_err(|e| e.to_string())?;
        let now = std::fs::read(worktree.join(path)).ok();
        if now != committed {
            plan.kept.push(path.clone());
            continue;
        }
        let earlier = repo.read_at(parent, path).map_err(|e| e.to_string())?;
        match earlier {
            Some(bytes) => std::fs::write(worktree.join(path), bytes).map_err(|e| e.to_string())?,
            None => std::fs::remove_file(worktree.join(path)).map_err(|e| e.to_string())?,
        }
        plan.restore.push(path.clone());
    }
    Ok(plan)
}
