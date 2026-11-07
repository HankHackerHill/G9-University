#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! g9-app wiring of the ADR-0012 assessment rules through the real `dispatch`
//! entry point: anchored levels via the rubric's explicit scale, the
//! non-compensatory pass rule, the reliability class recorded by
//! `assessment.approve_final` (operator = learner in the MVP →
//! `SELF_ADJUDICATED`), and retakes on a different variant (LRN-13).

mod common;

use common::*;
use g9_domain::{
    AssessmentId, AttemptId, CompetencyId, DataDomain, Difficulty, ErrorCategory, EvidenceId,
};
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::competency as competency_repo;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

/// Competency + definition + `versions` baseline versions (labels "1".."n").
fn seed(h: &Harness, versions: u32) -> AssessmentId {
    let conn = h.conn();
    let now = h.now();
    let competency = g9_competency::Competency {
        id: CompetencyId::parse("comp-boot").unwrap(),
        name: "Linux boot chain".into(),
        pack: "linux".into(),
        domain: DataDomain::Learning,
        description: "Recover a broken boot chain".into(),
        validity_window_days: 90,
        difficulty_band: (Difficulty::new(1).unwrap(), Difficulty::new(5).unwrap()),
        nice_refs: Vec::new(),
    };
    competency_repo::insert_competency(
        conn,
        &competency_repo::CompetencyRow::from_domain(&competency, "LINUX-BOOT", None, now),
    )
    .unwrap();
    let def = g9_assessment::AssessmentDefinition::new(
        AssessmentId::parse("ad-w1").unwrap(),
        "Week 1 blind practical",
        vec![competency.id.clone()],
        g9_assessment::AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )
    .unwrap();
    assessment_repo::insert_definition(
        conn,
        &assessment_repo::AssessmentDefinitionRow::from_domain(&def, "W1-BLIND", now).unwrap(),
    )
    .unwrap();
    for n in 1..=versions {
        let version = g9_assessment::AssessmentVersion {
            definition_id: def.id.clone(),
            version: n,
            rubric: g9_assessment::Rubric::baseline(),
            task_ref: EvidenceId::parse(&format!("evi-task-{n}")).unwrap(),
            sealed_key_ref: None,
            difficulty: Difficulty::new(3).unwrap(),
            equivalence_group: "W1".into(),
            created_at: now,
            checksum: String::new(),
        }
        .with_checksum()
        .unwrap();
        let row = assessment_repo::AssessmentVersionRow::from_domain(
            &version,
            &format!("av-w1-{n}"),
            &format!("Recover the boot chain (variant {n})"),
            None,
            operator_id(),
        )
        .unwrap();
        assessment_repo::insert_version(conn, &row).unwrap();
        let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
            &version.rubric,
            &format!("rub-w1-{n}"),
            &format!("av-w1-{n}"),
            "Baseline",
            "v1",
            now,
        );
        assessment_repo::insert_rubric(conn, &rubric, &criteria).unwrap();
    }
    def.id
}

fn levels_payload(attempt: &str, levels: [u8; 6], source: &str) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .zip(levels)
        .map(|(c, l)| {
            json!({ "criterion_code": c.code, "score": l, "source": source, "evidence_refs": [] })
        })
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

/// Begin, submit and open evidence review; returns the attempt id.
fn to_review(h: &mut Harness, def: &AssessmentId, retake_of: Option<&str>) -> (String, Value) {
    let mut payload = json!({ "assessment_id": def.as_str() });
    if let Some(r) = retake_of {
        payload["retake_of"] = json!(r);
    }
    let begun = h.call("assessment.begin", payload).unwrap();
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
    (attempt, begun)
}

fn finalize_at(h: &mut Harness, attempt: &str, levels: [u8; 6], source: &str) -> Value {
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(attempt, levels, source),
        )
        .unwrap();
    let rev = graded["revision"].as_u64().unwrap();
    h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    )
}

