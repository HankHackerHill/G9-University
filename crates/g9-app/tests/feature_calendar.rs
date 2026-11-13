#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Calendar events the operator adds, changes and deletes (`calendar.create_event`,
//! `calendar.update_event`, `calendar.delete_event`). Only events the operator
//! added can be changed; deleting needs an approval (`operator.delete.approval`).

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

const HOUR: i64 = 3_600_000;

fn listed(h: &mut Harness) -> Vec<Value> {
    let from = h.now().as_millis() - 24 * HOUR;
    let to = h.now().as_millis() + 7 * 24 * HOUR;
    h.call("calendar.list", json!({ "from": from, "to": to }))
        .unwrap()["result"]["events"]
        .as_array()
        .unwrap()
        .clone()
}

fn create(h: &mut Harness, title: &str) -> Value {
    let start = h.now().as_millis() + 2 * HOUR;
    h.call(
        "calendar.create_event",
        json!({ "title": title, "kind": "APPOINTMENT", "start": start, "end": start + HOUR }),
    )
    .unwrap()
}

#[test]
fn an_added_event_is_listed_with_operator_provenance() {
    let mut h = Harness::new();
    let created = create(&mut h, "Dentist");
    let id = created["result"]["event_id"].as_str().unwrap().to_string();
    assert_eq!(created["result"]["provenance"], "OPERATOR");
    let event = listed(&mut h)
        .into_iter()
        .find(|e| e["event_id"] == id.as_str())
        .expect("listed");
    assert_eq!(event["title"], "Dentist");
    assert_eq!(event["kind"], "APPOINTMENT");
}

#[test]
fn an_added_event_can_be_moved_and_renamed() {
    let mut h = Harness::new();
    let created = create(&mut h, "Lab review");
    let id = created["result"]["event_id"].as_str().unwrap().to_string();
    let start = h.now().as_millis() + 5 * HOUR;
    let updated = h
        .call(
            "calendar.update_event",
            json!({ "event_id": id, "title": "Lab review (moved)", "start": start, "end": start + 2 * HOUR }),
        )
        .unwrap();
    assert_eq!(updated["result"]["title"], "Lab review (moved)");
    assert_eq!(updated["result"]["start"], start);
    assert_eq!(updated["revision"], 2);
}

#[test]
fn deleting_an_added_event_needs_an_approval() {
    let mut h = Harness::new();
    let created = create(&mut h, "Temporary");
    let id = created["result"]["event_id"].as_str().unwrap().to_string();
    let deleted = h.approved(
        "calendar.delete_event",
        json!({ "event_id": id }),
        &Opts::default(),
    );
    assert_eq!(deleted["result"]["deleted"], true);
    assert!(!listed(&mut h).iter().any(|e| e["event_id"] == id.as_str()));
}

#[test]
fn an_event_from_the_scheduler_cannot_be_changed() {
    let mut h = Harness::new();
    let created = create(&mut h, "Mine");
    let id = created["result"]["event_id"].as_str().unwrap().to_string();
    h.conn()
        .execute(
            "UPDATE calendar_events SET source = 'SCHEDULER' WHERE id = ?1",
            [&id],
        )
        .unwrap();
    let refused = h
        .call(
            "calendar.update_event",
            json!({ "event_id": id, "title": "renamed" }),
        )
        .unwrap_err();
    assert_eq!(refused.code, "calendar.event_not_editable", "{refused:?}");
}

#[test]
fn an_event_that_ends_before_it_starts_is_refused() {
    let mut h = Harness::new();
    let start = h.now().as_millis() + HOUR;
    let refused = h
        .call(
            "calendar.create_event",
            json!({ "title": "x", "kind": "REVIEW", "start": start, "end": start - 1 }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput);
    assert_eq!(refused.code, "calendar.event_range_invalid");
}
