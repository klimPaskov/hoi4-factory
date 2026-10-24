//! Redaction rules shared by the log layer and the request log.
//!
//! The server never reads a request or response body for logging, and the request log carries
//! only an allowlist of fields (method, path, redacted query, status, latency, request id, user
//! agent). These rules are the second layer: any event from any crate that names a field after a
//! secret, or whose value looks like a cookie or credential header, is masked before it is
//! written.

use std::borrow::Cow;

pub const REDACTED: &str = "[redacted]";

const SENSITIVE_NAME_PARTS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "authorization",
    "cookie",
    "api_key",
    "apikey",
    "credential",
    "private_key",
];

/// Query keys that carry one-time values even though their names are short.
const SENSITIVE_QUERY_KEYS: &[&str] = &["code", "state", "sig", "signature", "key", "otp"];

/// True for field, header and query names that hold secrets.
pub fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase().replace('-', "_");
    SENSITIVE_NAME_PARTS.iter().any(|part| lower.contains(part))
}

/// Masks a value that looks like a credential header or cookie even under an innocent name.
pub fn scrub_value(value: &str) -> Cow<'_, str> {
    let lower = value.to_ascii_lowercase();
    let looks_secret = lower.contains("hf_session=")
        || lower.contains("hf_gh_state=")
        || lower.contains("password=")
        || lower.contains("token=")
        || lower.starts_with("bearer ")
        || lower.starts_with("basic ");
    if looks_secret {
        Cow::Borrowed(REDACTED)
    } else {
        Cow::Borrowed(value)
    }
}

/// A query string with the values of sensitive keys masked. `None` when there is no query.
pub fn redact_query(query: Option<&str>) -> Option<String> {
    let query = query.filter(|q| !q.is_empty())?;
    let parts: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _))
                if is_sensitive_name(key)
                    || SENSITIVE_QUERY_KEYS.contains(&key.to_ascii_lowercase().as_str()) =>
            {
                format!("{key}={REDACTED}")
            }
            _ => pair.to_owned(),
        })
        .collect();
    Some(parts.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_hold_secrets_are_recognised() {
        for name in [
            "password",
            "new_password",
            "Authorization",
            "cookie",
            "set-cookie",
            "session_token",
            "client_secret",
            "x-api-key",
            "API_KEY",
        ] {
            assert!(is_sensitive_name(name), "{name}");
        }
        for name in ["message", "path", "status", "request_id", "session_id"] {
            assert!(!is_sensitive_name(name), "{name}");
        }
    }

    #[test]
    fn query_values_are_masked_by_key() {
        assert_eq!(
            redact_query(Some("code=abc&state=xyz&page=2&token=t")).unwrap(),
            "code=[redacted]&state=[redacted]&page=2&token=[redacted]"
        );
        assert_eq!(redact_query(Some("")), None);
        assert_eq!(redact_query(None), None);
    }

    #[test]
    fn credential_looking_values_are_masked() {
        assert_eq!(scrub_value("hf_session=abcdef; Path=/"), REDACTED);
        assert_eq!(scrub_value("Bearer ghu_123"), REDACTED);
        assert_eq!(scrub_value("create project"), "create project");
    }
}
