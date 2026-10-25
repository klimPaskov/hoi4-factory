//! Closed schemas, the tool registry and hook ordering. No database needed.

use factory_broker::*;
use serde_json::{Value, json};

fn write_file_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "content"],
        "properties": {
            "path": {"type": "string", "maxLength": 200},
            "content": {"type": "string"},
            "mode": {"type": "string", "enum": ["create", "replace"]}
        }
    })
}

fn registry() -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register(RegisteredTool {
        name: "write_file".into(),
        version: "write_file@1".into(),
        effect_class: EffectClass::IdempotentWrite,
        operation_class: OperationClass::WriteTaskWorktree,
        input: ClosedSchema::compile(&write_file_schema()).unwrap(),
        output: Some(
            ClosedSchema::compile(&json!({
                "type": "object", "additionalProperties": false,
                "required": ["after_sha256"],
                "properties": {"after_sha256": {"type": "string"}}
            }))
            .unwrap(),
        ),
        max_output_bytes: 64,
        path_fields: vec!["path".into()],
    })
    .unwrap();
    r
}

fn not_closed(schema: Value) -> SchemaError {
    match ClosedSchema::compile(&schema) {
        Err(e) => e,
        Ok(_) => panic!("schema should have been refused: {schema}"),
    }
}

#[test]
fn open_schemas_are_refused() {
    // Top level without additionalProperties.
    let e = not_closed(json!({"type": "object", "properties": {"a": {"type": "string"}}}));
    assert!(matches!(e, SchemaError::NotClosed { .. }), "{e:?}");
    // additionalProperties: true, or a schema.
    not_closed(json!({"type": "object", "additionalProperties": true, "properties": {}}));
    not_closed(
        json!({"type": "object", "additionalProperties": {"type": "string"}, "properties": {}}),
    );
    // Nested object property.
    let e = not_closed(json!({
        "type": "object", "additionalProperties": false,
        "properties": {"inner": {"type": "object", "properties": {"x": {"type": "string"}}}}
    }));
    assert!(e.to_string().contains("/properties/inner"), "{e}");
    // Object inside array items.
    not_closed(json!({
        "type": "object", "additionalProperties": false,
        "properties": {"list": {"type": "array", "items": {"type": "object", "properties": {}}}}
    }));
    // Object in $defs.
    not_closed(json!({
        "type": "object", "additionalProperties": false, "properties": {},
        "$defs": {"d": {"type": "object", "properties": {}}}
    }));
    // Object in a combinator branch.
    not_closed(json!({
        "type": "object", "additionalProperties": false, "properties": {},
        "anyOf": [{"type": "object", "properties": {"a": {"type": "string"}}}]
    }));
    // Root must be an object.
    not_closed(json!({"type": "string"}));
    // A property that accepts anything.
    not_closed(json!({"type": "object", "additionalProperties": false, "properties": {"a": {}}}));
    not_closed(json!({"type": "object", "additionalProperties": false, "properties": {"a": true}}));
}

#[test]
fn weakening_keywords_and_remote_refs_are_refused() {
    let base = |extra: Value| {
        let mut s = json!({"type": "object", "additionalProperties": false, "properties": {"a": {"type": "string"}}});
        for (k, v) in extra.as_object().unwrap() {
            s[k] = v.clone();
        }
        s
    };
    not_closed(base(
        json!({"patternProperties": {"^x": {"type": "string"}}}),
    ));
    not_closed(base(json!({"unevaluatedProperties": true})));
    not_closed(base(json!({"$id": "https://example.com/s"})));
    not_closed(base(json!({"$dynamicRef": "#x"})));
    let remote = json!({
        "type": "object", "additionalProperties": false,
        "properties": {"a": {"$ref": "https://example.com/schema.json"}}
    });
    assert!(not_closed(remote).to_string().contains("local $ref"));
    // Local refs and unevaluatedProperties: false are fine.
    ClosedSchema::compile(&json!({
        "type": "object", "additionalProperties": false, "unevaluatedProperties": false,
        "properties": {"a": {"$ref": "#/$defs/s"}},
        "$defs": {"s": {"type": "string"}}
    }))
    .unwrap();
}

#[test]
fn validation_reports_bounded_issues() {
    let schema = ClosedSchema::compile(&write_file_schema()).unwrap();
    schema
        .validate(&json!({"path": "common/a.txt", "content": "x"}))
        .unwrap();
    let issues = schema
        .validate(&json!({"path": 3, "content": "x", "extra": 1}))
        .unwrap_err();
    assert!(!issues.is_empty() && issues.len() <= 20);
    assert!(issues.iter().all(|i| i.message.chars().count() <= 303));
    let long = "y".repeat(5000);
    let issues = schema
        .validate(&json!({"path": long, "content": "x"}))
        .unwrap_err();
    assert!(
        issues.iter().all(|i| i.message.chars().count() <= 303),
        "{issues:?}"
    );
}

struct Rewrite {
    path: &'static str,
}
impl Hook for Rewrite {
    fn name(&self) -> &str {
        "rewrite_path"
    }
    fn before_call(&self, _tool: &str, args: &mut Value) {
        args["path"] = json!(self.path);
    }
}
struct Retype;
impl Hook for Retype {
    fn name(&self) -> &str {
        "retype"
    }
    fn before_call(&self, _tool: &str, args: &mut Value) {
        args["content"] = json!(42);
    }
}
struct Noop;
impl Hook for Noop {
    fn name(&self) -> &str {
        "noop"
    }
    fn before_call(&self, _tool: &str, _args: &mut Value) {}
}

