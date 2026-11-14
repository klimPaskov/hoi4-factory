//! The tool set of one attempt: what the model is offered and how each call runs.
//!
//! Every tool is registered in the broker's [`ToolRegistry`] with a closed input schema, its
//! effect class and the C-08 operation class it needs. The runner validates the model's final
//! arguments there before anything else happens. The built-in file tools run in the task
//! worktree; HOI4 Agent Tools calls go through the configured read-only profile
//! ([`factory_hoi4_tools::agent_tools_read_only`]), whose session checks its own host schema
//! again and never sends a tool outside the profile.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use factory_agent::request::ToolSpec;
use factory_broker::{ClosedSchema, EffectClass, OperationClass, RegisteredTool, ToolRegistry};
use factory_hoi4_tools::{
    EffectClass as ProfileEffect, ToolSession, agent_tools_read_only, agent_tools_spec,
    prepare_state_dir,
};
use factory_tasks::TaskMode;
use serde_json::{Value, json};

use super::AgentToolsSettings;
use super::workspace_tools::{self as ws, LIST_FILES, READ_FILE, TOOL_VERSION, WRITE_FILE};

/// Largest tool output the model receives; the rest is cut and marked truncated.
pub const MAX_RESULT_CHARS: usize = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolKind {
    List,
    Read,
    Write,
    Finish,
    /// A HOI4 Agent Tools tool, by its exposed name.
    AgentTools,
    /// Reads an artifact an Agent Tools result linked (issue #5).
    ReadArtifact,
    /// The focus editor's projection of one tree (S04).
    FocusProjection,
    /// Focus operations through the same path as the editor's gestures (S04, FR-5).
    FocusEdit,
}

pub const READ_ARTIFACT: &str = "agent_tools_read_artifact";
pub const FOCUS_PROJECTION: &str = "focus_projection";
pub const FOCUS_EDIT: &str = "focus_edit";
const ARTIFACT_CHUNK: usize = 40_000;

/// What the focus tools need: the runtime, the task and the attempt that acts.
pub struct FocusContext {
    pub rt: super::AgentRuntime,
    pub keys: crate::focus::store::TaskKeys,
    pub attempt: factory_tasks::AttemptId,
    /// When the agent last read a focus tree: person edits after it are reported to the agent.
    pub last_read: chrono::DateTime<chrono::Utc>,
}

pub struct ToolSet {
    pub registry: ToolRegistry,
    pub kinds: HashMap<String, ToolKind>,
    pub specs: Vec<ToolSpec>,
    pub session: Option<ToolSession>,
    /// What discovery found, for the attempt's events and the C-06 capability profile.
    pub inventory: Value,
    pub focus: Option<FocusContext>,
}

/// What a tool call produced, before the receipt is written.
pub struct Executed {
    /// Text for the model.
    pub content: String,
    pub is_error: bool,
    /// For the receipt.
    pub status: factory_tasks::ReceiptStatus,
    pub before: Option<String>,
    pub after: Option<String>,
    pub artifact: Value,
    pub partial: bool,
}

fn closed(schema: &Value) -> ClosedSchema {
    // The built-in schemas and the profile's host schemas are closed by construction; a failure
    // here is a programming error caught by the tests.
    ClosedSchema::compile(schema).unwrap_or_else(|e| panic!("tool schema is not closed: {e}"))
}

