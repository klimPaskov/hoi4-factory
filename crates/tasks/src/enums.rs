//! Closed vocabularies stored as text. Unknown values fail to parse, so an unknown state read
//! from the database is an error, never a silent default.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} value {value:?}")]
pub struct ParseEnumError {
    pub kind: &'static str,
    pub value: String,
}

macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident, $kind:literal { $($(#[$vmeta:meta])* $var:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $text)] $var),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$var => $text),+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ParseEnumError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok($name::$var),)+
                    other => Err(ParseEnumError { kind: $kind, value: other.to_owned() }),
                }
            }
        }
    };
}

text_enum!(
    /// What a task may do to the workspace.
    TaskMode, "task mode" {
        ReadOnly => "read_only",
        Edit => "edit",
    }
);

text_enum!(
    /// Task state (SPEC-003). Pause is a separate durable condition, not a state.
    TaskState, "task state" {
        Planned => "planned",
        Ready => "ready",
        Running => "running",
        WaitingForInput => "waiting_for_input",
        InReview => "in_review",
        Completed => "completed",
        Blocked => "blocked",
        Cancelled => "cancelled",
    }
);

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Cancelled)
    }

    /// The explicit transition table. Completion is reachable only from review: an attempt that
    /// succeeded produces a draft, and acceptance policy (not the model) completes the task.
    pub fn can_transition_to(self, to: TaskState) -> bool {
        use TaskState::*;
        matches!(
            (self, to),
            (Planned, Ready | Blocked | Cancelled)
                | (Ready, Running | Planned | Blocked | Cancelled)
                | (
                    Running,
                    Ready | WaitingForInput | InReview | Blocked | Cancelled
                )
                | (WaitingForInput, Running | Ready | Blocked | Cancelled)
                | (InReview, Completed | Ready | Blocked | Cancelled)
                | (Blocked, Ready | Planned | Cancelled)
        )
    }
}

text_enum!(
    /// Attempt state (SPEC-003).
    AttemptState, "attempt state" {
        Admitted => "admitted",
        Running => "running",
        InterruptRequested => "interrupt_requested",
        Reconciling => "reconciling",
        Succeeded => "succeeded",
        Failed => "failed",
        Cancelled => "cancelled",
        UnknownEffect => "unknown_effect",
    }
);

impl AttemptState {
    /// Terminal attempts never change. `unknown_effect` is not terminal: it blocks the task's
    /// next attempt until reconciliation resolves it.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
        )
    }

    pub fn can_transition_to(self, to: AttemptState) -> bool {
        use AttemptState::*;
        matches!(
            (self, to),
            (Admitted, Running | Failed | Cancelled)
                | (
                    Running,
                    InterruptRequested | Reconciling | Succeeded | Failed | UnknownEffect
                )
                | (
                    InterruptRequested,
                    Reconciling | Cancelled | Failed | Succeeded | UnknownEffect
                )
                | (Reconciling, Cancelled | Failed | UnknownEffect)
                | (UnknownEffect, Reconciling | Failed | Cancelled)
        )
    }
}

text_enum!(
    /// Who pays for the model calls of an attempt.
    Funding, "funding" {
        OperatorAllowance => "operator_allowance",
        UserKey => "user_key",
    }
);

text_enum!(
    /// C-04 effect class. Unknown effects default to non-replayable.
    EffectClass, "effect class" {
        PureRead => "pure_read",
        DerivedJob => "derived_job",
        IdempotentWrite => "idempotent_write",
        UnknownEffect => "unknown_effect",
    }
);

text_enum!(
    /// The common envelope's retry classes.
    RetryClass, "retry class" {
        SafeSameInputs => "safe_same_inputs",
        RefreshPreconditions => "refresh_preconditions",
        ReconcileFirst => "reconcile_first",
        RequiresAuthorization => "requires_authorization",
        Terminal => "terminal",
    }
);

