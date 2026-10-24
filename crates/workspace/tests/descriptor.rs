mod common;

use std::fs;
use std::path::Path;

use factory_game_ref::{Compatibility, GameReference, WarnDirection};
use factory_workspace::{
    Access, Capability, CapabilityStatus, ComponentClosure, DESCRIPTOR_SCHEMA_VERSION,
    DescriptorInputs, FileState, GameProfile, GeneratedFile, GitRepo, InitOptions, MemberRole,
    Role, RootHandle, RootKind, SetupReceiptSummary, UnresolvedCheck, WorkspaceDescriptor,
    WorkspaceId,
};

use common::{Fx, account, bom_crlf, task, tgit};

fn game_ref() -> GameReference {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("launcher-settings.json"),
        r#"{"version":"3.4.5 (zz00)","rawVersion":"v3.4.5.0"}"#,
    )
    .unwrap();
    GameReference::read(dir.path(), "/game").unwrap()
}

fn full_inputs() -> DescriptorInputs {
    let mut inputs = DescriptorInputs::new(WorkspaceId::new("ws-0001").unwrap());
    inputs.member_roles = vec![
        MemberRole {
            account_id: account(),
            role: Role::Owner,
        },
        MemberRole {
            account_id: "acct-0002".parse().unwrap(),
            role: Role::Contributor,
        },
    ];
    inputs.extra_roots = vec![
        RootHandle {
            kind: RootKind::GameReference,
            locator: "game-ref:current".into(),
            access: Access::ReadOnly,
        },
        RootHandle {
            kind: RootKind::Artifacts,
            locator: "artifacts:proj-0001".into(),
            access: Access::ReadWrite,
        },
    ];
    inputs.game_profile =
        GameProfile::new(Some(game_ref()), Some("3.3.*".into()), vec!["base".into()]);
    inputs.component_closure = ComponentClosure {
        setup_revision: Some("0123abcd".into()),
        manifest_hash: Some("ab".repeat(32)),
        resolved_assets: vec!["asset:one".into()],
        tool_versions: [("agent-tools".to_owned(), "4.0.3".to_owned())].into(),
    };
    inputs.capabilities = vec![
        Capability {
            name: "validate".into(),
            status: CapabilityStatus::Available,
            reason: None,
        },
        Capability {
            name: "game_launch".into(),
            status: CapabilityStatus::Unavailable,
            reason: Some("the server does not run the game".into()),
        },
    ];
    inputs.setup_receipt = Some(SetupReceiptSummary {
        plan_id: "plan-1".into(),
        transaction_state: "committed".into(),
        generated_files: vec![GeneratedFile {
            path: "common/x.txt".into(),
            sha256: "cd".repeat(32),
        }],
    });
    inputs.unresolved_checks = vec![UnresolvedCheck {
        check: "in-game load".into(),
        owner: "maintainer".into(),
        impact: "behaviour unverified".into(),
    }];
    inputs
}

fn seeded_repo(fx: &Fx) -> GitRepo {
    fx.write(
        "descriptor.mod",
        &bom_crlf(&["name=\"Fixture\"", "supported_version=\"3.3.*\""]),
    );
    fx.write("common/a.txt", b"a\n");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    repo.baseline_commit("baseline").unwrap();
    repo
}

#[test]
fn describes_an_uploaded_mod_without_git() {
    let fx = Fx::new();
    fx.write("descriptor.mod", b"name=\"x\"\n");
    let d = fx.ws.describe(full_inputs()).unwrap();
    assert_eq!(d.schema_version, DESCRIPTOR_SCHEMA_VERSION);
    assert!(!d.git_state.is_repository);
    assert_eq!(d.source_snapshot.git_revision, None);
    assert_eq!(d.remote_link, None, "an uploaded mod has no remote link");
    assert_eq!(d.source_snapshot.inventory.entries.len(), 1);
    assert_eq!(
        d.source_snapshot.inventory.entries[0].state,
        FileState::Unmanaged
    );
    assert_eq!(
        d.source_snapshot.inventory_hash,
        d.source_snapshot.inventory.content_hash()
    );
    assert_eq!(d.root_handles[0].kind, RootKind::ModSource);
    assert_eq!(d.root_handles[0].access, Access::ReadWrite);
}

