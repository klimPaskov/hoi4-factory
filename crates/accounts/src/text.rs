//! Validation of user-supplied text: email addresses, display names and project names.
//!
//! Names keep exactly what the user typed (apostrophes, accents, emoji, right-to-left text) apart
//! from trimming surrounding whitespace. Limits count grapheme clusters, as a person counts
//! characters, and bytes, which is what the database column and a hostile client care about.

use sha2::{Digest, Sha256};
use unicode_segmentation::UnicodeSegmentation;

use crate::error::ApiError;

/// Graphemes allowed in a display name, and the byte ceiling that goes with it.
pub const DISPLAY_NAME_GRAPHEMES: usize = 80;
pub const DISPLAY_NAME_BYTES: usize = 320;
/// Graphemes allowed in a project name, and the byte ceiling that goes with it.
pub const PROJECT_NAME_GRAPHEMES: usize = 100;
pub const PROJECT_NAME_BYTES: usize = 400;

/// Trims and checks a free-text label. `what` names the field in the error message.
pub fn normalize_label(
    raw: &str,
    what: &str,
    max_graphemes: usize,
    max_bytes: usize,
) -> Result<String, ApiError> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(ApiError::invalid_request(format!(
            "{what} cannot be empty."
        )));
    }
    // Check bytes first so that a huge input is never segmented.
    if text.len() > max_bytes || text.graphemes(true).count() > max_graphemes {
        return Err(ApiError::invalid_request(format!(
            "{what} is too long: at most {max_graphemes} characters."
        )));
    }
    if text.chars().any(char::is_control) {
        return Err(ApiError::invalid_request(format!(
            "{what} cannot contain control characters."
        )));
    }
    Ok(text.to_owned())
}

pub fn normalize_display_name(requested: Option<&str>, email: &str) -> Result<String, ApiError> {
    let fallback = email.split('@').next().unwrap_or("player");
    let chosen = requested
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(fallback);
    normalize_label(
        chosen,
        "A display name",
        DISPLAY_NAME_GRAPHEMES,
        DISPLAY_NAME_BYTES,
    )
}

pub fn normalize_project_name(raw: &str) -> Result<String, ApiError> {
    normalize_label(
        raw,
        "A project name",
        PROJECT_NAME_GRAPHEMES,
        PROJECT_NAME_BYTES,
    )
}

/// Checks the shape of an email address and returns it trimmed but otherwise exactly as typed:
/// plus tags, dots and letter case survive, and the local part may contain any Unicode letters.
/// Not a full RFC 5322 parser: it accepts what a verification mail can reach. The domain must be
/// ASCII (an internationalised domain is given in its punycode form).
pub fn normalize_email(raw: &str) -> Result<String, ApiError> {
    let invalid = || ApiError::invalid_request("Enter a valid email address.");
    let email = raw.trim();
    if email.len() > 254 || email.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid());
    }
    let (local, domain) = email.rsplit_once('@').ok_or_else(invalid)?;
    let bad_local_char = |c: char| "@<>,;:\"()[]\\".contains(c);
    if local.is_empty()
        || local.len() > 64
        || local.chars().any(bad_local_char)
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return Err(invalid());
    }
    if !domain.is_ascii()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(invalid());
    }
    for label in domain.split('.') {
        let ok = !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !ok {
            return Err(invalid());
        }
    }
    Ok(email.to_owned())
}

/// The form of an address used for comparison, rate-limit keys and audit digests.
pub fn email_key(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Lowercase hex SHA-256 of the comparison form. Used where the address itself must not be stored.
pub fn email_digest(email: &str) -> String {
    let digest = Sha256::digest(email_key(email).as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_keep_what_was_typed() {
        for ok in [
            "Player@Example.ORG",
            "first.last+hoi4@Example.org",
            "o'brien@example.org",
            "łukasz.żółć@example.pl",
            "用户@example.cn",
            "مستخدم@example.org",
            "a@b.co",
        ] {
            assert_eq!(normalize_email(&format!("  {ok} ")).unwrap(), ok, "{ok}");
        }
        assert_eq!(
            email_key("First.Last+Tag@Example.ORG"),
            "first.last+tag@example.org"
        );
        assert_ne!(
            email_key("a+x@example.org"),
            email_key("a@example.org"),
            "+tag is never stripped"
        );
    }

    #[test]
    fn malformed_emails_are_refused() {
        for bad in [
            "",
            "plain",
            "@example.org",
            "a@b",
            "a@.org",
            "a@example..org",
            "a b@example.org",
            "a@exa mple.org",
            "a@@example.org",
            "a@-example.org",
            "a@exämple.org",
            "<a>@example.org",
            ".a@example.org",
            "a.@example.org",
            "a..b@example.org",
            "a\u{0}b@example.org",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad:?}");
        }
        assert!(normalize_email(&format!("{}@example.org", "x".repeat(65))).is_err());
        assert!(normalize_email(&format!("a@{}.org", "x".repeat(250))).is_err());
    }

    #[test]
    fn labels_accept_international_text_and_count_graphemes() {
        for ok in [
            "O'Brien's mod",
            "Café Ünïcode",
            "Weimar 🇩🇪 Republic",
            "👨‍👩‍👧‍👦 family",
            "עברית ומודל",
            "نموذج عربي",
            "日本語のMOD",
        ] {
            assert_eq!(normalize_label(ok, "x", 100, 400).unwrap(), ok);
        }
        // 100 family emoji are 100 graphemes but 2500 bytes: the byte ceiling stops it.
        assert!(normalize_project_name(&"👨‍👩‍👧‍👦".repeat(100)).is_err());
        // 50 flag emoji (8 bytes each) reach the 400 byte ceiling exactly.
        assert!(normalize_project_name(&"🇩🇪".repeat(50)).is_ok());
        assert!(normalize_project_name(&"é".repeat(100)).is_ok());
        assert!(normalize_project_name(&"é".repeat(101)).is_err());
        // Combining marks make one grapheme out of many chars.
        assert!(normalize_project_name(&"e\u{301}".repeat(100)).is_ok());
        assert!(
            normalize_project_name(&format!("e{}", "\u{301}".repeat(300))).is_err(),
            "bytes"
        );
        assert!(normalize_project_name("   ").is_err());
        assert!(normalize_project_name("bad\nname").is_err());
        assert!(normalize_project_name("nul\u{0}").is_err());
    }

    #[test]
    fn display_name_defaults_to_the_local_part() {
        assert_eq!(
            normalize_display_name(None, "kim@example.org").unwrap(),
            "kim"
        );
        assert_eq!(
            normalize_display_name(Some("  Kim  "), "k@example.org").unwrap(),
            "Kim"
        );
        assert_eq!(
            normalize_display_name(Some("   "), "k@example.org").unwrap(),
            "k"
        );
        assert!(normalize_display_name(Some(&"x".repeat(81)), "k@example.org").is_err());
        assert!(normalize_display_name(Some(&"x".repeat(80)), "k@example.org").is_ok());
    }
}