impl ToolSet {
    /// Built-in tools for `mode`, plus Agent Tools when installed and healthy for this mod.
    pub async fn build(
        mode: TaskMode,
        worktree: &Path,
        tool_state: &Path,
        agent_tools: Option<&AgentToolsSettings>,
    ) -> ToolSet {
        let mut registry = ToolRegistry::new();
        let mut kinds = HashMap::new();
        let mut specs = Vec::new();
        for (name, description, schema) in ws::schemas() {
            let (kind, effect, class, path_fields) = match name {
                LIST_FILES => (ToolKind::List, EffectClass::PureRead, OperationClass::ReadSource, vec![]),
                READ_FILE => (ToolKind::Read, EffectClass::PureRead, OperationClass::ReadSource, vec![]),
                WRITE_FILE => (
                    ToolKind::Write,
                    EffectClass::IdempotentWrite,
                    OperationClass::WriteTaskWorktree,
                    vec!["path".to_owned()],
                ),
                _ => (ToolKind::Finish, EffectClass::PureRead, OperationClass::ReadSource, vec![]),
            };
            if kind == ToolKind::Write && mode != TaskMode::Edit {
                continue;
            }
            let _ = registry.register(RegisteredTool {
                name: name.to_owned(),
                version: TOOL_VERSION.to_owned(),
                effect_class: effect,
                operation_class: class,
                input: closed(&schema),
                output: None,
                max_output_bytes: MAX_RESULT_CHARS,
                path_fields,
            });
            kinds.insert(name.to_owned(), kind);
            specs.push(ToolSpec {
                name: name.to_owned(),
                description: description.to_owned(),
                input_schema: schema,
            });
        }

        let mut session = None;
        let inventory = match agent_tools {
            None => json!({ "agent_tools": "not installed on this server" }),
            Some(settings) => {
                match start_agent_tools(settings, worktree, tool_state).await {
                    Err(why) => json!({ "agent_tools": "unavailable", "reason": why }),
                    Ok(s) => {
                        let inv = serde_json::to_value(s.inventory()).unwrap_or(Value::Null);
                        for tool in s.exposed_tools() {
                            let effect = match tool.effect_class {
                                ProfileEffect::PureRead => EffectClass::PureRead,
                                ProfileEffect::DerivedJob => EffectClass::DerivedJob,
                                _ => continue,
                            };
                            let input = match ClosedSchema::compile(&tool.input_schema) {
                                Ok(input) => input,
                                Err(e) => {
                                    // Withheld: the host cannot enforce this schema.
                                    tracing::warn!(tool = %tool.name, error = %e, "tool withheld: its host schema is not closed");
                                    continue;
                                }
                            };
                            let version = s
                                .inventory()
                                .server
                                .as_ref()
                                .map(|h| format!("{}/{}", h.server_name, h.server_version))
                                .unwrap_or_else(|| "hoi4-agent-tools".into());
                            if registry
                                .register(RegisteredTool {
                                    name: tool.name.clone(),
                                    version,
                                    effect_class: effect,
                                    operation_class: OperationClass::RunReadOnlyTool,
                                    input,
                                    output: None,
                                    max_output_bytes: MAX_RESULT_CHARS,
                                    path_fields: vec![],
                                })
                                .is_ok()
                            {
                                kinds.insert(tool.name.clone(), ToolKind::AgentTools);
                                specs.push(ToolSpec {
                                    name: tool.name,
                                    description: tool.description,
                                    input_schema: tool.input_schema,
                                });
                            }
                        }
                        session = Some(s);
                        json!({ "agent_tools": inv })
                    }
                }
            }
        };
        let mut set = ToolSet {
            registry,
            kinds,
            specs,
            session,
            inventory,
            focus: None,
        };
        if set.session.is_some() {
            set.add(
                READ_ARTIFACT,
                ToolKind::ReadArtifact,
                "Read an artifact (hoi4-agent:// URI) that an agent_tools_* result linked, in chunks of up to 40,000 characters.",
                json!({
                    "type": "object",
                    "properties": {
                        "uri": {"type": "string", "pattern": "^hoi4-agent://[A-Za-z0-9_./:%-]{1,600}$"},
                        "offset": {"type": "integer", "minimum": 0, "maximum": 100_000_000}
                    },
                    "required": ["uri"],
                    "additionalProperties": false
                }),
                EffectClass::PureRead,
                OperationClass::RunReadOnlyTool,
            );
        }
        set
    }

    fn add(&mut self, name: &str, kind: ToolKind, description: &str, schema: Value, effect: EffectClass, class: OperationClass) {
        let _ = self.registry.register(RegisteredTool {
            name: name.to_owned(),
            version: TOOL_VERSION.to_owned(),
            effect_class: effect,
            operation_class: class,
            input: closed(&schema),
            output: None,
            max_output_bytes: MAX_RESULT_CHARS,
            path_fields: vec![],
        });
        self.kinds.insert(name.to_owned(), kind);
        self.specs.push(ToolSpec { name: name.to_owned(), description: description.to_owned(), input_schema: schema });
    }

