//! Metering and budget stops against a real PostgreSQL with row-level security.

mod common;

use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use common::Env;
use factory_accounts::{AccountId, AesGcmSecretStore, ProjectId};
use factory_broker::*;
use serde_json::json;
use uuid::Uuid;

const ALLOWANCE: i64 = 1_000_000;
const TASK_BUDGET: i64 = 200_000;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap()
}

fn res(
    account: AccountId,
    project: ProjectId,
    task: Uuid,
    funding: Funding,
    reserve: i64,
) -> Reservation {
    Reservation {
        account,
        account_verified: true,
        project,
        task,
        attempt: Uuid::new_v4(),
        funding,
        provider: "anthropic".into(),
        model: "claude-haiku-5-5".into(),
        reserve_micros: reserve,
        task_budget_micros: TASK_BUDGET,
        allowance_micros: ALLOWANCE,
        now: now(),
    }
}

struct World {
    env: Env,
    account: AccountId,
    project: ProjectId,
}

async fn world() -> World {
    let env = Env::new().await;
    let account = env.account("payer").await;
    let project = env.project(account).await;
    World {
        env,
        account,
        project,
    }
}

async fn reserve(w: &World, r: Reservation) -> Result<ReservationId, MeterError> {
    let mut tx = w.env.tx(r.account).await;
    let id = Meter::new().reserve(&mut tx, r).await?;
    tx.commit().await.unwrap();
    Ok(id)
}

async fn settle(w: &World, id: ReservationId, s: Settlement) -> Result<SettleOutcome, MeterError> {
    let mut tx = w.env.tx(w.account).await;
    let r = Meter::new().settle(&mut tx, id, s, now()).await?;
    tx.commit().await.unwrap();
    Ok(r)
}

async fn summary(w: &World) -> UsageSummary {
    let mut tx = w.env.tx(w.account).await;
    Meter::new()
        .account_month(&mut tx, w.account, month_start(now()), ALLOWANCE)
        .await
        .unwrap()
}

fn stop(e: MeterError) -> BudgetStop {
    match e {
        MeterError::Stop(s) => s,
        other => panic!("expected a budget stop, got {other:?}"),
    }
}

fn op(w: &World, task: Uuid, reserve: i64) -> Reservation {
    res(
        w.account,
        w.project,
        task,
        Funding::OperatorAllowance,
        reserve,
    )
}

