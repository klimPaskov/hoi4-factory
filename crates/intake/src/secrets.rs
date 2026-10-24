//! Secret handling for GitHub credentials.
//!
//! The server supplies a [`SecretSource`] backed by its secret store (the accounts
//! crate's AES-GCM store, an environment, a vault). Intake asks for values by key name
//! only for the duration of one operation and never persists or logs them. Rows store the
//! key name, never the value (C-10).

use std::fmt;

/// A secret value. `Debug` and `Display` print a placeholder, and the bytes are
/// overwritten on drop.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        SecretString(value.into())
    }

    /// The value. Keep the result out of logs, errors and persisted rows.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([redacted])")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // Best effort without unsafe code: overwrite the bytes the string owns, then clear.
        let len = self.0.len();
        self.0.clear();
        self.0.extend(std::iter::repeat_n('\0', len));
        self.0.clear();
    }
}

/// Failure to read the secret store. Carries no value.
#[derive(Debug, thiserror::Error)]
#[error("secret store error: {0}")]
pub struct SecretError(pub String);

/// Read access to the server secret store.
pub trait SecretSource: Send + Sync + 'static {
    /// Value stored under `name`, or `None` when there is no such entry.
    fn get(&self, name: &str) -> Result<Option<SecretString>, SecretError>;
}

/// A fixed in-memory source, for tests and local development.
#[derive(Debug, Default, Clone)]
pub struct StaticSecrets(std::collections::BTreeMap<String, String>);

impl StaticSecrets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.0.insert(name.to_owned(), value.to_owned());
        self
    }
}

impl SecretSource for StaticSecrets {
    fn get(&self, name: &str) -> Result<Option<SecretString>, SecretError> {
        Ok(self.0.get(name).map(|v| SecretString::new(v.clone())))
    }
}

/// Valid secret-store key names: what a `repository_links.installation_ref` may hold.
pub fn valid_secret_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b':'))
        && !looks_like_credential(name)
}

/// Guards against a credential value being stored where a key name belongs.
pub fn looks_like_credential(text: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "ghs_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghr_",
        "github_pat_",
        "-----BEGIN",
    ];
    PREFIXES.iter().any(|p| text.starts_with(p))
}
