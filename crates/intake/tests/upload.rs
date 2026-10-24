//! Upload intake: happy paths, byte preservation, every refusal, Git handling.

mod common;

use std::fs;

use common::*;
use factory_intake::{
    DeclaredKind, EntryData, FileTypePolicy, Finding, FolderEntry, GitInitRequest, GitOutcome,
    IntakeLimits, UploadKind, UploadMeta, UploadOptions,
};
use factory_workspace::Identity;
use uuid::Uuid;

fn consent() -> UploadOptions {
    UploadOptions {
        init_git: Some(GitInitRequest::with_consent("main")),
    }
}

fn limits(f: impl FnOnce(&mut IntakeLimits)) -> IntakeLimits {
    let mut limits = IntakeLimits::default();
    f(&mut limits);
    limits
}

fn text(name: &str) -> ZEntry {
    ZEntry::file(name, b"x = 1\n".to_vec())
}

// ---------------------------------------------------------------------------------------------
// Happy path

#[test]
fn zip_upload_preserves_bytes_modes_and_the_original() {
    let env = Env::new();
    let crlf = b"focus = {\r\n\tid = X\r\n}\r\n".to_vec();
    let bom = [BOM, b"name=\"Test\"\r\n"].concat();
    let binary = binary_blob();
    let archive = build_zip(&[
        ZEntry::dir("common/"),
        ZEntry::file("descriptor.mod", bom.clone()),
        ZEntry::file("common/focus.txt", crlf.clone()),
        ZEntry::file("gfx/art.dds", binary.clone()).stored(),
        ZEntry::file("tool.txt", b"run\n".to_vec()).exec(),
        ZEntry::dir("empty/"),
    ]);

    let outcome = env.zip_ok(&archive);

    let source = env.source();
    assert_eq!(fs::read(source.join("descriptor.mod")).unwrap(), bom);
    assert_eq!(fs::read(source.join("common/focus.txt")).unwrap(), crlf);
    assert_eq!(fs::read(source.join("gfx/art.dds")).unwrap(), binary);
    assert!(source.join("empty").is_dir());
    assert!(!source.join(".git").exists(), "no Git without consent");
    assert_eq!(outcome.kind, UploadKind::Zip);
    assert_eq!(outcome.git, GitOutcome::NotInitialised);
    assert_eq!(outcome.extracted.files, 4);
    assert!(outcome.findings.contains(&Finding::ExecutableMode {
        path: "tool.txt".into()
    }));

    // The original is kept byte for byte, outside the workspace, under a generated name.
    let stored = env.intake.original_path(&env.target(), outcome.upload_id);
    assert_eq!(fs::read(&stored).unwrap(), archive);
    assert_eq!(outcome.original.sha256, sha256_hex(&archive));
    assert_eq!(outcome.original.bytes, archive.len() as u64);
    assert!(stored.starts_with(env.storage.data_root().join("uploads")));
    assert!(!stored.starts_with(env.storage.data_root().join("workspaces")));
    assert!(stored.to_string_lossy().ends_with(".zip.upload"));
    assert!(
        !outcome
            .original
            .locator
            .contains(&*env.storage.data_root().to_string_lossy())
    );
    assert_eq!(outcome.source_locator, "workspace:proj-0001/source");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &str| fs::metadata(source.join(p)).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode("tool.txt"), 0o755);
        assert_eq!(mode("descriptor.mod"), 0o644);
        let original_mode = fs::metadata(&stored).unwrap().permissions().mode() & 0o7777;
        assert_eq!(
            original_mode & 0o111,
            0,
            "the stored original is never executable"
        );
        assert_eq!(
            original_mode & 0o007,
            0,
            "the stored original is not world readable"
        );
    }
}

#[test]
fn zip64_end_records_are_read() {
    let env = Env::new();
    let archive = build_zip_with(&[text("a.txt"), text("dir/b.txt")], true, None);
    let outcome = env.zip_ok(&archive);
    assert_eq!(outcome.extracted.files, 2);
}

#[test]
fn every_allowed_type_is_accepted() {
    let env = Env::new();
    let names = [
        "a.dds",
        "a.tga",
        "a.png",
        "a.jpg",
        "a.ogg",
        "a.wav",
        "descriptor.mod",
        "a.txt",
        "a.yml",
        "a.csv",
        "a.gui",
        "a.gfx",
        "a.lua",
        "a.shader",
        "a.fxh",
        "a.json",
        "a.md",
        "LICENSE",
        ".gitignore",
        "UPPER.TXT",
    ];
    let entries: Vec<ZEntry> = names.iter().map(|n| text(n)).collect();
    let outcome = env.zip_ok(&build_zip(&entries));
    assert_eq!(outcome.extracted.files, names.len() as u64);
}

