//! Criterion results and grade decisions (ARCHITECTURE.md §8 "Grading" and
//! Addendum A items 1–2; ADR-0012; handoff §9, corrections C09/C21; gate A04).
//!
//! - A [`CriterionResult`] is one score for one criterion from one source.
//!   `GradeSource::ModelAdvisory` results are advisory only (C09).
//! - [`score_breakdown`] / [`score_breakdown_with_gates`] fold results into a
//!   [`ScoreBreakdown`]: per-criterion best non-advisory score and anchored
//!   level, the per-criterion floor, the weighted total, declared
//!   [`GateFailure`]s and the pass recommendation.
//! - **Pass rule (ADR-0012, non-compensatory):** PASS iff the weighted total is
//!   ≥ `PASS_THRESHOLD` (80) AND every criterion reaches ≥ 50 % of its weight
//!   ([`CRITERION_FLOOR_PERCENT`]) AND no [`GateFailure`] is recorded. A
//!   critical-gate rubric criterion fails exactly when it misses the floor.
//! - [`finalize_decision`] / [`finalize`] produce a final [`GradeDecision`] for
//!   an attempt in `GradedProvisional`; they refuse a non-operator reviewer, a
//!   criterion with only model-advisory results, or a missing criterion. A
//!   gate failure still yields a *final* decision with `pass = false`.
//! - Every decision records a [`ReliabilityClass`]; a decision whose reviewer
//!   is the attempt's learner is `SELF_ADJUDICATED` and never satisfies
//!   validation evidence (ADR-0005/0012, LRN-11).
//! - [`adjudicate_decision`] / [`adjudicate`] create a superseding decision;
//!   the prior decision is never mutated (C21, A04).
//!
//! Field placement: `GradeDecision` keeps its wave-1 shape because downstream
//! crates build it with struct literals; the reliability class and the gate
//! failures of a decision live in its `breakdown` and are read through
//! [`GradeDecision::reliability_class`] and [`GradeDecision::gate_failures`].

use crate::attempt::Attempt;
use crate::rubric::{AnchoredLevel, Rubric, CRITERION_FLOOR_PERCENT};
use g9_domain::{
    Actor, ActorId, ActorKind, AssessmentAttemptState, AttemptId, DurationMs, EvidenceId, G9Error,
    GradeDecisionId, GradeSource, LearningDimension, UtcMillis, PASS_THRESHOLD,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

/// Enumerated gate failures (ADR-0012, LRN-10). Any gate failure fails the
/// attempt regardless of the weighted total; it is recorded on the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateFailure {
    /// Work on a target or an action outside the authorized scope.
    OutOfScopeTargetOrAction,
    /// Unsafe or destructive action without authorization.
    UnsafeOrDestructiveUnauthorizedAction,
    /// Fabricated or altered evidence.
    FabricatedOrAlteredEvidence,
    /// The time limit passed without a submission inside it.
    TimeLimitExceededWithoutSubmission,
}

impl GateFailure {
    /// Every gate, in declaration order.
    pub const ALL: [GateFailure; 4] = [
        GateFailure::OutOfScopeTargetOrAction,
        GateFailure::UnsafeOrDestructiveUnauthorizedAction,
        GateFailure::FabricatedOrAlteredEvidence,
        GateFailure::TimeLimitExceededWithoutSubmission,
    ];

    /// Stable wire code (equals the serde form).
    pub const fn code(self) -> &'static str {
        match self {
            GateFailure::OutOfScopeTargetOrAction => "OUT_OF_SCOPE_TARGET_OR_ACTION",
            GateFailure::UnsafeOrDestructiveUnauthorizedAction => {
                "UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION"
            }
            GateFailure::FabricatedOrAlteredEvidence => "FABRICATED_OR_ALTERED_EVIDENCE",
            GateFailure::TimeLimitExceededWithoutSubmission => {
                "TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION"
            }
        }
    }
}

/// The time-limit gate. `elapsed_at_submission` must come from the monotonic
/// session timer (M04 `g9-calendar::timer`), never from `submitted_at -
/// started_at` wall-clock subtraction. A submission after the limit is a
/// submission that did not happen inside the limit.
pub fn time_limit_gate(
    limit: DurationMs,
    elapsed_at_submission: DurationMs,
) -> Option<GateFailure> {
    (elapsed_at_submission.as_millis() > limit.as_millis())
        .then_some(GateFailure::TimeLimitExceededWithoutSubmission)
}

