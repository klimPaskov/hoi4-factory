//! Typed identifiers. A workspace path is built only from these, never from a client
//! string, so a client cannot name a directory outside its own project.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::WorkspaceError;

const MAX_ID_LEN: usize = 64;

const RESERVED_DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Validate the shared identifier alphabet: lowercase ASCII letters, digits, `-` and `_`,
/// 1 to 64 characters, starting with a letter or digit. Lowercase only, so two ids can
/// never collide on a case-insensitive file system. UUIDs in hyphenated lowercase form
/// are valid.
pub(crate) fn validate_id(kind: &'static str, value: &str) -> Result<(), WorkspaceError> {
    let bad = |reason: &str| WorkspaceError::InvalidId {
        kind,
        reason: reason.to_owned(),
    };
    if value.is_empty() {
        return Err(bad("empty"));
    }
    if value.len() > MAX_ID_LEN {
        return Err(bad("longer than 64 characters"));
    }
    let first = value.as_bytes()[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(bad("must start with a lowercase letter or digit"));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(bad(
            "only lowercase letters, digits, '-' and '_' are allowed",
        ));
    }
    if RESERVED_DEVICE_NAMES.contains(&value) {
        return Err(bad("reserved device name"));
    }
    Ok(())
}

macro_rules! typed_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validate and wrap an identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceError> {
                let value = value.into();
                validate_id($kind, &value)?;
                Ok(Self(value))
            }

            /// The identifier text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = WorkspaceError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

typed_id!(
    /// Owning account.
    AccountId,
    "account id"
);
typed_id!(
    /// Project inside an account. One project has one server workspace.
    ProjectId,
    "project id"
);
typed_id!(
    /// Task. Names the owned branch, the task worktree and its restore points.
    TaskId,
    "task id"
);
typed_id!(
    /// Opaque workspace identity of C-01, verified against host registration.
    WorkspaceId,
    "workspace id"
);
typed_id!(
    /// Label of a restore point inside a task.
    RestoreLabel,
    "restore label"
);
