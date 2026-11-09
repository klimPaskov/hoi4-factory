//! The engine check that runs before every agent commit (S04, gate 8, contract "Commit check").
//!
//! The server links `hoi4-engine` from the same hoi4-agent-tools commit as the image's MCP tools
//! and runs its reference closure, lints, override audit and findings baseline on a proposal
//! worktree. The verdict is the engine's own:
//!
//! - `fail`: a finding that is not in the project baseline reaches the failure severity.
//! - `incomplete`: nothing new fails, but something could not be checked (unknown references,
//!   script folders without a context model, no game documentation). It is never reported as a
//!   pass, and its unknown counts travel with it.
//! - `pass`: everything was checked and nothing new fails.
//!
//! An agent commit is blocked by `fail` and returns the new findings as diagnostics. A person's
//! commit shows the same report and is not blocked by it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use hoi4_engine::analysis::Analysis;
pub use hoi4_engine::baseline::Baseline;
use hoi4_engine::delta;
pub use hoi4_engine::findings::{Finding, Severity};
use hoi4_engine::report;
use hoi4_engine::vocab::Vocabulary;
use hoi4_engine::workspace::{Workspace, WorkspaceOptions};
use serde::{Deserialize, Serialize};

/// The linked engine's crate version.
pub const ENGINE_VERSION: &str = hoi4_engine::VERSION;
/// The hoi4-agent-tools commit the engine is linked from. It must equal the `rev` of the
/// `hoi4-engine` dependency in the root `Cargo.toml` and the image's Agent Tools release commit.
pub const ENGINE_REV: &str = "73a5d7199ab7d1e4e3aeca74598d946512e23f3d";
pub const REPORT_SCHEMA: &str = report::SCHEMA;
pub const BASELINE_SCHEMA: &str = hoi4_engine::baseline::SCHEMA;

#[derive(Debug, Clone)]
pub struct CheckRequest {
    /// The checked mod: a proposal worktree or the workspace itself.
    pub mod_root: PathBuf,
    /// The read-only game reference. Without it every command and vanilla name is unknown, so the
    /// verdict is at best `incomplete`.
    pub game_root: Option<PathBuf>,
    pub include_dlc: bool,
    pub dependencies: Vec<PathBuf>,
    /// The project's findings baseline (`hoi4-engine-baseline/1` JSON). Findings in it never block.
    pub baseline: Option<Baseline>,
    /// The project threshold. Only new findings at or above it fail the check.
    pub fail_on: Severity,
}

impl CheckRequest {
    pub fn new(mod_root: impl Into<PathBuf>, game_root: Option<PathBuf>) -> Self {
        Self {
            mod_root: mod_root.into(),
            game_root,
            include_dlc: true,
            dependencies: Vec::new(),
            baseline: None,
            fail_on: Severity::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Incomplete,
    Fail,
}

impl Verdict {
    fn from_engine(v: &str) -> Self {
        match v {
            "pass" => Verdict::Pass,
            "fail" => Verdict::Fail,
            // Anything else, including a verdict this build does not know, is not a pass.
            _ => Verdict::Incomplete,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Incomplete => "incomplete",
            Verdict::Fail => "fail",
        }
    }
}

/// What the check could not decide, summed over every reference kind and command position.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownCounts {
    pub references_resolved: usize,
    pub references_unresolved: usize,
    pub references_unknown: usize,
    pub commands_resolved: usize,
    pub commands_unresolved: usize,
    pub commands_unknown: usize,
    /// Script files whose folder has no context model, so their references were not checked.
    pub unmodelled_script_files: usize,
    pub vocabulary_available: bool,
    pub game_layer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    pub verdict: Verdict,
    pub engine_version: String,
    pub engine_rev: String,
    pub report_schema: String,
    pub game_version: Option<String>,
    /// Findings that are not in the baseline and reach the threshold: the ones that fail the check.
    pub blocking: Vec<Finding>,
    /// Every finding, baselined or not.
    pub findings: Vec<Finding>,
    pub new_count: usize,
    pub baselined_count: usize,
    pub unknown: UnknownCounts,
    /// The engine's full `hoi4-engine-report/1` document.
    pub report: serde_json::Value,
    pub elapsed_ms: u64,
    /// One row per file of the checked mod: parsed, kept byte-exact or not understood (the
    /// import preservation report of C-05).
    pub preservation: Vec<PreservationRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preservation {
    /// A script or localisation file the engine parsed without a syntax problem.
    Parsed,
    /// A file the engine does not parse (art, sound, interface data): stored and served with its
    /// exact bytes.
    KeptByteExact,
    /// A file the engine read and could not fully understand (a syntax problem or unreadable
    /// text). It is still stored byte-exact, and edits to it need a person.
    NotUnderstood,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservationRow {
    pub path: String,
    pub status: Preservation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug)]
pub enum CheckError {
    ModRootMissing(PathBuf),
    Serialize(String),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::ModRootMissing(p) => {
                write!(f, "the checked mod folder does not exist: {}", p.display())
            }
            CheckError::Serialize(e) => write!(f, "the engine report could not be serialized: {e}"),
        }
    }
}

impl std::error::Error for CheckError {}

/// Reading the game's documentation takes seconds, so the vocabulary is read once per game folder.
fn vocabulary(game: Option<&Path>) -> Arc<Vocabulary> {
    static CACHE: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Vocabulary>>>> = OnceLock::new();
    let Some(game) = game else {
        return Arc::new(Vocabulary::default());
    };
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(v) = cache.lock().expect("vocabulary cache").get(game) {
        return v.clone();
    }
    let v = Arc::new(Vocabulary::from_game(game));
    cache
        .lock()
        .expect("vocabulary cache")
        .insert(game.to_path_buf(), v.clone());
    v
}

