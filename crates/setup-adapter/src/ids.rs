//! Typed identifiers and the per-request context.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use factory_accounts::{AccountId, ProjectId};

macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
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

uuid_id!(
    /// One onboarding session of one account on one project. It owns one facade session.
    OnboardingSessionId
);
uuid_id!(
    /// A Setup scan, equal to the scan ID Setup assigned.
    ScanId
);
uuid_id!(
    /// A Setup analysis, equal to the analysis ID Setup assigned (never chosen by a model).
    AnalysisId
);
uuid_id!(
    /// A prepared plan, equal to the plan ID Setup assigned. It is also the ID of the native
    /// transaction that applies it.
    PlanId
);
uuid_id!(
    /// A native transaction journal ID.
    JournalId
);
uuid_id!(
    /// A C-04 style receipt row.
    ReceiptId
);

/// Request fields of the common envelope that the adapter records in its receipts. None of
/// them grants anything by itself: the server validates the grant before it calls.
#[derive(Debug, Clone, Default)]
pub struct OpContext {
    /// Request ID. Generated when absent.
    pub request_id: Option<Uuid>,
    /// Caller idempotency key. Effectful operations default to a key derived from the subject.
    pub idempotency_key: Option<String>,
    /// C-08 grant the server checked before calling.
    pub grant_id: Option<String>,
    /// Revocation epoch of that grant when it was checked.
    pub revocation_epoch: Option<i64>,
}

impl OpContext {
    pub fn with_request_id(mut self, id: Uuid) -> Self {
        self.request_id = Some(id);
        self
    }

    pub fn with_grant(mut self, grant_id: impl Into<String>, epoch: i64) -> Self {
        self.grant_id = Some(grant_id.into());
        self.revocation_epoch = Some(epoch);
        self
    }

    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}
