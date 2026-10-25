mod common;

use std::fs;
use std::path::Path;
use std::time::Instant;

use factory_workspace::{
    GitError, GitRepo, Identity, InitOptions, Oid, RestoreLabel, SnapshotInventory, WorkspaceError,
    task_branch_name, valid_branch_name,
};

use common::{Fx, bom_crlf, task, tgit, tgit_raw, tgit_text};

fn init_repo(fx: &Fx) -> GitRepo {
    GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap()
}

/// Source tree with a BOM+CRLF script, an LF file and a binary containing CR LF bytes.
fn seed(fx: &Fx) {
    fx.write(
        "descriptor.mod",
        &bom_crlf(&["name=\"Fixture\"", "supported_version=\"1.2.*\""]),
    );
    fx.write(
        "common/ideas/a.txt",
        &bom_crlf(&["ideas = {", "\tx = { }", "}"]),
    );
    fx.write(
        "localisation/x_l_english.yml",
        &bom_crlf(&["l_english:", " KEY:0 \"v\""]),
    );
    fx.write("common/lf.txt", b"lf = yes\n");
    fx.write("gfx/blob.dds", b"DDS \r\n\r\n\x00\xff\x0d\x0a\x1a");
    fx.write("no_newline.txt", b"end without newline\r");
}

const SEEDED: &[&str] = &[
    "descriptor.mod",
    "common/ideas/a.txt",
    "localisation/x_l_english.yml",
    "common/lf.txt",
    "gfx/blob.dds",
    "no_newline.txt",
];

// ---- init and consent ----

#[test]
fn init_requires_explicit_consent_and_creates_nothing_without_it() {
    let fx = Fx::new();
    fx.write("a.txt", b"a");
    let mut options = InitOptions::with_consent("main");
    options.consent = false;
    let err = GitRepo::init(&fx.src(), options).unwrap_err();
    assert!(matches!(err, GitError::ConsentRequired), "{err}");
    assert!(!fx.src().join(".git").exists());
    assert!(fx.ws.git().unwrap().is_none());
}

#[test]
fn init_refuses_an_existing_repository_and_never_reinitializes() {
    let fx = Fx::new();
    let repo = init_repo(&fx);
    fx.write("a.txt", b"a");
    repo.baseline_commit("one").unwrap();
    let head = repo.head().unwrap();
    let err = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap_err();
    assert!(matches!(err, GitError::AlreadyRepository));
    assert_eq!(repo.head().unwrap(), head);
}

#[test]
fn init_rejects_odd_branch_names() {
    let fx = Fx::new();
    for bad in [
        "", "-x", "a..b", "a b", "x.lock", "/x", "x/", ".hidden", "a/.b", "$(id)",
    ] {
        let err = GitRepo::init(&fx.src(), InitOptions::with_consent(bad));
        assert!(err.is_err(), "{bad:?}");
        assert!(!fx.src().join(".git").exists());
    }
    assert!(valid_branch_name("main") && valid_branch_name("release/1.2"));
}

#[test]
fn init_records_no_global_state_and_stores_crlf_safety_locally() {
    let fx = Fx::new();
    let repo = init_repo(&fx);
    assert_eq!(
        tgit_text(&fx.src(), &["config", "--local", "core.autocrlf"]),
        "false"
    );
    assert_eq!(
        tgit_text(&fx.src(), &["config", "--local", "core.safecrlf"]),
        "false"
    );
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
    assert_eq!(repo.head().unwrap(), None);
    // The new repository has no hook samples, templates or user identity baked in.
    assert!(!fx.src().join(".git/hooks/pre-commit.sample").exists());
    assert!(
        tgit_raw(&fx.src(), &["config", "--local", "user.name"])
            .stdout
            .is_empty()
    );
}

// ---- CRLF and BOM bytes (S01-L03) ----

#[test]
fn baseline_commit_preserves_bom_and_crlf_even_when_autocrlf_is_enabled() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    // Simulate Git for Windows' system default: autocrlf=true in effect for this repo.
    tgit(&fx.src(), &["config", "--local", "core.autocrlf", "true"]);
    tgit(&fx.src(), &["config", "--local", "core.safecrlf", "false"]);

    // Control: plain git with autocrlf=true does rewrite the blob, so this test can fail.
    tgit(
        &fx.src(),
        &["-c", "core.autocrlf=true", "add", "common/ideas/a.txt"],
    );
    let rewritten = tgit(&fx.src(), &["cat-file", "blob", ":common/ideas/a.txt"]);
    assert_ne!(
        rewritten,
        fx.read("common/ideas/a.txt"),
        "control: autocrlf=true should normalise CRLF"
    );
    tgit(&fx.src(), &["rm", "--cached", "-q", "common/ideas/a.txt"]);

    let head = repo.baseline_commit("baseline").unwrap();
    for path in SEEDED {
        let blob = tgit(&fx.src(), &["cat-file", "blob", &format!("{head}:{path}")]);
        assert_eq!(
            blob,
            fx.read(path),
            "blob of {path} differs from the working file"
        );
    }
    assert!(fx.read("descriptor.mod").starts_with(common::BOM));
}

