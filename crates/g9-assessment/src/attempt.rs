//! Attempt lifecycle (ARCHITECTURE.md §8 "Attempt lifecycle"; handoff §16
//! Assessment machine; gate A04).
//!
//! An [`Attempt`] wraps the `g9_domain::AssessmentAttemptState` machine with the
//! actor rules of this crate. Every mutation takes the acting [`Actor`] and the
//! instant `now`; a refused mutation leaves the attempt untouched. Terminal
//! attempts (`ApprovedFinal`, `Voided`) accept no mutation at all; a retake is a
//! new attempt linked through `retake_of`, never an edit of the original.
//!
//! Actor rules (the single operator is both learner and human reviewer):
//!
//! | operation | permitted actors |
//! |---|---|
//! | begin, submit | the learner (an `Operator` actor whose id is the learner id) |
//! | abandon | the learner, or `System` (e.g. deadline expiry) |
//! | review_evidence, grade_provisional, void | `Operator` or `System` |
//! | approve_final | `Operator` only |
//!
//! `Model` and `Skill` actors can never drive the lifecycle (A04).

use crate::definition::{AssessmentDefinition, AssessmentVersion, DefinitionError};
use g9_domain::state::AssessmentEvent;
use g9_domain::{
    Actor, ActorId, ActorKind, AssessmentAttemptState, AssessmentId, AssistanceLevel, AttemptId,
    CompetencyId, Difficulty, G9Error, InteractionMode, Revision, TransitionError, UtcMillis,
};
use serde::{Deserialize, Serialize};

/// What was exposed to whom during an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposureKind {
    /// The task statement itself.
    Task,
    /// A hint (any assistance above `Unaided`).
    Hint,
    /// A reference solution.
    ReferenceSolution,
    /// The sealed answer key.
    AnswerKey,
}

impl ExposureKind {
    /// Every kind.
    pub const ALL: [ExposureKind; 4] = [
        ExposureKind::Task,
        ExposureKind::Hint,
        ExposureKind::ReferenceSolution,
        ExposureKind::AnswerKey,
    ];

    /// Whether exposure of this kind before submission compromises blindness.
    pub const fn compromises_blindness(self) -> bool {
        !matches!(self, ExposureKind::Task)
    }
}

/// One entry of the exposure log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureRecord {
    /// What was exposed.
    pub what: ExposureKind,
    /// When.
    pub at: UtcMillis,
    /// To whom (learner, reviewer, model, skill).
    pub actor: Actor,
}

/// Refused attempt operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttemptError {
    /// The state machine refused the edge.
    #[error(transparent)]
    Transition(#[from] TransitionError),
    /// The attempt is finalized or voided; nothing may change.
    #[error("attempt is {state}; finalized and voided attempts accept no mutation")]
    Finalized {
        /// Current (terminal) state.
        state: AssessmentAttemptState,
    },
    /// The actor kind may not perform this operation.
    #[error("actor kind {actor_kind} may not perform `{operation}` on an attempt")]
    ActorNotPermitted {
        /// Operation name.
        operation: &'static str,
        /// Actor kind label.
        actor_kind: &'static str,
    },
    /// Only the learner may perform this operation.
    #[error("only the attempt's learner may perform `{operation}`")]
    NotLearner {
        /// Operation name.
        operation: &'static str,
    },
    /// The version belongs to a different definition.
    #[error("assessment version belongs to a different definition")]
    VersionMismatch,
    /// Official assessments must run in EXAM mode.
    #[error("official assessments must run in EXAM mode")]
    OfficialRequiresExamMode,
    /// A retake can only follow a finalized or voided attempt.
    #[error("a retake can only be created from a finalized or voided attempt")]
    RetakeOfOpenAttempt,
    /// The operation is not allowed in the current state.
    #[error("`{operation}` is not allowed while the attempt is {state}")]
    InvalidState {
        /// Operation name.
        operation: &'static str,
        /// Current state.
        state: AssessmentAttemptState,
    },
    /// The definition or version is invalid or corrupted.
    #[error(transparent)]
    Definition(#[from] DefinitionError),
}

impl AttemptError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            AttemptError::Transition(_) => "assessment.attempt.transition_refused",
            AttemptError::Finalized { .. } => "assessment.attempt.finalized",
            AttemptError::ActorNotPermitted { .. } => "assessment.attempt.actor_not_permitted",
            AttemptError::NotLearner { .. } => "assessment.attempt.not_learner",
            AttemptError::VersionMismatch => "assessment.attempt.version_mismatch",
            AttemptError::OfficialRequiresExamMode => {
                "assessment.attempt.official_requires_exam_mode"
            }
            AttemptError::RetakeOfOpenAttempt => "assessment.attempt.retake_of_open_attempt",
            AttemptError::InvalidState { .. } => "assessment.attempt.invalid_state",
            AttemptError::Definition(e) => e.code(),
        }
    }
}