#[tokio::test]
async fn reservation_within_allowance_settles_to_actual() {
    let w = world().await;
    let task = Uuid::new_v4();
    let id = reserve(&w, op(&w, task, 150_000)).await.unwrap();
    let s = summary(&w).await;
    assert_eq!(s.operator_spent_micros, 150_000);
    assert_eq!(s.remaining_allowance_micros, 850_000);

    let out = settle(
        &w,
        id,
        Settlement::Known {
            actual_micros: 12_345,
            usage: json!({"input_tokens": 10, "output_tokens": 5}),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        out,
        SettleOutcome {
            status: UsageStatus::Settled,
            counted_micros: 12_345
        }
    );
    let s = summary(&w).await;
    assert_eq!(s.operator_spent_micros, 12_345);
    assert_eq!(s.user_key_spent_micros, 0);
    assert_eq!(s.operator_unknown_count, 0);

    // Settling twice is an error and changes nothing.
    let again = settle(&w, id, Settlement::Released).await.unwrap_err();
    assert!(
        matches!(again, MeterError::AlreadySettled(UsageStatus::Settled)),
        "{again:?}"
    );
    assert_eq!(summary(&w).await.operator_spent_micros, 12_345);
    let mut tx = w.env.tx(w.account).await;
    assert!(matches!(
        Meter::new()
            .settle(&mut tx, ReservationId::new(), Settlement::Released, now())
            .await,
        Err(MeterError::NotFound)
    ));
}

#[tokio::test]
async fn allowance_exhaustion_stops_the_next_reservation() {
    let w = world().await;
    // Five tasks of 200_000 use exactly the allowance (equality is allowed).
    for _ in 0..5 {
        reserve(&w, op(&w, Uuid::new_v4(), 200_000)).await.unwrap();
    }
    assert_eq!(summary(&w).await.remaining_allowance_micros, 0);
    let err = reserve(&w, op(&w, Uuid::new_v4(), 1)).await.unwrap_err();
    assert_eq!(
        stop(err),
        BudgetStop::AllowanceExhausted {
            spent: ALLOWANCE,
            allowance: ALLOWANCE
        }
    );

    // A new month starts with a fresh allowance.
    let mut next = op(&w, Uuid::new_v4(), 1000);
    next.now = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
    reserve(&w, next).await.unwrap();
}

#[tokio::test]
async fn released_reservations_free_the_allowance() {
    let w = world().await;
    let task = Uuid::new_v4();
    let id = reserve(&w, op(&w, task, 200_000)).await.unwrap();
    let out = settle(&w, id, Settlement::Released).await.unwrap();
    assert_eq!(out.counted_micros, 0);
    assert_eq!(summary(&w).await.operator_spent_micros, 0);
    reserve(&w, op(&w, task, 200_000)).await.unwrap();
}

#[tokio::test]
async fn unknown_usage_stays_counted_never_zero() {
    let w = world().await;
    let task = Uuid::new_v4();
    let id = reserve(&w, op(&w, task, 150_000)).await.unwrap();
    let out = settle(
        &w,
        id,
        Settlement::Unknown {
            reason: "stream ended without a usage record".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        out,
        SettleOutcome {
            status: UsageStatus::UsageUnknown,
            counted_micros: 150_000
        }
    );
    let s = summary(&w).await;
    assert_eq!(s.operator_spent_micros, 150_000);
    assert_eq!(s.operator_unknown_count, 1);

    let tu = {
        let mut tx = w.env.tx(w.account).await;
        Meter::new().task_usage(&mut tx, task).await.unwrap()
    };
    assert_eq!(tu.unknown_micros, 150_000);
    assert_eq!(tu.unknown_count, 1);
    assert_eq!(tu.spent_micros, 150_000);
    assert_eq!(tu.settled_micros, 0);

    // The unknown amount still counts against the task budget and the allowance.
    let err = reserve(&w, op(&w, task, 60_000)).await.unwrap_err();
    assert_eq!(
        stop(err),
        BudgetStop::TaskBudgetExhausted {
            spent: 150_000,
            budget: TASK_BUDGET
        }
    );
    let stored: Option<i64> =
        sqlx::query_scalar("SELECT actual_micros FROM model_usage WHERE id = $1")
            .bind(id.0)
            .fetch_one(&w.env.admin)
            .await
            .unwrap();
    assert_eq!(stored, None, "unknown cost is never written as zero");
}

#[tokio::test]
async fn user_key_usage_does_not_consume_the_allowance() {
    let w = world().await;
    let keys = KeyStore::new(Arc::new(AesGcmSecretStore::new(&[3u8; 32])));
    let key = {
        let mut tx = w.env.tx(w.account).await;
        let k = keys
            .save(
                &mut tx,
                w.account,
                NewKey {
                    provider: KeyProvider::Anthropic,
                    label: "mine".into(),
                    base_url: None,
                    plaintext: SecretString::new("sk-ant-my-own-key-1234"),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        k
    };
    // Use the whole allowance, then the user key still works, and vice versa.
    for _ in 0..5 {
        reserve(&w, op(&w, Uuid::new_v4(), 200_000)).await.unwrap();
    }
    assert!(matches!(
        stop(reserve(&w, op(&w, Uuid::new_v4(), 1)).await.unwrap_err()),
        BudgetStop::AllowanceExhausted { .. }
    ));
    let user_funding = Funding::UserKey { key_id: key.id };
    for _ in 0..3 {
        reserve(
            &w,
            res(w.account, w.project, Uuid::new_v4(), user_funding, 200_000),
        )
        .await
        .unwrap();
    }
    let s = summary(&w).await;
    assert_eq!(s.operator_spent_micros, ALLOWANCE);
    assert_eq!(s.user_key_spent_micros, 600_000);
    assert_eq!(s.remaining_allowance_micros, 0);

    // The user key is still bound by the task budget.
    let task = Uuid::new_v4();
    reserve(&w, res(w.account, w.project, task, user_funding, 150_000))
        .await
        .unwrap();
    let err = reserve(&w, res(w.account, w.project, task, user_funding, 60_000))
        .await
        .unwrap_err();
    assert!(matches!(stop(err), BudgetStop::TaskBudgetExhausted { .. }));

    // Revoked or foreign keys are unavailable.
    {
        let mut tx = w.env.tx(w.account).await;
        keys.revoke(&mut tx, w.account, key.id).await.unwrap();
        tx.commit().await.unwrap();
    }
    let err = reserve(
        &w,
        res(w.account, w.project, Uuid::new_v4(), user_funding, 10),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, MeterError::UserKeyUnavailable), "{err:?}");
    let err = reserve(
        &w,
        res(
            w.account,
            w.project,
            Uuid::new_v4(),
            Funding::UserKey {
                key_id: UserKeyId::new(),
            },
            10,
        ),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, MeterError::UserKeyUnavailable), "{err:?}");
}

#[tokio::test]
async fn task_budget_stop_is_independent_of_the_allowance() {
    let w = world().await;
    let task = Uuid::new_v4();
    reserve(&w, op(&w, task, 120_000)).await.unwrap();
    let id = reserve(&w, op(&w, task, 80_000)).await.unwrap();
    // 200_000 of 200_000 is allowed; one more micro is not, though the allowance has 800_000 left.
    let err = reserve(&w, op(&w, task, 1)).await.unwrap_err();
    assert_eq!(
        stop(err),
        BudgetStop::TaskBudgetExhausted {
            spent: 200_000,
            budget: TASK_BUDGET
        }
    );
    assert_eq!(summary(&w).await.remaining_allowance_micros, 800_000);
    // Another task is unaffected.
    reserve(&w, op(&w, Uuid::new_v4(), 100_000)).await.unwrap();
    // A larger actual cost than reserved is counted at its actual value.
    settle(
        &w,
        id,
        Settlement::Known {
            actual_micros: 95_000,
            usage: json!({}),
        },
    )
    .await
    .unwrap();
    let mut tx = w.env.tx(w.account).await;
    let tu = Meter::new().task_usage(&mut tx, task).await.unwrap();
    assert_eq!(tu.spent_micros, 215_000);
    assert_eq!(tu.calls, 2);
    assert_eq!(tu.in_flight_micros, 120_000);
    assert_eq!(tu.settled_micros, 95_000);
}

#[tokio::test]
async fn unverified_account_is_refused_the_allowance_but_may_use_its_own_key() {
    let w = world().await;
    let mut r = op(&w, Uuid::new_v4(), 1000);
    r.account_verified = false;
    assert_eq!(
        stop(reserve(&w, r).await.unwrap_err()),
        BudgetStop::AccountNotVerified
    );
    assert_eq!(w.env.count("SELECT count(*) FROM model_usage").await, 0);

    let keys = KeyStore::new(Arc::new(AesGcmSecretStore::new(&[3u8; 32])));
    let mut tx = w.env.tx(w.account).await;
    let key = keys
        .save(
            &mut tx,
            w.account,
            NewKey {
                provider: KeyProvider::Openai,
                label: "mine".into(),
                base_url: None,
                plaintext: SecretString::new("sk-proj-my-own-key-1234"),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut r = res(
        w.account,
        w.project,
        Uuid::new_v4(),
        Funding::UserKey { key_id: key.id },
        1000,
    );
    r.account_verified = false;
    reserve(&w, r).await.unwrap();
}

#[tokio::test]
async fn usage_is_private_to_the_account() {
    let w = world().await;
    let other = w.env.account("other").await;
    reserve(&w, op(&w, Uuid::new_v4(), 1000)).await.unwrap();
    let mut tx = w.env.tx(other).await;
    let s = Meter::new()
        .account_month(&mut tx, w.account, month_start(now()), ALLOWANCE)
        .await
        .unwrap();
    assert_eq!(s.operator_spent_micros, 0);
    // The other account cannot write usage for this one.
    let r = Meter::new()
        .reserve(&mut tx, op(&w, Uuid::new_v4(), 1))
        .await;
    assert!(matches!(r, Err(MeterError::Database(_))), "{r:?}");
    drop(tx);
    // The application role cannot delete usage.
    let mut tx = w.env.tx(w.account).await;
    let del = sqlx::query("DELETE FROM model_usage")
        .execute(&mut *tx)
        .await;
    assert!(del.is_err());
}

#[tokio::test]
async fn concurrent_reservations_never_overspend() {
    let w = world().await;
    // 12 tasks race for 400_000 each against a 1_000_000 allowance: exactly two can succeed.
    let mut handles = Vec::new();
    for _ in 0..12 {
        let db = w.env.database.clone();
        let account = w.account;
        let project = w.project;
        handles.push(tokio::spawn(async move {
            let mut r = res(
                account,
                project,
                Uuid::new_v4(),
                Funding::OperatorAllowance,
                400_000,
            );
            r.task_budget_micros = 1_000_000;
            let mut tx = db.scoped(account).begin().await.unwrap();
            match Meter::new().reserve(&mut tx, r).await {
                Ok(_) => {
                    tx.commit().await.unwrap();
                    true
                }
                Err(MeterError::Stop(BudgetStop::AllowanceExhausted { .. })) => false,
                Err(other) => panic!("unexpected {other:?}"),
            }
        }));
    }
    let mut ok = 0;
    for h in handles {
        if h.await.unwrap() {
            ok += 1;
        }
    }
    assert_eq!(ok, 2);
    assert_eq!(summary(&w).await.operator_spent_micros, 800_000);
    assert_eq!(w.env.count("SELECT count(*) FROM model_usage").await, 2);
}

#[tokio::test]
async fn concurrent_reservations_on_one_task_respect_the_task_budget() {
    let w = world().await;
    let task = Uuid::new_v4();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let db = w.env.database.clone();
        let (account, project) = (w.account, w.project);
        handles.push(tokio::spawn(async move {
            let r = res(account, project, task, Funding::OperatorAllowance, 70_000);
            let mut tx = db.scoped(account).begin().await.unwrap();
            match Meter::new().reserve(&mut tx, r).await {
                Ok(_) => {
                    tx.commit().await.unwrap();
                    true
                }
                Err(MeterError::Stop(BudgetStop::TaskBudgetExhausted { .. })) => false,
                Err(other) => panic!("unexpected {other:?}"),
            }
        }));
    }
    let mut ok = 0;
    for h in handles {
        ok += h.await.unwrap() as i32;
    }
    assert_eq!(
        ok, 2,
        "70_000 * 2 = 140_000 fits in 200_000, a third would not"
    );
}
