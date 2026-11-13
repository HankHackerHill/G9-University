#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! storage-repos-1 (2026-11-10 review): `close_interval` refuses a wall end
//! before the wall start, and `session.pause` / `session.complete` passed the
//! current wall time unclamped, so after the system clock stepped backward
//! (an NTP correction, a manual change) stopping the timer failed and the
//! session stayed locked RUNNING. The wall end is now clamped to the interval's
//! start, as open-time recovery already does; the monotonic clock decides.

mod common;

use common::*;
use serde_json::json;
use std::sync::atomic::Ordering;

const MIN: i64 = 60_000;

fn running(h: &mut Harness, id: &str) {
    insert_planned_session(h, id);
    h.call("session.prepare", json!({ "session_id": id }))
        .unwrap();
    h.call("session.start", json!({ "session_id": id }))
        .unwrap();
}

fn state_of(h: &Harness, id: &str) -> String {
    h.conn()
        .query_row(
            "SELECT state FROM study_sessions WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

fn open_intervals(h: &Harness) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM time_intervals WHERE end_utc IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn pause_after_the_wall_clock_steps_back_an_hour_does_not_lock_the_timer() {
    let mut h = Harness::new();
    running(&mut h, "s-ntp");
    h.advance(10 * MIN);
    h.clock.fetch_sub(60 * MIN, Ordering::SeqCst); // wall clock only

    h.call("session.pause", json!({ "session_id": "s-ntp" }))
        .expect("pause must not fail after a backward clock step");
    assert_eq!(open_intervals(&h), 0, "no interval is left open");
    assert_eq!(
        state_of(&h, "s-ntp"),
        "INTERRUPTED",
        "the clocks disagree: the interval is UNCERTAIN and the session awaits a correction"
    );
}

#[test]
fn complete_after_a_small_backward_step_closes_the_interval() {
    let mut h = Harness::new();
    running(&mut h, "s-small");
    h.advance(1_000);
    h.clock.fetch_sub(2_000, Ordering::SeqCst); // within the 5 s tolerance

    h.call("session.complete", json!({ "session_id": "s-small" }))
        .expect("complete must not fail after a small backward clock step");
    assert_eq!(open_intervals(&h), 0);
    assert_eq!(state_of(&h, "s-small"), "COMPLETED");
}
