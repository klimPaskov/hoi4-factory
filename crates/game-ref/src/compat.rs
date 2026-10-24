use serde::{Deserialize, Serialize};

use crate::reader::GameReference;

/// Which way the mod's declaration differs from the server reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarnDirection {
    /// The mod declares an older game version than the server reference.
    ModOlder,
    /// The mod declares a newer game version than the server reference.
    ModNewer,
}

/// Result of comparing a mod's declared `supported_version` with the server reference
/// (SPEC-006: mods that declare an older supported game version get a visible warning).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Compatibility {
    /// The declaration covers the server reference version.
    Match,
    /// The declaration does not cover the reference. Validation still runs, with a
    /// visible warning.
    Warn {
        direction: WarnDirection,
        reason: String,
    },
    /// A comparison is not possible. This is an explicit unknown, never a pass.
    Unknown { reason: String },
}

/// A parsed `supported_version` declaration such as `1.14.*`, `1.14.5` or `v1.14`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredVersion {
    /// Leading numeric components. Components after a wildcard are dropped.
    pub components: Vec<u32>,
    /// True when the declaration contains a `*`.
    pub wildcard: bool,
}

impl DeclaredVersion {
    /// Parse a declaration. Surrounding quotes, whitespace and a leading `v` are ignored.
    /// Returns `None` for anything that is not dot-separated numbers and `*`.
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim().trim_matches('"').trim();
        let trimmed = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
        if trimmed.is_empty() {
            return None;
        }
        let mut components = Vec::new();
        let mut wildcard = false;
        for token in trimmed.split('.') {
            if wildcard {
                if token != "*" && token.parse::<u32>().is_err() {
                    return None;
                }
                continue;
            }
            if token == "*" {
                wildcard = true;
            } else {
                components.push(token.parse::<u32>().ok()?);
            }
        }
        Some(DeclaredVersion {
            components,
            wildcard,
        })
    }
}

/// First `digits(.digits)+` run in `text`, for example `v1.19.3.0` or `1.19.3 (c01a)`.
pub(crate) fn parse_numeric_version(text: &str) -> Option<Vec<u32>> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if !bytes[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let run = text[start..end].trim_end_matches('.');
        let parts: Option<Vec<u32>> = run.split('.').map(|p| p.parse::<u32>().ok()).collect();
        match parts {
            Some(parts) if parts.len() >= 2 => return Some(parts),
            _ => start = end,
        }
    }
    None
}

fn join(components: &[u32]) -> String {
    components
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// Compare a mod's declared `supported_version` with the reference.
///
/// A declaration with fewer components than the reference matches as a prefix, so `7.8`
/// covers `7.8.9`. Missing components of the reference count as zero when the declaration
/// is longer. The first differing component decides the warning direction.
pub fn check_compatibility(reference: &GameReference, declared: Option<&str>) -> Compatibility {
    let Some(declared) = declared.map(str::trim).filter(|d| !d.is_empty()) else {
        return Compatibility::Unknown {
            reason: "the mod declares no supported_version".into(),
        };
    };
    let Some(parsed) = DeclaredVersion::parse(declared) else {
        return Compatibility::Unknown {
            reason: "the supported_version of the mod is not a version pattern".into(),
        };
    };
    let Some(actual) = reference.numeric_version.as_deref() else {
        return Compatibility::Unknown {
            reason: "the server reference has no parsed numeric version".into(),
        };
    };
    for (index, wanted) in parsed.components.iter().enumerate() {
        let have = actual.get(index).copied().unwrap_or(0);
        if *wanted == have {
            continue;
        }
        let (direction, word) = if *wanted < have {
            (WarnDirection::ModOlder, "older")
        } else {
            (WarnDirection::ModNewer, "newer")
        };
        return Compatibility::Warn {
            direction,
            reason: format!(
                "the mod declares supported_version {declared}, which is {word} than the server reference {}; diagnostics that depend on the version difference are labelled",
                join(actual)
            ),
        };
    }
    Compatibility::Match
}
