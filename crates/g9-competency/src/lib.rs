#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-competency
//!
//! Competency graph, dimension-specific observations, evidence-rule state
//! derivation with replay and correction (M06), and the learner-memory
//! hypothesis ledger (M09).
//!
//! Contract: `ARCHITECTURE.md` §8 ("Observations", "Derivation (replayable
//! fold)", "Learner memory"); handoff v6.0 §9 ("Learning dimensions", "Skill
//! state (not a universal float)"), §10 ("Diagnosis → prescription loop"),
//! corrections C10 and C21, acceptance gates A05 and B05.
//!
//! Design rules enforced here:
//!
//! * Observations are append-only facts. A correction is a *new* observation
//!   that `supersedes` an earlier one; nothing is overwritten (R-003).
//! * Only `TrustClass::Operator` / `TrustClass::SystemDeterministic`
//!   observations can affect competency state. Model output is proposal-only
//!   and is rejected at the observation boundary (C10, C21).
//! * `derive_state` is a pure, replayable fold: identical inputs produce an
//!   identical projection regardless of the order in which observations are
//!   supplied (A05). State moves only along the `g9_domain::CompetencyState`
//!   transition table; it never jumps.
//! * Learner hypotheses may be proposed by any actor but are accepted,
//!   rejected or retired only by the operator; rejecting a hypothesis names
//!   every derived plan/intervention/recommendation that must be revised (B05).
//! * No I/O, no clocks, no randomness: callers pass `now`.

pub mod derive;
pub mod diagnosis;
pub mod gaps;
pub mod graph;
pub mod memory;
pub mod observation;
pub mod rules;

#[cfg(test)]
pub(crate) mod testutil;

pub use derive::{
    derive_state, derive_state_checked, CompetencyProjection, Contradiction, DimensionEstimate,
};
pub use diagnosis::{
    classify_failure, DiagnosisContext, DiagnosisOutcome, EnvironmentFlags, FailureClass,
    FailureHypothesis, InstructionFlags, TestFlags,
};
pub use gaps::{gap_analysis, missing_prerequisites_of, prerequisite_met, Gap};
pub use graph::{Competency, CompetencyGraph, CompetencyGraphSpec};
pub use memory::{
    DerivedRef, DerivedRefKind, HypothesisId, HypothesisKind, HypothesisLedger, HypothesisStatus,
    LearnerHypothesis,
};
pub use observation::{CompetencyObservation, ObservationSource};
pub use rules::DerivationRules;

use g9_domain::{ErrorCategory, G9Error, TrustClass};
use serde::{Deserialize, Serialize};

/// Errors raised by this crate. Every variant maps onto exactly one of the
/// closed `g9_domain::ErrorCategory` values via `From<CompetencyError> for G9Error`.
/// Messages never carry secrets, raw content or backtraces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompetencyError {
    /// A competency with this id is already in the graph.
    #[error("competency {id} is already defined")]
    DuplicateCompetency {
        /// Offending id.
        id: String,
    },
    /// The referenced competency is not in the graph.
    #[error("competency {id} is not defined")]
    UnknownCompetency {
        /// Missing id.
        id: String,
    },
    /// A competency definition violates a structural rule.
    #[error("invalid competency definition: {reason}")]
    InvalidCompetency {
        /// Secret-free reason.
        reason: String,
    },
    /// Adding the edge would create a prerequisite cycle.
    #[error("edge {prerequisite} -> {dependent} would create a prerequisite cycle")]
    Cycle {
        /// Proposed prerequisite.
        prerequisite: String,
        /// Proposed dependent.
        dependent: String,
    },
    /// An observation failed structural validation (score range, self-reference, ...).
    #[error("invalid observation: {reason}")]
    InvalidObservation {
        /// Secret-free reason.
        reason: String,
    },
    /// The observation's provenance is not allowed to affect competency state (C10, C21).
    #[error("observation provenance {trust:?} cannot affect competency state; only OPERATOR or SYSTEM_DETERMINISTIC evidence is authoritative")]
    UntrustedProvenance {
        /// Trust class found on the observation.
        trust: TrustClass,
    },
    /// `supersedes` references an observation that is not in the known set.
    #[error("superseded observation {id} is unknown")]
    SupersedesUnknown {
        /// Referenced id.
        id: String,
    },
    /// `supersedes` references an observation that cannot be superseded by this one.
    #[error("cannot supersede observation {id}: {reason}")]
    SupersedesInvalid {
        /// Referenced id.
        id: String,
        /// Secret-free reason.
        reason: String,
    },
    /// Derivation rules are internally inconsistent.
    #[error("invalid derivation rules: {reason}")]
    InvalidRules {
        /// Secret-free reason.
        reason: String,
    },
    /// A learner hypothesis failed validation.
    #[error("invalid hypothesis: {reason}")]
    InvalidHypothesis {
        /// Secret-free reason.
        reason: String,
    },
    /// A hypothesis with this id already exists in the ledger.
    #[error("hypothesis {id} already exists")]
    DuplicateHypothesis {
        /// Offending id.
        id: String,
    },
    /// The hypothesis is not in the ledger.
    #[error("hypothesis {id} is not in the ledger")]
    UnknownHypothesis {
        /// Missing id.
        id: String,
    },
    /// The requested hypothesis status change is not allowed from its current status.
    #[error("hypothesis {id}: {event} is not allowed in status {status}")]
    HypothesisTransition {
        /// Hypothesis id.
        id: String,
        /// Current status.
        status: String,
        /// Attempted event.
        event: String,
    },
    /// Only the operator may perform this action.
    #[error("{action} requires the operator; actor kind {actor_kind} is not allowed")]
    OperatorRequired {
        /// Action attempted.
        action: String,
        /// Kind of the actor that tried.
        actor_kind: String,
    },
    /// A derived record cannot be registered against a hypothesis in its current status.
    #[error("cannot register a derived record against hypothesis {id} in status {status}")]
    DependentNotAllowed {
        /// Hypothesis id.
        id: String,
        /// Current status.
        status: String,
    },
}

