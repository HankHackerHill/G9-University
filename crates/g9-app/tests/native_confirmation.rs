#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! ADR-0014, SEC-03: an approval was granted by any `approvals.decide` call,
//! so script running in the webview could approve its own request. The
//! webview's request is now a proposal: the grant is the operator's answer to
//! a confirmation shown outside the webview, whose text Rust writes from the
//! stored request. Rejecting needs no confirmation.

mod common;

use common::*;
use g9_domain::{ApprovalId, ErrorCategory};
use serde_json::json;

/// A security-sensitive note needs an approval
/// (`operator.security_sensitive.write.approval`).
fn pending_approval(h: &mut Harness) -> ApprovalId {
    h.call(
        "notes.create",
        json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "SECURITY_SENSITIVE" }),
    )
    .unwrap_err()
    .approval_id
    .unwrap()
}

fn state_of(h: &Harness, id: &ApprovalId) -> String {
    let row = g9_storage::approvals_store::get_request(h.conn(), id)
        .unwrap()
        .unwrap();
    serde_json::to_value(row.state)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn approve(id: &ApprovalId) -> serde_json::Value {
    json!({ "approval_id": id.as_str(), "decision": "APPROVE" })
}

#[test]
fn a_grant_sent_from_the_webview_alone_is_refused() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let refused = h.call("approvals.decide", approve(&id)).unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Unauthorized, "{refused:?}");
    assert_eq!(refused.code, "approval.native_confirmation_required");
    assert_eq!(state_of(&h, &id), "PENDING");
}

#[test]
fn a_grant_the_operator_declines_in_the_dialog_is_refused() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let refused = h
        .confirmed_with("approvals.decide", approve(&id), &mut |_| false)
        .unwrap_err();
    assert_eq!(refused.code, "approval.not_confirmed", "{refused:?}");
    assert_eq!(state_of(&h, &id), "PENDING");
}

#[test]
fn a_confirmed_grant_goes_through_and_the_dialog_text_comes_from_the_stored_request() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let stored = g9_storage::approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap();
    let mut shown = None;
    h.confirmed_with("approvals.decide", approve(&id), &mut |summary| {
        shown = Some(summary.clone());
        true
    })
    .unwrap();
    assert_eq!(state_of(&h, &id), "APPROVED");

    let summary = shown.expect("the operator was asked");
    assert_eq!(summary.approval_id, id.as_str());
    assert_eq!(summary.operation, "notes.create");
    assert_eq!(summary.resource_kind, "NOTE");
    assert_eq!(summary.resource_id, stored.operation.resource.id);
    assert_eq!(summary.expires_at, stored.expires_at);
    let text = summary.text();
    assert!(text.contains("notes.create"), "{text}");
    assert!(text.contains(&stored.operation.resource.id), "{text}");
}

#[test]
fn rejecting_needs_no_confirmation() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    h.call(
        "approvals.decide",
        json!({ "approval_id": id.as_str(), "decision": "REJECT" }),
    )
    .unwrap();
    assert_eq!(state_of(&h, &id), "REJECTED");
}
