//! Account services: sign-up, sign-in, sessions, email verification and GitHub sign-in.
//!
//! [`AccountsState`] is the shared state of the router and the entry point for other crates
//! (`authenticate`, `scoped`).

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};

use crate::config::AccountsConfig;
use crate::db::Database;
use crate::error::{ApiError, RetryClass};
use crate::github::{GitHubClient, GitHubError, HttpGitHubClient};
use crate::ids::{AccountId, SessionId};
use crate::mail::{Mailer, OutgoingMail, SmtpMailer};
use crate::model::Account;
use crate::password::PasswordHasher;
use crate::ratelimit::{self, InMemoryRateLimiter, RateLimiter};
use crate::store::{AccountRow, Store};
use crate::tenant::TenantRepo;
use crate::text::{email_key, normalize_display_name, normalize_email};
use crate::tokens::{hash_token, random_token};

/// How often a session's `last_seen_at` is written back.
const TOUCH_INTERVAL_SECONDS: i64 = 60;

/// External collaborators, replaceable in tests.
pub struct AccountsDeps {
    pub mailer: Arc<dyn Mailer>,
    pub github: Option<Arc<dyn GitHubClient>>,
    pub limiter: Arc<dyn RateLimiter>,
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("mail setup failed: {0}")]
    Mail(#[from] crate::mail::MailError),
    #[error("GitHub client setup failed: {0}")]
    GitHub(String),
    #[error("password hashing setup failed: {0}")]
    Hashing(String),
}

impl AccountsDeps {
    /// SMTP mail, the HTTP GitHub client (when configured) and the in-memory rate limiter.
    pub fn production(config: &AccountsConfig, db: &Database) -> Result<Self, SetupError> {
        let github: Option<Arc<dyn GitHubClient>> = match &config.github {
            Some(gh) => Some(Arc::new(
                HttpGitHubClient::new(gh.clone()).map_err(|e| SetupError::GitHub(e.to_string()))?,
            )),
            None => None,
        };
        Ok(Self {
            mailer: Arc::new(SmtpMailer::from_url(&config.smtp_url, &config.mail_from)?),
            github,
            limiter: Arc::new(InMemoryRateLimiter::new(
                db.clock().clone(),
                &config.rate_limits,
            )),
        })
    }
}

struct Inner {
    db: Database,
    store: Store,
    config: AccountsConfig,
    mailer: Arc<dyn Mailer>,
    github: Option<Arc<dyn GitHubClient>>,
    limiter: Arc<dyn RateLimiter>,
    hasher: PasswordHasher,
}

/// Shared state of the accounts router. Cheap to clone.
#[derive(Clone)]
pub struct AccountsState {
    inner: Arc<Inner>,
}

/// The request's origin as far as rate limiting and the session record are concerned.
#[derive(Debug, Clone, Copy)]
pub struct ClientInfo<'a> {
    pub ip: &'a str,
    pub user_agent: Option<&'a str>,
}

/// The authenticated caller, derived from the session cookie on every request. Handlers take it
/// as an extractor. It never comes from a client-supplied field.
#[derive(Debug, Clone)]
pub struct Principal {
    pub account: Account,
    pub session_id: SessionId,
}

impl Principal {
    pub fn account_id(&self) -> AccountId {
        self.account.id
    }

    /// Agent tasks and the monthly allowance require a verified email (ADR-021). Later stages
    /// call this before dispatching work.
    pub fn require_verified(&self) -> Result<(), ApiError> {
        if self.account.email_verified {
            Ok(())
        } else {
            Err(ApiError::email_not_verified())
        }
    }
}

