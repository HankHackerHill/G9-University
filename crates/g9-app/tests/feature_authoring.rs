#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Authoring (`competency.create`, `assessment.define`): competencies and
//! assessments the operator writes, usable end to end like seeded ones.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

fn levels(attempt: &str, level: u8) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| json!({ "criterion_code": c.code, "score": level, "source": "HUMAN_REVIEWER", "evidence_refs": [] }))
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

fn competency(h: &mut Harness, code: &str) -> String {
    let created = h
        .call(
            "competency.create",
            json!({ "code": code, "title": "Web cache poisoning", "validity_days": 120 }),
        )
        .unwrap();
    created["result"]["competency_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn define(h: &mut Harness, competency_id: &str) -> Value {
    h.call(
        "assessment.define",
        json!({
            "code": "WCP-01",
            "title": "Poison a cache key",
            "kind": "BLIND_PRACTICAL",
            "competency_id": competency_id,
            "task_statement": "Find an unkeyed header that changes the cached response.",
            "difficulty": 3
        }),
    )
    .unwrap()["result"]
        .clone()
}

#[test]
fn an_authored_assessment_is_taken_graded_and_feeds_its_competency() {
    let mut h = Harness::new();
    let cid = competency(&mut h, "WCP");
    let defined = define(&mut h, &cid);
    let assessment_id = defined["assessment_id"].as_str().unwrap().to_string();
    assert_eq!(
        defined["criteria"].as_array().unwrap().len(),
        g9_assessment::Rubric::baseline().criteria().len()
    );

    let listed = h.call("assessment.list", json!({})).unwrap();
    assert!(
        listed.to_string().contains(&assessment_id),
        "the authored assessment is listed: {listed}"
    );

    let begun = h
        .call(
            "assessment.begin",
            json!({ "assessment_id": assessment_id }),
        )
        .unwrap();
    let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [], "notes": "done" }),
    )
    .unwrap();
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    h.call("assessment.grade_provisional", levels(&attempt, 4))
        .unwrap();
    let final_grade = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts {
            scope: Some(("LEARNING", "REVIEW")),
            ..Opts::default()
        },
    );
    assert_eq!(
        final_grade["result"]["grade"]["pass"], true,
        "{final_grade}"
    );

    let projection = h
        .call("competency.projection", json!({ "competency_id": cid }))
        .unwrap();
    assert!(
        projection.to_string().contains(&cid),
        "the competency has a projection from the authored assessment: {projection}"
    );
}

#[test]
fn an_assessment_for_an_unknown_competency_is_refused() {
    let mut h = Harness::new();
    let refused = h
        .call(
            "assessment.define",
            json!({
                "code": "X-1", "title": "x", "kind": "CAPSTONE", "competency_id": "comp-missing",
                "task_statement": "x", "difficulty": 2
            }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::NotFound, "{refused:?}");
}

#[test]
fn a_competency_code_is_unique() {
    let mut h = Harness::new();
    competency(&mut h, "DUP");
    let again = h
        .call(
            "competency.create",
            json!({ "code": "DUP", "title": "again" }),
        )
        .unwrap_err();
    assert!(
        matches!(
            again.category,
            ErrorCategory::Conflict | ErrorCategory::InvalidInput
        ),
        "{again:?}"
    );
}
