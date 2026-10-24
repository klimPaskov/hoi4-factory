//! Effect journal with lease fencing in PostgreSQL (probe).
//!
//! Intent is committed before any effect. The outcome is published only if
//! the caller still holds the current fencing token. Recovery never re-runs
//! an in-flight operation. It records `unknown_effect` with an observation and
//! leaves replay blocked until an explicit reconciliation.

use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};

use crate::broker::Prepared;
use crate::util::{canonical, sha256_hex};

pub const SCHEMA: &str = "s01_runtime_probe";

pub fn database_url() -> String {
    std::env::var("FACTORY_PROBE_DATABASE_URL")
        .unwrap_or_else(|_| "host=127.0.0.1 port=55432 user=factory dbname=factory connect_timeout=5".into())
}

pub struct Journal {
    pub c: Client,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum JournalRefusal {
    StaleFence { presented: i64, current: i64, current_owner: String },
    LeaseExpired { fence: i64 },
    Paused,
    RevokedEpoch { presented: i64, current: i64 },
    NoLease,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Lease {
    pub owner: String,
    pub fence: i64,
}

impl Journal {
    pub async fn connect() -> Result<Journal, tokio_postgres::Error> {
        let (c, conn) = tokio_postgres::connect(&database_url(), NoTls).await?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        c.batch_execute(&format!("SET search_path TO {SCHEMA}, public")).await.ok();
        Ok(Journal { c })
    }

    pub async fn reset(&self) -> Result<(), tokio_postgres::Error> {
        self.c
            .batch_execute(&format!(
                r#"
DROP SCHEMA IF EXISTS {SCHEMA} CASCADE;
CREATE SCHEMA {SCHEMA};
SET search_path TO {SCHEMA}, public;
CREATE TABLE lease (workspace_id text PRIMARY KEY, owner text NOT NULL, fence bigint NOT NULL, expires_at timestamptz NOT NULL);
CREATE TABLE control (workspace_id text PRIMARY KEY, paused boolean NOT NULL DEFAULT false, revocation_epoch bigint NOT NULL DEFAULT 1);
CREATE TABLE op_intent (
  op_id text PRIMARY KEY, workspace_id text NOT NULL, request_id text NOT NULL, attempt_id text NOT NULL,
  fence bigint NOT NULL, revocation_epoch bigint NOT NULL, tool text NOT NULL, tool_version text NOT NULL,
  schema_version int NOT NULL, root_identity text NOT NULL, args_hash text NOT NULL, args jsonb NOT NULL,
  replay_identity text NOT NULL, retry_class text NOT NULL, reconcile_method text NOT NULL,
  before_hash text NOT NULL, intended_after_hash text NOT NULL,
  state text NOT NULL, after_hash text, observation jsonb, created_at timestamptz NOT NULL DEFAULT now(), settled_at timestamptz);
CREATE TABLE attempt (attempt_id text PRIMARY KEY, workspace_id text NOT NULL, state text NOT NULL, worker_generation bigint NOT NULL DEFAULT 0, outcome jsonb, updated_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE run_event (seq bigserial PRIMARY KEY, workspace_id text NOT NULL, attempt_id text, event_type text NOT NULL, payload jsonb NOT NULL, payload_hash text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE supervisor (workspace_id text PRIMARY KEY, generation bigint NOT NULL);
CREATE TABLE diagnostic (id bigserial PRIMARY KEY, workspace_id text NOT NULL, kind text NOT NULL, payload jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
"#
            ))
            .await
    }

    pub async fn acquire_lease(&self, ws: &str, owner: &str, ttl_ms: i64) -> Result<Option<Lease>, tokio_postgres::Error> {
        let rows = self
            .c
            .query(
                "INSERT INTO lease (workspace_id, owner, fence, expires_at) VALUES ($1, $2, 1, now() + make_interval(secs => $3::float8 / 1000))
                 ON CONFLICT (workspace_id) DO UPDATE SET owner = EXCLUDED.owner, fence = lease.fence + 1, expires_at = EXCLUDED.expires_at
                 WHERE lease.expires_at < now() OR lease.owner = EXCLUDED.owner
                 RETURNING owner, fence",
                &[&ws, &owner, &(ttl_ms as f64)],
            )
            .await?;
        self.c
            .execute("INSERT INTO control (workspace_id) VALUES ($1) ON CONFLICT DO NOTHING", &[&ws])
            .await?;
        Ok(rows.first().map(|r| Lease { owner: r.get(0), fence: r.get(1) }))
    }

    pub async fn control(&self, ws: &str) -> Result<(bool, i64), tokio_postgres::Error> {
        let r = self.c.query_one("SELECT paused, revocation_epoch FROM control WHERE workspace_id=$1", &[&ws]).await?;
        Ok((r.get(0), r.get(1)))
    }

    pub async fn set_paused(&mut self, ws: &str, paused: bool) -> Result<(), tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        tx.execute("UPDATE control SET paused=$2 WHERE workspace_id=$1", &[&ws, &paused]).await?;
        Self::event_tx(&tx, ws, None, if paused { "paused" } else { "resumed" }, &json!({})).await?;
        tx.commit().await
    }

    pub async fn revoke(&mut self, ws: &str) -> Result<i64, tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        let r = tx
            .query_one("UPDATE control SET revocation_epoch = revocation_epoch + 1 WHERE workspace_id=$1 RETURNING revocation_epoch", &[&ws])
            .await?;
        let e: i64 = r.get(0);
        Self::event_tx(&tx, ws, None, "grant_revoked", &json!({"revocation_epoch": e})).await?;
        tx.commit().await?;
        Ok(e)
    }

    async fn event_tx(tx: &tokio_postgres::Transaction<'_>, ws: &str, attempt: Option<&str>, ty: &str, payload: &Value) -> Result<i64, tokio_postgres::Error> {
        let h = sha256_hex(canonical(payload).as_bytes());
        let r = tx
            .query_one(
                "INSERT INTO run_event (workspace_id, attempt_id, event_type, payload, payload_hash) VALUES ($1,$2,$3,$4,$5) RETURNING seq",
                &[&ws, &attempt, &ty, payload, &h],
            )
            .await?;
        Ok(r.get(0))
    }

    async fn check_authority(tx: &tokio_postgres::Transaction<'_>, ws: &str, owner: &str, fence: i64, epoch: Option<i64>) -> Result<Result<(), JournalRefusal>, tokio_postgres::Error> {
        let Some(l) = tx
            .query_opt("SELECT owner, fence, expires_at > now() FROM lease WHERE workspace_id=$1 FOR UPDATE", &[&ws])
            .await?
        else {
            return Ok(Err(JournalRefusal::NoLease));
        };
        let (cur_owner, cur_fence, live): (String, i64, bool) = (l.get(0), l.get(1), l.get(2));
        if cur_fence != fence || cur_owner != owner {
            return Ok(Err(JournalRefusal::StaleFence { presented: fence, current: cur_fence, current_owner: cur_owner }));
        }
        if !live {
            return Ok(Err(JournalRefusal::LeaseExpired { fence }));
        }
        if let Some(epoch) = epoch {
            let c = tx.query_one("SELECT paused, revocation_epoch FROM control WHERE workspace_id=$1 FOR UPDATE", &[&ws]).await?;
            let (paused, cur_epoch): (bool, i64) = (c.get(0), c.get(1));
            if paused {
                return Ok(Err(JournalRefusal::Paused));
            }
            if cur_epoch != epoch {
                return Ok(Err(JournalRefusal::RevokedEpoch { presented: epoch, current: cur_epoch }));
            }
        }
        Ok(Ok(()))
    }

    /// Persist intent before the effect. Checks fence, pause and grant epoch
    /// in the same transaction.
    #[allow(clippy::too_many_arguments)]
    pub async fn begin_intent(
        &mut self,
        ws: &str,
        owner: &str,
        fence: i64,
        epoch: i64,
        request_id: &str,
        attempt_id: &str,
        p: &Prepared,
        before_hash: &str,
    ) -> Result<Result<String, JournalRefusal>, tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        if let Err(r) = Self::check_authority(&tx, ws, owner, fence, Some(epoch)).await? {
            tx.execute(
                "INSERT INTO diagnostic (workspace_id, kind, payload) VALUES ($1, 'intent_refused', $2)",
                &[&ws, &serde_json::to_value(&r).unwrap()],
            )
            .await?;
            tx.commit().await?;
            return Ok(Err(r));
        }
        let op_id = crate::util::new_id("op");
        tx.execute(
            "INSERT INTO op_intent (op_id, workspace_id, request_id, attempt_id, fence, revocation_epoch, tool, tool_version, schema_version, root_identity, args_hash, args, replay_identity, retry_class, reconcile_method, before_hash, intended_after_hash, state)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'mutating_non_idempotent','observe_target_hash',$14,$15,'in_flight')",
            &[&op_id, &ws, &request_id, &attempt_id, &fence, &epoch, &p.tool, &p.tool_version, &(p.schema_version as i32), &p.root_identity, &p.args_hash, &p.args, &p.replay_identity, &before_hash, &crate::broker::intended_hash(p)],
        )
        .await?;
        Self::event_tx(&tx, ws, Some(attempt_id), "tool_intent", &json!({"op_id": op_id, "fence": fence, "args_hash": p.args_hash, "replay_identity": p.replay_identity})).await?;
        tx.commit().await?;
        Ok(Ok(op_id))
    }

    /// Publish the outcome. Refused when the fence is no longer current.
    pub async fn complete(&mut self, ws: &str, owner: &str, fence: i64, op_id: &str, after_hash: &str) -> Result<Result<i64, JournalRefusal>, tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        if let Err(r) = Self::check_authority(&tx, ws, owner, fence, None).await? {
            tx.execute(
                "INSERT INTO diagnostic (workspace_id, kind, payload) VALUES ($1, 'stale_result_rejected', $2)",
                &[&ws, &json!({"op_id": op_id, "refusal": r, "after_hash": after_hash})],
            )
            .await?;
            tx.commit().await?;
            return Ok(Err(r));
        }
        tx.execute("UPDATE op_intent SET state='succeeded', after_hash=$2, settled_at=now() WHERE op_id=$1 AND state='in_flight'", &[&op_id, &after_hash]).await?;
        let seq = Self::event_tx(&tx, ws, None, "tool_settled", &json!({"op_id": op_id, "after_hash": after_hash, "fence": fence})).await?;
        tx.commit().await?;
        Ok(Ok(seq))
    }

    /// Recovery under a fresh lease. In-flight ops become unknown_effect with
    /// an observation of the target. Nothing is executed.
    pub async fn recover(&mut self, ws: &str, owner: &str, root: &std::path::Path) -> Result<Value, tokio_postgres::Error> {
        let lease = self.acquire_lease(ws, owner, 60_000).await?;
        let Some(lease) = lease else { return Ok(json!({"error": "lease held by a live owner"})) };
        let tx = self.c.transaction().await?;
        let rows = tx
            .query("SELECT op_id, args, before_hash, intended_after_hash, fence FROM op_intent WHERE workspace_id=$1 AND state='in_flight' ORDER BY created_at FOR UPDATE", &[&ws])
            .await?;
        let mut out = vec![];
        for r in rows {
            let op_id: String = r.get(0);
            let args: Value = r.get(1);
            let before: String = r.get(2);
            let intended: String = r.get(3);
            let path = args["path"].as_str().unwrap_or_default();
            let target = crate::broker::resolve_in_root(root, path).ok();
            let observed = target.as_deref().map(crate::broker::file_hash).unwrap_or_else(|| "unresolvable".into());
            let staging_left: Vec<String> = target
                .as_ref()
                .and_then(|t| t.parent().map(|d| d.to_path_buf()))
                .and_then(|d| std::fs::read_dir(d).ok())
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .filter(|n| n.contains(&format!("factory-staging-{op_id}")))
                        .collect()
                })
                .unwrap_or_default();
            let classification = if observed == intended {
                "target_matches_intended_after_hash"
            } else if observed == before {
                "target_unchanged_from_before_hash"
            } else {
                "target_differs_from_both"
            };
            let obs = json!({
                "observed_hash": observed, "before_hash": before, "intended_after_hash": intended,
                "classification": classification, "staging_files_left": staging_left,
                "auto_replay": false, "recovered_by": owner, "recovery_fence": lease.fence,
                "intent_fence": r.get::<_, i64>(4),
            });
            tx.execute("UPDATE op_intent SET state='unknown_effect', observation=$2 WHERE op_id=$1", &[&op_id, &obs]).await?;
            Self::event_tx(&tx, ws, None, "unknown_effect_recorded", &json!({"op_id": op_id, "observation": obs})).await?;
            out.push(json!({"op_id": op_id, "observation": obs}));
        }
        tx.commit().await?;
        Ok(json!({"lease": lease, "unknown_effects": out}))
    }

    /// Decide whether a prior op may be replayed under the current identity.
    pub async fn request_replay(&self, op_id: &str, current: &Prepared) -> Result<Value, tokio_postgres::Error> {
        let r = self
            .c
            .query_one("SELECT state, replay_identity, root_identity, tool_version, args_hash, after_hash FROM op_intent WHERE op_id=$1", &[&op_id])
            .await?;
        let state: String = r.get(0);
        let stored_identity: String = r.get(1);
        let mut mismatch = vec![];
        if r.get::<_, String>(2) != current.root_identity {
            mismatch.push("root_identity");
        }
        if r.get::<_, String>(3) != current.tool_version {
            mismatch.push("tool_version");
        }
        if r.get::<_, String>(4) != current.args_hash {
            mismatch.push("args_hash");
        }
        let decision = if state == "unknown_effect" || state == "in_flight" {
            json!({"decision": "blocked", "reason": format!("op is {state}, reconciliation required before any replay"), "executes": false})
        } else if stored_identity != current.replay_identity {
            json!({"decision": "refused", "reason": "replay_identity_mismatch", "changed": mismatch, "executes": false})
        } else if state == "succeeded" || state == "reconciled_applied" {
            json!({"decision": "already_applied", "receipt_after_hash": r.get::<_, Option<String>>(5), "executes": false})
        } else {
            json!({"decision": "refused", "reason": format!("state {state}"), "executes": false})
        };
        Ok(json!({"op_id": op_id, "state": state, "stored_identity": stored_identity, "presented_identity": current.replay_identity, "result": decision}))
    }

    pub async fn reconcile(&mut self, ws: &str, op_id: &str, actor: &str, decision: &str, after_hash: &str) -> Result<(), tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        tx.execute("UPDATE op_intent SET state=$2, after_hash=$3, settled_at=now() WHERE op_id=$1 AND state='unknown_effect'", &[&op_id, &decision, &after_hash]).await?;
        Self::event_tx(&tx, ws, None, "reconciled", &json!({"op_id": op_id, "actor": actor, "decision": decision, "after_hash": after_hash})).await?;
        tx.commit().await
    }

    pub async fn record_attempt(&mut self, ws: &str, attempt_id: &str, state: &str, generation: i64, outcome: &Value) -> Result<i64, tokio_postgres::Error> {
        let tx = self.c.transaction().await?;
        tx.execute(
            "INSERT INTO attempt (attempt_id, workspace_id, state, worker_generation, outcome) VALUES ($1,$2,$3,$4,$5)
             ON CONFLICT (attempt_id) DO UPDATE SET state=EXCLUDED.state, outcome=EXCLUDED.outcome, worker_generation=EXCLUDED.worker_generation, updated_at=now()",
            &[&attempt_id, &ws, &state, &generation, outcome],
        )
        .await?;
        let seq = Self::event_tx(&tx, ws, Some(attempt_id), &format!("attempt_{state}"), outcome).await?;
        tx.commit().await?;
        Ok(seq)
    }

    pub async fn bump_generation(&mut self, ws: &str) -> Result<i64, tokio_postgres::Error> {
        let r = self
            .c
            .query_one(
                "INSERT INTO supervisor (workspace_id, generation) VALUES ($1, 1) ON CONFLICT (workspace_id) DO UPDATE SET generation = supervisor.generation + 1 RETURNING generation",
                &[&ws],
            )
            .await?;
        Ok(r.get(0))
    }

    pub async fn diagnostic(&self, ws: &str, kind: &str, payload: &Value) -> Result<(), tokio_postgres::Error> {
        self.c.execute("INSERT INTO diagnostic (workspace_id, kind, payload) VALUES ($1,$2,$3)", &[&ws, &kind, payload]).await?;
        Ok(())
    }

    pub async fn events(&self, ws: &str) -> Result<Vec<Value>, tokio_postgres::Error> {
        let rows = self.c.query("SELECT seq, event_type, payload_hash FROM run_event WHERE workspace_id=$1 ORDER BY seq", &[&ws]).await?;
        Ok(rows.iter().map(|r| json!({"seq": r.get::<_, i64>(0), "type": r.get::<_, String>(1), "payload_hash": r.get::<_, String>(2)})).collect())
    }

    pub async fn ops(&self, ws: &str) -> Result<Vec<Value>, tokio_postgres::Error> {
        let rows = self.c.query("SELECT op_id, state, fence, after_hash, observation FROM op_intent WHERE workspace_id=$1 ORDER BY created_at", &[&ws]).await?;
        Ok(rows
            .iter()
            .map(|r| json!({"op_id": r.get::<_, String>(0), "state": r.get::<_, String>(1), "fence": r.get::<_, i64>(2), "after_hash": r.get::<_, Option<String>>(3), "observation": r.get::<_, Option<Value>>(4)}))
            .collect())
    }

    pub async fn attempt(&self, attempt_id: &str) -> Result<Option<Value>, tokio_postgres::Error> {
        let r = self.c.query_opt("SELECT state, worker_generation, outcome FROM attempt WHERE attempt_id=$1", &[&attempt_id]).await?;
        Ok(r.map(|r| json!({"state": r.get::<_, String>(0), "worker_generation": r.get::<_, i64>(1), "outcome": r.get::<_, Option<Value>>(2)})))
    }

    pub async fn server_version(&self) -> String {
        self.c
            .query_one("SHOW server_version", &[])
            .await
            .map(|r| r.get::<_, String>(0))
            .unwrap_or_default()
    }
}
