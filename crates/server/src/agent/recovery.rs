//! What a new host incarnation does with attempts an earlier process left behind, and how a
//! suspended attempt resumes.
//!
//! Startup never reruns the last prompt (SPEC-003). For every nonterminal attempt it takes over
//! the workspace lease (a new generation, so the old process is fenced even if it still runs),
//! then reconciles each operation that was started but not settled: a file write is resolved by
//! observing the file's hash against the intent's expected-before and intended-after hashes; a
//! read or a derived job has no source effect and is resolved as not applied. Open model
//! reservations stay counted as `usage_unknown`. The attempt then ends (`failed` with
//! `host_restarted`, `cancelled` if an interrupt was pending, or `unknown_effect` when something
//! stays unresolved) and the task returns to `ready` (or `blocked`), so a new attempt gets the
//! earlier receipts and any uncertainty in its context bundle. A paused, suspended attempt with
//! nothing unresolved is kept as it is and resumes when the user resumes the task.

use std::time::Duration;

use factory_accounts::{AccountId, ApiError};
use factory_agent::conversation::Conversation;
use factory_agent::profile::ModelProfile;
use factory_broker::{Funding as MeterFunding, GrantId, UserKeyId, get_grant};
use factory_tasks::{
    Actor, Attempt, AttemptState, EffectClass, Funding, ReceiptStatus, Reconciliation,
    ReconciliationDecision, RecoveryItem, Task, TaskState,
};
use factory_workspace::{AccountId as WsAccount, ProjectId as WsProject};
use serde_json::json;

use super::runner::{AttemptPlan, ReplayBase, event, launch, task_error, ws_task};
use super::workspace_tools::{ABSENT, observe};
use super::AgentRuntime;

