//! Criterion results and grade decisions (ARCHITECTURE.md §8 "Grading";
//! handoff §9, corrections C09/C21; gate A04).
//!
//! - A [`CriterionResult`] is one score for one criterion from one source.
//!   `GradeSource::ModelAdvisory` results are advisory only (C09).
//! - [`score_breakdown`] folds results into a [`ScoreBreakdown`]: per-criterion
//!   best non-advisory score, weighted total (only when every criterion has a
//!   non-advisory result), critical-gate outcome and pass recommendation
//!   (`>= PASS_THRESHOLD` and all gates passed).
//! - [`finalize`] produces a final [`GradeDecision`] for an attempt in
//!   `GradedProvisional`; it refuses when the reviewer is not the operator, when
//!   any criterion has only model-advisory results, or when a criterion is
//!   missing. A failed critical gate still yields a *final* decision with
//!   `pass = false`.
//! - [`adjudicate`] creates a superseding decision; the prior decision is never
//!   mutated (C21, A04).

use crate::attempt::Attempt;
use crate::rubric::Rubric;
use g9_domain::{
    Actor, ActorId, ActorKind, AssessmentAttemptState, AttemptId, EvidenceId, G9Error,
    GradeDecisionId, GradeSource, LearningDimension, UtcMillis, PASS_THRESHOLD,
};
use serde::{Deserialize, Serialize};

/// A critical-gate criterion passes when its best non-advisory score reaches
/// this value (the pass threshold applied per gate criterion).
pub const CRITICAL_GATE_PASS_SCORE: u8 = PASS_THRESHOLD;

/// Grading failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GradeError {
    /// Score outside 0..=100.
    #[error("score must be within 0..=100, got {0}")]
    InvalidScore(u8),
    /// The recording actor kind cannot produce results of this source.
    #[error("actor kind {actor_kind} cannot record a {grade_source:?} result")]
    SourceActorMismatch {
        /// Claimed source.
        grade_source: GradeSource,
        /// Actor kind label.
        actor_kind: &'static str,
    },
    /// Only the operator (human reviewer) finalizes or adjudicates.
    #[error("actor kind {actor_kind} cannot finalize a grade; an operator reviewer is required")]
    ReviewerNotOperator {
        /// Actor kind label.
        actor_kind: &'static str,
    },
    /// Finalization requires `GradedProvisional`.
    #[error("attempt is {state}; finalization requires GRADED_PROVISIONAL")]
    AttemptNotGradedProvisional {
        /// Current state.
        state: AssessmentAttemptState,
    },
    /// Adjudication requires `GradedProvisional` or `ApprovedFinal`.
    #[error("attempt is {state}; adjudication requires GRADED_PROVISIONAL or APPROVED_FINAL")]
    AttemptNotAdjudicable {
        /// Current state.
        state: AssessmentAttemptState,
    },
    /// A result belongs to another attempt.
    #[error("criterion result for attempt {0} does not belong to the attempt being graded")]
    ForeignResult(AttemptId),
    /// A result names a criterion absent from the rubric.
    #[error("criterion `{0}` is not in the rubric")]
    UnknownCriterion(String),
    /// A criterion has only model-advisory results (C09).
    #[error(
        "criterion `{0}` has only model-advisory results; a checker or human result is required"
    )]
    ModelOnlyCriterion(String),
    /// A criterion has no result at all.
    #[error("criterion `{0}` has no result")]
    MissingCriterion(String),
    /// Adjudication needs a rationale.
    #[error("adjudication rationale is empty")]
    EmptyRationale,
    /// The prior decision belongs to another attempt.
    #[error("prior decision belongs to a different attempt")]
    PriorAttemptMismatch,
    /// The prior decision is not final.
    #[error("only final decisions can be adjudicated")]
    PriorNotFinal,
    /// The new decision reuses the prior decision's id.
    #[error("a superseding decision needs a new identifier")]
    DecisionIdReused,
    /// The breakdown was incomplete after validation (invariant failure).
    #[error("score breakdown incomplete after validation")]
    IncompleteBreakdown,
}

