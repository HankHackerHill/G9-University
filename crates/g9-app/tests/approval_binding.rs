#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Approval binding to the stored revision (security audit 2026-11-12, MEDIUM
//! "approval pre-bound to a future revision").
//!
//! An approval must bind to the revision the operator can actually see. Before the
//! fix, the request bound to the client's `expected_revision` without comparing it
//! with the stored one, so a client could ask for an approval of a FAILING attempt
//! "at revision r+2", regrade it to PASSING after the grant (r+1, r+2) and consume
//! the approval: a final, immutable grade the operator never saw.

mod common;

use common::*;
use g9_domain::{ApprovalId, ErrorCategory};
use serde_json::{json, Value};

fn levels_payload(attempt: &str, levels: [u8; 6]) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .zip(levels)
        .map(|(c, l)| json!({ "criterion_code": c.code, "score": l, "source": "HUMAN_REVIEWER", "evidence_refs": [] }))
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

/// An attempt graded FAILING, and its revision.
fn failing_attempt(h: &mut Harness) -> (String, u64) {
    seed_blind_practical(h, 1, Some(3));
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": "ad-w1" }))
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
    let failing = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [1; 6]),
        )
        .unwrap();
    assert_eq!(failing["result"]["grade"]["pass"], false);
    let r = failing["revision"].as_u64().unwrap();
    (attempt, r)
}

fn approval_rows(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM approval_requests", [], |r| r.get(0))
        .unwrap()
}

fn stored_binding(h: &Harness, id: &ApprovalId) -> Option<u64> {
    g9_storage::approvals_store::get_request(h.conn(), id)
        .unwrap()
        .unwrap()
        .expected_revision
        .map(|r| r.get())
}

#[test]
fn an_approval_cannot_be_requested_for_a_revision_the_attempt_does_not_have() {
    let mut h = Harness::new();
    let (attempt, r) = failing_attempt(&mut h);
    let before = approval_rows(&h);

    let refused = h
        .call_with(
            "assessment.approve_final",
            json!({ "attempt_id": attempt }),
            &Opts::revision(r + 2),
        )
        .unwrap_err();
    assert_eq!(
        refused.category,
        ErrorCategory::StaleRevision,
        "{refused:?}"
    );
    assert!(
        refused.approval_id.is_none(),
        "no approval may be issued for a revision the operator cannot see"
    );
    assert_eq!(
        approval_rows(&h),
        before,
        "no approval request row is created"
    );
}

#[test]
fn the_same_holds_for_another_approval_gated_operation() {
    let mut h = Harness::new();
    let created = h
        .call("notes.create", json!({ "title": "t", "body_markdown": "private", "classification": "PRIVATE", "domain": "LEARNING" }))
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    let rev = created["revision"].as_u64().unwrap();
    let before = approval_rows(&h);

    let refused = h
        .call_with(
            "deletion.request",
            json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "erase" }),
            &Opts::revision(rev + 5),
        )
        .unwrap_err();
    assert_eq!(
        refused.category,
        ErrorCategory::StaleRevision,
        "{refused:?}"
    );
    assert_eq!(approval_rows(&h), before);
}

#[test]
fn a_request_at_the_current_revision_still_gets_an_approval_bound_to_it() {
    let mut h = Harness::new();
    let (attempt, r) = failing_attempt(&mut h);
    let asked = h
        .call_with(
            "assessment.approve_final",
            json!({ "attempt_id": attempt }),
            &Opts::revision(r),
        )
        .unwrap_err();
    assert_eq!(asked.category, ErrorCategory::ApprovalRequired, "{asked:?}");
    assert_eq!(stored_binding(&h, &asked.approval_id.unwrap()), Some(r));
}

#[test]
fn without_a_client_revision_the_approval_binds_to_the_stored_one_and_a_regrade_refuses_it() {
    let mut h = Harness::new();
    let (attempt, r) = failing_attempt(&mut h);
    let asked = h
        .call("assessment.approve_final", json!({ "attempt_id": attempt }))
        .unwrap_err();
    let approval = asked.approval_id.unwrap();
    assert_eq!(stored_binding(&h, &approval), Some(r));
    h.decide(approval.as_str(), "APPROVE");

    // Regrade after the grant: the attempt moves to r + 2 and now passes.
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let passing = h
        .call(
            "assessment.grade_provisional",
            levels_payload(&attempt, [4; 6]),
        )
        .unwrap();
    assert_eq!(passing["result"]["grade"]["pass"], true);
    let now = passing["revision"].as_u64().unwrap();

    for expected in [None, Some(now)] {
        let refused = h
            .call_with(
                "assessment.approve_final",
                json!({ "attempt_id": attempt }),
                &Opts {
                    approval_id: Some(approval.as_str().to_string()),
                    expected_revision: expected,
                    ..Opts::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            refused.code, "approval.revision_mismatch",
            "expected_revision {expected:?}: {refused:?}"
        );
    }
}
