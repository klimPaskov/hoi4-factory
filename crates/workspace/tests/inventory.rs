mod common;

use std::fs;

use factory_workspace::{
    EntryKind, FileState, GitRepo, InitOptions, SnapshotInventory, sha256_file,
};

use common::{Fx, bom_crlf, sha256_hex, tgit, tgit_text};

/// Repository with committed files covering every class, in the fixture's source dir.
/// Returns the repo handle.
fn dirty_repo(fx: &Fx) -> GitRepo {
    fx.write(
        "descriptor.mod",
        &bom_crlf(&["name=\"Fixture\"", "version=\"1.0\""]),
    );
    fx.write("common/clean.txt", b"clean = yes\n");
    fx.write("common/modified.txt", &bom_crlf(&["a = 1"]));
    fx.write("common/staged.txt", b"s = 1\n");
    fx.write("common/staged_then_modified.txt", b"t = 1\n");
    fx.write("common/deleted.txt", b"gone\n");
    fx.write(".gitignore", b"*.log\nbuild/\n");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    repo.baseline_commit("baseline").unwrap();

    // Now make the working tree dirty in every way.
    fx.write("common/modified.txt", &bom_crlf(&["a = 2", "b = 3"]));
    fx.write("common/staged.txt", b"s = 2\n");
    tgit(&fx.src(), &["add", "common/staged.txt"]);
    fx.write("common/staged_then_modified.txt", b"t = 2\n");
    tgit(&fx.src(), &["add", "common/staged_then_modified.txt"]);
    fx.write("common/staged_then_modified.txt", b"t = 3\n");
    fx.write("common/new_staged.txt", b"n\n");
    tgit(&fx.src(), &["add", "common/new_staged.txt"]);
    fx.write("common/untracked.txt", b"u\n");
    fx.write("debug.log", b"ignored\n");
    fx.write("build/out.bin", b"\x00\x01\r\n\x02");
    fs::remove_file(fx.src().join("common/deleted.txt")).unwrap();
    repo
}

fn state_of(inv: &SnapshotInventory, path: &str) -> FileState {
    inv.get(path)
        .unwrap_or_else(|| panic!("{path} missing"))
        .state
}

#[test]
fn classifies_every_file_through_git() {
    let fx = Fx::new();
    let repo = dirty_repo(&fx);
    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();

    assert_eq!(state_of(&inv, "descriptor.mod"), FileState::TrackedClean);
    assert_eq!(state_of(&inv, "common/clean.txt"), FileState::TrackedClean);
    assert_eq!(state_of(&inv, ".gitignore"), FileState::TrackedClean);
    assert_eq!(
        state_of(&inv, "common/modified.txt"),
        FileState::TrackedModified
    );
    assert_eq!(state_of(&inv, "common/staged.txt"), FileState::Staged);
    assert!(!inv.get("common/staged.txt").unwrap().also_modified);
    assert_eq!(
        state_of(&inv, "common/staged_then_modified.txt"),
        FileState::Staged
    );
    assert!(
        inv.get("common/staged_then_modified.txt")
            .unwrap()
            .also_modified
    );
    assert_eq!(state_of(&inv, "common/new_staged.txt"), FileState::Staged);
    assert_eq!(state_of(&inv, "common/untracked.txt"), FileState::Untracked);
    assert_eq!(state_of(&inv, "debug.log"), FileState::Ignored);
    assert_eq!(state_of(&inv, "build/out.bin"), FileState::Ignored);
    assert_eq!(inv.missing_tracked, vec!["common/deleted.txt"]);
    assert!(inv.get("common/deleted.txt").is_none());
    assert!(inv.git_index_sha256.is_some());
    assert!(
        inv.entries
            .iter()
            .all(|e| !e.path.starts_with(".git/") && e.path != ".git")
    );
}

#[test]
fn hashes_and_sizes_are_the_exact_bytes_including_bom_and_crlf() {
    let fx = Fx::new();
    let repo = dirty_repo(&fx);
    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    for path in [
        "descriptor.mod",
        "common/modified.txt",
        "build/out.bin",
        "common/clean.txt",
    ] {
        let bytes = fx.read(path);
        let entry = inv.get(path).unwrap();
        assert_eq!(entry.size, bytes.len() as u64, "{path}");
        assert_eq!(
            entry.sha256.as_deref(),
            Some(sha256_hex(&bytes).as_str()),
            "{path}"
        );
        assert_eq!(entry.kind, EntryKind::File);
    }
    let desc = fx.read("descriptor.mod");
    assert!(desc.starts_with(b"\xEF\xBB\xBF") && desc.windows(2).any(|w| w == b"\r\n"));
}

