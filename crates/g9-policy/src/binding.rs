//! Approval binding (ARCHITECTURE.md §11):
//!
//! `binding_mac = HMAC-SHA256(install_secret, canonical_json(operation) || expected_revision || approval_id)`
//!
//! The concrete message is the canonical JSON of
//! `{"approval_id": <id>, "expected_revision": <rev|null>, "operation": <OperationDescriptor>}`
//! (object keys sorted, no whitespace; see `g9_domain::hashing::canonical_json`),
//! which frames the three parts unambiguously. The MAC is hex encoded.
//!
//! # Key handling (non-negotiable)
//!
//! * The [`BindingKey`] is the per-installation secret. It must live **only in
//!   the backend process**: loaded by `g9-app` from the OS keyring or a `0600`
//!   file (decided in `g9-app`), never sent over IPC, never logged, never
//!   serialized. `BindingKey` deliberately implements neither `Serialize` nor
//!   `Display`, and its `Debug` output is redacted.
//! * This crate never reads OS randomness. The caller generates the 32 bytes
//!   (e.g. from the OS CSPRNG at install time) and passes them to
//!   [`BindingKey::from_bytes`].
//! * Verification is constant-time (`subtle::ConstantTimeEq`).

use g9_domain::hashing::canonical_json;
use g9_domain::{ApprovalId, OperationDescriptor, Revision};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Length of a binding key in bytes.
pub const BINDING_KEY_LEN: usize = 32;

/// Length of a hex-encoded MAC.
pub const MAC_HEX_LEN: usize = 64;

/// Errors from key construction or signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    /// The supplied slice was not exactly [`BINDING_KEY_LEN`] bytes.
    #[error("binding key must be exactly {BINDING_KEY_LEN} bytes")]
    KeyLength,
    /// The HMAC implementation rejected the key (cannot happen for a 32-byte key).
    #[error("binding key rejected by the MAC implementation")]
    KeyRejected,
}

/// Per-installation HMAC key. Backend-process only; see the module docs.
#[derive(Clone)]
pub struct BindingKey([u8; BINDING_KEY_LEN]);

impl BindingKey {
    /// Wrap caller-supplied key material.
    pub const fn from_bytes(bytes: [u8; BINDING_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Wrap a slice that must be exactly 32 bytes long.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, BindingError> {
        let arr: [u8; BINDING_KEY_LEN] = bytes.try_into().map_err(|_| BindingError::KeyLength)?;
        Ok(Self(arr))
    }

    fn mac(&self) -> Result<HmacSha256, BindingError> {
        HmacSha256::new_from_slice(&self.0).map_err(|_| BindingError::KeyRejected)
    }
}

impl std::fmt::Debug for BindingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BindingKey(<redacted>)")
    }
}

impl PartialEq for BindingKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for BindingKey {}

/// The canonical message that is MACed.
pub fn binding_message(operation: &OperationDescriptor, approval_id: &ApprovalId, expected_revision: Option<Revision>) -> Vec<u8> {
    let value = serde_json::json!({
        "approval_id": approval_id,
        "expected_revision": expected_revision.map(Revision::get),
        "operation": operation,
    });
    canonical_json(&value).into_bytes()
}

/// Compute the hex HMAC-SHA256 binding an approval to an exact operation and revision.
pub fn sign(
    key: &BindingKey,
    operation: &OperationDescriptor,
    approval_id: &ApprovalId,
    expected_revision: Option<Revision>,
) -> Result<String, BindingError> {
    let mut mac = key.mac()?;
    mac.update(&binding_message(operation, approval_id, expected_revision));
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Constant-time verification of a hex MAC. Any malformed input yields `false`.
pub fn verify(
    key: &BindingKey,
    operation: &OperationDescriptor,
    approval_id: &ApprovalId,
    expected_revision: Option<Revision>,
    mac_hex: &str,
) -> bool {
    let Ok(expected) = sign(key, operation, approval_id, expected_revision) else {
        return false;
    };
    let Ok(given) = hex::decode(mac_hex) else {
        return false;
    };
    let Ok(expected_bytes) = hex::decode(expected) else {
        return false;
    };
    if given.len() != expected_bytes.len() {
        return false;
    }
    given.ct_eq(&expected_bytes).into()
}

/// Constant-time equality of two strings (equal length required).
pub fn ct_str_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{Action, DataDomain, InteractionMode, ResourceKind, ResourceRef, Scope};

    fn op() -> OperationDescriptor {
        OperationDescriptor {
            action: Action::Execute,
            resource: ResourceRef::new(ResourceKind::Process, "proc-1"),
            scope: Scope {
                domain: DataDomain::System,
                mode: InteractionMode::Maintenance,
            },
            parameters_sha256: "11".repeat(32),
        }
    }

    fn key(b: u8) -> BindingKey {
        BindingKey::from_bytes([b; 32])
    }

    #[test]
    fn sign_is_deterministic_and_verifies() {
        let id = ApprovalId::parse("apr-1").unwrap();
        let a = sign(&key(1), &op(), &id, Some(Revision::new(3))).unwrap();
        let b = sign(&key(1), &op(), &id, Some(Revision::new(3))).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), MAC_HEX_LEN);
        assert!(verify(&key(1), &op(), &id, Some(Revision::new(3)), &a));
    }

    #[test]
    fn verify_rejects_tampering() {
        let id = ApprovalId::parse("apr-1").unwrap();
        let mac = sign(&key(1), &op(), &id, Some(Revision::new(3))).unwrap();
        assert!(!verify(&key(2), &op(), &id, Some(Revision::new(3)), &mac), "wrong key");
        assert!(!verify(&key(1), &op(), &id, Some(Revision::new(4)), &mac), "wrong revision");
        assert!(!verify(&key(1), &op(), &id, None, &mac), "missing revision");
        let other = ApprovalId::parse("apr-2").unwrap();
        assert!(!verify(&key(1), &op(), &other, Some(Revision::new(3)), &mac), "wrong id");
        let mut op2 = op();
        op2.parameters_sha256 = "22".repeat(32);
        assert!(!verify(&key(1), &op2, &id, Some(Revision::new(3)), &mac), "wrong params");
        let mut flipped = mac.clone();
        flipped.replace_range(0..1, if &mac[0..1] == "a" { "b" } else { "a" });
        assert!(!verify(&key(1), &op(), &id, Some(Revision::new(3)), &flipped), "flipped nibble");
        assert!(!verify(&key(1), &op(), &id, Some(Revision::new(3)), "zz"), "not hex");
        assert!(!verify(&key(1), &op(), &id, Some(Revision::new(3)), ""), "empty");
    }

    #[test]
    fn key_is_redacted_and_length_checked() {
        assert_eq!(format!("{:?}", key(9)), "BindingKey(<redacted>)");
        assert_eq!(BindingKey::try_from_slice(&[0u8; 31]).unwrap_err(), BindingError::KeyLength);
        assert!(BindingKey::try_from_slice(&[0u8; 32]).is_ok());
        assert_eq!(key(1), key(1));
        assert_ne!(key(1), key(2));
    }

    #[test]
    fn message_is_canonical_json_with_three_parts() {
        let id = ApprovalId::parse("apr-1").unwrap();
        let msg = String::from_utf8(binding_message(&op(), &id, None)).unwrap();
        assert!(msg.starts_with("{\"approval_id\":\"apr-1\",\"expected_revision\":null,\"operation\":{"));
        assert!(ct_str_eq("abc", "abc"));
        assert!(!ct_str_eq("abc", "abd"));
        assert!(!ct_str_eq("abc", "ab"));
    }
}
