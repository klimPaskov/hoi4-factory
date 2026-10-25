//! User API keys (bring your own key, ADR-016).
//!
//! A key is sealed with the accounts crate's [`SecretStore`] before it reaches the database. The
//! authenticated context `user_api_key:v1:<account>:<key>:<provider>` ties the ciphertext to its
//! owner, row and provider, so a sealed value copied to another row or account does not open.
//! Records carry no secret, only the last four characters as a hint. Row-level security shows a
//! key to its owner alone, and every query also filters by the account. Errors never contain key
//! text.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use factory_accounts::{AccountId, SecretStore};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};

use crate::ids::UserKeyId;
use crate::secret::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyProvider {
    Anthropic,
    Openai,
    OpenaiCompatible,
}

impl KeyProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyProvider::Anthropic => "anthropic",
            KeyProvider::Openai => "openai",
            KeyProvider::OpenaiCompatible => "openai_compatible",
        }
    }

    pub fn parse(value: &str) -> Option<KeyProvider> {
        [
            KeyProvider::Anthropic,
            KeyProvider::Openai,
            KeyProvider::OpenaiCompatible,
        ]
        .into_iter()
        .find(|p| p.as_str() == value)
    }
}

pub struct NewKey {
    pub provider: KeyProvider,
    pub label: String,
    /// Required for `openai_compatible`, refused for the other providers.
    pub base_url: Option<String>,
    pub plaintext: SecretString,
}

impl std::fmt::Debug for NewKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewKey")
            .field("provider", &self.provider)
            .field("label", &self.label)
            .field("base_url", &self.base_url)
            .field("plaintext", &self.plaintext)
            .finish()
    }
}

/// A stored key as shown to its owner. No secret field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyRecord {
    pub id: UserKeyId,
    pub account_id: AccountId,
    pub provider: KeyProvider,
    pub label: String,
    pub base_url: Option<String>,
    /// Last four characters of the key.
    pub key_hint: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("key not found")]
    NotFound,
    #[error("key was revoked")]
    Revoked,
    #[error("the stored key could not be opened")]
    Decrypt,
    #[error("the key could not be sealed")]
    Seal,
    #[error("invalid key: {0}")]
    Invalid(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

const COLUMNS: &str =
    "id, account_id, provider, label, base_url, key_hint, created_at, last_used_at, revoked_at";

fn map_record(row: &PgRow) -> Result<KeyRecord, sqlx::Error> {
    let provider: String = row.try_get("provider")?;
    Ok(KeyRecord {
        id: UserKeyId(row.try_get("id")?),
        account_id: AccountId(row.try_get("account_id")?),
        provider: KeyProvider::parse(&provider)
            .ok_or_else(|| sqlx::Error::Decode("unknown provider".into()))?,
        label: row.try_get("label")?,
        base_url: row.try_get("base_url")?,
        key_hint: row.try_get("key_hint")?,
        created_at: row.try_get("created_at")?,
        last_used_at: row.try_get("last_used_at")?,
        revoked_at: row.try_get("revoked_at")?,
    })
}

fn context(account: AccountId, key: UserKeyId, provider: KeyProvider) -> Vec<u8> {
    format!("user_api_key:v1:{account}:{key}:{}", provider.as_str()).into_bytes()
}

fn validate(new: &NewKey) -> Result<(String, String), KeyError> {
    let invalid = |m: &str| KeyError::Invalid(m.to_owned());
    let label = new.label.trim();
    if label.is_empty() || label.chars().count() > 64 || label.chars().any(char::is_control) {
        return Err(invalid("the label must be 1 to 64 printable characters"));
    }
    match (new.provider, &new.base_url) {
        (KeyProvider::OpenaiCompatible, Some(url)) => {
            let rest = url
                .strip_prefix("https://")
                .or_else(|| url.strip_prefix("http://"))
                .ok_or_else(|| invalid("the base URL must start with http:// or https://"))?;
            let authority = rest.split('/').next().unwrap_or("");
            if url.len() > 512
                || authority.is_empty()
                || authority.contains('@')
                || url.chars().any(|c| c.is_control() || c.is_whitespace())
                || url.contains('?')
                || url.contains('#')
            {
                return Err(invalid("the base URL is not acceptable"));
            }
        }
        (KeyProvider::OpenaiCompatible, None) => {
            return Err(invalid("a self-hosted key needs a base URL"));
        }
        (_, Some(_)) => return Err(invalid("only a self-hosted key takes a base URL")),
        (_, None) => {}
    }
    let secret = new.plaintext.expose();
    if secret.chars().count() < 8
        || secret.len() > 512
        || secret.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(invalid(
            "the key must be 8 to 512 characters without spaces",
        ));
    }
    let hint: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok((label.to_owned(), hint))
}

