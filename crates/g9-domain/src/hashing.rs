//! Canonical hashing helpers used for approval binding, audit chains, content
//! addressing and idempotent replay detection.
//!
//! Canonical JSON: object keys sorted lexicographically at every level, no
//! insignificant whitespace, numbers as serde_json renders them. This is enough
//! for a single implementation hashing its own values; it is not a cross-language
//! JCS implementation and must not be presented as one.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Hex-encoded SHA-256 of raw bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Recursively sort object keys so that semantically equal values hash equally.
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let mut out = serde_json::Map::new();
            for (k, v) in entries {
                out.insert(k.clone(), canonicalize(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// Canonical JSON string.
pub fn canonical_json(value: &Value) -> String {
    // serde_json never fails to serialize a `Value`.
    serde_json::to_string(&canonicalize(value)).unwrap_or_default()
}

/// Hex SHA-256 of the canonical JSON form of a value.
pub fn sha256_json(value: &Value) -> String {
    sha256_hex(canonical_json(value).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_hash() {
        let a = json!({"b": 1, "a": {"y": [1, {"k": 2, "j": 3}], "x": 2}});
        let b = json!({"a": {"x": 2, "y": [1, {"j": 3, "k": 2}]}, "b": 1});
        assert_eq!(sha256_json(&a), sha256_json(&b));
        assert_eq!(canonical_json(&a), r#"{"a":{"x":2,"y":[1,{"j":3,"k":2}]},"b":1}"#);
    }

    #[test]
    fn known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
