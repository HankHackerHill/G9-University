//! Model registry vocabulary (ARCHITECTURE.md §10 "Model registry"; handoff
//! §13 "Candidate selection"; corrections C06/C07).
//!
//! No model is pre-selected here and no performance figure is stored. An entry
//! records provenance facts (architecture, weights source, license, checksum,
//! context length, tokenizer/template, runtime requirements) and whether the
//! candidate went through `docs/evaluations/model-benchmark-protocol.md`.
//!
//! Loading a model is an operator-only, per-operation approved action
//! (`Capability::ModelLoad`). This crate only checks that a [`ModelLoadRequest`]
//! *carries* an approval id and that the artifact checksum matches the entry;
//! whether the approval is valid, unexpired, bound to this operation and
//! single-use is decided by `g9-policy` and consumed by `g9-app`.

use g9_domain::hashing::sha256_hex;
use g9_domain::{ActorId, ApprovalId, Capability, G9Error, ModelId, UtcMillis};
use serde::{Deserialize, Serialize};

/// License class of a weights artifact. Redistribution/derivative terms are
/// recorded separately in [`ModelRegistryEntry::redistribution_allowed`];
/// `None` there means "not yet reviewed", never "allowed".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LicenseClass {
    /// Apache License 2.0.
    Apache2,
    /// MIT License.
    Mit,
    /// A Llama community license (terms vary by version; review required).
    LlamaCommunity,
    /// Gemma terms of use (review required).
    Gemma,
    /// Research/non-commercial only; not eligible for production use.
    ResearchOnly,
    /// Any other license; the string is the license identifier as published.
    Other(String),
}

/// One candidate row of `model_registry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRegistryEntry {
    /// Registry identifier (also the `ModelRef::model_id`).
    pub model_id: ModelId,
    /// Human-readable name for the UI.
    pub display_name: String,
    /// Base architecture family as published by the weights source.
    pub architecture: String,
    /// Publisher + URL/commit/hash of the weights artifact.
    pub weights_source: String,
    /// License class as published; redistribution terms recorded separately.
    pub license: LicenseClass,
    /// Reviewed redistribution/derivative terms; `None` = not reviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redistribution_allowed: Option<bool>,
    /// Quantization label of the artifact (e.g. a GGUF type string).
    pub quantization: String,
    /// Lowercase hex SHA-256 of the artifact file.
    pub checksum_sha256: String,
    /// Context window in tokens as published for this artifact.
    pub context_length: u32,
    /// Tokenizer / chat template identifier the runtime must use.
    pub tokenizer_template: String,
    /// Runtime and version requirements (adapter name, minimum version, build flags).
    pub runtime_requirements: String,
    /// True only after the benchmark protocol ran and its report was recorded.
    pub evaluated: bool,
}

impl ModelRegistryEntry {
    /// Shape checks: non-empty provenance fields, a well-formed checksum and a
    /// positive context length. This is not an eligibility decision.
    pub fn validate(&self) -> Result<(), RegistryError> {
        let required = [
            ("display_name", &self.display_name),
            ("architecture", &self.architecture),
            ("weights_source", &self.weights_source),
            ("quantization", &self.quantization),
            ("tokenizer_template", &self.tokenizer_template),
            ("runtime_requirements", &self.runtime_requirements),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(RegistryError::InvalidEntry {
                    field,
                    reason: "must not be empty",
                });
            }
        }
        if !is_sha256_hex(&self.checksum_sha256) {
            return Err(RegistryError::InvalidEntry {
                field: "checksum_sha256",
                reason: "must be 64 lowercase hex characters",
            });
        }
        if self.context_length == 0 {
            return Err(RegistryError::InvalidEntry {
                field: "context_length",
                reason: "must be greater than zero",
            });
        }
        if let LicenseClass::Other(name) = &self.license {
            if name.trim().is_empty() {
                return Err(RegistryError::InvalidEntry {
                    field: "license",
                    reason: "OTHER license must name the license",
                });
            }
        }
        Ok(())
    }
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Compare the SHA-256 of `bytes` with `expected_hex` (case-insensitive).
/// Any mismatch rejects the artifact (handoff §12 "Model integrity").
pub fn verify_checksum(expected_hex: &str, bytes: &[u8]) -> Result<(), RegistryError> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    if !is_sha256_hex(&expected) {
        return Err(RegistryError::InvalidEntry {
            field: "checksum_sha256",
            reason: "must be 64 lowercase hex characters",
        });
    }
    let actual = sha256_hex(bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(RegistryError::ChecksumMismatch { expected, actual })
    }
}

/// Request to load a model artifact into a runtime. `approval_id` must be
/// present; its validity is decided by policy, not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLoadRequest {
    /// Registry entry to load.
    pub model_id: ModelId,
    /// Actor requesting the load (policy requires the operator).
    pub requested_by: ActorId,
    /// Approval for this exact load operation (`Capability::ModelLoad`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<ApprovalId>,
    /// When the request was issued.
    pub requested_at: UtcMillis,
}

impl ModelLoadRequest {
    /// The capability a policy decision must grant for this request.
    pub const REQUIRED_CAPABILITY: Capability = Capability::ModelLoad;

    /// Presence check only: a load without an approval id is refused before
    /// any policy evaluation or file access happens.
    pub fn require_approval(&self) -> Result<&ApprovalId, RegistryError> {
        self.approval_id
            .as_ref()
            .ok_or(RegistryError::ApprovalMissing)
    }

