//! One HOI4 Agent Tools session on a task worktree, with the host's guards around a rewrite.
//!
//! A rewrite runs only when every mod file the projection read still has the hash it had
//! (otherwise `stale_base` with the current hashes, and nothing is written). The host checks that
//! the tool changed nothing but the tree's source file, removes the tool's `.focus-plan.json`
//! sidecar when the rewrite created it (the source file is the truth, ADR decision S04-D3), and
//! writes the localisation of new focuses into the tree's English localisation file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use factory_hoi4_tools::profile::FOCUS_REWRITE;
use factory_hoi4_tools::{
    ToolError, ToolSession, agent_tools_focus_editor, agent_tools_spec, prepare_state_dir,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::ops::{FocusOp, OpError, PlanChange, apply_ops};
use super::projection::{BuildError, Built, build};
use crate::agent::AgentToolsSettings;

pub const ABSENT: &str = "absent";
const INSPECT_TIMEOUT: Duration = Duration::from_secs(240);
const REWRITE_TIMEOUT: Duration = Duration::from_secs(240);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum FocusError {
    /// Agent Tools could not start or are not installed.
    ToolsUnavailable { message: String },
    TreeNotFound { tree_id: String },
    NotInMod { path: String },
    /// A mod file changed since the projection: nothing was written.
    StaleBase { current: BTreeMap<String, String> },
    /// The operations are outside the writable subset or invalid: nothing was written.
    OperationRefused { error: OpError },
    /// The tool refused the rewrite: nothing was written.
    ToolRefused { tool_code: String, message: String, blockers: Value },
    /// The rewrite may have run. The worktree must be observed before anything else happens.
    UnknownEffect { message: String },
    /// The tool wrote outside the tree's source file. The extra paths are listed.
    UnexpectedWrite { paths: Vec<String> },
    Internal { message: String },
}

impl FocusError {
    pub fn code(&self) -> &'static str {
        match self {
            FocusError::ToolsUnavailable { .. } => "tools_unavailable",
            FocusError::TreeNotFound { .. } => "tree_not_found",
            FocusError::NotInMod { .. } => "not_in_mod",
            FocusError::StaleBase { .. } => "stale_base",
            FocusError::OperationRefused { .. } => "operation_refused",
            FocusError::ToolRefused { .. } => "tool_refused",
            FocusError::UnknownEffect { .. } => "unknown_effect",
            FocusError::UnexpectedWrite { .. } => "unexpected_write",
            FocusError::Internal { .. } => "internal",
        }
    }
}

/// What one guarded rewrite did.
#[derive(Debug, Clone, Serialize)]
pub struct Rewritten {
    pub request_sha256: String,
    pub before: BTreeMap<String, String>,
    pub after: BTreeMap<String, String>,
    pub changed_paths: Vec<String>,
    pub change: PlanChange,
    /// One unified diff per changed path (`path`, `diff`), for the review card.
    pub diffs: Vec<(String, String)>,
}

