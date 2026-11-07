//! Competency observation drafts derived from final grade decisions
//! (ARCHITECTURE.md §8 "Observations" and Addendum A item 2; handoff §9
//! "Learning dimensions"; ADR-0005/0012).
//!
//! One draft per `(competency, dimension)` pair: the score is the weighted mean
//! of the criteria mapped to that dimension, using the decision's best
//! non-advisory scores. Assistance and mode are carried verbatim from the
//! attempt so that `g9-competency` can apply independence weights.
//!
//! Every draft carries the decision's [`ReliabilityClass`] and a
//! `validation_eligible` flag. A draft is validation-eligible only when the
//! decision passed, the attempt was blind (EXAM, unaided, no key/hint
//! exposure) and the decision is not `SELF_ADJUDICATED`. A caller that turns
//! a draft with `validation_eligible == false` into a
//! `g9_competency::CompetencyObservation` must pass that observation's id in
//! the `validation_ineligible` set of
//! `g9_competency::derive_state_excluding_validation`, which then never uses
//! it as validation evidence (state at most `Independent`, `provisional`
//! flag set).

use crate::attempt::Attempt;
use crate::grading::{GateFailure, GradeDecision, ReliabilityClass, Score};
use crate::rubric::Rubric;
use g9_domain::{
    ActorId, AssistanceLevel, AttemptId, CompetencyId, Difficulty, G9Error, GradeDecisionId,
    InteractionMode, LearningDimension, Provenance, TrustClass, UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Observation derivation failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObservationError {
    /// The decision belongs to another attempt.
    #[error("grade decision belongs to a different attempt")]
    DecisionAttemptMismatch,
    /// Only final decisions produce observations.
    #[error("only final grade decisions produce competency observations")]
    DecisionNotFinal,
    /// Provenance must be authoritative (operator or deterministic system).
    #[error("provenance trust class {0:?} may not produce authoritative observations")]
    ProvenanceNotAuthoritative(TrustClass),
    /// The decision names a criterion absent from the rubric.
    #[error("criterion `{0}` in the decision is not in the rubric")]
    UnknownCriterion(String),
    /// The decision has no non-advisory score for a criterion.
    #[error("criterion `{0}` has no non-advisory score in the decision")]
    UnscoredCriterion(String),
}

impl ObservationError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            ObservationError::DecisionAttemptMismatch => {
                "assessment.observation.decision_attempt_mismatch"
            }
            ObservationError::DecisionNotFinal => "assessment.observation.decision_not_final",
            ObservationError::ProvenanceNotAuthoritative(_) => {
                "assessment.observation.provenance_not_authoritative"
            }
            ObservationError::UnknownCriterion(_) => "assessment.observation.unknown_criterion",
            ObservationError::UnscoredCriterion(_) => "assessment.observation.unscored_criterion",
        }
    }
}

impl From<ObservationError> for G9Error {
    fn from(e: ObservationError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            ObservationError::DecisionAttemptMismatch | ObservationError::UnknownCriterion(_) => {
                G9Error::invalid_input(code, message)
            }
            ObservationError::DecisionNotFinal | ObservationError::UnscoredCriterion(_) => {
                G9Error::precondition_failed(code, message)
            }
            ObservationError::ProvenanceNotAuthoritative(_) => G9Error::unauthorized(code, message),
        }
    }
}

/// A competency observation ready to be appended by `g9-competency`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationDraft {
    /// Competency observed.
    pub competency_id: CompetencyId,
    /// Learning dimension.
    pub dimension: LearningDimension,
    /// Task difficulty.
    pub difficulty: Difficulty,
    /// Highest assistance used during the attempt.
    pub assistance: AssistanceLevel,
    /// Weighted mean of the dimension's criteria (0..=100).
    pub score: Score,
    /// Interaction mode of the attempt.
    pub mode: InteractionMode,
    /// Attempt the observation comes from.
    pub source_attempt: AttemptId,
    /// Grade decision the observation comes from.
    pub decision_id: GradeDecisionId,
    /// Criteria folded into this observation, in rubric order.
    pub criterion_codes: Vec<String>,
    /// Whether the attempt was blind (EXAM, unaided, no key/hint exposure).
    pub blind: bool,
    /// Reliability class of the decision (ADR-0012).
    pub reliability_class: ReliabilityClass,
    /// Whether the decision passed under the ADR-0012 rule.
    pub decision_passed: bool,
    /// Gate failures recorded on the decision.
    pub gate_failures: Vec<GateFailure>,
    /// May be used as validation evidence: decision passed, attempt blind,
    /// class not `SELF_ADJUDICATED`. `g9-competency` must never treat a draft
    /// with `false` as validation evidence.
    pub validation_eligible: bool,
    /// Reviewer who decided.
    pub reviewer: ActorId,
    /// Decision instant.
    pub observed_at: UtcMillis,
    /// Provenance of the observation.
    pub provenance: Provenance,
}