    /// Adds the focus tools for an edit task on a server with Agent Tools (S04).
    pub fn enable_focus(&mut self, context: FocusContext) {
        self.add(
            FOCUS_PROJECTION,
            ToolKind::FocusProjection,
            "Show one national focus tree of this mod as the editor sees it: every focus with its id, title, grid x and y, cost, prerequisite groups (AND of OR groups), whether it can be edited, the diagnostics and the projection hash. Read it before focus_edit.",
            json!({
                "type": "object",
                "properties": {"tree_id": {"type": "string", "pattern": "^[A-Za-z0-9_.-]{1,256}$"}},
                "required": ["tree_id"],
                "additionalProperties": false
            }),
            EffectClass::PureRead,
            OperationClass::ReadSource,
        );
        self.add(
            FOCUS_EDIT,
            ToolKind::FocusEdit,
            "Change a national focus tree with up to 20 operations, applied together or not at all, exactly like the editor: move {focus_id, x, y}, set_cost {focus_id, cost}, link {focus_id, prerequisites: [ids, any one of them]}, set_text {focus_id, title, description (either or both, of an existing focus)}, create {focus_id, x, y, cost, prerequisites, icon (a GFX_ sprite), title, description, reward (effect lines such as add_political_power = 25)}. Pass expected_projection_hash from focus_projection. Only focuses with a literal x and y can move or link. The tool writes the focus file and the English localisation, and returns the new projection.",
            crate::focus::ops::ops_schema(),
            EffectClass::IdempotentWrite,
            OperationClass::WriteTaskWorktree,
        );
        self.focus = Some(context);
    }

    pub fn kind(&self, name: &str) -> Option<&ToolKind> {
        self.kinds.get(name)
    }

    pub fn version_of(&self, name: &str) -> String {
        self.registry
            .get(name)
            .map(|t| t.version.clone())
            .unwrap_or_else(|| TOOL_VERSION.to_owned())
    }