#[test]
fn task_worktree_checkout_and_commit_preserve_bom_and_crlf_under_autocrlf_true() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    tgit(&fx.src(), &["config", "--local", "core.autocrlf", "true"]);

    let dir = fx.ws.task_dir(&task("t1"));
    let mut tw = repo.add_task_worktree(&task("t1"), &dir, &base).unwrap();
    for path in SEEDED {
        assert_eq!(
            fs::read(dir.join(path)).unwrap(),
            fx.read(path),
            "checkout of {path}"
        );
    }

    let edited = bom_crlf(&["ideas = {", "\ty = { }", "}"]);
    fs::write(dir.join("common/ideas/a.txt"), &edited).unwrap();
    let new_file = bom_crlf(&["added = yes"]);
    fs::create_dir_all(dir.join("events")).unwrap();
    fs::write(dir.join("events/new.txt"), &new_file).unwrap();
    let commit = tw
        .commit_paths(
            &["common/ideas/a.txt".into(), "events/new.txt".into()],
            "edit",
            None,
        )
        .unwrap();
    assert_eq!(
        tgit(
            &dir,
            &["cat-file", "blob", &format!("{commit}:common/ideas/a.txt")]
        ),
        edited
    );
    assert_eq!(
        tgit(
            &dir,
            &["cat-file", "blob", &format!("{commit}:events/new.txt")]
        ),
        new_file
    );
    // Untouched files stay byte-identical in the new commit as well.
    assert_eq!(
        tgit(
            &dir,
            &["cat-file", "blob", &format!("{commit}:descriptor.mod")]
        ),
        fx.read("descriptor.mod")
    );
}

// ---- the user's own state stays untouched ----

#[test]
fn branch_and_worktree_operations_leave_the_dirty_index_and_untracked_files_byte_identical() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();

    // The user's dirty state: modified, staged, staged+modified, untracked, ignored, deleted.
    fx.write(".gitignore", b"*.log\n");
    tgit(&fx.src(), &["add", ".gitignore"]);
    fx.write("common/lf.txt", b"lf = no\n");
    fx.write("common/staged.txt", &bom_crlf(&["staged = 1"]));
    tgit(&fx.src(), &["add", "common/staged.txt"]);
    fx.write("common/staged.txt", &bom_crlf(&["staged = 2"]));
    fx.write("scratch/notes.txt", b"my notes\r\n");
    fx.write("run.log", b"log\n");
    fs::remove_file(fx.src().join("no_newline.txt")).unwrap();

    let index_path = repo.index_path().unwrap();
    // Plain `git status` may refresh the index, so settle it before taking the baseline.
    let status_before = tgit_text(&fx.src(), &["status", "--porcelain=v2", "--ignored"]);
    let index_before = fs::read(&index_path).unwrap();
    let files_before = common::snapshot_bytes(&fx.src());
    let inventory_before = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    let branch_before = repo.current_branch().unwrap();
    let head_before = repo.head().unwrap();

    let t = task("t-dirty");
    let branch = repo.create_task_branch(&t, &base).unwrap();
    assert_eq!(branch.name, task_branch_name(&t));
    let mut tw = repo
        .add_task_worktree(
            &task("t-dirty-2"),
            &fx.ws.task_dir(&task("t-dirty-2")),
            &base,
        )
        .unwrap();
    tw.create_restore_point(&RestoreLabel::new("before-apply").unwrap())
        .unwrap();
    fs::write(tw.work_dir().join("common/lf.txt"), b"task edit\n").unwrap();
    tw.commit_paths(&["common/lf.txt".into()], "task commit", None)
        .unwrap();
    repo.open_task_branch(&t).unwrap();

    assert_eq!(
        fs::read(&index_path).unwrap(),
        index_before,
        "real index bytes changed"
    );
    let files_after = common::snapshot_bytes(&fx.src());
    assert_eq!(
        files_after, files_before,
        "a file in the user's working tree changed"
    );
    assert_eq!(
        tgit_text(&fx.src(), &["status", "--porcelain=v2", "--ignored"]),
        status_before
    );
    assert_eq!(repo.current_branch().unwrap(), branch_before);
    assert_eq!(
        repo.head().unwrap(),
        head_before,
        "the user's branch must not move"
    );
    let inventory_after = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert!(inventory_before.diff(&inventory_after).is_empty());
    assert_eq!(inventory_before, inventory_after);
}

