#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! core-7 (2026-11-10 review; SEC-02 correction in `docs/audit/defect-register.md`): an approval
//! retried while the wall clock stood before its request (or decision) instant is refused as
//! expired, fail closed. The refusal left the row `APPROVED` (the expiry sweep only matches
//! `expires_at <= now`), so once the clock was corrected the same approval was consumed. The
//! refusal now persists the expiry of the presented approval.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::json;
use std::sync::atomic::Ordering;

fn approval_state(h: &Harness, id: &str) -> String {
    h.conn()
        .query_row(
            "SELECT state FROM approval_requests WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

fn note_erased(h: &Harness, id: &str) -> i64 {
    h.conn()
        .query_row("SELECT erased FROM notes WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

#[test]
fn an_approval_refused_on_a_backwards_clock_stays_expired_after_the_clock_is_corrected() {
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
    let required = h.call("deletion.request", payload.clone()).unwrap_err();
    assert_eq!(required.category, ErrorCategory::ApprovalRequired);
    let id = required.approval_id.unwrap().as_str().to_string();
    h.decide(&id, "APPROVE");
    assert_eq!(approval_state(&h, &id), "APPROVED");

    // The wall clock steps back a minute, before the request instant.
    h.clock.fetch_sub(60_000, Ordering::SeqCst);
    let refused = h
        .call_with("deletion.request", payload.clone(), &Opts::approval(&id))
        .unwrap_err();
    assert_eq!(refused.code, "approval.expired", "{refused:?}");
    assert_eq!(
        approval_state(&h, &id),
        "EXPIRED",
        "the fail-closed expiry is persisted"
    );
    assert_eq!(
        h.ctx()
            .approvals()
            .get(&g9_domain::ApprovalId::parse(&id).unwrap())
            .map(|r| r.state),
        Some(g9_domain::ApprovalState::Expired),
        "the ledger agrees with the row"
    );

    // The clock is corrected, well inside the approval's window: the approval stays spent.
    h.clock.fetch_add(61_000, Ordering::SeqCst);
    let again = h
        .call_with("deletion.request", payload, &Opts::approval(&id))
        .unwrap_err();
    assert_eq!(again.code, "approval.not_approved", "{again:?}");
    assert_eq!(approval_state(&h, &id), "EXPIRED");
    assert_eq!(note_erased(&h, &note), 0, "nothing was erased");
}

#[test]
fn an_approval_refused_for_another_reason_is_not_expired() {
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
    let id = h
        .call("deletion.request", payload.clone())
        .unwrap_err()
        .approval_id
        .unwrap()
        .as_str()
        .to_string();
    h.decide(&id, "APPROVE");
    // Different parameters: refused as a mismatch, and the approval keeps its state.
    let other = json!({ "resource": { "kind": "NOTE", "id": note }, "reason": "another reason" });
    let refused = h
        .call_with("deletion.request", other, &Opts::approval(&id))
        .unwrap_err();
    assert_ne!(refused.code, "approval.expired", "{refused:?}");
    assert_eq!(approval_state(&h, &id), "APPROVED");
    let done = h
        .call_with("deletion.request", payload, &Opts::approval(&id))
        .unwrap();
    assert_eq!(done["result"]["state"], "APPLIED");
}

/// A pending deletion approval for a fresh note: (note id, deletion payload, approval id).
fn pending_deletion(h: &mut Harness) -> (String, serde_json::Value, String) {
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
    let id = h
        .call("deletion.request", payload.clone())
        .unwrap_err()
        .approval_id
        .unwrap()
        .as_str()
        .to_string();
    (note, payload, id)
}

/// `approvals.decide` APPROVE the way the desktop bridge sends it (native confirmation given).
#[allow(clippy::result_large_err)] // the error type of the harness (`G9Error`)
fn approve(h: &mut Harness, id: &str) -> Result<serde_json::Value, g9_domain::G9Error> {
    h.confirmed_with(
        "approvals.decide",
        json!({ "approval_id": id, "decision": "APPROVE" }),
        &mut |_| true,
    )
}

fn decisions_for(h: &Harness, id: &str) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM approval_decisions WHERE approval_id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

/// 2026-11-17 rework (core-7, the approve path): `approvals.decide` on a wall clock that stands
/// before the request instant is refused as expired by the ledger (fail closed). The refusal rolled
/// back, the row stayed `PENDING`, and once the clock was corrected the same request was approved
/// and spent.
#[test]
fn an_approval_decided_on_a_backwards_clock_stays_expired_after_the_clock_is_corrected() {
    let mut h = Harness::new();
    let (note, payload, id) = pending_deletion(&mut h);
    assert_eq!(approval_state(&h, &id), "PENDING");

    h.clock.fetch_sub(60_000, Ordering::SeqCst);
    let audit_before = h.audit_rows();
    let refused = approve(&mut h, &id).unwrap_err();
    assert_eq!(refused.code, "approval.expired", "{refused:?}");
    assert_eq!(
        refused.category,
        ErrorCategory::ApprovalRequired,
        "the caller sees the ledger's refusal unchanged"
    );
    assert_eq!(
        approval_state(&h, &id),
        "EXPIRED",
        "the fail-closed expiry is persisted"
    );
    assert_eq!(
        h.ctx()
            .approvals()
            .get(&g9_domain::ApprovalId::parse(&id).unwrap())
            .map(|r| r.state),
        Some(g9_domain::ApprovalState::Expired),
        "the ledger agrees with the row"
    );
    assert_eq!(
        decisions_for(&h, &id),
        0,
        "no operator decision is recorded"
    );
    assert_eq!(
        h.audit_rows(),
        audit_before + 1,
        "the refusal is audited once"
    );

    // The clock is corrected, well inside the request's window: the request stays expired.
    h.clock.fetch_add(61_000, Ordering::SeqCst);
    let again = approve(&mut h, &id).unwrap_err();
    assert_eq!(again.code, "approval.expired", "{again:?}");
    assert_eq!(approval_state(&h, &id), "EXPIRED");
    let spend = h
        .call_with("deletion.request", payload, &Opts::approval(&id))
        .unwrap_err();
    assert_eq!(spend.code, "approval.not_approved", "{spend:?}");
    assert_eq!(approval_state(&h, &id), "EXPIRED");
    assert_eq!(note_erased(&h, &note), 0, "nothing was erased");
}

/// The other expiry refusal of `approvals.decide` (the window has passed): it is persisted too,
/// so a wall clock stepped back into the window afterwards cannot revive the request.
#[test]
fn an_approval_decided_after_its_window_stays_expired_when_the_clock_steps_back() {
    let mut h = Harness::new();
    let (note, payload, id) = pending_deletion(&mut h);

    h.clock.fetch_add(11 * 60_000, Ordering::SeqCst);
    let refused = approve(&mut h, &id).unwrap_err();
    assert_eq!(refused.code, "approval.expired", "{refused:?}");
    assert_eq!(refused.category, ErrorCategory::PreconditionFailed);
    assert_eq!(
        approval_state(&h, &id),
        "EXPIRED",
        "the expiry is persisted"
    );

    h.clock.fetch_sub(10 * 60_000, Ordering::SeqCst);
    let again = approve(&mut h, &id).unwrap_err();
    assert_eq!(again.code, "approval.expired", "{again:?}");
    assert_eq!(approval_state(&h, &id), "EXPIRED");
    assert_eq!(decisions_for(&h, &id), 0);
    let spend = h
        .call_with("deletion.request", payload, &Opts::approval(&id))
        .unwrap_err();
    assert_eq!(spend.code, "approval.not_approved", "{spend:?}");
    assert_eq!(note_erased(&h, &note), 0, "nothing was erased");
}

/// Control: a request decided inside its window on a sane clock is approved and spent as before;
/// rejecting needs no clock check and is unchanged.
#[test]
fn an_approval_decided_inside_its_window_is_approved_as_before() {
    let mut h = Harness::new();
    let (_, payload, id) = pending_deletion(&mut h);
    let view = approve(&mut h, &id).unwrap();
    assert_eq!(view["result"]["state"], "APPROVED");
    assert_eq!(decisions_for(&h, &id), 1);
    let done = h
        .call_with("deletion.request", payload, &Opts::approval(&id))
        .unwrap();
    assert_eq!(done["result"]["state"], "APPLIED");

    let (_, _, other) = pending_deletion(&mut h);
    h.clock.fetch_sub(60_000, Ordering::SeqCst);
    let rejected = h
        .call(
            "approvals.decide",
            json!({ "approval_id": other, "decision": "REJECT" }),
        )
        .unwrap();
    assert_eq!(rejected["result"]["state"], "REJECTED");
}
