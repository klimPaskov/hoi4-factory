//! C-10 `RepositoryLink` types and the per-project repository allowlist.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::IntakeError;
use crate::secrets::valid_secret_name;

/// State of a repository link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// Recorded before the import starts.
    Pending,
    /// Imported. Remote operations are allowed.
    Active,
    /// GitHub says the installation or repository is gone (HTTP 401 or 404). The server
    /// workspace is not touched.
    AccessRevoked,
    /// The user disconnected GitHub.
    Disconnected,
    /// The import failed for another reason. See `state_reason`.
    Failed,
}

impl LinkState {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkState::Pending => "pending",
            LinkState::Active => "active",
            LinkState::AccessRevoked => "access_revoked",
            LinkState::Disconnected => "disconnected",
            LinkState::Failed => "failed",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "pending" => LinkState::Pending,
            "active" => LinkState::Active,
            "access_revoked" => LinkState::AccessRevoked,
            "disconnected" => LinkState::Disconnected,
            "failed" => LinkState::Failed,
            _ => return None,
        })
    }
}

/// A remote ref and the commit it pointed at when intake last looked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedRef {
    /// Full ref name, such as `refs/heads/main`.
    pub name: String,
    pub sha: String,
}

/// Name of the secret-store entry that holds the installation id. It is a key name, never
/// a credential: names that look like a token are refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstallationRef(String);

impl InstallationRef {
    pub fn new(name: impl Into<String>) -> Result<Self, IntakeError> {
        let name = name.into();
        if !valid_secret_name(&name) {
            return Err(IntakeError::InvalidRequest(
                "installation reference is not a valid secret-store key name".into(),
            ));
        }
        Ok(InstallationRef(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for InstallationRef {
    type Error = IntakeError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        InstallationRef::new(value)
    }
}

impl From<InstallationRef> for String {
    fn from(value: InstallationRef) -> String {
        value.0
    }
}

/// Split and validate `owner/name`. GitHub owners are 1 to 39 ASCII letters, digits and
/// hyphens, not starting or ending with a hyphen. Repository names are 1 to 100 letters,
/// digits, `.`, `_` and `-`, and are not `.` or `..`.
pub fn parse_full_name(text: &str) -> Option<(&str, &str)> {
    let (owner, name) = text.split_once('/')?;
    let owner_ok = (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-');
    let name_ok = (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && name != "."
        && name != "..";
    (owner_ok && name_ok).then_some((owner, name))
}

/// Repositories a project may import from. Matching ignores case, as GitHub does. An empty
/// allowlist allows nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryAllowlist {
    names: BTreeSet<String>,
}

impl RepositoryAllowlist {
    pub fn new<I, S>(names: I) -> Result<Self, IntakeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = BTreeSet::new();
        for name in names {
            let name = name.as_ref();
            if parse_full_name(name).is_none() {
                return Err(IntakeError::InvalidRequest(
                    "allowlist entry is not owner/name".into(),
                ));
            }
            set.insert(name.to_lowercase());
        }
        Ok(RepositoryAllowlist { names: set })
    }

    pub fn contains(&self, full_name: &str) -> bool {
        self.names.contains(&full_name.to_lowercase())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }
}

/// The only repository selection intake accepts.
pub const SELECTED_SCOPE: &str = "selected_repositories";

/// C-10 `RepositoryLink` as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryLink {
    pub id: Uuid,
    pub account_id: Uuid,
    pub project_id: Uuid,
    pub workspace_id: String,
    pub provider: String,
    pub repository_full_name: String,
    /// Known once GitHub has described the repository.
    pub repository_node_id: Option<String>,
    pub repository_id: Option<i64>,
    pub installation_ref: String,
    pub selected_scope: String,
    pub default_branch: Option<String>,
    pub imported_branch: Option<String>,
    pub imported_commit: Option<String>,
    pub observed_refs: Vec<ObservedRef>,
    pub observed_at: Option<DateTime<Utc>>,
    pub state: LinkState,
    pub state_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