impl From<AttemptError> for G9Error {
    fn from(e: AttemptError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            AttemptError::Definition(inner) => inner.into(),
            AttemptError::Transition(t) => G9Error::precondition_failed(code, message)
                .with_details(serde_json::json!({ "from": t.from, "event": t.event })),
            AttemptError::Finalized { state } => G9Error::precondition_failed(code, message)
                .with_details(serde_json::json!({ "state": state })),
            AttemptError::ActorNotPermitted { .. } | AttemptError::NotLearner { .. } => {
                G9Error::unauthorized(code, message)
            }
            AttemptError::VersionMismatch | AttemptError::OfficialRequiresExamMode => {
                G9Error::invalid_input(code, message)
            }
            AttemptError::RetakeOfOpenAttempt | AttemptError::InvalidState { .. } => {
                G9Error::precondition_failed(code, message)
            }
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

const fn event_name(event: AssessmentEvent) -> &'static str {
    match event {
        AssessmentEvent::Begin => "begin",
        AssessmentEvent::Submit => "submit",
        AssessmentEvent::ReviewEvidence => "review_evidence",
        AssessmentEvent::GradeProvisional => "grade_provisional",
        AssessmentEvent::ApproveFinal => "approve_final",
        AssessmentEvent::Void => "void",
        AssessmentEvent::Abandon => "abandon",
    }
}

/// One learner's attempt at one assessment version.
///
/// Fields are private so that the only way to change state is through the
/// lifecycle methods; storage rehydrates through serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    id: AttemptId,
    definition_id: AssessmentId,
    version: u32,
    learner: ActorId,
    mode: InteractionMode,
    state: AssessmentAttemptState,
    started_at: Option<UtcMillis>,
    submitted_at: Option<UtcMillis>,
    assistance: AssistanceLevel,
    retake_of: Option<AttemptId>,
    exposure: Vec<ExposureRecord>,
    revision: Revision,
    competency_ids: Vec<CompetencyId>,
    difficulty: Difficulty,
    created_at: UtcMillis,
    updated_at: UtcMillis,
}

impl Attempt {
    /// Create an `Available` attempt for `learner` against `version` of
    /// `definition`. Official kinds require `InteractionMode::Exam`; the version
    /// must belong to the definition and pass its checksum.
    pub fn new(
        id: AttemptId,
        definition: &AssessmentDefinition,
        version: &AssessmentVersion,
        learner: ActorId,
        mode: InteractionMode,
        now: UtcMillis,
    ) -> Result<Self, AttemptError> {
        definition.validate()?;
        version.validate()?;
        version.verify_checksum()?;
        if version.definition_id != definition.id {
            return Err(AttemptError::VersionMismatch);
        }
        if let Some(required) = definition.kind.required_mode() {
            if required != mode {
                return Err(AttemptError::OfficialRequiresExamMode);
            }
        }
        Ok(Self {
            id,
            definition_id: definition.id.clone(),
            version: version.version,
            learner,
            mode,
            state: AssessmentAttemptState::Available,
            started_at: None,
            submitted_at: None,
            assistance: AssistanceLevel::Unaided,
            retake_of: None,
            exposure: Vec::new(),
            revision: Revision::INITIAL,
            competency_ids: definition.competency_ids.clone(),
            difficulty: version.difficulty,
            created_at: now,
            updated_at: now,
        })
    }