#[test]
fn describes_a_clean_repository() {
    let fx = Fx::new();
    let repo = seeded_repo(&fx);
    let d = fx.ws.describe(full_inputs()).unwrap();
    let head = repo.head().unwrap().unwrap();
    assert!(d.git_state.is_repository);
    assert_eq!(d.git_state.head.as_ref(), Some(&head));
    assert_eq!(d.source_snapshot.git_revision.as_ref(), Some(&head));
    assert_eq!(d.git_state.branch.as_deref(), Some("main"));
    assert!(
        d.git_state.staged.is_empty()
            && d.git_state.unstaged.is_empty()
            && d.git_state.untracked.is_empty()
    );
    assert!(d.source_snapshot.dirty_files.is_empty());
    assert!(d.git_state.git_index_sha256.is_some());
    assert_eq!(d.git_state.worktree_locator, None);
    assert_eq!(d.git_state.expected_head, None);
}

#[test]
fn describes_dirty_state_with_hashes() {
    let fx = Fx::new();
    seeded_repo(&fx);
    fx.write("common/a.txt", b"a changed\n");
    fx.write("new_staged.txt", b"s\n");
    tgit(&fx.src(), &["add", "new_staged.txt"]);
    fx.write("untracked.txt", b"u\r\n");
    let d = fx.ws.describe(full_inputs()).unwrap();
    assert_eq!(d.git_state.unstaged, vec!["common/a.txt"]);
    assert_eq!(d.git_state.staged, vec!["new_staged.txt"]);
    assert_eq!(d.git_state.untracked, vec!["untracked.txt"]);
    let dirty: Vec<(&str, FileState)> = d
        .source_snapshot
        .dirty_files
        .iter()
        .map(|f| (f.path.as_str(), f.state))
        .collect();
    assert_eq!(
        dirty,
        vec![
            ("common/a.txt", FileState::TrackedModified),
            ("new_staged.txt", FileState::Staged),
            ("untracked.txt", FileState::Untracked),
        ]
    );
    assert_eq!(
        d.source_snapshot.dirty_files[2].sha256.as_deref(),
        Some(common::sha256_hex(b"u\r\n").as_str())
    );
}

#[test]
fn describes_a_task_worktree_with_expected_head_and_restore_points() {
    let fx = Fx::new();
    let repo = seeded_repo(&fx);
    let base = repo.head().unwrap().unwrap();
    let t = task("t1");
    let tw = repo
        .add_task_worktree(&t, &fx.ws.task_dir(&t), &base)
        .unwrap();
    tw.create_restore_point(&"start".parse().unwrap()).unwrap();

    let mut inputs = full_inputs();
    inputs.task = Some(tw.clone());
    let d = fx.ws.describe(inputs).unwrap();
    assert_eq!(d.git_state.branch.as_deref(), Some("hoi4factory/t1"));
    assert_eq!(d.git_state.expected_head.as_ref(), Some(&base));
    assert_eq!(
        d.git_state.worktree_locator.as_deref(),
        Some("workspace:proj-0001/tasks/t1")
    );
    assert_eq!(d.git_state.restore_points.len(), 1);
    let kinds: Vec<(RootKind, Access)> =
        d.root_handles.iter().map(|r| (r.kind, r.access)).collect();
    assert_eq!(
        kinds,
        vec![
            (RootKind::ModSource, Access::ReadOnly),
            (RootKind::TaskWorktree, Access::ReadWrite),
            (RootKind::GameReference, Access::ReadOnly),
            (RootKind::Artifacts, Access::ReadWrite),
        ]
    );
}