/// Runs recovery until every leftover attempt is handled. Attempts whose lease another live
/// incarnation still holds are retried after its expiry.
pub fn spawn(rt: AgentRuntime) {
    tokio::spawn(async move {
        for round in 0..30u32 {
            match recover_all(&rt).await {
                Ok(0) => break,
                Ok(left) => tracing::info!(left, round, "recovery is waiting for leases to expire"),
                Err(e) => tracing::error!(error = %e, "recovery failed; retrying"),
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

/// One recovery pass. Returns how many attempts are left for a later pass.
pub async fn recover_all(rt: &AgentRuntime) -> Result<usize, String> {
    let items = {
        let mut conn = rt.inner.db.pool().acquire().await.map_err(|e| e.to_string())?;
        factory_tasks::recovery_scan(&mut conn).await.map_err(|e| e.to_string())?
    };
    let mut left = 0;
    for item in items {
        if rt.is_running(item.attempt_id) {
            continue;
        }
        match recover_one(rt, &item).await {
            Ok(true) => {}
            Ok(false) => left += 1,
            Err(e) => {
                tracing::error!(attempt = %item.attempt_id, error = %e, "could not recover an attempt");
                left += 1;
            }
        }
    }
    Ok(left)
}

/// The account an attempt runs as: the grantor of its grant.
async fn grantor_of(rt: &AgentRuntime, owner: AccountId, attempt: &Attempt) -> Result<AccountId, String> {
    let mut tx = rt.tx(owner).await.map_err(|e| e.message)?;
    let grant = get_grant(&mut tx, GrantId(attempt.grant_id))
        .await
        .map_err(|e| e.to_string())?
        .ok_or("the attempt's grant is missing")?;
    Ok(grant.grantor_account_id)
}

async fn recover_one(rt: &AgentRuntime, item: &RecoveryItem) -> Result<bool, String> {
    let owner = item.account_id;
    let (task, attempt) = {
        let mut tx = rt.tx(owner).await.map_err(|e| e.message)?;
        let task = factory_tasks::get_task(&mut tx, item.task_id).await.map_err(|e| e.to_string())?;
        let attempt = factory_tasks::get_attempt(&mut tx, item.attempt_id).await.map_err(|e| e.to_string())?;
        (task, attempt)
    };
    let account = grantor_of(rt, task.owner_account_id, &attempt).await?;
    let mut tx = rt.tx(account).await.map_err(|e| e.message)?;
    let pending = factory_tasks::pending_for_task(&mut tx, task.id).await.map_err(|e| e.to_string())?;
    if task.paused && attempt.state == AttemptState::Running && pending.is_empty() {
        // Suspended by a pause: it keeps its conversation and resumes on request.
        return Ok(true);
    }
    let Some(lease) = factory_tasks::acquire_lease(
        &mut tx,
        task.project_id,
        task.owner_account_id,
        rt.inner.incarnation,
        rt.inner.settings.lease_ttl,
    )
    .await
    .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    let worktree = worktree_dir(rt, &task);
    let mut report = Vec::new();
    for (intent, receipt) in pending {
        if receipt.status == ReceiptStatus::Started {
            factory_tasks::mark_unknown(
                &mut tx,
                intent.operation_id,
                json!({ "reason": "the host stopped before the receipt was settled" }),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        let (decision, observation, observed) = if intent.effect_class == EffectClass::IdempotentWrite {
            let path = intent.observation_inputs["path"].as_str().unwrap_or("").to_owned();
            let expected = intent.observation_inputs["expected_before"].as_str().unwrap_or("").to_owned();
            let intended = intent.observation_inputs["intended_after"].as_str().unwrap_or("").to_owned();
            let root = worktree.clone();
            let p = path.clone();
            let seen = tokio::task::spawn_blocking(move || match root {
                Some(root) => observe(&root, &p).map_err(|e| e.to_string()),
                None => Err("the task worktree is missing".into()),
            })
            .await
            .map_err(|e| e.to_string())?;
            match seen {
                Ok(hash) => {
                    let decision = if hash == intended {
                        ReconciliationDecision::Applied
                    } else if hash == expected || (expected == ABSENT && hash == ABSENT) {
                        ReconciliationDecision::NotApplied
                    } else {
                        ReconciliationDecision::Conflict
                    };
                    let observed = (hash != ABSENT).then(|| hash.clone());
                    (decision, json!({ "path": path, "observed_sha256": hash, "expected_before": expected, "intended_after": intended }), observed)
                }
                Err(why) => (ReconciliationDecision::Conflict, json!({ "path": path, "error": why }), None),
            }
        } else {
            (ReconciliationDecision::NotApplied, json!({ "reason": "no source effect" }), None)
        };
        factory_tasks::reconcile(
            &mut tx,
            intent.operation_id,
            Reconciliation {
                method: intent.reconciliation_method.clone(),
                observation: observation.clone(),
                decision,
                decided_by: Actor::Host,
                observed_after_sha256: observed,
            },
        )
        .await
        .map_err(|e| e.to_string())?;
        report.push(json!({ "operation_id": intent.operation_id, "tool": intent.replay.tool, "decision": format!("{decision:?}"), "observation": observation }));
    }
    // Model requests in flight when the host stopped: accepted or not is unknown.
    let open: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM model_usage WHERE attempt_id = $1 AND status = 'reserved'",
    )
    .bind(attempt.id.0)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    for id in &open {
        let _ = factory_broker::Meter::new()
            .settle(
                &mut tx,
                factory_broker::ReservationId(*id),
                factory_broker::Settlement::Unknown { reason: "the host stopped during the request".into() },
                rt.inner.clock.now(),
            )
            .await;
    }
    let unresolved = !factory_tasks::unresolved_operations(&mut tx, task.id).await.map_err(|e| e.to_string())?.is_empty();
    let mut version = attempt.version;
    let mut state = attempt.state;
    let target = match state {
        AttemptState::InterruptRequested => AttemptState::Cancelled,
        _ if unresolved => AttemptState::UnknownEffect,
        _ => AttemptState::Failed,
    };
    if state == AttemptState::UnknownEffect && target == AttemptState::Failed {
        let a = factory_tasks::transition_attempt(&mut tx, attempt.id, version, AttemptState::Reconciling, None, None, Actor::Host)
            .await
            .map_err(|e| e.to_string())?;
        version = a.version;
        state = a.state;
    }
    if state != target {
        factory_tasks::transition_attempt(
            &mut tx,
            attempt.id,
            version,
            target,
            Some(json!({ "recovery": report, "unknown_model_requests": open.len() })),
            Some("host_restarted"),
            Actor::Host,
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    let task_now = factory_tasks::get_task(&mut tx, task.id).await.map_err(|e| e.to_string())?;
    if task_now.state == TaskState::Running {
        let to = if unresolved { TaskState::Blocked } else { TaskState::Ready };
        factory_tasks::transition_task(
            &mut tx,
            task.id,
            task_now.version,
            to,
            event(Some(attempt.id), Actor::Host, "recovery_report", "Recovered after a restart: nothing was repeated", json!({
                "operations": report,
                "unknown_model_requests": open.len(),
                "lease_generation": lease.generation,
                "attempt_state": target,
            })),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    rt.notify(task.id);
    Ok(true)
}

/// The task worktree on disk, if the workspace and worktree exist.
pub fn worktree_dir(rt: &AgentRuntime, task: &Task) -> Option<std::path::PathBuf> {
    let account = WsAccount::new(task.owner_account_id.to_string()).ok()?;
    let project = WsProject::new(task.project_id.to_string()).ok()?;
    let workspace = rt.inner.storage.open_workspace(&account, &project).ok()?;
    let dir = workspace.task_dir(&ws_task(task.id));
    dir.exists().then_some(dir)
}

/// Where the tools of a project keep their state, outside the mod.
pub fn tool_state_dir(rt: &AgentRuntime, task: &Task) -> std::path::PathBuf {
    rt.inner
        .storage
        .data_root()
        .join("tool-state")
        .join(task.owner_account_id.to_string())
        .join(task.project_id.to_string())
}

/// A suspended attempt ready to continue: its lease is taken, its plan rebuilt.
pub struct Resumable {
    plan: AttemptPlan,
    conversation: Option<Conversation>,
    first: Option<String>,
}

/// Starts a prepared attempt (after the pause is cleared).
pub fn start_resumed(rt: &AgentRuntime, r: Resumable) {
    launch(rt, r.plan, r.conversation, r.first);
}

/// Takes the lease and rebuilds the plan of a suspended attempt from what the ledger holds. A
/// busy workspace is refused here, before anything about the task changes.
pub async fn prepare_resume(rt: &AgentRuntime, account: AccountId, task: &Task, attempt: &Attempt) -> Result<Resumable, ApiError> {
    let _ = account;
    let mut tx = rt.tx(account).await?;
    let grant = get_grant(&mut tx, GrantId(attempt.grant_id))
        .await
        .map_err(|_| ApiError::internal())?
        .ok_or_else(ApiError::internal)?;
    // Resuming does not renew authority: an expired or revoked grant ends here, and the user
    // starts a new attempt (which gets the earlier receipts in its bundle).
    let now = rt.inner.clock.now();
    if grant.revoked_at.is_some() || grant.expires_at <= now {
        return Err(ApiError::new(
            axum::http::StatusCode::CONFLICT,
            if grant.revoked_at.is_some() { "grant_revoked" } else { "grant_expired" },
            "This attempt's permissions ended while it was paused. Interrupt it and start a new attempt.",
            factory_accounts::RetryClass::RequiresAuthorization,
        ));
    }
    let lease = factory_tasks::acquire_lease(
        &mut tx,
        task.project_id,
        task.owner_account_id,
        rt.inner.incarnation,
        rt.inner.settings.lease_ttl,
    )
    .await
    .map_err(task_error)?
    .ok_or_else(|| ApiError::conflict("workspace_busy", "Another server process holds this workspace."))?;
    let verified: bool = sqlx::query_scalar("SELECT email_verified_at IS NOT NULL FROM accounts WHERE id = $1")
        .bind(grant.grantor_account_id.0)
        .fetch_one(&mut *tx)
        .await?;
    let first = if attempt.conversation.is_null() || attempt.conversation == json!({}) {
        let bundle = factory_tasks::load_bundle(&mut tx, attempt.bundle_id)
            .await
            .map_err(|_| ApiError::internal())?;
        Some(super::runner::render_context(&task.objective, &bundle))
    } else {
        None
    };
    tx.commit().await?;
    let conversation: Option<Conversation> = serde_json::from_value(attempt.conversation.clone()).ok();
    let worktree = worktree_dir(rt, task).ok_or_else(|| ApiError::conflict("worktree_missing", "The task worktree is missing."))?;
    let head = head_of(&worktree).await;
    let profile: ModelProfile = serde_json::from_value(attempt.model_profile.clone()).map_err(|_| ApiError::internal())?;
    let funding = match (attempt.funding, attempt.user_key_id) {
        (Funding::UserKey, Some(id)) => MeterFunding::UserKey { key_id: UserKeyId(id) },
        _ => MeterFunding::OperatorAllowance,
    };
    let plan = AttemptPlan {
        account: grant.grantor_account_id,
        verified,
        project: task.project_id,
        owner: task.owner_account_id,
        task: task.id,
        attempt: attempt.id,
        mode: task.mode,
        tool_state: tool_state_dir(rt, task),
        replay: ReplayBase {
            workspace_id: format!("workspace:{}", task.project_id),
            root_identity: format!("workspace:{}/tasks/{}", task.project_id, task.id),
            source_snapshot: head,
        },
        worktree,
        grant: grant.id,
        epoch: attempt.grant_epoch,
        generation: lease.generation,
        profile,
        funding,
        budget_micros: task.budget_micros,
        max_turns: u32::try_from(task.max_turns).unwrap_or(20),
    };
    Ok(Resumable { plan, conversation: conversation.filter(|_| first.is_none()), first })
}

/// The worktree's HEAD commit, as the source snapshot of a replay identity.
pub async fn head_of(dir: &std::path::Path) -> String {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        factory_workspace::GitRepo::open(&dir)
            .ok()
            .and_then(|r| r.head().ok().flatten())
            .map(|oid| oid.to_string())
            .unwrap_or_else(|| "unknown".into())
    })
    .await
    .unwrap_or_else(|_| "unknown".into())
}