impl GradeError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            GradeError::InvalidScore(_) => "assessment.grade.invalid_score",
            GradeError::SourceActorMismatch { .. } => "assessment.grade.source_actor_mismatch",
            GradeError::ReviewerNotOperator { .. } => "assessment.grade.reviewer_not_operator",
            GradeError::AttemptNotGradedProvisional { .. } => {
                "assessment.grade.attempt_not_provisional"
            }
            GradeError::AttemptNotAdjudicable { .. } => "assessment.grade.attempt_not_adjudicable",
            GradeError::ForeignResult(_) => "assessment.grade.foreign_result",
            GradeError::UnknownCriterion(_) => "assessment.grade.unknown_criterion",
            GradeError::ModelOnlyCriterion(_) => "assessment.grade.model_only_criterion",
            GradeError::MissingCriterion(_) => "assessment.grade.missing_criterion",
            GradeError::EmptyRationale => "assessment.grade.empty_rationale",
            GradeError::PriorAttemptMismatch => "assessment.grade.prior_attempt_mismatch",
            GradeError::PriorNotFinal => "assessment.grade.prior_not_final",
            GradeError::DecisionIdReused => "assessment.grade.decision_id_reused",
            GradeError::IncompleteBreakdown => "assessment.grade.incomplete_breakdown",
        }
    }
}

impl From<GradeError> for G9Error {
    fn from(e: GradeError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            GradeError::InvalidScore(_)
            | GradeError::ForeignResult(_)
            | GradeError::UnknownCriterion(_)
            | GradeError::EmptyRationale
            | GradeError::PriorAttemptMismatch => G9Error::invalid_input(code, message),
            GradeError::SourceActorMismatch { .. } | GradeError::ReviewerNotOperator { .. } => {
                G9Error::unauthorized(code, message)
            }
            GradeError::AttemptNotGradedProvisional { state }
            | GradeError::AttemptNotAdjudicable { state } => {
                G9Error::precondition_failed(code, message)
                    .with_details(serde_json::json!({ "state": state }))
            }
            GradeError::ModelOnlyCriterion(ref c) | GradeError::MissingCriterion(ref c) => {
                G9Error::precondition_failed(code, message)
                    .with_details(serde_json::json!({ "criterion": c }))
            }
            GradeError::PriorNotFinal => G9Error::precondition_failed(code, message),
            GradeError::DecisionIdReused => G9Error::conflict(code, message),
            GradeError::IncompleteBreakdown => G9Error::internal_redacted(code),
        }
    }
}

const fn actor_kind_label(actor: &Actor) -> &'static str {
    match actor.kind {
        ActorKind::Operator => "OPERATOR",
        ActorKind::System => "SYSTEM",
        ActorKind::Skill { .. } => "SKILL",
        ActorKind::Model => "MODEL",
    }
}

/// A criterion score in 0..=100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Score(u8);

impl Score {
    /// Highest score.
    pub const MAX: Score = Score(100);
    /// Lowest score.
    pub const MIN: Score = Score(0);

    /// Validate a raw score.
    pub fn new(score: u8) -> Result<Self, GradeError> {
        if score <= 100 {
            Ok(Self(score))
        } else {
            Err(GradeError::InvalidScore(score))
        }
    }

    /// Raw value.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Score {
    type Error = GradeError;
    fn try_from(score: u8) -> Result<Self, Self::Error> {
        Self::new(score)
    }
}

impl From<Score> for u8 {
    fn from(s: Score) -> u8 {
        s.0
    }
}

/// One score for one criterion of one attempt from one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionResult {
    /// Attempt graded.
    pub attempt_id: AttemptId,
    /// Rubric criterion code.
    pub criterion_code: String,
    /// Score in 0..=100.
    pub score: Score,
    /// Who produced the score.
    pub source: GradeSource,
    /// Evidence the score rests on.
    pub evidence_refs: Vec<EvidenceId>,
    /// Free-text note (reviewer remark, checker summary). Never an instruction.
    pub note: String,
    /// When the result was recorded.
    pub recorded_at: UtcMillis,
    /// Who recorded it.
    pub recorded_by: ActorId,
}

impl CriterionResult {
    /// Record a result, checking that the actor kind matches the claimed source:
    /// `HumanReviewer` ← `Operator`, `DeterministicChecker` ← `System`,
    /// `ModelAdvisory` ← `Model` or `Skill`.
    pub fn new(
        attempt_id: AttemptId,
        criterion_code: impl Into<String>,
        score: Score,
        source: GradeSource,
        recorded_by: &Actor,
        recorded_at: UtcMillis,
    ) -> Result<Self, GradeError> {
        let consistent = matches!(
            (source, &recorded_by.kind),
            (GradeSource::HumanReviewer, ActorKind::Operator)
                | (GradeSource::DeterministicChecker, ActorKind::System)
                | (GradeSource::ModelAdvisory, ActorKind::Model)
                | (GradeSource::ModelAdvisory, ActorKind::Skill { .. })
        );
        if !consistent {
            return Err(GradeError::SourceActorMismatch {
                grade_source: source,
                actor_kind: actor_kind_label(recorded_by),
            });
        }
        Ok(Self {
            attempt_id,
            criterion_code: criterion_code.into(),
            score,
            source,
            evidence_refs: Vec::new(),
            note: String::new(),
            recorded_at,
            recorded_by: recorded_by.id.clone(),
        })
    }