#[test]
fn recorded_remote_appears_in_the_descriptor_and_can_be_overridden() {
    let fx = Fx::new();
    let repo = seeded_repo(&fx);
    repo.link_remote("https://git.invalid/o/r.git", "main")
        .unwrap();
    let d = fx.ws.describe(full_inputs()).unwrap();
    assert_eq!(
        d.remote_link.as_ref().unwrap().url,
        "https://git.invalid/o/r.git"
    );
    assert_eq!(
        d.remote_link.as_ref().unwrap().default_branch.as_deref(),
        Some("main")
    );
}

#[test]
fn descriptor_carries_every_c01_field_and_no_host_path() {
    let fx = Fx::new();
    seeded_repo(&fx);
    let d = fx.ws.describe(full_inputs()).unwrap();
    let value = serde_json::to_value(&d).unwrap();
    let object = value.as_object().unwrap();
    for field in [
        "workspace_id",
        "tenant",
        "root_handles",
        "source_snapshot",
        "git_state",
        "game_profile",
        "remote_link",
        "component_closure",
        "capabilities",
        "setup_receipt",
        "unresolved_checks",
    ] {
        assert!(object.contains_key(field), "missing C-01 field {field}");
    }
    let text = serde_json::to_string(&d).unwrap();
    let host = fx.tmp.path().to_string_lossy().into_owned();
    assert!(
        !text.contains(&host),
        "host path leaked into the descriptor"
    );
    assert!(!text.contains(&host.replace('\\', "/")));
    assert!(!text.contains("\\\\"), "backslash path separators leaked");
    for locator in d.root_handles.iter().map(|r| r.locator.as_str()) {
        assert!(
            !Path::new(locator).is_absolute() || locator.starts_with("/game"),
            "{locator}"
        );
    }
}

#[test]
fn descriptor_round_trips_through_json() {
    let fx = Fx::new();
    seeded_repo(&fx);
    fx.write("untracked.txt", b"u");
    let d = fx.ws.describe(full_inputs()).unwrap();
    let json = serde_json::to_string_pretty(&d).unwrap();
    let back: WorkspaceDescriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
    // A malformed id fails closed on the way in.
    let tampered = json.replace("\"ws-0001\"", "\"../ws\"");
    assert!(serde_json::from_str::<WorkspaceDescriptor>(&tampered).is_err());
}

#[test]
fn describe_is_read_only() {
    let fx = Fx::new();
    let repo = seeded_repo(&fx);
    fx.write("common/a.txt", b"dirty\n");
    fx.write("staged.txt", b"s\n");
    tgit(&fx.src(), &["add", "staged.txt"]);
    let _ = tgit(&fx.src(), &["status", "--porcelain"]); // settle any stat refresh
    let index = fs::read(repo.index_path().unwrap()).unwrap();
    let files = common::snapshot_bytes(&fx.src());
    for _ in 0..2 {
        fx.ws.describe(full_inputs()).unwrap();
    }
    assert_eq!(fs::read(repo.index_path().unwrap()).unwrap(), index);
    assert_eq!(common::snapshot_bytes(&fx.src()), files);
}

#[test]
fn game_profile_reports_older_declarations_and_explicit_unknowns() {
    let profile = GameProfile::new(Some(game_ref()), Some("3.3.*".into()), vec![]);
    match &profile.compatibility {
        Compatibility::Warn { direction, reason } => {
            assert_eq!(*direction, WarnDirection::ModOlder);
            assert!(reason.contains("3.3.*"));
        }
        other => panic!("{other:?}"),
    }
    assert!(profile.unknowns.is_empty());

    let matching = GameProfile::new(Some(game_ref()), Some("3.4.*".into()), vec![]);
    assert_eq!(matching.compatibility, Compatibility::Match);

    let undeclared = GameProfile::new(Some(game_ref()), None, vec![]);
    assert!(matches!(
        undeclared.compatibility,
        Compatibility::Unknown { .. }
    ));
    assert_eq!(undeclared.unknowns.len(), 1);

    let no_reference = GameProfile::new(None, Some("3.4".into()), vec![]);
    assert!(matches!(
        no_reference.compatibility,
        Compatibility::Unknown { .. }
    ));
    assert!(!no_reference.unknowns.is_empty());
    assert!(no_reference.reference.is_none());
}
