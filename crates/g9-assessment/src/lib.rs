#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-assessment (M07)
//!
//! Rubrics with anchored levels, assessment versions, blind attempts,
//! criterion results, provisional vs final grade decisions with reliability
//! classes, retake links, the exam exposure ledger and exam sealing. Pure
//! domain crate: depends only on `g9-domain`, performs no I/O, and is
//! deterministic.
//!
//! Contracts (ARCHITECTURE.md §8 and Addendum A items 1–3; ADR-0012; handoff
//! §9, §16, C09, C21, A04, A07, B01; defect register LRN-10/11/13):
//!
//! - [`rubric`]: a [`Rubric`] validates weights = 100, unique codes, ≥ 1
//!   critical gate and anchored 0–4 levels per criterion with an explicit
//!   [`LevelScale`] (standard 0/25/50/80/100) stored with the rubric, hence
//!   with the assessment version; [`Rubric::baseline`] mirrors
//!   `g9_domain::BaselineRubric`.
//! - [`definition`]: [`AssessmentDefinition`] and checksummed, immutable
//!   [`AssessmentVersion`]s whose `sealed_key_ref` is always `Secret`.
//! - [`attempt`]: [`Attempt`] drives `AssessmentAttemptState` with actor rules;
//!   `ApprovedFinal`/`Voided` accept no mutation; a retake is a new linked
//!   attempt on a different variant of equal difficulty; assistance is
//!   recorded as a maximum and never erased; every exposure is logged.
//! - [`grading`]: [`CriterionResult`]s fold into a [`ScoreBreakdown`]. PASS iff
//!   total ≥ 80 AND every criterion ≥ 50 % of its weight AND no
//!   [`GateFailure`]. [`finalize`] refuses model-only criteria
//!   (`GradeSource::ModelAdvisory` can never finalize) and non-operator
//!   reviewers; every decision records a [`ReliabilityClass`] (reviewer =
//!   learner → `SELF_ADJUDICATED`); [`adjudicate`] supersedes instead of
//!   mutating.
//! - [`exposure`]: exam item variants in fixture families, the append-only
//!   [`ExposureLedger`] (`DISPLAYED` / `AI_CONTEXT` / `PUBLIC_SOURCE_DECLARED`),
//!   [`blind_eligible`] and [`check_retake_item`].
//! - [`integrity`]: [`ExamContextPolicy`] keeps answer keys, reference solutions
//!   and `Secret` content out of model context in EXAM mode; [`HintPolicy`]
//!   delegates to `InteractionMode`; [`holdout_leakage_check`] detects exam
//!   items in assembled context.
//! - [`observations`]: [`observations_from_decision`] turns a final decision
//!   into per-`(competency, dimension)` observation drafts marked with the
//!   decision's reliability class and validation eligibility.
//! - [`reporting`]: [`pass_rates`] reports first-attempt and eventual pass
//!   rates separately.
//!
//! Every error type converts into `g9_domain::G9Error` with a closed category.

pub mod attempt;
pub mod definition;
pub mod exposure;
pub mod grading;
pub mod integrity;
pub mod observations;
pub mod reporting;
pub mod rubric;

pub use attempt::{Attempt, AttemptError, ExposureKind, ExposureRecord};
pub use definition::{AssessmentDefinition, AssessmentKind, AssessmentVersion, DefinitionError};
pub use exposure::{
    blind_eligible, check_retake_item, BlindEligibility, ExamItem, ExamItemStatus, ExposureEntry,
    ExposureError, ExposureLedger, ExposureType, Ineligibility, SIBLING_VARIANT_COOLDOWN_DAYS,
};
pub use g9_domain::PASS_THRESHOLD;
pub use grading::{
    adjudicate, adjudicate_decision, finalize, finalize_decision, meets_floor, score_breakdown,
    score_breakdown_with_gates, time_limit_gate, CriterionResult, CriterionScore, DecisionInput,
    GateFailure, GradeDecision, GradeError, ReliabilityClass, Score, ScoreBreakdown,
};
pub use integrity::{
    ensure_no_holdout_leakage, holdout_leakage_check, ExamContextPolicy, HintPolicy, IntegrityError,
};
pub use observations::{observations_from_decision, ObservationDraft, ObservationError};
pub use reporting::{pass_rates, PassRates};
pub use rubric::{
    AnchoredLevel, CriterionAnchors, LevelScale, Rubric, RubricAnchors, RubricCriterionDef,
    RubricError, RubricSpec, CRITERION_FLOOR_PERCENT, LEVEL_COUNT,
    MACHINE_CHECKABLE_TARGET_PERCENT, MAX_LEVEL, REQUIRED_TOTAL_WEIGHT, STANDARD_LEVEL_DESCRIPTORS,
    STANDARD_SETTING_NOTE,
};
