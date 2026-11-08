#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Grade decision -> competency observations, end to end through
//! `g9_app::dispatch` (ARCHITECTURE.md §8, Addendum A.2; ADR-0005, ADR-0012):
//! `assessment.approve_final` derives one append-only observation per
//! `(competency, dimension)` plus an evidence row, in the transaction that
//! finalizes the attempt, and `competency.projection` excludes
//! validation-ineligible evidence.
//!
//! In the MVP the operator is both learner and reviewer, so every decision
//! reachable through `dispatch` is `SELF_ADJUDICATED` (ADR-0012): the
//! `EXTERNAL_REVIEW` / `DETERMINISTIC` path that reaches `Validated` is tested
//! next to the code it exercises, in `crates/g9-app/src/commands/assessment/tests.rs`.

mod common;

use common::*;
use g9_competency::{derive_state, CompetencyObservation, DerivationRules, ObservationSource};
use g9_domain::{
    AssessmentAttemptState, AssistanceLevel, AttemptId, CompetencyId, Confidence, ErrorCategory,
    InteractionMode, LearningDimension, ObservationId, Provenance, SourceKind, TrustClass,
};
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::competency as competency_repo;
use g9_storage::StorageError;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

const OBSERVATIONS_NOT_DERIVED: &str = "competency.observations_not_derived";

fn comp() -> CompetencyId {
    CompetencyId::parse(COMPETENCY).unwrap()
}

fn levels_payload(attempt: &str, levels: [u8; 6]) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .zip(levels)
        .map(|(c, l)| {
            json!({ "criterion_code": c.code, "score": l, "source": "HUMAN_REVIEWER", "evidence_refs": [] })
        })
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

/// Begin, submit and open evidence review; returns the attempt id.
fn to_review(h: &mut Harness, def: &g9_domain::AssessmentId) -> String {
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
}

/// Grade `levels` (operator, HUMAN_REVIEWER) and return the revision to bind
/// the finalization approval to.
fn grade(h: &mut Harness, attempt: &str, levels: [u8; 6]) -> u64 {
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(attempt, levels),
        )
        .unwrap();
    graded["revision"].as_u64().unwrap()
}

/// begin .. grade .. approved finalization; returns the attempt id and the
/// `assessment.approve_final` outcome.
fn finalize(h: &mut Harness, def: &g9_domain::AssessmentId, levels: [u8; 6]) -> (String, Value) {
    let attempt = to_review(h, def);
    let rev = grade(h, &attempt, levels);
    let outcome = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    (attempt, outcome)
}

fn warnings(outcome: &Value) -> Vec<String> {
    outcome["warnings"]
        .as_array()
        .map(|w| w.iter().map(|x| x.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

fn not_derived_warnings(outcome: &Value) -> Vec<String> {
    warnings(outcome)
        .into_iter()
        .filter(|w| w.starts_with(OBSERVATIONS_NOT_DERIVED))
        .collect()
}

fn projection(h: &mut Harness) -> Value {
    let out = h
        .call(
            "competency.projection",
            json!({ "competency_id": COMPETENCY }),
        )
        .unwrap();
    let projections = out["result"]["projections"].as_array().unwrap();
    assert_eq!(projections.len(), 1);
    projections[0].clone()
}

fn observation_count(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM competency_observations", [], |r| {
            r.get(0)
        })
        .unwrap()
}

fn evidence_count(h: &Harness) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM competency_observation_evidence",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn aid(attempt: &str) -> AttemptId {
    AttemptId::parse(attempt).unwrap()
}