/// Runs the engine check. Blocking work: call it from `spawn_blocking` on an async runtime.
pub fn run_check(req: &CheckRequest) -> Result<CheckReport, CheckError> {
    if !req.mod_root.is_dir() {
        return Err(CheckError::ModRootMissing(req.mod_root.clone()));
    }
    let started = Instant::now();
    let game = req.game_root.as_deref().filter(|p| p.is_dir());
    let options = WorkspaceOptions {
        game: game.map(Path::to_path_buf),
        include_dlc: req.include_dlc,
        dependencies: req.dependencies.clone(),
        target: Some(req.mod_root.clone()),
    };
    let vocab = (*vocabulary(game)).clone();
    let analysis = Analysis::run(Workspace::load(&options), vocab);
    let built = report::build(&analysis, req.baseline.as_ref(), req.fail_on, &|_| None);
    let verdict = Verdict::from_engine(built.verdict);
    let value = serde_json::to_value(&built).map_err(|e| CheckError::Serialize(e.to_string()))?;

    let baselined = |f: &Finding| req.baseline.as_ref().is_some_and(|b| b.contains(f));
    let blocking: Vec<Finding> = analysis
        .findings
        .iter()
        .filter(|f| !baselined(f) && f.severity >= req.fail_on)
        .cloned()
        .collect();
    let baselined_count = analysis.findings.iter().filter(|f| baselined(f)).count();

    let mut preservation: Vec<PreservationRow> = Vec::new();
    if let Some(target) = analysis.target_layer() {
        let parsed: BTreeMap<usize, &hoi4_engine::analysis::Parsed> = analysis.parsed.iter().map(|p| (p.file, p)).collect();
        for (i, file) in analysis.workspace.files.iter().enumerate() {
            if file.layer != target {
                continue;
            }
            let (status, detail) = match parsed.get(&i) {
                Some(p) if p.unreadable => (Preservation::NotUnderstood, Some("unreadable text".to_owned())),
                Some(p) if !p.syntax.is_empty() => (Preservation::NotUnderstood, Some(p.syntax[0].0.to_owned())),
                Some(_) => (Preservation::Parsed, None),
                None => (Preservation::KeptByteExact, None),
            };
            preservation.push(PreservationRow { path: file.relative.clone(), status, detail });
        }
        preservation.sort_by(|a, b| a.path.cmp(&b.path));
    }

    let c = &analysis.coverage;
    let mut unknown = UnknownCounts {
        commands_resolved: c.effect_commands.resolved + c.trigger_commands.resolved,
        commands_unresolved: c.effect_commands.unresolved + c.trigger_commands.unresolved,
        commands_unknown: c.effect_commands.unknown + c.trigger_commands.unknown,
        unmodelled_script_files: c.unmodelled_script_folders.values().sum(),
        vocabulary_available: c.vocabulary_available,
        game_layer: c.game_layer,
        ..UnknownCounts::default()
    };
    for t in c.references.values() {
        unknown.references_resolved += t.resolved;
        unknown.references_unresolved += t.unresolved;
        unknown.references_unknown += t.unknown;
    }

    Ok(CheckReport {
        verdict,
        engine_version: ENGINE_VERSION.to_owned(),
        engine_rev: ENGINE_REV.to_owned(),
        report_schema: REPORT_SCHEMA.to_owned(),
        game_version: analysis.vocab.game_version.clone(),
        new_count: analysis.findings.len() - baselined_count,
        baselined_count,
        findings: analysis.findings.clone(),
        blocking,
        unknown,
        report: value,
        elapsed_ms: started.elapsed().as_millis() as u64,
        preservation,
    })
}

/// A baseline that accepts every current finding (`hoi4-engine-baseline/1`).
pub fn baseline_from(findings: &[Finding]) -> Baseline {
    Baseline::from_findings(findings)
}

pub fn parse_baseline(text: &str) -> Result<Baseline, String> {
    Baseline::parse(text)
}

/// The finding delta between two checks: introduced, worsened, changed, improved, unchanged and
/// resolved findings, for the review card.
#[derive(Debug, Clone, Serialize)]
pub struct FindingDelta {
    pub summary: serde_json::Value,
    pub entries: serde_json::Value,
}

pub fn finding_delta(before: &[Finding], after: &[Finding]) -> FindingDelta {
    let entries = delta::compare(before, after);
    let summary = delta::summarize(&entries);
    FindingDelta {
        summary: serde_json::to_value(summary).unwrap_or(serde_json::Value::Null),
        entries: serde_json::to_value(&entries).unwrap_or(serde_json::Value::Null),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitActor {
    Person,
    Agent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitDecision {
    /// The commit may go ahead. The verdict is recorded as is: `incomplete` stays `incomplete`.
    Allowed { verdict: Verdict },
    /// An agent commit with new findings at the threshold. They go back to the agent.
    Blocked { findings: Vec<String> },
}

/// The commit rule: new findings at the threshold block an agent commit. A person sees the same
/// report and decides.
pub fn commit_gate(report: &CheckReport, actor: CommitActor) -> CommitDecision {
    match (actor, report.verdict) {
        (CommitActor::Agent, Verdict::Fail) => CommitDecision::Blocked {
            findings: report
                .blocking
                .iter()
                .map(|f| {
                    format!(
                        "{}:{}:{}: {} {}",
                        f.path, f.line, f.column, f.code, f.message
                    )
                })
                .collect(),
        },
        (_, verdict) => CommitDecision::Allowed { verdict },
    }
}