#[test]
fn adr0012_operator_cannot_claim_checker_source_and_finalization_is_self_adjudicated() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (attempt, _) = to_review(&mut h, &def, None);
    let aid = AttemptId::parse(&attempt).unwrap();
    // The operator claims checker provenance for every criterion: only a
    // System-actor checker may write DETERMINISTIC_CHECKER results
    // (CriterionResult::new), so the call is refused and nothing is stored.
    let refused = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4; 6], "DETERMINISTIC_CHECKER"),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Unauthorized, "{refused:?}");
    assert_eq!(refused.code, "assessment.grade.source_actor_mismatch");
    assert!(assessment_repo::effective_criterion_results(h.conn(), &aid)
        .unwrap()
        .is_empty());
    let stored = assessment_repo::get_attempt(
        h.conn(),
        &g9_storage::repos::ReadScope::cross_domain(DataDomain::Learning),
        &aid,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        stored.state,
        g9_domain::AssessmentAttemptState::EvidenceReview,
        "the refused grade did not move the attempt"
    );

    // The operator's own (HUMAN_REVIEWER) grade: the operator is the learner,
    // so the crate classifies the decision SELF_ADJUDICATED.
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4; 6], "HUMAN_REVIEWER"),
        )
        .unwrap();
    assert_eq!(graded["result"]["grade"]["final"], false);
    assert_eq!(
        graded["result"]["grade"]["reliability_class"],
        "SELF_ADJUDICATED"
    );
    let rev = graded["revision"].as_u64().unwrap();
    let finalized = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    assert_eq!(finalized["result"]["state"], "APPROVED_FINAL");
    assert_eq!(finalized["result"]["grade"]["pass"], true);
    assert_eq!(
        finalized["result"]["grade"]["reliability_class"],
        "SELF_ADJUDICATED"
    );
    let row = assessment_repo::effective_final_decision(h.conn(), &aid)
        .unwrap()
        .unwrap();
    assert_eq!(
        row.reliability_class,
        assessment_repo::ReliabilityClass::SelfAdjudicated
    );
    assert_eq!(row.decided_by, operator_id());
    assert!(row.passed);
    assert!(assessment_repo::effective_criterion_results(h.conn(), &aid)
        .unwrap()
        .iter()
        .all(|r| r.row.source == g9_domain::GradeSource::HumanReviewer));
}

#[test]
fn adr0012_anchored_levels_use_the_rubric_scale_and_the_floor() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (attempt, _) = to_review(&mut h, &def, None);
    let aid = AttemptId::parse(&attempt).unwrap();

    // level 3 ("correct and complete") everywhere: 80 per criterion, total 80 -> pass
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [3; 6], "HUMAN_REVIEWER"),
        )
        .unwrap();
    let grade = &graded["result"]["grade"];
    assert_eq!(grade["total"], json!(80.0));
    assert_eq!(grade["pass"], true);
    assert!(grade["criteria"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["score"] == 3));
    let stored = assessment_repo::effective_criterion_results(h.conn(), &aid).unwrap();
    assert!(stored.iter().all(|r| r.row.score == 80));

    // LRN-10 compensatory case: level 0 on root-cause reasoning, 4 elsewhere,
    // totals 80 but misses the per-criterion floor -> fail
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let regraded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4, 0, 4, 4, 4, 4], "HUMAN_REVIEWER"),
        )
        .unwrap();
    assert_eq!(regraded["result"]["grade"]["total"], json!(80.0));
    assert_eq!(regraded["result"]["grade"]["pass"], false);
    let rev = regraded["revision"].as_u64().unwrap();
    let finalized = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    assert_eq!(finalized["result"]["grade"]["final"], true);
    assert_eq!(finalized["result"]["grade"]["pass"], false);
    let row = assessment_repo::effective_final_decision(h.conn(), &aid)
        .unwrap()
        .unwrap();
    assert!(!row.passed);
    assert_eq!(row.total_score, 80);
}

/// Level 5 is refused at pipeline step 1 by the request schema (`maximum: 4`);
/// the handler's rubric-scale check (`grade.level_out_of_range`) is the second
/// line of defence and is not reached here.
#[test]
fn adr0012_level_above_four_is_invalid_input() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (attempt, _) = to_review(&mut h, &def, None);
    let aid = AttemptId::parse(&attempt).unwrap();
    let mut payload = levels_payload(&attempt, [4; 6], "HUMAN_REVIEWER");
    payload["criteria"][2]["score"] = json!(5);
    let err = h.call("assessment.grade_provisional", payload).unwrap_err();
    assert_eq!(err.category, ErrorCategory::InvalidInput);
    // nothing was written
    assert!(assessment_repo::effective_criterion_results(h.conn(), &aid)
        .unwrap()
        .is_empty());
}

