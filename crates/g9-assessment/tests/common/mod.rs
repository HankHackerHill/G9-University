//! Deterministic fixtures shared by the g9-assessment integration tests.
//! No clocks, no randomness.
#![allow(dead_code)]

use g9_assessment::{
    AssessmentDefinition, AssessmentKind, AssessmentVersion, Attempt, CriterionResult, Rubric,
    Score,
};
use g9_domain::{
    Actor, ActorId, AssessmentId, AttemptId, CompetencyId, DataDomain, Difficulty, EvidenceId,
    GradeDecisionId, GradeSource, InteractionMode, SkillId, UtcMillis,
};

/// Fixed epoch: 2026-01-01T00:00:00Z.
pub const T0: i64 = 1_767_225_600_000;
pub const MINUTE_MS: i64 = 60_000;
pub const DAY_MS: i64 = 86_400_000;

pub fn t(minutes: i64) -> UtcMillis {
    UtcMillis::from_millis(T0 + minutes * MINUTE_MS)
}

pub fn day(n: i64) -> UtcMillis {
    UtcMillis::from_millis(T0 + n * DAY_MS)
}

/// The learner. In the MVP the operator is the learner.
pub fn learner() -> Actor {
    Actor::operator(ActorId::parse("operator-1").unwrap())
}

/// A reviewer who is not the learner (mentor-reviewer persona; not in the MVP).
pub fn mentor() -> Actor {
    Actor::operator(ActorId::parse("mentor-1").unwrap())
}

pub fn system() -> Actor {
    Actor::system(ActorId::parse("checker-1").unwrap())
}

pub fn model() -> Actor {
    Actor::model(ActorId::parse("model-1").unwrap())
}

pub fn skill() -> Actor {
    Actor::skill(
        ActorId::parse("skill-actor-1").unwrap(),
        SkillId::parse("g9-exam-grader").unwrap(),
    )
}

pub fn comp(id: &str) -> CompetencyId {
    CompetencyId::parse(id).unwrap()
}

pub fn definition() -> AssessmentDefinition {
    AssessmentDefinition::new(
        AssessmentId::parse("ad-w1").unwrap(),
        "Week 1 blind practical",
        vec![comp("comp-boot"), comp("comp-logs")],
        AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )
    .unwrap()
}

pub fn version_n(def: &AssessmentDefinition, n: u32, difficulty: u8) -> AssessmentVersion {
    AssessmentVersion {
        definition_id: def.id.clone(),
        version: n,
        rubric: Rubric::baseline(),
        task_ref: EvidenceId::parse(&format!("evi-task-{n}")).unwrap(),
        sealed_key_ref: Some(g9_domain::AttachmentId::parse(&format!("att-key-{n}")).unwrap()),
        difficulty: Difficulty::new(difficulty).unwrap(),
        equivalence_group: "W1-boot".into(),
        created_at: t(0),
        checksum: String::new(),
    }
    .with_checksum()
    .unwrap()
}

pub fn version(def: &AssessmentDefinition) -> AssessmentVersion {
    version_n(def, 1, 3)
}

pub fn attempt_id(s: &str) -> AttemptId {
    AttemptId::parse(s).unwrap()
}

pub fn decision_id(s: &str) -> GradeDecisionId {
    GradeDecisionId::parse(s).unwrap()
}

/// A fresh `Available` EXAM attempt by the learner.
pub fn available(id: &str) -> (AssessmentDefinition, AssessmentVersion, Attempt) {
    let def = definition();
    let ver = version(&def);
    let a = Attempt::new(
        attempt_id(id),
        &def,
        &ver,
        learner().id,
        InteractionMode::Exam,
        t(0),
    )
    .unwrap();
    (def, ver, a)
}

/// Drive an attempt to `GradedProvisional` (learner begins/submits, operator reviews).
pub fn to_graded(a: &mut Attempt) {
    a.begin(&learner(), t(1)).unwrap();
    a.submit(&learner(), t(100)).unwrap();
    a.review_evidence(&learner(), t(101)).unwrap();
    a.grade_provisional(&learner(), t(102)).unwrap();
}

/// An attempt in `GradedProvisional`.
pub fn graded(id: &str) -> (AssessmentDefinition, AssessmentVersion, Attempt) {
    let (def, ver, mut a) = available(id);
    to_graded(&mut a);
    (def, ver, a)
}

/// One result per baseline criterion, in rubric order, from `source` recorded by `by`.
pub fn results_with(
    a: &Attempt,
    scores: [u8; 6],
    source: GradeSource,
    by: &Actor,
) -> Vec<CriterionResult> {
    Rubric::baseline()
        .criteria()
        .iter()
        .zip(scores)
        .map(|(c, s)| {
            CriterionResult::new(
                a.id().clone(),
                c.code.clone(),
                Score::new(s).unwrap(),
                source,
                by,
                t(102),
            )
            .unwrap()
        })
        .collect()
}

/// Human-reviewer results recorded by the learner (self-grading, MVP).
pub fn self_scored(a: &Attempt, scores: [u8; 6]) -> Vec<CriterionResult> {
    results_with(a, scores, GradeSource::HumanReviewer, &learner())
}

/// Human-reviewer results recorded by the mentor (external review).
pub fn mentor_scored(a: &Attempt, scores: [u8; 6]) -> Vec<CriterionResult> {
    results_with(a, scores, GradeSource::HumanReviewer, &mentor())
}

/// Deterministic-checker results.
pub fn checker_scored(a: &Attempt, scores: [u8; 6]) -> Vec<CriterionResult> {
    results_with(a, scores, GradeSource::DeterministicChecker, &system())
}

/// Baseline criterion codes in rubric order.
pub fn codes() -> Vec<String> {
    Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| c.code.clone())
        .collect()
}