/// A declared difficulty is carried onto every observation; each observation has
/// an evidence row that names the decision, its class and its blindness.
/// FAILS ON THE OLD CODE: `approve_final` appended no observation and always
/// warned `competency.observations_not_derived`.
#[test]
fn declared_difficulty_derives_one_observation_per_dimension_with_evidence() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(4));
    let (attempt, outcome) = finalize(&mut h, &def, [4; 6]);
    assert_eq!(outcome["result"]["state"], "APPROVED_FINAL");
    assert!(
        not_derived_warnings(&outcome).is_empty(),
        "{:?}",
        warnings(&outcome)
    );

    let decision = assessment_repo::effective_final_decision(h.conn(), &aid(&attempt))
        .unwrap()
        .unwrap();
    let observations = competency_repo::observations_for_attempt(h.conn(), &aid(&attempt)).unwrap();
    // baseline rubric: Application, Understanding and Transfer criteria exist
    let dimensions: std::collections::BTreeSet<LearningDimension> =
        observations.iter().map(|o| o.dimension).collect();
    assert_eq!(
        dimensions,
        [
            LearningDimension::Understanding,
            LearningDimension::Application,
            LearningDimension::Transfer
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(observations.len(), 3);
    for o in &observations {
        assert_eq!(o.competency_id, comp());
        assert_eq!(o.source, ObservationSource::Attempt(aid(&attempt)));
        assert_eq!(o.mode, InteractionMode::Exam);
        assert_eq!(o.difficulty.get(), 4, "the difficulty the author declared");
        assert_eq!(o.assistance, AssistanceLevel::Unaided);
        assert_eq!(o.score, 100);
        assert_eq!(o.reviewer, Some(operator_id()));
        assert_eq!(o.environment, "assessment:ad-w1@1");
        assert_eq!(o.provenance.trust, TrustClass::Operator);
        assert_eq!(o.provenance.source_kind, SourceKind::AssessmentSubmission);
        assert_eq!(o.provenance.recorded_by, operator_id());
        assert_eq!(o.uncertainty, Confidence::new(0.0).unwrap());
        assert_eq!(o.observed_at, decision.decided_at);
        assert_eq!(o.supersedes, None);
    }

    let evidence = competency_repo::observation_evidence_for_competency(h.conn(), &comp()).unwrap();
    assert_eq!(evidence.len(), 3);
    for e in &evidence {
        assert_eq!(e.decision_id, decision.id);
        assert_eq!(
            e.reliability_class,
            assessment_repo::ReliabilityClass::SelfAdjudicated
        );
        assert!(e.blind, "EXAM, unaided, only the task was exposed");
        assert!(!e.validation_eligible, "the operator graded their own exam");
    }
    assert_eq!(
        competency_repo::ineligible_observation_ids(h.conn(), &comp())
            .unwrap()
            .len(),
        3
    );
}

/// (b) The identical attempt graded by the learner themself never reaches
/// Validated: Independent with `provisional` (and the exclusion is what stops
/// it: the same observations derive Validated without the ineligible set).
/// FAILS ON THE OLD CODE: no exam observation existed and `provisional` was the
/// constant false.
#[test]
fn self_adjudicated_blind_pass_stops_at_independent_and_is_provisional() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    lab_observation(&h, "obs-lab-1", 80, 3);
    lab_observation(&h, "obs-lab-2", 85, 2);

    let before = projection(&mut h);
    assert_eq!(before["state"], "INDEPENDENT");
    assert_eq!(before["provisional"], false);
    assert_eq!(before["evidence_refs"].as_array().unwrap().len(), 2);

    let (attempt, outcome) = finalize(&mut h, &def, [4; 6]);
    assert_eq!(outcome["result"]["grade"]["pass"], true);
    assert_eq!(
        outcome["result"]["grade"]["reliability_class"],
        "SELF_ADJUDICATED"
    );

    let after = projection(&mut h);
    assert_eq!(after["state"], "INDEPENDENT", "{after}");
    assert_eq!(after["provisional"], true, "{after}");
    let refs: Vec<&str> = after["evidence_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    assert_eq!(
        refs.len(),
        5,
        "two lab observations and three from the exam"
    );
    let exam_ids: Vec<String> = competency_repo::observations_for_attempt(h.conn(), &aid(&attempt))
        .unwrap()
        .iter()
        .map(|o| o.id.as_str().to_string())
        .collect();
    assert_eq!(exam_ids.len(), 3);
    assert!(exam_ids.iter().all(|id| refs.contains(&id.as_str())));
    // the exam's Application observation is in the projection
    assert_eq!(after["per_dimension"]["APPLICATION"]["n_unaided"], 3);

    // negative control: without the ineligible set the same history validates
    let node = competency_repo::get_competency(
        h.conn(),
        &g9_storage::ReadScope::domain(g9_domain::DataDomain::Learning),
        &comp(),
    )
    .unwrap()
    .unwrap()
    .to_domain()
    .unwrap();
    let observations = competency_repo::list_observations(h.conn(), &comp()).unwrap();
    let unfiltered = derive_state(&node, &observations, &DerivationRules::default(), h.now());
    assert_eq!(
        unfiltered.state,
        g9_domain::CompetencyState::Validated,
        "only the evidence rows keep this competency from validating"
    );
}

