//! Tool profiles: the only tools a model ever sees (SPEC-006 excludes "enable every discovered
//! MCP command").
//!
//! A profile names a server identity, a mode and a closed list of tools. Each tool carries the
//! model-facing name, a short description of our own, a closed host schema (a narrow subset of the
//! server's real arguments), a timeout, an output bound and a retry policy. Discovery can only
//! confirm or withhold what the profile lists; it never adds tools.
//!
//! # Classification of the nine Agent Tools reads (classified at 4.4.0, commit 731e64f; schemas unchanged at the 4.6.1 pin)
//!
//! I read what each handler does in the pinned source rather than trusting `readOnlyHint`:
//!
//! - `source_lookup`, `mod_index`, `script_validate`, `reference_search`, `reference_read` and
//!   `reference_context` only read the mod, the game folder and bundled docs. They may refresh
//!   the server's own index cache under its XDG state directory, which is deterministic and
//!   idempotent, so they are `PureRead`.
//! - `focus_inspect` is annotated `readOnlyHint: false` upstream because it stores its full report
//!   with `engine.artifacts.putChunked` (an artifact under the state directory). It never writes
//!   mod source: that path is only reached from the `*_rewrite` tools (`transaction-execution`).
//!   It is a `DerivedJob`.
//! - `event_inspect` and `decision_inspect` are annotated `readOnlyHint: true` upstream, but both
//!   also store report artifacts with `putChunked` in the state directory. The annotation is
//!   inconsistent with the code, so they are `DerivedJob` too. Neither writes mod source.
//!
//! The host schemas drop every argument that could widen a call beyond a read: `workspaceId`
//! (the session injects `"current"`), `refresh`, `render`, `proposedSources` overlays,
//! `previous`, cached revision URIs and the compare/impact/helper modes that need them.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The server a profile is written for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerIdentity {
    pub name: String,
    /// An exact version: any other version withholds every tool.
    pub version_req: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMode {
    ReadOnly,
}

/// What a tool does to the world, which decides whether it may be retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    /// No effect beyond reading; safe to repeat.
    PureRead,
    /// Writes only derived artifacts (reports, caches) outside the mod source; never repeated
    /// automatically.
    DerivedJob,
    /// Changes mod source or other durable state. A read-only profile never sends these; the
    /// variant exists so the session can refuse a mis-classified tool locally.
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Total attempts including the first. Only used for `PureRead`.
    pub max_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileTool {
    pub mcp_name: String,
    /// Model-facing name: namespaced, no dots.
    pub exposed_name: String,
    pub description: String,
    pub effect_class: EffectClass,
    /// A closed JSON schema: every object sets `additionalProperties`.
    pub host_schema: Value,
    #[serde(with = "duration_ms")]
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub retry: RetryPolicy,
    /// Whether the real tool takes `workspaceId`, which the session sets to `"current"`.
    pub inject_workspace: bool,
    /// The SHA-256 of the real input schema this host schema was written against. A different
    /// hash withholds the tool.
    pub expected_schema_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProfile {
    pub id: String,
    pub server: ServerIdentity,
    pub mode: ProfileMode,
    pub tools: Vec<ProfileTool>,
}

impl ToolProfile {
    pub fn tool_by_exposed(&self, name: &str) -> Option<&ProfileTool> {
        self.tools.iter().find(|t| t.exposed_name == name)
    }
}

mod duration_ms {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(d)?))
    }
}

/// A path relative to the mod root: no absolute paths, drive letters, backslashes or `..`.
fn rel_path(max: u32) -> Value {
    json!({
        "type": "string", "minLength": 1, "maxLength": max,
        "not": {"pattern": "(^/|^[A-Za-z]:|\\\\|(^|/)\\.\\.(/|$))"}
    })
}

fn int(min: u32, max: u32) -> Value {
    json!({"type": "integer", "minimum": min, "maximum": max})
}

fn text(max: u32) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": max})
}

const SCOPES: [&str; 12] = [
    "country",
    "state",
    "character",
    "combatant",
    "ace",
    "strategic_region",
    "operation",
    "industrial_org",
    "purchase_contract",
    "raid_instance",
    "special_project",
    "faction",
];

