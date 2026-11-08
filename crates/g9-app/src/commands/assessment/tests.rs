//! `finalize_attempt` with a reviewer who is not the learner.
//!
//! Through `dispatch` the operator is both learner and reviewer, so every
//! decision is `SELF_ADJUDICATED` and never validates (see
//! `tests/grade_observations.rs`). The `EXTERNAL_REVIEW` path of ADR-0012 is
//! exercised here by calling the same `finalize_attempt` that
//! `assessment.approve_final` calls, with a learner other than the reviewer,
//! and reading the result back through `competency.projection`.

use super::*;
use crate::test_harness::{
    lab_observation, operator_id, seed_blind_practical, Harness, Opts, COMPETENCY,
};
use g9_storage::repos::competency as competency_repo;
use serde_json::json;

struct Run {
    class: DecisionClass,
    passed: bool,
    derivation: usize,
    projection: Value,
    evidence: Vec<competency_repo::ObservationEvidenceRow>,
}

/// Two earlier independent LAB observations, then a blind EXAM attempt graded
/// at `levels` and finalized by `operator-1` for the learner `learner-1`.
/// `expose_key` logs the answer key as shown before submission.
fn run(levels: [u8; 6], expose_key: bool) -> Run {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    lab_observation(&h, "obs-lab-1", 80, 3);
    lab_observation(&h, "obs-lab-2", 85, 2);

    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();
    if expose_key {
        repo::append_exposure(
            h.conn(),
            &ExposureLogRow {
                id: "ex-key-1".into(),
                assessment_version_id: "av-w1-1".into(),
                attempt_id: Some(attempt.parse().unwrap()),
                exposed_to: ExposedTo::Operator,
                actor_id: operator_id(),
                mode: g9_domain::InteractionMode::Exam,
                content_kind: ExposureContentKind::Key,
                content_sha256: "0".repeat(64),
                reason: Some("test: key shown before submission".into()),
                exposed_at: h.now(),
                created_at: h.now(),
            },
        )
        .unwrap();
    }
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let criteria: Vec<Value> = Rubric::baseline()
        .criteria()
        .iter()
        .zip(levels)
        .map(|(c, l)| {
            json!({ "criterion_code": c.code, "score": l, "source": "HUMAN_REVIEWER", "evidence_refs": [] })
        })
        .collect();
    h.call(
        "assessment.grade_provisional",
        json!({ "attempt_id": attempt, "criteria": criteria }),
    )
    .unwrap();

    let attempt_id: AttemptId = attempt.parse().unwrap();
    let learner = Actor::operator(ActorId::parse("learner-1").unwrap());
    let reviewer = Actor::operator(operator_id());
    let loaded = load(
        h.conn(),
        &ReadScope::cross_domain(g9_domain::DataDomain::Learning),
        &attempt_id,
    )
    .unwrap();
    let done = finalize_attempt(h.conn(), &loaded, &learner, &reviewer, None, h.now()).unwrap();
    let derivation = match done.derivation {
        Derivation::Derived(n) => n,
        Derivation::NotDerived(reason) => panic!("nothing derived: {reason}"),
    };
    let out = h
        .call(
            "competency.projection",
            json!({ "competency_id": COMPETENCY }),
        )
        .unwrap();
    let competency = COMPETENCY.parse().unwrap();
    Run {
        class: done.decision.reliability_class(),
        passed: done.decision.pass,
        derivation,
        projection: out["result"]["projections"][0].clone(),
        evidence: competency_repo::observation_evidence_for_competency(h.conn(), &competency)
            .unwrap(),
    }
}

/// (a) A blind EXAM pass decided by someone other than the learner, on top of
/// two earlier independent LAB observations, validates the competency and the
/// projection is not provisional. FAILS ON THE OLD CODE: nothing was derived
/// from a grade, so the state could never leave `INDEPENDENT` through an exam.
#[test]
fn external_review_blind_pass_reaches_validated_and_is_not_provisional() {
    let run = run([4; 6], false);
    assert_eq!(run.class, DecisionClass::ExternalReview);
    assert!(run.passed);
    assert_eq!(run.derivation, 3);
    assert_eq!(run.evidence.len(), 3);
    for e in &run.evidence {
        assert_eq!(e.reliability_class, ReliabilityClass::ExternalReview);
        assert!(e.blind);
        assert!(e.validation_eligible);
    }
    assert_eq!(run.projection["state"], "VALIDATED", "{}", run.projection);
    assert_eq!(run.projection["provisional"], false);
    assert_eq!(run.projection["evidence_refs"].as_array().unwrap().len(), 5);
}

