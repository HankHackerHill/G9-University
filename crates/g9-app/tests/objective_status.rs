#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-7: the status `curriculum.daily_plan` gives an objective.
//!
//! Every objective was emitted with the constant `PLANNED`, rendered as a state
//! badge, whatever its sessions did. The status is now derived from the study
//! sessions linked to the objective (`session.create` with `objective_id`):
//! `PLANNED` while none has started, `IN_PROGRESS` once one has, `COMPLETED`
//! when every linked session that was not cancelled is completed. The sessions
//! `session.plan_day` makes for the day's blocks are linked to the day and its
//! blocks, not to an objective (no manifest says which block serves which
//! objective), so they move no objective. Nothing derives `VERIFIED`: what
//! verifies an objective is an owner decision, so the plan never claims it.

mod common;

use common::{seed_manifest, Harness, Opts};
use serde_json::{json, Value};

const DATE: &str = "2026-11-03";

fn imported() -> Harness {
    let mut h = Harness::new();
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    h.call(
        "curriculum.set_program_start",
        json!({ "date": "2026-11-02" }),
    )
    .unwrap();
    h
}

fn objectives(h: &mut Harness) -> Vec<Value> {
    h.call("curriculum.daily_plan", json!({ "date": DATE }))
        .unwrap()["result"]["objectives"]
        .as_array()
        .unwrap()
        .clone()
}

fn statuses(h: &mut Harness) -> Vec<(String, String)> {
    objectives(h)
        .into_iter()
        .map(|o| {
            (
                o["objective_id"].as_str().unwrap().to_string(),
                o["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn status_of(h: &mut Harness, objective: &str) -> String {
    statuses(h)
        .into_iter()
        .find(|(o, _)| o == objective)
        .unwrap()
        .1
}

fn linked_session(h: &mut Harness, objective: &str) -> String {
    h.call(
        "session.create",
        json!({ "planned_minutes": 30, "objective_id": objective }),
    )
    .unwrap()["result"]["session_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn run(h: &mut Harness, op: &str, session: &str) {
    h.call(op, json!({ "session_id": session })).unwrap();
}

fn work_through(h: &mut Harness, session: &str) {
    run(h, "session.prepare", session);
    run(h, "session.start", session);
    run(h, "session.complete", session);
}

#[test]
fn an_objective_reports_what_its_linked_sessions_did() {
    let mut h = imported();
    let before = statuses(&mut h);
    assert_eq!(before.len(), 3);
    assert!(before.iter().all(|(_, s)| s == "PLANNED"), "{before:?}");
    let objective = before[0].0.clone();

    // A linked session that only exists changes nothing.
    let first = linked_session(&mut h, &objective);
    assert_eq!(status_of(&mut h, &objective), "PLANNED");

    // It starts: the objective is in progress; the others are not.
    run(&mut h, "session.prepare", &first);
    run(&mut h, "session.start", &first);
    assert_eq!(status_of(&mut h, &objective), "IN_PROGRESS");
    for (o, s) in statuses(&mut h) {
        if o != objective {
            assert_eq!(s, "PLANNED", "{o}");
        }
    }

    // It completes: every linked session is done.
    run(&mut h, "session.complete", &first);
    assert_eq!(status_of(&mut h, &objective), "COMPLETED");

    // Another session planned for it: work on the objective is not finished.
    let second = linked_session(&mut h, &objective);
    assert_eq!(status_of(&mut h, &objective), "IN_PROGRESS");
    work_through(&mut h, &second);
    assert_eq!(status_of(&mut h, &objective), "COMPLETED");
    assert!(statuses(&mut h).iter().all(|(_, s)| s != "VERIFIED"));
}

#[test]
fn the_days_block_sessions_are_not_linked_to_an_objective_and_move_none() {
    let mut h = imported();
    let planned = h.call("session.plan_day", json!({ "date": DATE })).unwrap();
    let sessions: Vec<String> = planned["result"]["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["session_id"].as_str().unwrap().to_string())
        .collect();
    work_through(&mut h, &sessions[0]);
    let after = statuses(&mut h);
    assert!(after.iter().all(|(_, s)| s == "PLANNED"), "{after:?}");
}

/// No operation cancels a session yet; the state is set the way storage would hold it.
fn cancel(h: &Harness, session: &str) {
    h.conn()
        .execute(
            "UPDATE study_sessions SET state = 'CANCELLED' WHERE id = ?1",
            [session],
        )
        .unwrap();
}

#[test]
fn a_cancelled_session_neither_starts_an_objective_nor_holds_it_back() {
    let mut h = imported();
    let objective = statuses(&mut h)[1].0.clone();
    let dropped = linked_session(&mut h, &objective);
    cancel(&h, &dropped);
    assert_eq!(status_of(&mut h, &objective), "PLANNED");
    let done = linked_session(&mut h, &objective);
    work_through(&mut h, &done);
    assert_eq!(status_of(&mut h, &objective), "COMPLETED");
}