fn source_lookup_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "symbol": text(256),
            "path": rel_path(4096),
            "line": {"type": "integer", "minimum": 1},
            "column": {"type": "integer", "minimum": 1},
            "kind": {"type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}$"},
            "includeReferences": {"type": "boolean"},
            "maxDefinitions": int(1, 3),
            "maxReferences": int(0, 20),
            "maxLines": int(1, 80),
            "view": {"type": "string", "enum": ["source", "structure"]},
            "maxChildren": int(1, 40),
            "childOffset": int(0, 100000),
            "keyPath": {
                "type": "array", "maxItems": 24,
                "items": {"oneOf": [
                    {"type": "object", "properties": {"key": text(1024), "occurrence": int(0, 100000)},
                     "required": ["key"], "additionalProperties": false},
                    {"type": "object", "properties": {"index": int(0, 100000)},
                     "required": ["index"], "additionalProperties": false}
                ]}
            }
        },
        "anyOf": [{"required": ["symbol"]}, {"required": ["path"]}],
        "additionalProperties": false
    })
}

fn mod_index_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "enum": ["overview", "next_id"]},
            "layer": {"type": "string", "enum": ["mod", "all"]},
            "kind": {"type": "string", "enum": ["event", "focus", "decision", "idea", "leader", "technology"]},
            "prefix": text(256),
            "namespace": text(256)
        },
        "additionalProperties": false
    })
}

fn script_validate_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "source": text(64000),
            "path": rel_path(4096),
            "kind": {"type": "string", "enum": ["effect", "trigger"]},
            "scope": {"type": "string", "enum": SCOPES},
            "documentation": {"type": "string", "enum": ["game_doc", "script_doc"]},
            "bindings": {
                "type": "array", "maxItems": 16,
                "items": {"type": "object",
                    "properties": {"name": text(256), "scope": {"type": "string", "enum": SCOPES}},
                    "required": ["name", "scope"], "additionalProperties": false}
            },
            "limit": int(1, 32)
        },
        "anyOf": [{"required": ["source"]}, {"required": ["path"]}],
        "additionalProperties": false
    })
}

fn reference_search_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "minLength": 2, "maxLength": 160},
            "sources": {"type": "array", "minItems": 1, "maxItems": 3,
                "items": {"type": "string", "enum": ["game_doc", "wiki", "script_doc"]}},
            "limit": int(1, 12)
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

fn reference_read_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {"type": "string", "pattern": "^[a-f0-9]{64}$"},
            "revision": {"type": "string", "pattern": "^[a-f0-9]{64}$"},
            "source": {"type": "string", "enum": ["game_doc", "wiki", "script_doc"]},
            "startLine": {"type": "integer", "minimum": 1},
            "startColumn": {"type": "integer", "minimum": 1},
            "maxLines": int(1, 80)
        },
        "required": ["id", "revision"],
        "additionalProperties": false
    })
}

fn reference_context_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "surface": {"type": "string", "enum": ["general", "event", "decision", "idea", "focus", "technology", "gui", "map", "localisation", "ai"]},
            "question": {"type": "string", "minLength": 2, "maxLength": 160},
            "limit": int(1, 24)
        },
        "required": ["surface"],
        "additionalProperties": false
    })
}

fn focus_inspect_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "enum": ["national", "continuous"]},
            "relativePath": rel_path(1024),
            "treeId": text(256),
            "paletteId": text(256),
            "laneSpacing": int(1, 100),
            "nodeSpacing": int(1, 100),
            "scenario": {
                "type": "object",
                "properties": {"completedFocusIds": {"type": "array", "maxItems": 10000, "items": text(256)}},
                "required": ["completedFocusIds"],
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}

fn event_selector_schema() -> Value {
    json!({"oneOf": [
        {"type": "object", "properties": {"kind": {"const": "event"}, "eventId": text(256)},
         "required": ["kind", "eventId"], "additionalProperties": false},
        {"type": "object", "properties": {"kind": {"const": "namespace"}, "namespace": text(256)},
         "required": ["kind", "namespace"], "additionalProperties": false},
        {"type": "object", "properties": {"kind": {"const": "file"}, "sourcePath": rel_path(1024)},
         "required": ["kind", "sourcePath"], "additionalProperties": false},
        {"type": "object",
         "properties": {"kind": {"const": "source"}, "sourcePath": rel_path(1024),
                        "line": {"type": "integer", "minimum": 1}, "column": {"type": "integer", "minimum": 1}},
         "required": ["kind", "sourcePath", "line"], "additionalProperties": false}
    ]})
}