    /// Attach evidence references.
    pub fn with_evidence(mut self, refs: Vec<EvidenceId>) -> Self {
        self.evidence_refs = refs;
        self
    }

    /// Attach a note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
}

/// Per-criterion view of the results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionScore {
    /// Criterion code.
    pub code: String,
    /// Criterion weight.
    pub weight: u8,
    /// Criterion dimension.
    pub dimension: LearningDimension,
    /// Whether the criterion is a critical gate.
    pub critical_gate: bool,
    /// Best non-advisory score. Human-reviewer results take precedence over
    /// checker results; among results of the same source the highest is used.
    pub best_non_advisory: Option<u8>,
    /// Highest model-advisory score, for display only.
    pub advisory: Option<u8>,
    /// Gate outcome; `None` when the criterion is not a gate.
    pub gate_passed: Option<bool>,
}

/// Folded view of all results against a rubric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    /// One entry per rubric criterion, in rubric order.
    pub per_criterion: Vec<CriterionScore>,
    /// Weighted percentage; `None` if any criterion lacks a non-advisory result.
    pub weighted_total: Option<f64>,
    /// Every critical gate has a non-advisory score ≥ [`CRITICAL_GATE_PASS_SCORE`].
    pub critical_gates_passed: bool,
    /// `Some(total >= PASS_THRESHOLD && gates passed)`; `None` while incomplete.
    pub pass_recommendation: Option<bool>,
}

/// Fold criterion results into a [`ScoreBreakdown`]. Results naming codes that
/// are not in the rubric are ignored here; [`finalize`] rejects them.
pub fn score_breakdown(rubric: &Rubric, results: &[CriterionResult]) -> ScoreBreakdown {
    let mut per_criterion = Vec::with_capacity(rubric.criteria().len());
    let mut weighted_sum: Option<u32> = Some(0);
    for c in rubric.criteria() {
        let mine = results.iter().filter(|r| r.criterion_code == c.code);
        let mut human: Option<u8> = None;
        let mut checker: Option<u8> = None;
        let mut advisory: Option<u8> = None;
        for r in mine {
            let s = r.score.get();
            let slot = match r.source {
                GradeSource::HumanReviewer => &mut human,
                GradeSource::DeterministicChecker => &mut checker,
                GradeSource::ModelAdvisory => &mut advisory,
            };
            *slot = Some(slot.map_or(s, |prev| prev.max(s)));
        }
        let best = human.or(checker);
        let gate_passed = c
            .critical_gate
            .then(|| best.is_some_and(|s| s >= CRITICAL_GATE_PASS_SCORE));
        weighted_sum = match (weighted_sum, best) {
            (Some(sum), Some(s)) => Some(sum + u32::from(c.weight) * u32::from(s)),
            _ => None,
        };
        per_criterion.push(CriterionScore {
            code: c.code.clone(),
            weight: c.weight,
            dimension: c.dimension,
            critical_gate: c.critical_gate,
            best_non_advisory: best,
            advisory,
            gate_passed,
        });
    }
    let total_weight = rubric.total_weight();
    let critical_gates_passed = per_criterion
        .iter()
        .filter(|c| c.critical_gate)
        .all(|c| c.gate_passed == Some(true));
    let (weighted_total, pass_recommendation) = match weighted_sum {
        Some(sum) if total_weight > 0 => {
            let total = f64::from(sum) / f64::from(total_weight);
            let meets_threshold = sum >= u32::from(PASS_THRESHOLD) * total_weight;
            (Some(total), Some(meets_threshold && critical_gates_passed))
        }
        _ => (None, None),
    };
    ScoreBreakdown {
        per_criterion,
        weighted_total,
        critical_gates_passed,
        pass_recommendation,
    }
}

/// A grade decision. Final decisions are immutable; a correction is a new
/// decision whose `supersedes` points at the prior one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeDecision {
    /// Decision identifier.
    pub id: GradeDecisionId,
    /// Attempt decided.
    pub attempt_id: AttemptId,
    /// Breakdown the decision rests on.
    pub breakdown: ScoreBreakdown,
    /// Pass/fail outcome (threshold and critical gates).
    pub pass: bool,
    /// Always `true` for decisions produced by this crate.
    pub is_final: bool,
    /// Reviewer who decided.
    pub decided_by: ActorId,
    /// When.
    pub decided_at: UtcMillis,
    /// Prior decision this one supersedes, for adjudications.
    pub supersedes: Option<GradeDecisionId>,
    /// Adjudication rationale, for superseding decisions.
    pub rationale: Option<String>,
}

