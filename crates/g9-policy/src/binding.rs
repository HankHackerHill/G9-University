//! Approval binding (ARCHITECTURE.md §11):
//!
//! `binding_mac = HMAC-SHA256(install_secret, canonical_json(operation) || expected_revision || approval_id)`
//!
//! The concrete message is the canonical JSON of
//! `{"approval_id": <id>, "expected_revision": <rev|null>, "operation": <OperationDescriptor>}`
//! (object keys sorted, no whitespace; see `g9_domain::hashing::canonical_json`),
//! which frames the three parts unambiguously. The MAC is hex encoded.
//!
//! # Keyed digests of whole requests (ADR-0021)
//!
//! The ledgers that cannot be erased (policy decisions, approval requests and decisions, the
//! idempotency records, the audit details) hold a digest of the whole request. A plain SHA-256 of
//! `{operation, payload}` confirms a guess: a four-digit body is recovered from it in 10,000 tries
//! by whoever holds the database file or a snapshot. [`BindingKey::digest_json`] makes those
//! digests keyed: `HMAC-SHA256(subkey, len(label) ‖ label ‖ canonical_json(value))`, where
//! `subkey = HMAC-SHA256(binding_key, "g9-mentor/ledger-digest-key/v1")`. The subkey keeps the key
//! that MACs approvals from ever MACing a digest too, and the [`DigestDomain`] label keeps a digest
//! made for one use from standing in for another. Without the key a digest confirms nothing.
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

/// What a keyed digest is a digest of. The label is part of the MAC input: the same JSON digested
/// for two uses gives two unrelated values, so a digest made for one use cannot stand in for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DigestDomain {
    /// The operation and payload an approval is bound to (`OperationDescriptor::parameters_sha256`).
    Parameters,
    /// The whole request as the idempotency ledger compares it, meta fields included.
    Request,
    /// The full result of a command whose stored replay record is content-free.
    Result,
    /// The input of a skill invocation, as `skill_invocations.input_sha256` records it.
    SkillInput,
    /// The validated output of a skill invocation (`skill_invocations.output_sha256`).
    SkillOutput,
}

impl DigestDomain {
    const fn label(self) -> &'static str {
        match self {
            Self::Parameters => "parameters",
            Self::Request => "request",
            Self::Result => "result",
            Self::SkillInput => "skill-input",
            Self::SkillOutput => "skill-output",
        }
    }
}

/// Domain separation of the digest subkey from the key that MACs approvals.
const DIGEST_KEY_LABEL: &[u8] = b"g9-mentor/ledger-digest-key/v1";

