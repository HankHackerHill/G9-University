#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Review items the operator writes (`retention.create_item`) and review
//! results (`retention.record_review`). Before, `retention.due` was read-only
//! and the schedule never advanced.

mod common;

use common::*;
use serde_json::{json, Value};

const DAY: i64 = 86_400_000;

fn item(h: &mut Harness) -> String {
    let created = h
        .call(
            "retention.create_item",
            json!({ "prompt": "What does HSTS prevent?", "answer": "Protocol downgrade to plain HTTP." }),
        )
        .unwrap();
    created["result"]["review_item_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn record(h: &mut Harness, id: &str, outcome: &str) -> Value {
    h.call(
        "retention.record_review",
        json!({ "review_item_id": id, "outcome": outcome }),
    )
    .unwrap()["result"]
        .clone()
}

#[test]
fn a_new_item_is_due_after_a_day_and_its_answer_is_an_erasable_note() {
    let mut h = Harness::new();
    let created = h
        .call(
            "retention.create_item",
            json!({ "prompt": "Name the TLS 1.3 key exchange.", "answer": "(EC)DHE" }),
        )
        .unwrap();
    let r = &created["result"];
    assert_eq!(r["due_at"], h.now().as_millis() + DAY);
    assert_eq!(r["kind"], "DECLARATIVE");
    let answer_ref: String = h
        .conn()
        .query_row(
            "SELECT answer_ref FROM review_items WHERE id = ?1",
            [r["review_item_id"].as_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    let note_id = answer_ref.strip_prefix("note:").expect("note reference");
    let body: String = h
        .conn()
        .query_row(
            "SELECT body_markdown FROM notes WHERE id = ?1",
            [note_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(body, "(EC)DHE");
}

#[test]
fn successes_climb_the_baseline_ladder_and_a_failure_resets_it() {
    let mut h = Harness::new();
    let id = item(&mut h);
    h.advance(DAY);

    let first = record(&mut h, &id, "SUCCESS");
    assert_eq!(
        (first["stage"].as_u64(), first["offset_days"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(first["due_at"], h.now().as_millis() + DAY);

    h.advance(DAY);
    let second = record(&mut h, &id, "SUCCESS");
    assert_eq!(
        (second["stage"].as_u64(), second["offset_days"].as_u64()),
        (Some(2), Some(3))
    );

    h.advance(3 * DAY);
    let failed = record(&mut h, &id, "FAILURE");
    assert_eq!(
        (failed["stage"].as_u64(), failed["offset_days"].as_u64()),
        (Some(0), Some(1))
    );

    let attempts: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM review_attempts WHERE review_item_id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(attempts, 3, "every result is kept (append-only)");
}

#[test]
fn a_skipped_review_is_recorded_without_moving_the_schedule() {
    let mut h = Harness::new();
    let id = item(&mut h);
    h.advance(DAY);
    let before: i64 = h
        .conn()
        .query_row(
            "SELECT due_at FROM review_items WHERE id = ?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    let skipped = record(&mut h, &id, "SKIPPED");
    assert_eq!(skipped["due_at"], before);
    assert_eq!(skipped["stage"], 0);
}

#[test]
fn a_due_item_is_offered_by_retention_due() {
    let mut h = Harness::new();
    let id = item(&mut h);
    h.advance(DAY);
    let due = h.call("retention.due", json!({})).unwrap();
    let offered = due["result"]["due"]
        .as_array()
        .unwrap()
        .iter()
        .chain(due["result"]["overdue"].as_array().unwrap())
        .any(|i| i["review_item_id"] == id.as_str());
    assert!(offered, "{due}");
}