#[test]
fn prepare_validates_final_arguments_after_hooks() {
    let reg = registry();
    let good = json!({"path": "common/a.txt", "content": "x"});
    let prepared = reg
        .prepare_with_hooks("write_file", &good, &[&Noop])
        .unwrap();
    assert_eq!(prepared.tool, "write_file");
    assert_eq!(prepared.version, "write_file@1");
    assert_eq!(prepared.effect_class, EffectClass::IdempotentWrite);
    assert_eq!(prepared.operation_class, OperationClass::WriteTaskWorktree);
    assert!(prepared.hook_changes.is_empty());

    // A hook that changes a type is caught at prepare.
    let err = reg
        .prepare_with_hooks("write_file", &good, &[&Retype])
        .unwrap_err();
    assert!(
        matches!(err, BrokerRefusal::SchemaInvalid { .. }),
        "{err:?}"
    );

    // A hook that rewrites the path is applied, recorded, and the hash reflects the final value.
    let rewritten = reg
        .prepare_with_hooks(
            "write_file",
            &good,
            &[&Rewrite {
                path: "common2/b.txt",
            }],
        )
        .unwrap();
    assert_eq!(rewritten.normalized_arguments["path"], "common2/b.txt");
    assert_eq!(rewritten.hook_changes.len(), 1);
    assert_ne!(rewritten.arguments_sha256, prepared.arguments_sha256);
    assert_eq!(
        rewritten.hook_changes[0].after_sha256,
        rewritten.arguments_sha256
    );

    // The initial arguments being valid does not matter: only the final ones count.
    let bad_final = reg.prepare("write_file", &json!({"path": "a", "content": 1}));
    assert!(bad_final.is_err());
}

#[test]
fn unknown_tool_is_refused_before_hooks_run() {
    struct Panics;
    impl Hook for Panics {
        fn name(&self) -> &str {
            "panics"
        }
        fn before_call(&self, _t: &str, _a: &mut Value) {
            panic!("hook must not run for an unknown tool");
        }
    }
    let reg = registry();
    let err = reg
        .prepare_with_hooks("rm_rf", &json!({}), &[&Panics])
        .unwrap_err();
    assert_eq!(
        err,
        BrokerRefusal::UnknownTool {
            name: "rm_rf".into()
        }
    );
    assert!(matches!(
        reg.prepare("rm_rf", &json!({})),
        Err(BrokerRefusal::UnknownTool { .. })
    ));
}

#[test]
fn canonical_hash_is_stable_across_key_order() {
    let reg = registry();
    let a: Value = serde_json::from_str(r#"{"path":"p","content":"c","mode":"create"}"#).unwrap();
    let b: Value =
        serde_json::from_str(r#"{ "mode": "create", "content": "c", "path": "p" }"#).unwrap();
    let pa = reg.prepare("write_file", &a).unwrap();
    let pb = reg.prepare("write_file", &b).unwrap();
    assert_eq!(
        pa.canonical_json,
        r#"{"content":"c","mode":"create","path":"p"}"#
    );
    assert_eq!(pa.canonical_json, pb.canonical_json);
    assert_eq!(pa.arguments_sha256, pb.arguments_sha256);
    assert_eq!(pa.arguments_sha256.len(), 64);
    // Nested key order too.
    assert_eq!(
        canonical_json(&json!({"b": {"y": 1, "x": [ {"d": 1, "c": 2} ]}, "a": null})),
        r#"{"a":null,"b":{"x":[{"c":2,"d":1}],"y":1}}"#
    );
}

#[test]
fn oversized_arguments_are_refused() {
    let reg = registry();
    let big = "z".repeat(MAX_ARGUMENT_BYTES + 1);
    let err = reg
        .prepare("write_file", &json!({"path": "a", "content": big}))
        .unwrap_err();
    assert!(
        matches!(err, BrokerRefusal::ArgumentsTooLarge { .. }),
        "{err:?}"
    );
}

#[test]
fn output_is_validated_and_oversize_is_marked_truncated() {
    let reg = registry();
    let ok = reg
        .validate_output("write_file", &json!({"after_sha256": "abc"}))
        .unwrap();
    assert!(!ok.truncated);
    assert_eq!(ok.sha256.len(), 64);
    let bad = reg.validate_output("write_file", &json!({"after_sha256": 1}));
    assert!(matches!(bad, Err(BrokerRefusal::SchemaInvalid { .. })));
    let big = reg
        .validate_output("write_file", &json!({"after_sha256": "q".repeat(500)}))
        .unwrap();
    assert!(big.truncated && big.bytes > 64);
    assert!(matches!(
        reg.validate_output("nope", &json!({})),
        Err(BrokerRefusal::UnknownTool { .. })
    ));
}

#[test]
fn duplicate_registration_is_refused() {
    let mut reg = registry();
    let dup = RegisteredTool {
        name: "write_file".into(),
        version: "v2".into(),
        effect_class: EffectClass::PureRead,
        operation_class: OperationClass::ReadSource,
        input: ClosedSchema::compile(&write_file_schema()).unwrap(),
        output: None,
        max_output_bytes: 10,
        path_fields: vec![],
    };
    assert_eq!(
        reg.register(dup),
        Err(RegistryError::Duplicate("write_file".into()))
    );
}