/// A freshly issued session. `token` is the cookie value and is shown once.
#[derive(Debug, Clone)]
pub struct SessionGrant {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SignedIn {
    pub account: Account,
    pub session: SessionGrant,
    /// False when the verification mail could not be sent. The account exists; the user can
    /// request another mail.
    pub verification_email_sent: bool,
}

/// Result of the GitHub callback.
#[derive(Debug, Clone)]
pub enum GitHubOutcome {
    /// A GitHub identity was attached to the signed-in account.
    Linked(Account),
    SignedIn(SignedIn),
}

impl AccountsState {
    pub fn new(
        db: Database,
        config: AccountsConfig,
        deps: AccountsDeps,
    ) -> Result<Self, SetupError> {
        let hasher =
            PasswordHasher::new(config.argon2).map_err(|e| SetupError::Hashing(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                store: Store::new(db.pool().clone()),
                db,
                config,
                mailer: deps.mailer,
                github: deps.github,
                limiter: deps.limiter,
                hasher,
            }),
        })
    }

    /// Reads [`AccountsConfig::from_env`] and wires the production collaborators.
    pub fn from_env(db: Database) -> Result<Self, SetupError> {
        let config = AccountsConfig::from_env()?;
        let deps = AccountsDeps::production(&config, &db)?;
        Self::new(db, config, deps)
    }

    pub fn config(&self) -> &AccountsConfig {
        &self.inner.config
    }

    pub fn db(&self) -> &Database {
        &self.inner.db
    }

    fn now(&self) -> DateTime<Utc> {
        self.inner.db.clock().now()
    }

    /// The tenant-scoped project repository for `account`.
    pub fn scoped(&self, account: AccountId) -> TenantRepo {
        self.inner
            .db
            .scoped(account)
            .with_deletion_grace(self.inner.config.deletion_grace)
    }

    fn limit(&self, rule: &str, key: &str) -> Result<(), ApiError> {
        self.inner
            .limiter
            .check(rule, key)
            .map_err(ApiError::rate_limited)
    }

    fn chrono_duration(d: std::time::Duration) -> Duration {
        Duration::from_std(d).unwrap_or(Duration::days(30))
    }

    // ----- sessions -------------------------------------------------------------------------

    async fn issue_session(
        &self,
        account: AccountId,
        client: &ClientInfo<'_>,
    ) -> Result<SessionGrant, ApiError> {
        let now = self.now();
        let expires_at = now + Self::chrono_duration(self.inner.config.session_ttl);
        let token = random_token();
        self.inner
            .store
            .insert_session(
                account,
                &hash_token(&token),
                now,
                expires_at,
                client.user_agent,
            )
            .await?;
        Ok(SessionGrant { token, expires_at })
    }

    /// Resolves a session cookie value to its account, or `unauthenticated`. Expired, idle,
    /// signed-out and disabled-account sessions all fail the same way.
    pub async fn authenticate(&self, token: &str) -> Result<Principal, ApiError> {
        if token.is_empty() || token.len() > 128 {
            return Err(ApiError::unauthenticated());
        }
        let now = self.now();
        let idle_cutoff = now - Self::chrono_duration(self.inner.config.session_idle_ttl);
        let row = self
            .inner
            .store
            .find_live_session(&hash_token(token), now, idle_cutoff)
            .await?
            .ok_or_else(ApiError::unauthenticated)?;
        if (now - row.last_seen_at).num_seconds() >= TOUCH_INTERVAL_SECONDS {
            self.inner.store.touch_session(row.session_id, now).await?;
        }
        Ok(Principal {
            account: row.account.view(),
            session_id: row.session_id,
        })
    }

    /// Ends the session behind `token`. Unknown tokens are ignored.
    pub async fn sign_out(&self, token: &str) -> Result<(), ApiError> {
        self.inner.store.delete_session(&hash_token(token)).await
    }

    /// Ends every session of the account, including the current one. Used by "sign out
    /// everywhere" and available to administrative and security code. Returns how many ended.
    pub async fn revoke_all_sessions(&self, account: AccountId) -> Result<u64, ApiError> {
        self.inner.store.delete_sessions(account).await
    }

    /// Deletes expired sessions and spent tokens. For a periodic maintenance task.
    pub async fn purge_expired(&self) -> Result<u64, ApiError> {
        let now = self.now();
        let idle = now - Self::chrono_duration(self.inner.config.session_idle_ttl);
        self.inner.store.purge_expired(now, idle).await
    }

    // ----- email and password ---------------------------------------------------------------

    pub async fn sign_up(
        &self,
        email: &str,
        password: String,
        display_name: Option<&str>,
        client: &ClientInfo<'_>,
    ) -> Result<SignedIn, ApiError> {
        self.limit(ratelimit::SIGNUP_IP, client.ip)?;
        self.limit(ratelimit::SIGNUP_EMAIL, &email_key(email))?;
        let email = normalize_email(email)?;
        PasswordHasher::check_policy(&password)?;
        let display_name = normalize_display_name(display_name, &email)?;
        let hash = self.inner.hasher.hash(password).await?;
        let row = self
            .inner
            .store
            .insert_password_account(&email, &hash, &display_name, self.now())
            .await?;
        let verification_email_sent = self.send_verification(&row).await;
        let session = self.issue_session(row.id, client).await?;
        Ok(SignedIn {
            account: row.view(),
            session,
            verification_email_sent,
        })
    }

    pub async fn sign_in(
        &self,
        email: &str,
        password: String,
        client: &ClientInfo<'_>,
    ) -> Result<SignedIn, ApiError> {
        self.limit(ratelimit::SIGNIN_IP, client.ip)?;
        self.limit(ratelimit::SIGNIN_ACCOUNT, &email_key(email))?;
        // A malformed address cannot match any account. Treat it like a wrong password.
        let row = match normalize_email(email) {
            Ok(e) => self.inner.store.find_by_email(&e).await?,
            Err(_) => None,
        };
        let stored = row.as_ref().and_then(|r| r.password_hash.clone());
        let matches = self.inner.hasher.verify(password, stored).await;
        let Some(row) = row.filter(|_| matches) else {
            return Err(ApiError::invalid_credentials());
        };
        if !row.is_active() {
            return Err(ApiError::account_disabled());
        }
        let session = self.issue_session(row.id, client).await?;
        Ok(SignedIn {
            account: row.view(),
            session,
            verification_email_sent: false,
        })
    }

    /// Sends a verification mail with a fresh single-use token. Returns whether the mail was
    /// handed to the transport. Failures are logged without the address or the token.
    async fn send_verification(&self, account: &AccountRow) -> bool {
        let token = random_token();
        let now = self.now();
        let expires_at = now + Self::chrono_duration(self.inner.config.verification_ttl);
        let stored = self
            .inner
            .store
            .replace_verification_token(
                account.id,
                &account.email,
                &hash_token(&token),
                now,
                expires_at,
            )
            .await;
        if stored.is_err() {
            tracing::error!(account_id = %account.id, "could not store verification token");
            return false;
        }
        let link = format!("{}?token={}", self.inner.config.verify_link_base, token);
        let hours = self.inner.config.verification_ttl.as_secs() / 3600;
        let mail = OutgoingMail {
            to: account.email.clone(),
            subject: "Confirm your email address for HOI4 Factory".to_owned(),
            text: format!(
                "Hello {},\n\nConfirm your email address by opening this link:\n\n{}\n\n\
                 The link works once and expires in {} hours. If you did not create a HOI4 Factory \
                 account, ignore this message.\n",
                account.display_name, link, hours
            ),
        };
        match self.inner.mailer.send(mail).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(account_id = %account.id, error = %e, "verification mail was not sent");
                false
            }
        }
    }

    /// Re-sends the verification mail to the signed-in, unverified account.
    pub async fn resend_verification(&self, principal: &Principal) -> Result<bool, ApiError> {
        if principal.account.email_verified {
            return Ok(false);
        }
        self.limit(ratelimit::RESEND_ACCOUNT, &principal.account.id.to_string())?;
        let row = self
            .inner
            .store
            .find_by_id(principal.account.id)
            .await?
            .ok_or_else(ApiError::unauthenticated)?;
        Ok(self.send_verification(&row).await)
    }

    /// Redeems a verification token. Tokens are single use and expire.
    pub async fn verify_email(&self, token: &str) -> Result<Account, ApiError> {
        let invalid = invalid_token;
        if token.is_empty() || token.len() > 128 {
            return Err(invalid());
        }
        let now = self.now();
        let (account_id, email) = self
            .inner
            .store
            .consume_verification_token(&hash_token(token), now)
            .await?
            .ok_or_else(invalid)?;
        let row = self
            .inner
            .store
            .find_by_id(account_id)
            .await?
            .ok_or_else(invalid)?;
        if !row.email.eq_ignore_ascii_case(&email) {
            // The address changed after the mail went out.
            return Err(invalid());
        }
        self.inner.store.mark_verified(account_id, now).await?;
        let row = self
            .inner
            .store
            .find_by_id(account_id)
            .await?
            .ok_or_else(invalid)?;
        Ok(row.view())
    }

    // ----- password reset -------------------------------------------------------------------

    /// Starts a password reset. The caller learns nothing about whether the address belongs to
    /// an account: rate limits are keyed on the input, and the lookup, the token and the mail
    /// happen in a background task, so the answer and its timing are the same either way. The
    /// returned handle finishes when that work has; request handlers drop it, tests await it.
    pub fn request_password_reset(
        &self,
        email: &str,
        client: &ClientInfo<'_>,
    ) -> Result<tokio::task::JoinHandle<()>, ApiError> {
        self.limit(ratelimit::RESET_IP, client.ip)?;
        self.limit(ratelimit::RESET_EMAIL, &email_key(email))?;
        let this = self.clone();
        let email = email.to_owned();
        Ok(tokio::spawn(
            async move { this.send_reset_link(&email).await },
        ))
    }

    async fn send_reset_link(&self, email: &str) {
        let Ok(email) = normalize_email(email) else {
            return;
        };
        let row = match self.inner.store.find_by_email(&email).await {
            Ok(Some(row)) if row.is_active() => row,
            Ok(_) => return,
            Err(_) => return,
        };
        let token = random_token();
        let now = self.now();
        let expires_at = now + Self::chrono_duration(self.inner.config.reset_ttl);
        // A newer request replaces the older token.
        if self
            .inner
            .store
            .replace_reset_token(row.id, &hash_token(&token), now, expires_at)
            .await
            .is_err()
        {
            tracing::error!(account_id = %row.id, "could not store password reset token");
            return;
        }
        let minutes = self.inner.config.reset_ttl.as_secs() / 60;
        let mail = OutgoingMail {
            to: row.email.clone(),
            subject: "Reset your HOI4 Factory password".to_owned(),
            text: format!(
                "Hello {},

Someone asked to reset the password of this HOI4 Factory account.                  To choose a new password, open this link:

{}?token={}

                 The link works once and expires in {} minutes. If you did not ask for this,                  ignore this message; your password stays as it is.
",
                row.display_name, self.inner.config.reset_link_base, token, minutes
            ),
        };
        if let Err(e) = self.inner.mailer.send(mail).await {
            tracing::warn!(account_id = %row.id, error = %e, "password reset mail was not sent");
        }
    }

    /// Sets a new password with a reset link. The link works once and expires. Every session of
    /// the account ends; the user signs in again with the new password. Using the link also
    /// verifies the address, since it proved control of the mailbox.
    pub async fn confirm_password_reset(
        &self,
        token: &str,
        new_password: String,
    ) -> Result<(), ApiError> {
        PasswordHasher::check_policy(&new_password)?;
        if token.is_empty() || token.len() > 128 {
            return Err(invalid_token());
        }
        let hash = self.inner.hasher.hash(new_password).await?;
        self.inner
            .store
            .apply_password_reset(&hash_token(token), &hash, self.now())
            .await?
            .ok_or_else(invalid_token)?;
        Ok(())
    }

    // ----- GitHub ---------------------------------------------------------------------------

    fn github(&self) -> Result<&Arc<dyn GitHubClient>, ApiError> {
        self.inner.github.as_ref().ok_or_else(|| {
            ApiError::new(
                axum::http::StatusCode::NOT_FOUND,
                "github_not_configured",
                "GitHub sign-in is not available on this server.",
                RetryClass::Terminal,
            )
        })
    }

    /// A fresh anti-forgery state and the GitHub URL to send the browser to.
    pub fn github_authorize(&self) -> Result<(String, String), ApiError> {
        let gh = self.github()?;
        let state = random_token();
        let url = gh.authorize_url(&state);
        Ok((state, url))
    }

    /// Completes the OAuth web flow. `link_to` is set when the flow started from a signed-in
    /// session that asked to link GitHub; otherwise the result is a sign-in or sign-up.
    ///
    /// The GitHub user token is used for two reads and dropped. It is neither stored nor logged.
    pub async fn github_complete(
        &self,
        code: &str,
        link_to: Option<AccountId>,
        client: &ClientInfo<'_>,
    ) -> Result<GitHubOutcome, ApiError> {
        let gh = self.github()?.clone();
        self.limit(ratelimit::SIGNIN_IP, client.ip)?;
        if code.is_empty() || code.len() > 512 {
            return Err(ApiError::invalid_request(
                "The GitHub authorization code is missing.",
            ));
        }
        let token = gh.exchange_code(code).await.map_err(github_failure)?;
        let user = gh.fetch_user(&token).await.map_err(github_failure)?;
        let email = gh
            .fetch_verified_primary_email(&token)
            .await
            .map_err(github_failure)?;
        drop(token);
        let now = self.now();
        let store = &self.inner.store;

        if let Some(account_id) = link_to {
            let row = store
                .find_by_id(account_id)
                .await?
                .ok_or_else(ApiError::unauthenticated)?;
            match row.github_user_id {
                Some(existing) if existing == user.id => {
                    return Ok(GitHubOutcome::Linked(row.view()));
                }
                Some(_) => {
                    return Err(ApiError::conflict(
                        "github_already_linked",
                        "This account is already linked to a different GitHub account.",
                    ));
                }
                None => {}
            }
            store.link_github(account_id, user.id, now, false).await?;
            let row = store
                .find_by_id(account_id)
                .await?
                .ok_or_else(ApiError::unauthenticated)?;
            return Ok(GitHubOutcome::Linked(row.view()));
        }

        let row = if let Some(row) = store.find_by_github_id(user.id).await? {
            row
        } else {
            let email = email.as_deref().ok_or_else(|| {
                ApiError::new(
                    axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "github_email_unavailable",
                    "GitHub did not share a verified primary email address. Verify one on GitHub and try again.",
                    RetryClass::RefreshPreconditions,
                )
            })?;
            let email = normalize_email(email)?;
            match store.find_by_email(&email).await? {
                Some(existing) if existing.github_user_id.is_some() => {
                    return Err(ApiError::conflict(
                        "github_email_conflict",
                        "This email address belongs to an account linked to a different GitHub account.",
                    ));
                }
                Some(existing) => {
                    // GitHub verified the address, so its holder owns it. When the local account
                    // never proved ownership, whoever registered it first must not keep access.
                    let unverified = existing.email_verified_at.is_none();
                    store
                        .link_github(existing.id, user.id, now, unverified)
                        .await?;
                    store
                        .find_by_id(existing.id)
                        .await?
                        .ok_or_else(ApiError::unauthenticated)?
                }
                None => {
                    let name = user
                        .name
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .unwrap_or(&user.login);
                    let display_name = normalize_display_name(Some(name), &email)?;
                    store
                        .insert_github_account(user.id, &email, &display_name, now)
                        .await?
                }
            }
        };
        if !row.is_active() {
            return Err(ApiError::account_disabled());
        }
        let session = self.issue_session(row.id, client).await?;
        Ok(GitHubOutcome::SignedIn(SignedIn {
            account: row.view(),
            session,
            verification_email_sent: false,
        }))
    }
}

fn invalid_token() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::BAD_REQUEST,
        "invalid_token",
        "This link is invalid or has expired. Request a new one.",
        RetryClass::RequiresAuthorization,
    )
}

fn github_failure(err: GitHubError) -> ApiError {
    let (code, message) = match &err {
        GitHubError::Transport(_) => (
            "github_unreachable",
            "GitHub could not be reached. Try signing in again.",
        ),
        GitHubError::Rejected(_) => (
            "github_code_rejected",
            "GitHub did not accept the sign-in. Start again.",
        ),
        GitHubError::BadResponse(_) => (
            "github_bad_response",
            "GitHub returned an unexpected answer. Try again later.",
        ),
    };
    // The error text may describe the remote answer. Log it without any token material.
    tracing::warn!(error = %err, "GitHub OAuth step failed");
    ApiError::new(
        axum::http::StatusCode::BAD_GATEWAY,
        code,
        message,
        RetryClass::RefreshPreconditions,
    )
}
