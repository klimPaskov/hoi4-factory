mod common;

use std::fs;

use factory_workspace::{
    PathError, ResolveMode, fold_key, names_collide, resolve, validate_relative,
};

use common::Fx;

fn rejects(rel: &str) -> PathError {
    match validate_relative(rel) {
        Err(err) => err,
        Ok(parts) => panic!("{rel:?} was accepted as {parts:?}"),
    }
}

#[test]
fn accepts_ordinary_mod_paths() {
    for rel in [
        "descriptor.mod",
        "common/ideas/ger_ideas.txt",
        "localisation/english/my_mod_l_english.yml",
        "gfx/interface/goals/focus_ger_the_rhine.dds",
        "events/Ünïcödé.txt",
        "common/national_focus/a b.txt",
        "a.b.c/d.e",
        ".gitignore",
        ".gitattributes",
        "git/readme.txt",
        "gitx",
    ] {
        validate_relative(rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
    }
}

#[test]
fn rejects_absolute_paths() {
    for rel in [
        "/etc/passwd",
        "\\windows\\system32",
        "C:/Windows",
        "c:\\x",
        "C:",
        "//server/share",
        "\\\\server\\share",
    ] {
        assert!(matches!(rejects(rel), PathError::Absolute), "{rel}");
    }
}

#[test]
fn rejects_traversal_and_empty_components() {
    assert!(matches!(rejects(""), PathError::Empty));
    for rel in ["..", "../x", "a/../b", "a/..", "./a", "a/./b", "."] {
        assert!(matches!(rejects(rel), PathError::Traversal), "{rel}");
    }
    for rel in ["a//b", "a/", "a/b/"] {
        assert!(matches!(rejects(rel), PathError::EmptyComponent), "{rel}");
    }
}

#[test]
fn rejects_windows_hazards_and_hidden_characters() {
    for rel in [
        "a\\b",
        "dir/a:b",
        "file.txt:stream",
        "a<b",
        "a>b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        "a\u{0}b",
        "a\nb",
        "a\u{7f}b",
        "a\u{202e}txt.exe",
        "a\u{200b}b",
        "\u{feff}name",
        "x\u{2066}y",
    ] {
        assert!(
            matches!(rejects(rel), PathError::ForbiddenChar(_)),
            "{rel:?}"
        );
    }
    for rel in ["name.", "name ", "dir./x", "dir /x"] {
        assert!(
            matches!(rejects(rel), PathError::TrailingDotOrSpace(_)),
            "{rel:?}"
        );
    }
    for rel in [
        "con",
        "CON",
        "nul.txt",
        "Aux.tar.gz",
        "dir/COM1",
        "lpt9.log",
        "prn",
    ] {
        assert!(
            matches!(rejects(rel), PathError::ReservedName(_)),
            "{rel:?}"
        );
    }
}

#[test]
fn rejects_git_metadata_in_every_spelling_that_aliases_it() {
    for rel in [
        ".git",
        ".GIT",
        ".Git/config",
        "a/.git/hooks/pre-commit",
        "git~1",
        "GIT~1/x",
        "\u{ff0e}git",
    ] {
        let err = rejects(rel);
        assert!(matches!(err, PathError::GitMetadata(_)), "{rel:?}: {err}");
    }
}

#[test]
fn rejects_overlong_names() {
    assert!(matches!(rejects(&"a".repeat(256)), PathError::TooLong));
    assert!(matches!(
        rejects(&format!("{}/x", "a/".repeat(1100))),
        PathError::TooLong
    ));
    validate_relative(&"a".repeat(255)).unwrap();
}

#[test]
fn unicode_folding_detects_aliases_without_rewriting() {
    let nfc = "\u{e9}.txt"; // é precomposed
    let nfd = "e\u{301}.txt"; // e + combining acute
    assert_ne!(nfc, nfd);
    assert!(names_collide(nfc, nfd));
    assert!(names_collide("Foo.txt", "foo.txt"));
    assert!(
        names_collide("\u{ff21}.txt", "a.txt"),
        "fullwidth A folds to a"
    );
    assert!(!names_collide("same.txt", "same.txt"));
    assert!(!names_collide("a.txt", "b.txt"));
    assert_eq!(fold_key(nfc), fold_key(nfd));
}

#[test]
fn resolve_returns_the_requested_bytes_for_existing_and_new_paths() {
    let fx = Fx::new();
    fx.write("common/ideas/a.txt", b"x");
    let existing = resolve(&fx.src(), "common/ideas/a.txt", ResolveMode::MustExist).unwrap();
    assert_eq!(
        existing,
        fx.src().join("common").join("ideas").join("a.txt")
    );
    let new_path = resolve(&fx.src(), "common/new/deep/b.txt", ResolveMode::MayCreate).unwrap();
    assert_eq!(
        new_path,
        fx.src()
            .join("common")
            .join("new")
            .join("deep")
            .join("b.txt")
    );
    assert!(!new_path.exists(), "resolve must not create anything");
    assert!(matches!(
        resolve(&fx.src(), "common/new/b.txt", ResolveMode::MustExist),
        Err(PathError::NotFound)
    ));
}

#[test]
fn resolve_refuses_names_that_collide_with_existing_siblings() {
    let fx = Fx::new();
    fx.write("events/Foo.txt", b"x");
    fx.write("events/\u{e9}.txt", b"x"); // NFC name on disk
    let case = resolve(&fx.src(), "events/foo.txt", ResolveMode::MayCreate);
    assert!(
        matches!(case, Err(PathError::NameCollision { .. })),
        "{case:?}"
    );
    let nfd = resolve(&fx.src(), "events/e\u{301}.txt", ResolveMode::MayCreate);
    assert!(
        matches!(nfd, Err(PathError::NameCollision { .. })),
        "{nfd:?}"
    );
    let dir_case = resolve(&fx.src(), "EVENTS/other.txt", ResolveMode::MayCreate);
    assert!(
        matches!(dir_case, Err(PathError::NameCollision { .. })),
        "{dir_case:?}"
    );
    // The exact spelling still resolves.
    resolve(&fx.src(), "events/Foo.txt", ResolveMode::MustExist).unwrap();
    resolve(&fx.src(), "events/\u{e9}.txt", ResolveMode::MustExist).unwrap();
}

#[test]
fn resolve_refuses_a_file_used_as_a_directory() {
    let fx = Fx::new();
    fx.write("a.txt", b"x");
    assert!(matches!(
        resolve(&fx.src(), "a.txt/b", ResolveMode::MayCreate),
        Err(PathError::NotADirectory)
    ));
}

#[test]
fn resolve_requires_an_existing_root() {
    let fx = Fx::new();
    let gone = fx.tmp.path().join("nope");
    assert!(matches!(
        resolve(&gone, "a", ResolveMode::MayCreate),
        Err(PathError::NotFound)
    ));
}

#[cfg(unix)]
#[test]
fn symlinks_are_refused_in_every_position() {
    use std::os::unix::fs::symlink;
    let fx = Fx::new();
    let outside = fx.tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();
    fx.write("real/file.txt", b"x");

    // Directory link escaping the root.
    symlink(&outside, fx.src().join("escape")).unwrap();
    // File link escaping the root.
    symlink(outside.join("secret.txt"), fx.src().join("secret-link.txt")).unwrap();
    // Link that stays inside the root is refused too.
    symlink(fx.src().join("real"), fx.src().join("inside")).unwrap();
    // Dangling link.
    symlink(fx.tmp.path().join("missing"), fx.src().join("dangling")).unwrap();

    for rel in [
        "escape",
        "escape/secret.txt",
        "escape/new.txt",
        "secret-link.txt",
        "inside",
        "inside/file.txt",
        "dangling",
    ] {
        for mode in [ResolveMode::MustExist, ResolveMode::MayCreate] {
            let result = resolve(&fx.src(), rel, mode);
            assert!(
                matches!(result, Err(PathError::Symlink(_))),
                "{rel} {mode:?}: {result:?}"
            );
        }
    }
    resolve(&fx.src(), "real/file.txt", ResolveMode::MustExist).unwrap();
    assert_eq!(
        fs::read_to_string(outside.join("secret.txt")).unwrap(),
        "secret"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_parent_swapped_in_after_creation_is_caught() {
    use std::os::unix::fs::symlink;
    let fx = Fx::new();
    let outside = fx.tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fx.write("common/x.txt", b"x");
    resolve(&fx.src(), "common/x.txt", ResolveMode::MustExist).unwrap();
    fs::remove_dir_all(fx.src().join("common")).unwrap();
    symlink(&outside, fx.src().join("common")).unwrap();
    assert!(matches!(
        resolve(&fx.src(), "common/x.txt", ResolveMode::MayCreate),
        Err(PathError::Symlink(_))
    ));
}

#[cfg(windows)]
#[test]
fn junctions_are_refused_as_reparse_points() {
    let fx = Fx::new();
    let outside = fx.tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "secret").unwrap();
    let link = fx.src().join("junction");
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    for rel in ["junction", "junction/secret.txt", "junction/new.txt"] {
        let result = resolve(&fx.src(), rel, ResolveMode::MayCreate);
        assert!(
            matches!(
                result,
                Err(PathError::ReparsePoint(_)) | Err(PathError::Symlink(_))
            ),
            "{rel}: {result:?}"
        );
    }
}
