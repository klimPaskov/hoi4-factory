//! Typed identifiers. They carry no privilege; every query also scopes by tenant.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type)]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.hyphenated().fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

id_type!(
    /// A task: one bounded outcome the user delegated.
    TaskId
);
id_type!(
    /// One run of a task. At most one attempt per task is nonterminal at a time.
    AttemptId
);
id_type!(
    /// A C-03 run event.
    EventId
);
id_type!(
    /// A C-06 context bundle.
    BundleId
);
id_type!(
    /// A C-04 operation: one tool effect, from intent to receipt.
    OperationId
);
id_type!(
    /// One start of the server process.
    IncarnationId
);
