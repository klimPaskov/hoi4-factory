//! The server game reference: loaded at startup, never fatal.
//!
//! A missing or unreadable mount marks the reference unavailable. Everything that does not need
//! the game keeps working, and a background task retries so a late mount is picked up. Only the
//! manifest summary ever leaves this module toward a browser: no host path, no file content.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use factory_game_ref::{
    DEFAULT_MOUNT_LOCATOR, GameRefError, GameReference, ReadOnlyCheck, ReadOnlyEvidence,
    check_read_only,
};
use serde::Serialize;

const RETRY_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The directory does not exist or is not a directory.
    NotMounted,
    /// The directory exists but could not be read.
    Unreadable,
    /// The directory does not carry the expected game metadata.
    InvalidMetadata,
}

#[derive(Debug, Clone)]
pub enum GameRefStatus {
    Available {
        reference: Box<GameReference>,
        read_only: ReadOnlyCheck,
    },
    Unavailable {
        reason: UnavailableReason,
        /// Server-side detail for logs. Never sent to a client.
        detail: String,
    },
}

/// The only game reference data a client may see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameReferenceSummary {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<UnavailableReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    pub dlc_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_hash: Option<String>,
    /// True when the server could not create a file in the mount.
    pub read_only: bool,
    /// `read_only_filesystem`, `permission_denied`, `writable` or `inconclusive`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_evidence: Option<&'static str>,
}

impl GameRefStatus {
    pub fn summary(&self) -> GameReferenceSummary {
        match self {
            GameRefStatus::Available {
                reference,
                read_only,
            } => GameReferenceSummary {
                available: true,
                reason: None,
                version: reference.version.clone(),
                raw_version: reference.raw_version.clone(),
                rev: reference.rev.clone(),
                dlc_ids: reference.dlc_ids.clone(),
                manifest_hash: Some(reference.manifest_hash.clone()),
                read_only: read_only.is_read_only(),
                read_only_evidence: Some(evidence_name(&read_only.evidence)),
            },
            GameRefStatus::Unavailable { reason, .. } => GameReferenceSummary {
                available: false,
                reason: Some(*reason),
                version: None,
                raw_version: None,
                rev: None,
                dlc_ids: Vec::new(),
                manifest_hash: None,
                read_only: false,
                read_only_evidence: None,
            },
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, GameRefStatus::Available { .. })
    }

    /// Available and confirmed read-only.
    pub fn is_healthy(&self) -> bool {
        matches!(self, GameRefStatus::Available { read_only, .. } if read_only.is_read_only())
    }
}

fn evidence_name(evidence: &ReadOnlyEvidence) -> &'static str {
    match evidence {
        ReadOnlyEvidence::ReadOnlyFilesystem => "read_only_filesystem",
        ReadOnlyEvidence::PermissionDenied => "permission_denied",
        ReadOnlyEvidence::Writable { .. } => "writable",
        ReadOnlyEvidence::Inconclusive { .. } => "inconclusive",
    }
}

fn classify(err: &GameRefError) -> UnavailableReason {
    match err {
        GameRefError::NotADirectory => UnavailableReason::NotMounted,
        GameRefError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            UnavailableReason::NotMounted
        }
        GameRefError::Io { .. } => UnavailableReason::Unreadable,
        _ => UnavailableReason::InvalidMetadata,
    }
}

/// Blocking: reads the metadata files and probes the mount.
fn load(dir: &Path, locator: &str) -> GameRefStatus {
    match GameReference::read(dir, locator) {
        Ok(reference) => {
            let read_only = check_read_only(dir);
            if !read_only.is_read_only() {
                tracing::error!(
                    evidence = evidence_name(&read_only.evidence),
                    "the game reference mount is not confirmed read-only; mount it with :ro"
                );
            }
            GameRefStatus::Available {
                reference: Box::new(reference),
                read_only,
            }
        }
        Err(err) => GameRefStatus::Unavailable {
            reason: classify(&err),
            detail: err.to_string(),
        },
    }
}

/// Shared handle on the current status.
#[derive(Clone)]
pub struct GameReferenceHandle {
    dir: Arc<PathBuf>,
    locator: Arc<str>,
    status: Arc<RwLock<GameRefStatus>>,
}

impl GameReferenceHandle {
    /// Reads the reference once. Never fails: problems become [`GameRefStatus::Unavailable`].
    pub async fn load(dir: PathBuf) -> Self {
        let locator: Arc<str> = Arc::from(DEFAULT_MOUNT_LOCATOR);
        let (probe_dir, probe_locator) = (dir.clone(), locator.clone());
        let status = tokio::task::spawn_blocking(move || load(&probe_dir, &probe_locator))
            .await
            .unwrap_or_else(|_| GameRefStatus::Unavailable {
                reason: UnavailableReason::Unreadable,
                detail: "the reader task failed".to_owned(),
            });
        match &status {
            GameRefStatus::Available { reference, .. } => tracing::info!(
                version = reference.version.as_deref().unwrap_or("unknown"),
                dlc = reference.dlc_ids.len(),
                manifest = %reference.manifest_hash,
                "game reference loaded"
            ),
            GameRefStatus::Unavailable { reason, detail } => tracing::warn!(
                reason = ?reason,
                detail = %detail,
                "game reference unavailable; features that need it are disabled"
            ),
        }
        Self {
            dir: Arc::new(dir),
            locator,
            status: Arc::new(RwLock::new(status)),
        }
    }

    pub fn status(&self) -> GameRefStatus {
        self.status
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The manifest, when the reference is available.
    pub fn reference(&self) -> Option<GameReference> {
        match self.status() {
            GameRefStatus::Available { reference, .. } => Some(*reference),
            GameRefStatus::Unavailable { .. } => None,
        }
    }

    /// Host path of the mount, for server-side tools only. Never serialize it.
    pub fn mount_dir(&self) -> &Path {
        &self.dir
    }

    /// Re-reads the reference.
    pub async fn refresh(&self) {
        let (dir, locator) = (self.dir.clone(), self.locator.clone());
        if let Ok(status) = tokio::task::spawn_blocking(move || load(&dir, &locator)).await {
            *self.status.write().unwrap_or_else(|e| e.into_inner()) = status;
        }
    }

    /// Retries every minute while the reference is unavailable.
    pub fn spawn_retry(&self) {
        let handle = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(RETRY_EVERY).await;
                if !handle.status().is_available() {
                    handle.refresh().await;
                    if handle.status().is_available() {
                        tracing::info!("game reference became available");
                    }
                }
            }
        });
    }

    /// A handle with a fixed status, for tests of dependants.
    pub fn fixed(dir: PathBuf, status: GameRefStatus) -> Self {
        Self {
            dir: Arc::new(dir),
            locator: Arc::from(DEFAULT_MOUNT_LOCATOR),
            status: Arc::new(RwLock::new(status)),
        }
    }
}