#[test]
fn text_that_merely_starts_with_mz_is_accepted() {
    let env = Env::new();
    let mut body = b"MZ Division order of battle\n".to_vec();
    body.resize(0x200, b' ');
    env.zip_ok(&build_zip(&[ZEntry::file("units.txt", body)]));
}

#[test]
fn lfs_pointer_and_submodules_are_reported() {
    let env = Env::new();
    let pointer = b"version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 12\n".to_vec();
    let outcome = env.zip_ok(&build_zip(&[
        ZEntry::file("gfx/big.dds", pointer),
        ZEntry::file(".gitmodules", b"[submodule \"x\"]\n\tpath = x\n".to_vec()),
    ]));
    assert!(outcome.findings.contains(&Finding::LfsPointer {
        path: "gfx/big.dds".into()
    }));
    assert!(outcome.findings.contains(&Finding::Submodules {
        path: ".gitmodules".into()
    }));
}

// ---------------------------------------------------------------------------------------------
// Git

#[test]
fn consent_is_required_before_anything_is_written() {
    let env = Env::new();
    let options = UploadOptions {
        init_git: Some(GitInitRequest {
            consent: false,
            initial_branch: "main".into(),
            identity: Identity::factory(),
        }),
    };
    let result = env.zip(&build_zip(&[text("a.txt")]), &options);
    env.assert_refused(result, "git_consent_required");
    assert!(!env.storage.data_root().join("uploads").exists());
}

#[test]
fn an_invalid_initial_branch_is_refused_up_front() {
    let env = Env::new();
    let options = UploadOptions {
        init_git: Some(GitInitRequest::with_consent("../evil")),
    };
    let result = env.zip(&build_zip(&[text("a.txt")]), &options);
    env.assert_refused(result, "invalid_request");
}

#[test]
fn git_init_with_consent_commits_the_exact_bytes_whatever_gitattributes_says() {
    let env = Env::new();
    let crlf = b"a = 1\r\nb = 2\r\n".to_vec();
    let bom = [BOM, b"x\r\n"].concat();
    let binary = binary_blob();
    let attributes =
        b"* text=auto eol=lf\n*.txt filter=lfs\n# comment\n*.dds -text\n*.csv crlf\n".to_vec();
    let archive = build_zip(&[
        ZEntry::file(".gitattributes", attributes),
        ZEntry::file("common/crlf.txt", crlf.clone()),
        ZEntry::file("bom.gui", bom.clone()),
        ZEntry::file("art.dds", binary.clone()),
    ]);

    let outcome = env.zip(&archive, &consent()).unwrap();

    let GitOutcome::Initialised {
        head,
        branch,
        ignored_files,
    } = &outcome.git
    else {
        panic!("expected an initialised repository, got {:?}", outcome.git);
    };
    assert_eq!(branch, "main");
    assert_eq!(*ignored_files, 0);
    let source = env.source();
    assert_eq!(git_text(&source, &["rev-parse", "HEAD"]), head.to_string());
    assert_eq!(git_text(&source, &["rev-list", "--count", "HEAD"]), "1");
    // The commit holds the uploaded bytes, not Git's idea of them.
    assert_eq!(blob(&source, "HEAD", "common/crlf.txt"), crlf);
    assert_eq!(blob(&source, "HEAD", "bom.gui"), bom);
    assert_eq!(blob(&source, "HEAD", "art.dds"), binary);
    assert_eq!(git_text(&source, &["status", "--porcelain"]), "");
    let message = git_text(&source, &["log", "-1", "--format=%B"]);
    assert!(
        message.contains(&outcome.original.sha256),
        "commit records the upload hash"
    );
    assert_eq!(
        git_text(&source, &["config", "--local", "core.autocrlf"]),
        "false"
    );

    // Every conversion line is flagged, with file and line.
    let flagged: Vec<(u32, String)> = outcome
        .findings
        .iter()
        .filter_map(|f| match f {
            Finding::GitattributesConversion {
                path,
                line,
                attribute,
            } if path == ".gitattributes" => Some((*line, attribute.clone())),
            _ => None,
        })
        .collect();
    for expected in [
        (1, "text=auto"),
        (1, "eol=lf"),
        (2, "filter=lfs"),
        (5, "crlf"),
    ] {
        assert!(
            flagged.contains(&(expected.0, expected.1.to_owned())),
            "missing {expected:?} in {flagged:?}"
        );
    }
    assert!(
        !flagged.iter().any(|(line, _)| *line == 4),
        "`-text` converts nothing"
    );
}

#[test]
fn gitignore_rules_are_reported_not_hidden() {
    let env = Env::new();
    let archive = build_zip(&[
        ZEntry::file(".gitignore", b"*.dds\n".to_vec()),
        ZEntry::file("keep.txt", b"k\n".to_vec()),
        ZEntry::file("art.dds", binary_blob()),
    ]);
    let outcome = env.zip(&archive, &consent()).unwrap();
    assert!(
        outcome
            .findings
            .contains(&Finding::IgnoredByGitignore { files: 1 })
    );
    assert!(
        env.source().join("art.dds").exists(),
        "the file is still in the working tree"
    );
    assert_eq!(git_text(&env.source(), &["ls-files"]).lines().count(), 2);
}