fn event_inspect_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "enum": ["scan", "roots", "trace", "explain_path", "lint"]},
            "selector": event_selector_schema(),
            "from": text(256),
            "to": text(256),
            "direction": {"type": "string", "enum": ["upstream", "downstream", "both"]},
            "maxDepth": int(1, 64),
            "maxNodes": int(1, 5000),
            "maxEdges": int(1, 20000),
            "expandHelpers": {"type": "boolean"},
            "maxChainNodes": int(1, 5000)
        },
        "required": ["mode"],
        "additionalProperties": false
    })
}


/// Scenario inputs are left out: they are keyed maps (`state`, `eventTargets`), which the
/// host's closed-schema rule does not accept, and inventory or inspection without them is the
/// read-only use S03 needs.
fn decision_inspect_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "mode": {"type": "string", "enum": ["inventory", "inspect"]},
            "id": text(512)
        },
        "additionalProperties": false
    })
}

/// True when every object schema in `schema` states its `additionalProperties` and none is open.
pub fn is_closed_schema(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => {
            let is_object = map.get("type").and_then(Value::as_str) == Some("object")
                || map.contains_key("properties");
            if is_object
                && matches!(
                    map.get("additionalProperties"),
                    None | Some(Value::Bool(true))
                )
            {
                return false;
            }
            map.values().all(is_closed_schema)
        }
        Value::Array(items) => items.iter().all(is_closed_schema),
        _ => true,
    }
}

#[allow(clippy::too_many_arguments)]
fn tool(
    mcp_name: &str,
    exposed_name: &str,
    description: &str,
    effect_class: EffectClass,
    host_schema: Value,
    timeout_secs: u64,
    max_output_bytes: usize,
    expected_schema_sha256: Option<&str>,
) -> ProfileTool {
    ProfileTool {
        mcp_name: mcp_name.into(),
        exposed_name: exposed_name.into(),
        description: description.into(),
        effect_class,
        host_schema,
        timeout: Duration::from_secs(timeout_secs),
        max_output_bytes,
        retry: RetryPolicy { max_attempts: 2 },
        inject_workspace: true,
        expected_schema_sha256: expected_schema_sha256.map(str::to_owned),
    }
}

/// The pinned HOI4 Agent Tools: its commit (also `AGENT_TOOLS_REV` in `docker/server.Dockerfile`)
/// and package version. Move both together, then refresh [`SCHEMA_HASHES`] from the real server
/// (the `real_agent_tools` test prints them) and rerun that test.
pub const AGENT_TOOLS_REV: &str = "fbc514407155e572fe95fa1e263b7e1552d4d6fc";
pub const AGENT_TOOLS_VERSION: &str = "4.6.1";

/// The S03 profile: nine read tools of the pinned HOI4 Agent Tools, nothing that writes mod source.
pub fn agent_tools_read_only() -> ToolProfile {
    use EffectClass::{DerivedJob, PureRead};
    ToolProfile {
        id: "hoi4-agent-tools.read-only.v1".into(),
        server: ServerIdentity {
            name: "hoi4-agent-tools".into(),
            version_req: AGENT_TOOLS_VERSION.into(),
        },
        mode: ProfileMode::ReadOnly,
        tools: vec![
            tool(
                "hoi4.source_lookup",
                "agent_tools_source_lookup",
                "Find where a script symbol is defined and used in the game, DLC and this mod.",
                PureRead,
                source_lookup_schema(),
                60,
                64 * 1024,
                SCHEMA_HASHES[0],
            ),
            tool(
                "hoi4.mod_index",
                "agent_tools_mod_index",
                "Summarize this mod's content, or find a free event, focus or decision ID.",
                PureRead,
                mod_index_schema(),
                120,
                64 * 1024,
                SCHEMA_HASHES[1],
            ),
            tool(
                "hoi4.script_validate",
                "agent_tools_script_validate",
                "Check an effect or trigger snippet, or a mod file, for syntax, command and scope errors.",
                PureRead,
                script_validate_schema(),
                60,
                64 * 1024,
                SCHEMA_HASHES[2],
            ),
            tool(
                "hoi4.reference_search",
                "agent_tools_reference_search",
                "Search the offline modding wiki and installed game docs.",
                PureRead,
                reference_search_schema(),
                30,
                48 * 1024,
                SCHEMA_HASHES[3],
            ),
            tool(
                "hoi4.reference_read",
                "agent_tools_reference_read",
                "Read a documentation section found by reference search or context.",
                PureRead,
                reference_read_schema(),
                30,
                32 * 1024,
                SCHEMA_HASHES[4],
            ),
            tool(
                "hoi4.reference_context",
                "agent_tools_reference_context",
                "Find documentation for the kind of content being built (events, focuses, decisions and so on).",
                PureRead,
                reference_context_schema(),
                30,
                48 * 1024,
                SCHEMA_HASHES[5],
            ),
            tool(
                "hoi4.focus_inspect",
                "agent_tools_focus_inspect",
                "Inspect focus trees and continuous-focus palettes for branches, prerequisites and layout problems.",
                DerivedJob,
                focus_inspect_schema(),
                120,
                64 * 1024,
                SCHEMA_HASHES[6],
            ),
            tool(
                "hoi4.event_inspect",
                "agent_tools_event_inspect",
                "Follow event chains and find broken event calls.",
                DerivedJob,
                event_inspect_schema(),
                120,
                64 * 1024,
                SCHEMA_HASHES[7],
            ),
            tool(
                "hoi4.decision_inspect",
                "agent_tools_decision_inspect",
                "Explain why a decision is hidden, unavailable or unaffordable for given scenarios.",
                DerivedJob,
                decision_inspect_schema(),
                120,
                64 * 1024,
                SCHEMA_HASHES[8],
            ),
        ],
    }
}

