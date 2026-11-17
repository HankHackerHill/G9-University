#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-8, storage-repos-9: a criterion result cites only evidence of the attempt
//! it grades.
//!
//! `assessment.grade_provisional` parsed each `evidence_refs` entry as an id and
//! wrote it into the append-only `criterion_results` row: an id that did not
//! exist, or that belonged to another attempt or domain, was stored for good,
//! shown in the grade view and hashed into the final decision. Each reference
//! must now be evidence linked to this attempt (`attempt_evidence`) in the
//! attempt's domain; anything else is `INVALID_INPUT grade.evidence_unknown`
//! and no result row is appended.

mod common;

use common::*;
use g9_domain::{AssessmentId, ErrorCategory};
use serde_json::{json, Value};

fn note_evidence(h: &mut Harness, title: &str) -> String {
    let note = h
        .call(
            "notes.create",
            json!({ "title": title, "body_markdown": "Write-up.", "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();
    h.call("evidence.from_note", json!({ "note_id": note }))
        .unwrap()["result"]["evidence_id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Begin, submit with `evidence`, and move to EVIDENCE_REVIEW.
fn reviewed_attempt(h: &mut Harness, assessment: &AssessmentId, evidence: &[&str]) -> String {
    let attempt = h
        .call(
            "assessment.begin",
            json!({ "assessment_id": assessment.as_str() }),
        )
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": evidence }),
    )
    .unwrap();
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    attempt
}

/// Every baseline criterion at level 3; the first one cites `refs`.
fn grade(attempt: &str, refs: &[&str]) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let cited: Vec<&str> = if i == 0 { refs.to_vec() } else { Vec::new() };
            json!({ "criterion_code": c.code, "score": 3, "source": "HUMAN_REVIEWER", "evidence_refs": cited })
        })
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

fn result_rows(h: &Harness, attempt: &str) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM criterion_results WHERE attempt_id = ?1",
            [attempt],
            |r| r.get(0),
        )
        .unwrap()
}

fn state(h: &Harness, attempt: &str) -> String {
    h.conn()
        .query_row("SELECT state FROM attempts WHERE id = ?1", [attempt], |r| {
            r.get(0)
        })
        .unwrap()
}

fn assert_refused(h: &mut Harness, attempt: &str, refs: &[&str]) {
    let refused = h
        .call("assessment.grade_provisional", grade(attempt, refs))
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput, "{refused:?}");
    assert_eq!(refused.code, "grade.evidence_unknown", "{refused:?}");
    assert_eq!(result_rows(h, attempt), 0, "no criterion result is stored");
    assert_eq!(state(h, attempt), "EVIDENCE_REVIEW");
}

#[test]
fn grading_with_an_evidence_id_that_does_not_exist_is_refused_and_appends_nothing() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let own = note_evidence(&mut h, "Boot chain write-up");
    let attempt = reviewed_attempt(&mut h, &assessment, &[own.as_str()]);
    assert_refused(&mut h, &attempt, &["evi-does-not-exist"]);
    // One unknown id among known ones is enough to refuse the whole grade.
    assert_refused(&mut h, &attempt, &[own.as_str(), "evi-does-not-exist"]);
}

#[test]
fn grading_with_evidence_not_submitted_for_this_attempt_is_refused() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let own = note_evidence(&mut h, "Boot chain write-up");
    let unlinked = note_evidence(&mut h, "Unrelated notes");
    let attempt = reviewed_attempt(&mut h, &assessment, &[own.as_str()]);

    // Evidence that exists but was never linked to any attempt.
    assert_refused(&mut h, &attempt, &[unlinked.as_str()]);

    // Evidence submitted for another attempt (of another assessment).
    let other_evidence = note_evidence(&mut h, "Other attempt write-up");
    let other_assessment = h
        .call(
            "assessment.define",
            json!({
                "code": "W1-OTHER",
                "title": "Another practical",
                "kind": "BLIND_PRACTICAL",
                "competency_id": COMPETENCY,
                "task_statement": "Recover another boot chain.",
                "difficulty": 3,
            }),
        )
        .unwrap()["result"]["assessment_id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = reviewed_attempt(
        &mut h,
        &AssessmentId::parse(&other_assessment).unwrap(),
        &[other_evidence.as_str()],
    );
    assert_ne!(other, attempt);
    assert_refused(&mut h, &attempt, &[other_evidence.as_str()]);
}

#[test]
fn grading_with_linked_evidence_of_another_domain_is_refused() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let personal = personal_evidence(&h, "evi-personal-1");
    let attempt = reviewed_attempt(&mut h, &assessment, &[]);
    // A link stored before submissions checked the domain.
    g9_storage::repos::assessment::link_attempt_evidence(
        h.conn(),
        &g9_storage::repos::assessment::AttemptEvidenceRow {
            attempt_id: attempt.parse().unwrap(),
            evidence_item_id: personal.parse().unwrap(),
            role: g9_storage::repos::assessment::AttemptEvidenceRole::Submission,
            added_at: h.now(),
        },
    )
    .unwrap();
    assert_refused(&mut h, &attempt, &[personal.as_str()]);
}

#[test]
fn grading_with_the_attempts_own_evidence_stores_the_reference() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let own = note_evidence(&mut h, "Boot chain write-up");
    let attempt = reviewed_attempt(&mut h, &assessment, &[own.as_str()]);
    let graded = h
        .call(
            "assessment.grade_provisional",
            grade(&attempt, &[own.as_str()]),
        )
        .unwrap();
    assert_eq!(graded["result"]["state"], "GRADED_PROVISIONAL");
    assert_eq!(result_rows(&h, &attempt), 6);
    let cited: String = h
        .conn()
        .query_row(
            "SELECT evidence_refs FROM criterion_results WHERE attempt_id = ?1 AND evidence_refs <> '[]'",
            [attempt.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&cited).unwrap(),
        vec![own]
    );
}
