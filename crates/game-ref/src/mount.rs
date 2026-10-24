use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// What the write probe observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence", rename_all = "snake_case")]
pub enum ReadOnlyEvidence {
    /// Creating a file failed because the file system is mounted read-only.
    ReadOnlyFilesystem,
    /// Creating a file failed because this process lacks permission. The mount may still
    /// be writable for another user, so this is weaker evidence than `ReadOnlyFilesystem`.
    PermissionDenied,
    /// Creating a file succeeded. The probe file was removed. The mount is not read-only.
    Writable { probe_removed: bool },
    /// The probe could not decide (for example the directory is missing).
    Inconclusive { kind: String },
}

/// Result of [`check_read_only`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadOnlyCheck {
    pub evidence: ReadOnlyEvidence,
}

impl ReadOnlyCheck {
    /// True when the current process could not create a file in the mount.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self.evidence,
            ReadOnlyEvidence::ReadOnlyFilesystem | ReadOnlyEvidence::PermissionDenied
        )
    }

    /// True only for a real read-only mount rather than a permission failure.
    pub fn is_read_only_mount(&self) -> bool {
        matches!(self.evidence, ReadOnlyEvidence::ReadOnlyFilesystem)
    }
}

#[cfg(unix)]
fn is_erofs(err: &io::Error) -> bool {
    // EROFS
    err.raw_os_error() == Some(30)
}

#[cfg(windows)]
fn is_erofs(err: &io::Error) -> bool {
    // ERROR_WRITE_PROTECT
    err.raw_os_error() == Some(19)
}

#[cfg(not(any(unix, windows)))]
fn is_erofs(_: &io::Error) -> bool {
    false
}

/// Probe whether the current process can write into `mount`.
///
/// The probe tries to create one uniquely named file with `create_new` (it never opens or
/// truncates an existing file). Creation is expected to fail on the read-only game mount.
/// If it unexpectedly succeeds, the file is removed immediately and the result reports
/// `Writable`, which the caller must treat as a deployment defect.
pub fn check_read_only(mount: &Path) -> ReadOnlyCheck {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = mount.join(format!(".hoi4factory-ro-probe-{}-{nanos}", process::id()));
    let evidence = match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(handle) => {
            drop(handle);
            let probe_removed = std::fs::remove_file(&probe).is_ok();
            ReadOnlyEvidence::Writable { probe_removed }
        }
        Err(err) if is_erofs(&err) => ReadOnlyEvidence::ReadOnlyFilesystem,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            ReadOnlyEvidence::PermissionDenied
        }
        Err(err) => ReadOnlyEvidence::Inconclusive {
            kind: format!("{:?}", err.kind()),
        },
    };
    ReadOnlyCheck { evidence }
}