fn normalize_gates(gates: &[GateFailure]) -> Vec<GateFailure> {
    gates
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Reliability class of a grade decision (ADR-0012, LRN-11;
/// `grade_decisions.reliability_class`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReliabilityClass {
    /// Every criterion was scored by a deterministic checker and the deciding
    /// reviewer is not the learner.
    Deterministic,
    /// A reviewer other than the learner decided, with at least one
    /// human-scored criterion, and the learner recorded no score.
    ExternalReview,
    /// The reviewer is the attempt's learner, or the learner recorded a score
    /// the decision rests on.
    SelfAdjudicated,
}

impl ReliabilityClass {
    /// Every class.
    pub const ALL: [ReliabilityClass; 3] = [
        ReliabilityClass::Deterministic,
        ReliabilityClass::ExternalReview,
        ReliabilityClass::SelfAdjudicated,
    ];

    /// Stable wire code (equals the serde form and the storage CHECK list).
    pub const fn code(self) -> &'static str {
        match self {
            ReliabilityClass::Deterministic => "DETERMINISTIC",
            ReliabilityClass::ExternalReview => "EXTERNAL_REVIEW",
            ReliabilityClass::SelfAdjudicated => "SELF_ADJUDICATED",
        }
    }

    /// Whether a decision of this class may supply validation evidence
    /// (`SELF_ADJUDICATED` never does; ADR-0005/0012).
    pub const fn satisfies_validation_evidence(self) -> bool {
        !matches!(self, ReliabilityClass::SelfAdjudicated)
    }

    /// Classify a decision by `reviewer` on an attempt of `learner` resting on
    /// `results` (model-advisory results and results for codes outside the
    /// rubric are ignored):
    ///
    /// 1. reviewer is the learner → `SelfAdjudicated`;
    /// 2. a non-advisory result was recorded by the learner → `SelfAdjudicated`;
    /// 3. every rubric criterion has a checker result and none has a human
    ///    result → `Deterministic`;
    /// 4. otherwise → `ExternalReview`.
    pub fn classify(
        learner: &ActorId,
        reviewer: &ActorId,
        rubric: &Rubric,
        results: &[CriterionResult],
    ) -> Self {
        if reviewer == learner {
            return ReliabilityClass::SelfAdjudicated;
        }
        let relevant: Vec<&CriterionResult> = results
            .iter()
            .filter(|r| {
                r.source != GradeSource::ModelAdvisory
                    && rubric.criterion(&r.criterion_code).is_some()
            })
            .collect();
        if relevant.iter().any(|r| &r.recorded_by == learner) {
            return ReliabilityClass::SelfAdjudicated;
        }
        let all_deterministic = rubric.criteria().iter().all(|c| {
            let mine: Vec<&&CriterionResult> = relevant
                .iter()
                .filter(|r| r.criterion_code == c.code)
                .collect();
            !mine.is_empty()
                && mine
                    .iter()
                    .all(|r| r.source == GradeSource::DeterministicChecker)
        });
        if all_deterministic {
            ReliabilityClass::Deterministic
        } else {
            ReliabilityClass::ExternalReview
        }
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

    /// Score of an anchored level on `rubric`'s level scale.
    pub fn from_level(rubric: &Rubric, level: AnchoredLevel) -> Self {
        Self(rubric.level_scale().score_for(level))
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
    /// Source of [`CriterionScore::best_non_advisory`].
    pub best_source: Option<GradeSource>,
    /// Anchored level of the best non-advisory score on the rubric's scale.
    pub level: Option<AnchoredLevel>,
    /// Highest model-advisory score, for display only.
    pub advisory: Option<u8>,
    /// Whether the best non-advisory score reaches ≥ 50 % of the weight
    /// (ADR-0012 floor); `None` while unscored.
    pub meets_floor: Option<bool>,
    /// Gate outcome for critical-gate criteria (equals `meets_floor`); `None`
    /// when the criterion is not a gate.
    pub gate_passed: Option<bool>,
}

/// Folded view of all results against a rubric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    /// One entry per rubric criterion, in rubric order.
    pub per_criterion: Vec<CriterionScore>,
    /// Weighted percentage; `None` if any criterion lacks a non-advisory result.
    pub weighted_total: Option<f64>,
    /// Every critical-gate criterion is scored and meets the floor.
    pub critical_gates_passed: bool,
    /// Every criterion is scored and meets the floor (ADR-0012).
    pub criterion_floors_met: bool,
    /// Enumerated gate failures recorded for the attempt (sorted, unique).
    #[serde(default)]
    pub gate_failures: Vec<GateFailure>,
    /// Reliability class of the decision this breakdown belongs to; `None` on
    /// a provisional breakdown that no reviewer has decided yet.
    #[serde(default)]
    pub reliability_class: Option<ReliabilityClass>,
    /// `Some(false)` as soon as a gate failure is recorded or a scored
    /// criterion misses the floor; otherwise `Some(total >= PASS_THRESHOLD)`
    /// once every criterion is scored, `None` before.
    pub pass_recommendation: Option<bool>,
}

/// Whether `score` reaches the ADR-0012 floor of a criterion of `weight`
/// (criterion points ≥ 50 % of the weight; exact integer arithmetic).
pub fn meets_floor(weight: u8, score: u8) -> bool {
    u32::from(weight) * u32::from(score) >= u32::from(weight) * u32::from(CRITERION_FLOOR_PERCENT)
}

/// Fold criterion results into a [`ScoreBreakdown`] with no gate failure
/// recorded. Results naming codes that are not in the rubric are ignored here;
/// [`finalize`] rejects them.
pub fn score_breakdown(rubric: &Rubric, results: &[CriterionResult]) -> ScoreBreakdown {
    score_breakdown_with_gates(rubric, results, &[])
}

/// [`score_breakdown`] with recorded gate failures.
pub fn score_breakdown_with_gates(
    rubric: &Rubric,
    results: &[CriterionResult],
    gate_failures: &[GateFailure],
) -> ScoreBreakdown {
    let scale = rubric.level_scale();
    let mut per_criterion = Vec::with_capacity(rubric.criteria().len());
    let mut weighted_sum: Option<u32> = Some(0);
    for c in rubric.criteria() {
        let mut human: Option<u8> = None;
        let mut checker: Option<u8> = None;
        let mut advisory: Option<u8> = None;
        for r in results.iter().filter(|r| r.criterion_code == c.code) {
            let s = r.score.get();
            let slot = match r.source {
                GradeSource::HumanReviewer => &mut human,
                GradeSource::DeterministicChecker => &mut checker,
                GradeSource::ModelAdvisory => &mut advisory,
            };
            *slot = Some(slot.map_or(s, |prev| prev.max(s)));
        }
        let (best, best_source) = match (human, checker) {
            (Some(h), _) => (Some(h), Some(GradeSource::HumanReviewer)),
            (None, Some(k)) => (Some(k), Some(GradeSource::DeterministicChecker)),
            (None, None) => (None, None),
        };
        let floor = best.map(|s| meets_floor(c.weight, s));
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
            best_source,
            level: best.map(|s| scale.level_for(s)),
            advisory,
            meets_floor: floor,
            gate_passed: c.critical_gate.then_some(floor == Some(true)),
        });
    }
    let total_weight = rubric.total_weight();
    let critical_gates_passed = per_criterion
        .iter()
        .filter(|c| c.critical_gate)
        .all(|c| c.gate_passed == Some(true));
    let criterion_floors_met = per_criterion.iter().all(|c| c.meets_floor == Some(true));
    let known_failure = per_criterion.iter().any(|c| c.meets_floor == Some(false));
    let gate_failures = normalize_gates(gate_failures);
    let weighted_total = match weighted_sum {
        Some(sum) if total_weight > 0 => Some(f64::from(sum) / f64::from(total_weight)),
        _ => None,
    };
    let pass_recommendation = if !gate_failures.is_empty() || known_failure {
        Some(false)
    } else {
        match weighted_sum {
            Some(sum) if total_weight > 0 => {
                Some(sum >= u32::from(PASS_THRESHOLD) * total_weight && criterion_floors_met)
            }
            _ => None,
        }
    };
    ScoreBreakdown {
        per_criterion,
        weighted_total,
        critical_gates_passed,
        criterion_floors_met,
        gate_failures,
        reliability_class: None,
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
    /// Breakdown the decision rests on, including its gate failures and
    /// reliability class.
    pub breakdown: ScoreBreakdown,
    /// Pass/fail outcome under the ADR-0012 rule.
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

impl GradeDecision {
    /// Reliability class. A decision built outside this crate without a class
    /// reads as the least-trusted `SelfAdjudicated`.
    pub fn reliability_class(&self) -> ReliabilityClass {
        self.breakdown
            .reliability_class
            .unwrap_or(ReliabilityClass::SelfAdjudicated)
    }

    /// Enumerated gate failures recorded on the decision.
    pub fn gate_failures(&self) -> &[GateFailure] {
        &self.breakdown.gate_failures
    }

    /// Whether this decision may supply validation evidence (its class is not
    /// `SelfAdjudicated`).
    pub fn satisfies_validation_evidence(&self) -> bool {
        self.reliability_class().satisfies_validation_evidence()
    }
}

/// Everything a final decision is computed from.
#[derive(Debug, Clone, Copy)]
pub struct DecisionInput<'a> {
    /// Attempt being decided.
    pub attempt: &'a Attempt,
    /// Rubric of the attempt's version.
    pub rubric: &'a Rubric,
    /// Every criterion result for the attempt.
    pub results: &'a [CriterionResult],
    /// Gate failures declared by the reviewer or detected by the system
    /// (e.g. [`time_limit_gate`]).
    pub gate_failures: &'a [GateFailure],
    /// Deciding reviewer (must be an operator).
    pub reviewer: &'a Actor,
    /// Decision instant.
    pub now: UtcMillis,
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

fn require_operator(reviewer: &Actor) -> Result<(), GradeError> {
    if reviewer.is_operator() {
        Ok(())
    } else {
        Err(GradeError::ReviewerNotOperator {
            actor_kind: actor_kind_label(reviewer),
        })
    }
}

fn decide(input: &DecisionInput<'_>) -> Result<(ScoreBreakdown, bool), GradeError> {
    validate_results(input.attempt, input.rubric, input.results)?;
    let mut breakdown =
        score_breakdown_with_gates(input.rubric, input.results, input.gate_failures);
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
    breakdown.reliability_class = Some(ReliabilityClass::classify(
        input.attempt.learner(),
        &input.reviewer.id,
        input.rubric,
        input.results,
    ));
    Ok((breakdown, pass))
}

/// Produce the final grade decision for `input.attempt`.
///
/// Rejects when the reviewer is not an `Operator`, the attempt is not in
/// `GradedProvisional`, a result belongs to another attempt or an unknown
/// criterion, a criterion has only model-advisory results (C09), or a criterion
/// has no result. A gate failure or a missed floor is still a final decision,
/// with `pass = false`. The decision records its reliability class.
pub fn finalize_decision(
    id: GradeDecisionId,
    input: DecisionInput<'_>,
) -> Result<GradeDecision, GradeError> {
    require_operator(input.reviewer)?;
    if input.attempt.state() != AssessmentAttemptState::GradedProvisional {
        return Err(GradeError::AttemptNotGradedProvisional {
            state: input.attempt.state(),
        });
    }
    let (breakdown, pass) = decide(&input)?;
    Ok(GradeDecision {
        id,
        attempt_id: input.attempt.id().clone(),
        breakdown,
        pass,
        is_final: true,
        decided_by: input.reviewer.id.clone(),
        decided_at: input.now,
        supersedes: None,
        rationale: None,
    })
}

/// [`finalize_decision`] with no gate failure declared.
pub fn finalize(
    id: GradeDecisionId,
    attempt: &Attempt,
    rubric: &Rubric,
    results: &[CriterionResult],
    reviewer: &Actor,
    now: UtcMillis,
) -> Result<GradeDecision, GradeError> {
    finalize_decision(
        id,
        DecisionInput {
            attempt,
            rubric,
            results,
            gate_failures: &[],
            reviewer,
            now,
        },
    )
}

/// Create a decision that supersedes `prior` (grading dispute, reviewer
/// correction). `prior` is never modified; the new decision is recomputed from
/// `input` under the same rules as [`finalize_decision`] (its gate failures
/// are exactly `input.gate_failures`) and records `rationale` and a freshly
/// classified reliability class. Allowed while the attempt is
/// `GradedProvisional` or `ApprovedFinal`.
pub fn adjudicate_decision(
    id: GradeDecisionId,
    prior: &GradeDecision,
    rationale: &str,
    input: DecisionInput<'_>,
) -> Result<GradeDecision, GradeError> {
    require_operator(input.reviewer)?;
    let rationale = rationale.trim();
    if rationale.is_empty() {
        return Err(GradeError::EmptyRationale);
    }
    if prior.attempt_id != *input.attempt.id() {
        return Err(GradeError::PriorAttemptMismatch);
    }
    if !prior.is_final {
        return Err(GradeError::PriorNotFinal);
    }
    if prior.id == id {
        return Err(GradeError::DecisionIdReused);
    }
    if !matches!(
        input.attempt.state(),
        AssessmentAttemptState::GradedProvisional | AssessmentAttemptState::ApprovedFinal
    ) {
        return Err(GradeError::AttemptNotAdjudicable {
            state: input.attempt.state(),
        });
    }
    let (breakdown, pass) = decide(&input)?;
    Ok(GradeDecision {
        id,
        attempt_id: input.attempt.id().clone(),
        breakdown,
        pass,
        is_final: true,
        decided_by: input.reviewer.id.clone(),
        decided_at: input.now,
        supersedes: Some(prior.id.clone()),
        rationale: Some(rationale.to_string()),
    })
}

/// [`adjudicate_decision`] that carries the prior decision's gate failures
/// forward unchanged (a gate failure is removed only by an explicit
/// [`adjudicate_decision`]).
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
    adjudicate_decision(
        id,
        prior,
        rationale,
        DecisionInput {
            attempt,
            rubric,
            results,
            gate_failures: prior.gate_failures(),
            reviewer,
            now,
        },
    )
}
