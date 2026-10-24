mod common;

use std::fs;

use factory_workspace::{AccountId, ProjectId, ResolveMode, TaskId, WorkspaceError};

use common::{Fx, account, project};

#[test]
fn ids_accept_uuids_and_slugs_and_reject_everything_else() {
    for ok in [
        "a",
        "0",
        "acct-0001",
        "proj_1",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        &"a".repeat(64),
    ] {
        AccountId::new(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    for bad in [
        "",
        "-lead",
        "_lead",
        ".",
        "..",
        "a/b",
        "a\\b",
        "a b",
        "A",
        "Acct",
        "a.b",
        "é",
        "a\u{0}",
        "con",
        "nul",
        "com1",
        "lpt1",
        "a:b",
        "../x",
        "a%2fb",
        &"a".repeat(65),
    ] {
        assert!(AccountId::new(bad).is_err(), "{bad:?} was accepted");
        assert!(ProjectId::new(bad).is_err(), "{bad:?} was accepted");
        assert!(TaskId::new(bad).is_err(), "{bad:?} was accepted");
    }
}

#[test]
fn ids_validate_on_deserialize_and_serialize_as_plain_strings() {
    let id: AccountId = serde_json::from_str("\"acct-0001\"").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"acct-0001\"");
    assert!(serde_json::from_str::<AccountId>("\"../etc\"").is_err());
    assert!(serde_json::from_str::<TaskId>("\"\"").is_err());
}

#[test]
fn creates_the_documented_layout() {
    let fx = Fx::new();
    let data = fx.root.data_root();
    let project_dir = data.join("workspaces").join("acct-0001").join("proj-0001");
    assert!(project_dir.join("source").is_dir());
    assert!(project_dir.join("tasks").is_dir());
    assert_eq!(fx.ws.source_dir(), project_dir.join("source"));
    assert_eq!(
        fx.ws.task_dir(&common::task("t1")),
        project_dir.join("tasks").join("t1")
    );
    assert!(
        fx.root.verify_layout(Some(&fx.ws)).is_empty(),
        "{:?}",
        fx.root.verify_layout(Some(&fx.ws))
    );
}

#[test]
fn logical_locators_do_not_contain_host_paths() {
    let fx = Fx::new();
    let host = fx.root.data_root().to_string_lossy().into_owned();
    for locator in [
        fx.ws.locator(),
        fx.ws.source_locator(),
        fx.ws.task_locator(&common::task("t1")),
    ] {
        assert!(!locator.contains(&host), "{locator}");
        assert!(locator.starts_with("workspace:proj-0001"), "{locator}");
    }
}

#[test]
fn creating_twice_reports_already_exists_and_keeps_content() {
    let fx = Fx::new();
    fx.write("descriptor.mod", b"keep me");
    let again = fx.root.create_workspace(&account(), &project());
    assert!(matches!(again, Err(WorkspaceError::AlreadyExists)));
    assert_eq!(fx.read("descriptor.mod"), b"keep me");
}

#[test]
fn open_finds_existing_and_reports_missing() {
    let fx = Fx::new();
    let opened = fx.root.open_workspace(&account(), &project()).unwrap();
    assert_eq!(opened.source_dir(), fx.ws.source_dir());
    let other = ProjectId::new("proj-0002").unwrap();
    assert!(matches!(
        fx.root.open_workspace(&account(), &other),
        Err(WorkspaceError::NotFound)
    ));
    let other_account = AccountId::new("acct-0002").unwrap();
    assert!(matches!(
        fx.root.open_workspace(&other_account, &project()),
        Err(WorkspaceError::NotFound)
    ));
}

#[test]
fn accounts_and_projects_get_separate_directories() {
    let fx = Fx::new();
    let a2 = AccountId::new("acct-0002").unwrap();
    let ws2 = fx.root.create_workspace(&a2, &project()).unwrap();
    let ws3 = fx
        .root
        .create_workspace(&account(), &ProjectId::new("proj-0003").unwrap())
        .unwrap();
    assert_ne!(ws2.source_dir(), fx.ws.source_dir());
    assert_ne!(ws3.source_dir(), fx.ws.source_dir());
    fs::write(fx.ws.source_dir().join("only-mine.txt"), "x").unwrap();
    assert!(!ws2.source_dir().join("only-mine.txt").exists());
    // A path that tries to name a sibling project cannot be expressed.
    assert!(
        ws2.resolve_source("../proj-0001/only-mine.txt", ResolveMode::MustExist)
            .is_err()
    );
}

#[test]
fn resolve_helpers_work_inside_source_and_task_dirs() {
    let fx = Fx::new();
    fx.write("common/a.txt", b"x");
    fx.ws
        .resolve_source("common/a.txt", ResolveMode::MustExist)
        .unwrap();
    assert!(
        fx.ws
            .resolve_source("../x", ResolveMode::MayCreate)
            .is_err()
    );
    let t = common::task("t1");
    fs::create_dir_all(fx.ws.task_dir(&t)).unwrap();
    fx.ws
        .resolve_in_task(&t, "new/file.txt", ResolveMode::MayCreate)
        .unwrap();
    assert!(
        fx.ws
            .resolve_in_task(&t, "/abs", ResolveMode::MayCreate)
            .is_err()
    );
}

#[test]
fn workspace_without_git_reports_none() {
    let fx = Fx::new();
    assert!(fx.ws.git().unwrap().is_none());
}

#[cfg(unix)]
mod unix_modes {
    use super::*;
    use factory_workspace::StorageRoot;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn mode(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    #[test]
    fn modes_are_exact_regardless_of_umask() {
        let fx = Fx::new();
        let data = fx.root.data_root();
        assert_eq!(mode(data), 0o755);
        assert_eq!(mode(&data.join("workspaces")), 0o755);
        assert_eq!(mode(&data.join("workspaces/acct-0001")), 0o750);
        assert_eq!(mode(&data.join("workspaces/acct-0001/proj-0001")), 0o750);
        assert_eq!(mode(&fx.ws.source_dir()), 0o750);
        assert_eq!(mode(&fx.ws.tasks_dir()), 0o750);
    }

    #[test]
    fn a_narrow_account_directory_is_reported_and_repaired() {
        // S01-L22: an account directory at 0711 made Setup fail with EACCES.
        let fx = Fx::new();
        let account_dir = fx.root.data_root().join("workspaces/acct-0001");
        fs::set_permissions(&account_dir, fs::Permissions::from_mode(0o711)).unwrap();
        let issues = fx.root.verify_layout(Some(&fx.ws));
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].directory, "account");
        assert!(
            !serde_json::to_string(&issues)
                .unwrap()
                .contains(fx.root.data_root().to_str().unwrap())
        );

        fx.root
            .create_workspace(&account(), &ProjectId::new("proj-0002").unwrap())
            .unwrap();
        assert_eq!(
            mode(&account_dir),
            0o751 | 0o750,
            "repair widens, never narrows"
        );
        assert!(fx.root.verify_layout(Some(&fx.ws)).is_empty());
    }

    #[test]
    fn world_writable_directories_are_flagged() {
        let fx = Fx::new();
        fs::set_permissions(fx.ws.source_dir(), fs::Permissions::from_mode(0o777)).unwrap();
        let issues = fx.root.verify_layout(Some(&fx.ws));
        assert!(
            issues
                .iter()
                .any(|i| i.directory == "source" && i.problem.contains("world-writable"))
        );
    }

    #[test]
    fn existing_roots_are_not_chmodded_but_are_checked() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        fs::create_dir_all(data.join("workspaces")).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
        let root = StorageRoot::new(&data);
        root.ensure_layout().unwrap();
        assert_eq!(mode(&data), 0o700, "an existing data root is left alone");
        let issues = root.verify_layout(None);
        assert!(
            issues.iter().any(|i| i.directory == "data_root"),
            "{issues:?}"
        );
    }

    #[test]
    fn symlinked_account_or_project_directories_are_refused() {
        let fx = Fx::new();
        let outside = fx.tmp.path().join("elsewhere");
        fs::create_dir_all(outside.join("source")).unwrap();
        let workspaces = fx.root.data_root().join("workspaces");
        symlink(&outside, workspaces.join("acct-0009")).unwrap();
        let a9 = AccountId::new("acct-0009").unwrap();
        assert!(matches!(
            fx.root.create_workspace(&a9, &project()),
            Err(WorkspaceError::Layout(_))
        ));
        assert!(matches!(
            fx.root.open_workspace(&a9, &project()),
            Err(WorkspaceError::Layout(_)) | Err(WorkspaceError::NotFound)
        ));
        assert!(
            !outside.join("proj-0001").exists(),
            "nothing may be created through the link"
        );

        let acct_dir = workspaces.join("acct-0001");
        symlink(&outside, acct_dir.join("proj-0007")).unwrap();
        let p7 = ProjectId::new("proj-0007").unwrap();
        assert!(fx.root.create_workspace(&account(), &p7).is_err());
        assert!(matches!(
            fx.root.open_workspace(&account(), &p7),
            Err(WorkspaceError::Layout(_))
        ));
    }
}

#[test]
fn error_codes_and_text_hide_host_paths() {
    let fx = Fx::new();
    let err = fx
        .root
        .create_workspace(&account(), &project())
        .unwrap_err();
    assert_eq!(err.code(), "workspace_exists");
    let text = err.to_string();
    assert!(!text.contains(fx.root.data_root().to_str().unwrap()));
    let path_err = fx
        .ws
        .resolve_source("../x", ResolveMode::MayCreate)
        .unwrap_err();
    assert_eq!(path_err.code(), "path_rejected");
}