#[test]
fn nothing_is_pushed_fetched_or_remote_tracked() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    // An unroutable remote: any network use would hang or fail visibly.
    repo.link_remote("https://git.invalid/owner/mod.git", "main")
        .unwrap();
    let started = Instant::now();
    let mut tw = repo
        .add_task_worktree(&task("t1"), &fx.ws.task_dir(&task("t1")), &base)
        .unwrap();
    fs::write(tw.work_dir().join("common/lf.txt"), b"x\n").unwrap();
    tw.commit_paths(&["common/lf.txt".into()], "c", None)
        .unwrap();
    assert!(started.elapsed().as_secs() < 30);
    let refs = tgit_text(&fx.src(), &["for-each-ref", "--format=%(refname)"]);
    assert!(!refs.contains("refs/remotes"), "{refs}");
    assert!(!fx.src().join(".git/FETCH_HEAD").exists());
}

// ---- owned branches and worktrees ----

#[test]
fn creates_and_reopens_an_owned_branch_without_touching_the_checkout() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("task-1");
    let created = repo.create_task_branch(&t, &base).unwrap();
    assert_eq!(created.name, "hoi4factory/task-1");
    assert_eq!(created.tip, base);
    let reopened = repo.open_task_branch(&t).unwrap();
    assert_eq!(reopened, created);
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));
    assert!(matches!(
        repo.create_task_branch(&t, &base),
        Err(GitError::BranchExists)
    ));
    assert!(matches!(
        repo.open_task_branch(&task("other")),
        Err(GitError::BranchMissing)
    ));
}

#[test]
fn invalid_base_commits_are_rejected() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    let missing = Oid::new("1".repeat(40)).unwrap();
    assert!(repo.create_task_branch(&task("t1"), &missing).is_err());
    assert!(
        repo.add_task_worktree(&task("t1"), &fx.ws.task_dir(&task("t1")), &missing)
            .is_err()
    );
    assert!(Oid::new("xyz").is_err());
    assert!(
        Oid::new("A".repeat(40)).is_err(),
        "uppercase is not canonical"
    );
    assert!(!fx.ws.task_dir(&task("t1")).exists());
}

#[test]
fn task_worktree_is_a_linked_checkout_of_the_base_on_an_owned_branch() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let tw = repo.add_task_worktree(&t, &dir, &base).unwrap();
    assert_eq!(tw.branch(), "hoi4factory/t1");
    assert_eq!(tw.expected_head(), &base);
    assert_eq!(
        tw.repo().current_branch().unwrap().as_deref(),
        Some("hoi4factory/t1")
    );
    assert!(
        dir.join(".git").is_file(),
        "linked worktrees use a .git file"
    );
    assert_eq!(tw.verify_head().unwrap(), base);
    // The primary checkout stays on its own branch.
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));

    // Reopen later from the persisted expectation.
    let again = repo.open_task_worktree(&t, &dir, Some(&base)).unwrap();
    assert_eq!(again.expected_head(), &base);
    // Collisions are refused and leave the first worktree alone.
    assert!(matches!(
        repo.add_task_worktree(&t, &dir, &base),
        Err(GitError::BranchExists)
    ));
    assert!(matches!(
        repo.add_task_worktree(&task("t2"), &dir, &base),
        Err(GitError::WorktreeExists)
    ));
}

#[test]
fn reopen_refuses_directories_that_are_not_this_tasks_worktree() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t1 = task("t1");
    let t2 = task("t2");
    let dir1 = fx.ws.task_dir(&t1);
    let dir2 = fx.ws.task_dir(&t2);
    repo.add_task_worktree(&t1, &dir1, &base).unwrap();
    repo.add_task_worktree(&t2, &dir2, &base).unwrap();
    // t1's directory with t2's branch name.
    assert!(matches!(
        repo.open_task_worktree(&t2, &dir1, None),
        Err(GitError::WrongBranch { .. })
    ));
    // A plain directory.
    let plain = fx.ws.tasks_dir().join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert!(matches!(
        repo.open_task_worktree(&t1, &plain, None),
        Err(GitError::WorktreeMissing)
    ));
    // The primary source dir is not a linked worktree.
    assert!(repo.open_task_worktree(&t1, &fx.src(), None).is_err());
}