#[derive(Default)]
struct Accumulator {
    weighted_num: u32,
    weighted_den: u32,
    plain_sum: u32,
    count: u32,
    codes: Vec<String>,
}

impl Accumulator {
    fn push(&mut self, code: &str, weight: u8, score: u8) {
        self.weighted_num += u32::from(weight) * u32::from(score);
        self.weighted_den += u32::from(weight);
        self.plain_sum += u32::from(score);
        self.count += 1;
        self.codes.push(code.to_string());
    }

    /// Weighted mean (plain mean when every weight is zero), rounded half up.
    fn mean(&self) -> u8 {
        let (num, den) = if self.weighted_den > 0 {
            (self.weighted_num, self.weighted_den)
        } else {
            (self.plain_sum, self.count)
        };
        if den == 0 {
            return 0;
        }
        let rounded = (2 * num + den) / (2 * den);
        u8::try_from(rounded.min(100)).unwrap_or(100)
    }
}

/// Derive one observation draft per `(competency, dimension)` from a final
/// decision. Dimensions with no mapped criteria produce no draft.
pub fn observations_from_decision(
    attempt: &Attempt,
    rubric: &Rubric,
    decision: &GradeDecision,
    provenance: Provenance,
) -> Result<Vec<ObservationDraft>, ObservationError> {
    if decision.attempt_id != *attempt.id() {
        return Err(ObservationError::DecisionAttemptMismatch);
    }
    if !decision.is_final {
        return Err(ObservationError::DecisionNotFinal);
    }
    if !provenance.is_trusted_for_authority() {
        return Err(ObservationError::ProvenanceNotAuthoritative(
            provenance.trust,
        ));
    }
    let mut groups: BTreeMap<LearningDimension, Accumulator> = BTreeMap::new();
    for cs in &decision.breakdown.per_criterion {
        let def = rubric
            .criterion(&cs.code)
            .ok_or_else(|| ObservationError::UnknownCriterion(cs.code.clone()))?;
        let score = cs
            .best_non_advisory
            .ok_or_else(|| ObservationError::UnscoredCriterion(cs.code.clone()))?;
        groups
            .entry(def.dimension)
            .or_default()
            .push(&cs.code, def.weight, score);
    }
    let blind = attempt.is_blind();
    let reliability_class = decision.reliability_class();
    let validation_eligible =
        decision.pass && blind && reliability_class.satisfies_validation_evidence();
    let mut drafts = Vec::with_capacity(attempt.competency_ids().len() * groups.len());
    for competency_id in attempt.competency_ids() {
        for dimension in LearningDimension::ALL {
            let Some(acc) = groups.get(&dimension) else {
                continue;
            };
            let score = Score::new(acc.mean()).unwrap_or(Score::MAX);
            drafts.push(ObservationDraft {
                competency_id: competency_id.clone(),
                dimension,
                difficulty: attempt.difficulty(),
                assistance: attempt.assistance(),
                score,
                mode: attempt.mode(),
                source_attempt: attempt.id().clone(),
                decision_id: decision.id.clone(),
                criterion_codes: acc.codes.clone(),
                blind,
                reliability_class,
                decision_passed: decision.pass,
                gate_failures: decision.gate_failures().to_vec(),
                validation_eligible,
                reviewer: decision.decided_by.clone(),
                observed_at: decision.decided_at,
                provenance: provenance.clone(),
            });
        }
    }
    Ok(drafts)
}
