//! Evidence items (M05; table `evidence_items`): typed pointers from a
//! deliverable, attempt, session or competency observation to the artefact
//! that substantiates it.
//!
//! Link rules:
//!
//! - every item carries a `data_domain` (R-017) and a `classification`;
//! - an item that substantiates an **attempt** or a **competency observation**
//!   must carry `Operator` or `SystemDeterministic` provenance — model or
//!   imported content can be *referenced by* a note, but it cannot itself be
//!   the evidence behind a grade or a competency state (ADR-0005, handoff §12
//!   "Memory poisoning");
//! - `ref_id` is a record identifier (a `NoteId`, `AttachmentId`, `AttemptId`,
//!   `SessionId`, `ObservationId` or `SourceId`), never a raw path or URL;
//! - `summary` is operator-facing text of at most [`MAX_SUMMARY_CHARS`]
//!   characters without control characters other than `\n`/`\t`.

use crate::error::EvidenceError;
use g9_domain::{Classification, DataDomain, EvidenceId, Provenance, TrustClass, UtcMillis};
use serde::{Deserialize, Serialize};

/// Maximum summary length in Unicode scalar values.
pub const MAX_SUMMARY_CHARS: usize = 500;

/// Maximum `ref_id` length in bytes (matches `g9_domain::ids`).
const MAX_REF_ID_LEN: usize = 128;

/// What an evidence item points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceKind {
    /// A note revision (`ref_id` = `NoteId`).
    Note,
    /// A stored attachment (`ref_id` = `AttachmentId`).
    Attachment,
    /// An assessment attempt (`ref_id` = `AttemptId`).
    Attempt,
    /// A study session (`ref_id` = `SessionId`).
    Session,
    /// A competency observation (`ref_id` = `ObservationId`).
    Observation,
    /// An approved research source (`ref_id` = `SourceId`).
    ExternalReference,
}

impl EvidenceKind {
    /// Kinds whose provenance must be operator or deterministic-system.
    pub const fn requires_trusted_provenance(self) -> bool {
        matches!(self, EvidenceKind::Attempt | EvidenceKind::Observation)
    }

    fn label(self) -> &'static str {
        match self {
            EvidenceKind::Note => "NOTE",
            EvidenceKind::Attachment => "ATTACHMENT",
            EvidenceKind::Attempt => "ATTEMPT",
            EvidenceKind::Session => "SESSION",
            EvidenceKind::Observation => "OBSERVATION",
            EvidenceKind::ExternalReference => "EXTERNAL_REFERENCE",
        }
    }
}

/// A validated evidence item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Evidence identifier.
    pub id: EvidenceId,
    /// What the item points at.
    pub kind: EvidenceKind,
    /// Identifier of the referenced record.
    pub ref_id: String,
    /// Privacy domain (R-017).
    pub data_domain: DataDomain,
    /// Classification of the referenced content.
    pub classification: Classification,
    /// Where the item came from.
    pub provenance: Provenance,
    /// Operator-facing summary (≤ 500 characters).
    pub summary: String,
    /// When the item was recorded.
    pub created_at: UtcMillis,
}

impl EvidenceItem {
    /// Build and validate an evidence item.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EvidenceId,
        kind: EvidenceKind,
        ref_id: impl Into<String>,
        data_domain: DataDomain,
        classification: Classification,
        provenance: Provenance,
        summary: impl Into<String>,
        created_at: UtcMillis,
    ) -> Result<Self, EvidenceError> {
        let item = Self {
            id,
            kind,
            ref_id: ref_id.into(),
            data_domain,
            classification,
            provenance,
            summary: summary.into(),
            created_at,
        };
        item.validate()?;
        Ok(item)
    }

    /// Re-check the link rules (for records loaded from storage).
    pub fn validate(&self) -> Result<(), EvidenceError> {
        validate_ref_id(&self.ref_id)?;
        validate_summary(&self.summary)?;
        if self.kind.requires_trusted_provenance() && !self.provenance.is_trusted_for_authority() {
            return Err(EvidenceError::UntrustedProvenance {
                kind: self.kind.label().to_string(),
                trust: trust_label(self.provenance.trust).to_string(),
            });
        }
        Ok(())
    }
}

fn validate_ref_id(ref_id: &str) -> Result<(), EvidenceError> {
    if ref_id.is_empty() {
        return Err(EvidenceError::RefIdInvalid {
            reason: "empty".into(),
        });
    }
    if ref_id.len() > MAX_REF_ID_LEN {
        return Err(EvidenceError::RefIdInvalid {
            reason: format!("longer than {MAX_REF_ID_LEN} bytes"),
        });
    }
    if !ref_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return Err(EvidenceError::RefIdInvalid {
            reason: "contains characters outside [A-Za-z0-9._:-]".into(),
        });
    }
    Ok(())
}

fn validate_summary(summary: &str) -> Result<(), EvidenceError> {
    if summary.trim().is_empty() {
        return Err(EvidenceError::SummaryInvalid { reason: "empty" });
    }
    if summary.chars().count() > MAX_SUMMARY_CHARS {
        return Err(EvidenceError::SummaryInvalid {
            reason: "longer than 500 characters",
        });
    }
    if summary
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(EvidenceError::SummaryInvalid {
            reason: "contains control characters",
        });
    }
    Ok(())
}

