use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn sha256_hex(b: &[u8]) -> String {
    let d = Sha256::digest(b);
    d.iter().map(|x| format!("{x:02x}")).collect()
}

/// Canonical JSON (serde_json maps are sorted by key without preserve_order).
pub fn canonical(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

static CTR: AtomicU64 = AtomicU64::new(0);

pub fn new_id(prefix: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let c = CTR.fetch_add(1, Ordering::Relaxed);
    let h = sha256_hex(format!("{n}-{c}-{}", std::process::id()).as_bytes());
    format!("{prefix}_{}", &h[..16])
}

pub fn write_json(path: &Path, v: &Value) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    std::fs::write(path, s).unwrap();
    eprintln!("wrote {}", path.display());
}

pub fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

pub fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn recordings_dir() -> PathBuf {
    repo_root().join("probes/runtime/recordings")
}

pub fn evidence_dir(exp: &str) -> PathBuf {
    repo_root().join("evidence/S01").join(exp)
}

/// A recorded real provider stream: raw bytes, chunk boundaries and metadata.
pub struct Recording {
    pub name: String,
    pub bytes: Vec<u8>,
    pub chunks: Vec<usize>,
    pub meta: Value,
}

impl Recording {
    pub fn load(name: &str) -> Recording {
        let d = recordings_dir();
        let bytes = std::fs::read(d.join(format!("{name}.sse"))).expect("recording bytes");
        let meta: Value =
            serde_json::from_slice(&std::fs::read(d.join(format!("{name}.meta.json"))).unwrap()).unwrap();
        let chunks = meta["chunk_sizes"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_u64().unwrap() as usize).collect())
            .unwrap_or_default();
        let actual = sha256_hex(&bytes);
        assert_eq!(
            meta["sse_sha256"].as_str(),
            Some(actual.as_str()),
            "recording {name} hash mismatch"
        );
        Recording { name: name.into(), bytes, chunks, meta }
    }

    pub fn source(&self, fault: &factory_agent::transport::Fault) -> factory_agent::transport::ByteSource {
        factory_agent::transport::ByteSource::Replay(factory_agent::transport::Replay::new(
            bytes::Bytes::from(self.bytes.clone()),
            Some(self.chunks.clone()),
            fault,
        ))
    }

    /// Byte offsets (end of event) for every SSE event, with its data text.
    pub fn events(&self) -> Vec<(usize, usize, String)> {
        let mut p = factory_agent::sse::SseParser::new();
        let mut start = 0;
        let mut out = vec![];
        for ev in p.push(&self.bytes).unwrap() {
            out.push((start, ev.end_offset, ev.data.clone()));
            start = ev.end_offset;
        }
        out
    }

    pub fn reference(&self) -> Value {
        json!({
            "recording": format!("probes/runtime/recordings/{}.sse", self.name),
            "sse_sha256": self.meta["sse_sha256"],
            "provider": self.meta["provider"],
            "model": self.meta["model_reported"],
            "captured": self.meta["captured_utc"],
            "evidence_kind_of_capture": "live",
        })
    }
}

pub fn now_utc() -> String {
    // RFC 3339 without a date crate: ask the OS clock in seconds.
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let days = s / 86400;
    let rem = s % 86400;
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Content hash of every file under `dir` (relative path, length, sha256).
pub fn tree_snapshot(dir: &Path) -> String {
    let mut entries = vec![];
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                out.push(format!("d {rel}"));
                walk(base, &p, out);
            } else {
                let b = std::fs::read(&p).unwrap_or_default();
                out.push(format!("f {rel} {} {}", b.len(), sha256_hex(&b)));
            }
        }
    }
    walk(dir, dir, &mut entries);
    entries.sort();
    sha256_hex(entries.join("\n").as_bytes())
}

pub fn probe_base() -> PathBuf {
    std::env::temp_dir().join("hoi4-factory-s01-runtime")
}

/// The tool request fed to host-side probes. A recorded real model tool call
/// is used when the recording exists. Otherwise the request is host-authored
/// and labelled as such. It is never presented as model output.
pub struct ToolRequest {
    pub name: String,
    pub args: serde_json::Value,
    pub input_source: &'static str,
    pub evidence_kind: &'static str,
    pub reference: serde_json::Value,
}

pub const HOST_AUTHORED: &str = "host-authored operation request (not model output). No recorded real model tool call exists";

pub async fn tool_request() -> ToolRequest {
    if let Some((tool, _, _)) = primary_set() {
        let r = Recording::load(tool);
        let t = crate::provider::assemble(crate::provider::of(&r), r.source(&factory_agent::transport::Fault::None), |_| factory_agent::openai::Control::Continue)
            .await
            .expect("recorded tool turn assembles");
        let c = &t.tool_calls[0];
        return ToolRequest { name: c.name.clone(), args: c.arguments.clone(), input_source: "recorded real model tool call", evidence_kind: "recorded-real-replay", reference: r.reference() };
    }
    ToolRequest {
        name: "write_file".into(),
        args: json!({"path": "notes/probe.txt", "content": "S01 runtime probe ok"}),
        input_source: HOST_AUTHORED,
        evidence_kind: "local (host-authored input, no model output)",
        reference: json!(null),
    }
}

/// The recorded real stream set used by replay probes: Anthropic first (the
/// only provider with credit on 2026-10-24), then OpenAI.
pub fn primary_set() -> Option<(&'static str, &'static str, &'static str)> {
    [("anthropic-tool-turn", "anthropic-followup-turn", "anthropic-text-turn"), ("openai-tool-turn", "openai-followup-turn", "openai-text-turn")]
        .into_iter()
        .find(|(a, b, c)| [a, b, c].iter().all(|n| recordings_dir().join(format!("{n}.sse")).exists()))
}

pub fn has_recordings() -> bool {
    primary_set().is_some()
}