#[test]
fn a_gitdir_pointer_outside_the_repository_is_refused() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    repo.create_task_branch(&task("t1"), &base).unwrap();
    // A real worktree makes .git/worktrees exist, so the pointer check is what refuses.
    repo.add_task_worktree(&task("t0"), &fx.ws.task_dir(&task("t0")), &base)
        .unwrap();
    let outside = fx.tmp.path().join("evil-gitdir");
    fs::create_dir_all(&outside).unwrap();
    let dir = fx.ws.task_dir(&task("t1"));
    fs::create_dir_all(&dir).unwrap();
    let pointer = format!("gitdir: {}\n", outside.to_string_lossy().replace('\\', "/"));
    fs::write(dir.join(".git"), pointer).unwrap();
    let err = repo
        .open_task_worktree(&task("t1"), &dir, None)
        .unwrap_err();
    assert!(matches!(err, GitError::GitDirOutsideWorkspace), "{err}");
}

// ---- HEAD movement ----

#[test]
fn refuses_when_head_moved_unexpectedly() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let mut tw = repo.add_task_worktree(&t, &dir, &base).unwrap();

    // Someone else commits on the task branch behind the host's back.
    fs::write(dir.join("common/lf.txt"), b"outside change\n").unwrap();
    tgit(&dir, &["add", "common/lf.txt"]);
    tgit(
        &dir,
        &[
            "-c",
            "core.autocrlf=false",
            "commit",
            "-q",
            "-m",
            "external",
        ],
    );
    let moved = Oid::new(tgit_text(&dir, &["rev-parse", "HEAD"])).unwrap();
    assert_ne!(moved, base);

    let err = tw.verify_head().unwrap_err();
    match err {
        GitError::HeadMoved { expected, actual } => {
            assert_eq!(expected, base.to_string());
            assert_eq!(actual, moved.to_string());
        }
        other => panic!("{other}"),
    }
    assert!(matches!(
        repo.open_task_worktree(&t, &dir, Some(&base)),
        Err(GitError::HeadMoved { .. })
    ));
    assert!(matches!(
        tw.create_restore_point(&RestoreLabel::new("r1").unwrap()),
        Err(GitError::HeadMoved { .. })
    ));
    fs::write(dir.join("common/lf.txt"), b"my change\n").unwrap();
    let err = tw
        .commit_paths(&["common/lf.txt".into()], "mine", None)
        .unwrap_err();
    assert!(
        matches!(err, WorkspaceError::Git(GitError::HeadMoved { .. })),
        "{err}"
    );
    assert_eq!(err.code(), "head_moved");
    assert_eq!(
        fs::read(dir.join("common/lf.txt")).unwrap(),
        b"my change\n",
        "file must stay as written"
    );
    assert!(matches!(
        repo.remove_task_worktree(&tw),
        Err(GitError::HeadMoved { .. })
    ));

    // After the host reconciles, work continues from the new head.
    assert_eq!(tw.accept_current_head().unwrap(), moved);
    tw.verify_head().unwrap();
    tw.commit_paths(&["common/lf.txt".into()], "mine", None)
        .unwrap();
}

#[test]
fn refuses_when_the_branch_was_reset_from_another_worktree() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    fx.write("later.txt", b"later\n");
    tgit(&fx.src(), &["add", "later.txt"]);
    tgit(&fx.src(), &["commit", "-q", "-m", "later"]);
    let later = repo.head().unwrap().unwrap();
    let t = task("t1");
    let tw = repo
        .add_task_worktree(&t, &fx.ws.task_dir(&t), &base)
        .unwrap();
    tgit(
        &fx.src(),
        &["update-ref", "refs/heads/hoi4factory/t1", later.as_str()],
    );
    assert!(matches!(tw.verify_head(), Err(GitError::HeadMoved { .. })));
}

#[test]
fn refuses_when_the_worktree_was_switched_to_another_branch() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let tw = repo.add_task_worktree(&t, &dir, &base).unwrap();
    tgit(&dir, &["checkout", "-q", "-b", "somewhere-else"]);
    assert!(matches!(
        tw.verify_head(),
        Err(GitError::WrongBranch { .. })
    ));
    assert!(matches!(
        repo.open_task_worktree(&t, &dir, None),
        Err(GitError::WrongBranch { .. })
    ));
    let detached = tgit_raw(&dir, &["checkout", "-q", "--detach"]);
    assert!(detached.status.success());
    assert!(matches!(
        tw.verify_head(),
        Err(GitError::WrongBranch { actual: None, .. })
    ));
}