fn validate_results(
    attempt: &Attempt,
    rubric: &Rubric,
    results: &[CriterionResult],
) -> Result<(), GradeError> {
    for r in results {
        if r.attempt_id != *attempt.id() {
            return Err(GradeError::ForeignResult(r.attempt_id.clone()));
        }
        if rubric.criterion(&r.criterion_code).is_none() {
            return Err(GradeError::UnknownCriterion(r.criterion_code.clone()));
        }
    }
    Ok(())
}

fn decide(
    attempt: &Attempt,
    rubric: &Rubric,
    results: &[CriterionResult],
) -> Result<(ScoreBreakdown, bool), GradeError> {
    validate_results(attempt, rubric, results)?;
    let breakdown = score_breakdown(rubric, results);
    for c in &breakdown.per_criterion {
        if c.best_non_advisory.is_none() {
            return Err(if c.advisory.is_some() {
                GradeError::ModelOnlyCriterion(c.code.clone())
            } else {
                GradeError::MissingCriterion(c.code.clone())
            });
        }
    }
    let pass = breakdown
        .pass_recommendation
        .ok_or(GradeError::IncompleteBreakdown)?;
    Ok((breakdown, pass))
}

/// Produce the final grade decision for `attempt`.
///
/// Rejects when the reviewer is not an `Operator`, the attempt is not in
/// `GradedProvisional`, a result belongs to another attempt or an unknown
/// criterion, a criterion has only model-advisory results (C09), or a criterion
/// has no result. A failed critical gate is still a final decision, with
/// `pass = false`.
pub fn finalize(
    id: GradeDecisionId,
    attempt: &Attempt,
    rubric: &Rubric,
    results: &[CriterionResult],
    reviewer: &Actor,
    now: UtcMillis,
) -> Result<GradeDecision, GradeError> {
    if !reviewer.is_operator() {
        return Err(GradeError::ReviewerNotOperator {
            actor_kind: actor_kind_label(reviewer),
        });
    }
    if attempt.state() != AssessmentAttemptState::GradedProvisional {
        return Err(GradeError::AttemptNotGradedProvisional {
            state: attempt.state(),
        });
    }
    let (breakdown, pass) = decide(attempt, rubric, results)?;
    Ok(GradeDecision {
        id,
        attempt_id: attempt.id().clone(),
        breakdown,
        pass,
        is_final: true,
        decided_by: reviewer.id.clone(),
        decided_at: now,
        supersedes: None,
        rationale: None,
    })
}

/// Create a decision that supersedes `prior` (grading dispute, reviewer
/// correction). `prior` is never modified; the new decision is recomputed from
/// `results` under the same rules as [`finalize`] and records `rationale`.
/// Allowed while the attempt is `GradedProvisional` or `ApprovedFinal`.
#[allow(clippy::too_many_arguments)]
pub fn adjudicate(
    id: GradeDecisionId,
    prior: &GradeDecision,
    rationale: &str,
    attempt: &Attempt,
    rubric: &Rubric,
    results: &[CriterionResult],
    reviewer: &Actor,
    now: UtcMillis,
) -> Result<GradeDecision, GradeError> {
    if !reviewer.is_operator() {
        return Err(GradeError::ReviewerNotOperator {
            actor_kind: actor_kind_label(reviewer),
        });
    }
    let rationale = rationale.trim();
    if rationale.is_empty() {
        return Err(GradeError::EmptyRationale);
    }
    if prior.attempt_id != *attempt.id() {
        return Err(GradeError::PriorAttemptMismatch);
    }
    if !prior.is_final {
        return Err(GradeError::PriorNotFinal);
    }
    if prior.id == id {
        return Err(GradeError::DecisionIdReused);
    }
    if !matches!(
        attempt.state(),
        AssessmentAttemptState::GradedProvisional | AssessmentAttemptState::ApprovedFinal
    ) {
        return Err(GradeError::AttemptNotAdjudicable {
            state: attempt.state(),
        });
    }
    let (breakdown, pass) = decide(attempt, rubric, results)?;
    Ok(GradeDecision {
        id,
        attempt_id: attempt.id().clone(),
        breakdown,
        pass,
        is_final: true,
        decided_by: reviewer.id.clone(),
        decided_at: now,
        supersedes: Some(prior.id.clone()),
        rationale: Some(rationale.to_string()),
    })
}