/// (c) A failing decision never validates, however high a dimension scored.
/// FAILS ON THE OLD CODE: no exam observation or evidence row was recorded.
#[test]
fn failing_decision_observations_never_validate() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    lab_observation(&h, "obs-lab-1", 80, 3);
    lab_observation(&h, "obs-lab-2", 85, 2);
    // root-cause reasoning at level 0 misses the per-criterion floor: the
    // decision fails (ADR-0012) although every Application criterion is 100
    let (attempt, outcome) = finalize(&mut h, &def, [4, 0, 4, 4, 4, 4]);
    assert_eq!(outcome["result"]["grade"]["pass"], false);

    let observations = competency_repo::observations_for_attempt(h.conn(), &aid(&attempt)).unwrap();
    let application = observations
        .iter()
        .find(|o| o.dimension == LearningDimension::Application)
        .unwrap();
    assert_eq!(
        application.score, 100,
        "high enough to validate if eligible"
    );
    let evidence = competency_repo::observation_evidence_for_competency(h.conn(), &comp()).unwrap();
    assert_eq!(evidence.len(), 3);
    assert!(evidence.iter().all(|e| !e.validation_eligible));
    let projected = projection(&mut h);
    assert_eq!(projected["state"], "INDEPENDENT", "{projected}");
    assert_ne!(projected["state"], "VALIDATED");
}

/// A correction supersedes a self-graded observation but cannot launder it: the
/// new row has no evidence row of its own, so it inherits the ineligibility of
/// the row it supersedes (a raised score does not make it validation evidence).
#[test]
fn a_correction_of_a_self_adjudicated_observation_cannot_validate() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    lab_observation(&h, "obs-lab-1", 80, 3);
    lab_observation(&h, "obs-lab-2", 85, 2);
    let (attempt, _) = finalize(&mut h, &def, [4; 6]);
    let derived = competency_repo::observations_for_attempt(h.conn(), &aid(&attempt))
        .unwrap()
        .into_iter()
        .find(|o| o.dimension == LearningDimension::Application)
        .unwrap();
    h.advance(60_000);
    let correction = CompetencyObservation {
        id: ObservationId::parse("obs-correction-1").unwrap(),
        source: ObservationSource::ManualCorrection,
        score: 95,
        provenance: Provenance {
            source_kind: SourceKind::OperatorInput,
            source_ref: attempt.clone(),
            trust: TrustClass::Operator,
            content_sha256: None,
            recorded_at: h.now(),
            recorded_by: operator_id(),
            version: None,
        },
        observed_at: h.now(),
        supersedes: Some(derived.id.clone()),
        ..derived.clone()
    };
    competency_repo::append_observation(h.conn(), &correction).unwrap();
    assert!(
        competency_repo::ineligible_observation_ids(h.conn(), &comp())
            .unwrap()
            .contains(&correction.id),
        "the correction inherits the ineligibility of the grade it corrects"
    );
    let projected = projection(&mut h);
    assert_eq!(projected["state"], "INDEPENDENT", "{projected}");
    assert_eq!(projected["provisional"], true);
    // the superseded row is retained in storage but is no longer evidence
    let refs = projected["evidence_refs"].as_array().unwrap();
    assert!(!refs.contains(&json!(derived.id.as_str())));
    assert!(refs.contains(&json!(correction.id.as_str())));

    // negative control: the unfiltered fold would validate on the corrected score
    let node = competency_repo::get_competency(
        h.conn(),
        &g9_storage::ReadScope::domain(g9_domain::DataDomain::Learning),
        &comp(),
    )
    .unwrap()
    .unwrap()
    .to_domain()
    .unwrap();
    let observations = competency_repo::list_observations(h.conn(), &comp()).unwrap();
    assert_eq!(
        derive_state(&node, &observations, &DerivationRules::default(), h.now()).state,
        g9_domain::CompetencyState::Validated
    );
}

