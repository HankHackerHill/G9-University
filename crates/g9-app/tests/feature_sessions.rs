#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `session.create`: the operator schedules a study session (it existed only as
//! seed data before), and it runs like any other session.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::json;

#[test]
fn a_created_session_is_planned_and_runs_to_completion() {
    let mut h = Harness::new();
    let created = h
        .call("session.create", json!({ "planned_minutes": 45 }))
        .unwrap();
    let r = &created["result"];
    assert_eq!(r["state"], "PLANNED", "{r}");
    assert_eq!(r["planned_ms"], 45 * 60_000);
    assert_eq!(r["day_id"], "UNLINKED");
    let id = r["session_id"].as_str().unwrap().to_string();

    h.call("session.prepare", json!({ "session_id": id }))
        .unwrap();
    h.call("session.start", json!({ "session_id": id }))
        .unwrap();
    h.advance(20 * 60_000);
    h.call("session.complete", json!({ "session_id": id }))
        .unwrap();

    let listed = h.call("session.list", json!({})).unwrap();
    let mine = listed["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["session_id"] == id.as_str())
        .cloned()
        .expect("the new session is listed");
    assert_eq!(mine["state"], "COMPLETED");
    assert_eq!(mine["credited_ms"], 20 * 60_000);
}

#[test]
fn an_exam_mode_session_is_refused_at_the_boundary() {
    let mut h = Harness::new();
    let refused = h
        .call(
            "session.create",
            json!({ "planned_minutes": 30, "mode": "EXAM" }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput, "{refused:?}");
}

#[test]
fn an_unknown_objective_is_refused() {
    let mut h = Harness::new();
    let refused = h
        .call(
            "session.create",
            json!({ "planned_minutes": 30, "objective_id": "NO-SUCH-OBJECTIVE" }),
        )
        .unwrap_err();
    assert!(
        matches!(
            refused.code.as_str(),
            "objective.not_found" | "curriculum.not_imported"
        ),
        "{refused:?}"
    );
}
