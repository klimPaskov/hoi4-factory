//! Host tool broker (probe). Validates final arguments after every hook or
//! repair step against a closed JSON Schema, then applies the workspace root
//! policy. Only a `Prepared` call may reach the effect journal.

use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

use crate::util::{canonical, sha256_hex};

pub const ROOT_MARKER: &str = ".factory-root-id";

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub identity: String,
}

impl Workspace {
    /// Open or create a workspace. Identity binds the canonical root path and
    /// a per-workspace marker, so a moved or substituted root changes it.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(path)?;
        let marker = path.join(ROOT_MARKER);
        if !marker.exists() {
            std::fs::write(&marker, crate::util::new_id("root"))?;
        }
        let root = std::fs::canonicalize(path)?;
        let id = std::fs::read_to_string(&marker)?;
        let identity = sha256_hex(format!("{}\n{}", root.display(), id.trim()).as_bytes());
        Ok(Self { root, identity })
    }
}

pub struct ToolDef {
    pub name: &'static str,
    pub version: &'static str,
    pub validator: jsonschema::Validator,
}

pub type Hook = Box<dyn Fn(&mut String, &mut Value) + Send + Sync>;

pub struct Broker {
    pub ws: Workspace,
    pub tools: Vec<ToolDef>,
    pub hooks: Vec<(String, Hook)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Prepared {
    pub tool: String,
    pub tool_version: String,
    pub schema_version: u32,
    pub args: Value,
    pub args_hash: String,
    pub rel_path: String,
    pub root_identity: String,
    pub replay_identity: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    UnknownTool { name: String },
    SchemaInvalid { errors: Vec<String> },
    PathPolicy { path: String, reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct PrepareReport {
    pub initial_validation: Result<(), Vec<String>>,
    pub hooks: Vec<Value>,
    pub final_args: Value,
    pub result: Result<Prepared, Refusal>,
}

pub fn replay_identity(tool: &str, version: &str, schema_version: u32, root_identity: &str, args_hash: &str) -> String {
    sha256_hex(format!("{tool}\n{version}\n{schema_version}\n{root_identity}\n{args_hash}").as_bytes())
}

impl Broker {
    pub fn new(ws: Workspace) -> Self {
        let schema = crate::scenario::host_schema();
        let validator = jsonschema::validator_for(&schema).expect("host schema compiles");
        Self {
            ws,
            tools: vec![ToolDef { name: "write_file", version: "write_file@1", validator }],
            hooks: vec![],
        }
    }

    fn validate(&self, name: &str, args: &Value) -> Result<&ToolDef, Refusal> {
        let t = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| Refusal::UnknownTool { name: name.into() })?;
        let errors: Vec<String> = t.validator.iter_errors(args).map(|e| format!("{} at {}", e, e.instance_path())).collect();
        if errors.is_empty() { Ok(t) } else { Err(Refusal::SchemaInvalid { errors }) }
    }

    /// Adapter-level parse already happened in the agent crate. This runs the
    /// initial check (for the report only), every hook, then the authoritative
    /// final validation and root policy.
    pub fn prepare(&self, name: &str, model_args: &Value) -> PrepareReport {
        let initial_validation = self.validate(name, model_args).map(|_| ()).map_err(|r| match r {
            Refusal::SchemaInvalid { errors } => errors,
            other => vec![format!("{other:?}")],
        });
        let mut name = name.to_owned();
        let mut args = model_args.clone();
        let mut hooks = vec![];
        for (hname, h) in &self.hooks {
            let before = sha256_hex(canonical(&args).as_bytes());
            let before_name = name.clone();
            h(&mut name, &mut args);
            hooks.push(json!({
                "hook": hname,
                "args_hash_before": before,
                "args_hash_after": sha256_hex(canonical(&args).as_bytes()),
                "tool_before": before_name,
                "tool_after": name,
            }));
        }
        let result = (|| {
            let t = self.validate(&name, &args)?;
            let path = args["path"].as_str().unwrap_or_default().to_owned();
            resolve_in_root(&self.ws.root, &path)?;
            let args_hash = sha256_hex(canonical(&args).as_bytes());
            Ok(Prepared {
                tool: t.name.into(),
                tool_version: t.version.into(),
                schema_version: 1,
                replay_identity: replay_identity(t.name, t.version, 1, &self.ws.identity, &args_hash),
                args: args.clone(),
                args_hash,
                rel_path: path,
                root_identity: self.ws.identity.clone(),
            })
        })();
        PrepareReport { initial_validation, hooks, final_args: args, result }
    }
}

/// Workspace root policy. Relative forward-slash paths only, no parent or
/// root or drive components, no marker writes, and the nearest existing
/// ancestor must canonicalize inside the root (symlink escape check).
pub fn resolve_in_root(root: &Path, rel: &str) -> Result<PathBuf, Refusal> {
    let deny = |reason: &str| Refusal::PathPolicy { path: rel.into(), reason: reason.into() };
    if rel.is_empty() {
        return Err(deny("empty path"));
    }
    if rel.contains('\0') || rel.contains('\\') || rel.contains(':') {
        return Err(deny("NUL, backslash or drive separator"));
    }
    if rel.starts_with('/') {
        return Err(deny("absolute path"));
    }
    let p = Path::new(rel);
    for c in p.components() {
        match c {
            Component::Normal(_) => {}
            Component::CurDir => return Err(deny("'.' component")),
            Component::ParentDir => return Err(deny("parent traversal '..'")),
            Component::RootDir | Component::Prefix(_) => return Err(deny("root or prefix component")),
        }
    }
    if p.components().next().map(|c| c.as_os_str() == ROOT_MARKER).unwrap_or(false) {
        return Err(deny("workspace marker is host-owned"));
    }
    let full = root.join(p);
    let mut anc = full.parent();
    while let Some(a) = anc {
        if a.exists() {
            let canon = std::fs::canonicalize(a).map_err(|e| deny(&format!("canonicalize: {e}")))?;
            if !canon.starts_with(root) {
                return Err(deny("existing ancestor resolves outside root (symlink escape)"));
            }
            break;
        }
        anc = a.parent();
    }
    if full.exists() {
        let meta = std::fs::symlink_metadata(&full).map_err(|e| deny(&e.to_string()))?;
        if meta.file_type().is_symlink() {
            return Err(deny("target is a symlink"));
        }
    }
    Ok(full)
}

/// Staged write then rename. Returns (before hash or "absent", after hash).
pub fn file_hash(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(b) => sha256_hex(&b),
        Err(_) => "absent".into(),
    }
}

pub fn stage(root: &Path, p: &Prepared, op_id: &str) -> std::io::Result<PathBuf> {
    let full = resolve_in_root(root, &p.rel_path).map_err(|r| std::io::Error::other(format!("{r:?}")))?;
    if let Some(d) = full.parent() {
        std::fs::create_dir_all(d)?;
    }
    let staging = full.with_extension(format!("factory-staging-{op_id}"));
    std::fs::write(&staging, p.args["content"].as_str().unwrap_or_default())?;
    Ok(staging)
}

pub fn commit_stage(root: &Path, p: &Prepared, staging: &Path) -> std::io::Result<String> {
    let full = resolve_in_root(root, &p.rel_path).map_err(|r| std::io::Error::other(format!("{r:?}")))?;
    std::fs::rename(staging, &full)?;
    Ok(file_hash(&full))
}

pub fn intended_hash(p: &Prepared) -> String {
    sha256_hex(p.args["content"].as_str().unwrap_or_default().as_bytes())
}