/// (d) A version that declares no difficulty derives nothing and says why.
/// FAILS ON THE OLD CODE: the warning named the "wave-1 gap" instead.
#[test]
fn version_without_declared_difficulty_derives_nothing_and_warns() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, None);
    let version = assessment_repo::get_version(h.conn(), "av-w1-1")
        .unwrap()
        .unwrap();
    assert_eq!(version.difficulty, None);

    let (attempt, outcome) = finalize(&mut h, &def, [4; 6]);
    // the grade itself is final and unaffected
    assert_eq!(outcome["result"]["state"], "APPROVED_FINAL");
    assert_eq!(outcome["result"]["grade"]["pass"], true);
    let stored = assessment_repo::get_attempt(
        h.conn(),
        &g9_storage::ReadScope::cross_domain(g9_domain::DataDomain::Learning),
        &aid(&attempt),
    )
    .unwrap()
    .unwrap();
    assert_eq!(stored.state, AssessmentAttemptState::ApprovedFinal);

    let warned = not_derived_warnings(&outcome);
    assert_eq!(warned.len(), 1, "{:?}", warnings(&outcome));
    assert!(
        warned[0].contains("declares no difficulty"),
        "the reason names the missing difficulty: {}",
        warned[0]
    );
    assert!(!warned[0].contains("wave-1"), "{}", warned[0]);
    assert_eq!(observation_count(&h), 0);
    assert_eq!(evidence_count(&h), 0);
    let projected = projection(&mut h);
    assert_eq!(projected["state"], "UNASSESSED");
    assert_eq!(projected["provisional"], false);
    assert_eq!(projected["evidence_refs"], json!([]));
}

/// (e) Replaying the command with the same idempotency key returns the stored
/// outcome and appends nothing; a second finalization with a new key is refused.
#[test]
fn replay_does_not_duplicate_observations_or_evidence() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let attempt = to_review(&mut h, &def);
    let rev = grade(&mut h, &attempt, [4; 6]);
    let payload = json!({ "attempt_id": attempt });
    let key = Opts {
        idempotency_key: Some("finalize-1".into()),
        expected_revision: Some(rev),
        ..Opts::default()
    };
    let refused = h
        .call_with("assessment.approve_final", payload.clone(), &key)
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::ApprovalRequired);
    let approval = refused.approval_id.clone().unwrap();
    h.decide(approval.as_str(), "APPROVE");
    let with_approval = Opts {
        approval_id: Some(approval.as_str().to_string()),
        ..key
    };
    let first = h
        .call_with("assessment.approve_final", payload.clone(), &with_approval)
        .unwrap();
    assert_eq!(first["replayed"], false);
    assert_eq!(observation_count(&h), 3);
    assert_eq!(evidence_count(&h), 3);
    let ids = |h: &Harness| -> Vec<ObservationId> {
        competency_repo::list_observations(h.conn(), &comp())
            .unwrap()
            .into_iter()
            .map(|o| o.id)
            .collect()
    };
    let observed = ids(&h);

    let replay = h
        .call_with("assessment.approve_final", payload.clone(), &with_approval)
        .unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"], first["result"]);
    assert_eq!(observation_count(&h), 3, "the replay appended nothing");
    assert_eq!(evidence_count(&h), 3);
    assert_eq!(ids(&h), observed);

    // a different key is a new command on a finalized attempt: refused, nothing appended
    let again = h
        .call_with(
            "assessment.approve_final",
            payload,
            &Opts::key("finalize-2"),
        )
        .unwrap_err();
    assert!(
        matches!(
            again.category,
            ErrorCategory::Unauthorized | ErrorCategory::PreconditionFailed
        ),
        "{again:?}"
    );
    assert_eq!(observation_count(&h), 3);
    assert_eq!(evidence_count(&h), 3);
    assert_eq!(ids(&h), observed);
}