text_enum!(
    /// C-04 settlement status. Moves forward only.
    ReceiptStatus, "receipt status" {
        Started => "started",
        Succeeded => "succeeded",
        Unchanged => "unchanged",
        Refused => "refused",
        Failed => "failed",
        UnknownEffect => "unknown_effect",
        ReconciledApplied => "reconciled_applied",
        ReconciledNotApplied => "reconciled_not_applied",
        ReconciledConflict => "reconciled_conflict",
    }
);

impl ReceiptStatus {
    /// An operation whose effect is not established: it blocks the next attempt.
    pub fn is_unresolved(self) -> bool {
        matches!(self, ReceiptStatus::Started | ReceiptStatus::UnknownEffect)
    }

    pub fn can_transition_to(self, to: ReceiptStatus) -> bool {
        use ReceiptStatus::*;
        matches!(
            (self, to),
            (
                Started,
                Succeeded | Unchanged | Refused | Failed | UnknownEffect
            ) | (
                UnknownEffect,
                ReconciledApplied | ReconciledNotApplied | ReconciledConflict
            )
        )
    }
}

/// Who caused an event. Stored as `user:<uuid>`, `host` or `worker:<generation>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Actor {
    User(uuid::Uuid),
    Host,
    Worker(i64),
}

impl Actor {
    pub fn user(id: factory_accounts::AccountId) -> Self {
        Actor::User(id.0)
    }

    /// Worker output is never an acceptance decision.
    pub fn is_worker(&self) -> bool {
        matches!(self, Actor::Worker(_))
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actor::User(id) => write!(f, "user:{}", id.hyphenated()),
            Actor::Host => f.write_str("host"),
            Actor::Worker(generation) => write!(f, "worker:{generation}"),
        }
    }
}

impl FromStr for Actor {
    type Err = ParseEnumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || ParseEnumError {
            kind: "actor",
            value: s.to_owned(),
        };
        if s == "host" {
            return Ok(Actor::Host);
        }
        if let Some(id) = s.strip_prefix("user:") {
            return uuid::Uuid::parse_str(id)
                .map(Actor::User)
                .map_err(|_| bad());
        }
        if let Some(generation) = s.strip_prefix("worker:") {
            return generation.parse().map(Actor::Worker).map_err(|_| bad());
        }
        Err(bad())
    }
}

impl Serialize for Actor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Actor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_round_trips() {
        for s in TaskState::ALL {
            assert_eq!(s.as_str().parse::<TaskState>().unwrap(), *s);
        }
        for s in AttemptState::ALL {
            assert_eq!(s.as_str().parse::<AttemptState>().unwrap(), *s);
        }
        for s in ReceiptStatus::ALL {
            assert_eq!(s.as_str().parse::<ReceiptStatus>().unwrap(), *s);
            assert_eq!(
                serde_json::to_string(s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
        }
        assert!("nope".parse::<EffectClass>().is_err());
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for from in TaskState::ALL.iter().filter(|s| s.is_terminal()) {
            assert!(TaskState::ALL.iter().all(|to| !from.can_transition_to(*to)));
        }
        for from in AttemptState::ALL.iter().filter(|s| s.is_terminal()) {
            assert!(
                AttemptState::ALL
                    .iter()
                    .all(|to| !from.can_transition_to(*to))
            );
        }
        for from in [ReceiptStatus::Succeeded, ReceiptStatus::ReconciledApplied] {
            assert!(
                ReceiptStatus::ALL
                    .iter()
                    .all(|to| !from.can_transition_to(*to))
            );
        }
    }

    #[test]
    fn completion_only_from_review() {
        for from in TaskState::ALL {
            assert_eq!(
                from.can_transition_to(TaskState::Completed),
                *from == TaskState::InReview
            );
        }
    }

    #[test]
    fn actor_round_trip() {
        let id = uuid::Uuid::new_v4();
        for a in [Actor::Host, Actor::Worker(7), Actor::User(id)] {
            assert_eq!(a.to_string().parse::<Actor>().unwrap(), a);
        }
        assert!("root".parse::<Actor>().is_err());
    }
}