    /// Full pre-load check: approval present, the request names `entry`, the
    /// entry is well-formed, and `artifact_bytes` matches the registered
    /// checksum. Returns the approval id for `g9-app` to consume via policy.
    pub fn check_against(
        &self,
        entry: &ModelRegistryEntry,
        artifact_bytes: &[u8],
    ) -> Result<&ApprovalId, RegistryError> {
        let approval = self.require_approval()?;
        if self.model_id != entry.model_id {
            return Err(RegistryError::ModelMismatch);
        }
        entry.validate()?;
        verify_checksum(&entry.checksum_sha256, artifact_bytes)?;
        Ok(approval)
    }
}

/// Registry errors. Hashes are not secrets; no file paths or contents are carried.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("model registry entry invalid: {field} {reason}")]
    /// A registry entry failed its shape check.
    InvalidEntry {
        /// Field that failed.
        field: &'static str,
        /// Why it failed (static text, never a value).
        reason: &'static str,
    },
    #[error("model artifact checksum mismatch")]
    /// The artifact's SHA-256 differs from the registered one.
    ChecksumMismatch {
        /// Registered lowercase hex SHA-256.
        expected: String,
        /// Computed lowercase hex SHA-256 of the offered bytes.
        actual: String,
    },
    #[error("model load requires an approval id")]
    /// A load request carried no approval id.
    ApprovalMissing,
    #[error("model load request does not name the registry entry")]
    /// The load request names a different model than the entry.
    ModelMismatch,
}

impl From<RegistryError> for G9Error {
    fn from(e: RegistryError) -> Self {
        match e {
            RegistryError::InvalidEntry { field, reason } => G9Error::invalid_input(
                "ai.model.registry.invalid_entry",
                format!("model registry entry invalid: {field} {reason}"),
            ),
            RegistryError::ChecksumMismatch { expected, actual } => G9Error::precondition_failed(
                "ai.model.checksum_mismatch",
                "model artifact checksum does not match the registry entry; load refused",
            )
            .with_details(serde_json::json!({ "expected": expected, "actual": actual })),
            RegistryError::ApprovalMissing => G9Error::approval_required(
                "ai.model.load.approval_required",
                "loading a model requires an operator approval (MODEL_LOAD)",
            ),
            RegistryError::ModelMismatch => G9Error::invalid_input(
                "ai.model.load.model_mismatch",
                "load request does not name the registry entry",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ErrorCategory;
    use pretty_assertions::assert_eq;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    pub(crate) fn entry() -> ModelRegistryEntry {
        ModelRegistryEntry {
            model_id: ModelId::parse("candidate-a").unwrap(),
            display_name: "Candidate A".into(),
            architecture: "decoder-only transformer".into(),
            weights_source: "publisher/repo@commit".into(),
            license: LicenseClass::Apache2,
            redistribution_allowed: None,
            quantization: "Q4_K_M".into(),
            checksum_sha256: ABC_SHA256.into(),
            context_length: 8192,
            tokenizer_template: "chatml".into(),
            runtime_requirements: "llama.cpp-compatible >= build with GPU offload".into(),
            evaluated: false,
        }
    }

    #[test]
    fn checksum_match_and_mismatch() {
        assert_eq!(verify_checksum(ABC_SHA256, b"abc"), Ok(()));
        assert_eq!(
            verify_checksum(&ABC_SHA256.to_ascii_uppercase(), b"abc"),
            Ok(())
        );
        let err = verify_checksum(ABC_SHA256, b"abd").unwrap_err();
        assert!(matches!(err, RegistryError::ChecksumMismatch { .. }));
        let g: G9Error = err.into();
        assert_eq!(g.category, ErrorCategory::PreconditionFailed);
        assert!(matches!(
            verify_checksum("nothex", b"abc"),
            Err(RegistryError::InvalidEntry { .. })
        ));
    }

    #[test]
    fn entry_validation_rejects_bad_shapes() {
        assert_eq!(entry().validate(), Ok(()));
        let mut e = entry();
        e.checksum_sha256 = "ABC".into();
        assert!(e.validate().is_err());
        let mut e = entry();
        e.context_length = 0;
        assert!(e.validate().is_err());
        let mut e = entry();
        e.license = LicenseClass::Other("  ".into());
        assert!(e.validate().is_err());
        let mut e = entry();
        e.weights_source = String::new();
        assert!(e.validate().is_err());
    }

    #[test]
    fn load_requires_approval_presence_only() {
        let mut req = ModelLoadRequest {
            model_id: ModelId::parse("candidate-a").unwrap(),
            requested_by: ActorId::parse("operator").unwrap(),
            approval_id: None,
            requested_at: UtcMillis::from_millis(0),
        };
        assert_eq!(req.require_approval(), Err(RegistryError::ApprovalMissing));
        let g: G9Error = req.check_against(&entry(), b"abc").unwrap_err().into();
        assert_eq!(g.category, ErrorCategory::ApprovalRequired);
        req.approval_id = Some(ApprovalId::parse("appr-1").unwrap());
        assert_eq!(
            req.check_against(&entry(), b"abc").map(|a| a.as_str()),
            Ok("appr-1")
        );
        assert!(ModelLoadRequest::REQUIRED_CAPABILITY.operator_only());
        req.model_id = ModelId::parse("other").unwrap();
        assert_eq!(
            req.check_against(&entry(), b"abc"),
            Err(RegistryError::ModelMismatch)
        );
    }

    #[test]
    fn license_serde_round_trip() {
        let other = LicenseClass::Other("CC-BY-NC-4.0".into());
        let j = serde_json::to_value(&other).unwrap();
        assert_eq!(j, serde_json::json!({"OTHER": "CC-BY-NC-4.0"}));
        assert_eq!(serde_json::from_value::<LicenseClass>(j).unwrap(), other);
        assert_eq!(
            serde_json::to_value(LicenseClass::LlamaCommunity).unwrap(),
            serde_json::json!("LLAMA_COMMUNITY")
        );
    }
}