/// (f) Observations and their evidence rows are append-only.
#[test]
fn observations_and_evidence_are_immutable() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let (_, _) = finalize(&mut h, &def, [4; 6]);
    assert_eq!(observation_count(&h), 3);
    for sql in [
        "UPDATE competency_observations SET score = 0",
        "DELETE FROM competency_observations",
        "UPDATE competency_observation_evidence SET validation_eligible = 1",
        "DELETE FROM competency_observation_evidence",
    ] {
        let err = StorageError::from(h.conn().execute(sql, []).unwrap_err());
        assert!(
            matches!(err, StorageError::Immutable(_)),
            "{sql} -> {err:?}"
        );
    }
    assert_eq!(observation_count(&h), 3);
    assert_eq!(evidence_count(&h), 3);
    // the declared difficulty of the version is frozen with it
    let err = StorageError::from(
        h.conn()
            .execute("UPDATE assessment_versions SET difficulty = 5", [])
            .unwrap_err(),
    );
    assert!(matches!(err, StorageError::Immutable(_)), "{err:?}");
}

/// Blindness is computed from the attempt's recorded exposure, not assumed: an
/// answer key shown before submission makes the observations non-blind.
#[test]
fn blindness_follows_the_exposure_log() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();
    assessment_repo::append_exposure(
        h.conn(),
        &assessment_repo::ExposureLogRow {
            id: "ex-key-1".into(),
            assessment_version_id: "av-w1-1".into(),
            attempt_id: Some(aid(&attempt)),
            exposed_to: assessment_repo::ExposedTo::Operator,
            actor_id: operator_id(),
            mode: InteractionMode::Exam,
            content_kind: assessment_repo::ExposureContentKind::Key,
            content_sha256: "0".repeat(64),
            reason: Some("test: key shown before submission".into()),
            exposed_at: h.now(),
            created_at: h.now(),
        },
    )
    .unwrap();
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
    let rev = grade(&mut h, &attempt, [4; 6]);
    h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    let evidence = competency_repo::observation_evidence_for_competency(h.conn(), &comp()).unwrap();
    assert_eq!(evidence.len(), 3);
    assert!(evidence.iter().all(|e| !e.blind), "{evidence:?}");
}

/// `competency.projection` for a competency with no observations is unchanged:
/// UNASSESSED, not provisional, null estimates (passes before and after the change).
#[test]
fn projection_without_observations_is_unchanged() {
    let mut h = Harness::new();
    let _ = seed_blind_practical(&h, 1, Some(3));
    let projected = projection(&mut h);
    assert_eq!(projected["competency_id"], COMPETENCY);
    assert_eq!(projected["state"], "UNASSESSED");
    assert_eq!(projected["provisional"], false);
    assert_eq!(projected["contradictions"], json!([]));
    assert_eq!(projected["evidence_refs"], json!([]));
    for dimension in ["RECALL", "UNDERSTANDING", "APPLICATION", "TRANSFER"] {
        assert_eq!(
            projected["per_dimension"][dimension],
            json!({
                "score_estimate": null,
                "n_unaided": 0,
                "last_observed_at": null,
                "uncertainty": null,
            }),
            "{dimension}"
        );
    }
}