    /// Runs one call whose arguments the broker already accepted and whose intent is recorded.
    pub async fn execute(&mut self, name: &str, args: &Value, worktree: &Path) -> Executed {
        match self.kinds.get(name).cloned() {
            Some(ToolKind::List) | Some(ToolKind::Read) => {
                let root = worktree.to_path_buf();
                let args = args.clone();
                let read = name == READ_FILE;
                let result = tokio::task::spawn_blocking(move || {
                    let path = args["path"].as_str().unwrap_or("");
                    if read {
                        ws::read_file(&root, path)
                    } else {
                        ws::list_files(&root, path, args["recursive"].as_bool().unwrap_or(false))
                    }
                })
                .await;
                match result {
                    Ok(Ok(value)) => {
                        let hash = value["sha256"].as_str().map(str::to_owned);
                        let (content, partial) = bounded(&value);
                        Executed {
                            content,
                            is_error: false,
                            status: factory_tasks::ReceiptStatus::Succeeded,
                            before: hash.clone(),
                            after: hash,
                            artifact: json!({ "result_sha256": ws::sha256_hex(value.to_string().as_bytes()) }),
                            partial,
                        }
                    }
                    Ok(Err(failure)) => refused(failure.code(), &failure.to_string()),
                    Err(_) => failed("the read did not finish"),
                }
            }
            Some(ToolKind::Write) => {
                let root = worktree.to_path_buf();
                let path = args["path"].as_str().unwrap_or("").to_owned();
                let content = args["content"].as_str().unwrap_or("").to_owned();
                let expected = args["expected_sha256"].as_str().unwrap_or("").to_owned();
                let result = tokio::task::spawn_blocking(move || {
                    ws::write_file(&root, &path, &content, &expected)
                })
                .await;
                match result {
                    Ok(Ok(outcome)) => Executed {
                        content: json!({
                            "path": args["path"],
                            "written": outcome.changed,
                            "sha256": outcome.after_sha256,
                        })
                        .to_string(),
                        is_error: false,
                        status: if outcome.changed {
                            factory_tasks::ReceiptStatus::Succeeded
                        } else {
                            factory_tasks::ReceiptStatus::Unchanged
                        },
                        before: Some(outcome.before_sha256.clone()),
                        after: Some(outcome.after_sha256.clone()),
                        artifact: json!({
                            "path": args["path"],
                            "before": outcome.before_sha256,
                            "after": outcome.after_sha256,
                        }),
                        partial: false,
                    },
                    Ok(Err(failure)) => refused(failure.code(), &failure.to_string()),
                    // The blocking task died: whether bytes reached the disk is not known here.
                    // Recovery observes the file and reconciles.
                    Err(_) => Executed {
                        content: "The write did not finish; its outcome is being checked.".into(),
                        is_error: true,
                        status: factory_tasks::ReceiptStatus::UnknownEffect,
                        before: None,
                        after: None,
                        artifact: json!({ "path": args["path"] }),
                        partial: true,
                    },
                }
            }
            Some(ToolKind::AgentTools) => {
                let Some(session) = self.session.as_mut() else {
                    return refused("tool_unavailable", "HOI4 Agent Tools are not running");
                };
                let deadline = Instant::now() + Duration::from_secs(120);
                match session.call(name, args, deadline).await {
                    Ok(result) => {
                        let output = &result.output;
                        let body = output
                            .structured
                            .clone()
                            .unwrap_or_else(|| json!({ "content": output.content }));
                        let (content, partial) = bounded(&body);
                        Executed {
                            content,
                            is_error: output.is_error,
                            status: if output.is_error {
                                factory_tasks::ReceiptStatus::Failed
                            } else {
                                factory_tasks::ReceiptStatus::Succeeded
                            },
                            before: None,
                            after: None,
                            artifact: json!({
                                "bytes": output.bytes,
                                "attempts": result.attempts,
                                "elapsed_ms": result.elapsed_ms,
                                "result_sha256": ws::sha256_hex(body.to_string().as_bytes()),
                            }),
                            partial: partial || output.truncated,
                        }
                    }
                    Err(err) => {
                        // A read-only profile tool has no source effect: a timeout or a dead
                        // server leaves at most a cache entry, so the receipt is a failure.
                        failed(&err.to_string())
                    }
                }
            }
            Some(ToolKind::ReadArtifact) => {
                let Some(session) = self.session.as_mut() else {
                    return refused("tool_unavailable", "HOI4 Agent Tools are not running");
                };
                let uri = args["uri"].as_str().unwrap_or("");
                let offset = usize::try_from(args["offset"].as_u64().unwrap_or(0)).unwrap_or(0);
                match session.read_artifact(uri, Instant::now() + Duration::from_secs(120)).await {
                    Ok(a) => {
                        let chars: Vec<char> = a.text.chars().skip(offset).take(ARTIFACT_CHUNK + 1).collect();
                        let more = chars.len() > ARTIFACT_CHUNK;
                        let chunk: String = chars.into_iter().take(ARTIFACT_CHUNK).collect();
                        let next = more.then(|| offset + ARTIFACT_CHUNK);
                        let body = json!({"uri": a.uri, "mime_type": a.mime_type, "offset": offset, "next_offset": next, "text": chunk});
                        let (content, partial) = bounded(&body);
                        Executed {
                            content,
                            is_error: false,
                            status: factory_tasks::ReceiptStatus::Succeeded,
                            before: None,
                            after: None,
                            artifact: json!({"uri": uri, "offset": offset, "result_sha256": ws::sha256_hex(body.to_string().as_bytes())}),
                            partial,
                        }
                    }
                    Err(factory_hoi4_tools::ToolError::UnknownArtifact(_)) => {
                        refused("unknown_artifact", "No agent_tools_* result of this attempt linked that URI.")
                    }
                    Err(e) => failed(&e.to_string()),
                }
            }
            Some(ToolKind::FocusProjection) | Some(ToolKind::FocusEdit) => {
                let Some(ctx) = self.focus.as_mut() else {
                    return refused("tool_unavailable", "the focus tools are not enabled for this task");
                };
                focus_call(ctx, name == FOCUS_EDIT, args, worktree).await
            }
            Some(ToolKind::Finish) | None => refused("unknown_tool", "this tool does not run"),
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = session.shutdown().await;
        }
    }
}