// ---- restore points ----

#[test]
fn restore_points_pin_the_commit_under_the_owned_namespace() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let mut tw = repo
        .add_task_worktree(&t, &fx.ws.task_dir(&t), &base)
        .unwrap();
    let first = tw
        .create_restore_point(&RestoreLabel::new("start").unwrap())
        .unwrap();
    assert_eq!(first.ref_name, "refs/hoi4factory/restore/t1/start");
    assert_eq!(first.oid, base);
    assert!(matches!(
        tw.create_restore_point(&RestoreLabel::new("start").unwrap()),
        Err(GitError::RestorePointExists)
    ));

    fs::write(tw.work_dir().join("common/lf.txt"), b"v2\n").unwrap();
    let c2 = tw
        .commit_paths(&["common/lf.txt".into()], "v2", None)
        .unwrap();
    let second = tw
        .create_restore_point(&RestoreLabel::new("after-v2").unwrap())
        .unwrap();
    assert_eq!(second.oid, c2);

    let listed = tw.restore_points().unwrap();
    assert_eq!(listed, vec![second, first.clone()]);
    // Another task's namespace is separate.
    let other = task("t2");
    let tw2 = repo
        .add_task_worktree(&other, &fx.ws.task_dir(&other), &base)
        .unwrap();
    assert!(tw2.restore_points().unwrap().is_empty());

    // The ref really holds the commit: git resolves it and it is not a branch.
    assert_eq!(
        tgit_text(&fx.src(), &["rev-parse", &first.ref_name]),
        base.to_string()
    );
    let branches = tgit_text(&fx.src(), &["branch", "--format=%(refname)"]);
    assert!(!branches.contains("restore"), "{branches}");
}

#[test]
fn branch_from_an_unborn_repository_is_refused() {
    let fx = Fx::new();
    let repo = init_repo(&fx);
    assert!(
        repo.create_task_branch(&task("t1"), &Oid::new("2".repeat(40)).unwrap())
            .is_err()
    );
    assert_eq!(repo.head().unwrap(), None);
}

// ---- commit_paths ----

#[test]
fn commit_paths_commits_exactly_the_named_set_with_author_and_committer() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let mut tw = repo.add_task_worktree(&t, &dir, &base).unwrap();

    fs::write(dir.join("common/lf.txt"), b"named\n").unwrap();
    fs::write(dir.join("unrelated.txt"), b"not named\n").unwrap();
    fs::write(
        dir.join("common/ideas/a.txt"),
        b"also changed but not named\n",
    )
    .unwrap();
    let author = Identity::new("Ada Member", "ada@example.invalid").unwrap();
    let commit = tw
        .commit_paths(
            &["common/lf.txt".into()],
            "Named change\n\nBody line",
            Some(&author),
        )
        .unwrap();
    assert_eq!(tw.expected_head(), &commit);
    let files = tgit_text(
        &dir,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            commit.as_str(),
        ],
    );
    assert_eq!(files, "common/lf.txt");
    assert_eq!(
        tgit_text(&dir, &["log", "-1", "--format=%an <%ae>"]),
        "Ada Member <ada@example.invalid>"
    );
    assert_eq!(
        tgit_text(&dir, &["log", "-1", "--format=%cn <%ce>"]),
        "HOI4 Factory <factory@hoi4-factory.invalid>"
    );
    assert_eq!(
        tgit_text(&dir, &["log", "-1", "--format=%B"]),
        "Named change\n\nBody line"
    );
    // The unnamed changes are still there, unstaged and untouched.
    assert_eq!(fs::read(dir.join("unrelated.txt")).unwrap(), b"not named\n");
    assert_eq!(
        fs::read(dir.join("common/ideas/a.txt")).unwrap(),
        b"also changed but not named\n"
    );
    assert!(tgit_text(&dir, &["diff", "--cached", "--name-only"]).is_empty());
}

