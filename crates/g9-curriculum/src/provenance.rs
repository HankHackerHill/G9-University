//! Provenance gate (gate A01; correction C01; `docs/curriculum/
//! provenance-and-reconciliation.md`).
//!
//! Promotion rules:
//!
//! | From | To | Requirement checked here | Checked elsewhere |
//! |---|---|---|---|
//! | `PROPOSED_SEED` | `RECONCILED_CANDIDATE` | a [`ReconciliationReport`] with `promotion_allowed` | — |
//! | `RECONCILED_CANDIDATE` | `APPROVED_CANONICAL` | a [`ReconciliationReport`] with `promotion_allowed` | operator approval bound to the candidate checksum (`g9-app` command `curriculum.promote`, `Capability::CurriculumPromote`) |
//! | `PROPOSED_SEED` | `APPROVED_CANONICAL` | never (skips reconciliation) | — |
//! | any | an earlier state | never (provenance never regresses; corrections are new versions) | — |
//!
//! This crate cannot see approvals; it only exposes [`can_promote`] and
//! [`check_transition`] so that `g9-app` applies the same rule the tests prove.

use crate::reconcile::{ReconciliationBinding, ReconciliationReport};
use g9_domain::ManifestProvenance;
use serde::{Deserialize, Serialize};

/// Wire name of a provenance state (`PROPOSED_SEED`, `RECONCILED_CANDIDATE`,
/// `APPROVED_CANONICAL`), for messages and reports.
pub const fn provenance_label(p: ManifestProvenance) -> &'static str {
    match p {
        ManifestProvenance::ProposedSeed => "PROPOSED_SEED",
        ManifestProvenance::ReconciledCandidate => "RECONCILED_CANDIDATE",
        ManifestProvenance::ApprovedCanonical => "APPROVED_CANONICAL",
    }
}

/// Why a promotion is refused. Every reason is explicit; nothing is silently
/// downgraded or auto-approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "reason", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromotionBlocked {
    /// No reconciliation report was supplied (the original approved plan is
    /// unavailable). Blocks authoritative import; does not block fixtures.
    #[error("no reconciliation report: the original approved plan was not reconciled")]
    ReconciliationReportRequired,
    /// The original and candidate describe different curricula.
    #[error("curriculum id differs between the original and the candidate")]
    CurriculumIdMismatch,
    /// Content differs and the candidate carries no reconciliation block.
    #[error(
        "candidate differs from the original ({changed} changed, {missing} missing, {added} added day(s), {metadata} metadata change(s)) and carries no reconciliation block"
    )]
    UnreconciledDifferences {
        /// Days present in both with differing fields.
        changed: u32,
        /// Days present in the original only.
        missing: u32,
        /// Days present in the candidate only.
        added: u32,
        /// Top-level content fields that differ.
        metadata: u32,
    },
    /// The reconciliation block references a different original.
    #[error("reconciliation block references original checksum {found}, expected {expected}")]
    OriginalChecksumMismatch {
        /// Checksum of the original this report was computed against.
        expected: String,
        /// Checksum written in the candidate's reconciliation block.
        found: String,
    },
    /// The reconciliation block references a diff other than the current one.
    #[error("reconciliation block references diff checksum {found}, expected {expected}")]
    DiffChecksumMismatch {
        /// Checksum of the diff computed now.
        expected: String,
        /// Checksum written in the candidate's reconciliation block.
        found: String,
    },
    /// The requested provenance transition is not in the table.
    #[error("provenance transition {from:?} -> {to:?} is not allowed: {detail}")]
    TransitionNotAllowed {
        /// Current provenance.
        from: ManifestProvenance,
        /// Requested provenance.
        to: ManifestProvenance,
        /// Why the table has no such edge.
        detail: String,
    },
}

/// Whether a reconciliation report satisfies the promotion precondition.
/// `Ok(())` means the *content* precondition holds; the operator approval is a
/// separate, command-level requirement.
pub fn can_promote(report: &ReconciliationReport) -> Result<(), PromotionBlocked> {
    if !report.curriculum_id_matches {
        return Err(PromotionBlocked::CurriculumIdMismatch);
    }
    if !report.summary.has_content_diffs() {
        return Ok(());
    }
    match &report.binding {
        ReconciliationBinding::Matched => Ok(()),
        ReconciliationBinding::Absent => Err(PromotionBlocked::UnreconciledDifferences {
            changed: report.summary.changed,
            missing: report.summary.missing_in_candidate,
            added: report.summary.added_in_candidate,
            metadata: u32::try_from(report.summary.metadata_changes.len()).unwrap_or(u32::MAX),
        }),
        ReconciliationBinding::OriginalChecksumMismatch { found } => {
            Err(PromotionBlocked::OriginalChecksumMismatch {
                expected: report.original_checksum.as_str().to_string(),
                found: found.clone(),
            })
        }
        ReconciliationBinding::DiffChecksumMismatch { found } => {
            Err(PromotionBlocked::DiffChecksumMismatch {
                expected: report.diff_checksum.as_str().to_string(),
                found: found.clone(),
            })
        }
    }
}

/// Check a provenance transition against the table in the module docs.
/// `report` is required for every allowed transition.
pub fn check_transition(
    from: ManifestProvenance,
    to: ManifestProvenance,
    report: Option<&ReconciliationReport>,
) -> Result<(), PromotionBlocked> {
    use ManifestProvenance::*;
    match (from, to) {
        (ProposedSeed, ReconciledCandidate) | (ReconciledCandidate, ApprovedCanonical) => {
            let report = report.ok_or(PromotionBlocked::ReconciliationReportRequired)?;
            can_promote(report)
        }
        (ProposedSeed, ApprovedCanonical) => Err(PromotionBlocked::TransitionNotAllowed {
            from,
            to,
            detail: "a seed cannot become canonical without reconciliation".to_string(),
        }),
        (a, b) if a == b => Err(PromotionBlocked::TransitionNotAllowed {
            from,
            to,
            detail: "no change".to_string(),
        }),
        _ => Err(PromotionBlocked::TransitionNotAllowed {
            from,
            to,
            detail: "provenance never regresses; publish a new version instead".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_without_report_are_blocked_or_rejected() {
        use ManifestProvenance::*;
        assert_eq!(
            check_transition(ProposedSeed, ReconciledCandidate, None),
            Err(PromotionBlocked::ReconciliationReportRequired)
        );
        assert_eq!(
            check_transition(ReconciledCandidate, ApprovedCanonical, None),
            Err(PromotionBlocked::ReconciliationReportRequired)
        );
        assert!(matches!(
            check_transition(ProposedSeed, ApprovedCanonical, None),
            Err(PromotionBlocked::TransitionNotAllowed { .. })
        ));
        for (a, b) in [
            (ApprovedCanonical, ProposedSeed),
            (ApprovedCanonical, ReconciledCandidate),
            (ReconciledCandidate, ProposedSeed),
            (ProposedSeed, ProposedSeed),
            (ApprovedCanonical, ApprovedCanonical),
        ] {
            assert!(
                matches!(
                    check_transition(a, b, None),
                    Err(PromotionBlocked::TransitionNotAllowed { .. })
                ),
                "{a:?} -> {b:?}"
            );
        }
    }

    #[test]
    fn blocked_reasons_serialize_as_tagged_metadata() {
        let v = serde_json::to_value(PromotionBlocked::UnreconciledDifferences {
            changed: 1,
            missing: 2,
            added: 3,
            metadata: 0,
        })
        .unwrap();
        assert_eq!(v["reason"], "UNRECONCILED_DIFFERENCES");
        assert_eq!(v["missing"], 2);
    }
}
