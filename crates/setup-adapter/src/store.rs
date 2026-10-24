//! PostgreSQL persistence: sessions, scans, analyses, plans, approvals, transaction projections
//! and C-04 style receipts.
//!
//! Every method opens a transaction through `TenantRepo::begin`, which sets `app.account_id`
//! first, so the row-level security of migrations 0400 and 0401 applies to every statement. The
//! queries also name the project explicitly, so they stay correct for a role that bypasses RLS.

use chrono::{DateTime, Utc};
use factory_accounts::{Database, RetryClass};
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::PgRow;
use uuid::Uuid;

use crate::error::SetupError;
use crate::ids::{AccountId, ProjectId};
use crate::pin::SETUP_PIN;

/// Who acts on which project. `owner` is the account whose directory holds the workspace.
#[derive(Debug, Clone, Copy)]
pub struct Scope {
    pub actor: AccountId,
    pub owner: AccountId,
    pub project: ProjectId,
}

/// The C-09 operations that leave a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupOperation {
    InspectProject,
    ConfirmSetupEvidence,
    AnalyzeSetup,
    ConfirmSetupAnalysis,
    PlanSetup,
    ResolveSetupConflicts,
    ApproveSetupPlan,
    ApplySetupPlan,
    RecoverSetup,
}

impl SetupOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InspectProject => "inspect_project",
            Self::ConfirmSetupEvidence => "confirm_setup_evidence",
            Self::AnalyzeSetup => "analyze_setup",
            Self::ConfirmSetupAnalysis => "confirm_setup_analysis",
            Self::PlanSetup => "plan_setup",
            Self::ResolveSetupConflicts => "resolve_setup_conflicts",
            Self::ApproveSetupPlan => "approve_setup_plan",
            Self::ApplySetupPlan => "apply_setup_plan",
            Self::RecoverSetup => "recover_setup",
        }
    }

    /// Effect class recorded in the receipt.
    pub fn effect_class(self) -> &'static str {
        match self {
            Self::InspectProject
            | Self::ConfirmSetupEvidence
            | Self::ConfirmSetupAnalysis
            | Self::PlanSetup
            | Self::ResolveSetupConflicts
            | Self::ApproveSetupPlan => "read_only",
            Self::AnalyzeSetup => "external_call",
            Self::ApplySetupPlan | Self::RecoverSetup => "project_write",
        }
    }

    /// How an unknown outcome is reconciled.
    pub fn reconciliation(self) -> &'static str {
        match self {
            Self::ApplySetupPlan | Self::RecoverSetup => {
                "read the native journal in the state root, then inspect_recovery"
            }
            Self::AnalyzeSetup => "repeat the analysis; nothing is applied by it",
            _ => "repeat; the operation changes no project file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Intent,
    Succeeded,
    Refused,
    Failed,
    UnknownEffect,
}

impl ReceiptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Succeeded => "succeeded",
            Self::Refused => "refused",
            Self::Failed => "failed",
            Self::UnknownEffect => "unknown_effect",
        }
    }
}

pub(crate) fn retry_text(class: RetryClass) -> &'static str {
    match class {
        RetryClass::SafeSameInputs => "safe_same_inputs",
        RetryClass::RefreshPreconditions => "refresh_preconditions",
        RetryClass::ReconcileFirst => "reconcile_first",
        RetryClass::RequiresAuthorization => "requires_authorization",
        RetryClass::Terminal => "terminal",
    }
}

/// Intent, written before the effect.
pub struct ReceiptIntent {
    pub id: Uuid,
    pub session_id: Option<Uuid>,
    pub operation: SetupOperation,
    pub request_id: Uuid,
    pub idempotency_key: Option<String>,
    pub request: Value,
    pub request_sha256: String,
    pub subject_id: Option<Uuid>,
    pub root_identity: Option<String>,
    pub grant_id: Option<String>,
    pub revocation_epoch: Option<i64>,
    pub fence_generation: Option<i64>,
    pub retry_class: RetryClass,
    pub started_at: DateTime<Utc>,
}

