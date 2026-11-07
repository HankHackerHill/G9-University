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
fn adr0012_operator_finalization_is_self_adjudicated_even_with_checker_sources() {
    let mut h = Harness::new();
    let def = seed(&h, 1);
    let (attempt, _) = to_review(&mut h, &def, None);
    // The operator claims checker sources for every criterion: before ADR-0012
    // wiring this was stored as DETERMINISTIC. The operator is the learner, so
    // the crate classifies the decision SELF_ADJUDICATED.
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4; 6], "DETERMINISTIC_CHECKER"),
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
    let aid = AttemptId::parse(&attempt).unwrap();
    let row = assessment_repo::effective_final_decision(h.conn(), &aid)
        .unwrap()
        .unwrap();
    assert_eq!(
        row.reliability_class,
        assessment_repo::ReliabilityClass::SelfAdjudicated
    );
    assert_eq!(row.decided_by, operator_id());
    assert!(row.passed);
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