impl BindingKey {
    /// Keyed digest (hex, 64 characters) of the canonical JSON of `value` in `domain`; see the
    /// module docs. Deterministic for one key, unrelated across keys and across domains.
    pub fn digest_json(
        &self,
        domain: DigestDomain,
        value: &serde_json::Value,
    ) -> Result<String, BindingError> {
        let mut derive = self.mac()?;
        derive.update(DIGEST_KEY_LABEL);
        let subkey = derive.finalize().into_bytes();
        let mut mac = HmacSha256::new_from_slice(&subkey).map_err(|_| BindingError::KeyRejected)?;
        let label = domain.label().as_bytes();
        // One length byte frames the label (every label is shorter than 256 bytes).
        mac.update(&[u8::try_from(label.len()).unwrap_or(u8::MAX)]);
        mac.update(label);
        mac.update(canonical_json(value).as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
}

/// The canonical message that is MACed.
pub fn binding_message(
    operation: &OperationDescriptor,
    approval_id: &ApprovalId,
    expected_revision: Option<Revision>,
) -> Vec<u8> {
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
        assert!(
            !verify(&key(2), &op(), &id, Some(Revision::new(3)), &mac),
            "wrong key"
        );
        assert!(
            !verify(&key(1), &op(), &id, Some(Revision::new(4)), &mac),
            "wrong revision"
        );
        assert!(!verify(&key(1), &op(), &id, None, &mac), "missing revision");
        let other = ApprovalId::parse("apr-2").unwrap();
        assert!(
            !verify(&key(1), &op(), &other, Some(Revision::new(3)), &mac),
            "wrong id"
        );
        let mut op2 = op();
        op2.parameters_sha256 = "22".repeat(32);
        assert!(
            !verify(&key(1), &op2, &id, Some(Revision::new(3)), &mac),
            "wrong params"
        );
        let mut flipped = mac.clone();
        flipped.replace_range(0..1, if &mac[0..1] == "a" { "b" } else { "a" });
        assert!(
            !verify(&key(1), &op(), &id, Some(Revision::new(3)), &flipped),
            "flipped nibble"
        );
        assert!(
            !verify(&key(1), &op(), &id, Some(Revision::new(3)), "zz"),
            "not hex"
        );
        assert!(
            !verify(&key(1), &op(), &id, Some(Revision::new(3)), ""),
            "empty"
        );
    }

    #[test]
    fn key_is_redacted_and_length_checked() {
        assert_eq!(format!("{:?}", key(9)), "BindingKey(<redacted>)");
        assert_eq!(
            BindingKey::try_from_slice(&[0u8; 31]).unwrap_err(),
            BindingError::KeyLength
        );
        assert!(BindingKey::try_from_slice(&[0u8; 32]).is_ok());
        assert_eq!(key(1), key(1));
        assert_ne!(key(1), key(2));
    }

    #[test]
    fn digest_matches_an_independent_implementation() {
        // Vectors computed with Python's hmac/hashlib, not with this code:
        //   subkey = HMAC(key, b"g9-mentor/ledger-digest-key/v1")
        //   digest = HMAC(subkey, bytes([len(label)]) + label + canonical_json)
        let value = serde_json::json!({ "b": [true, null], "a": 1 });
        for (domain, expected) in [
            (
                DigestDomain::Parameters,
                "eab6df8222823dc4b53cd6b816144a3f7d617066d5952091d71cd39e8ba05846",
            ),
            (
                DigestDomain::Request,
                "9bef8a393eb6ee83cf2965e7a89cae44040199a9443c38f299d101d350beb430",
            ),
            (
                DigestDomain::Result,
                "431397c412db8c07ac0f82c515cf4b4c5480f2116943295bf93ff1f06b022140",
            ),
            (
                DigestDomain::SkillInput,
                "8164df9fa280021178c1e99120efdfa4e513350d0f94f20f20f4a7de292a4ebc",
            ),
            (
                DigestDomain::SkillOutput,
                "565ce11d51a57ef105722a71728a99e8e6b926fcd93a3b739a51e08f15799c12",
            ),
        ] {
            assert_eq!(key(1).digest_json(domain, &value).unwrap(), expected);
        }
        assert_eq!(
            key(2)
                .digest_json(DigestDomain::Parameters, &value)
                .unwrap(),
            "6cbac29bd97ac92baaeb25afa59e7ffd53d1f4d85c530c3b623c2eade24e2d38"
        );
    }

    #[test]
    fn digest_is_keyed_domain_separated_and_not_a_plain_hash() {
        use g9_domain::hashing::sha256_json;
        let a = serde_json::json!({ "title": "pin", "body": "4711" });
        let reordered = serde_json::json!({ "body": "4711", "title": "pin" });
        let digest = key(1).digest_json(DigestDomain::Parameters, &a).unwrap();
        assert_eq!(digest.len(), MAC_HEX_LEN);
        assert!(digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(
            digest,
            key(1)
                .digest_json(DigestDomain::Parameters, &reordered)
                .unwrap(),
            "canonical form: key order does not matter"
        );
        assert_ne!(
            digest,
            key(1)
                .digest_json(
                    DigestDomain::Parameters,
                    &serde_json::json!({ "title": "pin", "body": "4712" })
                )
                .unwrap(),
            "the value matters"
        );
        assert_ne!(
            digest,
            key(2).digest_json(DigestDomain::Parameters, &a).unwrap(),
            "another key gives another digest"
        );
        let others: Vec<String> = [
            DigestDomain::Request,
            DigestDomain::Result,
            DigestDomain::SkillInput,
            DigestDomain::SkillOutput,
        ]
        .into_iter()
        .map(|d| key(1).digest_json(d, &a).unwrap())
        .collect();
        let distinct: std::collections::BTreeSet<&String> = others.iter().collect();
        assert!(
            !others.contains(&digest) && distinct.len() == others.len(),
            "domains are separated"
        );
        assert_ne!(
            digest,
            sha256_json(&a),
            "not the plain SHA-256 that confirms a guess"
        );

        // Nor the MAC that binds an approval: a digest is never a MAC over the bare canonical JSON.
        let mut mac = key(1).mac().unwrap();
        mac.update(g9_domain::hashing::canonical_json(&a).as_bytes());
        assert_ne!(digest, hex::encode(mac.finalize().into_bytes()));
    }

    #[test]
    fn message_is_canonical_json_with_three_parts() {
        let id = ApprovalId::parse("apr-1").unwrap();
        let msg = String::from_utf8(binding_message(&op(), &id, None)).unwrap();
        assert!(msg
            .starts_with("{\"approval_id\":\"apr-1\",\"expected_revision\":null,\"operation\":{"));
        assert!(ct_str_eq("abc", "abc"));
        assert!(!ct_str_eq("abc", "abd"));
        assert!(!ct_str_eq("abc", "ab"));
    }
}
