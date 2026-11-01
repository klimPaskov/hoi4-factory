//! Canonical JSON and hashing. Hashes in this crate (event payloads, request artifacts, bundle
//! manifests) are SHA-256 over canonical JSON: object keys sorted at every depth, no whitespace.
//! The ordering is explicit here instead of relying on serde_json's map type, which another
//! crate's `preserve_order` feature could change for the whole workspace.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Writes `value` with sorted object keys and no whitespace.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                // Serializing a string cannot fail.
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write_value(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// SHA-256 (lowercase hex) of the canonical JSON of `value`.
pub fn canonical_sha256(value: &Value) -> String {
    sha256_hex(canonical_json(value).as_bytes())
}

pub(crate) fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_at_every_depth() {
        let a = json!({"b": 1, "a": {"y": [1, {"d": 1, "c": 2}], "x": null}});
        assert_eq!(
            canonical_json(&a),
            r#"{"a":{"x":null,"y":[1,{"c":2,"d":1}]},"b":1}"#
        );
    }

    #[test]
    fn known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