#[test]
fn lrn13_retake_uses_a_different_variant_when_one_exists() {
    let mut h = Harness::new();
    let def = seed(&h, 2);
    let (first, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["variant"], "2", "latest version first");
    finalize_at(&mut h, &first, [1; 6], "HUMAN_REVIEWER");

    let (retake, begun) = to_review(&mut h, &def, Some(&first));
    assert_eq!(begun["result"]["retake_of"], json!(first));
    assert_eq!(begun["result"]["variant"], "1", "the other variant");
    assert!(begun["warnings"]
        .as_array()
        .is_none_or(|w| w.iter().all(|x| !x
            .as_str()
            .unwrap_or_default()
            .starts_with("assessment.retake_same_variant"))));
    let stored = assessment_repo::get_attempt(
        h.conn(),
        &g9_storage::repos::ReadScope::cross_domain(DataDomain::Learning),
        &AttemptId::parse(&retake).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(stored.assessment_version_id, "av-w1-1");
}

#[test]
fn lrn13_retake_on_the_only_variant_is_flagged() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (first, _) = to_review(&mut h, &def, None);
    finalize_at(&mut h, &first, [1; 6], "HUMAN_REVIEWER");
    let (_, begun) = to_review(&mut h, &def, Some(&first));
    assert_eq!(begun["result"]["variant"], "1");
    let warnings: Vec<String> = begun["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("assessment.retake_same_variant")),
        "{warnings:?}"
    );
}

#[test]
fn lrn13_second_begin_without_retake_of_is_recorded_as_a_linked_retake() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (first, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["retake_of"], json!(null));
    finalize_at(&mut h, &first, [1; 6], "HUMAN_REVIEWER");

    // The UI sends `{ assessment_id }` only.
    let (second, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["retake_of"], json!(first), "{begun}");
    let warnings: Vec<String> = begun["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap().to_string())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("assessment.retake_inferred")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("assessment.retake_same_variant")),
        "{warnings:?}"
    );
    let stored = assessment_repo::get_attempt(
        h.conn(),
        &g9_storage::repos::ReadScope::cross_domain(DataDomain::Learning),
        &AttemptId::parse(&second).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(stored.retake_of, Some(AttemptId::parse(&first).unwrap()));
}

#[test]
fn lrn13_inferred_retake_moves_to_a_variant_that_was_never_shown() {
    let mut h = Harness::new();
    let def = seed(&h, 3);
    let (first, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["variant"], "3");
    finalize_at(&mut h, &first, [1; 6], "HUMAN_REVIEWER");
    let (second, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["variant"], "2");
    finalize_at(&mut h, &second, [1; 6], "HUMAN_REVIEWER");
    // Variant 3 (the original of the chain) was shown already; 1 was not.
    let (third, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["retake_of"], json!(second));
    assert_eq!(begun["result"]["variant"], "1");
    finalize_at(&mut h, &third, [1; 6], "HUMAN_REVIEWER");
    // Every variant was shown: the retake is flagged as not blind-eligible.
    let (_, begun) = to_review(&mut h, &def, None);
    assert_eq!(begun["result"]["variant"], "3");
    assert!(
        begun["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with("assessment.retake_exposed_variant")),
        "{begun}"
    );
}

#[test]
fn adr0003_approval_binds_to_the_stored_revision_when_the_client_sends_none() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (attempt, _) = to_review(&mut h, &def, None);
    let aid = AttemptId::parse(&attempt).unwrap();
    let failing = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [1; 6], "HUMAN_REVIEWER"),
        )
        .unwrap();
    assert_eq!(failing["result"]["grade"]["pass"], false);
    let graded_rev = failing["revision"].as_u64().unwrap();

    // Requested without meta.expected_revision: bound to the stored revision.
    let asked = h
        .call("assessment.approve_final", json!({ "attempt_id": attempt }))
        .unwrap_err();
    assert_eq!(asked.category, ErrorCategory::ApprovalRequired);
    let approval = asked.approval_id.unwrap();
    let row = g9_storage::approvals_store::get_request(h.conn(), &approval)
        .unwrap()
        .unwrap();
    assert_eq!(row.expected_revision.map(|r| r.get()), Some(graded_rev));
    h.decide(approval.as_str(), "APPROVE");

    // The grade changes after the grant (FAIL -> PASS).
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let passing = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4; 6], "HUMAN_REVIEWER"),
        )
        .unwrap();
    assert_eq!(passing["result"]["grade"]["pass"], true);

    // The approval granted on the failing grade cannot finalize the new one.
    let stale = h
        .call_with(
            "assessment.approve_final",
            json!({ "attempt_id": attempt }),
            &Opts::approval(approval.as_str()),
        )
        .unwrap_err();
    assert_eq!(stale.category, ErrorCategory::StaleRevision, "{stale:?}");
    assert!(assessment_repo::effective_final_decision(h.conn(), &aid)
        .unwrap()
        .is_none());
    assert_ne!(
        g9_storage::approvals_store::get_request(h.conn(), &approval)
            .unwrap()
            .unwrap()
            .state,
        g9_domain::ApprovalState::Consumed
    );
}

