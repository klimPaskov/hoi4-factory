use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compat::parse_numeric_version;
use crate::hex;

/// Version of the serialized [`GameReference`] shape.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Locator used when the caller has no other name for the mount. It is the in-container
/// path of the read-only mount, a logical name that the browser may see.
pub const DEFAULT_MOUNT_LOCATOR: &str = "/game";

const LAUNCHER_SETTINGS: &str = "launcher-settings.json";
const REV_FILE: &str = "hoi4_rev.txt";
const BRANCH_FILE: &str = "hoi4_branch.txt";
const CHECKSUM_FILE: &str = "checksum_manifest.txt";
const DLC_DIR: &str = "dlc";

const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;
const MAX_SMALL_TEXT_BYTES: u64 = 4096;
const MAX_DLC_ENTRIES: usize = 1024;
const MAX_DLC_NAME_BYTES: usize = 128;

/// Errors from reading the reference metadata. Messages never contain host paths.
#[derive(Debug, thiserror::Error)]
pub enum GameRefError {
    /// A required metadata file is absent, so the mount is not a usable reference.
    #[error("required game metadata file {file} is missing")]
    MissingRequired { file: &'static str },
    /// A metadata file exceeds the size bound for that file.
    #[error("game metadata file {file} is larger than {limit} bytes")]
    TooLarge { file: &'static str, limit: u64 },
    /// `launcher-settings.json` is not valid JSON or has the wrong shape.
    #[error("launcher-settings.json is not valid: {0}")]
    InvalidSettings(String),
    /// `launcher-settings.json` has neither a usable `version` nor `rawVersion`.
    #[error("launcher-settings.json carries no version field")]
    MissingVersion,
    /// A text metadata file is not valid UTF-8.
    #[error("game metadata file {file} is not UTF-8 text")]
    NotText { file: &'static str },
    /// The mount path is not a directory.
    #[error("game reference mount is not a directory")]
    NotADirectory,
    /// Underlying I/O failure while reading one of the metadata paths.
    #[error("io error reading {what}: {source}")]
    Io {
        what: &'static str,
        source: io::Error,
    },
}

/// Identity manifest of the server game reference (SPEC-006 "Identity").
///
/// Every evidence record names `manifest_hash`. A different manifest hash makes older
/// diagnostics and previews stale (T-GAME-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameReference {
    pub schema_version: u32,
    /// Logical locator of the mount. Never a host path.
    pub mount_locator: String,
    /// `version` field of `launcher-settings.json`, verbatim.
    pub version: Option<String>,
    /// `rawVersion` field of `launcher-settings.json`, verbatim.
    pub raw_version: Option<String>,
    /// Numeric components parsed from `rawVersion`, else from `version`.
    pub numeric_version: Option<Vec<u32>>,
    /// First line of `hoi4_rev.txt`.
    pub rev: Option<String>,
    /// First line of `hoi4_branch.txt`.
    pub branch: Option<String>,
    /// SHA-256 of the bytes of `checksum_manifest.txt`.
    pub checksum_manifest_sha256: Option<String>,
    /// Sorted names of the folders directly under `dlc/`.
    pub dlc_ids: Vec<String>,
    /// Absent optional metadata and ignored entries. Not an error.
    pub warnings: Vec<String>,
    /// SHA-256 over the canonical identity fields above (not over `warnings`).
    pub manifest_hash: String,
}

#[derive(Deserialize)]
struct LauncherSettings {
    version: Option<String>,
    #[serde(rename = "rawVersion")]
    raw_version: Option<String>,
}

impl GameReference {
    /// Read the metadata of the game directory at `mount`. `mount_locator` is the logical
    /// name stored in the manifest in place of the host path.
    pub fn read(mount: &Path, mount_locator: &str) -> Result<Self, GameRefError> {
        match fs::metadata(mount) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(GameRefError::NotADirectory),
            Err(source) => {
                return Err(GameRefError::Io {
                    what: "mount",
                    source,
                });
            }
        }
        let mut warnings = Vec::new();

        let settings_bytes = read_bounded(mount, LAUNCHER_SETTINGS, MAX_SETTINGS_BYTES)?.ok_or(
            GameRefError::MissingRequired {
                file: LAUNCHER_SETTINGS,
            },
        )?;
        let settings: LauncherSettings = serde_json::from_slice(&settings_bytes)
            .map_err(|err| GameRefError::InvalidSettings(describe_json_error(&err)))?;
        let version = settings
            .version
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        let raw_version = settings
            .raw_version
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if version.is_none() && raw_version.is_none() {
            return Err(GameRefError::MissingVersion);
        }
        let numeric_version = raw_version
            .as_deref()
            .and_then(parse_numeric_version)
            .or_else(|| version.as_deref().and_then(parse_numeric_version));
        if numeric_version.is_none() {
            warnings.push("no numeric version could be parsed from launcher-settings.json".into());
        }

        let rev = first_line(mount, REV_FILE, &mut warnings)?;
        let branch = first_line(mount, BRANCH_FILE, &mut warnings)?;
        let checksum_manifest_sha256 = hash_file(mount, CHECKSUM_FILE, &mut warnings)?;
        let dlc_ids = list_dlc(mount, &mut warnings)?;

        let mut reference = GameReference {
            schema_version: MANIFEST_SCHEMA_VERSION,
            mount_locator: mount_locator.to_owned(),
            version,
            raw_version,
            numeric_version,
            rev,
            branch,
            checksum_manifest_sha256,
            dlc_ids,
            warnings,
            manifest_hash: String::new(),
        };
        reference.manifest_hash = reference.compute_manifest_hash();
        Ok(reference)
    }

    /// Recompute the identity hash from the identity fields. Use it to verify a
    /// deserialized manifest.
    pub fn compute_manifest_hash(&self) -> String {
        let mut canonical = String::new();
        let mut field = |key: &str, value: &str| {
            canonical.push_str(key);
            canonical.push('=');
            canonical.push_str(&value.replace('\n', "\\n"));
            canonical.push('\n');
        };
        field("schema", &self.schema_version.to_string());
        field("version", self.version.as_deref().unwrap_or("<none>"));
        field(
            "raw_version",
            self.raw_version.as_deref().unwrap_or("<none>"),
        );
        field("rev", self.rev.as_deref().unwrap_or("<none>"));
        field("branch", self.branch.as_deref().unwrap_or("<none>"));
        field(
            "checksum_manifest",
            self.checksum_manifest_sha256.as_deref().unwrap_or("<none>"),
        );
        field("dlc", &self.dlc_ids.join(","));
        hex(&Sha256::digest(canonical.as_bytes()))
    }

    /// True when the stored hash matches the identity fields.
    pub fn verify_manifest_hash(&self) -> bool {
        self.manifest_hash == self.compute_manifest_hash()
    }

    /// Short display string for the browser, for example `7.8.9 (ab12)`.
    pub fn display_version(&self) -> String {
        self.version
            .clone()
            .or_else(|| self.raw_version.clone())
            .unwrap_or_else(|| "unknown".into())
    }
}

/// Serde error text without any excerpt of the input.
fn describe_json_error(err: &serde_json::Error) -> String {
    format!(
        "{:?} at line {} column {}",
        err.classify(),
        err.line(),
        err.column()
    )
}

/// Open one fixed metadata file and read at most `limit` bytes. `None` when absent.
fn read_bounded(
    mount: &Path,
    file: &'static str,
    limit: u64,
) -> Result<Option<Vec<u8>>, GameRefError> {
    let handle = match File::open(mount.join(file)) {
        Ok(handle) => handle,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(GameRefError::Io { what: file, source }),
    };
    let mut bytes = Vec::new();
    handle
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| GameRefError::Io { what: file, source })?;
    if bytes.len() as u64 > limit {
        return Err(GameRefError::TooLarge { file, limit });
    }
    Ok(Some(bytes))
}