#[test]
fn commit_paths_validates_paths_and_state() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let mut tw = repo.add_task_worktree(&t, &dir, &base).unwrap();

    for bad in ["../outside.txt", "/abs.txt", ".git/config", "a//b", "C:\\x"] {
        let err = tw.commit_paths(&[bad.to_owned()], "m", None).unwrap_err();
        assert!(matches!(err, WorkspaceError::Path(_)), "{bad}: {err}");
    }
    assert!(matches!(
        tw.commit_paths(&[], "m", None),
        Err(WorkspaceError::Git(GitError::NothingToCommit))
    ));
    assert!(
        matches!(
            tw.commit_paths(&["common/lf.txt".into()], "m", None),
            Err(WorkspaceError::Git(GitError::NothingToCommit))
        ),
        "unchanged path"
    );

    // Pre-staged changes would be swept into the commit, so it refuses.
    fs::write(dir.join("staged.txt"), b"s\n").unwrap();
    tgit(&dir, &["add", "staged.txt"]);
    fs::write(dir.join("common/lf.txt"), b"x\n").unwrap();
    assert!(matches!(
        tw.commit_paths(&["common/lf.txt".into()], "m", None),
        Err(WorkspaceError::Git(GitError::UnexpectedStagedChanges))
    ));
    assert_eq!(tw.verify_head().unwrap(), base);
}

#[test]
fn commit_paths_can_record_a_deletion() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let mut tw = repo.add_task_worktree(&t, &dir, &base).unwrap();
    fs::remove_file(dir.join("common/lf.txt")).unwrap();
    let commit = tw
        .commit_paths(&["common/lf.txt".into()], "delete", None)
        .unwrap();
    let status = tgit_text(
        &dir,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-status",
            "-r",
            commit.as_str(),
        ],
    );
    assert_eq!(status, "D\tcommon/lf.txt");
}

#[test]
fn remove_task_worktree_is_not_forced() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let tw = repo.add_task_worktree(&t, &dir, &base).unwrap();
    fs::write(dir.join("precious.txt"), b"uncommitted work\n").unwrap();
    assert!(matches!(
        repo.remove_task_worktree(&tw),
        Err(GitError::Failed { .. })
    ));
    assert_eq!(
        fs::read(dir.join("precious.txt")).unwrap(),
        b"uncommitted work\n"
    );
    fs::remove_file(dir.join("precious.txt")).unwrap();
    repo.remove_task_worktree(&tw).unwrap();
    assert!(!dir.exists());
    // The branch survives so the work stays attributable.
    repo.open_task_branch(&t).unwrap();
}

// ---- baseline commit ----

#[test]
fn baseline_commit_refuses_an_existing_history() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let head = repo.baseline_commit("baseline").unwrap();
    fx.write("later.txt", b"later\n");
    assert!(matches!(
        repo.baseline_commit("again"),
        Err(GitError::HeadAlreadyExists)
    ));
    assert_eq!(repo.head().unwrap(), Some(head));
    assert!(tgit_text(&fx.src(), &["diff", "--cached", "--name-only"]).is_empty());
}

#[test]
fn baseline_of_an_empty_workspace_can_still_be_committed() {
    let fx = Fx::new();
    let repo = init_repo(&fx);
    let baseline = repo.baseline().unwrap();
    assert_eq!(baseline.head, None);
    assert_eq!(baseline.branch.as_deref(), Some("main"));
    let head = repo.baseline_commit("empty baseline").unwrap();
    assert_eq!(repo.baseline().unwrap().head, Some(head));
}

#[test]
fn baseline_records_head_branch_and_index_hash() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let head = repo.baseline_commit("baseline").unwrap();
    let baseline = repo.baseline().unwrap();
    assert_eq!(baseline.head, Some(head));
    assert_eq!(baseline.branch.as_deref(), Some("main"));
    let index_bytes = fs::read(repo.index_path().unwrap()).unwrap();
    assert_eq!(
        baseline.index_sha256.as_deref(),
        Some(common::sha256_hex(&index_bytes).as_str())
    );
}

// ---- repository automation (GIT-10) ----

fn hook_script(marker: &Path) -> String {
    format!(
        "#!/bin/sh\necho ran >> \"{}\"\nexit 0\n",
        marker.to_string_lossy().replace('\\', "/")
    )
}

