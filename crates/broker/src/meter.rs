//! Metering and budget stops.
//!
//! Every model call reserves its conservative cost before it is made ([`Meter::reserve`]) and
//! settles afterwards ([`Meter::settle`]). Spend counts a reservation's reserved amount until it
//! settles, a settled call's actual amount, and a `usage_unknown` call's reserved amount: cost
//! unknowns stay unknown and are never counted as zero. A released reservation (a request known
//! not to have been accepted) counts nothing.
//!
//! Two budgets are checked inside one transaction, serialized per account with an advisory
//! transaction lock (and per task, always in that order), so concurrent reservations cannot
//! overspend:
//!
//! * the task budget counts the task's spend under every funding source;
//! * the operator allowance counts only `operator_allowance` spend in the calendar month (UTC),
//!   and needs a verified account (ADR-021). Usage paid with the user's own key never counts
//!   against it, and operator spend never counts against a user key.
//!
//! The task total is read through row-level security, which shows only the paying account's
//! rows, so a task must be funded by one account.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use factory_accounts::{AccountId, ProjectId};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::ids::{ReservationId, UserKeyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Funding {
    OperatorAllowance,
    UserKey { key_id: UserKeyId },
}

impl Funding {
    fn as_str(self) -> &'static str {
        match self {
            Funding::OperatorAllowance => "operator_allowance",
            Funding::UserKey { .. } => "user_key",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reservation {
    pub account: AccountId,
    /// Whether the account's email is verified. The operator allowance requires it.
    pub account_verified: bool,
    pub project: ProjectId,
    pub task: Uuid,
    pub attempt: Uuid,
    pub funding: Funding,
    pub provider: String,
    pub model: String,
    pub reserve_micros: i64,
    pub task_budget_micros: i64,
    /// The monthly operator allowance ($1 = 1_000_000 micros per verified user per month).
    pub allowance_micros: i64,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "stop", rename_all = "snake_case")]
pub enum BudgetStop {
    #[error("task budget exhausted: {spent} of {budget} micros spent")]
    TaskBudgetExhausted { spent: i64, budget: i64 },
    #[error("operator allowance exhausted: {spent} of {allowance} micros spent this month")]
    AllowanceExhausted { spent: i64, allowance: i64 },
    #[error("the operator allowance needs a verified email address")]
    AccountNotVerified,
}

#[derive(Debug, thiserror::Error)]
pub enum MeterError {
    #[error(transparent)]
    Stop(#[from] BudgetStop),
    #[error("the user key is not available for this account")]
    UserKeyUnavailable,
    #[error("invalid metering request: {0}")]
    Invalid(String),
    #[error("reservation not found")]
    NotFound,
    #[error("reservation is already {0:?}")]
    AlreadySettled(UsageStatus),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Reserved,
    Settled,
    UsageUnknown,
    Released,
}

impl UsageStatus {
    fn parse(value: &str) -> Option<UsageStatus> {
        match value {
            "reserved" => Some(UsageStatus::Reserved),
            "settled" => Some(UsageStatus::Settled),
            "usage_unknown" => Some(UsageStatus::UsageUnknown),
            "released" => Some(UsageStatus::Released),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Settlement {
    Known {
        actual_micros: i64,
        usage: Value,
    },
    /// The call may have been served but its usage is missing. The reserved amount stays spent.
    Unknown {
        reason: String,
    },
    /// The request is known not to have been accepted.
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettleOutcome {
    pub status: UsageStatus,
    /// What this call now counts as spend.
    pub counted_micros: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageSummary {
    pub allowance_micros: i64,
    pub operator_spent_micros: i64,
    pub operator_unknown_count: i64,
    pub user_key_spent_micros: i64,
    pub remaining_allowance_micros: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskUsage {
    /// Everything the task counts as spent: settled + reserved + unknown.
    pub spent_micros: i64,
    pub settled_micros: i64,
    /// Reserved and not yet settled.
    pub in_flight_micros: i64,
    pub unknown_micros: i64,
    pub unknown_count: i64,
    pub calls: i64,
}

/// First day of the UTC month containing `now`.
pub fn month_start(now: DateTime<Utc>) -> NaiveDate {
    NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap_or(NaiveDate::MIN)
}

/// What a row counts as spent, by status.
const SPEND: &str = "CASE status WHEN 'settled' THEN COALESCE(actual_micros, 0) \
                     WHEN 'released' THEN 0 ELSE reserved_micros END";

#[derive(Debug, Clone, Copy, Default)]
pub struct Meter;

impl Meter {
    pub fn new() -> Self {
        Self
    }

    /// Reserves `reserve_micros` or stops with the budget that would be exceeded. Equality is
    /// allowed: spend plus the reservation may equal the budget.
    pub async fn reserve(
        &self,
        conn: &mut PgConnection,
        r: Reservation,
    ) -> Result<ReservationId, MeterError> {
        if r.reserve_micros < 0 || r.task_budget_micros < 0 || r.allowance_micros < 0 {
            return Err(MeterError::Invalid("amounts cannot be negative".into()));
        }
        if r.provider.is_empty()
            || r.provider.len() > 64
            || r.model.is_empty()
            || r.model.len() > 128
        {
            return Err(MeterError::Invalid(
                "provider or model is out of bounds".into(),
            ));
        }
        // Serialize reservations: account first, then task, always in this order.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("model_usage:account:{}", r.account))
            .execute(&mut *conn)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("model_usage:task:{}", r.task))
            .execute(&mut *conn)
            .await?;

        let user_key_id = match r.funding {
            Funding::OperatorAllowance => {
                if !r.account_verified {
                    return Err(BudgetStop::AccountNotVerified.into());
                }
                None
            }
            Funding::UserKey { key_id } => {
                let usable: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM user_api_keys \
                     WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL)",
                )
                .bind(key_id.0)
                .bind(r.account.0)
                .fetch_one(&mut *conn)
                .await?;
                if !usable {
                    return Err(MeterError::UserKeyUnavailable);
                }
                Some(key_id.0)
            }
        };

        let task_spent: i64 = sqlx::query_scalar(&format!(
            "SELECT COALESCE(SUM({SPEND}), 0)::bigint FROM model_usage WHERE task_id = $1"
        ))
        .bind(r.task)
        .fetch_one(&mut *conn)
        .await?;
        if task_spent.saturating_add(r.reserve_micros) > r.task_budget_micros {
            return Err(BudgetStop::TaskBudgetExhausted {
                spent: task_spent,
                budget: r.task_budget_micros,
            }
            .into());
        }

        let month = month_start(r.now);
        if matches!(r.funding, Funding::OperatorAllowance) {
            let spent: i64 = sqlx::query_scalar(&format!(
                "SELECT COALESCE(SUM({SPEND}), 0)::bigint FROM model_usage \
                 WHERE account_id = $1 AND period_month = $2 AND funding = 'operator_allowance'"
            ))
            .bind(r.account.0)
            .bind(month)
            .fetch_one(&mut *conn)
            .await?;
            if spent.saturating_add(r.reserve_micros) > r.allowance_micros {
                return Err(BudgetStop::AllowanceExhausted {
                    spent,
                    allowance: r.allowance_micros,
                }
                .into());
            }
        }

        let id = ReservationId::new();
        sqlx::query(
            "INSERT INTO model_usage (id, account_id, project_id, task_id, attempt_id, funding, \
             user_key_id, provider, model, period_month, reserved_micros, status, created_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'reserved',$12)",
        )
        .bind(id.0)
        .bind(r.account.0)
        .bind(r.project.0)
        .bind(r.task)
        .bind(r.attempt)
        .bind(r.funding.as_str())
        .bind(user_key_id)
        .bind(&r.provider)
        .bind(&r.model)
        .bind(month)
        .bind(r.reserve_micros)
        .bind(r.now)
        .execute(&mut *conn)
        .await?;
        Ok(id)
    }

    /// Settles a reservation. A reservation settles once; settling it again is an error.
    pub async fn settle(
        &self,
        conn: &mut PgConnection,
        id: ReservationId,
        settlement: Settlement,
        now: DateTime<Utc>,
    ) -> Result<SettleOutcome, MeterError> {
        let (status, actual, usage): (&str, Option<i64>, Option<Value>) = match settlement {
            Settlement::Known {
                actual_micros,
                usage,
            } => {
                if actual_micros < 0 {
                    return Err(MeterError::Invalid("actual cost cannot be negative".into()));
                }
                ("settled", Some(actual_micros), Some(usage))
            }
            Settlement::Unknown { reason } => {
                let reason: String = reason.chars().take(500).collect();
                (
                    "usage_unknown",
                    None,
                    Some(serde_json::json!({ "unknown_reason": reason })),
                )
            }
            Settlement::Released => ("released", None, None),
        };
        let row = sqlx::query(
            "UPDATE model_usage SET status = $2, actual_micros = $3, usage = $4, settled_at = $5 \
             WHERE id = $1 AND status = 'reserved' RETURNING reserved_micros",
        )
        .bind(id.0)
        .bind(status)
        .bind(actual)
        .bind(usage)
        .bind(now)
        .fetch_optional(&mut *conn)
        .await?;
        match row {
            Some(row) => {
                let reserved: i64 = row.try_get("reserved_micros")?;
                let new_status = UsageStatus::parse(status).unwrap_or(UsageStatus::Settled);
                let counted_micros = match new_status {
                    UsageStatus::Settled => actual.unwrap_or(0),
                    UsageStatus::Released => 0,
                    _ => reserved,
                };
                Ok(SettleOutcome {
                    status: new_status,
                    counted_micros,
                })
            }
            None => {
                let current: Option<String> =
                    sqlx::query_scalar("SELECT status FROM model_usage WHERE id = $1")
                        .bind(id.0)
                        .fetch_optional(&mut *conn)
                        .await?;
                match current.as_deref().and_then(UsageStatus::parse) {
                    Some(status) => Err(MeterError::AlreadySettled(status)),
                    None => Err(MeterError::NotFound),
                }
            }
        }
    }

    /// The account's usage for the month starting at `month`, against `allowance_micros`.
    pub async fn account_month(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
        month: NaiveDate,
        allowance_micros: i64,
    ) -> Result<UsageSummary, MeterError> {
        let rows = sqlx::query(&format!(
            "SELECT funding, COALESCE(SUM({SPEND}), 0)::bigint AS spent, \
             COUNT(*) FILTER (WHERE status = 'usage_unknown')::bigint AS unknown \
             FROM model_usage WHERE account_id = $1 AND period_month = $2 GROUP BY funding"
        ))
        .bind(account.0)
        .bind(month)
        .fetch_all(&mut *conn)
        .await?;
        let mut summary = UsageSummary {
            allowance_micros,
            operator_spent_micros: 0,
            operator_unknown_count: 0,
            user_key_spent_micros: 0,
            remaining_allowance_micros: allowance_micros,
        };
        for row in rows {
            let funding: String = row.try_get("funding")?;
            let spent: i64 = row.try_get("spent")?;
            if funding == "operator_allowance" {
                summary.operator_spent_micros = spent;
                summary.operator_unknown_count = row.try_get("unknown")?;
            } else {
                summary.user_key_spent_micros = spent;
            }
        }
        summary.remaining_allowance_micros =
            (allowance_micros - summary.operator_spent_micros).max(0);
        Ok(summary)
    }

    /// What a task has spent, from the rows the current account can see.
    pub async fn task_usage(
        &self,
        conn: &mut PgConnection,
        task: Uuid,
    ) -> Result<TaskUsage, MeterError> {
        let row = sqlx::query(&format!(
            "SELECT COALESCE(SUM({SPEND}), 0)::bigint AS spent, \
             COALESCE(SUM(COALESCE(actual_micros, 0)) FILTER (WHERE status = 'settled'), 0)::bigint AS settled, \
             COALESCE(SUM(reserved_micros) FILTER (WHERE status = 'reserved'), 0)::bigint AS in_flight, \
             COALESCE(SUM(reserved_micros) FILTER (WHERE status = 'usage_unknown'), 0)::bigint AS unknown_micros, \
             COUNT(*) FILTER (WHERE status = 'usage_unknown')::bigint AS unknown_count, \
             COUNT(*)::bigint AS calls \
             FROM model_usage WHERE task_id = $1"
        ))
        .bind(task)
        .fetch_one(&mut *conn)
        .await?;
        Ok(TaskUsage {
            spent_micros: row.try_get("spent")?,
            settled_micros: row.try_get("settled")?,
            in_flight_micros: row.try_get("in_flight")?,
            unknown_micros: row.try_get("unknown_micros")?,
            unknown_count: row.try_get("unknown_count")?,
            calls: row.try_get("calls")?,
        })
    }
}