/// A real repository with two commits, a modified file and an untracked file.
fn repository_fixture() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write_file(&work, "descriptor.mod", b"name=\"x\"\r\n");
    write_file(&work, "common/a.txt", b"a\r\n");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    write_file(&work, "common/b.txt", b"b\n");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "two"]);
    let head = git_text(&work, &["rev-parse", "HEAD"]);
    write_file(&work, "common/a.txt", b"a edited\r\n");
    write_file(&work, "new.txt", b"untracked\n");
    (tmp, work, head)
}

#[test]
fn an_uploaded_repository_keeps_its_history_after_the_safety_checks() {
    let (_tmp, work, head) = repository_fixture();
    let env = Env::new();
    let archive = zip_directory(&work);

    // Consent to init is irrelevant: a repository exists, so nothing is initialised.
    let outcome = env.zip(&archive, &consent()).unwrap();

    let source = env.source();
    match &outcome.git {
        GitOutcome::ExistingRepository {
            head: found,
            branch,
            unstaged,
            untracked,
            staged,
        } => {
            assert_eq!(found.as_ref().unwrap().to_string(), head);
            assert_eq!(branch.as_deref(), Some("main"));
            assert_eq!((*staged, *unstaged, *untracked), (0, 1, 1));
        }
        other => panic!("expected an existing repository, got {other:?}"),
    }
    assert_eq!(
        git_text(&source, &["rev-list", "--count", "HEAD"]),
        "2",
        "history is intact"
    );
    assert_eq!(
        git_text(&source, &["rev-parse", "HEAD"]),
        head,
        "no new commit"
    );
    assert_eq!(
        fs::read(source.join("common/a.txt")).unwrap(),
        b"a edited\r\n"
    );
    assert!(
        !source.join(".git/hooks").exists(),
        "hook scripts are not extracted"
    );
    assert!(
        outcome
            .findings
            .iter()
            .any(|f| matches!(f, Finding::DroppedGitHooks { files } if *files > 0))
    );
}

#[test]
fn an_uploaded_repository_with_automation_in_its_config_is_refused() {
    for (extra, key) in [
        (
            "[filter \"evil\"]\n\tclean = echo pwned\n",
            "filter.evil.clean",
        ),
        ("[core]\n\tsshCommand = echo pwned\n", "core.sshcommand"),
        ("[credential]\n\thelper = echo pwned\n", "credential.helper"),
        ("[diff]\n\texternal = echo pwned\n", "diff.external"),
        ("[include]\n\tpath = /etc/passwd\n", "include.path"),
        ("[merge \"m\"]\n\tdriver = echo pwned\n", "merge.m.driver"),
    ] {
        let (_tmp, work, _) = repository_fixture();
        let config = work.join(".git/config");
        let mut body = fs::read_to_string(&config).unwrap();
        body.push_str(extra);
        fs::write(&config, body).unwrap();
        let env = Env::new();
        let result = env.zip(&zip_directory(&work), &consent());
        let failure = result.as_ref().expect_err("unsafe config must be refused");
        assert_eq!(failure.code(), "git_unsafe_config", "{key}");
        let message = failure.error.to_string();
        assert!(message.contains(key), "{message} should name {key}");
        assert!(!message.contains("pwned"), "values are never echoed");
        env.assert_refused(result, "git_unsafe_config");
    }
}

#[test]
fn alternates_in_an_uploaded_repository_are_refused() {
    let (_tmp, work, _) = repository_fixture();
    let info = work.join(".git/objects/info");
    fs::create_dir_all(&info).unwrap();
    fs::write(info.join("alternates"), "/somewhere/else/objects\n").unwrap();
    let env = Env::new();
    let result = env.zip(&zip_directory(&work), &consent());
    env.assert_refused(result, "git_unsafe_config");
}

#[test]
fn a_git_directory_that_is_not_a_repository_is_refused() {
    let env = Env::new();
    let result = env.zip(
        &build_zip(&[
            ZEntry::file(".git/notes", b"nothing\n".to_vec()),
            text("a.txt"),
        ]),
        &consent(),
    );
    env.assert_refused(result, "git_not_repository");
}

#[test]
fn git_pointer_files_and_nested_repositories_are_refused() {
    let env = Env::new();
    let result = env.zip(
        &build_zip(&[ZEntry::file(".git", b"gitdir: /elsewhere\n".to_vec())]),
        &consent(),
    );
    env.assert_refused(result, "git_pointer_file");

    let env = Env::new();
    let result = env.zip(
        &build_zip(&[ZEntry::file("sub/.git/config", b"[core]\n".to_vec())]),
        &consent(),
    );
    env.assert_refused(result, "nested_git_directory");

    let env = Env::new();
    let result = env.zip(
        &build_zip(&[ZEntry::file("sub/.GIT/config", b"[core]\n".to_vec())]),
        &consent(),
    );
    env.assert_refused(result, "path_rejected");
}

