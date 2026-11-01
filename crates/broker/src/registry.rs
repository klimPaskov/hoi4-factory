//! The tool registry and the final validation step (RT-04, EXT-01).
//!
//! [`ToolRegistry::prepare`] is called on the FINAL arguments, after every hook or repair step,
//! so a hook that moves a path or changes a type is caught. [`ToolRegistry::prepare_with_hooks`]
//! makes that ordering part of the API: hooks run first, validation last, and a hook cannot
//! rename the tool. Only a [`PreparedCall`] may be recorded as an intent.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{canonical_json, sha256_hex};
use crate::grants::{GrantSnapshot, OperationClass, PathRefusal, authorize_paths};
use crate::schema::{ClosedSchema, ValidationIssue};

/// Largest argument document the broker accepts.
pub const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    PureRead,
    DerivedJob,
    IdempotentWrite,
    UnknownEffect,
}

pub struct RegisteredTool {
    pub name: String,
    pub version: String,
    pub effect_class: EffectClass,
    pub operation_class: OperationClass,
    pub input: ClosedSchema,
    pub output: Option<ClosedSchema>,
    pub max_output_bytes: usize,
    /// Top-level argument names that hold workspace-relative paths (a string or an array of
    /// strings). [`ToolRegistry::prepare_for_grant`] checks them against the grant's roots.
    pub path_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("tool {0:?} is already registered")]
    Duplicate(String),
    #[error("tool name and version must be non-empty")]
    EmptyName,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrokerRefusal {
    #[error("unknown tool {name:?}")]
    UnknownTool { name: String },
    #[error("arguments for {tool:?} do not match its schema")]
    SchemaInvalid {
        tool: String,
        issues: Vec<ValidationIssue>,
    },
    #[error("arguments are {bytes} bytes, more than the {max} allowed")]
    ArgumentsTooLarge { bytes: usize, max: usize },
    #[error("a path argument is refused: {0}")]
    PathRefused(PathRefusal),
    #[error("the tool needs operation class {needed:?} but the grant was checked for {checked:?}")]
    ClassMismatch {
        needed: OperationClass,
        checked: OperationClass,
    },
}

/// A call that passed final validation. The only input to an intent record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparedCall {
    pub tool: String,
    pub version: String,
    pub effect_class: EffectClass,
    pub operation_class: OperationClass,
    /// The validated arguments, exactly as they will be executed.
    pub normalized_arguments: Value,
    pub canonical_json: String,
    pub arguments_sha256: String,
    /// The path arguments (see [`RegisteredTool::path_fields`]) in the final arguments.
    pub paths: Vec<String>,
    /// Hooks that changed the arguments, in order.
    pub hook_changes: Vec<HookChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HookChange {
    pub hook: String,
    pub before_sha256: String,
    pub after_sha256: String,
}

/// A step that may rewrite arguments before the final validation.
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn before_call(&self, tool: &str, args: &mut Value);
}

