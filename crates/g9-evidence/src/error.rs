//! Crate error type and its mapping to the closed `G9Error` category set.
//!
//! Messages never carry note bodies, attachment bytes, declared media-type
//! strings or path contents: they reference identifiers, versions, sizes,
//! reasons and (for refused attachments) the content hash only.

use crate::attachments::PathRejection;
use g9_domain::{G9Error, TransitionError};

/// Errors produced by the notes, attachments, evidence, classification,
/// sources and sanitize modules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvidenceError {
    /// Content is empty; empty notes/attachments are not evidence.
    #[error("content is empty")]
    EmptyContent,
    /// Content exceeds the crate limit for its kind.
    #[error("content is {actual} bytes, larger than the {limit}-byte limit")]
    ContentTooLarge {
        /// Observed size in bytes.
        actual: u64,
        /// Limit in bytes.
        limit: u64,
    },
    /// The requested note version does not exist.
    #[error("note version {version} does not exist")]
    VersionNotFound {
        /// Requested version number.
        version: u32,
    },
    /// The note version was already redacted; redaction is single-shot.
    #[error("note version {version} is already redacted")]
    AlreadyRedacted {
        /// Version number.
        version: u32,
    },
    /// A deserialized note violates the append-only version invariants.
    #[error("note version ledger is inconsistent at index {index}: {reason}")]
    VersionLedgerInconsistent {
        /// Index into `Note::versions`.
        index: usize,
        /// Which invariant failed.
        reason: &'static str,
    },
    /// The actor kind may not perform this operation.
    #[error("actor kind {actor_kind} may not {operation}")]
    ActorNotPermitted {
        /// Actor kind label (`OPERATOR`, `SYSTEM`, `SKILL`, `MODEL`).
        actor_kind: String,
        /// Operation label.
        operation: &'static str,
    },
    /// A path was rejected by the approved-roots check.
    #[error("attachment path rejected: {0}")]
    PathRejected(#[from] PathRejection),
    /// The declared media type string is malformed.
    #[error("declared media type is malformed: {reason}")]
    MediaTypeInvalid {
        /// Why the string was refused.
        reason: &'static str,
    },
    /// The attachment bytes are refused by the attachment policy (SEC-13):
    /// executables, archives and packet captures are never stored. Record the
    /// hash and a written report instead.
    #[error(
        "attachment refused by policy ({detected}); store the hash {sha256} and a report instead"
    )]
    ContentRefused {
        /// Hex SHA-256 of the refused bytes, for the operator's report.
        sha256: String,
        /// Detected content family.
        detected: &'static str,
    },
    /// The attachment is not quarantined, so there is nothing to release.
    #[error("attachment is not quarantined")]
    NotQuarantined,
    /// The evidence summary violates its constraints.
    #[error("evidence summary invalid: {reason}")]
    SummaryInvalid {
        /// Which constraint failed.
        reason: &'static str,
    },
    /// The evidence reference id is not a valid record identifier.
    #[error("evidence reference id invalid: {reason}")]
    RefIdInvalid {
        /// Which constraint failed.
        reason: String,
    },
    /// Evidence of this kind must carry operator or deterministic-system provenance.
    #[error(
        "evidence of kind {kind} requires OPERATOR or SYSTEM_DETERMINISTIC provenance, got {trust}"
    )]
    UntrustedProvenance {
        /// Evidence kind label.
        kind: String,
        /// Trust class label that was supplied.
        trust: String,
    },
    /// A research source field violates its constraints.
    #[error("source field `{field}` invalid: {reason}")]
    SourceFieldInvalid {
        /// Field name.
        field: &'static str,
        /// Which constraint failed.
        reason: &'static str,
    },
    /// A research source cannot be approved until every governance field is present.
    #[error("source record incomplete for approval; missing fields: {}", missing.join(", "))]
    SourceIncomplete {
        /// Names of the empty fields.
        missing: Vec<&'static str>,
    },
    /// A source cannot supersede itself.
    #[error("a source cannot be superseded by itself")]
    SelfSupersede,
    /// A deserialized source record violates its invariants.
    #[error("source record is inconsistent: {reason}")]
    SourceInconsistent {
        /// Which invariant failed.
        reason: &'static str,
    },
    /// A state-machine edge was refused.
    #[error(transparent)]
    Transition(#[from] TransitionError),
}