// ---------------------------------------------------------------------------------------------
// Names

#[test]
fn hostile_paths_are_refused_before_any_workspace_write() {
    let cases = [
        "../evil.txt",
        "a/../../evil.txt",
        "a/../b.txt",
        "./a.txt",
        "a/./b.txt",
        "a//b.txt",
        "/abs.txt",
        "\\abs.txt",
        "C:/evil.txt",
        "C:evil.txt",
        "c:\\evil.txt",
        "a\\b.txt",
        "a/b\\c.txt",
        "con.txt",
        "aux/x.txt",
        "a.txt:stream",
        "trailing.",
        "dir/trailing space ",
        "bidi\u{202e}txt.exe",
        "ctl\u{1}.txt",
        "..",
        "a/..",
        "star*.txt",
    ];
    for name in cases {
        let env = Env::new();
        let result = env.zip(
            &build_zip(&[ZEntry::file(name, b"x\n".to_vec()).stored()]),
            &UploadOptions::default(),
        );
        env.assert_refused(result, "path_rejected");
    }
}

#[test]
fn names_that_are_not_utf8_are_refused() {
    let env = Env::new();
    let mut entry = text("x.txt");
    entry.name = vec![b'a', 0xff, 0xfe, b'.', b't', b'x', b't'];
    let result = env.zip(&build_zip(&[entry]), &UploadOptions::default());
    env.assert_refused(result, "path_rejected");
}

#[test]
fn duplicate_and_colliding_names_are_refused() {
    let cases: Vec<(Vec<ZEntry>, &str)> = vec![
        (vec![text("a.txt"), text("a.txt")], "duplicate_entry"),
        (
            vec![text("c/Common.txt"), text("c/common.txt")],
            "name_collision",
        ),
        (
            vec![text("caf\u{e9}.txt"), text("cafe\u{301}.txt")],
            "name_collision",
        ),
        (
            vec![text("\u{ff21}\u{ff22}.txt"), text("ab.txt")],
            "name_collision",
        ),
        (vec![text("Dir/a.txt"), text("dir/b.txt")], "name_collision"),
        (
            vec![ZEntry::dir("Dir/"), text("dir/b.txt")],
            "name_collision",
        ),
        (vec![text("a.txt"), text("a.txt/b.txt")], "path_conflict"),
        (vec![ZEntry::dir("x.txt/"), text("x.txt")], "path_conflict"),
    ];
    for (entries, code) in cases {
        let env = Env::new();
        let result = env.zip(&build_zip(&entries), &UploadOptions::default());
        env.assert_refused(result, code);
    }
}

#[test]
fn symbolic_links_devices_and_pipes_are_refused() {
    let env = Env::new();
    let result = env.zip(
        &build_zip(&[
            text("fine.txt"),
            ZEntry::symlink("link.txt", "../../etc/passwd"),
        ]),
        &UploadOptions::default(),
    );
    env.assert_refused(result, "symlink_entry");

    // The creating system field says DOS, the mode bits say symlink: still refused.
    let env = Env::new();
    let mut entry = ZEntry::symlink("link.txt", "x");
    entry.host = 0;
    let result = env.zip(&build_zip(&[entry]), &UploadOptions::default());
    env.assert_refused(result, "symlink_entry");

    for mode in [0o060644, 0o020644, 0o010644, 0o140644] {
        let env = Env::new();
        let result = env.zip(
            &build_zip(&[ZEntry::special("dev.txt", mode)]),
            &UploadOptions::default(),
        );
        env.assert_refused(result, "device_entry");
    }
}

// ---------------------------------------------------------------------------------------------
// File types

#[test]
fn executables_and_scripts_are_refused_by_name() {
    for ext in [
        "exe", "dll", "bat", "cmd", "ps1", "sh", "so", "dylib", "jar", "msi", "scr", "com",
    ] {
        for name in [
            format!("bin/payload.{ext}"),
            format!("bin/PAYLOAD.{}", ext.to_uppercase()),
        ] {
            let env = Env::new();
            let result = env.zip(
                &build_zip(&[text("ok.txt"), text(&name)]),
                &UploadOptions::default(),
            );
            env.assert_refused(result, "executable_refused");
        }
    }
}

#[test]
fn unknown_types_are_refused() {
    for name in [
        "readme.xyz",
        "noextension",
        "a.zip",
        "page.html",
        "script.js",
        "x.py",
        ".env",
    ] {
        let env = Env::new();
        let result = env.zip(&build_zip(&[text(name)]), &UploadOptions::default());
        env.assert_refused(result, "file_type_not_allowed");
    }
}