/// The other two eligibility conditions hold for an external reviewer too: a
/// key shown before submission (not blind) or a failing decision never validates.
#[test]
fn external_review_never_validates_without_a_blind_pass() {
    let exposed = run([4; 6], true);
    assert_eq!(exposed.class, DecisionClass::ExternalReview);
    assert!(exposed.passed);
    assert!(exposed
        .evidence
        .iter()
        .all(|e| !e.blind && !e.validation_eligible));
    assert_eq!(exposed.projection["state"], "INDEPENDENT");

    // level 0 on root-cause reasoning misses the per-criterion floor: a fail
    let failed = run([4, 0, 4, 4, 4, 4], false);
    assert_eq!(failed.class, DecisionClass::ExternalReview);
    assert!(!failed.passed);
    assert!(failed.evidence.iter().all(|e| !e.validation_eligible));
    assert_eq!(failed.projection["state"], "INDEPENDENT");
}

/// LRN-13: the retake of a variant that was already shown is not blind, so an
/// external review of it never validates. The first attempt (graded by the
/// operator for themself) consumed the only variant; the retake passes with an
/// external reviewer on top of two independent LAB observations.
/// FAILS ON THE IMPLEMENTATION UNDER TEST: the retake was recorded as blind and
/// validation-eligible (only its own exposure rows were read), so it reached
/// `VALIDATED`.
#[test]
fn external_review_of_a_retake_on_a_shown_variant_never_validates() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    lab_observation(&h, "obs-lab-1", 80, 3);
    lab_observation(&h, "obs-lab-2", 85, 2);
    let criteria = |levels: [u8; 6]| -> Vec<Value> {
        Rubric::baseline()
            .criteria()
            .iter()
            .zip(levels)
            .map(|(c, l)| {
                json!({ "criterion_code": c.code, "score": l, "source": "HUMAN_REVIEWER", "evidence_refs": [] })
            })
            .collect()
    };
    let review = |h: &mut Harness| -> String {
        let begun = h
            .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
            .unwrap();
        let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();
        h.call(
            "assessment.submit",
            json!({ "attempt_id": attempt, "evidence_refs": [] }),
        )
        .unwrap();
        h.call(
            "assessment.review_evidence",
            json!({ "attempt_id": attempt }),
        )
        .unwrap();
        attempt
    };
    // first attempt: finalized through the command (operator = learner)
    let first = review(&mut h);
    let graded = h
        .call(
            "assessment.grade_provisional",
            json!({ "attempt_id": first, "criteria": criteria([4; 6]) }),
        )
        .unwrap();
    h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": first }),
        &Opts::revision(graded["revision"].as_u64().unwrap()),
    );

    // the retake: same (only) variant, finalized by a reviewer who is not the learner
    h.advance(60_000);
    let second = review(&mut h);
    h.call(
        "assessment.grade_provisional",
        json!({ "attempt_id": second, "criteria": criteria([4; 6]) }),
    )
    .unwrap();
    let attempt_id: AttemptId = second.parse().unwrap();
    let learner = Actor::operator(ActorId::parse("learner-1").unwrap());
    let reviewer = Actor::operator(operator_id());
    let loaded = load(
        h.conn(),
        &ReadScope::cross_domain(g9_domain::DataDomain::Learning),
        &attempt_id,
    )
    .unwrap();
    let done = finalize_attempt(h.conn(), &loaded, &learner, &reviewer, None, h.now()).unwrap();
    assert_eq!(
        done.decision.reliability_class(),
        DecisionClass::ExternalReview
    );
    assert!(done.decision.pass);

    let competency = COMPETENCY.parse().unwrap();
    let derived: Vec<_> = competency_repo::observations_for_attempt(h.conn(), &attempt_id)
        .unwrap()
        .into_iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(derived.len(), 3);
    let evidence: Vec<_> =
        competency_repo::observation_evidence_for_competency(h.conn(), &competency)
            .unwrap()
            .into_iter()
            .filter(|e| derived.contains(&e.observation_id))
            .collect();
    assert_eq!(evidence.len(), 3);
    for e in &evidence {
        assert_eq!(e.reliability_class, ReliabilityClass::ExternalReview);
        assert!(!e.blind, "the variant was shown for the first attempt");
        assert!(!e.validation_eligible);
    }
    let out = h
        .call(
            "competency.projection",
            json!({ "competency_id": COMPETENCY }),
        )
        .unwrap();
    let projection = &out["result"]["projections"][0];
    assert_eq!(projection["state"], "INDEPENDENT", "{projection}");
    assert_eq!(projection["provisional"], true);
}