/// The projection as the model sees it: the facts it needs to plan an edit, without source maps.
fn compact_projection(p: &Value) -> Value {
    let nodes: Vec<Value> = p["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|n| {
            let mut o = json!({
                "id": n["id"], "title": n["label"], "x": n["x"], "y": n["y"], "cost": n["cost"],
                "prerequisites": n["prerequisiteGroups"], "mutually_exclusive": n["mutuallyExclusive"],
                "editable": n["editable"],
            });
            if n["editable"] == false {
                o["not_editable_reason"] = n["notEditableReason"].clone();
            }
            o
        })
        .collect();
    let diagnostics: Vec<Value> = p["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .take(30)
        .map(|d| json!({"code": d["code"], "severity": d["severity"], "focus_id": d["focusId"], "message": d["message"]}))
        .collect();
    json!({
        "tree_id": p["treeId"], "source_path": p["sourcePath"], "projection_hash": p["projectionHash"],
        "layout_verdict": p["layoutVerdict"], "focuses": nodes, "diagnostics": diagnostics,
    })
}

type FocusOk = (Value, Option<String>, Option<String>, Value);

async fn focus_call(ctx: &mut FocusContext, edit: bool, args: &Value, worktree: &Path) -> Executed {
    use crate::focus::session::FocusError;
    let tree = args["tree_id"].as_str().unwrap_or("").to_owned();
    // Person edits since the agent's last read (C-03 addition), reported with every result.
    let since = ctx.last_read;
    let person_changes = match ctx.rt.tx(ctx.keys.acting).await {
        Ok(mut tx) => {
            let found = crate::focus::store::person_changes_since(&mut tx, ctx.keys.id, since).await.unwrap_or_default();
            let _ = tx.commit().await;
            found
        }
        Err(_) => Vec::new(),
    };
    ctx.last_read = ctx.rt.inner.clock.now();
    let notes: Vec<Value> = person_changes.iter().map(|(t, ids)| json!({"tree_id": t, "changed_ids": ids})).collect();
    let result: Result<FocusOk, FocusError> = if edit {
        let ops: Vec<crate::focus::ops::FocusOp> = match serde_json::from_value(args["operations"].clone()) {
            Ok(o) => o,
            Err(e) => return refused("invalid_operations", &e.to_string()),
        };
        crate::focus::apply::apply_operations(
            &ctx.rt,
            &ctx.keys,
            worktree,
            crate::focus::store::ActorKind::NativeAgent(ctx.attempt),
            &tree,
            args["expected_projection_hash"].as_str(),
            ops,
        )
        .await
        .map(|a| {
            let path = a.proposal["changedPaths"][0].as_str().map(str::to_owned);
            let before = path.as_ref().and_then(|p| a.proposal["before"][p].as_str()).map(str::to_owned);
            let after = path.as_ref().and_then(|p| a.proposal["after"][p].as_str()).map(str::to_owned);
            (
                json!({
                    "proposal_id": a.proposal["id"], "changed_ids": a.proposal["changedIds"],
                    "changed_paths": a.proposal["changedPaths"], "summary": a.proposal["summary"],
                    "projection": compact_projection(&a.projection),
                }),
                before,
                after,
                json!({"proposal_id": a.proposal["id"], "proposal_hash": a.proposal["proposalHash"], "changed_paths": a.proposal["changedPaths"]}),
            )
        })
    } else {
        crate::focus::apply::inspect_tree(&ctx.rt, &ctx.keys, worktree, &tree)
            .await
            .map(|p| (compact_projection(&p), None, None, json!({"projection_hash": p["projectionHash"]})))
    };
    let status_of = |e: &FocusError| match e {
        FocusError::UnknownEffect { .. } | FocusError::UnexpectedWrite { .. } => factory_tasks::ReceiptStatus::UnknownEffect,
        FocusError::Internal { .. } | FocusError::ToolsUnavailable { .. } => factory_tasks::ReceiptStatus::Failed,
        _ => factory_tasks::ReceiptStatus::Refused,
    };
    match result {
        Ok((mut body, before, after, artifact)) => {
            if !notes.is_empty() {
                body["changed_by_a_person_since_your_last_read"] = json!(notes);
            }
            let (content, partial) = bounded(&body);
            Executed {
                content,
                is_error: false,
                status: factory_tasks::ReceiptStatus::Succeeded,
                before,
                after,
                artifact,
                partial,
            }
        }
        Err(e) => {
            let mut body = json!({"error": e.code(), "detail": serde_json::to_value(&e).unwrap_or(Value::Null)});
            if !notes.is_empty() {
                body["changed_by_a_person_since_your_last_read"] = json!(notes);
            }
            let status = status_of(&e);
            Executed {
                content: body.to_string(),
                is_error: true,
                partial: status == factory_tasks::ReceiptStatus::UnknownEffect,
                status,
                before: None,
                after: None,
                artifact: json!({"error": e.code()}),
            }
        }
    }
}