#[derive(Clone)]
pub struct KeyStore {
    secrets: Arc<dyn SecretStore>,
}

impl KeyStore {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets }
    }

    /// Seals and stores a key for `account`.
    pub async fn save(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
        new: NewKey,
    ) -> Result<KeyRecord, KeyError> {
        let (label, hint) = validate(&new)?;
        let id = UserKeyId::new();
        let sealed = self
            .secrets
            .seal(
                new.plaintext.expose().as_bytes(),
                &context(account, id, new.provider),
            )
            .map_err(|_| KeyError::Seal)?;
        let sql = format!(
            "INSERT INTO user_api_keys (id, account_id, provider, label, base_url, sealed, key_hint) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING {COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(id.0)
            .bind(account.0)
            .bind(new.provider.as_str())
            .bind(&label)
            .bind(&new.base_url)
            .bind(&sealed)
            .bind(&hint)
            .fetch_one(&mut *conn)
            .await?;
        Ok(map_record(&row)?)
    }

    /// The account's keys, newest first, revoked ones included.
    pub async fn list(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
    ) -> Result<Vec<KeyRecord>, KeyError> {
        let sql = format!(
            "SELECT {COLUMNS} FROM user_api_keys WHERE account_id = $1 ORDER BY created_at DESC, id"
        );
        let rows = sqlx::query(&sql)
            .bind(account.0)
            .fetch_all(&mut *conn)
            .await?;
        Ok(rows.iter().map(map_record).collect::<Result<_, _>>()?)
    }

    /// Marks the key revoked. It stays listed but can no longer be opened.
    pub async fn revoke(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
        key: UserKeyId,
    ) -> Result<KeyRecord, KeyError> {
        let sql = format!(
            "UPDATE user_api_keys SET revoked_at = COALESCE(revoked_at, now()) \
             WHERE id = $1 AND account_id = $2 RETURNING {COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(key.0)
            .bind(account.0)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(KeyError::NotFound)?;
        Ok(map_record(&row)?)
    }

    /// Deletes the key row for good. Only its owner can.
    pub async fn delete(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
        key: UserKeyId,
    ) -> Result<(), KeyError> {
        let done = sqlx::query("DELETE FROM user_api_keys WHERE id = $1 AND account_id = $2")
            .bind(key.0)
            .bind(account.0)
            .execute(&mut *conn)
            .await?;
        if done.rows_affected() == 0 {
            return Err(KeyError::NotFound);
        }
        Ok(())
    }

    /// Opens the key for its owner and records the use. Another account's key is `NotFound`.
    pub async fn open_for_owner(
        &self,
        conn: &mut PgConnection,
        account: AccountId,
        key: UserKeyId,
    ) -> Result<SecretString, KeyError> {
        let row = sqlx::query(
            "SELECT provider, sealed, revoked_at FROM user_api_keys WHERE id = $1 AND account_id = $2",
        )
        .bind(key.0)
        .bind(account.0)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(KeyError::NotFound)?;
        if row
            .try_get::<Option<DateTime<Utc>>, _>("revoked_at")?
            .is_some()
        {
            return Err(KeyError::Revoked);
        }
        let provider: String = row.try_get("provider")?;
        let provider = KeyProvider::parse(&provider).ok_or(KeyError::Decrypt)?;
        let sealed: Vec<u8> = row.try_get("sealed")?;
        let plain = self
            .secrets
            .open(&sealed, &context(account, key, provider))
            .map_err(|_| KeyError::Decrypt)?;
        let text = String::from_utf8(plain).map_err(|_| KeyError::Decrypt)?;
        sqlx::query(
            "UPDATE user_api_keys SET last_used_at = now() WHERE id = $1 AND account_id = $2",
        )
        .bind(key.0)
        .bind(account.0)
        .execute(&mut *conn)
        .await?;
        Ok(SecretString::new(text))
    }
}