fn first_line(
    mount: &Path,
    file: &'static str,
    warnings: &mut Vec<String>,
) -> Result<Option<String>, GameRefError> {
    let Some(bytes) = read_bounded(mount, file, MAX_SMALL_TEXT_BYTES)? else {
        warnings.push(format!("{file} is absent"));
        return Ok(None);
    };
    let text = String::from_utf8(bytes).map_err(|_| GameRefError::NotText { file })?;
    let text = text.trim_start_matches('\u{feff}');
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        warnings.push(format!("{file} is empty"));
        return Ok(None);
    }
    Ok(Some(line.to_owned()))
}

fn hash_file(
    mount: &Path,
    file: &'static str,
    warnings: &mut Vec<String>,
) -> Result<Option<String>, GameRefError> {
    let mut handle = match File::open(mount.join(file)) {
        Ok(handle) => handle,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            warnings.push(format!("{file} is absent"));
            return Ok(None);
        }
        Err(source) => return Err(GameRefError::Io { what: file, source }),
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = handle
            .read(&mut buffer)
            .map_err(|source| GameRefError::Io { what: file, source })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(hex(&hasher.finalize())))
}

/// Names of the directories directly under `dlc/`. Only names are read. Entries that are
/// not plain directories or whose names are unusual are skipped and noted.
fn list_dlc(mount: &Path, warnings: &mut Vec<String>) -> Result<Vec<String>, GameRefError> {
    let entries = match fs::read_dir(mount.join(DLC_DIR)) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            warnings.push("dlc directory is absent".into());
            return Ok(Vec::new());
        }
        Err(source) => {
            return Err(GameRefError::Io {
                what: DLC_DIR,
                source,
            });
        }
    };
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| GameRefError::Io {
            what: DLC_DIR,
            source,
        })?;
        let file_type = entry.file_type().map_err(|source| GameRefError::Io {
            what: DLC_DIR,
            source,
        })?;
        if !file_type.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            warnings.push("a dlc entry has a non-UTF-8 name and was ignored".into());
            continue;
        };
        if name.is_empty()
            || name.len() > MAX_DLC_NAME_BYTES
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            warnings.push("a dlc entry has an unusual name and was ignored".into());
            continue;
        }
        if ids.len() >= MAX_DLC_ENTRIES {
            warnings.push("dlc entry limit reached, remaining entries ignored".into());
            break;
        }
        ids.push(name);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}
