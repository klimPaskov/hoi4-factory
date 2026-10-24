//! Limits and file type policy for upload intake.
//!
//! Both come from server configuration. [`IntakeLimits::default`] is a conservative
//! starting point for a hosted mod workspace, not a statement about what HOI4 mods need:
//! the operator sets the real values.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

const MIB: u64 = 1024 * 1024;

/// Hard limits applied to every upload, whatever its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntakeLimits {
    /// Bytes of the archive as received (ZIP), or of all files as received (folder).
    pub max_upload_bytes: u64,
    /// Bytes of all files after extraction.
    pub max_total_bytes: u64,
    /// Bytes of one file after extraction.
    pub max_file_bytes: u64,
    /// Regular files, including those under `.git/`.
    pub max_files: u64,
    /// Entries of any kind (files, directories), counted before anything is read.
    pub max_entries: u64,
    /// Components in a path, counting the file name.
    pub max_path_depth: usize,
    /// Bytes in a whole relative path.
    pub max_path_bytes: usize,
    /// Largest accepted ratio of uncompressed to compressed bytes for one entry whose
    /// uncompressed size exceeds [`IntakeLimits::ratio_floor_bytes`].
    pub max_compression_ratio: u64,
    /// Entries smaller than this are exempt from the per-entry ratio check, because a
    /// small file of zeros legitimately compresses by a large factor.
    pub ratio_floor_bytes: u64,
}

impl Default for IntakeLimits {
    fn default() -> Self {
        IntakeLimits {
            max_upload_bytes: 1024 * MIB,
            max_total_bytes: 2048 * MIB,
            max_file_bytes: 512 * MIB,
            max_files: 20_000,
            max_entries: 40_000,
            max_path_depth: 24,
            max_path_bytes: 240,
            max_compression_ratio: 200,
            ratio_floor_bytes: MIB,
        }
    }
}

/// Which file names an upload may contain.
///
/// Deny wins over allow. Anything not allowed is refused with
/// `file_type_not_allowed`; names on the deny list get the more specific
/// `executable_refused`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileTypePolicy {
    /// Extensions without the dot, lowercase.
    pub allowed_extensions: BTreeSet<String>,
    /// Whole file names (lowercase) allowed without an extension rule.
    pub allowed_names: BTreeSet<String>,
    /// Extensions without the dot, lowercase, that can run or be loaded as code.
    pub denied_extensions: BTreeSet<String>,
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

impl Default for FileTypePolicy {
    fn default() -> Self {
        FileTypePolicy {
            allowed_extensions: set(&[
                // Mod descriptors and script/text formats of the game.
                "mod",
                "txt",
                "yml",
                "yaml",
                "csv",
                "gui",
                "gfx",
                "lua",
                "shader",
                "fxh",
                "json",
                "md",
                "asset",
                "mesh",
                "anim",
                "map",
                "info",
                "settings",
                "tsv",
                // Images and audio.
                "dds",
                "tga",
                "png",
                "jpg",
                "jpeg",
                "bmp",
                "ogg",
                "wav",
                // Repository metadata a mod commonly carries.
                "gitignore",
                "gitattributes",
                "gitmodules",
            ]),
            allowed_names: set(&[
                "license",
                "licence",
                "readme",
                "notice",
                "authors",
                "changelog",
                "thumbnail",
                ".gitignore",
                ".gitattributes",
                ".gitmodules",
                ".gitkeep",
                ".editorconfig",
            ]),
            denied_extensions: set(&[
                "exe", "dll", "bat", "cmd", "ps1", "sh", "so", "dylib", "jar", "msi", "scr", "com",
                "vbs", "vbe", "wsf", "wsh", "hta", "lnk", "reg", "pif", "cpl", "msc", "apk", "app",
                "bin", "run", "elf",
            ]),
        }
    }
}

/// Result of checking one file name against a [`FileTypePolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeVerdict {
    Allowed,
    /// Executable or script extension.
    Executable,
    /// Not on the allow list.
    NotAllowed,
}

impl FileTypePolicy {
    /// Judge the last component of a path by name alone. Only the final extension counts,
    /// so `notes.exe.txt` is text and `payload.txt.exe` is an executable. Content is
    /// checked separately.
    pub fn judge(&self, file_name: &str) -> TypeVerdict {
        let lower = crate::util::lower(file_name);
        if self.allowed_names.contains(&lower) {
            return TypeVerdict::Allowed;
        }
        // For a dotfile such as `.gitignore` the extension is the part after the dot.
        let extension = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        if self.denied_extensions.contains(extension) {
            return TypeVerdict::Executable;
        }
        if self.allowed_extensions.contains(extension) {
            TypeVerdict::Allowed
        } else {
            TypeVerdict::NotAllowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_final_extension_decides() {
        let policy = FileTypePolicy::default();
        assert_eq!(policy.judge("focus.txt"), TypeVerdict::Allowed);
        assert_eq!(policy.judge("FOCUS.TXT"), TypeVerdict::Allowed);
        assert_eq!(policy.judge("notes.exe.txt"), TypeVerdict::Allowed);
        assert_eq!(policy.judge("payload.txt.exe"), TypeVerdict::Executable);
        assert_eq!(policy.judge("Setup.BAT"), TypeVerdict::Executable);
        assert_eq!(policy.judge("run.sh"), TypeVerdict::Executable);
        assert_eq!(policy.judge("LICENSE"), TypeVerdict::Allowed);
        assert_eq!(policy.judge(".gitignore"), TypeVerdict::Allowed);
        assert_eq!(policy.judge(".sh"), TypeVerdict::Executable);
        assert_eq!(policy.judge(".env"), TypeVerdict::NotAllowed);
        assert_eq!(policy.judge("noextension"), TypeVerdict::NotAllowed);
        assert_eq!(policy.judge("archive.zip"), TypeVerdict::NotAllowed);
    }

    #[test]
    fn deny_wins_over_allow() {
        let mut policy = FileTypePolicy::default();
        policy.allowed_extensions.insert("exe".into());
        assert_eq!(policy.judge("a.exe"), TypeVerdict::Executable);
    }
}