impl EvidenceError {
    /// Stable machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            EvidenceError::EmptyContent => "evidence.content_empty",
            EvidenceError::ContentTooLarge { .. } => "evidence.content_too_large",
            EvidenceError::VersionNotFound { .. } => "evidence.note_version_not_found",
            EvidenceError::AlreadyRedacted { .. } => "evidence.note_version_already_redacted",
            EvidenceError::VersionLedgerInconsistent { .. } => "evidence.note_ledger_inconsistent",
            EvidenceError::ActorNotPermitted { .. } => "evidence.actor_not_permitted",
            EvidenceError::PathRejected(_) => "evidence.path_rejected",
            EvidenceError::MediaTypeInvalid { .. } => "evidence.media_type_invalid",
            EvidenceError::ContentRefused { .. } => "evidence.attachment_refused",
            EvidenceError::NotQuarantined => "evidence.attachment_not_quarantined",
            EvidenceError::SummaryInvalid { .. } => "evidence.summary_invalid",
            EvidenceError::RefIdInvalid { .. } => "evidence.ref_id_invalid",
            EvidenceError::UntrustedProvenance { .. } => "evidence.provenance_untrusted",
            EvidenceError::SourceFieldInvalid { .. } => "evidence.source_field_invalid",
            EvidenceError::SourceIncomplete { .. } => "evidence.source_incomplete",
            EvidenceError::SelfSupersede => "evidence.source_self_supersede",
            EvidenceError::SourceInconsistent { .. } => "evidence.source_inconsistent",
            EvidenceError::Transition(_) => "evidence.transition_refused",
        }
    }
}

impl From<EvidenceError> for G9Error {
    fn from(e: EvidenceError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            EvidenceError::EmptyContent
            | EvidenceError::ContentTooLarge { .. }
            | EvidenceError::PathRejected(_)
            | EvidenceError::MediaTypeInvalid { .. }
            | EvidenceError::SummaryInvalid { .. }
            | EvidenceError::RefIdInvalid { .. }
            | EvidenceError::SourceFieldInvalid { .. }
            | EvidenceError::SelfSupersede => G9Error::invalid_input(code, message),
            EvidenceError::ContentRefused { sha256, detected } => {
                G9Error::invalid_input(code, message).with_details(serde_json::json!({
                    "sha256": sha256,
                    "detected": detected,
                }))
            }
            EvidenceError::VersionNotFound { .. } => G9Error::not_found(code, message),
            EvidenceError::AlreadyRedacted { .. }
            | EvidenceError::NotQuarantined
            | EvidenceError::SourceIncomplete { .. } => G9Error::precondition_failed(code, message),
            EvidenceError::Transition(t) => G9Error::precondition_failed(code, message)
                .with_details(serde_json::json!({ "from": t.from, "event": t.event })),
            EvidenceError::VersionLedgerInconsistent { .. }
            | EvidenceError::SourceInconsistent { .. } => G9Error::data_corruption(code, message),
            EvidenceError::ActorNotPermitted { .. } | EvidenceError::UntrustedProvenance { .. } => {
                G9Error::unauthorized(code, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ErrorCategory;

    #[test]
    fn evidence_errors_map_to_closed_categories() {
        let cases: Vec<(EvidenceError, ErrorCategory)> = vec![
            (EvidenceError::EmptyContent, ErrorCategory::InvalidInput),
            (
                EvidenceError::ContentTooLarge {
                    actual: 2,
                    limit: 1,
                },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::VersionNotFound { version: 9 },
                ErrorCategory::NotFound,
            ),
            (
                EvidenceError::AlreadyRedacted { version: 1 },
                ErrorCategory::PreconditionFailed,
            ),
            (
                EvidenceError::VersionLedgerInconsistent {
                    index: 0,
                    reason: "x",
                },
                ErrorCategory::DataCorruption,
            ),
            (
                EvidenceError::ActorNotPermitted {
                    actor_kind: "MODEL".into(),
                    operation: "redact",
                },
                ErrorCategory::Unauthorized,
            ),
            (
                EvidenceError::PathRejected(PathRejection::ParentTraversal),
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::MediaTypeInvalid { reason: "x" },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::ContentRefused {
                    sha256: "ab".into(),
                    detected: "ELF executable",
                },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::NotQuarantined,
                ErrorCategory::PreconditionFailed,
            ),
            (
                EvidenceError::SummaryInvalid { reason: "x" },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::RefIdInvalid { reason: "x".into() },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::UntrustedProvenance {
                    kind: "ATTEMPT".into(),
                    trust: "MODEL_OUTPUT".into(),
                },
                ErrorCategory::Unauthorized,
            ),
            (
                EvidenceError::SourceFieldInvalid {
                    field: "license",
                    reason: "x",
                },
                ErrorCategory::InvalidInput,
            ),
            (
                EvidenceError::SourceIncomplete {
                    missing: vec!["license"],
                },
                ErrorCategory::PreconditionFailed,
            ),
            (EvidenceError::SelfSupersede, ErrorCategory::InvalidInput),
            (
                EvidenceError::SourceInconsistent { reason: "x" },
                ErrorCategory::DataCorruption,
            ),
            (
                EvidenceError::Transition(TransitionError {
                    machine: "research_source",
                    from: "UNTRUSTED".into(),
                    event: "APPROVE".into(),
                }),
                ErrorCategory::PreconditionFailed,
            ),
        ];
        for (err, cat) in cases {
            let g: G9Error = err.clone().into();
            assert_eq!(g.category, cat, "{err:?}");
            assert!(g.code.starts_with("evidence."), "{}", g.code);
            let json = serde_json::to_value(&g).unwrap();
            assert!(json.get("backtrace").is_none());
        }
    }
}