    /// Create a new attempt linked to `original` through `retake_of`. The
    /// original must be terminal (finalized or voided) and is never modified.
    /// `version` may differ from the original's (same definition, typically the
    /// same equivalence group).
    pub fn new_retake(
        id: AttemptId,
        original: &Attempt,
        version: &AssessmentVersion,
        now: UtcMillis,
    ) -> Result<Self, AttemptError> {
        if !original.state.is_terminal() {
            return Err(AttemptError::RetakeOfOpenAttempt);
        }
        version.validate()?;
        version.verify_checksum()?;
        if version.definition_id != original.definition_id {
            return Err(AttemptError::VersionMismatch);
        }
        Ok(Self {
            id,
            definition_id: original.definition_id.clone(),
            version: version.version,
            learner: original.learner.clone(),
            mode: original.mode,
            state: AssessmentAttemptState::Available,
            started_at: None,
            submitted_at: None,
            assistance: AssistanceLevel::Unaided,
            retake_of: Some(original.id.clone()),
            exposure: Vec::new(),
            revision: Revision::INITIAL,
            competency_ids: original.competency_ids.clone(),
            difficulty: version.difficulty,
            created_at: now,
            updated_at: now,
        })
    }

    /// Attempt identifier.
    pub fn id(&self) -> &AttemptId {
        &self.id
    }
    /// Definition identifier.
    pub fn definition_id(&self) -> &AssessmentId {
        &self.definition_id
    }
    /// Assessment version number.
    pub fn version(&self) -> u32 {
        self.version
    }
    /// Learner actor id.
    pub fn learner(&self) -> &ActorId {
        &self.learner
    }
    /// Interaction mode the attempt runs in (EXAM for official assessments).
    pub fn mode(&self) -> InteractionMode {
        self.mode
    }
    /// Current lifecycle state.
    pub fn state(&self) -> AssessmentAttemptState {
        self.state
    }
    /// When `begin` succeeded.
    pub fn started_at(&self) -> Option<UtcMillis> {
        self.started_at
    }
    /// When `submit` succeeded.
    pub fn submitted_at(&self) -> Option<UtcMillis> {
        self.submitted_at
    }
    /// Highest assistance level used; recorded, never erased.
    pub fn assistance(&self) -> AssistanceLevel {
        self.assistance
    }
    /// Original attempt when this is a retake.
    pub fn retake_of(&self) -> Option<&AttemptId> {
        self.retake_of.as_ref()
    }
    /// Append-only exposure log.
    pub fn exposure(&self) -> &[ExposureRecord] {
        &self.exposure
    }
    /// Optimistic-concurrency revision; increments on every accepted mutation.
    pub fn revision(&self) -> Revision {
        self.revision
    }
    /// Competencies snapshotted from the definition at creation.
    pub fn competency_ids(&self) -> &[CompetencyId] {
        &self.competency_ids
    }
    /// Difficulty snapshotted from the version at creation.
    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }
    /// Creation instant.
    pub fn created_at(&self) -> UtcMillis {
        self.created_at
    }
    /// Instant of the last accepted mutation.
    pub fn updated_at(&self) -> UtcMillis {
        self.updated_at
    }

    /// Whether the attempt is terminal (`ApprovedFinal` or `Voided`).
    pub fn is_finalized(&self) -> bool {
        self.state.is_terminal()
    }

    /// A blind attempt ran in EXAM mode, unaided, and nothing but the task was
    /// exposed before submission. Only blind attempts yield *validated*
    /// competency evidence.
    pub fn is_blind(&self) -> bool {
        if self.mode != InteractionMode::Exam || self.assistance != AssistanceLevel::Unaided {
            return false;
        }
        !self
            .exposure
            .iter()
            .any(|e| e.what.compromises_blindness() && self.submitted_at.is_none_or(|s| e.at <= s))
    }

    fn permitted(&self, event: AssessmentEvent, actor: &Actor) -> Result<(), AttemptError> {
        let operation = event_name(event);
        let refuse = || AttemptError::ActorNotPermitted {
            operation,
            actor_kind: actor_kind_label(actor),
        };
        match event {
            AssessmentEvent::Begin | AssessmentEvent::Submit => {
                if !actor.is_operator() {
                    return Err(refuse());
                }
                if actor.id != self.learner {
                    return Err(AttemptError::NotLearner { operation });
                }
                Ok(())
            }
            AssessmentEvent::Abandon => match actor.kind {
                ActorKind::Operator if actor.id == self.learner => Ok(()),
                ActorKind::Operator => Err(AttemptError::NotLearner { operation }),
                ActorKind::System => Ok(()),
                ActorKind::Skill { .. } | ActorKind::Model => Err(refuse()),
            },
            AssessmentEvent::ReviewEvidence
            | AssessmentEvent::GradeProvisional
            | AssessmentEvent::Void => match actor.kind {
                ActorKind::Operator | ActorKind::System => Ok(()),
                ActorKind::Skill { .. } | ActorKind::Model => Err(refuse()),
            },
            AssessmentEvent::ApproveFinal => {
                if actor.is_operator() {
                    Ok(())
                } else {
                    Err(refuse())
                }
            }
        }
    }

    fn apply(
        &mut self,
        event: AssessmentEvent,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<(), AttemptError> {
        self.permitted(event, actor)?;
        if self.state.is_terminal() {
            return Err(AttemptError::Finalized { state: self.state });
        }
        let next = self.state.transition(event)?;
        self.state = next;
        match event {
            AssessmentEvent::Begin => self.started_at = Some(now),
            AssessmentEvent::Submit => self.submitted_at = Some(now),
            _ => {}
        }
        self.touch(now);
        Ok(())
    }

    fn touch(&mut self, now: UtcMillis) {
        self.updated_at = now;
        self.revision = self.revision.next();
    }

    /// `Available → Started`; learner only.
    pub fn begin(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::Begin, actor, now)
    }

    /// `Started → Submitted`; learner only.
    pub fn submit(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::Submit, actor, now)
    }

    /// `Submitted | GradedProvisional → EvidenceReview`; operator or system.
    pub fn review_evidence(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::ReviewEvidence, actor, now)
    }

    /// `EvidenceReview → GradedProvisional`; operator or system.
    pub fn grade_provisional(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::GradeProvisional, actor, now)
    }

    /// `GradedProvisional → ApprovedFinal`; operator (human reviewer) only. The
    /// application layer pairs this with a final `GradeDecision` from
    /// `grading::finalize` inside one transaction.
    pub fn approve_final(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::ApproveFinal, actor, now)
    }

    /// `Submitted | EvidenceReview | GradedProvisional → Voided`; operator or system.
    pub fn void(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::Void, actor, now)
    }

    /// `Started → Voided`; learner or system.
    pub fn abandon(&mut self, actor: &Actor, now: UtcMillis) -> Result<(), AttemptError> {
        self.apply(AssessmentEvent::Abandon, actor, now)
    }

    /// Record assistance used during the attempt. The stored level only ever
    /// rises to the maximum used; it is never lowered or erased. Only valid while
    /// `Started`. Returns `true` when the level rose.
    pub fn record_assistance(
        &mut self,
        level: AssistanceLevel,
        now: UtcMillis,
    ) -> Result<bool, AttemptError> {
        if self.state.is_terminal() {
            return Err(AttemptError::Finalized { state: self.state });
        }
        if self.state != AssessmentAttemptState::Started {
            return Err(AttemptError::InvalidState {
                operation: "record_assistance",
                state: self.state,
            });
        }
        if level > self.assistance {
            self.assistance = level;
            self.touch(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Append an exposure record. Allowed in every non-terminal state (reviewers
    /// legitimately open reference solutions during evidence review).
    pub fn record_exposure(
        &mut self,
        what: ExposureKind,
        actor: &Actor,
        at: UtcMillis,
    ) -> Result<(), AttemptError> {
        if self.state.is_terminal() {
            return Err(AttemptError::Finalized { state: self.state });
        }
        self.exposure.push(ExposureRecord {
            what,
            at,
            actor: actor.clone(),
        });
        self.touch(at);
        Ok(())
    }
}
