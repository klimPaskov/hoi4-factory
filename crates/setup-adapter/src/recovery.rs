//! Recovery variants, mapped one to one from what the Setup transaction permits.
//!
//! The facade reports the single action the native journal allows: `Resume`, `Rollback` or
//! `DiscardStaging`, or nothing when the journal needs manual review. A generic "resume" flag
//! would offer resume after a partial apply, which the transaction refuses. The variants here
//! name the situation, and each one permits exactly one action.
//!
//! | Variant | Native situation | Only action |
//! |---|---|---|
//! | `ResumableStaging` | staging finished, nothing reached the project | resume |
//! | `DiscardableStaging` | staging unfinished, nothing reached the project | discard staging |
//! | `RollbackRequiredPartialApply` | at least one operation reached the project | rollback |
//! | `VerifiableFinalization` | all operations applied, the transaction was finalizing | resume (verifies the lock and the rollback record) |
//! | `ManualReview` | the journal permits nothing, or contradicts itself | none |

use std::collections::BTreeMap;

use hoi4_mod_setup::models::TransactionJournal;
use hoi4_mod_setup::service::{RecoveryAction, RecoveryStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryVariant {
    ResumableStaging,
    DiscardableStaging,
    RollbackRequiredPartialApply,
    VerifiableFinalization,
    ManualReview,
}

impl RecoveryVariant {
    pub const ALL: [RecoveryVariant; 5] = [
        Self::ResumableStaging,
        Self::DiscardableStaging,
        Self::RollbackRequiredPartialApply,
        Self::VerifiableFinalization,
        Self::ManualReview,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResumableStaging => "resumable_staging",
            Self::DiscardableStaging => "discardable_staging",
            Self::RollbackRequiredPartialApply => "rollback_required_partial_apply",
            Self::VerifiableFinalization => "verifiable_finalization",
            Self::ManualReview => "manual_review",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == text)
    }

    /// The one native action this variant permits.
    pub fn action(self) -> Option<RecoveryAction> {
        match self {
            Self::ResumableStaging | Self::VerifiableFinalization => Some(RecoveryAction::Resume),
            Self::DiscardableStaging => Some(RecoveryAction::DiscardStaging),
            Self::RollbackRequiredPartialApply => Some(RecoveryAction::Rollback),
            Self::ManualReview => None,
        }
    }

    /// What the person sees. It never offers an action the variant does not permit.
    pub fn explanation(self) -> &'static str {
        match self {
            Self::ResumableStaging => {
                "Setup was interrupted after its files were staged and before the project was \
                 touched. Resume it after the project, the plan hash and the approval are \
                 checked again, or discard the staging."
            }
            Self::DiscardableStaging => {
                "Setup was interrupted while staging files. Nothing reached the project. The \
                 staging can be discarded and the plan rebuilt."
            }
            Self::RollbackRequiredPartialApply => {
                "Setup was interrupted after it started changing the project. The only safe \
                 action is to roll back to the files as they were before. It cannot be resumed."
            }
            Self::VerifiableFinalization => {
                "Setup applied every file and was interrupted while finalizing. Resuming checks \
                 the install lock and the rollback record before it reports success."
            }
            Self::ManualReview => {
                "The transaction record permits no automatic recovery. Review it manually; no \
                 files were changed by this check."
            }
        }
    }
}

/// Whether the journal shows an operation that reached the project. Mirrors the transaction's
/// own derivation, used only to refuse contradictory records.
fn apply_started(journal: &TransactionJournal) -> bool {
    journal.recovery.project_apply_started
        || journal.operations.iter().any(|operation| {
            matches!(
                operation.status.as_str(),
                "applying" | "applied" | "verified" | "rollback_applying" | "rolled_back"
            )
        })
}

/// Map the facade's answer to a variant. Anything contradictory is `ManualReview`.
pub fn classify(status: &RecoveryStatus) -> RecoveryVariant {
    classify_parts(status.allowed_action, &status.journal)
}

pub(crate) fn classify_parts(
    allowed: Option<RecoveryAction>,
    journal: &TransactionJournal,
) -> RecoveryVariant {
    let finalizing = journal.state == "finalizing";
    match allowed {
        None => RecoveryVariant::ManualReview,
        Some(RecoveryAction::Resume) if finalizing => RecoveryVariant::VerifiableFinalization,
        Some(RecoveryAction::Resume) if !apply_started(journal) => {
            RecoveryVariant::ResumableStaging
        }
        Some(RecoveryAction::Resume) => RecoveryVariant::ManualReview,
        Some(RecoveryAction::DiscardStaging) if !apply_started(journal) => {
            RecoveryVariant::DiscardableStaging
        }
        Some(RecoveryAction::DiscardStaging) => RecoveryVariant::ManualReview,
        Some(RecoveryAction::Rollback) => RecoveryVariant::RollbackRequiredPartialApply,
    }
}

