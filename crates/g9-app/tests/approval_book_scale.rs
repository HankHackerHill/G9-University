#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Audit 2026-11-18, finding F1: `ApprovalBook::hydrate` rebuilt the whole in-memory ledger
//! (serialise, insert one record, parse) for every stored approval row, so opening a database
//! that held N approval requests cost O(N squared). A caller that can reach the single IPC
//! bridge can mint approval requests without limit (terminal rows cannot be deleted), which
//! made the next start take hours. The hydration builds the ledger once.

mod common;

use common::*;
use g9_domain::{ApprovalId, ErrorCategory};
use g9_storage::approvals_store::{self, NewApprovalRequest};
use serde_json::json;
use std::time::{Duration, Instant};

/// Stored approval requests the test opens the database with. Quadratic hydration took 123
/// seconds at 2,000 on the machine that ran this test before the fix (NOT a benchmark);
/// linear hydration takes a fraction of a second.
const STORED: usize = 2_000;

#[test]
fn opening_a_database_with_many_approval_requests_builds_the_ledger_once() {
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
    let required = h
        .call(
            "deletion.request",
            json!({ "resource": { "kind": "NOTE", "id": note }, "reason": "erase" }),
        )
        .unwrap_err();
    assert_eq!(required.category, ErrorCategory::ApprovalRequired);
    let template = approvals_store::get_request(h.conn(), &required.approval_id.unwrap())
        .unwrap()
        .unwrap();

    h.conn().execute_batch("BEGIN IMMEDIATE").unwrap();
    for _ in 0..STORED {
        approvals_store::insert_request(
            h.conn(),
            &NewApprovalRequest {
                id: ApprovalId::new_v7(),
                operation: template.operation.clone(),
                required_capability: template.required_capability,
                requested_by: template.requested_by.clone(),
                requested_at: template.requested_at,
                expires_at: template.expires_at,
                expected_revision: template.expected_revision,
            },
        )
        .unwrap();
    }
    h.conn().execute_batch("COMMIT").unwrap();

    let started = Instant::now();
    h.restart();
    let took = started.elapsed();

    assert_eq!(
        h.ctx().approvals().len(),
        STORED + 1,
        "every stored request is in the ledger after the open"
    );
    assert!(
        took < Duration::from_secs(10),
        "opening with {STORED} stored approval requests took {took:?}: hydration is not linear"
    );
}
