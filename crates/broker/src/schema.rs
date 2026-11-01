//! Closed JSON Schemas.
//!
//! The host enforces a tool's schema on the final arguments, after every hook or repair step. A
//! schema it cannot enforce strictly is refused at registration: every `object` (including
//! nested ones, `items`, `properties`, `$defs`) must set `additionalProperties: false`, `$ref`
//! must be local (`#...`), and keywords that would weaken validation (`patternProperties`,
//! `unevaluatedProperties` other than `false`, a schema-valued `additionalProperties`, `$id`,
//! dynamic references) are refused. A property or an array item with no `type`, `enum`, `const`,
//! `$ref` or combinator is refused too, because it would accept anything.

use jsonschema::Validator;
use serde_json::Value;

const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_ISSUES: usize = 20;
const MAX_PATH_CHARS: usize = 200;
const MAX_MESSAGE_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    #[error("schema is not closed at {path}: {reason}")]
    NotClosed { path: String, reason: String },
    #[error("schema is too large or too deeply nested")]
    TooComplex,
    #[error("schema does not compile: {0}")]
    Compile(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ValidationIssue {
    /// JSON pointer of the offending value inside the instance.
    pub path: String,
    pub message: String,
}

/// A schema the host can enforce strictly.
pub struct ClosedSchema {
    validator: Validator,
}

impl std::fmt::Debug for ClosedSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClosedSchema")
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(max).collect();
        cut.push_str("...");
        cut
    }
}

impl ClosedSchema {
    pub fn compile(schema: &Value) -> Result<ClosedSchema, SchemaError> {
        if schema.to_string().len() > MAX_SCHEMA_BYTES {
            return Err(SchemaError::TooComplex);
        }
        match schema.get("type") {
            Some(Value::String(t)) if t == "object" => {}
            _ => {
                return Err(not_closed("#", "the root schema must have type \"object\""));
            }
        }
        check(schema, "#", 0, Role::Root)?;
        let validator =
            jsonschema::validator_for(schema).map_err(|e| SchemaError::Compile(e.to_string()))?;
        Ok(ClosedSchema { validator })
    }

    /// Validates an instance. At most 20 issues are returned, each bounded in size.
    pub fn validate(&self, instance: &Value) -> Result<(), Vec<ValidationIssue>> {
        let issues: Vec<ValidationIssue> = self
            .validator
            .iter_errors(instance)
            .take(MAX_ISSUES)
            .map(|e| ValidationIssue {
                path: truncate(&e.instance_path().to_string(), MAX_PATH_CHARS),
                message: truncate(&e.to_string(), MAX_MESSAGE_CHARS),
            })
            .collect();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }
}

fn not_closed(path: &str, reason: &str) -> SchemaError {
    SchemaError::NotClosed {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Root,
    /// A property value or array item: must constrain something.
    Value,
    /// A `$defs` entry or a combinator branch.
    Other,
}

const REFUSED_KEYWORDS: [(&str, &str); 6] = [
    (
        "patternProperties",
        "patternProperties would accept undeclared properties",
    ),
    (
        "$id",
        "$id changes the base URI that references resolve against",
    ),
    ("$dynamicRef", "dynamic references are not enforceable"),
    ("$recursiveRef", "dynamic references are not enforceable"),
    ("$dynamicAnchor", "dynamic references are not enforceable"),
    ("$recursiveAnchor", "dynamic references are not enforceable"),
];

fn has_type_object(node: &serde_json::Map<String, Value>) -> bool {
    match node.get("type") {
        Some(Value::String(t)) => t == "object",
        Some(Value::Array(types)) => types.iter().any(|t| t == "object"),
        _ => false,
    }
}

fn check(node: &Value, path: &str, depth: usize, role: Role) -> Result<(), SchemaError> {
    if depth > MAX_DEPTH {
        return Err(SchemaError::TooComplex);
    }
    let map = match node {
        Value::Bool(false) => return Ok(()),
        Value::Bool(true) => {
            return Err(not_closed(path, "the schema `true` accepts anything"));
        }
        Value::Object(map) => map,
        _ => return Err(not_closed(path, "a schema must be an object")),
    };
    for (keyword, reason) in REFUSED_KEYWORDS {
        if map.contains_key(keyword) {
            return Err(not_closed(path, reason));
        }
    }
    if let Some(reference) = map.get("$ref") {
        match reference.as_str() {
            Some(r) if r.starts_with('#') => {}
            _ => {
                return Err(not_closed(
                    path,
                    "only local $ref (starting with #) is allowed",
                ));
            }
        }
    }
    if let Some(unevaluated) = map.get("unevaluatedProperties")
        && unevaluated != &Value::Bool(false)
    {
        return Err(not_closed(path, "unevaluatedProperties must be false"));
    }
    if let Some(additional) = map.get("additionalProperties")
        && additional != &Value::Bool(false)
    {
        return Err(not_closed(path, "additionalProperties must be false"));
    }
    let object_like = has_type_object(map) || map.contains_key("properties");
    if object_like && map.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err(not_closed(
            path,
            "an object must set additionalProperties to false",
        ));
    }
    if role == Role::Value {
        let constrains = ["type", "enum", "const", "$ref", "anyOf", "oneOf", "allOf"]
            .iter()
            .any(|k| map.contains_key(*k));
        if !constrains {
            return Err(not_closed(path, "the schema accepts any value"));
        }
    }

    for key in ["properties", "$defs", "definitions", "dependentSchemas"] {
        if let Some(Value::Object(children)) = map.get(key) {
            for (name, child) in children {
                let role = if key == "properties" {
                    Role::Value
                } else {
                    Role::Other
                };
                check(child, &format!("{path}/{key}/{name}"), depth + 1, role)?;
            }
        }
    }
    match map.get("items") {
        Some(Value::Array(items)) => {
            for (i, child) in items.iter().enumerate() {
                check(child, &format!("{path}/items/{i}"), depth + 1, Role::Value)?;
            }
        }
        Some(child) => check(child, &format!("{path}/items"), depth + 1, Role::Value)?,
        None => {}
    }
    if let Some(Value::Array(items)) = map.get("prefixItems") {
        for (i, child) in items.iter().enumerate() {
            check(
                child,
                &format!("{path}/prefixItems/{i}"),
                depth + 1,
                Role::Value,
            )?;
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(branches)) = map.get(key) {
            for (i, child) in branches.iter().enumerate() {
                check(child, &format!("{path}/{key}/{i}"), depth + 1, Role::Other)?;
            }
        }
    }
    for key in ["not", "if", "then", "else", "contains", "additionalItems"] {
        if let Some(child) = map.get(key) {
            check(child, &format!("{path}/{key}"), depth + 1, Role::Other)?;
        }
    }
    Ok(())
}
