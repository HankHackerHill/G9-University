#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Observation drafts from final decisions (ARCHITECTURE.md §8
//! "Observations", Addendum A item 2; ADR-0005/0012).

mod common;

use common::*;
use g9_assessment::{
    finalize, finalize_decision, observations_from_decision, Attempt, DecisionInput, ExposureKind,
    GateFailure, GradeDecision, ObservationError, ReliabilityClass, Rubric,
};
use g9_domain::{
    ActorId, AssistanceLevel, CompetencyId, ErrorCategory, G9Error, InteractionMode,
    LearningDimension, Provenance, SourceKind, TrustClass,
};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, BTreeSet};

fn provenance(trust: TrustClass) -> Provenance {
    Provenance {
        source_kind: SourceKind::AssessmentSubmission,
        source_ref: "grade-decision".into(),
        trust,
        content_sha256: None,
        recorded_at: t(200),
        recorded_by: ActorId::parse("operator-1").unwrap(),
        version: None,
    }
}

fn decided_by(
    a: &Attempt,
    scores: [u8; 6],
    reviewer: &g9_domain::Actor,
    gates: &[GateFailure],
) -> GradeDecision {
    let results = if reviewer.id == learner().id {
        self_scored(a, scores)
    } else {
        mentor_scored(a, scores)
    };
    finalize_decision(
        decision_id("gd-1"),
        DecisionInput {
            attempt: a,
            rubric: &Rubric::baseline(),
            results: &results,
            gate_failures: gates,
            reviewer,
            now: t(200),
        },
    )
    .unwrap()
}

#[test]
fn observations_mapping_covers_all_criteria() {
    let rubric = Rubric::baseline();
    let (_, _, a) = graded("attempt-1");
    // ie 25, rcr 20, rv 20, dv 15, ed 10, scr 10
    let d = decided_by(&a, [100, 60, 80, 90, 40, 70], &mentor(), &[]);
    let drafts =
        observations_from_decision(&a, &rubric, &d, provenance(TrustClass::Operator)).unwrap();
    // two competencies x three dimensions with criteria (no Recall criterion)
    assert_eq!(drafts.len(), 6);
    let mut covered = BTreeSet::new();
    let mut by_key: BTreeMap<(CompetencyId, LearningDimension), u8> = BTreeMap::new();
    for draft in &drafts {
        covered.extend(draft.criterion_codes.iter().cloned());
        by_key.insert(
            (draft.competency_id.clone(), draft.dimension),
            draft.score.get(),
        );
        assert_eq!(draft.source_attempt, *a.id());
        assert_eq!(draft.decision_id, d.id);
        assert_eq!(draft.mode, InteractionMode::Exam);
        assert_eq!(draft.assistance, AssistanceLevel::Unaided);
        assert_eq!(draft.difficulty.get(), 3);
        assert_eq!(draft.reviewer, mentor().id);
        assert_eq!(draft.observed_at, t(200));
    }
    // every rubric criterion feeds exactly one dimension
    let all: BTreeSet<String> = codes().into_iter().collect();
    assert_eq!(covered, all);
    for c in rubric.criteria() {
        assert_eq!(
            rubric.dimension_of(&c.code),
            Rubric::baseline_dimension(&c.code)
        );
    }
    // weighted means, rounded half up:
    // Application = (25*100 + 20*80 + 10*70) / 55 = 87.27 -> 87
    // Understanding = (20*60 + 10*40) / 30 = 53.33 -> 53
    // Transfer = 90
    let c = comp("comp-boot");
    assert_eq!(by_key[&(c.clone(), LearningDimension::Application)], 87);
    assert_eq!(by_key[&(c.clone(), LearningDimension::Understanding)], 53);
    assert_eq!(by_key[&(c.clone(), LearningDimension::Transfer)], 90);
    assert!(!by_key.contains_key(&(c, LearningDimension::Recall)));
    let app = drafts
        .iter()
        .find(|d| d.dimension == LearningDimension::Application)
        .unwrap();
    assert_eq!(
        app.criterion_codes,
        vec![
            "independent_execution".to_string(),
            "remediation_verification".to_string(),
            "scope_compliance_reporting".to_string()
        ]
    );
}

#[test]
fn self_adjudicated_drafts_are_never_validation_eligible() {
    let rubric = Rubric::baseline();
    let (_, _, a) = graded("attempt-1");
    // MVP: the operator (= learner) finalizes a perfect blind exam
    let d = decided_by(&a, [100; 6], &learner(), &[]);
    assert!(d.pass);
    assert_eq!(d.reliability_class(), ReliabilityClass::SelfAdjudicated);
    let drafts =
        observations_from_decision(&a, &rubric, &d, provenance(TrustClass::Operator)).unwrap();
    assert!(!drafts.is_empty());
    for draft in &drafts {
        assert!(draft.blind);
        assert!(draft.decision_passed);
        assert_eq!(draft.reliability_class, ReliabilityClass::SelfAdjudicated);
        assert!(!draft.validation_eligible, "{:?}", draft.dimension);
    }
    let json = serde_json::to_value(&drafts[0]).unwrap();
    assert_eq!(json["reliability_class"], "SELF_ADJUDICATED");
    assert_eq!(json["validation_eligible"], false);
}

