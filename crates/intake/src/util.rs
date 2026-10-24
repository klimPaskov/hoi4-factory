//! Small shared helpers.

use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

pub(crate) fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// Lowercase hexadecimal encoding.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Copy `reader` to `writer`, hashing and counting. Stops with `Ok(Copied::LimitExceeded)`
/// as soon as more than `limit` bytes were read, having written at most `limit` bytes.
pub(crate) enum Copied {
    Done { bytes: u64, sha256: String },
    LimitExceeded,
}

pub(crate) fn copy_hashed(
    reader: &mut (impl Read + ?Sized),
    writer: &mut impl Write,
    limit: u64,
) -> io::Result<Copied> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > limit {
            return Ok(Copied::LimitExceeded);
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
    }
    Ok(Copied::Done {
        bytes: total,
        sha256: hex(&hasher.finalize()),
    })
}

/// Replace every occurrence of each secret in `text`.
pub(crate) fn scrub(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_owned();
    for secret in secrets {
        if secret.len() >= 4 {
            out = out.replace(secret, "[redacted]");
        }
    }
    out
}