fn refused(code: &str, message: &str) -> Executed {
    Executed {
        content: json!({ "error": code, "message": message }).to_string(),
        is_error: true,
        status: factory_tasks::ReceiptStatus::Refused,
        before: None,
        after: None,
        artifact: json!({ "error": code }),
        partial: false,
    }
}

fn failed(message: &str) -> Executed {
    Executed {
        content: json!({ "error": "tool_failed", "message": message }).to_string(),
        is_error: true,
        status: factory_tasks::ReceiptStatus::Failed,
        before: None,
        after: None,
        artifact: json!({ "error": "tool_failed" }),
        partial: false,
    }
}

/// The value as text for the model, cut at [`MAX_RESULT_CHARS`].
fn bounded(value: &Value) -> (String, bool) {
    let text = value.to_string();
    if text.len() <= MAX_RESULT_CHARS {
        return (text, false);
    }
    let mut end = MAX_RESULT_CHARS;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}… [truncated]", &text[..end]), true)
}

async fn start_agent_tools(
    settings: &AgentToolsSettings,
    worktree: &Path,
    tool_state: &Path,
) -> Result<ToolSession, String> {
    let state: PathBuf = {
        let dir = tool_state.to_path_buf();
        tokio::task::spawn_blocking(move || prepare_state_dir(&dir))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("tool state directory: {e}"))?
    };
    let spec = agent_tools_spec(
        &settings.install,
        worktree,
        &state,
        settings.game_root.as_deref(),
    );
    let session = ToolSession::start(spec, agent_tools_read_only()).await;
    if !session.inventory().is_healthy() {
        let reason = serde_json::to_string(&session.inventory().health).unwrap_or_default();
        let _ = session.shutdown().await;
        return Err(reason);
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::super::workspace_tools::FINISH;
    use super::*;

    #[test]
    fn every_agent_tools_profile_schema_is_enforceable_by_the_broker() {
        // A tool whose host schema the broker refuses would be withheld silently at runtime.
        for tool in agent_tools_read_only().tools {
            ClosedSchema::compile(&tool.host_schema)
                .unwrap_or_else(|e| panic!("{}: {e}", tool.exposed_name));
        }
    }

    #[tokio::test]
    async fn built_in_schemas_are_closed_and_read_only_has_no_write() {
        let dir = tempfile::tempdir().unwrap();
        let read_only = ToolSet::build(TaskMode::ReadOnly, dir.path(), dir.path(), None).await;
        assert!(read_only.kind(WRITE_FILE).is_none());
        assert!(read_only.kind(READ_FILE).is_some());
        let edit = ToolSet::build(TaskMode::Edit, dir.path(), dir.path(), None).await;
        assert_eq!(edit.kind(WRITE_FILE), Some(&ToolKind::Write));
        assert_eq!(edit.kind(FINISH), Some(&ToolKind::Finish));
        // A hook-mutated path outside the grant is the broker's job; a type change is refused here.
        assert!(edit.registry.prepare(WRITE_FILE, &json!({"path": 3})).is_err());
    }
}
