//! HOI4 Factory accounts: email and GitHub sign-in, sessions, projects, roles and tenant
//! isolation.
//!
//! * [`router`] exposes the HTTP API. Mount it at [`API_PREFIX`] (`/api/v1`).
//! * [`AccountsState`] carries the configuration and collaborators. Other crates use
//!   [`AccountsState::authenticate`], the [`Principal`] extractor and
//!   [`AccountsState::scoped`] for tenant-scoped access.
//! * [`Database::scoped`] / [`TenantRepo`] are the only API for the `projects` and
//!   `project_members` tables. PostgreSQL row-level security (migration `0100_accounts.up.sql`)
//!   backs it: a query without `app.account_id` set sees nothing.
//! * [`migrator`] applies this crate's migrations (versions 0100 to 0199).

pub mod clock;
pub mod config;
pub mod db;
pub mod error;
pub mod github;
mod http;
pub mod ids;
pub mod mail;
pub mod model;
pub mod password;
pub mod ratelimit;
pub mod secrets;
pub mod service;
mod store;
mod tenant;
pub mod text;
pub mod tokens;

/// Path prefix the router is mounted at.
pub const API_PREFIX: &str = "/api/v1";

pub use clock::{Clock, ManualClock, SystemClock};
pub use config::{AccountsConfig, Environment, GitHubOAuthConfig, Redacted};
pub use db::{Database, DatabaseSetupError, PoolSettings, PostureError, RolePosture, migrator};
pub use error::{ApiError, RetryClass};
pub use github::{GitHubClient, GitHubError, GitHubUser, HttpGitHubClient, UserToken};
pub use http::{ApiJson, ClientIp, cookies, router};
pub use ids::{AccountId, ProjectId, SessionId};
pub use mail::{MailError, Mailer, MemoryMailer, OutgoingMail, SmtpMailer};
pub use model::{Account, Member, Permission, Project, Role, Visibility};
pub use ratelimit::{InMemoryRateLimiter, RateLimitConfig, RateLimiter, Rule};
pub use secrets::{AesGcmSecretStore, SecretError, SecretStore};
pub use service::{
    AccountsDeps, AccountsState, ClientInfo, GitHubOutcome, Principal, SessionGrant, SetupError,
    SignedIn,
};
pub use tenant::TenantRepo;
