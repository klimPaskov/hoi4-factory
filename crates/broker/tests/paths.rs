//! Pure path and destination authorization on a grant snapshot, and the grant-aware prepare.

use chrono::{Duration, Utc};
use factory_accounts::{AccountId, ProjectId, Role};
use factory_broker::*;
use serde_json::{Value, json};
use uuid::Uuid;

fn snapshot(roots: &[&str], class: OperationClass) -> GrantSnapshot {
    let now = Utc::now();
    GrantSnapshot {
        grant: Grant {
            id: GrantId::new(),
            project_id: ProjectId::new(),
            owner_account_id: AccountId::new(),
            grantor_account_id: AccountId::new(),
            grantor_role: Role::Owner,
            task_id: Uuid::new_v4(),
            operation_classes: vec![class],
            allowed_roots: roots.iter().map(|r| r.to_string()).collect(),
            provider_destinations: vec!["anthropic:https://api.anthropic.com".into()],
            max_cost_micros: 1,
            max_turns: 1,
            max_changed_files: 1,
            expires_at: now + Duration::hours(1),
            policy_version: "p1".into(),
            proposal_sha256: None,
            revocation_epoch: 1,
            revoked_at: None,
            revoked_by: None,
            created_at: now,
        },
        class,
        checked_at: now,
    }
}

#[test]
fn paths_inside_roots_are_accepted() {
    let s = snapshot(&["common", "events/mod"], OperationClass::WriteTaskWorktree);
    authorize_paths(
        &s,
        &[
            "common/a.txt",
            "common/deep/er/b.txt",
            "events/mod/x.txt",
            "common",
        ],
    )
    .unwrap();
    authorize_paths(&s, &[]).unwrap();
}

#[test]
fn paths_outside_roots_are_refused() {
    let s = snapshot(&["common"], OperationClass::WriteTaskWorktree);
    // Sibling prefix: common must not authorize common2.
    assert!(matches!(
        authorize_paths(&s, &["common2/x"]),
        Err(PathRefusal::OutsideRoots { .. })
    ));
    assert!(matches!(
        authorize_paths(&s, &["commonx"]),
        Err(PathRefusal::OutsideRoots { .. })
    ));
    // Case variations compare exactly.
    assert!(matches!(
        authorize_paths(&s, &["Common/x"]),
        Err(PathRefusal::OutsideRoots { .. })
    ));
    assert!(matches!(
        authorize_paths(&s, &["COMMON/x"]),
        Err(PathRefusal::OutsideRoots { .. })
    ));
    // A path shorter than the root.
    let deep = snapshot(
        &["common/national_focus"],
        OperationClass::WriteTaskWorktree,
    );
    assert!(matches!(
        authorize_paths(&deep, &["common"]),
        Err(PathRefusal::OutsideRoots { .. })
    ));
    // One bad path among good ones fails the lot.
    assert!(authorize_paths(&s, &["common/ok", "events/x"]).is_err());
}

#[test]
fn malformed_paths_are_refused_as_invalid() {
    let s = snapshot(&["common"], OperationClass::WriteTaskWorktree);
    for bad in [
        "common/../events/x",
        "../common/x",
        "common/./x",
        "/common/x",
        "\\common\\x",
        "common\\x",
        "C:/common/x",
        "c:common",
        "",
        "common//x",
        "common/x\0",
        "common/.git/config",
    ] {
        assert!(
            matches!(
                authorize_paths(&s, &[bad]),
                Err(PathRefusal::Invalid { .. })
            ),
            "{bad:?} should be invalid, got {:?}",
            authorize_paths(&s, &[bad])
        );
    }
}

#[test]
fn no_roots_means_no_path() {
    let s = snapshot(&[], OperationClass::ReadSource);
    assert!(authorize_paths(&s, &["common/a"]).is_err());
}

#[test]
fn destinations_match_exactly() {
    let s = snapshot(&[], OperationClass::ModelInference);
    authorize_destination(&s, "anthropic:https://api.anthropic.com").unwrap();
    assert!(authorize_destination(&s, "anthropic:https://api.anthropic.com/").is_err());
    assert!(authorize_destination(&s, "openai:https://api.openai.com").is_err());
    assert!(authorize_destination(&s, "").is_err());
}

struct Move(&'static str);
impl Hook for Move {
    fn name(&self) -> &str {
        "move"
    }
    fn before_call(&self, _t: &str, a: &mut Value) {
        a["path"] = json!(self.0);
    }
}

fn registry(class: OperationClass) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register(RegisteredTool {
        name: "write_file".into(),
        version: "write_file@1".into(),
        effect_class: EffectClass::IdempotentWrite,
        operation_class: class,
        input: ClosedSchema::compile(&json!({
            "type": "object", "additionalProperties": false, "required": ["path"],
            "properties": {"path": {"type": "string"}, "content": {"type": "string"}}
        }))
        .unwrap(),
        output: None,
        max_output_bytes: 1024,
        path_fields: vec!["path".into()],
    })
    .unwrap();
    r
}

#[test]
fn a_hook_that_moves_a_path_outside_the_roots_is_refused_at_prepare() {
    let reg = registry(OperationClass::WriteTaskWorktree);
    let s = snapshot(&["common"], OperationClass::WriteTaskWorktree);
    let args = json!({"path": "common/a.txt", "content": "x"});
    let ok = reg.prepare_for_grant("write_file", &args, &[], &s).unwrap();
    assert_eq!(ok.paths, vec!["common/a.txt".to_string()]);

    for target in [
        "events/a.txt",
        "common2/a.txt",
        "../a.txt",
        "common/../events/a.txt",
    ] {
        let err = reg
            .prepare_for_grant("write_file", &args, &[&Move(target)], &s)
            .unwrap_err();
        assert!(
            matches!(err, BrokerRefusal::PathRefused(_)),
            "{target}: {err:?}"
        );
    }
}

#[test]
fn operation_class_must_match_the_checked_class() {
    let reg = registry(OperationClass::WriteTaskWorktree);
    let s = snapshot(&["common"], OperationClass::ReadSource);
    let err = reg
        .prepare_for_grant("write_file", &json!({"path": "common/a"}), &[], &s)
        .unwrap_err();
    assert!(
        matches!(err, BrokerRefusal::ClassMismatch { .. }),
        "{err:?}"
    );
}
