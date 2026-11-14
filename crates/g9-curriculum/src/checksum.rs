//! Content checksums for manifests: SHA-256 over canonical JSON
//! (`g9_domain::hashing`: keys sorted at every level, no insignificant
//! whitespace). Two manifests that differ only in key order or in the presence
//! of empty optional lists hash equally; any content change changes the hash.
//!
//! Checksums are the identity that reconciliation and promotion approvals bind
//! to (ADR-0003: approvals bind to exact operations; `docs/curriculum/
//! provenance-and-reconciliation.md` step 5).

use crate::error::CurriculumError;
use crate::manifest::Manifest;
use g9_domain::hashing::sha256_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// A validated hex SHA-256 digest (64 lowercase hex characters).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Checksum(String);

impl Checksum {
    /// Parse and validate a hex digest.
    pub fn parse(s: &str) -> Result<Self, CurriculumError> {
        if s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(s.to_string()))
        } else {
            Err(CurriculumError::InvalidChecksum)
        }
    }

    /// Hex digest as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `other` (an untrusted string from a manifest) equals this digest.
    pub fn matches(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl TryFrom<String> for Checksum {
    type Error = CurriculumError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<Checksum> for String {
    fn from(c: Checksum) -> String {
        c.0
    }
}

impl fmt::Debug for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Checksum({})", self.0)
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Checksum of an arbitrary JSON value in canonical form.
pub fn json_checksum(value: &Value) -> Checksum {
    // `sha256_json` always yields 64 lowercase hex characters.
    Checksum(sha256_json(value))
}

/// Checksum of a manifest: it is serialized with serde (empty optional lists are
/// omitted) and hashed in canonical form.
pub fn manifest_checksum(manifest: &Manifest) -> Result<Checksum, CurriculumError> {
    let value = serde_json::to_value(manifest).map_err(|_| CurriculumError::Serialization)?;
    Ok(json_checksum(&value))
}

/// Top-level manifest fields that say where a manifest comes from, not what it
/// teaches. They are expected to differ between an original and its candidate, so
/// they are left out of [`content_checksum`] and of the comparison in `reconcile`.
const PROVENANCE_FIELDS: [&str; 5] = [
    "manifest_schema_version",
    "version",
    "provenance",
    "provenance_note",
    "reconciliation",
];

/// Checksum of the curriculum content of a manifest: the manifest without its
/// provenance fields (`manifest_schema_version`, `version`, `provenance`,
/// `provenance_note`, `reconciliation`), hashed in canonical form.
///
/// A candidate keeps this checksum when it is relabelled (a new version, the
/// `RECONCILED_CANDIDATE` provenance, the reconciliation block) and changes it with
/// any edit to the content, so a reviewed diff can be bound to exactly what was
/// reviewed: the field names of a diff alone do not say what the new values are.
pub fn content_checksum(manifest: &Manifest) -> Result<Checksum, CurriculumError> {
    let mut value = serde_json::to_value(manifest).map_err(|_| CurriculumError::Serialization)?;
    if let Some(object) = value.as_object_mut() {
        for key in PROVENANCE_FIELDS {
            object.remove(key);
        }
    }
    Ok(json_checksum(&value))
}

/// Checksum of raw JSON text, computed on its parsed canonical form so that
/// formatting and key order do not matter. The text is parsed as a generic
/// value, not as a manifest, so this also works for files that fail validation.
pub fn json_text_checksum(text: &str) -> Result<Checksum, CurriculumError> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| CurriculumError::Parse(parse_message(&e)))?;
    Ok(json_checksum(&value))
}

/// Position-only description of a JSON parse error; never echoes the input.
pub(crate) fn parse_message(e: &serde_json::Error) -> String {
    format!(
        "{} at line {} column {}",
        match e.classify() {
            serde_json::error::Category::Io => "i/o error",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "data does not match the manifest format",
            serde_json::error::Category::Eof => "unexpected end of input",
        },
        e.line(),
        e.column()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checksum_parse_validates_hex() {
        assert!(Checksum::parse(&"a".repeat(64)).is_ok());
        assert!(Checksum::parse(&"A".repeat(64)).is_err());
        assert!(Checksum::parse(&"a".repeat(63)).is_err());
        assert!(Checksum::parse("").is_err());
        let via_serde: Result<Checksum, _> = serde_json::from_str("\"zz\"");
        assert!(via_serde.is_err());
    }

    #[test]
    fn checksum_stable_under_key_order() {
        let a = json_text_checksum(r#"{"b":[{"y":1,"x":2}],"a":"v"}"#).unwrap();
        let b = json_text_checksum(r#"{ "a": "v", "b": [ { "x": 2, "y": 1 } ] }"#).unwrap();
        assert_eq!(a, b);
        let c = json_text_checksum(r#"{"a":"v","b":[{"x":2,"y":2}]}"#).unwrap();
        assert_ne!(a, c);
        assert_eq!(a, json_checksum(&json!({"a":"v","b":[{"x":2,"y":1}]})));
    }

    #[test]
    fn parse_errors_do_not_echo_input() {
        let err = json_text_checksum("{\"secret\": AKIA").unwrap_err();
        let text = err.to_string();
        assert!(!text.contains("AKIA"), "{text}");
        assert!(text.contains("line 1"), "{text}");
    }
}
