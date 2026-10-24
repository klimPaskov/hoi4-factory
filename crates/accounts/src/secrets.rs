//! Encryption of small secrets at rest (AES-256-GCM).
//!
//! The accounts component stores no GitHub user token: the OAuth exchange uses it once, in
//! memory, to read the user's identity. This store exists so that any later stage that must keep
//! a credential (installation references, bring-your-own model keys) encrypts it the same way,
//! and so that no secret column is ever written in clear text.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rand::RngCore;
use rand::rngs::OsRng;

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("the secret key must be 32 bytes, base64 encoded")]
    BadKey,
    #[error("the sealed value is malformed or was sealed under a different key or context")]
    Open,
    #[error("sealing failed")]
    Seal,
}

/// Encrypts and decrypts secret values. `context` is authenticated but not stored; use it to bind
/// a ciphertext to its owner (for example the account id) so rows cannot be swapped.
pub trait SecretStore: Send + Sync + 'static {
    fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, SecretError>;
    fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Vec<u8>, SecretError>;
}

/// Layout of a sealed value: `version(1) || nonce(12) || ciphertext || tag(16)`.
pub struct AesGcmSecretStore {
    cipher: Aes256Gcm,
}

impl AesGcmSecretStore {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.into()),
        }
    }

    /// Key as standard base64 of 32 bytes (`openssl rand -base64 32`).
    pub fn from_base64(key: &str) -> Result<Self, SecretError> {
        let bytes = STANDARD
            .decode(key.trim())
            .map_err(|_| SecretError::BadKey)?;
        let key: [u8; 32] = bytes.try_into().map_err(|_| SecretError::BadKey)?;
        Ok(Self::new(&key))
    }
}

impl SecretStore for AesGcmSecretStore {
    fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, SecretError> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ct = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: context,
                },
            )
            .map_err(|_| SecretError::Seal)?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Vec<u8>, SecretError> {
        if sealed.len() < 1 + NONCE_LEN + 16 || sealed[0] != VERSION {
            return Err(SecretError::Open);
        }
        let (nonce, ct) = sealed[1..].split_at(NONCE_LEN);
        self.cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ct,
                    aad: context,
                },
            )
            .map_err(|_| SecretError::Open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AesGcmSecretStore {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        AesGcmSecretStore::new(&key)
    }

    #[test]
    fn round_trip_and_ciphertext_hides_plaintext() {
        let s = store();
        let sealed = s.seal(b"ghu_example_token_value", b"account-1").unwrap();
        assert!(!sealed.windows(8).any(|w| w == b"ghu_exam"));
        assert_eq!(
            s.open(&sealed, b"account-1").unwrap(),
            b"ghu_example_token_value"
        );
        // Fresh nonce every time.
        assert_ne!(
            sealed,
            s.seal(b"ghu_example_token_value", b"account-1").unwrap()
        );
    }

    #[test]
    fn tampering_wrong_context_and_wrong_key_fail() {
        let s = store();
        let sealed = s.seal(b"secret", b"account-1").unwrap();
        assert!(s.open(&sealed, b"account-2").is_err());
        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        assert!(s.open(&flipped, b"account-1").is_err());
        assert!(s.open(&sealed[..10], b"account-1").is_err());
        assert!(store().open(&sealed, b"account-1").is_err());
        let mut bad_version = sealed;
        bad_version[0] = 9;
        assert!(s.open(&bad_version, b"account-1").is_err());
    }

    #[test]
    fn key_parsing() {
        assert!(AesGcmSecretStore::from_base64("AAAA").is_err());
        assert!(AesGcmSecretStore::from_base64("not base64!!").is_err());
        assert!(AesGcmSecretStore::from_base64(&STANDARD.encode([7u8; 32])).is_ok());
    }
}
