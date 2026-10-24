//! Safe path resolution inside a workspace.
//!
//! A client names a file by a workspace-relative string such as `common/ideas/x.txt`.
//! [`validate_relative`] checks the string without touching the file system. [`resolve`]
//! then walks it component by component under a root and refuses anything that could
//! leave the root or alias another file:
//!
//! * absolute paths, drive letters, UNC prefixes, `..`, `.` and empty components
//! * backslashes, colons (alternate data streams) and other characters Windows forbids
//! * control characters and invisible or bidirectional formatting characters
//! * trailing dots or spaces and DOS device names, which Windows silently rewrites
//! * `.git` (any case, and the 8.3 alias `git~1`), so mod writes never reach Git metadata
//! * symbolic links and Windows reparse points (junctions, mount points) in any component
//! * a name that differs from an existing sibling only by case or Unicode normalization
//!   form, because two file systems would disagree about which file it is
//!
//! Names are never rewritten: the returned path uses the bytes the caller supplied, and a
//! near-duplicate is an error instead of being silently mapped to the existing file.
//!
//! The walk checks and then uses a path, so it cannot stop a concurrent process that swaps
//! a component for a link between the check and the open. The workspace directories are
//! writable only by the service user, which is the boundary that matters. Callers that
//! need to close the gap on Linux should open through `openat2` with `RESOLVE_NO_SYMLINKS`.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

const MAX_COMPONENT_BYTES: usize = 255;
const MAX_PATH_BYTES: usize = 2048;

const RESERVED_DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Why a path was refused.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("path is empty")]
    Empty,
    #[error("path is absolute")]
    Absolute,
    #[error("path contains an empty component")]
    EmptyComponent,
    #[error("path contains a '.' or '..' component")]
    Traversal,
    #[error("component {0:?} contains a forbidden character")]
    ForbiddenChar(String),
    #[error("component {0:?} is a reserved device name")]
    ReservedName(String),
    #[error("component {0:?} ends with a dot or a space")]
    TrailingDotOrSpace(String),
    #[error("component {0:?} names Git metadata")]
    GitMetadata(String),
    #[error("path or component is too long")]
    TooLong,
    #[error("component {0:?} is a symbolic link")]
    Symlink(String),
    #[error("component {0:?} is a reparse point")]
    ReparsePoint(String),
    #[error("component {requested:?} collides with existing {existing:?}")]
    NameCollision { requested: String, existing: String },
    #[error("a parent component is not a directory")]
    NotADirectory,
    #[error("path does not exist")]
    NotFound,
    #[error("io error while resolving: {0}")]
    Io(io::Error),
}

/// Whether the last component must already exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMode {
    /// Every component must exist.
    MustExist,
    /// Missing trailing components are allowed (a file or directory about to be created).
    /// Existing components are still checked.
    MayCreate,
}

/// Comparison key under which two names count as the same file on some common file
/// system: compatibility normalization (NFKC), lowercase, NFKC again. Used to detect
/// collisions only. It is never used to rewrite a name.
pub fn fold_key(name: &str) -> String {
    name.nfkc()
        .collect::<String>()
        .to_lowercase()
        .nfkc()
        .collect()
}

/// True when two names are different strings but collide under [`fold_key`].
pub fn names_collide(a: &str, b: &str) -> bool {
    a != b && fold_key(a) == fold_key(b)
}

fn is_invisible_or_bidi(c: char) -> bool {
    matches!(c,
        '\u{00ad}' // soft hyphen
        | '\u{061c}'
        | '\u{180e}'
        | '\u{200b}'..='\u{200f}'
        | '\u{2028}'..='\u{202e}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{206f}'
        | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}')
}