/// The verdict on a tool's returned data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutputVerdict {
    pub bytes: usize,
    pub sha256: String,
    /// True when the output is larger than the tool's bound. The host must not treat it as
    /// complete evidence.
    pub truncated: bool,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: RegisteredTool) -> Result<(), RegistryError> {
        if tool.name.trim().is_empty() || tool.version.trim().is_empty() {
            return Err(RegistryError::EmptyName);
        }
        if self.tools.contains_key(&tool.name) {
            return Err(RegistryError::Duplicate(tool.name));
        }
        self.tools.insert(tool.name.clone(), tool);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredTool> {
        self.tools.get(name)
    }

    fn tool(&self, name: &str) -> Result<&RegisteredTool, BrokerRefusal> {
        self.tools
            .get(name)
            .ok_or_else(|| BrokerRefusal::UnknownTool { name: name.into() })
    }

    /// Final validation of the arguments, as they are after every hook or repair step.
    pub fn prepare(
        &self,
        name: &str,
        final_arguments: &Value,
    ) -> Result<PreparedCall, BrokerRefusal> {
        self.prepare_inner(name, final_arguments, Vec::new())
    }

    fn prepare_inner(
        &self,
        name: &str,
        arguments: &Value,
        hook_changes: Vec<HookChange>,
    ) -> Result<PreparedCall, BrokerRefusal> {
        let tool = self.tool(name)?;
        let canonical = canonical_json(arguments);
        if canonical.len() > MAX_ARGUMENT_BYTES {
            return Err(BrokerRefusal::ArgumentsTooLarge {
                bytes: canonical.len(),
                max: MAX_ARGUMENT_BYTES,
            });
        }
        tool.input
            .validate(arguments)
            .map_err(|issues| BrokerRefusal::SchemaInvalid {
                tool: name.into(),
                issues,
            })?;
        let mut paths = Vec::new();
        for field in &tool.path_fields {
            match arguments.get(field) {
                Some(Value::String(p)) => paths.push(p.clone()),
                Some(Value::Array(items)) => {
                    paths.extend(items.iter().filter_map(|v| v.as_str()).map(str::to_owned));
                }
                _ => {}
            }
        }
        Ok(PreparedCall {
            tool: tool.name.clone(),
            paths,
            version: tool.version.clone(),
            effect_class: tool.effect_class,
            operation_class: tool.operation_class,
            normalized_arguments: arguments.clone(),
            arguments_sha256: sha256_hex(canonical.as_bytes()),
            canonical_json: canonical,
            hook_changes,
        })
    }

    /// Runs the hooks in order on a copy of the model's arguments, then validates the result.
    /// An unknown tool is refused before any hook runs.
    pub fn prepare_with_hooks(
        &self,
        name: &str,
        arguments: &Value,
        hooks: &[&dyn Hook],
    ) -> Result<PreparedCall, BrokerRefusal> {
        self.tool(name)?;
        let mut args = arguments.clone();
        let mut changes = Vec::new();
        for hook in hooks {
            let before = sha256_hex(canonical_json(&args).as_bytes());
            hook.before_call(name, &mut args);
            let after = sha256_hex(canonical_json(&args).as_bytes());
            if before != after {
                changes.push(HookChange {
                    hook: hook.name().to_owned(),
                    before_sha256: before,
                    after_sha256: after,
                });
            }
        }
        self.prepare_inner(name, &args, changes)
    }

    /// [`prepare_with_hooks`](Self::prepare_with_hooks), then the grant: the tool's operation
    /// class must be the one the snapshot was checked for, and every path argument of the FINAL
    /// arguments must sit under an allowed root. A hook that moves a path outside the roots is
    /// refused here.
    pub fn prepare_for_grant(
        &self,
        name: &str,
        arguments: &Value,
        hooks: &[&dyn Hook],
        snapshot: &GrantSnapshot,
    ) -> Result<PreparedCall, BrokerRefusal> {
        let prepared = self.prepare_with_hooks(name, arguments, hooks)?;
        if prepared.operation_class != snapshot.class {
            return Err(BrokerRefusal::ClassMismatch {
                needed: prepared.operation_class,
                checked: snapshot.class,
            });
        }
        let paths: Vec<&str> = prepared.paths.iter().map(String::as_str).collect();
        authorize_paths(snapshot, &paths).map_err(BrokerRefusal::PathRefused)?;
        Ok(prepared)
    }

    /// Checks returned data before it can become evidence: against the output schema when the
    /// tool has one, and against its size bound. Oversized output is marked truncated.
    pub fn validate_output(
        &self,
        name: &str,
        output: &Value,
    ) -> Result<OutputVerdict, BrokerRefusal> {
        let tool = self.tool(name)?;
        if let Some(schema) = &tool.output {
            schema
                .validate(output)
                .map_err(|issues| BrokerRefusal::SchemaInvalid {
                    tool: name.into(),
                    issues,
                })?;
        }
        let canonical = canonical_json(output);
        Ok(OutputVerdict {
            bytes: canonical.len(),
            sha256: sha256_hex(canonical.as_bytes()),
            truncated: canonical.len() > tool.max_output_bytes,
        })
    }
}
