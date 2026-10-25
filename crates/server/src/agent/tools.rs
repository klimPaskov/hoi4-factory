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
}

pub struct ToolSet {
    pub registry: ToolRegistry,
    pub kinds: HashMap<String, ToolKind>,
    pub specs: Vec<ToolSpec>,
    pub session: Option<ToolSession>,
    /// What discovery found, for the attempt's events and the C-06 capability profile.
    pub inventory: Value,
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
                            let Ok(input) = ClosedSchema::compile(&tool.input_schema) else {
                                continue;
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
        ToolSet {
            registry,
            kinds,
            specs,
            session,
            inventory,
        }
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
                        before: Some(outcome.before_sha256),
                        after: Some(outcome.after_sha256),
                        artifact: json!({ "path": args["path"] }),
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
            Some(ToolKind::Finish) | None => refused("unknown_tool", "this tool does not run"),
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = session.shutdown().await;
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
