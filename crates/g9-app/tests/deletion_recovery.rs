#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-10 (2026-11-10 review): `deletion.request` commits the consumed approval, the `REQUESTED`
//! row and a SUCCESS audit row in its command transaction; the erasure runs afterwards in a
//! transaction of its own. When the process dies between the two commits, or the erasure fails,
//! the approval is spent and the content is intact, and nothing reported it. Open-time recovery
//! now lists those requests (`RecoveryReport::pending_deletions`) and `system.status` warns
//! while their content is still stored. They are reported, not executed: the erasure needs a
//! fresh approval (fail closed, `commands/privacy.rs`).
//!
//! The crash is simulated by a trigger that makes the erasure transaction fail after the command
//! transaction committed; the trigger is dropped before the restart, so the schema the next open
//! verifies is exactly the migrated one. The database then holds what a crash between the two
//! commits leaves (plus the failure audit row of the post-commit step).

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

const BODY: &str = "PLUGH-sentinel body that must be erased";

fn warnings(outcome: &Value) -> Vec<String> {
    outcome["warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.as_str().map(str::to_string))
        .collect()
}

fn one(h: &Harness, sql: &str, arg: &str) -> String {
    h.conn().query_row(sql, [arg], |r| r.get(0)).unwrap()
}

/// Leave `h` in the state a crash between `deletion.request`'s two commits leaves; returns
/// (note id, deletion payload, approval id, deletion request id).
fn crash_between_the_commits(h: &mut Harness) -> (String, Value, String, String) {
    let note = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": BODY, "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();
    let payload = json!({ "resource": { "kind": "NOTE", "id": note }, "reason": "erase" });
    let approval = h
        .call("deletion.request", payload.clone())
        .unwrap_err()
        .approval_id
        .unwrap()
        .as_str()
        .to_string();
    h.decide(&approval, "APPROVE");

    h.conn()
        .execute_batch(
            "CREATE TRIGGER zz_fail_erasure BEFORE INSERT ON deletion_tombstones
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
    let failed = h
        .call_with(
            "deletion.request",
            payload.clone(),
            &Opts::approval(&approval),
        )
        .unwrap_err();
    assert_ne!(
        failed.category,
        ErrorCategory::ApprovalRequired,
        "{failed:?}"
    );
    h.conn()
        .execute_batch("DROP TRIGGER zz_fail_erasure")
        .unwrap();

    // What the first commit left: a spent approval, a REQUESTED request, a SUCCESS audit row for
    // deletion.request, and the content.
    let request = one(
        h,
        "SELECT id FROM deletion_requests WHERE approval_id = ?1",
        &approval,
    );
    assert_eq!(
        one(
            h,
            "SELECT state FROM deletion_requests WHERE id = ?1",
            &request
        ),
        "REQUESTED"
    );
    assert_eq!(
        one(
            h,
            "SELECT state FROM approval_requests WHERE id = ?1",
            &approval
        ),
        "CONSUMED"
    );
    assert_eq!(
        one(
            h,
            "SELECT outcome FROM audit_metadata WHERE operation = ?1 ORDER BY seq DESC LIMIT 1",
            "deletion.request"
        ),
        "SUCCESS"
    );
    assert_eq!(
        one(h, "SELECT body_markdown FROM notes WHERE id = ?1", &note),
        BODY
    );

    (note, payload, approval, request)
}

#[test]
fn an_erasure_that_did_not_run_after_the_request_committed_is_reported_at_open_and_in_status() {
    let mut h = Harness::new();
    let (_, payload, _, request) = crash_between_the_commits(&mut h);
    h.restart();
    let report = serde_json::to_value(h.ctx().recovery_report()).unwrap();
    assert_eq!(report["pending_deletions"], json!([request]), "{report}");
    assert_eq!(h.ctx().recovery_report().write_block_reason(), None);

    let status = h.call("system.status", json!({})).unwrap();
    let pending: Vec<String> = warnings(&status)
        .into_iter()
        .filter(|w| w.starts_with("deletion.erasure_incomplete:"))
        .collect();
    assert_eq!(pending.len(), 1, "{status}");
    assert!(pending[0].contains(&request), "{pending:?}");
    assert!(!pending[0].contains("PLUGH"), "{pending:?}");

    // The operator requests the deletion again with a fresh approval: the content is erased
    // and the warning goes away (the old request stays REQUESTED, its subject is erased).
    let done = h.approved("deletion.request", payload, &Opts::default());
    assert_eq!(done["result"]["state"], "APPLIED");
    let status = h.call("system.status", json!({})).unwrap();
    assert!(
        warnings(&status)
            .iter()
            .all(|w| !w.starts_with("deletion.erasure_incomplete:")),
        "{status}"
    );
    h.restart();
    let report = serde_json::to_value(h.ctx().recovery_report()).unwrap();
    assert_eq!(report["pending_deletions"], json!([]), "{report}");
}

#[test]
fn a_completed_erasure_leaves_nothing_to_report() {
    let mut h = Harness::new();
    let note = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();
    let payload = json!({ "resource": { "kind": "NOTE", "id": note }, "reason": "erase" });
    let done = h.approved("deletion.request", payload, &Opts::default());
    assert_eq!(done["result"]["state"], "APPLIED");
    h.restart();
    let report = serde_json::to_value(h.ctx().recovery_report()).unwrap();
    assert_eq!(report["pending_deletions"], json!([]), "{report}");
    let status = h.call("system.status", json!({})).unwrap();
    assert!(
        warnings(&status)
            .iter()
            .all(|w| !w.starts_with("deletion.erasure_incomplete:")),
        "{status}"
    );
}

/// Rework (2026-11-17): the listing at open is a report, not a precondition. A request row that
/// cannot be decoded (a REQUESTED row is not immutable, and `report_json` is only checked to be
/// JSON) made the whole open fail; the open now goes on and reports the error code, and
/// `system.status` warns that the list is unreadable instead of "nothing pending".
#[test]
fn an_unreadable_pending_deletion_does_not_stop_the_open() {
    let mut h = Harness::new();
    let (_, _, _, request) = crash_between_the_commits(&mut h);
    h.conn()
        .execute(
            "UPDATE deletion_requests SET report_json = '{}' WHERE id = ?1",
            [&request],
        )
        .unwrap();

    let db = h.ctx.take().unwrap().into_database();
    let reopened = Harness::with_database(db, |_| {});
    let mut h = match reopened {
        Ok(h) => h,
        Err(e) => panic!("the open failed on an unreadable deletion request: {e:?}"),
    };
    h.continue_ids_from(10_000);
    let report = h.ctx().recovery_report().clone();
    assert_eq!(report.write_block_reason(), None, "{report:?}");
    assert!(report.pending_deletions.is_empty(), "{report:?}");
    let code = report
        .pending_deletions_unreadable
        .clone()
        .expect("the listing error is reported");
    assert!(code.starts_with("storage."), "{code}");

    let status = h.call("system.status", json!({})).unwrap();
    let unreadable: Vec<String> = warnings(&status)
        .into_iter()
        .filter(|w| w.starts_with("deletion.erasure_incomplete_unreadable:"))
        .collect();
    assert_eq!(unreadable.len(), 1, "{status}");
    assert!(!unreadable[0].contains(&request), "{unreadable:?}");
}