/// A unified diff of two file versions, with lossy text for non-UTF-8 bytes.
pub fn unified_diff(path: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> String {
    let old = before.map(String::from_utf8_lossy).unwrap_or_default();
    let new = after.map(String::from_utf8_lossy).unwrap_or_default();
    similar::TextDiff::from_lines(old.as_ref(), new.as_ref())
        .unified_diff()
        .context_radius(3)
        .header(
            &if before.is_some() { format!("a/{path}") } else { "/dev/null".to_owned() },
            &if after.is_some() { format!("b/{path}") } else { "/dev/null".to_owned() },
        )
        .to_string()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_file(root: &Path, rel: &str) -> String {
    match std::fs::read(root.join(rel)) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(_) => ABSENT.to_owned(),
    }
}

/// Paths `git status` reports as changed or untracked in the worktree (renames as both sides).
fn dirty_paths(root: &Path) -> Result<BTreeSet<String>, FocusError> {
    let out = std::process::Command::new("git")
        .args(["-c", "core.autocrlf=false", "-c", "core.quotepath=off", "status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(root)
        .output()
        .map_err(|e| FocusError::Internal { message: format!("git status: {e}") })?;
    if !out.status.success() {
        return Err(FocusError::Internal { message: "git status failed in the task worktree".into() });
    }
    let mut paths = BTreeSet::new();
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let (status, path) = entry.split_at(3);
        paths.insert(path.to_owned());
        if status.starts_with('R') || status.starts_with('C') {
            if let Some(from) = parts.next() {
                paths.insert(from.to_owned());
            }
        }
    }
    Ok(paths)
}

pub fn sidecar_path(relative_path: &str) -> String {
    match relative_path.strip_suffix(".txt") {
        Some(stem) => format!("{stem}.focus-plan.json"),
        None => format!("{relative_path}.focus-plan.json"),
    }
}

pub fn localisation_path(tree_id: &str) -> String {
    format!("localisation/english/{tree_id}_l_english.yml")
}

fn localisation_text_ok(text: &str) -> bool {
    !text.contains('"') && !text.contains('\n') && !text.contains('\r')
}

/// Adds `key:0 "text"` lines to the localisation file, creating it with a BOM when missing.
/// Refuses a key that already exists in the file.
fn append_localisation(root: &Path, rel: &str, lines: &[(String, String)]) -> Result<(), FocusError> {
    let path = root.join(rel);
    let existing = std::fs::read(&path).ok();
    let mut text = match &existing {
        Some(bytes) => String::from_utf8(bytes.clone())
            .map_err(|_| FocusError::Internal { message: format!("{rel} is not UTF-8") })?,
        None => "\u{feff}l_english:\n".to_owned(),
    };
    for (key, _) in lines {
        let needle = format!("\n {key}:");
        if text.contains(&needle) || text.contains(&format!("\n{key}:")) {
            return Err(FocusError::OperationRefused {
                error: OpError {
                    code: "focus_exists",
                    focus_id: key.clone(),
                    message: format!("the localisation key {key} already exists in {rel}"),
                },
            });
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    for (key, value) in lines {
        text.push_str(&format!(" {key}:0 \"{value}\"\n"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| FocusError::Internal { message: e.to_string() })?;
    }
    std::fs::write(&path, text).map_err(|e| FocusError::Internal { message: e.to_string() })
}

pub struct FocusSession {
    session: ToolSession,
    root: PathBuf,
    tool_version: String,
}

impl FocusSession {
    /// Starts Agent Tools on the worktree with the focus editor profile.
    pub async fn start(settings: &AgentToolsSettings, root: &Path, state_dir: &Path) -> Result<Self, FocusError> {
        let state = {
            let dir = state_dir.to_path_buf();
            tokio::task::spawn_blocking(move || prepare_state_dir(&dir))
                .await
                .map_err(|e| FocusError::Internal { message: e.to_string() })?
                .map_err(|e| FocusError::Internal { message: format!("tool state: {e}") })?
        };
        let spec = agent_tools_spec(&settings.install, root, &state, settings.game_root.as_deref());
        let session = ToolSession::start(spec, agent_tools_focus_editor()).await;
        if !session.inventory().is_healthy() {
            let message = serde_json::to_string(&session.inventory().health).unwrap_or_default();
            let _ = session.shutdown().await;
            return Err(FocusError::ToolsUnavailable { message });
        }
        let tool_version = session
            .inventory()
            .server
            .as_ref()
            .map(|h| h.server_version.clone())
            .unwrap_or_default();
        Ok(Self { session, root: root.to_path_buf(), tool_version })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tool_version(&self) -> &str {
        &self.tool_version
    }

    /// Inspects one tree and builds its projection.
    pub async fn inspect(&mut self, tree_id: &str) -> Result<Built, FocusError> {
        let deadline = Instant::now() + INSPECT_TIMEOUT;
        let result = self
            .session
            .call("agent_tools_focus_inspect", &json!({ "treeId": tree_id }), deadline)
            .await
            .map_err(tool_error)?;
        let structured = result.output.structured.clone().unwrap_or(Value::Null);
        if result.output.is_error {
            return Err(FocusError::ToolRefused {
                tool_code: structured["code"].as_str().unwrap_or("").to_owned(),
                message: structured["blockers"][0]["message"].as_str().unwrap_or("focus_inspect failed").to_owned(),
                blockers: structured["blockers"].clone(),
            });
        }
        let uri = structured["artifacts"][0]["uri"]
            .as_str()
            .ok_or_else(|| FocusError::Internal { message: "focus_inspect linked no artifact".into() })?
            .to_owned();
        let artifact = self.session.read_artifact(&uri, deadline).await.map_err(tool_error)?;
        let doc: Value = serde_json::from_str(&artifact.text)
            .map_err(|e| FocusError::Internal { message: format!("artifact JSON: {e}") })?;
        build(tree_id, &structured, &doc, &self.tool_version).map_err(|e| match e {
            BuildError::TreeNotFound => FocusError::TreeNotFound { tree_id: tree_id.to_owned() },
            BuildError::NotInMod(path) => FocusError::NotInMod { path },
            BuildError::Malformed(m) => FocusError::Internal { message: m.into() },
        })
    }

    /// The current hash of every file the projection read, or `None` when none changed.
    pub fn stale_files(&self, base: &Built) -> Option<BTreeMap<String, String>> {
        let current: BTreeMap<String, String> = base
            .source_hashes
            .keys()
            .map(|p| (p.clone(), hash_file(&self.root, p)))
            .collect();
        (current != base.source_hashes).then_some(current)
    }

    /// The rewrite request the operations produce on `base`, without running it. The gesture
    /// path, the agent path and the coding-client parity test all send exactly this.
    pub fn request(base: &Built, ops: &[FocusOp]) -> Result<(Value, PlanChange), FocusError> {
        let (plan, change) = apply_ops(&base.plan, ops).map_err(|error| FocusError::OperationRefused { error })?;
        for (_, text) in &change.localisation {
            if !localisation_text_ok(text) {
                return Err(FocusError::OperationRefused {
                    error: OpError {
                        code: "invalid_value",
                        focus_id: String::new(),
                        message: "titles and descriptions are one line without double quotes".into(),
                    },
                });
            }
        }
        let request = json!({
            "relativePath": base.relative_path,
            "treeId": base.tree_id,
            "plan": plan,
        });
        Ok((request, change))
    }

    /// Applies the operations with the host's guards. See the module documentation.
    pub async fn rewrite(&mut self, base: &Built, ops: &[FocusOp]) -> Result<Rewritten, FocusError> {
        if let Some(current) = self.stale_files(base) {
            return Err(FocusError::StaleBase { current });
        }
        let (request, change) = Self::request(base, ops)?;
        let request_sha256 = factory_hoi4_tools::mcp::canonical_sha256(&request);
        let sidecar = sidecar_path(&base.relative_path);
        let loc = localisation_path(&base.tree_id);
        let mut touched = vec![base.relative_path.clone(), sidecar.clone()];
        if !change.localisation.is_empty() {
            touched.push(loc.clone());
        }
        let before: BTreeMap<String, String> = touched.iter().map(|p| (p.clone(), hash_file(&self.root, p))).collect();
        let before_bytes: BTreeMap<String, Option<Vec<u8>>> =
            touched.iter().map(|p| (p.clone(), std::fs::read(self.root.join(p)).ok())).collect();
        let dirty_before = dirty_paths(&self.root)?;

        let deadline = Instant::now() + REWRITE_TIMEOUT;
        let result = match self.session.call(FOCUS_REWRITE, &request, deadline).await {
            Ok(r) => r,
            Err(e) if e.outcome_unknown() => return Err(FocusError::UnknownEffect { message: e.to_string() }),
            Err(e) => return Err(tool_error(e)),
        };
        let structured = result.output.structured.clone().unwrap_or(Value::Null);
        if result.output.is_error {
            let source_now = hash_file(&self.root, &base.relative_path);
            if source_now != before[&base.relative_path] {
                return Err(FocusError::UnknownEffect {
                    message: "the tool refused the rewrite but the source file changed".into(),
                });
            }
            if before[&sidecar] == ABSENT {
                let _ = std::fs::remove_file(self.root.join(&sidecar));
            }
            return Err(FocusError::ToolRefused {
                tool_code: structured["code"].as_str().unwrap_or("").to_owned(),
                message: structured["blockers"][0]["message"].as_str().unwrap_or("focus_rewrite refused").to_owned(),
                blockers: structured["blockers"].clone(),
            });
        }
        if before[&sidecar] == ABSENT {
            let _ = std::fs::remove_file(self.root.join(&sidecar));
        }
        let dirty_after = dirty_paths(&self.root)?;
        let unexpected: Vec<String> = dirty_after
            .difference(&dirty_before)
            .filter(|p| !touched.contains(p))
            .cloned()
            .collect();
        if !unexpected.is_empty() {
            return Err(FocusError::UnexpectedWrite { paths: unexpected });
        }
        if !change.localisation.is_empty() {
            append_localisation(&self.root, &loc, &change.localisation)?;
        }
        let after: BTreeMap<String, String> = touched.iter().map(|p| (p.clone(), hash_file(&self.root, p))).collect();
        let changed_paths: Vec<String> = touched.iter().filter(|p| before[*p] != after[*p]).cloned().collect();
        let diffs = changed_paths
            .iter()
            .map(|p| {
                let now = std::fs::read(self.root.join(p)).ok();
                (p.clone(), unified_diff(p, before_bytes[p].as_deref(), now.as_deref()))
            })
            .collect();
        Ok(Rewritten { request_sha256, before, after, changed_paths, change, diffs })
    }

    pub async fn shutdown(self) {
        let _ = self.session.shutdown().await;
    }
}

fn tool_error(e: ToolError) -> FocusError {
    match e {
        ToolError::NotAvailable { name, reason } => FocusError::ToolsUnavailable { message: format!("{name}: {reason}") },
        other => FocusError::Internal { message: other.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localisation_is_appended_with_a_bom_and_never_duplicated() {
        let dir = tempfile::tempdir().unwrap();
        let rel = localisation_path("t");
        append_localisation(dir.path(), &rel, &[("a".into(), "Alpha".into()), ("a_desc".into(), "First.".into())]).unwrap();
        let text = std::fs::read(dir.path().join(&rel)).unwrap();
        assert_eq!(text, "\u{feff}l_english:\n a:0 \"Alpha\"\n a_desc:0 \"First.\"\n".as_bytes());
        let e = append_localisation(dir.path(), &rel, &[("a".into(), "Again".into())]).unwrap_err();
        assert_eq!(e.code(), "operation_refused");
        assert_eq!(std::fs::read(dir.path().join(&rel)).unwrap(), text);
    }

    #[test]
    fn sidecar_and_localisation_paths() {
        assert_eq!(sidecar_path("common/national_focus/x.txt"), "common/national_focus/x.focus-plan.json");
        assert_eq!(localisation_path("my_tree"), "localisation/english/my_tree_l_english.yml");
    }
}