/// Read-only description of an incomplete transaction, derived from the native journal.
#[derive(Debug, Clone, Serialize)]
pub struct RecoveryView {
    pub journal_id: Uuid,
    pub variant: RecoveryVariant,
    /// `resume`, `rollback` or `discard_staging`; `None` for manual review.
    pub allowed_action: Option<&'static str>,
    pub explanation: &'static str,
    pub journal_state: String,
    pub last_checkpoint: String,
    pub transaction_kind: String,
    pub project_apply_started: bool,
    pub plan_sha256: Option<String>,
    /// Number of operations per journal status.
    pub operation_statuses: BTreeMap<String, u32>,
    pub journal_sha256: String,
    /// Recovery reads the native journal and needs no model.
    pub needs_model: bool,
    /// Another owner holds the project lease right now (an apply or recovery is running). The
    /// journal is in motion, so no action is offered until that owner finishes.
    pub owner_active: bool,
}

impl RecoveryView {
    /// The same view under a different variant (and so a different permitted action).
    pub fn with_variant(mut self, variant: RecoveryVariant) -> Self {
        self.variant = variant;
        self.allowed_action = variant.action().map(RecoveryAction::as_str);
        self.explanation = variant.explanation();
        self
    }

    pub fn from_status(status: &RecoveryStatus) -> Self {
        let journal = &status.journal;
        let variant = classify(status);
        let mut operation_statuses = BTreeMap::new();
        for operation in &journal.operations {
            *operation_statuses
                .entry(operation.status.clone())
                .or_insert(0u32) += 1;
        }
        Self {
            journal_id: journal.transaction_id,
            variant,
            allowed_action: variant.action().map(RecoveryAction::as_str),
            explanation: variant.explanation(),
            journal_state: journal.state.clone(),
            last_checkpoint: journal.last_checkpoint.clone(),
            transaction_kind: journal.transaction_kind.clone(),
            project_apply_started: apply_started(journal),
            plan_sha256: journal.plan_sha256.clone(),
            operation_statuses,
            journal_sha256: journal_sha256(journal),
            needs_model: false,
            owner_active: false,
        }
    }
}

/// SHA-256 of the journal's canonical JSON. The journal itself stays in the state root.
pub fn journal_sha256(journal: &TransactionJournal) -> String {
    let bytes = serde_json::to_vec(journal).unwrap_or_default();
    hex(&Sha256::digest(bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn journal(state: &str, op_status: &str, apply_started: bool) -> TransactionJournal {
        serde_json::from_value(json!({
            "schema_version": "1.0.0",
            "transaction_id": Uuid::new_v4(),
            "project_id": "example",
            "state": state,
            "created_at": "2026-10-24T00:00:00Z",
            "updated_at": "2026-10-24T00:00:00Z",
            "last_checkpoint": "x",
            "stages": [],
            "operations": [{
                "id": "op-1",
                "destination": "AGENTS.md",
                "status": op_status,
                "action": "create",
            }],
            "recovery": {
                "resume_allowed": false,
                "rollback_allowed": false,
                "discard_staging_allowed": false,
                "project_apply_started": apply_started,
                "recommended_action": "none",
            },
        }))
        .expect("journal fixture parses")
    }

    #[test]
    fn each_native_action_maps_to_one_variant_and_back() {
        let pre = journal("staging", "pending", false);
        let started = journal("applying", "applied", true);
        let finalizing = journal("finalizing", "verified", true);

        assert_eq!(
            classify_parts(Some(RecoveryAction::Resume), &pre),
            RecoveryVariant::ResumableStaging
        );
        assert_eq!(
            classify_parts(Some(RecoveryAction::DiscardStaging), &pre),
            RecoveryVariant::DiscardableStaging
        );
        assert_eq!(
            classify_parts(Some(RecoveryAction::Rollback), &started),
            RecoveryVariant::RollbackRequiredPartialApply
        );
        assert_eq!(
            classify_parts(Some(RecoveryAction::Resume), &finalizing),
            RecoveryVariant::VerifiableFinalization
        );
        assert_eq!(classify_parts(None, &pre), RecoveryVariant::ManualReview);
        // The action of each variant round-trips.
        assert_eq!(
            RecoveryVariant::ResumableStaging.action(),
            Some(RecoveryAction::Resume)
        );
        assert_eq!(
            RecoveryVariant::VerifiableFinalization.action(),
            Some(RecoveryAction::Resume)
        );
        assert_eq!(
            RecoveryVariant::DiscardableStaging.action(),
            Some(RecoveryAction::DiscardStaging)
        );
        assert_eq!(
            RecoveryVariant::RollbackRequiredPartialApply.action(),
            Some(RecoveryAction::Rollback)
        );
        assert_eq!(RecoveryVariant::ManualReview.action(), None);
    }

    #[test]
    fn resume_after_a_partial_apply_is_never_offered() {
        // A record that says "resume" although an operation reached the project contradicts
        // the transaction's own rules. It is manual review, not a resume.
        let started = journal("applying", "applied", true);
        assert_eq!(
            classify_parts(Some(RecoveryAction::Resume), &started),
            RecoveryVariant::ManualReview
        );
        assert_eq!(
            classify_parts(Some(RecoveryAction::DiscardStaging), &started),
            RecoveryVariant::ManualReview
        );
    }

    #[test]
    fn variants_round_trip_through_text() {
        for v in RecoveryVariant::ALL {
            assert_eq!(RecoveryVariant::parse(v.as_str()), Some(v));
        }
        assert_eq!(RecoveryVariant::parse("resume"), None);
    }
}