#[test]
fn executables_hiding_behind_an_allowed_extension_are_refused_by_content() {
    let mut pe = vec![0u8; 0x200];
    pe[0] = b'M';
    pe[1] = b'Z';
    pe[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    pe[0x80..0x84].copy_from_slice(b"PE\0\0");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("notes.txt", b"\x7fELF\x02\x01\x01".to_vec()),
        ("data.json", pe),
        ("run.lua", b"#!/bin/sh\necho hi\n".to_vec()),
        ("mac.yml", vec![0xcf, 0xfa, 0xed, 0xfe, 0, 0]),
        ("fat.md", vec![0xca, 0xfe, 0xba, 0xbe, 0, 0]),
    ];
    for (name, bytes) in cases {
        let env = Env::new();
        let result = env.zip(
            &build_zip(&[ZEntry::file(name, bytes)]),
            &UploadOptions::default(),
        );
        env.assert_refused(result, "executable_refused");
    }
}

#[test]
fn the_policy_is_configurable() {
    let mut policy = FileTypePolicy::default();
    policy.allowed_extensions.insert("html".into());
    let env = Env::with(IntakeLimits::default(), policy);
    env.zip_ok(&build_zip(&[text("doc.html")]));
}

// ---------------------------------------------------------------------------------------------
// Limits and archive structure

#[test]
fn limits_are_enforced() {
    // (limits, entries, code)
    let cases: Vec<(IntakeLimits, Vec<ZEntry>, &str)> = vec![
        (
            limits(|l| l.max_files = 3),
            (0..4).map(|i| text(&format!("f{i}.txt"))).collect(),
            "too_many_files",
        ),
        (
            limits(|l| l.max_entries = 3),
            (0..4).map(|i| text(&format!("f{i}.txt"))).collect(),
            "too_many_entries",
        ),
        (
            limits(|l| l.max_file_bytes = 10),
            vec![ZEntry::file("big.txt", vec![b'a'; 11])],
            "file_too_large",
        ),
        (
            limits(|l| l.max_total_bytes = 20),
            vec![
                ZEntry::file("a.txt", vec![b'a'; 15]),
                ZEntry::file("b.txt", vec![b'b'; 15]),
            ],
            "total_too_large",
        ),
        (
            limits(|l| l.max_path_depth = 3),
            vec![text("a/b/c/d.txt")],
            "path_too_deep",
        ),
        (
            limits(|l| l.max_path_bytes = 20),
            vec![text("a/very/long/directory/name.txt")],
            "path_too_long",
        ),
    ];
    for (limits, entries, code) in cases {
        let env = Env::with_limits(limits);
        let result = env.zip(&build_zip(&entries), &UploadOptions::default());
        env.assert_refused(result, code);
    }
}

#[test]
fn an_oversized_upload_stops_receiving() {
    let env = Env::with_limits(limits(|l| l.max_upload_bytes = 100));
    let archive = build_zip(&[ZEntry::file("a.txt", vec![b'z'; 4096]).stored()]);
    let result = env.zip(&archive, &UploadOptions::default());
    env.assert_refused(result, "upload_too_large");
}

#[test]
fn a_zip_bomb_is_refused_by_ratio() {
    // 20 MiB of zeros deflates to about 20 KiB: a ratio of roughly 1000.
    let env = Env::with_limits(limits(|l| {
        l.max_file_bytes = 100 << 20;
        l.max_total_bytes = 100 << 20;
    }));
    let result = env.zip(
        &build_zip(&[ZEntry::file("bomb.txt", vec![0u8; 20 << 20])]),
        &UploadOptions::default(),
    );
    env.assert_refused(result, "compression_ratio_exceeded");
}

#[test]
fn a_zip_bomb_is_refused_by_size_before_extraction() {
    let env = Env::with_limits(limits(|l| l.max_file_bytes = 1 << 20));
    let result = env.zip(
        &build_zip(&[ZEntry::file("bomb.txt", vec![0u8; 8 << 20])]),
        &UploadOptions::default(),
    );
    env.assert_refused(result, "file_too_large");
}

#[test]
fn a_header_that_understates_its_size_is_caught_while_streaming() {
    let env = Env::new();
    let data = vec![0u8; 5 << 20];
    let mut entry = ZEntry::file("liar.txt", data.clone());
    entry.declared_uncompressed = Some(10);
    entry.declared_crc = Some(crc32fast::hash(&data));
    let result = env.zip(&build_zip(&[entry]), &UploadOptions::default());
    env.assert_refused(result, "size_mismatch");
}

#[test]
fn a_header_that_overstates_its_size_or_lies_about_the_checksum_is_refused() {
    let env = Env::new();
    let mut entry = text("short.txt");
    entry.declared_uncompressed = Some(100);
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "size_mismatch",
    );

    let env = Env::new();
    let mut entry = text("crc.txt");
    entry.declared_crc = Some(0xdead_beef);
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "size_mismatch",
    );
}

