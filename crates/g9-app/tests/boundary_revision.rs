#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Security audit 2026-11-12 (LOW): an approval-gated command with `expected_revision` 0 reached
//! the storage CHECK constraint, answered raw SQLite text and left no decision or audit row.
//! The revision is refused at the boundary, constraint text never reaches the caller, and a
//! refusal whose approval request cannot be stored is still recorded.

mod common;

use common::{Harness, Opts};
use g9_domain::ErrorCategory;
use serde_json::json;

fn export_payload() -> serde_json::Value {
    json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false })
}

fn count(h: &Harness, table: &str) -> i64 {
    h.conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_revision_of_zero_is_refused_at_the_boundary_without_storage_text() {
    let mut h = Harness::with_dirs();
    let before = (
        count(&h, "approval_requests"),
        count(&h, "policy_decisions"),
    );
    let e = h
        .call_with("export.request", export_payload(), &Opts::revision(0))
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::InvalidInput);
    assert_eq!(e.code, "meta.invalid", "{e:?}");
    let text = format!("{} {}", e.message, e.details.clone().unwrap_or_default());
    for leak in ["CHECK", "constraint", "expected_revision IS NULL", "SQLITE"] {
        assert!(
            !text.contains(leak),
            "the refusal shows storage text ({leak}): {text}"
        );
    }
    // Refused before policy: nothing was stored for it.
    assert_eq!(
        before,
        (
            count(&h, "approval_requests"),
            count(&h, "policy_decisions")
        )
    );
}

#[test]
fn a_revision_beyond_what_storage_can_hold_is_refused_the_same_way() {
    let mut h = Harness::with_dirs();
    for revision in [u64::MAX, i64::MAX as u64 + 1] {
        let e = h
            .call_with(
                "export.request",
                export_payload(),
                &Opts::revision(revision),
            )
            .unwrap_err();
        assert_eq!(e.category, ErrorCategory::InvalidInput, "{revision}");
        assert_eq!(e.code, "meta.invalid", "{revision}: {e:?}");
    }
    // The largest revision storage holds is still a plain (stale or approval) answer, not a refusal of the meta.
    let e = h
        .call_with(
            "export.request",
            export_payload(),
            &Opts::revision(i64::MAX as u64),
        )
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::ApprovalRequired, "{e:?}");
}

#[test]
fn a_refusal_whose_approval_request_cannot_be_stored_is_still_recorded_and_says_nothing_of_the_storage(
) {
    let mut h = Harness::with_dirs();
    // Make the request insert fail the way a damaged or full database would.
    h.conn()
        .execute_batch(
            "CREATE TRIGGER zz_no_approval_requests BEFORE INSERT ON approval_requests
             BEGIN SELECT RAISE(ABORT, 'SQLITE-INTERNAL-DETAIL'); END;",
        )
        .unwrap();
    let decisions = count(&h, "policy_decisions");
    let audit = h.audit_rows();
    let e = h.call("export.request", export_payload()).unwrap_err();
    assert_eq!(e.code, "approval.request_not_stored", "{e:?}");
    assert!(!e.message.contains("SQLITE-INTERNAL-DETAIL"), "{e:?}");
    assert!(
        !format!("{:?}", e.details).contains("SQLITE-INTERNAL-DETAIL"),
        "{e:?}"
    );
    // The refused attempt left its decision and its audit row, without an approval id.
    assert_eq!(count(&h, "policy_decisions"), decisions + 1);
    assert_eq!(h.audit_rows(), audit + 1);
    let orphan: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM policy_decisions WHERE approval_id IS NOT NULL AND approval_id NOT IN (SELECT id FROM approval_requests)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphan, 0);
}
