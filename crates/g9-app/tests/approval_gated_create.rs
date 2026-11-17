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

#[test]
fn an_approval_presented_on_other_content_does_not_take_the_approved_id() {
    // Final security review authz-3: creation_id reused the approved id for any
    // create of the same kind that presented the approval. A plain create with
    // other content (policy allows it without approval) then took the id, the
    // approved retry failed with storage.unique_violation, and the approval's
    // confirmation described the decoy record.
    let mut h = Harness::new();
    let approved = sensitive_note("rotated the lab VPN credentials");
    let approval = approved_request(&mut h, &approved);
    let bound = g9_storage::approvals_store::get_request(h.conn(), &approval)
        .unwrap()
        .unwrap()
        .operation
        .resource
        .id;

    let decoy = json!({ "title": "decoy", "body_markdown": "x", "classification": "INTERNAL", "domain": "LEARNING" });
    if let Ok(made) = h.call_with("notes.create", decoy, &with_approval(&approval)) {
        assert_ne!(
            made["result"]["note_id"],
            json!(bound),
            "the decoy took the approved id"
        );
    }

    let created = h
        .call_with("notes.create", approved, &with_approval(&approval))
        .expect("the approved retry must still create the approved note");
    assert_eq!(created["result"]["note_id"], json!(bound));
}