#[test]
fn a_central_directory_that_claims_millions_of_entries_is_refused_without_reading_it() {
    let env = Env::new();
    let archive = build_zip_with(&[text("a.txt")], false, Some(60_000));
    env.assert_refused(
        env.zip(&archive, &UploadOptions::default()),
        "too_many_entries",
    );
    let env = Env::new();
    let archive = build_zip_with(&[text("a.txt")], true, Some(100_000_000));
    env.assert_refused(
        env.zip(&archive, &UploadOptions::default()),
        "too_many_entries",
    );
    // Fewer entries than claimed, within the limit: the directory size does not add up.
    let env = Env::new();
    let archive = build_zip_with(&[text("a.txt")], false, Some(5_000));
    env.assert_refused(
        env.zip(&archive, &UploadOptions::default()),
        "archive_malformed",
    );
}

#[test]
fn encrypted_and_unsupported_entries_are_refused() {
    let env = Env::new();
    let mut entry = text("secret.txt");
    entry.flags = 1;
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "encrypted_entry",
    );

    let env = Env::new();
    let mut entry = text("aes.txt").stored();
    entry.method = 99;
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "encrypted_entry",
    );

    let env = Env::new();
    let mut entry = text("bzip.txt").stored();
    entry.method = 12;
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "unsupported_compression",
    );
}

#[test]
fn a_stored_entry_with_different_sizes_is_malformed() {
    let env = Env::new();
    let mut entry = text("odd.txt").stored();
    entry.raw_compressed = Some(b"abcdefghij".to_vec());
    env.assert_refused(
        env.zip(&build_zip(&[entry]), &UploadOptions::default()),
        "archive_malformed",
    );
}

#[test]
fn things_that_are_not_zip_archives_are_refused() {
    let good = build_zip(&[text("a.txt")]);
    let mut truncated = good.clone();
    truncated.truncate(good.len() - 10);
    let mut trailing = good.clone();
    trailing.extend_from_slice(b"trailing bytes");
    let mut prefixed = b"PREFIXED!!".to_vec();
    prefixed.extend_from_slice(&good);
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (Vec::new(), "not_a_zip_archive"),
        (
            b"hello world, this is not a zip file at all".to_vec(),
            "not_a_zip_archive",
        ),
        (truncated, "not_a_zip_archive"),
        (trailing, "not_a_zip_archive"),
        (prefixed, "archive_malformed"),
    ];
    for (bytes, code) in cases {
        let env = Env::new();
        env.assert_refused(env.zip(&bytes, &UploadOptions::default()), code);
    }
}

