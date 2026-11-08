//! The `hoi4-engine` binary bundled with HOI4 Agent Tools (T-ENG-01).
//!
//! Agent Tools 4.8.0 and later ship one engine binary per platform under `engine/bin/` with a
//! manifest of SHA-256 values (`hoi4-engine-bundle/1`). Its own engine client runs a binary only
//! when the manifest lists it with the same hash, and answers `ENGINE_UNAVAILABLE` (or
//! `FOCUS_ENGINE_UNAVAILABLE` for focus layout) otherwise, never a silent fallback. The host applies
//! the same rule here so `/readyz` reports what the tools will find.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const BUNDLE_MANIFEST_SCHEMA: &str = "hoi4-engine-bundle/1";

/// The manifest key of this build's platform, as Node names it (`<platform>-<arch>`).
pub fn platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("windows", "x86_64") => Some("win32-x64"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EngineStatus {
    Available {
        platform: String,
        engine_version: String,
        sha256: String,
    },
    Unavailable {
        /// `platform_unsupported`, `manifest_invalid`, `binary_not_listed`, `binary_not_found` or
        /// `binary_checksum_mismatch`.
        reason: &'static str,
        message: String,
    },
}

impl EngineStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, EngineStatus::Available { .. })
    }
}

/// The package root of an install whose entry point is `<root>/dist/bin/stdio.js`.
pub fn package_root(entry: &Path) -> Option<PathBuf> {
    Some(entry.parent()?.parent()?.parent()?.to_path_buf())
}

fn unavailable(reason: &'static str, message: impl Into<String>) -> EngineStatus {
    EngineStatus::Unavailable {
        reason,
        message: message.into(),
    }
}

/// Checks the bundled binary for this platform against the package's manifest.
pub fn bundled_engine_status(package_root: &Path) -> EngineStatus {
    let Some(key) = platform_key() else {
        return unavailable(
            "platform_unsupported",
            format!(
                "no bundled hoi4-engine for {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        );
    };
    let bin = package_root.join("engine").join("bin");
    let manifest: serde_json::Value = match std::fs::read(bin.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(v) => v,
        None => {
            return unavailable(
                "manifest_invalid",
                "engine/bin/manifest.json is missing or not JSON",
            );
        }
    };
    if manifest["schema"] != BUNDLE_MANIFEST_SCHEMA {
        return unavailable(
            "manifest_invalid",
            "engine/bin/manifest.json has another schema",
        );
    }
    let entry = &manifest["binaries"][key];
    let (Some(file), Some(want)) = (entry["file"].as_str(), entry["sha256"].as_str()) else {
        return unavailable(
            "binary_not_listed",
            format!("engine/bin/manifest.json lists no hoi4-engine for {key}"),
        );
    };
    if file.contains("..") || Path::new(file).is_absolute() {
        return unavailable(
            "manifest_invalid",
            "the manifest names a path outside engine/bin",
        );
    }
    let bytes = match std::fs::read(bin.join(file)) {
        Ok(b) => b,
        Err(_) => {
            return unavailable(
                "binary_not_found",
                format!("the bundled hoi4-engine for {key} is missing"),
            );
        }
    };
    let got: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got != want {
        return unavailable(
            "binary_checksum_mismatch",
            format!("the bundled hoi4-engine for {key} does not match engine/bin/manifest.json"),
        );
    }
    EngineStatus::Available {
        platform: key.to_owned(),
        engine_version: manifest["engineVersion"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        sha256: got,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(root: &Path, bytes: &[u8], listed_sha: Option<&str>) {
        let key = platform_key().expect("supported test platform");
        let dir = root.join("engine/bin").join(key);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hoi4-engine"), bytes).unwrap();
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let manifest = serde_json::json!({
            "schema": BUNDLE_MANIFEST_SCHEMA,
            "engineVersion": "0.1.0",
            "binaries": { key: { "file": format!("{key}/hoi4-engine"), "sha256": listed_sha.unwrap_or(&sha), "size": bytes.len() } }
        });
        std::fs::write(root.join("engine/bin/manifest.json"), manifest.to_string()).unwrap();
    }

    #[test]
    fn a_listed_binary_with_the_same_hash_is_available() {
        let dir = tempfile::tempdir().unwrap();
        bundle(dir.path(), b"engine", None);
        assert!(bundled_engine_status(dir.path()).is_available());
    }

    #[test]
    fn missing_or_changed_binaries_are_unavailable_with_a_reason() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            bundled_engine_status(dir.path()),
            EngineStatus::Unavailable {
                reason: "manifest_invalid",
                ..
            }
        ));
        bundle(dir.path(), b"engine", Some(&"0".repeat(64)));
        assert!(matches!(
            bundled_engine_status(dir.path()),
            EngineStatus::Unavailable {
                reason: "binary_checksum_mismatch",
                ..
            }
        ));
        let key = platform_key().unwrap();
        bundle(dir.path(), b"engine", None);
        std::fs::remove_file(dir.path().join("engine/bin").join(key).join("hoi4-engine")).unwrap();
        assert!(matches!(
            bundled_engine_status(dir.path()),
            EngineStatus::Unavailable {
                reason: "binary_not_found",
                ..
            }
        ));
    }

    #[test]
    fn package_root_is_three_levels_above_the_entry() {
        let root = package_root(Path::new("/opt/hoi4-agent-tools/dist/bin/stdio.js")).unwrap();
        assert_eq!(root, Path::new("/opt/hoi4-agent-tools"));
    }
}
