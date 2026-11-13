#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-1 (2026-11-10 review): an approval-gated create could never complete.
//! The handler minted a fresh resource id on every call, the approval bound the
//! first call's id, and the approved retry presented a new one
//! (`approval.resource_mismatch`). A retry that carries an approval for the
//! same kind of creation now reuses the approved id; the pipeline still
//! verifies the whole binding, so the approval creates exactly what the
//! operator approved and nothing else.

mod common;

use common::*;
use g9_domain::{ApprovalId, ErrorCategory};
use serde_json::{json, Value};

fn sensitive_note(body: &str) -> Value {
    json!({ "title": "lab", "body_markdown": body, "classification": "PRIVATE", "domain": "SECURITY_SENSITIVE" })
}

fn with_approval(approval: &ApprovalId) -> Opts {
    Opts {
        approval_id: Some(approval.as_str().to_string()),
        ..Opts::default()
    }
}

fn approved_request(h: &mut Harness, payload: &Value) -> ApprovalId {
    let asked = h.call("notes.create", payload.clone()).unwrap_err();
    assert_eq!(asked.category, ErrorCategory::ApprovalRequired, "{asked:?}");
    let approval = asked.approval_id.unwrap();
    h.decide(approval.as_str(), "APPROVE");
    approval
}

#[test]
fn an_approved_security_sensitive_note_is_created_on_the_approved_retry() {
    let mut h = Harness::new();
    let payload = sensitive_note("rotated the lab VPN credentials");
    let approval = approved_request(&mut h, &payload);

    let created = h
        .call_with("notes.create", payload, &with_approval(&approval))
        .expect("the approved retry must create the note");
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    let bound = g9_storage::approvals_store::get_request(h.conn(), &approval)
        .unwrap()
        .unwrap()
        .operation
        .resource
        .id;
    assert_eq!(note_id, bound, "the note carries the id that was approved");
    let stored: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE id = ?1",
            [&note_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, 1);
}

#[test]
fn the_approval_cannot_create_a_note_with_other_content() {
    let mut h = Harness::new();
    let approval = approved_request(&mut h, &sensitive_note("approved text"));
    let before: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
        .unwrap();

    let refused = h
        .call_with(
            "notes.create",
            sensitive_note("different text"),
            &with_approval(&approval),
        )
        .unwrap_err();
    assert!(
        refused.code.starts_with("approval."),
        "a different payload must not consume the approval: {refused:?}"
    );
    let after: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, before, "nothing was created");
}
