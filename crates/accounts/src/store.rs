//! Queries on the account-level tables: accounts, sessions, verification and reset tokens.
//!
//! These tables are not tenant data (they are what identifies the tenant), so they sit outside
//! row-level security. They are reachable only from this crate.

use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ApiError;
use crate::ids::{AccountId, SessionId};
use crate::model::Account;

/// Everything about an account row, including the password hash. Never serialised.
#[derive(Clone)]
pub(crate) struct AccountRow {
    pub id: AccountId,
    pub email: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub password_hash: Option<String>,
    pub github_user_id: Option<i64>,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub disabled_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl AccountRow {
    pub fn view(&self) -> Account {
        Account {
            id: self.id,
            email: self.email.clone(),
            email_verified: self.email_verified_at.is_some(),
            display_name: self.display_name.clone(),
            github_linked: self.github_user_id.is_some(),
            has_password: self.password_hash.is_some(),
            created_at: self.created_at,
        }
    }

    /// Neither disabled nor soft deleted.
    pub fn is_active(&self) -> bool {
        self.disabled_at.is_none() && self.deleted_at.is_none()
    }
}

/// Column list for queries over `accounts a`.
const ACCOUNT_COLUMNS: &str = "a.id, a.email, a.email_verified_at, a.password_hash, a.github_user_id, \
     a.display_name, a.created_at, a.disabled_at, a.deleted_at";

fn map_account(row: &PgRow) -> Result<AccountRow, sqlx::Error> {
    Ok(AccountRow {
        id: AccountId(row.try_get::<Uuid, _>("id")?),
        email: row.try_get("email")?,
        email_verified_at: row.try_get("email_verified_at")?,
        password_hash: row.try_get("password_hash")?,
        github_user_id: row.try_get("github_user_id")?,
        display_name: row.try_get("display_name")?,
        created_at: row.try_get("created_at")?,
        disabled_at: row.try_get("disabled_at")?,
        deleted_at: row.try_get("deleted_at")?,
    })
}

/// A live session joined with its (active) account.
pub(crate) struct SessionRow {
    pub session_id: SessionId,
    pub last_seen_at: DateTime<Utc>,
    pub account: AccountRow,
}

fn unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

#[derive(Clone)]
pub(crate) struct Store {
    pool: PgPool,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ----- accounts -------------------------------------------------------------------------

    pub async fn insert_password_account(
        &self,
        email: &str,
        password_hash: &str,
        display_name: &str,
        now: DateTime<Utc>,
    ) -> Result<AccountRow, ApiError> {
        let id = AccountId::new();
        let result = sqlx::query(&format!(
            "INSERT INTO accounts AS a (id, email, password_hash, display_name, created_at)
             VALUES ($1, $2, $3, $4, $5) RETURNING {ACCOUNT_COLUMNS}"
        ))
        .bind(id.0)
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .bind(now)
        .fetch_one(&self.pool)
        .await;
        match result {
            Ok(row) => Ok(map_account(&row)?),
            Err(e) if unique_violation(&e) => Err(ApiError::conflict(
                "email_taken",
                "An account with this email address already exists.",
            )),
            Err(e) => Err(e.into()),
        }
    }

    /// GitHub sign-in counts as verified, so the row is created verified.
    pub async fn insert_github_account(
        &self,
        github_user_id: i64,
        email: &str,
        display_name: &str,
        now: DateTime<Utc>,
    ) -> Result<AccountRow, ApiError> {
        let id = AccountId::new();
        let result = sqlx::query(&format!(
            "INSERT INTO accounts AS a (id, email, email_verified_at, github_user_id, display_name, created_at)
             VALUES ($1, $2, $3, $4, $5, $3) RETURNING {ACCOUNT_COLUMNS}"
        ))
        .bind(id.0)
        .bind(email)
        .bind(now)
        .bind(github_user_id)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await;
        match result {
            Ok(row) => Ok(map_account(&row)?),
            Err(e) if unique_violation(&e) => Err(ApiError::conflict(
                "account_exists",
                "An account for this GitHub user or email address already exists.",
            )),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<AccountRow>, ApiError> {
        let row = sqlx::query(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM accounts a WHERE lower(a.email) = lower($1)"
        ))
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(map_account).transpose()?)
    }

