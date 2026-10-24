//! Integration tests for the game reference reader. They use the synthetic fixture under
//! `tests/fixtures/ref-basic` (invented values, no game content) and temporary copies.

use std::fs;
use std::path::{Path, PathBuf};

use factory_game_ref::{
    Compatibility, GameRefError, GameReference, ReadOnlyEvidence, WarnDirection,
    check_compatibility, check_read_only,
};

const FIXTURE_CHECKSUM_SHA256: &str =
    "3dce3912967ec3cdf180c2534d0c3d269bbff2822b6b6a5a2e804f81db63209a";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ref-basic")
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn minimal(version_json: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "launcher-settings.json", version_json);
    dir
}

#[test]
fn reads_fixture_metadata() {
    let reference = GameReference::read(&fixture(), "/game").unwrap();
    assert_eq!(reference.mount_locator, "/game");
    assert_eq!(reference.version.as_deref(), Some("7.8.9 (ab12)"));
    assert_eq!(reference.raw_version.as_deref(), Some("v7.8.9.0"));
    assert_eq!(reference.numeric_version, Some(vec![7, 8, 9, 0]));
    assert_eq!(reference.rev.as_deref(), Some("ab12"));
    assert_eq!(reference.branch.as_deref(), Some("fixture-branch"));
    assert_eq!(
        reference.checksum_manifest_sha256.as_deref(),
        Some(FIXTURE_CHECKSUM_SHA256)
    );
    assert_eq!(reference.dlc_ids, vec!["dlc001_alpha", "dlc002_beta"]);
    assert!(reference.warnings.is_empty(), "{:?}", reference.warnings);
    assert!(reference.verify_manifest_hash());
    assert_eq!(reference.display_version(), "7.8.9 (ab12)");
}

#[test]
fn manifest_never_contains_the_host_path() {
    let reference = GameReference::read(&fixture(), "/game").unwrap();
    let json = serde_json::to_string(&reference).unwrap();
    let host = fixture().to_string_lossy().into_owned();
    assert!(!json.contains(&host));
    assert!(!json.contains("ref-basic"));
    assert!(json.contains("\"/game\""));
}

#[test]
fn manifest_round_trips_and_detects_tampering() {
    let reference = GameReference::read(&fixture(), "/game").unwrap();
    let json = serde_json::to_string(&reference).unwrap();
    let back: GameReference = serde_json::from_str(&json).unwrap();
    assert_eq!(back, reference);
    assert!(back.verify_manifest_hash());

    let mut tampered = back;
    tampered.dlc_ids.push("dlc999_extra".into());
    assert!(!tampered.verify_manifest_hash());
}

#[test]
fn manifest_hash_changes_with_each_identity_field_but_not_with_the_locator() {
    let base = GameReference::read(&fixture(), "/game").unwrap();
    let relocated = GameReference::read(&fixture(), "/mnt/ref").unwrap();
    assert_eq!(base.manifest_hash, relocated.manifest_hash);

    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), copy.path());
    write(copy.path(), "checksum_manifest.txt", "different\n");
    let changed_checksum = GameReference::read(copy.path(), "/game").unwrap();
    assert_ne!(base.manifest_hash, changed_checksum.manifest_hash);

    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), copy.path());
    fs::create_dir_all(copy.path().join("dlc/dlc003_gamma")).unwrap();
    let more_dlc = GameReference::read(copy.path(), "/game").unwrap();
    assert_ne!(base.manifest_hash, more_dlc.manifest_hash);
    assert_eq!(more_dlc.dlc_ids.len(), 3);

    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), copy.path());
    write(copy.path(), "hoi4_rev.txt", "cd34\n");
    let other_rev = GameReference::read(copy.path(), "/game").unwrap();
    assert_ne!(base.manifest_hash, other_rev.manifest_hash);
}

