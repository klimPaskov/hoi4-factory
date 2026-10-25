//! C-06 context bundles: a pure builder, no I/O.
//!
//! `build_context` takes the already-gathered inputs (task scope, source locators with their
//! hashes and optional content, approved references, receipt summaries, visible messages and the
//! capability profile) and a token budget, and returns a frozen bundle.
//!
//! Mandatory content is never truncated: the task scope, the safety constraints, unresolved
//! effects, the capability profile and every `required` source locator. If it does not fit the
//! budget the builder returns [`ContextError::RequiredContextTooLarge`] so the host can offer a
//! narrower task. Optional content is added by priority until the budget is met; every item left
//! out is recorded in `manifest.omissions` with a reason.
//!
//! Priority of optional content, highest first: approved references, optional source locators
//! (by their `priority`, then path), recent messages (newest first), completed receipt
//! summaries. Lower priority content is dropped first because it is the most derived.
//!
//! Token counts are estimates: `ceil(bytes / 4)` of each item's canonical JSON. The manifest says
//! so. The bundle has no field for credential values or provider-private reasoning, so it
//! cannot carry them. The manifest hash covers the manifest, the source versions and a hash of
//! the content, and excludes the bundle ID, so identical inputs give identical hashes.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::canon::{canonical_json, canonical_sha256, is_sha256_hex, sha256_hex};
use crate::enums::{ReceiptStatus, TaskMode};
use crate::ids::{BundleId, OperationId};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const TOKEN_ESTIMATE_METHOD: &str = "ceil(canonical_json_bytes / 4)";

/// Safety constraints present in every bundle. They are facts about the frame, not grants.
pub const SAFETY_CONSTRAINTS: &[&str] = &[
    "This bundle grants no authority. Tools run only under a grant checked by the host immediately before each effect.",
    "Writes are limited to the task's allowed paths inside its own worktree; the mod's main source tree is never written.",
    "A task is not complete because the model says so. A successful attempt produces a draft for review.",
    "An operation with an unresolved effect must be reconciled before it is repeated; never repeat it blindly.",
    "Do not invent file contents, hashes or tool results. Cite source locators from this bundle.",
];

/// Task scope: what was asked and what may be touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskScope {
    pub objective: String,
    pub mode: TaskMode,
    pub allowed_paths: Vec<String>,
    pub requirement_ids: Vec<String>,
}

/// A byte or line region of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "unit", rename_all = "snake_case")]
pub enum Region {
    Bytes { start: u64, end: u64 },
    Lines { start: u32, end: u32 },
}

/// A pointer to source with the version it was read at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocator {
    /// Relative, forward-slash path inside the workspace.
    pub path: String,
    /// SHA-256 of the whole file at the time of retrieval.
    pub sha256: String,
    pub bytes: u64,
    pub region: Option<Region>,
    /// Why retrieval chose this source.
    pub reason: String,
    /// Required locators are mandatory content.
    pub required: bool,
    /// Lower value, higher priority, among optional locators.
    #[serde(default)]
    pub priority: u32,
    /// The text of the region (or the whole file when `region` is `None`), when included.
    pub content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Canon,
    Rule,
}

/// An approved canon or rule reference. May be empty in S03.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub id: String,
    pub kind: ReferenceKind,
    pub version: String,
    pub owner: String,
    pub approval_revision: String,
    pub text: String,
}

/// A factual summary of an operation, from its intent and receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptSummary {
    pub operation_id: OperationId,
    pub tool: String,
    pub status: ReceiptStatus,
    pub args_sha256: String,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
    pub partial: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

/// A visible conversation message. Provider-private reasoning has no representation here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBudget {
    /// The provider's context window for this profile.
    pub context_tokens: u64,
    pub reserved_output: u64,
    pub reserved_tools: u64,
}

