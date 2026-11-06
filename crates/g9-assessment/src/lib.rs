#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-assessment (M07)
//!
//! Rubrics, assessment versions, blind attempts, criterion results, provisional
//! vs final grade decisions, retake links and exam sealing. Pure domain crate:
//! depends only on `g9-domain`, performs no I/O, and is deterministic.
//!
//! Contracts (ARCHITECTURE.md §8; handoff §9, §16, C09, C21, A04, A07, B01):
//!
//! - [`rubric`]: a [`Rubric`] validates weights = 100, unique codes and ≥ 1
//!   critical gate; [`Rubric::baseline`] mirrors `g9_domain::BaselineRubric`.
//! - [`definition`]: [`AssessmentDefinition`] and checksummed, immutable
//!   [`AssessmentVersion`]s whose `sealed_key_ref` is always `Secret`.
//! - [`attempt`]: [`Attempt`] drives `AssessmentAttemptState` with actor rules;
//!   `ApprovedFinal`/`Voided` accept no mutation; a retake is a new linked
//!   attempt; assistance is recorded as a maximum and never erased; every
//!   exposure is logged.
//! - [`grading`]: [`CriterionResult`]s fold into a [`ScoreBreakdown`];
//!   [`finalize`] refuses model-only criteria (`GradeSource::ModelAdvisory` can
//!   never finalize) and non-operator reviewers; [`adjudicate`] supersedes
//!   instead of mutating.
//! - [`integrity`]: [`ExamContextPolicy`] keeps answer keys, reference solutions
//!   and `Secret` content out of model context in EXAM mode; [`HintPolicy`]
//!   delegates to `InteractionMode`; [`holdout_leakage_check`] detects exam
//!   items in assembled context.
//! - [`observations`]: [`observations_from_decision`] turns a final decision
//!   into per-`(competency, dimension)` observation drafts.
//!
//! Every error type converts into `g9_domain::G9Error` with a closed category.

pub mod attempt;
pub mod definition;
pub mod grading;
pub mod integrity;
pub mod observations;
pub mod rubric;

pub use attempt::{Attempt, AttemptError, ExposureKind, ExposureRecord};
pub use definition::{AssessmentDefinition, AssessmentKind, AssessmentVersion, DefinitionError};
pub use g9_domain::PASS_THRESHOLD;
pub use grading::{
    adjudicate, finalize, score_breakdown, CriterionResult, CriterionScore, GradeDecision,
    GradeError, Score, ScoreBreakdown, CRITICAL_GATE_PASS_SCORE,
};
pub use integrity::{
    ensure_no_holdout_leakage, holdout_leakage_check, ExamContextPolicy, HintPolicy, IntegrityError,
};
pub use observations::{observations_from_decision, ObservationDraft, ObservationError};
pub use rubric::{Rubric, RubricCriterionDef, RubricError, RubricSpec, REQUIRED_TOTAL_WEIGHT};
