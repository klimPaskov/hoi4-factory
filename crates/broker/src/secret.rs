//! A secret string that cannot be printed by accident.

use std::fmt;

/// A secret value (an API key). `Debug` prints `[REDACTED]`; there is no `Display`, no
/// `Serialize` and no `Clone`. The bytes are overwritten on drop (best effort, no unsafe code).
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value. Keep the result out of logs, errors and persisted rows.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        let len = self.0.len();
        self.0.clear();
        self.0.extend(std::iter::repeat_n('\0', len));
        self.0.clear();
    }
}
