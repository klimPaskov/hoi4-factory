//! Upload intake and GitHub import for HOI4 Factory (S02-W3, backend).
//!
//! * [`UploadIntake`]: a ZIP archive or a folder upload becomes a fresh workspace `source/`
//!   under strict name, type and size rules. The original upload is kept as received in a
//!   separate uploads area. Git is initialised only with explicit consent. An uploaded
//!   `.git` is used only after `factory-workspace`'s configuration checks pass.
//! * [`GithubImporter`]: lists the repositories a GitHub App installation can see and
//!   clones one selected repository and branch with a short-lived installation token
//!   minted for that repository alone. No push, no pull request.
//! * [`IntakeStore`] and [`IntakeService`]: PostgreSQL records (uploads, C-10
//!   `RepositoryLink`, per-project allowlist) behind row-level security keyed on
//!   `app.account_id`, and the operations that tie them to the work.
//!
//! The crate opens no listening socket. The server supplies the storage root, the
//! database pool, a [`SecretSource`] and, for GitHub, an HTTP client through [`GithubApi`].
//! Migrations are in the repository's `migrations/` directory, versions 0300 to 0302.

mod config;
mod error;
mod extract;
mod gitcmd;
mod gitfinal;
pub mod github;
mod model;
mod plan;
mod secrets;
mod service;
mod store;
mod upload;
mod util;
mod zip;

pub use config::{FileTypePolicy, IntakeLimits, TypeVerdict};
pub use error::{IntakeError, RetryClass};
pub use github::{
    GithubApi, GithubConfig, GithubEndpoints, GithubError, GithubImporter, HttpGithubApi,
    ImportOutcome, ImportRequest, InstallationInfo, InstallationRef, InstallationToken, LinkState,
    ObservedRef, PreparedImport, RepositoryAllowlist, RepositoryInfo, RepositoryLink,
    VisibleRepository, app_jwt, parse_full_name,
};
pub use model::{
    ExtractionSummary, Finding, GitInitRequest, GitOutcome, InventorySummary, StoredOriginal,
    UploadFailure, UploadKind, UploadMeta, UploadOptions, UploadOutcome,
};
pub use secrets::{
    SecretError, SecretSource, SecretString, StaticSecrets, looks_like_credential,
    valid_secret_name,
};
pub use service::{GithubImportRequest, IntakeService};
pub use store::{IntakeStore, UploadRecord};
pub use upload::{EntryData, FolderEntry, UploadIntake, UploadTarget};
pub use zip::DeclaredKind;

/// Directory of the SQL migrations for this crate, relative to the repository root.
pub const MIGRATIONS_DIR: &str = "migrations";
/// First and last migration version this crate owns.
pub const MIGRATION_VERSIONS: std::ops::RangeInclusive<u32> = 300..=399;