#[test]
fn imported_hooks_never_run_through_the_wrapper() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    let marker = fx.tmp.path().join("hook-ran.txt");
    let hooks = fx.src().join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    for name in [
        "pre-commit",
        "post-commit",
        "post-checkout",
        "reference-transaction",
        "commit-msg",
        "pre-auto-gc",
    ] {
        let path = hooks.join(name);
        fs::write(&path, hook_script(&marker)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let base = repo.baseline_commit("baseline").unwrap();
    let t = task("t1");
    let dir = fx.ws.task_dir(&t);
    let mut tw = repo.add_task_worktree(&t, &dir, &base).unwrap();
    fs::write(dir.join("common/lf.txt"), b"x\n").unwrap();
    tw.commit_paths(&["common/lf.txt".into()], "c", None)
        .unwrap();
    tw.create_restore_point(&RestoreLabel::new("r").unwrap())
        .unwrap();
    assert!(
        !marker.exists(),
        "a repository hook ran through the wrapper"
    );

    // Control: plain git does run these hooks, so the assertion above is meaningful.
    fs::write(fx.src().join("control.txt"), b"c\n").unwrap();
    tgit(&fx.src(), &["add", "control.txt"]);
    tgit(&fx.src(), &["commit", "-q", "-m", "control"]);
    assert!(
        marker.exists(),
        "control: the planted hooks do not run, the test proves nothing"
    );
}

#[test]
fn local_configuration_that_runs_programs_is_refused_by_open() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    GitRepo::open(&fx.src()).unwrap();
    assert!(fx.ws.git().unwrap().is_some());

    for (key, value, finding) in [
        ("filter.evil.clean", "touch /tmp/pwned", "filter.evil.clean"),
        ("filter.evil.smudge", "cat", "filter.evil.smudge"),
        ("diff.evil.textconv", "cat", "diff.evil.textconv"),
        ("core.sshCommand", "ssh -o x", "core.sshcommand"),
        ("credential.helper", "store", "credential.helper"),
        ("core.worktree", "/", "core.worktree"),
        ("core.pager", "less", "core.pager"),
        ("merge.x.driver", "true", "merge.x.driver"),
        (
            "includeIf.gitdir:/.path",
            "/etc/x",
            "includeif.gitdir:/.path",
        ),
    ] {
        tgit(&fx.src(), &["config", "--local", key, value]);
        let err = GitRepo::open(&fx.src()).unwrap_err();
        match &err {
            GitError::UnsafeConfig(findings) => {
                assert!(findings.iter().any(|f| f == finding), "{key}: {findings:?}");
                assert!(
                    !err.to_string().contains(value),
                    "values must not be echoed: {err}"
                );
            }
            other => panic!("{key}: {other}"),
        }
        assert!(matches!(
            fx.ws.git(),
            Err(WorkspaceError::Git(GitError::UnsafeConfig(_)))
        ));
        tgit(&fx.src(), &["config", "--local", "--unset-all", key]);
        GitRepo::open(&fx.src()).unwrap();
    }
}

#[test]
fn harmless_local_configuration_is_accepted() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    for (key, value) in [
        ("core.autocrlf", "true"),
        ("user.name", "Someone"),
        ("remote.origin.url", "https://example.invalid/x/y.git"),
        ("branch.main.remote", "origin"),
        ("core.filemode", "false"),
    ] {
        tgit(&fx.src(), &["config", "--local", key, value]);
    }
    GitRepo::open(&fx.src()).unwrap();
}

#[test]
fn an_include_is_reported_without_being_followed() {
    // Git parses includes while it discovers a repository. An include naming an existing file
    // that is not configuration (`/etc/passwd` on Linux and macOS) used to fail the repository
    // as unreadable there, while on Windows the same upload reported the key. The relative
    // path resolves next to .git/config, so this runs the same way on every platform.
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    fs::write(
        fx.src().join(".git/not-config"),
        "root:x:0:0:root:/root:/bin/sh\n",
    )
    .unwrap();
    let config = fx.src().join(".git/config");
    let mut body = fs::read_to_string(&config).unwrap();
    body.push_str("[include]\n\tpath = not-config\n");
    fs::write(&config, body).unwrap();
    match GitRepo::open(&fx.src()).unwrap_err() {
        GitError::UnsafeConfig(findings) => {
            assert_eq!(findings, vec!["include.path".to_string()]);
        }
        other => panic!("expected UnsafeConfig, got {other}"),
    }
}

#[test]
fn alternates_and_commondir_redirection_are_refused() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    let info = fx.src().join(".git/objects/info");
    fs::create_dir_all(&info).unwrap();
    fs::write(info.join("alternates"), "/some/other/objects\n").unwrap();
    assert!(matches!(
        GitRepo::open(&fx.src()),
        Err(GitError::UnsafeConfig(_))
    ));
    fs::remove_file(info.join("alternates")).unwrap();
    fs::write(fx.src().join(".git/commondir"), "../elsewhere\n").unwrap();
    assert!(GitRepo::open(&fx.src()).is_err());
}

