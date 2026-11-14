//! # g9-domain
//!
//! Pure, I/O-free domain vocabulary shared by every G9 MENTOR crate.
//!
//! Dependency law (handoff v6.0 §6): UI → typed IPC DTOs → application services →
//! policy + domain services → repositories/SQLite and the approved inference adapter.
//! This crate sits at the bottom of that graph and must never depend on storage,
//! the model runtime, or the UI.
//!
//! Everything here is deterministic and unit-testable. State machines are explicit
//! transition tables (§16); invariants are constants with checkers (§8); trust and
//! provenance are first-class so that no model output can be mistaken for an
//! authoritative record (§6, §12).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod actor;
pub mod assessment;
pub mod competency;
pub mod curriculum;
pub mod envelope;
pub mod events;
pub mod hashing;
pub mod ids;
pub mod mode;
pub mod policy;
pub mod privacy;
pub mod revision;
pub mod skill;
pub mod state;
pub mod text;
pub mod time;
pub mod trust;

pub use actor::{Actor, ActorKind};
pub use assessment::{BaselineRubric, GradeSource, RubricCriterion, PASS_THRESHOLD};
pub use competency::{AssistanceLevel, Confidence, Difficulty, LearningDimension};
pub use curriculum::{
    CurriculumCounts, DayOfWeek, InvariantViolation, ManifestProvenance, CURRICULUM_INVARIANTS,
};
pub use envelope::{
    CommandMeta, CommandOutcome, CommandResult, ErrorCategory, G9Error, ResourceRef, SchemaVersion,
    Scope,
};
pub use events::EventHeader;
pub use ids::*;
pub use mode::InteractionMode;
pub use policy::{Action, Capability, Decision, OperationDescriptor, ResourceKind};
pub use privacy::{Classification, DataDomain};
pub use revision::Revision;
pub use skill::{
    ResourceBudget, SkillCode, SkillFamily, SkillInvocationEnvelope, SkillOutputEnvelope,
    SkillOutputStatus,
};
pub use state::{
    ApprovalState, AssessmentAttemptState, BackupState, CompetencyState, InterventionState,
    SessionState, SkillInvocationState, SourceState, TransitionError,
};
pub use time::{CivilDateTime, DurationMs, IanaZone, TimeError, UtcMillis};
pub use trust::{Provenance, SourceKind, TrustClass};
