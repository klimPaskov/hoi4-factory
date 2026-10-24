//! App JWT (RS256) for the GitHub App.

use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;

use crate::error::IntakeError;
use crate::secrets::SecretString;

#[derive(Serialize)]
struct Claims<'a> {
    iat: i64,
    exp: i64,
    iss: &'a str,
}

/// Sign an App JWT valid for nine minutes, backdated by one minute for clock drift.
/// GitHub accepts at most ten minutes. `pem` may be PKCS#1 or PKCS#8.
pub fn app_jwt(
    app_id: &str,
    pem: &SecretString,
    now_unix: i64,
) -> Result<SecretString, IntakeError> {
    let key = EncodingKey::from_rsa_pem(pem.expose().as_bytes()).map_err(|_| {
        IntakeError::Credentials("the App private key is not a valid RSA PEM".into())
    })?;
    let claims = Claims {
        iat: now_unix - 60,
        exp: now_unix + 540,
        iss: app_id,
    };
    let token = encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|_| IntakeError::Credentials("signing the App JWT failed".into()))?;
    Ok(SecretString::new(token))
}