// ---------------------------------------------------------------------------
// skills.rs:314 — the interaction mode is backend-derived during an open exam
// ---------------------------------------------------------------------------

fn insert_session(h: &Harness, id: &str) -> g9_domain::SessionId {
    let sid = g9_domain::SessionId::parse(id).unwrap();
    g9_storage::repos::sessions::insert_session(
        h.conn(),
        &g9_storage::repos::sessions::StudySessionRow {
            id: sid.clone(),
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            state: g9_domain::SessionState::Planned,
            mode: g9_domain::InteractionMode::Study,
            zone: g9_domain::IanaZone::utc(),
            planned_start_utc: Some(h.now()),
            planned_end_utc: None,
            planned_minutes: 90,
            started_at: None,
            ended_at: None,
            credited_ms: g9_domain::DurationMs::ZERO,
            uncertain_ms: g9_domain::DurationMs::ZERO,
            data_domain: DataDomain::Learning,
            classification: g9_domain::Classification::Internal,
            revision: g9_domain::Revision::INITIAL,
            created_at: h.now(),
            updated_at: h.now(),
        },
    )
    .unwrap();
    sid
}

fn brief_output() -> Value {
    json!({
        "status": "COMPLETED",
        "result_schema_version": "1.0",
        "observations": [],
        "inferences": [],
        "evidence_refs": [],
        "confidence_limits": ["fixture"],
        "recommended_actions": [],
        "approval_requirements": [],
        "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 },
        "validation_results": [],
        "errors": [],
        "result": {
            "brief": { "day_id": "D15", "objectives": [], "due_reviews": [], "open_interventions": [], "risks": [] },
            "basis": { "evidence_refs": [], "unresolved_refs": [] }
        }
    })
}

#[test]
fn tec25_declared_mode_cannot_widen_the_backend_exam_mode() {
    use g9_ai_adapter::{ModelRef, ScriptedRuntime};
    use std::sync::Arc;
    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(g9_domain::ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    let out = brief_output();
    runtime.push_response(runtime.response(out.to_string(), Some(out)));
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime.clone();
    let mut h = Harness::with(|s| {
        s.runtime = Some(rt);
        s.skills = Some(registry);
    });
    let def = seed(&h, 1);
    let sid = insert_session(&h, "session-exam");
    h.call("session.prepare", json!({ "session_id": "session-exam" }))
        .unwrap();
    h.call("session.start", json!({ "session_id": "session-exam" }))
        .unwrap();
    h.advance(10 * 60_000);
    h.restart();
    let interval = g9_storage::repos::sessions::list_intervals(h.conn(), &sid).unwrap()[0]
        .id
        .clone();

    // Open blind attempt (STARTED, mode EXAM).
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    assert_eq!(begun["result"]["mode"], "EXAM");
    let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();

    // The UI declares STUDY: the skill (modes STUDY, LAB) is refused in the
    // backend-derived EXAM mode and the model is never called.
    let invoke = json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" } });
    let refused = h
        .call_with(
            "skills.invoke",
            invoke.clone(),
            &Opts {
                scope: Some(("LEARNING", "STUDY")),
                ..Opts::default()
            },
        )
        .unwrap_err();
    assert_eq!(refused.code, "skills.preflight_failed", "{refused:?}");
    assert_eq!(runtime.remaining(), 1, "the model was not called");
    // Time corrections are refused during the exam as well (no EXAM rule allows them).
    let correction = json!({ "session_id": "session-exam", "interval_id": interval, "credited_ms": 60_000, "reason": "gap" });
    let denied = h
        .call("session.correct_time", correction.clone())
        .unwrap_err();
    assert_eq!(denied.category, ErrorCategory::Unauthorized, "{denied:?}");

    // Once the attempt is submitted the declared mode applies again.
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    let ok = h.call("skills.invoke", invoke).unwrap();
    assert_eq!(ok["result"]["state"], "COMPLETED");
    assert_eq!(runtime.remaining(), 0);
    let corrected = h.call("session.correct_time", correction).unwrap();
    assert_eq!(corrected["result"]["state"], "PAUSED");
}