pub(crate) fn trust_label(trust: TrustClass) -> &'static str {
    match trust {
        TrustClass::Operator => "OPERATOR",
        TrustClass::SystemDeterministic => "SYSTEM_DETERMINISTIC",
        TrustClass::ApprovedReference => "APPROVED_REFERENCE",
        TrustClass::ImportedDocument => "IMPORTED_DOCUMENT",
        TrustClass::SkillPackage => "SKILL_PACKAGE",
        TrustClass::ModelOutput => "MODEL_OUTPUT",
        TrustClass::Unknown => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{ActorId, SourceKind};
    use pretty_assertions::assert_eq;

    fn prov(trust: TrustClass) -> Provenance {
        Provenance {
            source_kind: match trust {
                TrustClass::ModelOutput => SourceKind::ModelResponse,
                TrustClass::SystemDeterministic => SourceKind::SystemGenerated,
                _ => SourceKind::OperatorInput,
            },
            source_ref: "test".into(),
            trust,
            content_sha256: None,
            recorded_at: UtcMillis::from_millis(5),
            recorded_by: ActorId::parse("actor").unwrap(),
            version: None,
        }
    }

    fn item(kind: EvidenceKind, trust: TrustClass) -> Result<EvidenceItem, EvidenceError> {
        EvidenceItem::new(
            EvidenceId::parse("ev-1").unwrap(),
            kind,
            "attempt-42",
            DataDomain::Learning,
            Classification::Internal,
            prov(trust),
            "Blind practical D06 report with checker output",
            UtcMillis::from_millis(10),
        )
    }

    #[test]
    fn evidence_for_attempt_requires_trusted_provenance() {
        assert!(item(EvidenceKind::Attempt, TrustClass::Operator).is_ok());
        assert!(item(EvidenceKind::Attempt, TrustClass::SystemDeterministic).is_ok());
        for untrusted in [
            TrustClass::ModelOutput,
            TrustClass::SkillPackage,
            TrustClass::ImportedDocument,
            TrustClass::ApprovedReference,
            TrustClass::Unknown,
        ] {
            let err = item(EvidenceKind::Attempt, untrusted).unwrap_err();
            assert_eq!(
                err,
                EvidenceError::UntrustedProvenance {
                    kind: "ATTEMPT".into(),
                    trust: trust_label(untrusted).into()
                }
            );
            // Observations are evidentiary too (ADR-0005).
            assert!(matches!(
                item(EvidenceKind::Observation, untrusted),
                Err(EvidenceError::UntrustedProvenance { .. })
            ));
        }
        // Notes, attachments, sessions and references may carry any provenance.
        for kind in [
            EvidenceKind::Note,
            EvidenceKind::Attachment,
            EvidenceKind::Session,
            EvidenceKind::ExternalReference,
        ] {
            assert!(item(kind, TrustClass::ModelOutput).is_ok(), "{kind:?}");
        }
        // A deserialized item is re-validated the same way.
        let mut tampered = item(EvidenceKind::Note, TrustClass::ModelOutput).unwrap();
        tampered.kind = EvidenceKind::Attempt;
        assert!(matches!(
            tampered.validate(),
            Err(EvidenceError::UntrustedProvenance { .. })
        ));
    }

    #[test]
    fn evidence_summary_and_ref_id_are_validated() {
        let mk = |ref_id: &str, summary: &str| {
            EvidenceItem::new(
                EvidenceId::parse("ev-2").unwrap(),
                EvidenceKind::Note,
                ref_id,
                DataDomain::Learning,
                Classification::Internal,
                prov(TrustClass::Operator),
                summary,
                UtcMillis::from_millis(1),
            )
        };
        assert!(mk("note-1", "ok").is_ok());
        assert!(mk("note-1", &"x".repeat(500)).is_ok());
        assert!(mk("note-1", &"é".repeat(500)).is_ok(), "chars, not bytes");
        assert!(matches!(
            mk("note-1", &"x".repeat(501)),
            Err(EvidenceError::SummaryInvalid { .. })
        ));
        assert!(matches!(
            mk("note-1", "   "),
            Err(EvidenceError::SummaryInvalid { .. })
        ));
        assert!(matches!(
            mk("note-1", "line\u{0}break"),
            Err(EvidenceError::SummaryInvalid { .. })
        ));
        assert!(mk("note-1", "line\nbreak\ttab").is_ok());
        assert!(matches!(
            mk("", "ok"),
            Err(EvidenceError::RefIdInvalid { .. })
        ));
        assert!(matches!(
            mk("../etc/passwd", "ok"),
            Err(EvidenceError::RefIdInvalid { .. })
        ));
        assert!(matches!(
            mk("https://x.example/", "ok"),
            Err(EvidenceError::RefIdInvalid { .. })
        ));
        assert!(matches!(
            mk(&"a".repeat(129), "ok"),
            Err(EvidenceError::RefIdInvalid { .. })
        ));
    }

    #[test]
    fn evidence_item_serde_round_trip() {
        let it = item(EvidenceKind::Session, TrustClass::SystemDeterministic).unwrap();
        let json = serde_json::to_value(&it).unwrap();
        assert_eq!(json["kind"], "SESSION");
        assert_eq!(json["data_domain"], "LEARNING");
        let back: EvidenceItem = serde_json::from_value(json).unwrap();
        assert_eq!(back, it);
    }
}