    pub async fn find_by_github_id(
        &self,
        github_user_id: i64,
    ) -> Result<Option<AccountRow>, ApiError> {
        let row = sqlx::query(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM accounts a WHERE a.github_user_id = $1"
        ))
        .bind(github_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(map_account).transpose()?)
    }

    pub async fn find_by_id(&self, id: AccountId) -> Result<Option<AccountRow>, ApiError> {
        let row = sqlx::query(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM accounts a WHERE a.id = $1"
        ))
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(map_account).transpose()?)
    }

    pub async fn mark_verified(&self, id: AccountId, now: DateTime<Utc>) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE accounts SET email_verified_at = COALESCE(email_verified_at, $2) WHERE id = $1",
        )
        .bind(id.0)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Links a GitHub identity to an existing account and verifies it. When the account was
    /// never verified, the password is removed and every session ended: anyone who registered
    /// the address before its real owner proved ownership through GitHub loses access.
    pub async fn link_github(
        &self,
        id: AccountId,
        github_user_id: i64,
        now: DateTime<Utc>,
        reset_unverified_credentials: bool,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        let result = if reset_unverified_credentials {
            sqlx::query(
                "UPDATE accounts SET github_user_id = $2, email_verified_at = $3, password_hash = NULL WHERE id = $1",
            )
        } else {
            sqlx::query(
                "UPDATE accounts SET github_user_id = $2, email_verified_at = COALESCE(email_verified_at, $3) WHERE id = $1",
            )
        }
        .bind(id.0)
        .bind(github_user_id)
        .bind(now)
        .execute(&mut *tx)
        .await;
        match result {
            Ok(_) => {}
            Err(e) if unique_violation(&e) => {
                return Err(ApiError::conflict(
                    "github_already_linked",
                    "This GitHub account is already linked to another account.",
                ));
            }
            Err(e) => return Err(e.into()),
        }
        if reset_unverified_credentials {
            for table in [
                "sessions",
                "email_verification_tokens",
                "password_reset_tokens",
            ] {
                sqlx::query(&format!("DELETE FROM {table} WHERE account_id = $1"))
                    .bind(id.0)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    // ----- sessions -------------------------------------------------------------------------

    pub async fn insert_session(
        &self,
        account: AccountId,
        token_hash: &[u8],
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        user_agent: Option<&str>,
    ) -> Result<SessionId, ApiError> {
        let id = SessionId::new();
        sqlx::query(
            "INSERT INTO sessions (id, account_id, token_hash, created_at, expires_at, last_seen_at, user_agent)
             VALUES ($1, $2, $3, $4, $5, $4, $6)",
        )
        .bind(id.0)
        .bind(account.0)
        .bind(token_hash)
        .bind(now)
        .bind(expires_at)
        .bind(user_agent.map(|ua| ua.chars().take(256).collect::<String>()))
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// The session behind `token_hash`, if it has neither expired nor idled out and its account
    /// is active.
    pub async fn find_live_session(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
        idle_cutoff: DateTime<Utc>,
    ) -> Result<Option<SessionRow>, ApiError> {
        let row = sqlx::query(&format!(
            "SELECT s.id AS session_id, s.last_seen_at, {ACCOUNT_COLUMNS}
             FROM sessions s JOIN accounts a ON a.id = s.account_id
             WHERE s.token_hash = $1 AND s.expires_at > $2 AND s.last_seen_at > $3
               AND a.disabled_at IS NULL AND a.deleted_at IS NULL"
        ))
        .bind(token_hash)
        .bind(now)
        .bind(idle_cutoff)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        Ok(Some(SessionRow {
            session_id: SessionId(row.try_get::<Uuid, _>("session_id")?),
            last_seen_at: row.try_get("last_seen_at")?,
            account: map_account(&row)?,
        }))
    }

    pub async fn touch_session(&self, id: SessionId, now: DateTime<Utc>) -> Result<(), ApiError> {
        sqlx::query("UPDATE sessions SET last_seen_at = $2 WHERE id = $1")
            .bind(id.0)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, token_hash: &[u8]) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ends every session of the account. Returns how many were ended.
    pub async fn delete_sessions(&self, account: AccountId) -> Result<u64, ApiError> {
        let done = sqlx::query("DELETE FROM sessions WHERE account_id = $1")
            .bind(account.0)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }

    /// Removes expired sessions and spent or expired tokens.
    pub async fn purge_expired(
        &self,
        now: DateTime<Utc>,
        idle_cutoff: DateTime<Utc>,
    ) -> Result<u64, ApiError> {
        let mut removed = 0;
        removed += sqlx::query("DELETE FROM sessions WHERE expires_at <= $1 OR last_seen_at <= $2")
            .bind(now)
            .bind(idle_cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected();
        for table in ["email_verification_tokens", "password_reset_tokens"] {
            removed += sqlx::query(&format!(
                "DELETE FROM {table} WHERE expires_at <= $1 OR consumed_at IS NOT NULL"
            ))
            .bind(now)
            .execute(&self.pool)
            .await?
            .rows_affected();
        }
        Ok(removed)
    }

    // ----- email verification ---------------------------------------------------------------

    /// Replaces any outstanding verification token of the account with a new one.
    pub async fn replace_verification_token(
        &self,
        account: AccountId,
        email: &str,
        token_hash: &[u8],
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM email_verification_tokens WHERE account_id = $1 AND consumed_at IS NULL",
        )
        .bind(account.0)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO email_verification_tokens (id, account_id, email, token_hash, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(account.0)
        .bind(email)
        .bind(token_hash)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Single-use: marks the token consumed and returns its account and address, or `None` when
    /// it is unknown, expired or already used.
    pub async fn consume_verification_token(
        &self,
        token_hash: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Option<(AccountId, String)>, ApiError> {
        let row = sqlx::query(
            "UPDATE email_verification_tokens SET consumed_at = $2
             WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > $2
             RETURNING account_id, email",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        Ok(Some((
            AccountId(row.try_get::<Uuid, _>("account_id")?),
            row.try_get("email")?,
        )))
    }

    // ----- password reset and change --------------------------------------------------------

    /// Stores a new reset token, which invalidates every earlier unused one.
    pub async fn replace_reset_token(
        &self,
        account: AccountId,
        token_hash: &[u8],
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM password_reset_tokens WHERE account_id = $1 AND consumed_at IS NULL",
        )
        .bind(account.0)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO password_reset_tokens (id, account_id, token_hash, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(account.0)
        .bind(token_hash)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Redeems a reset token in one transaction: marks it used, sets the new password hash,
    /// marks the address verified (the link proved control of the mailbox), drops every other
    /// reset token and ends every session. Returns the account's id and address, or `None` when
    /// the token is unknown, expired, used, or its account is disabled.
    pub async fn apply_password_reset(
        &self,
        token_hash: &[u8],
        new_password_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<(AccountId, String)>, ApiError> {
        let mut tx = self.pool.begin().await?;
        let consumed = sqlx::query(
            "UPDATE password_reset_tokens SET consumed_at = $2
             WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > $2
             RETURNING account_id",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(consumed) = consumed else {
            return Ok(None);
        };
        let account = AccountId(consumed.try_get::<Uuid, _>("account_id")?);
        let updated = sqlx::query(
            "UPDATE accounts SET password_hash = $2, email_verified_at = COALESCE(email_verified_at, $3)
             WHERE id = $1 AND disabled_at IS NULL AND deleted_at IS NULL
             RETURNING email",
        )
        .bind(account.0)
        .bind(new_password_hash)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(updated) = updated else {
            return Ok(None);
        };
        sqlx::query("DELETE FROM password_reset_tokens WHERE account_id = $1")
            .bind(account.0)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM sessions WHERE account_id = $1")
            .bind(account.0)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some((account, updated.try_get("email")?)))
    }
}
