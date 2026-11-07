#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Grading, the ADR-0012 pass rule, gate failures, reliability classes and
//! adjudication (ARCHITECTURE.md §8 "Grading", Addendum A items 1–2; C09/C21;
//! gate A04).

mod common;

use common::*;
use g9_assessment::{
    adjudicate, adjudicate_decision, finalize, finalize_decision, meets_floor, pass_rates,
    score_breakdown, score_breakdown_with_gates, time_limit_gate, AnchoredLevel, Attempt,
    CriterionResult, DecisionInput, GateFailure, GradeDecision, GradeError, ReliabilityClass,
    Rubric, Score, CRITERION_FLOOR_PERCENT, PASS_THRESHOLD,
};
use g9_domain::{AssessmentAttemptState as S, DurationMs, ErrorCategory, G9Error, GradeSource};
use pretty_assertions::assert_eq;

fn decide(a: &Attempt, results: &[CriterionResult], gates: &[GateFailure]) -> GradeDecision {
    finalize_decision(
        decision_id("gd-1"),
        DecisionInput {
            attempt: a,
            rubric: &Rubric::baseline(),
            results,
            gate_failures: gates,
            reviewer: &mentor(),
            now: t(200),
        },
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// pass rule (ADR-0012)
// ---------------------------------------------------------------------------

#[test]
fn pass_threshold_80_boundary() {
    let (_, _, a) = graded("attempt-1");
    // exactly 80 on every criterion: total 80.0, every floor met -> PASS
    let d = decide(&a, &mentor_scored(&a, [80; 6]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(80.0));
    assert!(d.pass);
    assert!(d.is_final);
    assert_eq!(d.breakdown.pass_recommendation, Some(true));
    // one point short on a weight-10 criterion: total 79.0 -> FAIL
    let d = decide(&a, &mentor_scored(&a, [80, 80, 80, 80, 70, 80]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(79.0));
    assert!(!d.pass);
    assert!(d.breakdown.criterion_floors_met);
    assert!(d.gate_failures().is_empty());
    assert_eq!(PASS_THRESHOLD, 80);
}

#[test]
fn adr0012_per_criterion_floor_is_non_compensatory() {
    let (_, _, a) = graded("attempt-1");
    // LRN-10 example: 0 on root-cause reasoning (20) and 100 elsewhere totals 80,
    // which passed under the compensatory rule. It fails now.
    let d = decide(&a, &mentor_scored(&a, [100, 0, 100, 100, 100, 100]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(80.0));
    assert!(!d.pass);
    assert!(!d.breakdown.criterion_floors_met);
    let rcr = &d.breakdown.per_criterion[1];
    assert_eq!(rcr.code, "root_cause_reasoning");
    assert_eq!(rcr.meets_floor, Some(false));
    // 0 on detection (15) totals 85 and still fails
    let d = decide(&a, &mentor_scored(&a, [100, 100, 100, 0, 100, 100]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(85.0));
    assert!(!d.pass);
    // floor boundary: exactly 50 % of the weight passes, 49 fails
    let d = decide(&a, &mentor_scored(&a, [100, 50, 100, 100, 100, 100]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(90.0));
    assert!(d.pass);
    let d = decide(&a, &mentor_scored(&a, [100, 49, 100, 100, 100, 100]), &[]);
    assert!(!d.pass);
    assert_eq!(CRITERION_FLOOR_PERCENT, 50);
    assert!(meets_floor(20, 50));
    assert!(!meets_floor(20, 49));
    assert!(meets_floor(0, 0), "a weight-0 criterion has a zero floor");
}

#[test]
fn adr0012_pass_iff_total_and_floors_and_no_gate_failure() {
    let (_, _, a) = graded("attempt-1");
    // exhaustive over a grid of uniform scores x one weak criterion x gates
    for base in [60u8, 80, 90, 100] {
        for weak in [0u8, 40, 50, 70, 100] {
            for gates in [vec![], vec![GateFailure::OutOfScopeTargetOrAction]] {
                let mut scores = [base; 6];
                scores[0] = weak;
                let d = decide(&a, &mentor_scored(&a, scores), &gates);
                let total = d.breakdown.weighted_total.unwrap();
                let expected = total >= 80.0 && scores.iter().all(|s| *s >= 50) && gates.is_empty();
                assert_eq!(d.pass, expected, "base {base} weak {weak} gates {gates:?}");
                assert!(d.is_final);
            }
        }
    }
}

#[test]
fn anchored_levels_agree_with_the_cut() {
    let rubric = Rubric::baseline();
    let (_, _, a) = graded("attempt-1");
    let level = |l: u8| Score::from_level(&rubric, AnchoredLevel::new(l).unwrap()).get();
    // level 3 ("correct and complete") everywhere: exactly 80 -> PASS
    let d = decide(&a, &mentor_scored(&a, [level(3); 6]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(80.0));
    assert!(d.pass);
    assert!(d
        .breakdown
        .per_criterion
        .iter()
        .all(|c| c.level == Some(AnchoredLevel::new(3).unwrap())));
    // level 2 everywhere meets every floor but totals 50 -> FAIL
    let d = decide(&a, &mentor_scored(&a, [level(2); 6]), &[]);
    assert!(d.breakdown.criterion_floors_met);
    assert!(!d.pass);
    // level 1 on one criterion misses its floor even with level 4 elsewhere
    let mut scores = [level(4); 6];
    scores[5] = level(1);
    let d = decide(&a, &mentor_scored(&a, scores), &[]);
    assert!(!d.pass);
    assert_eq!(d.breakdown.per_criterion[5].meets_floor, Some(false));
}

#[test]
fn critical_gate_failure_yields_final_fail() {
    let (_, _, a) = graded("attempt-1");
    // the rubric's critical-gate criterion (scope compliance) below its floor
    let d = decide(&a, &mentor_scored(&a, [100, 100, 100, 100, 100, 40]), &[]);
    assert_eq!(d.breakdown.weighted_total, Some(94.0));
    assert!(d.is_final);
    assert!(!d.pass);
    assert!(!d.breakdown.critical_gates_passed);
    let scope = d.breakdown.per_criterion.last().unwrap();
    assert!(scope.critical_gate);
    assert_eq!(scope.gate_passed, Some(false));

    // every enumerated gate failure fails a perfect score, as a final decision,
    // and is recorded on the decision
    for gate in GateFailure::ALL {
        let d = decide(&a, &mentor_scored(&a, [100; 6]), &[gate]);
        assert!(d.is_final, "{gate:?}");
        assert!(!d.pass, "{gate:?}");
        assert_eq!(d.gate_failures(), &[gate]);
        assert_eq!(d.breakdown.pass_recommendation, Some(false));
        assert_eq!(d.breakdown.weighted_total, Some(100.0));
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["breakdown"]["gate_failures"][0], gate.code());
    }
    // duplicates collapse; order is canonical
    let d = decide(
        &a,
        &mentor_scored(&a, [100; 6]),
        &[
            GateFailure::TimeLimitExceededWithoutSubmission,
            GateFailure::OutOfScopeTargetOrAction,
            GateFailure::TimeLimitExceededWithoutSubmission,
        ],
    );
    assert_eq!(
        d.gate_failures(),
        &[
            GateFailure::OutOfScopeTargetOrAction,
            GateFailure::TimeLimitExceededWithoutSubmission
        ]
    );
}

#[test]
fn gate_failures_are_enumerated_with_stable_codes() {
    let codes: Vec<&str> = GateFailure::ALL.iter().map(|g| g.code()).collect();
    assert_eq!(
        codes,
        vec![
            "OUT_OF_SCOPE_TARGET_OR_ACTION",
            "UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION",
            "FABRICATED_OR_ALTERED_EVIDENCE",
            "TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION",
        ]
    );
    for g in GateFailure::ALL {
        assert_eq!(serde_json::to_value(g).unwrap(), g.code());
    }
}

#[test]
fn time_limit_gate_uses_monotonic_elapsed() {
    let limit = DurationMs::from_minutes_u32(120);
    assert_eq!(
        time_limit_gate(limit, DurationMs::from_minutes_u32(120)),
        None
    );
    assert_eq!(
        time_limit_gate(limit, DurationMs::new(120 * 60_000 + 1).unwrap()),
        Some(GateFailure::TimeLimitExceededWithoutSubmission)
    );
    assert_eq!(
        time_limit_gate(limit, DurationMs::from_minutes_u32(30)),
        None
    );
}

#[test]
fn provisional_breakdown_reports_known_failures_early() {
    let rubric = Rubric::baseline();
    let (_, _, a) = graded("attempt-1");
    // incomplete and no failure yet: unknown
    let partial = &mentor_scored(&a, [100; 6])[..3];
    let b = score_breakdown(&rubric, partial);
    assert_eq!(b.weighted_total, None);
    assert_eq!(b.pass_recommendation, None);
    assert_eq!(b.reliability_class, None);
    // incomplete but a scored criterion misses its floor: already a fail
    let mut low = mentor_scored(&a, [10; 6]);
    low.truncate(1);
    assert_eq!(
        score_breakdown(&rubric, &low).pass_recommendation,
        Some(false)
    );
    // incomplete with a gate failure: already a fail
    let b = score_breakdown_with_gates(
        &rubric,
        partial,
        &[GateFailure::FabricatedOrAlteredEvidence],
    );
    assert_eq!(b.pass_recommendation, Some(false));
}

// ---------------------------------------------------------------------------
// C09: model-advisory results and actor rules
// ---------------------------------------------------------------------------

#[test]
fn a04_model_only_criterion_blocks_finalization() {
    let (_, ver, a) = graded("attempt-1");
    let mut results = self_scored(&a, [100; 6]);
    // replace root-cause reasoning with a model-only score
    results.remove(1);
    results.push(
        CriterionResult::new(
            a.id().clone(),
            "root_cause_reasoning",
            Score::new(100).unwrap(),
            GradeSource::ModelAdvisory,
            &model(),
            t(150),
        )
        .unwrap(),
    );
    let err = finalize(
        decision_id("gd-1"),
        &a,
        &ver.rubric,
        &results,
        &learner(),
        t(200),
    )
    .unwrap_err();
    assert_eq!(
        err,
        GradeError::ModelOnlyCriterion("root_cause_reasoning".into())
    );
    let g = G9Error::from(err);
    assert_eq!(g.category, ErrorCategory::PreconditionFailed);
    assert_eq!(g.code, "assessment.grade.model_only_criterion");

    // a model-advisory score never raises the final score above the human one
    let mut mixed = self_scored(&a, [60, 100, 100, 100, 100, 100]);
    mixed.push(
        CriterionResult::new(
            a.id().clone(),
            "independent_execution",
            Score::new(100).unwrap(),
            GradeSource::ModelAdvisory,
            &skill(),
            t(150),
        )
        .unwrap(),
    );
    let d = finalize(
        decision_id("gd-2"),
        &a,
        &ver.rubric,
        &mixed,
        &learner(),
        t(200),
    )
    .unwrap();
    let ie = &d.breakdown.per_criterion[0];
    assert_eq!(ie.best_non_advisory, Some(60));
    assert_eq!(ie.advisory, Some(100));
    assert_eq!(ie.best_source, Some(GradeSource::HumanReviewer));

    // a criterion with no result at all
    let missing = &self_scored(&a, [100; 6])[1..];
    assert_eq!(
        finalize(
            decision_id("gd-3"),
            &a,
            &ver.rubric,
            missing,
            &learner(),
            t(200)
        )
        .unwrap_err(),
        GradeError::MissingCriterion("independent_execution".into())
    );
}

#[test]
fn result_sources_must_match_actor_kinds() {
    let (_, _, a) = graded("attempt-1");
    let attempt = a.id().clone();
    let s = Score::new(90).unwrap();
    for (source, actor) in [
        (GradeSource::HumanReviewer, model()),
        (GradeSource::HumanReviewer, system()),
        (GradeSource::DeterministicChecker, learner()),
        (GradeSource::DeterministicChecker, model()),
        (GradeSource::ModelAdvisory, learner()),
        (GradeSource::ModelAdvisory, system()),
    ] {
        let err = CriterionResult::new(
            attempt.clone(),
            "independent_execution",
            s,
            source,
            &actor,
            t(1),
        )
        .unwrap_err();
        assert!(matches!(err, GradeError::SourceActorMismatch { .. }));
        assert_eq!(G9Error::from(err).category, ErrorCategory::Unauthorized);
    }
    assert_eq!(Score::new(101).unwrap_err(), GradeError::InvalidScore(101));
    assert!(serde_json::from_str::<Score>("101").is_err());
}

#[test]
fn finalize_refuses_bad_reviewers_states_and_results() {
    let (_, ver, a) = graded("attempt-1");
    let results = self_scored(&a, [100; 6]);
    for reviewer in [system(), model(), skill()] {
        let err = finalize(
            decision_id("gd-1"),
            &a,
            &ver.rubric,
            &results,
            &reviewer,
            t(200),
        )
        .unwrap_err();
        assert!(matches!(err, GradeError::ReviewerNotOperator { .. }));
        assert_eq!(G9Error::from(err).category, ErrorCategory::Unauthorized);
    }
    // not yet graded provisionally
    let (_, _, mut open) = available("attempt-2");
    open.begin(&learner(), t(1)).unwrap();
    open.submit(&learner(), t(2)).unwrap();
    let err = finalize(
        decision_id("gd-1"),
        &open,
        &ver.rubric,
        &self_scored(&open, [100; 6]),
        &learner(),
        t(200),
    )
    .unwrap_err();
    assert_eq!(
        err,
        GradeError::AttemptNotGradedProvisional {
            state: S::Submitted
        }
    );
    // foreign result
    let foreign = self_scored(&open, [100; 6]);
    assert!(matches!(
        finalize(
            decision_id("gd-1"),
            &a,
            &ver.rubric,
            &foreign,
            &learner(),
            t(200)
        ),
        Err(GradeError::ForeignResult(_))
    ));
    // unknown criterion
    let mut unknown = results.clone();
    unknown[0].criterion_code = "made_up".into();
    assert_eq!(
        finalize(
            decision_id("gd-1"),
            &a,
            &ver.rubric,
            &unknown,
            &learner(),
            t(200)
        )
        .unwrap_err(),
        GradeError::UnknownCriterion("made_up".into())
    );
}

// ---------------------------------------------------------------------------
// reliability class (ADR-0012, LRN-11)
// ---------------------------------------------------------------------------

#[test]
fn reliability_self_adjudicated_when_reviewer_is_learner() {
    let (_, ver, a) = graded("attempt-1");
    // MVP: the operator is the learner and finalizes their own attempt
    for results in [
        self_scored(&a, [100; 6]),
        mentor_scored(&a, [100; 6]),
        checker_scored(&a, [100; 6]),
    ] {
        let d = finalize(
            decision_id("gd-1"),
            &a,
            &ver.rubric,
            &results,
            &learner(),
            t(200),
        )
        .unwrap();
        assert_eq!(d.reliability_class(), ReliabilityClass::SelfAdjudicated);
        assert_eq!(
            d.breakdown.reliability_class,
            Some(ReliabilityClass::SelfAdjudicated)
        );
        assert!(!d.satisfies_validation_evidence());
        assert!(d.pass, "the grade stands; only its reliability is limited");
    }
}

#[test]
fn reliability_self_adjudicated_when_learner_recorded_a_score() {
    let (_, ver, a) = graded("attempt-1");
    // a mentor finalizes, but one criterion rests on the learner's own score
    let mut results = mentor_scored(&a, [100; 6]);
    results[4] = self_scored(&a, [100; 6])[4].clone();
    let d = finalize(
        decision_id("gd-1"),
        &a,
        &ver.rubric,
        &results,
        &mentor(),
        t(200),
    )
    .unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::SelfAdjudicated);
    // a model-advisory result never counts toward the class
    let mut with_advisory = mentor_scored(&a, [100; 6]);
    with_advisory.push(
        CriterionResult::new(
            a.id().clone(),
            "root_cause_reasoning",
            Score::new(20).unwrap(),
            GradeSource::ModelAdvisory,
            &model(),
            t(150),
        )
        .unwrap(),
    );
    let d = finalize(
        decision_id("gd-2"),
        &a,
        &ver.rubric,
        &with_advisory,
        &mentor(),
        t(200),
    )
    .unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::ExternalReview);
}

#[test]
fn reliability_external_review_and_deterministic() {
    let (_, ver, a) = graded("attempt-1");
    let rubric = &ver.rubric;
    let d = finalize(
        decision_id("gd-1"),
        &a,
        rubric,
        &mentor_scored(&a, [100; 6]),
        &mentor(),
        t(200),
    )
    .unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::ExternalReview);
    assert!(d.satisfies_validation_evidence());

    let d = finalize(
        decision_id("gd-2"),
        &a,
        rubric,
        &checker_scored(&a, [100; 6]),
        &mentor(),
        t(200),
    )
    .unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::Deterministic);
    assert!(d.satisfies_validation_evidence());

    // checker everywhere plus one human score: external review
    let mut mixed = checker_scored(&a, [100; 6]);
    mixed.push(mentor_scored(&a, [100; 6])[2].clone());
    let d = finalize(decision_id("gd-3"), &a, rubric, &mixed, &mentor(), t(200)).unwrap();
    assert_eq!(d.reliability_class(), ReliabilityClass::ExternalReview);

    // classification is a pure function
    assert_eq!(
        ReliabilityClass::classify(&learner().id, &mentor().id, rubric, &mixed),
        ReliabilityClass::ExternalReview
    );
    assert_eq!(
        ReliabilityClass::classify(&learner().id, &learner().id, rubric, &mixed),
        ReliabilityClass::SelfAdjudicated
    );
    let codes: Vec<&str> = ReliabilityClass::ALL.iter().map(|c| c.code()).collect();
    assert_eq!(
        codes,
        vec!["DETERMINISTIC", "EXTERNAL_REVIEW", "SELF_ADJUDICATED"]
    );
    for c in ReliabilityClass::ALL {
        assert_eq!(serde_json::to_value(c).unwrap(), c.code());
    }
}

#[test]
fn decision_without_class_reads_as_self_adjudicated() {
    // a decision built by a struct literal elsewhere (no class in the breakdown)
    let (_, ver, a) = graded("attempt-1");
    let results = mentor_scored(&a, [100; 6]);
    let d = GradeDecision {
        id: decision_id("gd-x"),
        attempt_id: a.id().clone(),
        breakdown: score_breakdown(&ver.rubric, &results),
        pass: true,
        is_final: true,
        decided_by: mentor().id,
        decided_at: t(200),
        supersedes: None,
        rationale: None,
    };
    assert_eq!(d.reliability_class(), ReliabilityClass::SelfAdjudicated);
    assert!(!d.satisfies_validation_evidence());
    // an old serialized breakdown without the new fields still loads
    let mut json = serde_json::to_value(&d).unwrap();
    let b = json["breakdown"].as_object_mut().unwrap();
    b.remove("gate_failures");
    b.remove("reliability_class");
    let back: GradeDecision = serde_json::from_value(json).unwrap();
    assert!(back.gate_failures().is_empty());
    assert_eq!(back.reliability_class(), ReliabilityClass::SelfAdjudicated);
}

// ---------------------------------------------------------------------------
// adjudication (C21, A04)
// ---------------------------------------------------------------------------

#[test]
fn a04_adjudication_supersedes_not_mutates() {
    let (_, ver, mut a) = graded("attempt-1");
    let rubric = &ver.rubric;
    let first = self_scored(&a, [100, 30, 100, 100, 100, 100]);
    let prior = finalize(decision_id("gd-1"), &a, rubric, &first, &learner(), t(200)).unwrap();
    assert!(!prior.pass);
    a.approve_final(&learner(), t(201)).unwrap();
    let prior_snapshot = prior.clone();

    // dispute: the reviewer re-scores root-cause reasoning
    let mut corrected = first.clone();
    corrected[1] = mentor_scored(&a, [0, 90, 0, 0, 0, 0])[1].clone();
    let next = adjudicate(
        decision_id("gd-2"),
        &prior,
        "re-scored after appeal: root cause was identified in the report",
        &a,
        rubric,
        &corrected,
        &mentor(),
        t(300),
    )
    .unwrap();
    assert_eq!(next.supersedes, Some(prior.id.clone()));
    assert_ne!(next.id, prior.id);
    assert!(next.is_final);
    assert_eq!(
        next.rationale.as_deref(),
        Some("re-scored after appeal: root cause was identified in the report")
    );
    // human results take precedence; the max human score per criterion is used
    assert_eq!(next.breakdown.per_criterion[1].best_non_advisory, Some(90));
    assert!(next.pass);
    // the learner still recorded scores the decision rests on
    assert_eq!(next.reliability_class(), ReliabilityClass::SelfAdjudicated);
    // the prior decision and the finalized attempt are untouched
    assert_eq!(prior, prior_snapshot);
    assert_eq!(a.state(), S::ApprovedFinal);

    // refusals
    assert_eq!(
        adjudicate(
            decision_id("gd-1"),
            &prior,
            "x",
            &a,
            rubric,
            &corrected,
            &mentor(),
            t(300)
        )
        .unwrap_err(),
        GradeError::DecisionIdReused
    );
    assert_eq!(
        adjudicate(
            decision_id("gd-3"),
            &prior,
            "  ",
            &a,
            rubric,
            &corrected,
            &mentor(),
            t(300)
        )
        .unwrap_err(),
        GradeError::EmptyRationale
    );
    assert!(matches!(
        adjudicate(
            decision_id("gd-3"),
            &prior,
            "x",
            &a,
            rubric,
            &corrected,
            &model(),
            t(300)
        ),
        Err(GradeError::ReviewerNotOperator { .. })
    ));
    let mut provisional = prior.clone();
    provisional.is_final = false;
    assert_eq!(
        adjudicate(
            decision_id("gd-3"),
            &provisional,
            "x",
            &a,
            rubric,
            &corrected,
            &mentor(),
            t(300)
        )
        .unwrap_err(),
        GradeError::PriorNotFinal
    );
    let (_, _, other) = graded("attempt-9");
    assert_eq!(
        adjudicate(
            decision_id("gd-3"),
            &prior,
            "x",
            &other,
            rubric,
            &corrected,
            &mentor(),
            t(300)
        )
        .unwrap_err(),
        GradeError::PriorAttemptMismatch
    );
    let (_, _, mut started) = available("attempt-1");
    started.begin(&learner(), t(1)).unwrap();
    assert_eq!(
        adjudicate(
            decision_id("gd-3"),
            &prior,
            "x",
            &started,
            rubric,
            &corrected,
            &mentor(),
            t(300)
        )
        .unwrap_err(),
        GradeError::AttemptNotAdjudicable { state: S::Started }
    );
    assert_eq!(
        G9Error::from(GradeError::DecisionIdReused).category,
        ErrorCategory::Conflict
    );
}

#[test]
fn adjudication_keeps_gate_failures_unless_explicitly_removed() {
    let (_, ver, a) = graded("attempt-1");
    let results = mentor_scored(&a, [100; 6]);
    let prior = finalize_decision(
        decision_id("gd-1"),
        DecisionInput {
            attempt: &a,
            rubric: &ver.rubric,
            results: &results,
            gate_failures: &[GateFailure::UnsafeOrDestructiveUnauthorizedAction],
            reviewer: &mentor(),
            now: t(200),
        },
    )
    .unwrap();
    assert!(!prior.pass);
    // a score correction carries the gate forward
    let carried = adjudicate(
        decision_id("gd-2"),
        &prior,
        "typo in one score",
        &a,
        &ver.rubric,
        &results,
        &mentor(),
        t(300),
    )
    .unwrap();
    assert_eq!(carried.gate_failures(), prior.gate_failures());
    assert!(!carried.pass);
    // removing the gate needs an explicit adjudication with a rationale
    let cleared = adjudicate_decision(
        decision_id("gd-3"),
        &carried,
        "action was inside the signed lab authorization",
        DecisionInput {
            attempt: &a,
            rubric: &ver.rubric,
            results: &results,
            gate_failures: &[],
            reviewer: &mentor(),
            now: t(400),
        },
    )
    .unwrap();
    assert!(cleared.gate_failures().is_empty());
    assert!(cleared.pass);
    assert_eq!(cleared.supersedes, Some(carried.id.clone()));
}

// ---------------------------------------------------------------------------
// reporting (Addendum A item 3)
// ---------------------------------------------------------------------------

#[test]
fn first_attempt_and_eventual_pass_rates_are_separate() {
    let def = definition();
    let v2 = version_n(&def, 2, 3);
    let rubric = Rubric::baseline();
    let fin = |a: &mut Attempt, id: &str, score: u8| {
        let d = finalize(
            decision_id(id),
            a,
            &rubric,
            &mentor_scored(a, [score; 6]),
            &mentor(),
            t(200),
        )
        .unwrap();
        a.approve_final(&learner(), t(201)).unwrap();
        d
    };
    // chain 1: fail, then retake pass
    let (_, _, mut a1) = graded("attempt-1");
    let d1 = fin(&mut a1, "gd-1", 40);
    let mut r1 = Attempt::new_retake(attempt_id("attempt-1r"), &a1, &v2, t(300)).unwrap();
    to_graded(&mut r1);
    let d1r = fin(&mut r1, "gd-1r", 90);
    // chain 2: first-attempt pass
    let (_, _, mut a2) = graded("attempt-2");
    let d2 = fin(&mut a2, "gd-2", 95);
    // chain 3: fail, no retake yet
    let (_, _, mut a3) = graded("attempt-3");
    let d3 = fin(&mut a3, "gd-3", 30);
    // chain 4: never decided
    let (_, _, a4) = available("attempt-4");

    let rates = pass_rates(&[a1, r1, a2, a3, a4], &[d1, d1r, d2, d3.clone()]);
    assert_eq!(rates.first_attempts_decided, 3);
    assert_eq!(rates.first_attempt_passes, 1);
    assert_eq!(rates.chains_decided, 3);
    assert_eq!(rates.eventual_passes, 2);
    assert_eq!(rates.retakes_decided, 1);
    assert_eq!(rates.first_attempt_pass_rate, Some(1.0 / 3.0));
    assert_eq!(rates.eventual_pass_rate, Some(2.0 / 3.0));

    // a superseding decision replaces the one it supersedes
    let (_, _, a3b) = graded("attempt-3");
    let mut d3_fixed = d3.clone();
    d3_fixed.id = decision_id("gd-3b");
    d3_fixed.pass = true;
    d3_fixed.supersedes = Some(d3.id.clone());
    let only = pass_rates(&[a3b], &[d3, d3_fixed]);
    assert_eq!(only.first_attempt_passes, 1);
    assert_eq!(pass_rates(&[], &[]).first_attempt_pass_rate, None);
}

#[test]
fn grade_errors_map_to_closed_categories() {
    let cases = [
        (GradeError::InvalidScore(101), ErrorCategory::InvalidInput),
        (GradeError::EmptyRationale, ErrorCategory::InvalidInput),
        (GradeError::PriorNotFinal, ErrorCategory::PreconditionFailed),
        (
            GradeError::MissingCriterion("x".into()),
            ErrorCategory::PreconditionFailed,
        ),
        (GradeError::IncompleteBreakdown, ErrorCategory::Internal),
    ];
    for (e, cat) in cases {
        let code = e.code();
        let g = G9Error::from(e);
        assert_eq!(g.category, cat);
        assert_eq!(g.code, code);
    }
}