fn validate_component(component: &str) -> Result<(), PathError> {
    if component.is_empty() {
        return Err(PathError::EmptyComponent);
    }
    if component == "." || component == ".." {
        return Err(PathError::Traversal);
    }
    if component.len() > MAX_COMPONENT_BYTES {
        return Err(PathError::TooLong);
    }
    if component
        .chars()
        .any(|c| c.is_control() || is_invisible_or_bidi(c) || "\\:<>\"|?*".contains(c))
    {
        return Err(PathError::ForbiddenChar(component.to_owned()));
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace(component.to_owned()));
    }
    let folded = fold_key(component);
    let stem = folded.split('.').next().unwrap_or("");
    if RESERVED_DEVICE_NAMES.contains(&stem.trim_end()) {
        return Err(PathError::ReservedName(component.to_owned()));
    }
    if folded == ".git" || folded == "git~1" {
        return Err(PathError::GitMetadata(component.to_owned()));
    }
    Ok(())
}

/// Check a client-supplied workspace-relative path without touching the file system and
/// return its components. Separators are `/` only.
pub fn validate_relative(rel: &str) -> Result<Vec<&str>, PathError> {
    if rel.is_empty() {
        return Err(PathError::Empty);
    }
    if rel.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    let bytes = rel.as_bytes();
    if bytes[0] == b'/' || bytes[0] == b'\\' {
        return Err(PathError::Absolute);
    }
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(PathError::Absolute);
    }
    let components: Vec<&str> = rel.split('/').collect();
    for component in &components {
        validate_component(component)?;
    }
    Ok(components)
}

/// True for a symbolic link or, on Windows, any reparse point.
pub fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    reparse_attribute(metadata)
}

#[cfg(windows)]
fn reparse_attribute(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn reparse_attribute(_: &fs::Metadata) -> bool {
    false
}

enum Lookup {
    Exact,
    Collision(String),
    Missing,
}

fn lookup(dir: &Path, name: &str) -> io::Result<Lookup> {
    // Linux file systems compare names exactly, so one lstat decides. Elsewhere a lookup
    // can succeed through case folding, so the directory listing is the authority.
    if cfg!(target_os = "linux") && fs::symlink_metadata(dir.join(name)).is_ok() {
        return Ok(Lookup::Exact);
    }
    let key = fold_key(name);
    let mut collision = None;
    for entry in fs::read_dir(dir)? {
        let entry_name = entry?.file_name();
        if entry_name == OsStr::new(name) {
            return Ok(Lookup::Exact);
        }
        if let Some(text) = entry_name.to_str()
            && fold_key(text) == key
        {
            collision = Some(text.to_owned());
        }
    }
    Ok(match collision {
        Some(existing) => Lookup::Collision(existing),
        None => Lookup::Missing,
    })
}

/// Resolve `rel` under `root` and return the absolute path, or the reason it is refused.
///
/// `root` is trusted: it is built by the storage layer from typed ids. Every component
/// below it is checked.
pub fn resolve(root: &Path, rel: &str, mode: ResolveMode) -> Result<PathBuf, PathError> {
    let components = validate_relative(rel)?;
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(PathError::NotADirectory),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(PathError::NotFound),
        Err(err) => return Err(PathError::Io(err)),
    }
    let mut current = root.to_path_buf();
    let last = components.len() - 1;
    let mut missing = false;
    for (index, component) in components.iter().enumerate() {
        if missing {
            current.push(component);
            continue;
        }
        match lookup(&current, component).map_err(PathError::Io)? {
            Lookup::Exact => {}
            Lookup::Collision(existing) => {
                return Err(PathError::NameCollision {
                    requested: (*component).to_owned(),
                    existing,
                });
            }
            Lookup::Missing => {
                if mode == ResolveMode::MustExist {
                    return Err(PathError::NotFound);
                }
                missing = true;
                current.push(component);
                continue;
            }
        }
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(PathError::Io)?;
        if metadata.file_type().is_symlink() {
            return Err(PathError::Symlink((*component).to_owned()));
        }
        if reparse_attribute(&metadata) {
            return Err(PathError::ReparsePoint((*component).to_owned()));
        }
        if index < last && !metadata.is_dir() {
            return Err(PathError::NotADirectory);
        }
    }
    Ok(current)
}
