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
    /// The bytes before the change of each changed path, `None` for a file that did not exist.
    pub before_contents: BTreeMap<String, Option<Vec<u8>>>,
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

/// The English localisation file under `localisation/` that defines `key`, if any.
fn find_localisation(root: &Path, key: &str) -> Option<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        if depth > 4 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = format!("{rel}/{name}");
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path(), &child, depth + 1, out),
                Ok(kind) if kind.is_file() && name.ends_with("l_english.yml") => out.push(child),
                _ => {}
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("localisation"), "localisation", 0, &mut files);
    files.sort();
    files.into_iter().find(|rel| {
        std::fs::read(root.join(rel))
            .ok()
            .is_some_and(|bytes| String::from_utf8_lossy(&bytes).lines().any(|line| key_of(line) == Some(key)))
    })
}

/// The key a localisation line defines: `key` of ` key:0 "text"`.
fn key_of(line: &str) -> Option<&str> {
    let line = line.trim_start_matches('\u{feff}').trim_start();
    let (key, _) = line.split_once(':')?;
    (!key.is_empty() && !key.starts_with('#') && !key.contains(char::is_whitespace)).then_some(key)
}

/// The text of a localisation file with the line of `key` set to `value`. The line keeps its
/// indentation, its version number and its line ending. A key the file does not have is added
/// at the end. `existing` is `None` for a file that is created, with a BOM and the language header.
fn with_localisation_line(existing: Option<&str>, key: &str, value: &str) -> String {
    let text = existing.map(str::to_owned).unwrap_or_else(|| "\u{feff}l_english:\n".to_owned());
    let mut out = String::with_capacity(text.len() + value.len() + key.len() + 8);
    let mut replaced = false;
    for line in text.split_inclusive('\n') {
        if !replaced && key_of(line) == Some(key) {
            let body = line.trim_end_matches(['\r', '\n']);
            let ending = &line[body.len()..];
            let head_len = body.find(':').map_or(body.len(), |i| i + 1);
            let version: String = body[head_len..].chars().take_while(char::is_ascii_digit).collect();
            out.push_str(&body[..head_len]);
            out.push_str(&version);
            out.push_str(&format!(" \"{value}\""));
            out.push_str(ending);
            replaced = true;
        } else {
            out.push_str(line);
        }
    }
    if !replaced {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!(" {key}:0 \"{value}\"\n"));
    }
    out
}

fn write_localisation_line(root: &Path, rel: &str, key: &str, value: &str) -> Result<(), FocusError> {
    let path = root.join(rel);
    let existing = match std::fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| FocusError::Internal { message: format!("{rel} is not UTF-8") })?),
        Err(_) => None,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| FocusError::Internal { message: e.to_string() })?;
    }
    std::fs::write(&path, with_localisation_line(existing.as_deref(), key, value)).map_err(|e| FocusError::Internal { message: e.to_string() })
}

/// The localisation file each text edit of `ops` writes: the file that defines the key today, or
/// the tree's own English file for a key no file defines. A caller checks these paths against the
/// task's allowed paths before the edit, and the rewrite writes exactly these.
pub fn text_targets(root: &Path, base: &Built, ops: &[FocusOp]) -> Vec<(String, String)> {
    let Ok((_, change)) = apply_ops(&base.plan, ops) else { return Vec::new() };
    change
        .text
        .iter()
        .map(|(key, _)| (key.clone(), find_localisation(root, key).unwrap_or_else(|| localisation_path(&base.tree_id))))
        .collect()
}

pub struct FocusSession {
    session: ToolSession,
    root: PathBuf,
    tool_version: String,
    /// The tools' state folder, where they keep their artifacts.
    state: PathBuf,
}

/// A game-view render: the PNG and its hover map (each focus's rectangle in image pixels).
#[derive(Debug, Clone)]
pub struct Rendered {
    pub png: Vec<u8>,
    pub map: Value,
}

