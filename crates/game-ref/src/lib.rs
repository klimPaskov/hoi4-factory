//! Server game reference for HOI4 Factory.
//!
//! The server holds one read-only copy of the game (SPEC-006, ADR-015). This crate reads
//! only the identity metadata of that copy and produces a [`GameReference`] manifest that
//! evidence records can name. It opens exactly these paths and nothing else:
//!
//! * `launcher-settings.json`, only the `version` and `rawVersion` fields are kept
//! * `hoi4_rev.txt` and `hoi4_branch.txt`, short text, both optional
//! * `checksum_manifest.txt`, streamed through SHA-256 and never retained
//! * the entry names directly under `dlc/`, never the contents of those folders
//!
//! No game file is copied, cached or returned. The manifest carries a logical mount
//! locator (for example `/game`), never a host path, so it can travel to evidence and to
//! the browser client.

mod compat;
mod mount;
mod reader;

pub use compat::{Compatibility, DeclaredVersion, WarnDirection, check_compatibility};
pub use mount::{ReadOnlyCheck, ReadOnlyEvidence, check_read_only};
pub use reader::{DEFAULT_MOUNT_LOCATOR, GameRefError, GameReference, MANIFEST_SCHEMA_VERSION};

/// Lowercase hexadecimal encoding used for every hash in this crate.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}