#[test]
fn sha256_file_matches_a_published_vector() {
    let fx = Fx::new();
    fx.write("abc.txt", b"abc");
    assert_eq!(
        sha256_file(&fx.src().join("abc.txt")).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    fx.write("empty.txt", b"");
    assert_eq!(
        sha256_file(&fx.src().join("empty.txt")).unwrap(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn order_is_deterministic_and_bytewise() {
    let fx = Fx::new();
    for p in [
        "z.txt",
        "B.txt",
        "a/b/c.txt",
        "a/B.txt",
        "a.txt",
        "\u{e9}.txt",
        "10.txt",
        "2.txt",
        "a-b.txt",
        "a/z.txt",
    ] {
        fx.write(p, p.as_bytes());
    }
    let one = SnapshotInventory::capture(&fx.src(), None).unwrap();
    let two = SnapshotInventory::capture(&fx.src(), None).unwrap();
    assert_eq!(one, two);
    assert_eq!(one.content_hash(), two.content_hash());
    let paths: Vec<&str> = one.entries.iter().map(|e| e.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    assert_eq!(paths, sorted);
    assert!(one.entries.iter().all(|e| e.state == FileState::Unmanaged));
    assert!(one.git_index_sha256.is_none());
}

#[test]
fn serializes_to_json_and_back_unchanged() {
    let fx = Fx::new();
    let repo = dirty_repo(&fx);
    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    let json = serde_json::to_string(&inv).unwrap();
    let back: SnapshotInventory = serde_json::from_str(&json).unwrap();
    assert_eq!(back, inv);
    assert!(json.contains("\"tracked_modified\""));
    assert!(json.contains("\"untracked\""));
    assert!(!json.contains(fx.tmp.path().to_str().unwrap()));
}

#[test]
fn capture_does_not_change_the_index_or_any_file() {
    let fx = Fx::new();
    let repo = dirty_repo(&fx);
    // Plain `git status` may refresh the index, so settle it before taking the baseline.
    let status_before = tgit_text(&fx.src(), &["status", "--porcelain=v2", "--ignored"]);
    let index_before = fs::read(repo.index_path().unwrap()).unwrap();
    let files_before = common::snapshot_bytes(&fx.src());

    for _ in 0..3 {
        SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
        repo.dirty_state().unwrap();
        repo.baseline().unwrap();
    }

    assert_eq!(
        fs::read(repo.index_path().unwrap()).unwrap(),
        index_before,
        "index bytes changed"
    );
    assert_eq!(common::snapshot_bytes(&fx.src()), files_before);
    assert_eq!(
        tgit_text(&fx.src(), &["status", "--porcelain=v2", "--ignored"]),
        status_before
    );
}

#[test]
fn compare_reports_added_removed_changed_and_mode_changed() {
    let fx = Fx::new();
    fx.write("keep.txt", b"keep");
    fx.write("edit.txt", b"v1");
    fx.write("remove.txt", b"r");
    fx.write("exec.sh", b"#!/bin/sh\n");
    fx.write("same-size.txt", b"aaaa");
    let before = SnapshotInventory::capture(&fx.src(), None).unwrap();

    fx.write("edit.txt", b"v2 longer");
    fx.write("same-size.txt", b"bbbb");
    fs::remove_file(fx.src().join("remove.txt")).unwrap();
    fx.write("added/new.txt", b"n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(fx.src().join("exec.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let after = SnapshotInventory::capture(&fx.src(), None).unwrap();

    let diff = before.diff(&after);
    assert_eq!(diff.added, vec!["added/new.txt"]);
    assert_eq!(diff.removed, vec!["remove.txt"]);
    assert_eq!(diff.changed, vec!["edit.txt", "same-size.txt"]);
    #[cfg(unix)]
    assert_eq!(diff.mode_changed, vec!["exec.sh"]);
    #[cfg(not(unix))]
    assert!(diff.mode_changed.is_empty());
    assert!(!diff.index_changed);
    assert!(!diff.is_empty());
    assert_ne!(before.content_hash(), after.content_hash());

    let reverse = after.diff(&before);
    assert_eq!(reverse.added, vec!["remove.txt"]);
    assert_eq!(reverse.removed, vec!["added/new.txt"]);

    assert!(before.diff(&before).is_empty());
}

#[test]
fn compare_notices_an_index_only_change_and_a_staging_change() {
    let fx = Fx::new();
    fx.write("a.txt", b"a");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    repo.baseline_commit("base").unwrap();
    fx.write("b.txt", b"b");
    let before = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert_eq!(state_of(&before, "b.txt"), FileState::Untracked);

    tgit(&fx.src(), &["add", "b.txt"]);
    let after = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert_eq!(state_of(&after, "b.txt"), FileState::Staged);
    let diff = before.diff(&after);
    assert!(
        diff.index_changed,
        "staging b.txt must change the recorded index hash"
    );
    assert!(diff.added.is_empty() && diff.removed.is_empty() && diff.changed.is_empty());
    assert_eq!(
        before.content_hash(),
        after.content_hash(),
        "bytes did not change"
    );
}

#[test]
fn byte_flip_with_equal_size_is_detected() {
    let fx = Fx::new();
    fx.write("x.txt", b"line\r\n");
    let before = SnapshotInventory::capture(&fx.src(), None).unwrap();
    fx.write("x.txt", b"line\n\n");
    let after = SnapshotInventory::capture(&fx.src(), None).unwrap();
    assert_eq!(before.diff(&after).changed, vec!["x.txt"]);
}

#[test]
fn nested_repositories_are_recorded_not_descended() {
    let fx = Fx::new();
    fx.write("main.txt", b"m");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    repo.baseline_commit("base").unwrap();
    let nested = fx.src().join("vendor/lib");
    fs::create_dir_all(&nested).unwrap();
    tgit(&nested, &["init", "-q"]);
    fs::write(nested.join("inner.txt"), "inner").unwrap();
    fx.write("vendor/readme.txt", b"outer file");

    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert_eq!(inv.nested_repositories, vec!["vendor/lib"]);
    assert!(inv.get("vendor/lib/inner.txt").is_none());
    assert!(inv.entries.iter().all(|e| !e.path.contains("/.git")));
    assert_eq!(state_of(&inv, "vendor/readme.txt"), FileState::Untracked);
}

#[test]
fn unborn_repository_lists_files_as_untracked() {
    let fx = Fx::new();
    fx.write("a.txt", b"a");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert_eq!(state_of(&inv, "a.txt"), FileState::Untracked);
    assert!(inv.missing_tracked.is_empty());
}

#[test]
fn file_names_with_spaces_and_unicode_are_reported_exactly() {
    let fx = Fx::new();
    fx.write("common/my file.txt", b"1");
    fx.write("common/\u{c4}\u{e4}.txt", b"2");
    let repo = GitRepo::init(&fx.src(), InitOptions::with_consent("main")).unwrap();
    repo.baseline_commit("base").unwrap();
    fx.write("common/my file.txt", b"changed");
    let inv = SnapshotInventory::capture(&fx.src(), Some(&repo)).unwrap();
    assert_eq!(
        state_of(&inv, "common/my file.txt"),
        FileState::TrackedModified
    );
    assert_eq!(
        state_of(&inv, "common/\u{c4}\u{e4}.txt"),
        FileState::TrackedClean
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_listed_not_followed() {
    use std::os::unix::fs::symlink;
    let fx = Fx::new();
    let outside = fx.tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();
    symlink(&outside, fx.src().join("linkdir")).unwrap();
    symlink("target-name", fx.src().join("linkfile")).unwrap();
    let inv = SnapshotInventory::capture(&fx.src(), None).unwrap();
    assert_eq!(inv.get("linkdir").unwrap().kind, EntryKind::Symlink);
    assert_eq!(inv.get("linkfile").unwrap().kind, EntryKind::Symlink);
    assert_eq!(
        inv.get("linkfile").unwrap().sha256.as_deref(),
        Some(sha256_hex(b"target-name").as_str())
    );
    assert!(inv.get("linkdir/secret.txt").is_none());
}

#[cfg(unix)]
#[test]
fn executable_bit_is_recorded_as_mode() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new();
    fx.write("run.sh", b"#!/bin/sh\n");
    fs::set_permissions(fx.src().join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    fx.write("plain.txt", b"x");
    fs::set_permissions(
        fx.src().join("plain.txt"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    let inv = SnapshotInventory::capture(&fx.src(), None).unwrap();
    assert_eq!(inv.get("run.sh").unwrap().mode, 0o755);
    assert_eq!(inv.get("plain.txt").unwrap().mode, 0o640);
}
