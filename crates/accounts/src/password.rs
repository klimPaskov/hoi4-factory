//! Argon2id password hashing. Work runs on the blocking pool.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::config::Argon2Params;
use crate::error::ApiError;

pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_PASSWORD_CHARS: usize = 256;

#[derive(Clone)]
pub struct PasswordHasher {
    params: Argon2Params,
    /// Verified against when the account does not exist or has no password, so that the
    /// response time does not reveal which case applied.
    dummy_hash: String,
}

fn argon(params: Argon2Params) -> Result<Argon2<'static>, argon2::Error> {
    let p = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        None,
    )?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, p))
}

fn hash_blocking(params: Argon2Params, password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = argon(params).map_err(|e| {
        tracing::error!(error = %e, "invalid argon2 parameters");
        ApiError::internal()
    })?;
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| {
            tracing::error!(error = %e, "argon2 hashing failed");
            ApiError::internal()
        })
}

impl PasswordHasher {
    pub fn new(params: Argon2Params) -> Result<Self, argon2::Error> {
        argon(params)?;
        let dummy = hash_blocking(params, &crate::tokens::random_token())
            .map_err(|_| argon2::Error::MemoryTooLittle)?;
        Ok(Self {
            params,
            dummy_hash: dummy,
        })
    }

    /// Checks the length policy. Counts characters, not bytes.
    pub fn check_policy(password: &str) -> Result<(), ApiError> {
        let n = password.chars().count();
        if n < MIN_PASSWORD_CHARS {
            return Err(ApiError::invalid_request(format!(
                "Use a password of at least {MIN_PASSWORD_CHARS} characters."
            ))
            .with_id("field", "password"));
        }
        if n > MAX_PASSWORD_CHARS {
            return Err(ApiError::invalid_request(format!(
                "Use a password of at most {MAX_PASSWORD_CHARS} characters."
            ))
            .with_id("field", "password"));
        }
        Ok(())
    }

    pub async fn hash(&self, password: String) -> Result<String, ApiError> {
        let params = self.params;
        tokio::task::spawn_blocking(move || hash_blocking(params, &password))
            .await
            .map_err(|_| ApiError::internal())?
    }

    /// Verifies `password` against `stored`. With `None` it burns the same time against a dummy
    /// hash and returns false.
    pub async fn verify(&self, password: String, stored: Option<String>) -> bool {
        let (hash, real) = match stored {
            Some(h) => (h, true),
            None => (self.dummy_hash.clone(), false),
        };
        let result = tokio::task::spawn_blocking(move || {
            let Ok(parsed) = PasswordHash::new(&hash) else {
                return false;
            };
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .await
        .unwrap_or(false);
        result && real
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> PasswordHasher {
        PasswordHasher::new(Argon2Params {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn hash_is_argon2id_salted_and_verifies() {
        let h = fast();
        let a = h.hash("correct horse battery".to_owned()).await.unwrap();
        let b = h.hash("correct horse battery".to_owned()).await.unwrap();
        assert!(a.starts_with("$argon2id$v=19$m=8,t=1,p=1$"), "{a}");
        assert_ne!(a, b, "salts differ");
        assert!(
            h.verify("correct horse battery".to_owned(), Some(a.clone()))
                .await
        );
        assert!(!h.verify("correct horse batterz".to_owned(), Some(a)).await);
    }

    #[tokio::test]
    async fn missing_hash_never_verifies() {
        let h = fast();
        assert!(!h.verify("anything at all".to_owned(), None).await);
        assert!(
            !h.verify("x".to_owned(), Some("not a phc string".to_owned()))
                .await
        );
    }

    #[test]
    fn policy_counts_characters() {
        assert!(PasswordHasher::check_policy("short").is_err());
        assert!(PasswordHasher::check_policy("exactly-10").is_ok());
        assert!(PasswordHasher::check_policy(&"ä".repeat(10)).is_ok());
        assert!(PasswordHasher::check_policy(&"x".repeat(257)).is_err());
    }
}