/// Input-schema hashes of the pinned server ([`AGENT_TOOLS_VERSION`]), in profile order.
const SCHEMA_HASHES: [Option<&str>; 9] = [
    Some("939462b1642f19d2753b6b192e5458be0745a41ffc38056047e613a58afb0a7c"),
    Some("ae6435313935a25847456a6fd057bbabe373150fbd8433012d22210affd20287"),
    Some("077ffc26f39c6510fbed9700a4885c05e8b4b6b79f6a038b4b8b9b5a38c2ae61"),
    Some("d86e10b44856dae435eb5b67a900c7e13218e52faedccb80c9a073630f3096ec"),
    Some("6b51bfb51884acc7bac95b3a77e750f059459b752c6758d8583385756e86fbed"),
    Some("5159991d7d1b98d3f274530ebe8ca4bcb5b77122d9580f5dcff7fd99ff97a7d8"),
    Some("b0abf49e379a42e76060b17190330e3483320f3f2cddf054de681b5b3b7200d9"),
    Some("3f93693404b4e038cb91472437e7d6865c4b51c74001606f7815121088527b81"),
    Some("b2135ab776f8fb9de86dd9d96d01799dba999f88c5010d5a89ce53f62f59ea04"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_closed_and_namespaced() {
        let profile = agent_tools_read_only();
        assert_eq!(profile.tools.len(), 9);
        for t in &profile.tools {
            assert!(is_closed_schema(&t.host_schema), "{} is open", t.mcp_name);
            assert!(!t.exposed_name.contains('.'));
            assert!(t.exposed_name.starts_with("agent_tools_"));
            assert!(
                t.host_schema["properties"].get("workspaceId").is_none(),
                "{} exposes workspaceId",
                t.mcp_name
            );
            jsonschema::validator_for(&t.host_schema)
                .unwrap_or_else(|e| panic!("{} schema invalid: {e}", t.mcp_name));
        }
    }

    #[test]
    fn open_schema_is_detected() {
        assert!(!is_closed_schema(&json!({"type":"object","properties":{}})));
        assert!(!is_closed_schema(
            &json!({"type":"object","properties":{"a":{"type":"object","additionalProperties":false}},"additionalProperties":true})
        ));
        assert!(is_closed_schema(
            &json!({"type":"object","properties":{},"additionalProperties":false})
        ));
    }

    #[test]
    fn paths_must_stay_inside_the_mod() {
        let schema = source_lookup_schema();
        let v = jsonschema::validator_for(&schema).expect("schema");
        for good in ["events/a.txt", "common/national_focus/x.txt"] {
            assert!(v.is_valid(&json!({"path": good})), "{good}");
        }
        for bad in ["/etc/passwd", "../x", "a/../../x", "C:/x", "a\\b"] {
            assert!(!v.is_valid(&json!({"path": bad})), "{bad}");
        }
        assert!(!v.is_valid(&json!({"workspaceId":"other","symbol":"a"})));
        assert!(!v.is_valid(&json!({})));
    }
}