#[test]
fn open_refuses_a_dot_git_file_a_bogus_dot_git_and_a_parent_repository() {
    let fx = Fx::new();
    // No .git at all.
    assert!(matches!(
        GitRepo::open(&fx.src()),
        Err(GitError::NotARepository)
    ));

    // The data root itself is a repository: the workspace must not pick it up.
    tgit(fx.root.data_root(), &["init", "-q"]);
    assert!(matches!(
        GitRepo::open(&fx.src()),
        Err(GitError::NotARepository)
    ));
    assert!(fx.ws.git().unwrap().is_none());

    // A .git file (gitdir pointer) is not a primary repository.
    let elsewhere = fx.tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    tgit(&elsewhere, &["init", "-q"]);
    fs::write(
        fx.src().join(".git"),
        format!(
            "gitdir: {}\n",
            elsewhere.join(".git").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(matches!(
        GitRepo::open(&fx.src()),
        Err(GitError::NotARepository)
    ));
    fs::remove_file(fx.src().join(".git")).unwrap();

    // An empty .git directory is not a repository either, and discovery must not walk up.
    fs::create_dir(fx.src().join(".git")).unwrap();
    assert!(matches!(
        GitRepo::open(&fx.src()),
        Err(GitError::NotARepository)
    ));
}

// ---- remote link ----

#[test]
fn records_remote_url_and_default_branch_without_network() {
    let fx = Fx::new();
    seed(&fx);
    let repo = init_repo(&fx);
    repo.baseline_commit("baseline").unwrap();
    assert_eq!(repo.remote_link().unwrap(), None);

    let link = repo
        .link_remote("https://git.invalid/owner/mod.git", "trunk")
        .unwrap();
    assert_eq!(link.name, "origin");
    assert_eq!(link.default_branch.as_deref(), Some("trunk"));
    assert_eq!(repo.remote_link().unwrap(), Some(link.clone()));
    // Same link again is a no-op, a different URL is a conflict.
    assert_eq!(
        repo.link_remote("https://git.invalid/owner/mod.git", "trunk")
            .unwrap(),
        link
    );
    assert!(matches!(
        repo.link_remote("https://git.invalid/other/mod.git", "trunk"),
        Err(GitError::RemoteConflict)
    ));
    assert_eq!(repo.remote_link().unwrap(), Some(link));
    assert_eq!(
        tgit_text(&fx.src(), &["config", "--local", "remote.origin.url"]),
        "https://git.invalid/owner/mod.git"
    );
    // Repository still passes the open check.
    GitRepo::open(&fx.src()).unwrap();
}

#[test]
fn rejects_unsafe_remote_urls_and_branch_names() {
    let fx = Fx::new();
    let repo = init_repo(&fx);
    for bad in [
        "http://github.com/o/r.git",
        "git@github.com:o/r.git",
        "ssh://git@github.com/o/r.git",
        "file:///etc",
        "/local/path",
        "https://user:token@github.com/o/r.git",
        "https://token@github.com/o/r.git",
        "https://github.com",
        "https://github.com/",
        "https://github.com/o/r.git?x=1",
        "https://github.com/o/r.git#frag",
        "https://github.com/o r.git",
        "https://github.com/o/r.git\nfoo",
        "https:///o/r",
        "https://-bad.example/o/r",
        "https://exa mple.com/o/r",
        "https://github.com:port/o/r",
        "https://github.com/o\\r",
        "-https://github.com/o/r",
    ] {
        assert!(repo.link_remote(bad, "main").is_err(), "{bad:?} accepted");
    }
    assert!(
        repo.link_remote("https://github.com/o/r.git", "bad branch")
            .is_err()
    );
    assert!(
        repo.link_remote("https://github.com/o/r.git", "--upload-pack=x")
            .is_err()
    );
    assert_eq!(
        repo.remote_link().unwrap(),
        None,
        "a rejected link must leave no trace"
    );
    repo.link_remote("https://github.com:8443/o/r.git", "main")
        .unwrap();
}

#[test]
fn identity_validation() {
    assert!(Identity::new("A B", "a@b.invalid").is_ok());
    for (n, e) in [
        ("", "a@b"),
        ("A", ""),
        ("A<", "a@b"),
        ("A", "a>b"),
        ("A\nB", "a@b"),
        ("  ", "a@b"),
    ] {
        assert!(Identity::new(n, e).is_err(), "{n:?} {e:?}");
    }
}
