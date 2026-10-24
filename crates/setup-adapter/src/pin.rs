//! Which HOI4-Mod-Setup revision this build runs, and what that revision is known to contain.
//!
//! The values come from the workspace `Cargo.toml` through `build.rs`, so they cannot drift
//! from the revision Cargo actually builds. Every session and receipt stores them, and a gate
//! that must be judged against a Setup revision with a given fix reads them from there.

use serde::Serialize;

/// The pinned Setup dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SetupPin {
    /// Git URL of the Setup repository.
    pub git: &'static str,
    /// `owner/name` of the repository.
    pub repository: &'static str,
    /// Full commit id Cargo builds.
    pub revision: &'static str,
    /// Branch the revision was taken from (informational).
    pub branch: &'static str,
    /// Pull request that carries the revision (informational).
    pub pull_request: &'static str,
    /// Head commit of the recovery-fix pull request (PR 105).
    pub pr_105_head: &'static str,
    /// Whether `revision` contains the PR 105 placement-temporary recovery fix. Without it a
    /// hard stop in the middle of apply can leave an extra `.hoi4ms-*.tmp` file after rollback,
    /// so preservation and recovery gates must not count against this revision.
    pub contains_pr_105_fix: bool,
    /// Whether `revision` ships a default `PlanBuilder`. Without one `planSetup` is unavailable
    /// unless the host injects its own builder.
    pub default_plan_builder: bool,
}

const fn parse_flag(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 4 && b[0] == b't' && b[1] == b'r' && b[2] == b'u' && b[3] == b'e'
}

/// The pin of this build.
pub const SETUP_PIN: SetupPin = SetupPin {
    git: env!("SETUP_PIN_GIT"),
    repository: env!("SETUP_PIN_REPOSITORY"),
    revision: env!("SETUP_PIN_REV"),
    branch: env!("SETUP_PIN_BRANCH"),
    pull_request: env!("SETUP_PIN_PULL_REQUEST"),
    pr_105_head: env!("SETUP_PIN_PR105_HEAD"),
    contains_pr_105_fix: parse_flag(env!("SETUP_PIN_HAS_PR105_FIX")),
    default_plan_builder: parse_flag(env!("SETUP_PIN_DEFAULT_PLAN_BUILDER")),
};

impl SetupPin {
    /// Evidence from apply and recovery counts toward gates S02-G2 and S02-G4 only against a
    /// revision that contains the PR 105 fix. This is the check a gate runner uses.
    pub fn counts_for_recovery_gates(&self) -> bool {
        self.contains_pr_105_fix
    }

    /// Short form for logs and receipts: `owner/name@<12 hex>`.
    pub fn short(&self) -> String {
        format!(
            "{}@{}",
            self.repository,
            &self.revision[..12.min(self.revision.len())]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_parse() {
        assert!(parse_flag("true"));
        assert!(!parse_flag("false"));
        assert!(!parse_flag("True"));
    }

    #[test]
    fn the_pin_is_a_full_revision() {
        assert_eq!(SETUP_PIN.revision.len(), 40);
        assert!(SETUP_PIN.git.starts_with("https://github.com/"));
    }
}