#[test]
fn eligible_only_when_passed_blind_and_not_self_adjudicated() {
    let rubric = Rubric::baseline();
    let prov = || provenance(TrustClass::Operator);
    let eligible = |a: &Attempt, d: &GradeDecision| {
        observations_from_decision(a, &rubric, d, prov())
            .unwrap()
            .iter()
            .all(|o| o.validation_eligible)
    };

    // external review, blind, pass -> eligible
    let (_, _, a) = graded("attempt-1");
    let d = decided_by(&a, [100; 6], &mentor(), &[]);
    assert_eq!(d.reliability_class(), ReliabilityClass::ExternalReview);
    assert!(eligible(&a, &d));
    // deterministic checker, blind, pass -> eligible
    let d = finalize(
        decision_id("gd-2"),
        &a,
        &rubric,
        &checker_scored(&a, [100; 6]),
        &mentor(),
        t(200),
    )
    .unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::Deterministic);
    assert!(eligible(&a, &d));

    // failed (floor) -> not eligible
    let d = decided_by(&a, [100, 0, 100, 100, 100, 100], &mentor(), &[]);
    assert!(!d.pass);
    assert!(!eligible(&a, &d));
    // gate failure -> not eligible, and the gate travels with the draft
    let d = decided_by(
        &a,
        [100; 6],
        &mentor(),
        &[GateFailure::FabricatedOrAlteredEvidence],
    );
    let drafts = observations_from_decision(&a, &rubric, &d, prov()).unwrap();
    assert!(drafts.iter().all(|o| !o.validation_eligible
        && !o.decision_passed
        && o.gate_failures == vec![GateFailure::FabricatedOrAlteredEvidence]));

    // not blind: answer key seen before submission -> not eligible
    let (_, _, mut leaked) = available("attempt-2");
    leaked.begin(&learner(), t(1)).unwrap();
    leaked
        .record_exposure(ExposureKind::AnswerKey, &learner(), t(2))
        .unwrap();
    leaked.submit(&learner(), t(3)).unwrap();
    leaked.review_evidence(&learner(), t(4)).unwrap();
    leaked.grade_provisional(&learner(), t(5)).unwrap();
    let d = decided_by(&leaked, [100; 6], &mentor(), &[]);
    let drafts = observations_from_decision(&leaked, &rubric, &d, prov()).unwrap();
    assert!(drafts.iter().all(|o| !o.blind && !o.validation_eligible));
}

#[test]
fn observation_errors_are_explicit() {
    let rubric = Rubric::baseline();
    let (_, _, a) = graded("attempt-1");
    let d = decided_by(&a, [100; 6], &mentor(), &[]);
    // untrusted provenance
    for trust in [
        TrustClass::ModelOutput,
        TrustClass::SkillPackage,
        TrustClass::Unknown,
    ] {
        let err = observations_from_decision(&a, &rubric, &d, provenance(trust)).unwrap_err();
        assert_eq!(err, ObservationError::ProvenanceNotAuthoritative(trust));
        assert_eq!(G9Error::from(err).category, ErrorCategory::Unauthorized);
    }
    // non-final decision
    let mut provisional = d.clone();
    provisional.is_final = false;
    assert_eq!(
        observations_from_decision(&a, &rubric, &provisional, provenance(TrustClass::Operator))
            .unwrap_err(),
        ObservationError::DecisionNotFinal
    );
    // decision of another attempt
    let (_, _, other) = graded("attempt-2");
    assert_eq!(
        observations_from_decision(&other, &rubric, &d, provenance(TrustClass::Operator))
            .unwrap_err(),
        ObservationError::DecisionAttemptMismatch
    );
    // rubric that does not contain the decision's criteria
    let mut criteria = rubric.criteria().to_vec();
    criteria[0].code = "renamed".into();
    let other_rubric = Rubric::new(criteria).unwrap();
    assert_eq!(
        observations_from_decision(&a, &other_rubric, &d, provenance(TrustClass::Operator))
            .unwrap_err(),
        ObservationError::UnknownCriterion("independent_execution".into())
    );
    // system-deterministic provenance is authoritative
    assert!(observations_from_decision(
        &a,
        &rubric,
        &d,
        provenance(TrustClass::SystemDeterministic)
    )
    .is_ok());
}