fn with_comment(mut zip: Vec<u8>, comment: &[u8]) -> Vec<u8> {
    let at = zip.len() - 2;
    zip[at..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    zip.extend_from_slice(comment);
    zip
}

fn fake_eocd(entries: u16) -> Vec<u8> {
    let mut fake = b"PK\x05\x06".to_vec();
    fake.extend_from_slice(&[0, 0, 0, 0]);
    fake.extend_from_slice(&entries.to_le_bytes());
    fake.extend_from_slice(&entries.to_le_bytes());
    fake.extend_from_slice(&[0u8; 8]);
    fake.extend_from_slice(&[0, 0]);
    fake
}

#[test]
fn an_end_record_hidden_in_the_comment_cannot_redirect_the_parser() {
    // A fake record inside the comment whose own comment length does not reach the end of
    // the file is ignored: the real record is used.
    let mut comment = fake_eocd(99);
    comment.extend_from_slice(b"padding");
    let archive = with_comment(build_zip(&[text("a.txt")]), &comment);
    let env = Env::new();
    assert_eq!(env.zip_ok(&archive).extracted.files, 1);

    // A fake record that does reach the end wins the search. It describes an impossible
    // directory, so the upload is refused rather than misread.
    let archive = with_comment(build_zip(&[text("a.txt")]), &fake_eocd(99));
    let env = Env::new();
    env.assert_refused(
        env.zip(&archive, &UploadOptions::default()),
        "archive_malformed",
    );
}

#[test]
fn a_reused_upload_id_is_refused_and_leaves_the_first_upload_alone() {
    let env = Env::new();
    let meta = UploadMeta {
        upload_id: Uuid::new_v4(),
        original_name: None,
    };
    let archive = build_zip(&[text("a.txt")]);
    let mut reader = std::io::Cursor::new(archive.clone());
    env.intake
        .ingest_zip(&env.target(), &meta, &mut reader, &UploadOptions::default())
        .unwrap();
    let mut again = std::io::Cursor::new(archive.clone());
    let failure = env
        .intake
        .ingest_zip(&env.target(), &meta, &mut again, &UploadOptions::default())
        .unwrap_err();
    assert_eq!(failure.code(), "invalid_request");
    assert_eq!(
        fs::read(env.intake.original_path(&env.target(), meta.upload_id)).unwrap(),
        archive
    );
    assert!(env.source().join("a.txt").exists());
}

#[test]
fn an_existing_workspace_is_never_touched() {
    let env = Env::new();
    let workspace = env
        .storage
        .create_workspace(&env.account, &env.project)
        .unwrap();
    write_file(&workspace.source_dir(), "keep.txt", b"mine\r\n");

    let failure = env
        .zip(&build_zip(&[text("a.txt")]), &UploadOptions::default())
        .unwrap_err();
    assert_eq!(failure.code(), "workspace_exists");
    assert_eq!(
        fs::read(env.source().join("keep.txt")).unwrap(),
        b"mine\r\n"
    );
    assert!(!env.source().join("a.txt").exists());
    assert!(
        env.stored_originals().is_empty(),
        "the refused original was removed"
    );

    let failure = env
        .intake
        .ingest_folder(
            &env.target(),
            &env.meta(),
            vec![FolderEntry::file("a.txt", b"x\n".to_vec())],
            &UploadOptions::default(),
        )
        .unwrap_err();
    assert_eq!(failure.code(), "workspace_exists");
    assert_eq!(
        fs::read(env.source().join("keep.txt")).unwrap(),
        b"mine\r\n"
    );
    assert!(env.stored_originals().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Folder uploads

fn folder(
    env: &Env,
    entries: Vec<FolderEntry>,
    options: &UploadOptions,
) -> Result<factory_intake::UploadOutcome, factory_intake::UploadFailure> {
    env.intake
        .ingest_folder(&env.target(), &env.meta(), entries, options)
}

#[test]
fn folder_upload_preserves_bytes_and_keeps_the_original_as_received() {
    let env = Env::new();
    let crlf = b"x = {\r\n}\r\n".to_vec();
    let bom = [BOM, b"y\r\n"].concat();
    let binary = binary_blob();
    let spool = env.tmp.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    fs::write(spool.join("part-1"), &binary).unwrap();
    let mut with_mode = FolderEntry::file("tool.txt", b"run\n".to_vec());
    with_mode.mode = Some(0o755);
    let entries = vec![
        FolderEntry::file("descriptor.mod", bom.clone()),
        FolderEntry::file("common/focus.txt", crlf.clone()),
        FolderEntry {
            path: "gfx/art.dds".into(),
            data: EntryData::Path(spool.join("part-1")),
            mode: None,
            kind: DeclaredKind::File,
        },
        with_mode,
        FolderEntry {
            path: "empty/".into(),
            data: EntryData::Bytes(Vec::new()),
            mode: None,
            kind: DeclaredKind::Directory,
        },
    ];
    let outcome = folder(&env, entries, &UploadOptions::default()).unwrap();

    assert_eq!(outcome.kind, UploadKind::Folder);
    let source = env.source();
    assert_eq!(fs::read(source.join("descriptor.mod")).unwrap(), bom);
    assert_eq!(fs::read(source.join("common/focus.txt")).unwrap(), crlf);
    assert_eq!(fs::read(source.join("gfx/art.dds")).unwrap(), binary);
    assert!(source.join("empty").is_dir());

    // The received files are kept under the uploads area, with an aggregate hash.
    let retained = env.uploads_dir().join(outcome.upload_id.to_string());
    assert_eq!(fs::read(retained.join("common/focus.txt")).unwrap(), crlf);
    assert_eq!(fs::read(retained.join("gfx/art.dds")).unwrap(), binary);
    let mut lines = [
        format!("{} {} descriptor.mod\n", sha256_hex(&bom), bom.len()),
        format!("{} {} common/focus.txt\n", sha256_hex(&crlf), crlf.len()),
        format!("{} {} gfx/art.dds\n", sha256_hex(&binary), binary.len()),
        format!("{} {} tool.txt\n", sha256_hex(b"run\n"), 4),
    ];
    lines.sort_by_key(|l| l.splitn(3, ' ').nth(2).unwrap().to_owned());
    assert_eq!(
        outcome.original.sha256,
        sha256_hex(lines.concat().as_bytes())
    );
    assert!(outcome.findings.contains(&Finding::ExecutableMode {
        path: "tool.txt".into()
    }));
}

#[test]
fn folder_upload_with_consent_commits_the_bytes() {
    let env = Env::new();
    let crlf = b"a\r\nb\r\n".to_vec();
    let outcome = folder(
        &env,
        vec![
            FolderEntry::file(".gitattributes", b"* text eol=lf\n".to_vec()),
            FolderEntry::file("a.txt", crlf.clone()),
        ],
        &consent(),
    )
    .unwrap();
    assert!(matches!(outcome.git, GitOutcome::Initialised { .. }));
    assert_eq!(blob(&env.source(), "HEAD", "a.txt"), crlf);
    assert!(
        outcome
            .findings
            .iter()
            .any(|f| matches!(f, Finding::GitattributesConversion { .. }))
    );
}

#[test]
fn folder_upload_refusals() {
    let cases: Vec<(Vec<FolderEntry>, &str)> = vec![
        (
            vec![FolderEntry::file("../evil.txt", b"x".to_vec())],
            "path_rejected",
        ),
        (
            vec![FolderEntry::file("/abs.txt", b"x".to_vec())],
            "path_rejected",
        ),
        (
            vec![FolderEntry::file("a\\b.txt", b"x".to_vec())],
            "path_rejected",
        ),
        (
            vec![
                FolderEntry::file("a.txt", b"x".to_vec()),
                FolderEntry::file("a.txt", b"y".to_vec()),
            ],
            "duplicate_entry",
        ),
        (
            vec![
                FolderEntry::file("A.txt", b"x".to_vec()),
                FolderEntry::file("a.txt", b"y".to_vec()),
            ],
            "name_collision",
        ),
        (
            vec![FolderEntry::file("payload.exe", b"x".to_vec())],
            "executable_refused",
        ),
        (
            vec![FolderEntry::file("a.txt", b"\x7fELF".to_vec())],
            "executable_refused",
        ),
        (
            vec![FolderEntry::file("x.xyz", b"x".to_vec())],
            "file_type_not_allowed",
        ),
        (
            vec![FolderEntry::file("sub/.git/config", b"x".to_vec())],
            "nested_git_directory",
        ),
    ];
    for (entries, code) in cases {
        let env = Env::new();
        env.assert_refused(folder(&env, entries, &UploadOptions::default()), code);
    }
    for (kind, code) in [
        (DeclaredKind::Symlink, "symlink_entry"),
        (DeclaredKind::Hardlink, "hardlink_entry"),
        (DeclaredKind::Device, "device_entry"),
    ] {
        let env = Env::new();
        let mut entry = FolderEntry::file("thing.txt", b"x".to_vec());
        entry.kind = kind;
        env.assert_refused(folder(&env, vec![entry], &UploadOptions::default()), code);
    }
}

#[test]
fn folder_limits_and_consent() {
    let env = Env::with_limits(limits(|l| l.max_upload_bytes = 10));
    let result = folder(
        &env,
        vec![FolderEntry::file("a.txt", vec![b'x'; 11])],
        &UploadOptions::default(),
    );
    env.assert_refused(result, "upload_too_large");

    let env = Env::with_limits(limits(|l| l.max_files = 2));
    let entries = (0..3)
        .map(|i| FolderEntry::file(format!("f{i}.txt"), b"x".to_vec()))
        .collect();
    env.assert_refused(
        folder(&env, entries, &UploadOptions::default()),
        "too_many_files",
    );

    let env = Env::new();
    let options = UploadOptions {
        init_git: Some(GitInitRequest {
            consent: false,
            initial_branch: "main".into(),
            identity: Identity::factory(),
        }),
    };
    env.assert_refused(
        folder(
            &env,
            vec![FolderEntry::file("a.txt", b"x".to_vec())],
            &options,
        ),
        "git_consent_required",
    );
}

#[test]
fn spooled_files_that_are_not_plain_files_are_refused() {
    let env = Env::new();
    let spool = env.tmp.path().join("spool");
    fs::create_dir_all(spool.join("a-directory")).unwrap();
    let entry = FolderEntry {
        path: "a.txt".into(),
        data: EntryData::Path(spool.join("a-directory")),
        mode: None,
        kind: DeclaredKind::File,
    };
    env.assert_refused(
        folder(&env, vec![entry], &UploadOptions::default()),
        "device_entry",
    );
}

#[cfg(unix)]
#[test]
fn spooled_symlinks_and_hardlinks_are_refused() {
    let env = Env::new();
    let spool = env.tmp.path().join("spool");
    fs::create_dir_all(&spool).unwrap();
    fs::write(spool.join("target"), b"secret").unwrap();
    std::os::unix::fs::symlink(spool.join("target"), spool.join("link")).unwrap();
    let entry = FolderEntry {
        path: "a.txt".into(),
        data: EntryData::Path(spool.join("link")),
        mode: None,
        kind: DeclaredKind::File,
    };
    env.assert_refused(
        folder(&env, vec![entry], &UploadOptions::default()),
        "symlink_entry",
    );

    let env = Env::new();
    fs::hard_link(spool.join("target"), spool.join("second-name")).unwrap();
    let entry = FolderEntry {
        path: "a.txt".into(),
        data: EntryData::Path(spool.join("second-name")),
        mode: None,
        kind: DeclaredKind::File,
    };
    env.assert_refused(
        folder(&env, vec![entry], &UploadOptions::default()),
        "hardlink_entry",
    );
}