#[test]
fn dlc_ids_are_sorted_and_only_directories_count() {
    let dir = minimal(r#"{"version":"1.2.3","rawVersion":"1.2.3.0"}"#);
    write(dir.path(), "dlc/zeta/x.txt", "x");
    write(dir.path(), "dlc/alpha/x.txt", "x");
    write(dir.path(), "dlc/middle/x.txt", "x");
    write(dir.path(), "dlc/loose-file.txt", "x");
    write(dir.path(), "dlc/bad name/x.txt", "x");
    let reference = GameReference::read(dir.path(), "/game").unwrap();
    assert_eq!(reference.dlc_ids, vec!["alpha", "middle", "zeta"]);
    assert!(
        reference
            .warnings
            .iter()
            .any(|w| w.contains("unusual name"))
    );
}

#[test]
fn optional_files_missing_become_warnings_not_errors() {
    let dir = minimal(r#"{"rawVersion":"4.5.6.0"}"#);
    let reference = GameReference::read(dir.path(), "/game").unwrap();
    assert_eq!(reference.version, None);
    assert_eq!(reference.rev, None);
    assert_eq!(reference.branch, None);
    assert_eq!(reference.checksum_manifest_sha256, None);
    assert!(reference.dlc_ids.is_empty());
    assert_eq!(reference.warnings.len(), 4, "{:?}", reference.warnings);
    assert_eq!(reference.display_version(), "4.5.6.0");
}

#[test]
fn missing_or_bad_launcher_settings_are_errors() {
    let empty = tempfile::tempdir().unwrap();
    assert!(matches!(
        GameReference::read(empty.path(), "/game"),
        Err(GameRefError::MissingRequired {
            file: "launcher-settings.json"
        })
    ));

    let bad_json = minimal("{not json");
    assert!(matches!(
        GameReference::read(bad_json.path(), "/game"),
        Err(GameRefError::InvalidSettings(_))
    ));

    let no_version = minimal(r#"{"gameId":"hoi4"}"#);
    assert!(matches!(
        GameReference::read(no_version.path(), "/game"),
        Err(GameRefError::MissingVersion)
    ));

    let blank_version = minimal(r#"{"version":"  ","rawVersion":""}"#);
    assert!(matches!(
        GameReference::read(blank_version.path(), "/game"),
        Err(GameRefError::MissingVersion)
    ));

    let file = tempfile::NamedTempFile::new().unwrap();
    assert!(matches!(
        GameReference::read(file.path(), "/game"),
        Err(GameRefError::NotADirectory)
    ));

    let gone = tempfile::tempdir().unwrap();
    let missing = gone.path().join("nope");
    assert!(matches!(
        GameReference::read(&missing, "/game"),
        Err(GameRefError::Io { what: "mount", .. })
    ));
}

#[test]
fn error_text_has_no_host_path_or_input_excerpt() {
    let secret = "SECRET-TOKEN-TEXT";
    let dir = minimal(&format!("{{\"version\": {secret}"));
    let err = GameReference::read(dir.path(), "/game")
        .unwrap_err()
        .to_string();
    assert!(!err.contains(secret));
    assert!(!err.contains(&dir.path().to_string_lossy().into_owned()));
}

#[test]
fn oversize_metadata_is_rejected() {
    let dir = minimal(r#"{"version":"1.2.3"}"#);
    write(dir.path(), "hoi4_rev.txt", &"x".repeat(10_000));
    assert!(matches!(
        GameReference::read(dir.path(), "/game"),
        Err(GameRefError::TooLarge {
            file: "hoi4_rev.txt",
            ..
        })
    ));
}

#[test]
fn non_utf8_rev_file_is_rejected_and_bom_is_stripped() {
    let dir = minimal(r#"{"version":"1.2.3"}"#);
    fs::write(dir.path().join("hoi4_rev.txt"), [0xff, 0xfe, 0xfd]).unwrap();
    assert!(matches!(
        GameReference::read(dir.path(), "/game"),
        Err(GameRefError::NotText {
            file: "hoi4_rev.txt"
        })
    ));
    fs::write(
        dir.path().join("hoi4_rev.txt"),
        b"\xef\xbb\xbfzz99\r\nsecond line\r\n",
    )
    .unwrap();
    let reference = GameReference::read(dir.path(), "/game").unwrap();
    assert_eq!(reference.rev.as_deref(), Some("zz99"));
}

#[test]
fn checksum_hash_covers_exact_bytes_including_crlf() {
    let dir = minimal(r#"{"version":"1.2.3"}"#);
    fs::write(dir.path().join("checksum_manifest.txt"), b"a\r\nb\r\n").unwrap();
    let crlf = GameReference::read(dir.path(), "/game").unwrap();
    fs::write(dir.path().join("checksum_manifest.txt"), b"a\nb\n").unwrap();
    let lf = GameReference::read(dir.path(), "/game").unwrap();
    assert_ne!(crlf.checksum_manifest_sha256, lf.checksum_manifest_sha256);
}

#[cfg(unix)]
#[test]
fn reader_never_opens_other_game_files_or_dlc_contents() {
    use std::os::unix::fs::PermissionsExt;
    // Running as root bypasses mode bits, which makes this test meaningless.
    if is_root() {
        eprintln!("skipped: running as root");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), dir.path());
    write(dir.path(), "common/secret.txt", "never read");
    write(dir.path(), "dlc/dlc001_alpha/content.txt", "never read");
    let locked = [
        dir.path().join("common/secret.txt"),
        dir.path().join("common-not-read.txt"),
        dir.path().join("dlc/dlc001_alpha/content.txt"),
        dir.path().join("dlc/dlc001_alpha/placeholder.txt"),
    ];
    for path in &locked {
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
    }
    fs::set_permissions(dir.path().join("common"), fs::Permissions::from_mode(0o000)).unwrap();
    fs::set_permissions(
        dir.path().join("dlc/dlc001_alpha"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();

    let result = GameReference::read(dir.path(), "/game");

    // Restore so the temp dir can be removed.
    fs::set_permissions(dir.path().join("common"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(
        dir.path().join("dlc/dlc001_alpha"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let reference = result.expect("metadata read must not touch the locked paths");
    assert_eq!(reference.dlc_ids, vec!["dlc001_alpha", "dlc002_beta"]);
}

#[cfg(unix)]
fn is_root() -> bool {
    // Probe by trying to read a mode-000 file.
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("probe");
    fs::write(&file, "x").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read(&file).is_ok();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    readable
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

// ---- compatibility ----

fn reference(version: &str) -> GameReference {
    let dir = minimal(&format!("{{\"rawVersion\":\"{version}\"}}"));
    GameReference::read(dir.path(), "/game").unwrap()
}

#[test]
fn compatibility_match_cases() {
    let r = reference("v7.8.9.0");
    for declared in [
        "7.8.9",
        "7.8.*",
        "7.*",
        "v7.8",
        "\"7.8.9.0\"",
        " 7.8.9 ",
        "*",
        "7",
    ] {
        assert_eq!(
            check_compatibility(&r, Some(declared)),
            Compatibility::Match,
            "{declared}"
        );
    }
}

#[test]
fn compatibility_warns_older_and_newer_with_a_reason() {
    let r = reference("7.8.9.0");
    for (declared, expected) in [
        ("7.7.*", WarnDirection::ModOlder),
        ("6.99", WarnDirection::ModOlder),
        ("7.8.8", WarnDirection::ModOlder),
        ("7.9.*", WarnDirection::ModNewer),
        ("8.0.0", WarnDirection::ModNewer),
        ("7.8.9.1", WarnDirection::ModNewer),
    ] {
        match check_compatibility(&r, Some(declared)) {
            Compatibility::Warn { direction, reason } => {
                assert_eq!(direction, expected, "{declared}");
                assert!(
                    reason.contains(declared) && reason.contains("7.8.9.0"),
                    "{reason}"
                );
            }
            other => panic!("{declared}: expected warn, got {other:?}"),
        }
    }
}

#[test]
fn compatibility_unknown_cases() {
    let r = reference("7.8.9.0");
    for declared in [
        None,
        Some(""),
        Some("   "),
        Some("latest"),
        Some("7.x"),
        Some("7..8"),
        Some("-1"),
    ] {
        assert!(
            matches!(
                check_compatibility(&r, declared),
                Compatibility::Unknown { .. }
            ),
            "{declared:?}"
        );
    }
    let dir = minimal(r#"{"version":"codename only"}"#);
    let unparsed = GameReference::read(dir.path(), "/game").unwrap();
    assert_eq!(unparsed.numeric_version, None);
    assert!(matches!(
        check_compatibility(&unparsed, Some("7.8.9")),
        Compatibility::Unknown { .. }
    ));
}

#[test]
fn compatibility_serializes_with_a_status_tag() {
    let r = reference("7.8.9.0");
    let json = serde_json::to_value(check_compatibility(&r, Some("7.7"))).unwrap();
    assert_eq!(json["status"], "warn");
    assert_eq!(json["direction"], "mod_older");
    let json = serde_json::to_value(check_compatibility(&r, Some("7.8"))).unwrap();
    assert_eq!(json, serde_json::json!({"status": "match"}));
}

#[test]
fn version_falls_back_from_raw_to_display_version() {
    let dir = minimal(r#"{"version":"1.19.3 (zz00)"}"#);
    let r = GameReference::read(dir.path(), "/game").unwrap();
    assert_eq!(r.numeric_version, Some(vec![1, 19, 3]));
}

// ---- read-only check ----

#[test]
fn writable_directory_is_reported_and_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let check = check_read_only(dir.path());
    assert_eq!(
        check.evidence,
        ReadOnlyEvidence::Writable {
            probe_removed: true
        }
    );
    assert!(!check.is_read_only());
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "probe file left behind"
    );
}

#[test]
fn missing_directory_is_inconclusive_not_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let check = check_read_only(&dir.path().join("absent"));
    assert!(
        matches!(check.evidence, ReadOnlyEvidence::Inconclusive { .. }),
        "{check:?}"
    );
    assert!(!check.is_read_only());
}

#[cfg(unix)]
#[test]
fn permission_denied_directory_counts_as_read_only_but_not_as_a_mount() {
    use std::os::unix::fs::PermissionsExt;
    if is_root() {
        eprintln!("skipped: running as root");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let check = check_read_only(dir.path());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(check.evidence, ReadOnlyEvidence::PermissionDenied);
    assert!(check.is_read_only());
    assert!(!check.is_read_only_mount());
}

/// Set FACTORY_RO_MOUNT to a directory on a read-only mount (for example a Docker bind
/// mount with `:ro`) to assert real EROFS reporting.
#[test]
fn real_read_only_mount_reports_read_only_filesystem() {
    let Ok(mount) = std::env::var("FACTORY_RO_MOUNT") else {
        eprintln!("skipped: FACTORY_RO_MOUNT not set");
        return;
    };
    let before = fs::read_dir(&mount).unwrap().count();
    let check = check_read_only(Path::new(&mount));
    assert_eq!(check.evidence, ReadOnlyEvidence::ReadOnlyFilesystem);
    assert!(check.is_read_only_mount());
    assert_eq!(fs::read_dir(&mount).unwrap().count(), before);
}

/// Reads metadata only from the real game folder named by HOI4_GAME_REF. Prints only the
/// manifest. Run with `cargo test -p factory-game-ref -- --ignored`.
#[test]
#[ignore = "needs HOI4_GAME_REF pointing at a game directory; reads metadata only"]
fn real_game_reference_metadata_only() {
    let mount = std::env::var("HOI4_GAME_REF").expect("HOI4_GAME_REF must be set");
    let reference = GameReference::read(Path::new(&mount), "/game").unwrap();
    assert!(reference.numeric_version.is_some());
    assert!(reference.verify_manifest_hash());
    println!("{}", serde_json::to_string_pretty(&reference).unwrap());
    let ro = check_read_only(Path::new(&mount));
    println!("read-only probe: {:?}", ro.evidence);
}