impl TokenBudget {
    pub fn available(&self) -> u64 {
        self.context_tokens
            .saturating_sub(self.reserved_output)
            .saturating_sub(self.reserved_tools)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextInputs {
    pub scope: TaskScope,
    pub source_locators: Vec<SourceLocator>,
    pub approved_references: Vec<Reference>,
    pub completed_receipts: Vec<ReceiptSummary>,
    pub unresolved_effects: Vec<ReceiptSummary>,
    /// Oldest first.
    pub recent_messages: Vec<VisibleMessage>,
    /// Opaque to the ledger: the tools actually available, with versions and what is unavailable.
    pub capability_profile: Value,
    pub budget: TokenBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextError {
    /// The mandatory part does not fit. The host should offer a narrower task or retrieval step.
    #[error(
        "required context needs about {required_tokens} tokens but {available_tokens} are available"
    )]
    RequiredContextTooLarge {
        required_tokens: u64,
        available_tokens: u64,
    },
    #[error("source locator {path:?}: {why}")]
    InvalidLocator { path: String, why: String },
    /// Inline content does not match the hash the locator claims: the source changed or the
    /// locator is wrong.
    #[error("source {path:?} content does not match its recorded hash")]
    SourceMismatch { path: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Scope,
    SafetyConstraints,
    CapabilityProfile,
    UnresolvedEffect,
    SourceLocator,
    SourceContent,
    ApprovedReference,
    CompletedReceipt,
    RecentMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncludedItem {
    pub kind: ItemKind,
    pub id: String,
    pub tokens: u64,
    pub mandatory: bool,
    /// Retrieval reason for sources, empty otherwise.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
    /// Left out because the budget was met by higher-priority content.
    BudgetExceeded,
    /// The same locator was given twice.
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Omission {
    pub kind: ItemKind,
    pub id: String,
    pub reason: OmissionReason,
    /// Estimated tokens the item would have used.
    pub tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCalculation {
    pub context_tokens: u64,
    pub reserved_output: u64,
    pub reserved_tools: u64,
    pub available: u64,
    pub mandatory_tokens: u64,
    pub optional_tokens: u64,
    pub remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEstimate {
    pub method: String,
    /// Always true: these are not provider token counts.
    pub is_estimate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVersion {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub region: Option<Region>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub included: Vec<IncludedItem>,
    pub omissions: Vec<Omission>,
    pub budget: BudgetCalculation,
    pub token_estimate: TokenEstimate,
    pub source_versions: Vec<SourceVersion>,
    /// Hash of the canonical content, so the manifest hash also binds what the model sees.
    pub content_sha256: String,
}

/// The material a provider request renders from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleContent {
    pub scope: TaskScope,
    pub safety_constraints: Vec<String>,
    pub capability_profile: Value,
    pub unresolved_effects: Vec<ReceiptSummary>,
    pub approved_references: Vec<Reference>,
    pub sources: Vec<SourceLocator>,
    pub completed_receipts: Vec<ReceiptSummary>,
    pub recent_messages: Vec<VisibleMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBundle {
    pub id: BundleId,
    pub manifest: Manifest,
    pub manifest_sha256: String,
    pub content: BundleContent,
}

/// SHA-256 of the canonical JSON of a manifest.
pub fn manifest_hash(manifest: &Manifest) -> String {
    // Serializing plain data cannot fail.
    canonical_sha256(&serde_json::to_value(manifest).unwrap_or(Value::Null))
}

/// SHA-256 of the canonical JSON of the content.
pub fn content_hash(content: &BundleContent) -> String {
    canonical_sha256(&serde_json::to_value(content).unwrap_or(Value::Null))
}

impl ContextBundle {
    /// Recomputes both hashes. `Ok` when the bundle is exactly what was hashed.
    pub fn verify(&self) -> Result<(), String> {
        let content = content_hash(&self.content);
        if content != self.manifest.content_sha256 {
            return Err(content);
        }
        let manifest = manifest_hash(&self.manifest);
        if manifest != self.manifest_sha256 {
            return Err(manifest);
        }
        Ok(())
    }
}

/// Estimated tokens: `ceil(bytes / 4)` of the item's canonical JSON, at least 1.
pub fn estimate_tokens<T: Serialize>(item: &T) -> u64 {
    let json = serde_json::to_value(item).unwrap_or(Value::Null);
    (canonical_json(&json).len() as u64).div_ceil(4).max(1)
}

fn validate_locator(l: &SourceLocator) -> Result<(), ContextError> {
    let bad = |why: &str| {
        Err(ContextError::InvalidLocator {
            path: l.path.clone(),
            why: why.to_owned(),
        })
    };
    if l.path.is_empty()
        || l.path.starts_with('/')
        || l.path.contains('\\')
        || l.path.split('/').any(|s| s.is_empty() || s == "." || s == "..")
    {
        return bad("the path must be relative with forward slashes and no . or .. segment");
    }
    if !is_sha256_hex(&l.sha256) {
        return bad("the hash must be 64 lowercase hex digits");
    }
    if let Some(content) = &l.content
        && l.region.is_none()
        && (content.len() as u64 != l.bytes || sha256_hex(content.as_bytes()) != l.sha256)
    {
        return Err(ContextError::SourceMismatch {
            path: l.path.clone(),
        });
    }
    Ok(())
}

fn source_version(l: &SourceLocator) -> SourceVersion {
    SourceVersion {
        path: l.path.clone(),
        sha256: l.sha256.clone(),
        bytes: l.bytes,
        region: l.region,
    }
}

fn locator_id(l: &SourceLocator) -> String {
    match l.region {
        None => l.path.clone(),
        Some(Region::Bytes { start, end }) => format!("{}#bytes={start}-{end}", l.path),
        Some(Region::Lines { start, end }) => format!("{}#lines={start}-{end}", l.path),
    }
}

/// Builds a bundle with a fresh ID. See the module docs for the rules.
pub fn build_context(inputs: ContextInputs) -> Result<ContextBundle, ContextError> {
    build_context_with_id(inputs, BundleId::new())
}

/// [`build_context`] with a caller-chosen ID (for fixtures and tests).
pub fn build_context_with_id(
    inputs: ContextInputs,
    id: BundleId,
) -> Result<ContextBundle, ContextError> {
    let ContextInputs {
        scope,
        source_locators,
        mut approved_references,
        mut completed_receipts,
        mut unresolved_effects,
        recent_messages,
        capability_profile,
        budget,
    } = inputs;

    for l in &source_locators {
        validate_locator(l)?;
    }

    // Deterministic order, independent of how the caller gathered things.
    let mut scope = scope;
    scope.allowed_paths.sort();
    scope.requirement_ids.sort();
    approved_references.sort_by(|a, b| a.id.cmp(&b.id));
    completed_receipts.sort_by_key(|r| r.operation_id);
    unresolved_effects.sort_by_key(|r| r.operation_id);

    let mut included: Vec<IncludedItem> = Vec::new();
    let mut omissions: Vec<Omission> = Vec::new();
    let mut mandatory_tokens = 0u64;

    let safety: Vec<String> = SAFETY_CONSTRAINTS.iter().map(|s| (*s).to_owned()).collect();
    let mut mandatory = |kind, id: String, tokens: u64, reason: String| {
        mandatory_tokens += tokens;
        included.push(IncludedItem {
            kind,
            id,
            tokens,
            mandatory: true,
            reason,
        });
    };
    mandatory(ItemKind::Scope, "scope".into(), estimate_tokens(&scope), String::new());
    mandatory(
        ItemKind::SafetyConstraints,
        "safety_constraints".into(),
        estimate_tokens(&safety),
        String::new(),
    );
    mandatory(
        ItemKind::CapabilityProfile,
        "capability_profile".into(),
        estimate_tokens(&capability_profile),
        String::new(),
    );
    for r in &unresolved_effects {
        mandatory(
            ItemKind::UnresolvedEffect,
            r.operation_id.to_string(),
            estimate_tokens(r),
            String::new(),
        );
    }

    // Required locators are mandatory, with their content. Sort, then drop exact duplicates.
    let mut locators = source_locators;
    locators.sort_by(|a, b| {
        (!a.required, a.priority, locator_id(a)).cmp(&(!b.required, b.priority, locator_id(b)))
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut required: Vec<SourceLocator> = Vec::new();
    let mut optional: Vec<SourceLocator> = Vec::new();
    for l in locators {
        let key = locator_id(&l);
        if !seen.insert(key.clone()) {
            omissions.push(Omission {
                kind: ItemKind::SourceLocator,
                id: key,
                reason: OmissionReason::Duplicate,
                tokens: estimate_tokens(&l),
            });
            continue;
        }
        if l.required {
            required.push(l);
        } else {
            optional.push(l);
        }
    }
    for l in &required {
        mandatory(
            ItemKind::SourceLocator,
            locator_id(l),
            estimate_tokens(l),
            l.reason.clone(),
        );
    }

    let available = budget.available();
    if mandatory_tokens > available {
        return Err(ContextError::RequiredContextTooLarge {
            required_tokens: mandatory_tokens,
            available_tokens: available,
        });
    }

    let mut optional_tokens = 0u64;
    let room = |used: u64, tokens: u64| mandatory_tokens + used + tokens <= available;

    let mut references_out = Vec::new();
    for r in approved_references {
        let tokens = estimate_tokens(&r);
        if room(optional_tokens, tokens) {
            optional_tokens += tokens;
            included.push(IncludedItem {
                kind: ItemKind::ApprovedReference,
                id: r.id.clone(),
                tokens,
                mandatory: false,
                reason: String::new(),
            });
            references_out.push(r);
        } else {
            omissions.push(Omission {
                kind: ItemKind::ApprovedReference,
                id: r.id,
                reason: OmissionReason::BudgetExceeded,
                tokens,
            });
        }
    }

    let mut sources_out: Vec<SourceLocator> = required;
    for mut l in optional {
        let id = locator_id(&l);
        let full = estimate_tokens(&l);
        if room(optional_tokens, full) {
            optional_tokens += full;
            included.push(IncludedItem {
                kind: ItemKind::SourceLocator,
                id,
                tokens: full,
                mandatory: false,
                reason: l.reason.clone(),
            });
            sources_out.push(l);
            continue;
        }
        // Keep the pointer without its text when only the pointer fits.
        let content = l.content.take();
        let pointer = estimate_tokens(&l);
        if content.is_some() && room(optional_tokens, pointer) {
            optional_tokens += pointer;
            omissions.push(Omission {
                kind: ItemKind::SourceContent,
                id: id.clone(),
                reason: OmissionReason::BudgetExceeded,
                tokens: full - pointer.min(full),
            });
            included.push(IncludedItem {
                kind: ItemKind::SourceLocator,
                id,
                tokens: pointer,
                mandatory: false,
                reason: l.reason.clone(),
            });
            sources_out.push(l);
        } else {
            omissions.push(Omission {
                kind: ItemKind::SourceLocator,
                id,
                reason: OmissionReason::BudgetExceeded,
                tokens: full,
            });
        }
    }

    // Messages: newest first so the oldest are dropped first, restored to conversation order.
    let mut messages_out: Vec<(usize, VisibleMessage)> = Vec::new();
    for (index, m) in recent_messages.into_iter().enumerate().rev() {
        let tokens = estimate_tokens(&m);
        if room(optional_tokens, tokens) {
            optional_tokens += tokens;
            included.push(IncludedItem {
                kind: ItemKind::RecentMessage,
                id: format!("message:{index}"),
                tokens,
                mandatory: false,
                reason: String::new(),
            });
            messages_out.push((index, m));
        } else {
            omissions.push(Omission {
                kind: ItemKind::RecentMessage,
                id: format!("message:{index}"),
                reason: OmissionReason::BudgetExceeded,
                tokens,
            });
        }
    }
    messages_out.sort_by_key(|(i, _)| *i);

    let mut receipts_out = Vec::new();
    for r in completed_receipts {
        let tokens = estimate_tokens(&r);
        if room(optional_tokens, tokens) {
            optional_tokens += tokens;
            included.push(IncludedItem {
                kind: ItemKind::CompletedReceipt,
                id: r.operation_id.to_string(),
                tokens,
                mandatory: false,
                reason: String::new(),
            });
            receipts_out.push(r);
        } else {
            omissions.push(Omission {
                kind: ItemKind::CompletedReceipt,
                id: r.operation_id.to_string(),
                reason: OmissionReason::BudgetExceeded,
                tokens,
            });
        }
    }

    let source_versions: Vec<SourceVersion> = sources_out.iter().map(source_version).collect();
    let content = BundleContent {
        scope,
        safety_constraints: safety,
        capability_profile,
        unresolved_effects,
        approved_references: references_out,
        sources: sources_out,
        completed_receipts: receipts_out,
        recent_messages: messages_out.into_iter().map(|(_, m)| m).collect(),
    };
    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        included,
        omissions,
        budget: BudgetCalculation {
            context_tokens: budget.context_tokens,
            reserved_output: budget.reserved_output,
            reserved_tools: budget.reserved_tools,
            available,
            mandatory_tokens,
            optional_tokens,
            remaining: available - mandatory_tokens - optional_tokens,
        },
        token_estimate: TokenEstimate {
            method: TOKEN_ESTIMATE_METHOD.to_owned(),
            is_estimate: true,
        },
        source_versions,
        content_sha256: content_hash(&content),
    };
    let manifest_sha256 = manifest_hash(&manifest);
    Ok(ContextBundle {
        id,
        manifest,
        manifest_sha256,
        content,
    })
}

/// A rendering helper for tests and fixtures: the bundle as plain JSON.
pub fn bundle_json(bundle: &ContextBundle) -> Value {
    json!(bundle)
}