/// Finds an artifact file by its SHA-256 under the tools' state folder and checks its bytes.
/// Agent Tools stores each artifact in a folder named after its hash, and `resources/read` stops
/// at 1 MiB, which a full render passes.
fn artifact_bytes(state: &Path, name: &str, sha256: &str) -> Option<Vec<u8>> {
    fn walk(dir: &Path, name: &str, sha256: &str, depth: usize) -> Option<Vec<u8>> {
        if depth > 8 {
            return None;
        }
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                if let Some(found) = walk(&path, name, sha256, depth + 1) {
                    return Some(found);
                }
            } else if kind.is_file() && entry.file_name().to_string_lossy() == name {
                let bytes = std::fs::read(&path).ok()?;
                if sha256_hex(&bytes) == sha256 {
                    return Some(bytes);
                }
            }
        }
        None
    }
    walk(state, name, sha256, 0)
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
        Ok(Self { session, root: root.to_path_buf(), tool_version, state })
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

    /// Draws the tree as the game draws it (`hoi4.focus_render`, view game).
    pub async fn render(&mut self, base: &Built) -> Result<Rendered, FocusError> {
        let deadline = Instant::now() + REWRITE_TIMEOUT;
        let request = json!({ "relativePath": base.relative_path, "treeId": base.tree_id, "view": "game" });
        let result = self
            .session
            .call(factory_hoi4_tools::profile::FOCUS_RENDER, &request, deadline)
            .await
            .map_err(tool_error)?;
        let structured = result.output.structured.clone().unwrap_or(Value::Null);
        if result.output.is_error || structured["status"] != "ok" {
            return Err(FocusError::ToolRefused {
                tool_code: structured["code"].as_str().unwrap_or("").to_owned(),
                message: structured["blockers"][0]["message"].as_str().unwrap_or("focus_render failed").to_owned(),
                blockers: structured["blockers"].clone(),
            });
        }
        let artifacts = structured["artifacts"].as_array().cloned().unwrap_or_default();
        let find = |suffix: &str| artifacts.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(suffix))).cloned();
        let (Some(png), Some(map)) = (find(".png"), find(".json")) else {
            return Err(FocusError::Internal { message: "focus_render linked no PNG or hover map".into() });
        };
        let map_text = self
            .session
            .read_artifact(map["uri"].as_str().unwrap_or(""), deadline)
            .await
            .map_err(tool_error)?;
        let map: Value = serde_json::from_str(&map_text.text).map_err(|e| FocusError::Internal { message: format!("hover map: {e}") })?;
        let (state, name, sha) = (
            self.state.clone(),
            png["name"].as_str().unwrap_or("").to_owned(),
            png["sha256"].as_str().unwrap_or("").to_owned(),
        );
        let png = tokio::task::spawn_blocking(move || artifact_bytes(&state, &name, &sha))
            .await
            .map_err(|e| FocusError::Internal { message: e.to_string() })?
            .ok_or_else(|| FocusError::Internal { message: "the render's PNG was not found with its hash".into() })?;
        Ok(Rendered { png, map })
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
        for (_, text) in change.localisation.iter().chain(&change.text) {
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
        // A text edit is not part of the tool's request, so it is hashed beside it.
        let request_sha256 = if change.text.is_empty() {
            factory_hoi4_tools::mcp::canonical_sha256(&request)
        } else {
            factory_hoi4_tools::mcp::canonical_sha256(&json!({ "request": request, "text": change.text }))
        };
        // Operations that only set text leave the plan as it is. The tool is not called for them:
        // it would answer that the rewrite changed nothing.
        let plan_changed = request["plan"] != base.plan;
        let text_files = text_targets(&self.root, base, ops);
        let sidecar = sidecar_path(&base.relative_path);
        let loc = localisation_path(&base.tree_id);
        let mut touched = vec![base.relative_path.clone(), sidecar.clone()];
        if !change.localisation.is_empty() {
            touched.push(loc.clone());
        }
        for (_, file) in &text_files {
            if !touched.contains(file) {
                touched.push(file.clone());
            }
        }
        let before: BTreeMap<String, String> = touched.iter().map(|p| (p.clone(), hash_file(&self.root, p))).collect();
        let before_bytes: BTreeMap<String, Option<Vec<u8>>> =
            touched.iter().map(|p| (p.clone(), std::fs::read(self.root.join(p)).ok())).collect();
        let dirty_before = dirty_paths(&self.root)?;

        if plan_changed {
        let deadline = Instant::now() + REWRITE_TIMEOUT;
        let result = match self.session.call(FOCUS_REWRITE, &request, deadline).await {
            Ok(r) => r,
            Err(e) if e.outcome_unknown() => return Err(FocusError::UnknownEffect { message: e.to_string() }),
            Err(e) => return Err(tool_error(e)),
        };
        let structured = result.output.structured.clone().unwrap_or(Value::Null);
        // The tool can block a rewrite without marking the result an error (its own guards, such
        // as a reference to a missing helper, answer status "blocked"). Only "ok" applied.
        let applied = !result.output.is_error && structured["status"] == "ok";
        let source_now = hash_file(&self.root, &base.relative_path);
        if !applied || source_now == before[&base.relative_path] {
            if source_now != before[&base.relative_path] {
                return Err(FocusError::UnknownEffect {
                    message: "the tool refused the rewrite but the source file changed".into(),
                });
            }
            if before[&sidecar] == ABSENT {
                let _ = std::fs::remove_file(self.root.join(&sidecar));
            }
            // Blocking diagnostics explain a guard; show the errors first.
            let mut reasons: Vec<Value> = structured["blockers"].as_array().cloned().unwrap_or_default();
            reasons.extend(
                structured["diagnostics"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|d| d["severity"] == "error")
                    .take(10)
                    .cloned(),
            );
            let message = reasons
                .first()
                .and_then(|r| r["message"].as_str())
                .unwrap_or(if applied { "the rewrite changed nothing" } else { "focus_rewrite refused" })
                .to_owned();
            return Err(FocusError::ToolRefused {
                tool_code: structured["code"].as_str().unwrap_or(if applied { "NO_SOURCE_CHANGE" } else { "" }).to_owned(),
                message,
                blockers: Value::Array(reasons),
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
        }
        if !change.localisation.is_empty() {
            append_localisation(&self.root, &loc, &change.localisation)?;
        }
        for ((key, text), (_, file)) in change.text.iter().zip(&text_files) {
            write_localisation_line(&self.root, file, key, text)?;
        }
        let after: BTreeMap<String, String> = touched.iter().map(|p| (p.clone(), hash_file(&self.root, p))).collect();
        let changed_paths: Vec<String> = touched.iter().filter(|p| before[*p] != after[*p]).cloned().collect();
        if changed_paths.is_empty() {
            // Only possible for text that is already what was asked: there is nothing to review.
            return Err(FocusError::ToolRefused {
                tool_code: "NO_SOURCE_CHANGE".to_owned(),
                message: "the text is already what was asked".to_owned(),
                blockers: Value::Array(Vec::new()),
            });
        }
        let diffs = changed_paths
            .iter()
            .map(|p| {
                let now = std::fs::read(self.root.join(p)).ok();
                (p.clone(), unified_diff(p, before_bytes[p].as_deref(), now.as_deref()))
            })
            .collect();
        let before_contents = changed_paths.iter().map(|p| (p.clone(), before_bytes[p].clone())).collect();
        Ok(Rewritten { request_sha256, before, after, changed_paths, change, diffs, before_contents })
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
mod text_tests {
    use super::*;

    #[test]
    fn a_localisation_line_is_replaced_in_place_or_added() {
        let file = "\u{feff}l_english:\r\n a:0 \"Alpha\"\r\n  a_desc:1 \"Old text\" # note\r\n b: \"Beta\"\r\n";
        let out = with_localisation_line(Some(file), "a_desc", "New text");
        assert_eq!(out, "\u{feff}l_english:\r\n a:0 \"Alpha\"\r\n  a_desc:1 \"New text\"\r\n b: \"Beta\"\r\n", "indentation, version and line ending stay");
        let out = with_localisation_line(Some(file), "b", "B");
        assert!(out.contains("\n b: \"B\"\r\n"), "a line without a version number keeps none: {out:?}");
        let out = with_localisation_line(Some(" a:0 \"Alpha\""), "c", "Gamma");
        assert_eq!(out, " a:0 \"Alpha\"\n c:0 \"Gamma\"\n", "a missing key is added at the end");
        assert_eq!(with_localisation_line(None, "c", "Gamma"), "\u{feff}l_english:\n c:0 \"Gamma\"\n", "a new file gets the BOM and the header");
        // A key that only starts like another is a different key.
        assert_eq!(key_of(" a_desc:0 \"x\""), Some("a_desc"));
        assert_eq!(key_of("# a:0 \"x\""), None);
        assert_eq!(key_of("\u{feff}l_english:"), Some("l_english"));
    }

    #[test]
    fn a_text_edit_goes_to_the_file_that_defines_its_key() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("localisation/english")).unwrap();
        std::fs::write(root.join("localisation/english/other_l_english.yml"), "\u{feff}l_english:\n a:0 \"Alpha\"\n").unwrap();
        std::fs::write(root.join("localisation/english/notes.txt"), " a_desc:0 \"not a localisation file\"\n").unwrap();
        assert_eq!(find_localisation(root, "a").as_deref(), Some("localisation/english/other_l_english.yml"));
        assert_eq!(find_localisation(root, "a_desc"), None);
        write_localisation_line(root, "localisation/english/other_l_english.yml", "a", "Alpha two").unwrap();
        write_localisation_line(root, &localisation_path("t"), "a_desc", "Text").unwrap();
        assert_eq!(std::fs::read_to_string(root.join("localisation/english/other_l_english.yml")).unwrap(), "\u{feff}l_english:\n a:0 \"Alpha two\"\n");
        assert_eq!(std::fs::read_to_string(root.join("localisation/english/t_l_english.yml")).unwrap(), "\u{feff}l_english:\n a_desc:0 \"Text\"\n");
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