/// Outcome, written after the effect.
#[derive(Default)]
pub struct ReceiptOutcome {
    pub before_inventory_sha256: Option<String>,
    pub after_inventory_sha256: Option<String>,
    pub result: Option<Value>,
    pub result_sha256: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiptRow {
    pub id: Uuid,
    pub session_id: Option<Uuid>,
    pub operation: String,
    pub status: String,
    pub request_id: Uuid,
    pub idempotency_key: Option<String>,
    pub implementation: String,
    pub setup_revision: String,
    pub setup_has_pr105_fix: bool,
    pub request_sha256: String,
    pub subject_id: Option<Uuid>,
    pub root_identity: Option<String>,
    pub grant_id: Option<String>,
    pub fence_generation: Option<i64>,
    pub effect_class: String,
    pub retry_class: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub before_inventory_sha256: Option<String>,
    pub after_inventory_sha256: Option<String>,
    pub result: Option<Value>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

fn receipt_row(row: &PgRow) -> Result<ReceiptRow, sqlx::Error> {
    Ok(ReceiptRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        operation: row.try_get("operation")?,
        status: row.try_get("status")?,
        request_id: row.try_get("request_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        implementation: row.try_get("implementation")?,
        setup_revision: row.try_get("setup_revision")?,
        setup_has_pr105_fix: row.try_get("setup_has_pr105_fix")?,
        request_sha256: row.try_get("request_sha256")?,
        subject_id: row.try_get("subject_id")?,
        root_identity: row.try_get("root_identity")?,
        grant_id: row.try_get("grant_id")?,
        fence_generation: row.try_get("fence_generation")?,
        effect_class: row.try_get("effect_class")?,
        retry_class: row.try_get("retry_class")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
        before_inventory_sha256: row.try_get("before_inventory_sha256")?,
        after_inventory_sha256: row.try_get("after_inventory_sha256")?,
        result: row.try_get("result")?,
        error_code: row.try_get("error_code")?,
        error_message: row.try_get("error_message")?,
    })
}

const RECEIPT_COLUMNS: &str = "id, session_id, operation, status, request_id, idempotency_key, \
    implementation, setup_revision, setup_has_pr105_fix, request_sha256, subject_id, root_identity, \
    grant_id, fence_generation, effect_class, retry_class, started_at, finished_at, \
    before_inventory_sha256, after_inventory_sha256, result, error_code, error_message";

/// A scan row to insert.
pub struct ScanInsert {
    pub id: Uuid,
    pub session_id: Uuid,
    pub root_identity: String,
    pub mod_root: String,
    pub coverage: String,
    pub partial: bool,
    pub cancelled: bool,
    pub files_scanned: i64,
    pub directories_scanned: i64,
    pub bytes_read: i64,
    pub finding_count: i32,
    pub conflict_count: i32,
    pub semantic_analysis_required: bool,
    pub scan_sha256: String,
    pub result: Value,
    pub snapshot_before_sha256: Option<String>,
    pub snapshot_after_sha256: Option<String>,
    pub snapshot_unchanged: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub root_identity: String,
    pub coverage: String,
    pub partial: bool,
    pub cancelled: bool,
    pub finding_count: i32,
    pub conflict_count: i32,
    pub semantic_analysis_required: bool,
    pub snapshot_unchanged: Option<bool>,
    pub evidence_approved_at: Option<DateTime<Utc>>,
    pub evidence_count: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// A session row as the database holds it.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: Uuid,
    pub mod_root: String,
    pub client_state: Value,
    pub opened_at: DateTime<Utc>,
}

/// An analysis row to insert.
pub struct AnalysisInsert {
    pub id: Uuid,
    pub session_id: Uuid,
    pub scan_id: Option<Uuid>,
    pub mode: String,
    pub provider: String,
    pub model: String,
    pub reasoning_effort: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub source_revision: Option<String>,
    pub source_manifest_sha256: Option<String>,
    pub record: Value,
    pub proposals: Value,
    pub model_response_id: Option<String>,
    pub usage: Option<Value>,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub scan_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct PlanInsert {
    pub id: Uuid,
    pub session_id: Uuid,
    pub analysis_id: Option<Uuid>,
    pub scan_id: Option<Uuid>,
    pub plan_sha256: String,
    pub root_identity: String,
    pub plan: Value,
    pub operation_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub analysis_id: Option<Uuid>,
    pub scan_id: Option<Uuid>,
    pub plan_sha256: String,
    pub root_identity: String,
    pub operation_count: i32,
    pub conflict_decisions: Value,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ApprovalInsert {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub session_id: Uuid,
    pub reviewed_plan_sha256: String,
    pub approved_plan_sha256: String,
    pub root_identity: String,
    pub grant_id: Option<String>,
    pub revocation_epoch: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub approved_by_account_id: Uuid,
    pub reviewed_plan_sha256: String,
    pub approved_plan_sha256: String,
    pub root_identity: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

pub struct TransactionInsert {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub plan_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub mod_root: String,
    pub state: &'static str,
    pub plan_sha256: Option<String>,
    pub root_identity: Option<String>,
    pub pre_inventory_sha256: Option<String>,
    pub fence_generation: Option<i64>,
    pub holder: String,
    pub started_at: DateTime<Utc>,
}

/// The fields of a transaction row that follow the native journal.
#[derive(Default, Clone)]
pub struct TransactionUpdate {
    pub state: Option<&'static str>,
    pub journal_state: Option<String>,
    pub recovery_variant: Option<&'static str>,
    pub allowed_action: Option<&'static str>,
    pub project_apply_started: Option<bool>,
    pub journal_sha256: Option<String>,
    pub post_inventory_sha256: Option<String>,
    pub fence_generation: Option<i64>,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionRow {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub plan_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub mod_root: String,
    pub state: String,
    pub journal_state: Option<String>,
    pub recovery_variant: Option<String>,
    pub allowed_action: Option<String>,
    pub project_apply_started: Option<bool>,
    pub journal_sha256: Option<String>,
    pub plan_sha256: Option<String>,
    pub root_identity: Option<String>,
    pub pre_inventory_sha256: Option<String>,
    pub post_inventory_sha256: Option<String>,
    pub fence_generation: Option<i64>,
    pub holder: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

const TX_COLUMNS: &str = "id, journal_id, plan_id, session_id, mod_root, state, journal_state, \
    recovery_variant, allowed_action, project_apply_started, journal_sha256, plan_sha256, \
    root_identity, pre_inventory_sha256, post_inventory_sha256, fence_generation, holder, \
    started_at, updated_at, finished_at";

fn tx_row(row: &PgRow) -> Result<TransactionRow, sqlx::Error> {
    Ok(TransactionRow {
        id: row.try_get("id")?,
        journal_id: row.try_get("journal_id")?,
        plan_id: row.try_get("plan_id")?,
        session_id: row.try_get("session_id")?,
        mod_root: row.try_get("mod_root")?,
        state: row.try_get("state")?,
        journal_state: row.try_get("journal_state")?,
        recovery_variant: row.try_get("recovery_variant")?,
        allowed_action: row.try_get("allowed_action")?,
        project_apply_started: row.try_get("project_apply_started")?,
        journal_sha256: row.try_get("journal_sha256")?,
        plan_sha256: row.try_get("plan_sha256")?,
        root_identity: row.try_get("root_identity")?,
        pre_inventory_sha256: row.try_get("pre_inventory_sha256")?,
        post_inventory_sha256: row.try_get("post_inventory_sha256")?,
        fence_generation: row.try_get("fence_generation")?,
        holder: row.try_get("holder")?,
        started_at: row.try_get("started_at")?,
        updated_at: row.try_get("updated_at")?,
        finished_at: row.try_get("finished_at")?,
    })
}

/// Persistence for one database.
#[derive(Clone)]
pub struct Store {
    db: Database,
}

impl Store {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.db.clock().now()
    }

    // ----- sessions ---------------------------------------------------------------------

    pub async fn insert_session(
        &self,
        scope: &Scope,
        id: Uuid,
        mod_root: &str,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "INSERT INTO setup_sessions (id, project_id, owner_account_id, opened_by_account_id,
                 setup_repository, setup_revision, setup_has_pr105_fix, setup_default_plan_builder,
                 mod_root, opened_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(scope.actor.0)
        .bind(SETUP_PIN.repository)
        .bind(SETUP_PIN.revision)
        .bind(SETUP_PIN.contains_pr_105_fix)
        .bind(SETUP_PIN.default_plan_builder)
        .bind(mod_root)
        .bind(self.now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn close_session(&self, scope: &Scope, id: Uuid) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_sessions SET state = 'closed', closed_at = $3
             WHERE id = $1 AND project_id = $2 AND state = 'open'",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(self.now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The newest session this actor left open on the project, as the database holds it. After a
    /// restart the row exists but the facade session does not (see [`SessionRow`]).
    pub async fn latest_open_session(
        &self,
        scope: &Scope,
    ) -> Result<Option<SessionRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, mod_root, client_state, opened_at FROM setup_sessions
             WHERE project_id = $1 AND opened_by_account_id = $2 AND state = 'open'
             ORDER BY opened_at DESC LIMIT 1",
        )
        .bind(scope.project.0)
        .bind(scope.actor.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(SessionRow {
                id: r.try_get("id")?,
                mod_root: r.try_get("mod_root")?,
                client_state: r.try_get("client_state")?,
                opened_at: r.try_get("opened_at")?,
            })
        })
        .transpose()
        .map_err(|e: sqlx::Error| e.into())
    }

    /// Replaces the bounded, non-secret intent kept with the session.
    pub async fn update_session_state(
        &self,
        scope: &Scope,
        id: Uuid,
        client_state: &Value,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_sessions SET client_state = $3
             WHERE id = $1 AND project_id = $2 AND state = 'open'",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(client_state)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Mod root of the most recent unfinished transaction, else of the most recent session.
    pub async fn latest_mod_root(&self, scope: &Scope) -> Result<Option<String>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let from_tx: Option<String> = sqlx::query_scalar(
            "SELECT mod_root FROM setup_transactions WHERE project_id = $1
             ORDER BY (state IN ('applying','recovery_required','recovering','unknown')) DESC, started_at DESC
             LIMIT 1",
        )
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        let root = match from_tx {
            Some(r) => Some(r),
            None => {
                sqlx::query_scalar(
                    "SELECT mod_root FROM setup_sessions WHERE project_id = $1
                     ORDER BY opened_at DESC LIMIT 1",
                )
                .bind(scope.project.0)
                .fetch_optional(&mut *tx)
                .await?
            }
        };
        tx.commit().await?;
        Ok(root)
    }

    // ----- receipts ---------------------------------------------------------------------

    /// Persist the intent. A reused idempotency key is `IdempotencyReplay`.
    pub async fn insert_receipt(
        &self,
        scope: &Scope,
        intent: &ReceiptIntent,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let result = sqlx::query(
            "INSERT INTO setup_receipts (id, session_id, project_id, owner_account_id, actor_account_id,
                 operation, request_id, idempotency_key, implementation, setup_revision,
                 setup_has_pr105_fix, request, request_sha256, subject_id, root_identity, grant_id,
                 revocation_epoch, fence_generation, effect_class, retry_class, reconciliation,
                 started_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)",
        )
        .bind(intent.id)
        .bind(intent.session_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(scope.actor.0)
        .bind(intent.operation.as_str())
        .bind(intent.request_id)
        .bind(&intent.idempotency_key)
        .bind(SETUP_PIN.short())
        .bind(SETUP_PIN.revision)
        .bind(SETUP_PIN.contains_pr_105_fix)
        .bind(&intent.request)
        .bind(&intent.request_sha256)
        .bind(intent.subject_id)
        .bind(&intent.root_identity)
        .bind(&intent.grant_id)
        .bind(intent.revocation_epoch)
        .bind(intent.fence_generation)
        .bind(intent.operation.effect_class())
        .bind(retry_text(intent.retry_class))
        .bind(intent.operation.reconciliation())
        .bind(intent.started_at)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
                drop(tx);
                let prior = self
                    .receipt_by_key(scope, intent.idempotency_key.as_deref().unwrap_or_default())
                    .await?;
                return Err(SetupError::IdempotencyReplay {
                    receipt_id: prior.map(|p| p.id).unwrap_or(intent.id),
                });
            }
            Err(e) => return Err(e.into()),
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn settle_receipt(
        &self,
        scope: &Scope,
        id: Uuid,
        status: ReceiptStatus,
        outcome: &ReceiptOutcome,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_receipts SET status = $3, finished_at = $4, before_inventory_sha256 = $5,
                 after_inventory_sha256 = $6, result = $7, result_sha256 = $8, error_code = $9,
                 error_message = $10
             WHERE id = $1 AND project_id = $2 AND status = 'intent'",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(status.as_str())
        .bind(self.now())
        .bind(&outcome.before_inventory_sha256)
        .bind(&outcome.after_inventory_sha256)
        .bind(&outcome.result)
        .bind(&outcome.result_sha256)
        .bind(&outcome.error_code)
        .bind(&outcome.error_message)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn receipt_by_key(
        &self,
        scope: &Scope,
        key: &str,
    ) -> Result<Option<ReceiptRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM setup_receipts WHERE project_id = $1 AND idempotency_key = $2"
        ))
        .bind(scope.project.0)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row.as_ref().map(receipt_row).transpose()?)
    }

    pub async fn list_receipts(
        &self,
        scope: &Scope,
        limit: i64,
    ) -> Result<Vec<ReceiptRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let rows = sqlx::query(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM setup_receipts WHERE project_id = $1
             ORDER BY started_at DESC, id LIMIT $2"
        ))
        .bind(scope.project.0)
        .bind(limit.clamp(1, 500))
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.iter().map(receipt_row).collect::<Result<_, _>>()?)
    }

    // ----- scans ------------------------------------------------------------------------

    pub async fn insert_scan(&self, scope: &Scope, scan: &ScanInsert) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "INSERT INTO setup_scans (id, session_id, project_id, owner_account_id, root_identity,
                 mod_root, coverage, partial, cancelled, files_scanned, directories_scanned,
                 bytes_read, finding_count, conflict_count, semantic_analysis_required, scan_sha256,
                 result, snapshot_before_sha256, snapshot_after_sha256, snapshot_unchanged, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
        )
        .bind(scan.id)
        .bind(scan.session_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(&scan.root_identity)
        .bind(&scan.mod_root)
        .bind(&scan.coverage)
        .bind(scan.partial)
        .bind(scan.cancelled)
        .bind(scan.files_scanned)
        .bind(scan.directories_scanned)
        .bind(scan.bytes_read)
        .bind(scan.finding_count)
        .bind(scan.conflict_count)
        .bind(scan.semantic_analysis_required)
        .bind(&scan.scan_sha256)
        .bind(&scan.result)
        .bind(&scan.snapshot_before_sha256)
        .bind(&scan.snapshot_after_sha256)
        .bind(scan.snapshot_unchanged)
        .bind(scan.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_evidence_approved(
        &self,
        scope: &Scope,
        scan_id: Uuid,
        count: i32,
        evidence_sha256: &str,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_scans SET evidence_approved_at = $3, evidence_count = $4, evidence_sha256 = $5
             WHERE id = $1 AND project_id = $2",
        )
        .bind(scan_id)
        .bind(scope.project.0)
        .bind(self.now())
        .bind(count)
        .bind(evidence_sha256)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn latest_scan(
        &self,
        scope: &Scope,
        session: Option<Uuid>,
    ) -> Result<Option<ScanRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, session_id, root_identity, coverage, partial, cancelled, finding_count,
                    conflict_count, semantic_analysis_required, snapshot_unchanged,
                    evidence_approved_at, evidence_count, created_at
             FROM setup_scans WHERE project_id = $1 AND ($2::uuid IS NULL OR session_id = $2)
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(scope.project.0)
        .bind(session)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(ScanRow {
                id: r.try_get("id")?,
                session_id: r.try_get("session_id")?,
                root_identity: r.try_get("root_identity")?,
                coverage: r.try_get("coverage")?,
                partial: r.try_get("partial")?,
                cancelled: r.try_get("cancelled")?,
                finding_count: r.try_get("finding_count")?,
                conflict_count: r.try_get("conflict_count")?,
                semantic_analysis_required: r.try_get("semantic_analysis_required")?,
                snapshot_unchanged: r.try_get("snapshot_unchanged")?,
                evidence_approved_at: r.try_get("evidence_approved_at")?,
                evidence_count: r.try_get("evidence_count")?,
                created_at: r.try_get("created_at")?,
            })
        })
        .transpose()
        .map_err(|e: sqlx::Error| e.into())
    }

    // ----- analyses ---------------------------------------------------------------------

    pub async fn insert_analysis(
        &self,
        scope: &Scope,
        a: &AnalysisInsert,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "INSERT INTO setup_analyses (id, session_id, scan_id, project_id, owner_account_id, mode,
                 provider, model, reasoning_effort, input_sha256, output_sha256, source_revision,
                 source_manifest_sha256, record, proposals, model_response_id, usage, attempts, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",
        )
        .bind(a.id)
        .bind(a.session_id)
        .bind(a.scan_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(&a.mode)
        .bind(&a.provider)
        .bind(&a.model)
        .bind(&a.reasoning_effort)
        .bind(&a.input_sha256)
        .bind(&a.output_sha256)
        .bind(&a.source_revision)
        .bind(&a.source_manifest_sha256)
        .bind(&a.record)
        .bind(&a.proposals)
        .bind(&a.model_response_id)
        .bind(&a.usage)
        .bind(a.attempts)
        .bind(a.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_analysis_confirmed(
        &self,
        scope: &Scope,
        id: Uuid,
        confirmed_fields: &Value,
        confirmed_values_sha256: &str,
        record: &Value,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_analyses SET status = 'confirmed', confirmed_fields = $3,
                 confirmed_values_sha256 = $4, confirmed_at = $5, record = $6
             WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(confirmed_fields)
        .bind(confirmed_values_sha256)
        .bind(self.now())
        .bind(record)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn analysis(
        &self,
        scope: &Scope,
        id: Uuid,
    ) -> Result<Option<AnalysisRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, session_id, scan_id, provider, model, status, input_sha256, output_sha256,
                    confirmed_at, created_at
             FROM setup_analyses WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(AnalysisRow {
                id: r.try_get("id")?,
                session_id: r.try_get("session_id")?,
                scan_id: r.try_get("scan_id")?,
                provider: r.try_get("provider")?,
                model: r.try_get("model")?,
                status: r.try_get("status")?,
                input_sha256: r.try_get("input_sha256")?,
                output_sha256: r.try_get("output_sha256")?,
                confirmed_at: r.try_get("confirmed_at")?,
                created_at: r.try_get("created_at")?,
            })
        })
        .transpose()
        .map_err(|e: sqlx::Error| e.into())
    }

    // ----- plans and approvals -----------------------------------------------------------

    pub async fn insert_plan(&self, scope: &Scope, p: &PlanInsert) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "INSERT INTO setup_plans (id, session_id, analysis_id, scan_id, project_id, owner_account_id,
                 plan_sha256, root_identity, plan, operation_count, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)",
        )
        .bind(p.id)
        .bind(p.session_id)
        .bind(p.analysis_id)
        .bind(p.scan_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(&p.plan_sha256)
        .bind(&p.root_identity)
        .bind(&p.plan)
        .bind(p.operation_count)
        .bind(p.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// A conflict decision: new plan hash and JSON, decisions recorded, plan back to `built`,
    /// every active approval withdrawn.
    pub async fn record_conflict_decision(
        &self,
        scope: &Scope,
        plan_id: Uuid,
        plan_sha256: &str,
        plan: &Value,
        decisions: &Value,
    ) -> Result<(), SetupError> {
        let now = self.now();
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_plans SET plan_sha256 = $3, plan = $4, conflict_decisions = $5,
                 state = 'built', updated_at = $6
             WHERE id = $1 AND project_id = $2",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .bind(plan_sha256)
        .bind(plan)
        .bind(decisions)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE setup_approvals SET state = 'withdrawn', withdrawn_reason = 'conflict decision changed the plan',
                 withdrawn_at = $3
             WHERE plan_id = $1 AND project_id = $2 AND state = 'active'",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn insert_approval(
        &self,
        scope: &Scope,
        a: &ApprovalInsert,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        // A new approval replaces any live one for the same plan.
        sqlx::query(
            "UPDATE setup_approvals SET state = 'withdrawn', withdrawn_reason = 'superseded by a new approval',
                 withdrawn_at = $3
             WHERE plan_id = $1 AND project_id = $2 AND state = 'active'",
        )
        .bind(a.plan_id)
        .bind(scope.project.0)
        .bind(a.created_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO setup_approvals (id, plan_id, session_id, project_id, owner_account_id,
                 approved_by_account_id, reviewed_plan_sha256, approved_plan_sha256, root_identity,
                 grant_id, revocation_epoch, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(a.id)
        .bind(a.plan_id)
        .bind(a.session_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(scope.actor.0)
        .bind(&a.reviewed_plan_sha256)
        .bind(&a.approved_plan_sha256)
        .bind(&a.root_identity)
        .bind(&a.grant_id)
        .bind(a.revocation_epoch)
        .bind(a.created_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE setup_plans SET state = 'approved', updated_at = $3 WHERE id = $1 AND project_id = $2",
        )
        .bind(a.plan_id)
        .bind(scope.project.0)
        .bind(a.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Consume the live approval and set the plan state, in one transaction.
    pub async fn consume_approval(
        &self,
        scope: &Scope,
        plan_id: Uuid,
        plan_state: &str,
    ) -> Result<(), SetupError> {
        let now = self.now();
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_approvals SET state = 'consumed', consumed_at = $3
             WHERE plan_id = $1 AND project_id = $2 AND state = 'active'",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE setup_plans SET state = $3, updated_at = $4 WHERE id = $1 AND project_id = $2",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .bind(plan_state)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_plan_state(
        &self,
        scope: &Scope,
        plan_id: Uuid,
        state: &str,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_plans SET state = $3, updated_at = $4 WHERE id = $1 AND project_id = $2",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .bind(state)
        .bind(self.now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn plan(&self, scope: &Scope, id: Uuid) -> Result<Option<PlanRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, session_id, analysis_id, scan_id, plan_sha256, root_identity, operation_count,
                    conflict_decisions, state, created_at, updated_at
             FROM setup_plans WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(|r| plan_row(&r)).transpose().map_err(Into::into)
    }

    pub async fn latest_plan(
        &self,
        scope: &Scope,
        session: Option<Uuid>,
    ) -> Result<Option<(PlanRow, Value)>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, session_id, analysis_id, scan_id, plan_sha256, root_identity, operation_count,
                    conflict_decisions, state, created_at, updated_at, plan
             FROM setup_plans WHERE project_id = $1 AND ($2::uuid IS NULL OR session_id = $2)
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(scope.project.0)
        .bind(session)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        match row {
            Some(r) => {
                let plan: Value = r.try_get("plan")?;
                Ok(Some((plan_row(&r)?, plan)))
            }
            None => Ok(None),
        }
    }

    /// The stored (redacted) plan JSON of one plan.
    pub async fn latest_plan_by_id(
        &self,
        scope: &Scope,
        id: Uuid,
    ) -> Result<Option<Value>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let plan: Option<Value> =
            sqlx::query_scalar("SELECT plan FROM setup_plans WHERE id = $1 AND project_id = $2")
                .bind(id)
                .bind(scope.project.0)
                .fetch_optional(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(plan)
    }

    /// Source manifest hash of the newest analysis of the project.
    pub async fn latest_manifest_hash(&self, scope: &Scope) -> Result<Option<String>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let hash: Option<Option<String>> = sqlx::query_scalar(
            "SELECT source_manifest_sha256 FROM setup_analyses WHERE project_id = $1
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(hash.flatten())
    }

    pub async fn active_approval(
        &self,
        scope: &Scope,
        plan_id: Uuid,
    ) -> Result<Option<ApprovalRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(
            "SELECT id, plan_id, approved_by_account_id, reviewed_plan_sha256, approved_plan_sha256,
                    root_identity, state, created_at
             FROM setup_approvals WHERE plan_id = $1 AND project_id = $2 AND state = 'active'",
        )
        .bind(plan_id)
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(ApprovalRow {
                id: r.try_get("id")?,
                plan_id: r.try_get("plan_id")?,
                approved_by_account_id: r.try_get("approved_by_account_id")?,
                reviewed_plan_sha256: r.try_get("reviewed_plan_sha256")?,
                approved_plan_sha256: r.try_get("approved_plan_sha256")?,
                root_identity: r.try_get("root_identity")?,
                state: r.try_get("state")?,
                created_at: r.try_get("created_at")?,
            })
        })
        .transpose()
        .map_err(|e: sqlx::Error| e.into())
    }

    // ----- transactions -----------------------------------------------------------------

    pub async fn insert_transaction(
        &self,
        scope: &Scope,
        t: &TransactionInsert,
    ) -> Result<(), SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let result = sqlx::query(
            "INSERT INTO setup_transactions (id, journal_id, plan_id, session_id, project_id,
                 owner_account_id, mod_root, state, plan_sha256, root_identity, pre_inventory_sha256,
                 fence_generation, holder, started_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$14)",
        )
        .bind(t.id)
        .bind(t.journal_id)
        .bind(t.plan_id)
        .bind(t.session_id)
        .bind(scope.project.0)
        .bind(scope.owner.0)
        .bind(&t.mod_root)
        .bind(t.state)
        .bind(&t.plan_sha256)
        .bind(&t.root_identity)
        .bind(&t.pre_inventory_sha256)
        .bind(t.fence_generation)
        .bind(&t.holder)
        .bind(t.started_at)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            // The partial unique index: another transaction is unfinished on this project.
            Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
                return Err(SetupError::ProjectBusy {
                    project: scope.project,
                });
            }
            Err(e) => return Err(e.into()),
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_transaction(
        &self,
        scope: &Scope,
        id: Uuid,
        u: &TransactionUpdate,
    ) -> Result<(), SetupError> {
        let now = self.now();
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        sqlx::query(
            "UPDATE setup_transactions SET
                 state = COALESCE($3, state),
                 journal_state = COALESCE($4, journal_state),
                 recovery_variant = $5,
                 allowed_action = $6,
                 project_apply_started = COALESCE($7, project_apply_started),
                 journal_sha256 = COALESCE($8, journal_sha256),
                 post_inventory_sha256 = COALESCE($9, post_inventory_sha256),
                 fence_generation = COALESCE($10, fence_generation),
                 updated_at = $11,
                 finished_at = CASE WHEN $12 THEN $11 ELSE finished_at END
             WHERE id = $1 AND project_id = $2",
        )
        .bind(id)
        .bind(scope.project.0)
        .bind(u.state)
        .bind(&u.journal_state)
        .bind(u.recovery_variant)
        .bind(u.allowed_action)
        .bind(u.project_apply_started)
        .bind(&u.journal_sha256)
        .bind(&u.post_inventory_sha256)
        .bind(u.fence_generation)
        .bind(now)
        .bind(u.finished)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The unfinished transaction row of a project, if any.
    pub async fn unfinished_transaction(
        &self,
        scope: &Scope,
    ) -> Result<Option<TransactionRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(&format!(
            "SELECT {TX_COLUMNS} FROM setup_transactions WHERE project_id = $1
             AND state IN ('applying','recovery_required','recovering','unknown')"
        ))
        .bind(scope.project.0)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row.as_ref().map(tx_row).transpose()?)
    }

    pub async fn transaction_by_journal(
        &self,
        scope: &Scope,
        journal_id: Uuid,
    ) -> Result<Option<TransactionRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let row = sqlx::query(&format!(
            "SELECT {TX_COLUMNS} FROM setup_transactions WHERE project_id = $1 AND journal_id = $2"
        ))
        .bind(scope.project.0)
        .bind(journal_id)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row.as_ref().map(tx_row).transpose()?)
    }

    pub async fn list_transactions(
        &self,
        scope: &Scope,
        limit: i64,
    ) -> Result<Vec<TransactionRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let rows = sqlx::query(&format!(
            "SELECT {TX_COLUMNS} FROM setup_transactions WHERE project_id = $1
             ORDER BY started_at DESC LIMIT $2"
        ))
        .bind(scope.project.0)
        .bind(limit.clamp(1, 200))
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.iter().map(tx_row).collect::<Result<_, _>>()?)
    }

    /// Receipts that recorded an intent and never settled.
    pub async fn unsettled_receipts(&self, scope: &Scope) -> Result<Vec<ReceiptRow>, SetupError> {
        let mut tx = self.db.scoped(scope.actor).begin().await?;
        let rows = sqlx::query(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM setup_receipts WHERE project_id = $1 AND status = 'intent'
             ORDER BY started_at"
        ))
        .bind(scope.project.0)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.iter().map(receipt_row).collect::<Result<_, _>>()?)
    }
}

fn plan_row(r: &PgRow) -> Result<PlanRow, sqlx::Error> {
    Ok(PlanRow {
        id: r.try_get("id")?,
        session_id: r.try_get("session_id")?,
        analysis_id: r.try_get("analysis_id")?,
        scan_id: r.try_get("scan_id")?,
        plan_sha256: r.try_get("plan_sha256")?,
        root_identity: r.try_get("root_identity")?,
        operation_count: r.try_get("operation_count")?,
        conflict_decisions: r.try_get("conflict_decisions")?,
        state: r.try_get("state")?,
        created_at: r.try_get("created_at")?,
        updated_at: r.try_get("updated_at")?,
    })
}
