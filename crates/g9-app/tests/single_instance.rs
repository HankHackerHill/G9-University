#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-4 (2026-11-10 review): open-time recovery treats an open timer interval
//! as a crash and closes it UNCERTAIN, so a second process opening the same
//! data (the CLI, a second window) interrupted the first one's live session.
//! One process at a time now holds the data: a second open is refused before
//! recovery runs, and the live session is untouched.

mod common;

use common::*;
use g9_domain::G9Error;
use g9_storage::Database;
use serde_json::json;
use std::path::Path;

#[allow(clippy::result_large_err)] // test helper; G9Error is the API's error type
fn open(path: &Path) -> Result<Harness, G9Error> {
    let db = Database::open(path, g9_app::database_options("0.1.0-test")).unwrap();
    Harness::with_database(db, |_| {})
}

#[test]
fn a_second_process_cannot_open_the_data_while_a_session_runs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g9.sqlite");
    let mut first = open(&path).unwrap();
    insert_planned_session(&first, "s-live");
    first
        .call("session.prepare", json!({ "session_id": "s-live" }))
        .unwrap();
    first
        .call("session.start", json!({ "session_id": "s-live" }))
        .unwrap();

    let refused = open(&path)
        .err()
        .expect("a second open of the same data must be refused");
    assert_eq!(refused.code, "app.data_in_use", "{refused:?}");

    let state: String = first
        .conn()
        .query_row(
            "SELECT state FROM study_sessions WHERE id = 's-live'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "RUNNING", "the live session is not interrupted");
    let open_intervals: i64 = first
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM time_intervals WHERE end_utc IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(open_intervals, 1, "the live interval stays open");
}

#[test]
fn the_data_opens_again_once_the_first_context_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g9.sqlite");
    let first = open(&path).unwrap();
    drop(first);
    assert!(
        open(&path).is_ok(),
        "the lock is released with the context that held it"
    );
}