impl CompetencyError {
    /// The closed error category this error maps to.
    pub fn category(&self) -> ErrorCategory {
        match self {
            CompetencyError::DuplicateCompetency { .. }
            | CompetencyError::Cycle { .. }
            | CompetencyError::DuplicateHypothesis { .. } => ErrorCategory::Conflict,
            CompetencyError::UnknownCompetency { .. }
            | CompetencyError::SupersedesUnknown { .. }
            | CompetencyError::UnknownHypothesis { .. } => ErrorCategory::NotFound,
            CompetencyError::InvalidCompetency { .. }
            | CompetencyError::InvalidObservation { .. }
            | CompetencyError::SupersedesInvalid { .. }
            | CompetencyError::InvalidRules { .. }
            | CompetencyError::InvalidHypothesis { .. } => ErrorCategory::InvalidInput,
            CompetencyError::UntrustedProvenance { .. }
            | CompetencyError::OperatorRequired { .. } => ErrorCategory::Unauthorized,
            CompetencyError::HypothesisTransition { .. }
            | CompetencyError::DependentNotAllowed { .. } => ErrorCategory::PreconditionFailed,
        }
    }

    /// Stable machine-readable code (`competency.<area>.<reason>`).
    pub fn code(&self) -> &'static str {
        match self {
            CompetencyError::DuplicateCompetency { .. } => "competency.graph.duplicate",
            CompetencyError::UnknownCompetency { .. } => "competency.graph.unknown",
            CompetencyError::InvalidCompetency { .. } => "competency.graph.invalid",
            CompetencyError::Cycle { .. } => "competency.graph.cycle",
            CompetencyError::InvalidObservation { .. } => "competency.observation.invalid",
            CompetencyError::UntrustedProvenance { .. } => {
                "competency.observation.untrusted_provenance"
            }
            CompetencyError::SupersedesUnknown { .. } => {
                "competency.observation.supersedes_unknown"
            }
            CompetencyError::SupersedesInvalid { .. } => {
                "competency.observation.supersedes_invalid"
            }
            CompetencyError::InvalidRules { .. } => "competency.rules.invalid",
            CompetencyError::InvalidHypothesis { .. } => "competency.memory.invalid_hypothesis",
            CompetencyError::DuplicateHypothesis { .. } => "competency.memory.duplicate",
            CompetencyError::UnknownHypothesis { .. } => "competency.memory.unknown",
            CompetencyError::HypothesisTransition { .. } => "competency.memory.transition",
            CompetencyError::OperatorRequired { .. } => "competency.memory.operator_required",
            CompetencyError::DependentNotAllowed { .. } => {
                "competency.memory.dependent_not_allowed"
            }
        }
    }
}

impl From<CompetencyError> for G9Error {
    fn from(err: CompetencyError) -> Self {
        let message = err.to_string();
        let details = serde_json::to_value(&err).ok();
        let base = G9Error::new(err.category(), err.code(), message);
        match details {
            Some(d) => base.with_details(d),
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_maps_to_a_closed_category_and_stable_code() {
        let samples = vec![
            CompetencyError::DuplicateCompetency { id: "c".into() },
            CompetencyError::UnknownCompetency { id: "c".into() },
            CompetencyError::InvalidCompetency { reason: "r".into() },
            CompetencyError::Cycle {
                prerequisite: "a".into(),
                dependent: "b".into(),
            },
            CompetencyError::InvalidObservation { reason: "r".into() },
            CompetencyError::UntrustedProvenance {
                trust: TrustClass::ModelOutput,
            },
            CompetencyError::SupersedesUnknown { id: "o".into() },
            CompetencyError::SupersedesInvalid {
                id: "o".into(),
                reason: "r".into(),
            },
            CompetencyError::InvalidRules { reason: "r".into() },
            CompetencyError::InvalidHypothesis { reason: "r".into() },
            CompetencyError::DuplicateHypothesis { id: "h".into() },
            CompetencyError::UnknownHypothesis { id: "h".into() },
            CompetencyError::HypothesisTransition {
                id: "h".into(),
                status: "REJECTED".into(),
                event: "ACCEPT".into(),
            },
            CompetencyError::OperatorRequired {
                action: "accept".into(),
                actor_kind: "MODEL".into(),
            },
            CompetencyError::DependentNotAllowed {
                id: "h".into(),
                status: "REJECTED".into(),
            },
        ];
        for e in samples {
            let g: G9Error = e.clone().into();
            assert!(ErrorCategory::ALL.contains(&g.category), "{e:?}");
            assert!(g.code.starts_with("competency."), "{e:?}");
            assert!(g.details.is_some());
            assert!(!g.message.is_empty());
        }
        let untrusted: G9Error = CompetencyError::UntrustedProvenance {
            trust: TrustClass::ModelOutput,
        }
        .into();
        assert_eq!(untrusted.category, ErrorCategory::Unauthorized);
    }
}
