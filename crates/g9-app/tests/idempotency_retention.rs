#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Data inventory gap G1 (`docs/data-model.md`): operator text that command
//! results repeat is kept verbatim in `idempotency_records.outcome_json`.
//!
//! * The documented retention (30 days, `docs/privacy/data-lifecycle.md`) was
//!   never enforced: `purge_older_than` had no caller. Recovery now purges older
//!   records at open, with its other sweeps, only when writes are allowed.
//! * A deleted calendar event's title outlived the event in the stored results
//!   of the commands that created or changed it. Deleting the event now replaces
//!   every stored result that holds its view with a content-free record; a
//!   replay of such a command answers that the event was deleted.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

fn idempotency_text(h: &Harness) -> String {
    h.conn()
        .query_row(
            "SELECT COALESCE(group_concat(outcome_json, ' '), '') FROM idempotency_records",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn idempotency_rows(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM idempotency_records", [], |r| r.get(0))
        .unwrap()
}

fn event_payload(h: &Harness, title: &str) -> Value {
    let start = h.now().as_millis() + 2 * HOUR;
    json!({ "title": title, "kind": "APPOINTMENT", "start": start, "end": start + HOUR })
}

#[test]
fn deleting_an_event_removes_its_title_from_the_stored_results() {
    let mut h = Harness::new();
    let create = event_payload(&h, "Dentist with Dr Example");
    let id = h
        .call_with(
            "calendar.create_event",
            create.clone(),
            &Opts::key("ev-create"),
        )
        .unwrap()["result"]["event_id"]
        .as_str()
        .unwrap()
        .to_string();
    let update = json!({ "event_id": id, "title": "Dentist with Dr Example, moved" });
    h.call_with(
        "calendar.update_event",
        update.clone(),
        &Opts::key("ev-update"),
    )
    .unwrap();
    // An event that stays keeps its stored result.
    h.call_with(
        "calendar.create_event",
        event_payload(&h, "Physiotherapy"),
        &Opts::key("ev-other"),
    )
    .unwrap();
    assert!(idempotency_text(&h).contains("Dr Example"));

    h.approved(
        "calendar.delete_event",
        json!({ "event_id": id }),
        &Opts::default(),
    );
    let text = idempotency_text(&h);
    assert!(!text.contains("Dr Example"), "{text}");
    assert!(text.contains("Physiotherapy"), "{text}");

    // A replay of either command says the event is gone; it creates nothing.
    for (op, payload, key) in [
        ("calendar.create_event", create, "ev-create"),
        ("calendar.update_event", update, "ev-update"),
    ] {
        let replay = h.call_with(op, payload, &Opts::key(key)).unwrap_err();
        assert_eq!(replay.category, ErrorCategory::NotFound, "{op}: {replay:?}");
        assert_eq!(replay.code, "calendar.event_deleted", "{op}: {replay:?}");
    }
    let events: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM calendar_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 1);
}

#[test]
fn records_older_than_thirty_days_are_purged_when_the_application_opens() {
    let mut h = Harness::new();
    h.call_with(
        "competency.create",
        json!({ "code": "OLD", "title": "Old competency title" }),
        &Opts::key("old"),
    )
    .unwrap();
    h.advance(29 * DAY);
    h.call_with(
        "competency.create",
        json!({ "code": "NEW", "title": "Recent competency title" }),
        &Opts::key("new"),
    )
    .unwrap();
    h.advance(2 * DAY);
    assert_eq!(idempotency_rows(&h), 2);

    h.restart();
    assert_eq!(h.ctx().recovery_report().purged_idempotency_records, 1);
    let text = idempotency_text(&h);
    assert!(!text.contains("Old competency title"), "{text}");
    assert!(text.contains("Recent competency title"), "{text}");
    assert_eq!(idempotency_rows(&h), 1);
    assert!(g9_storage::AuditChain::verify_chain(h.conn()).unwrap().ok);
}

#[test]
fn nothing_is_purged_while_writes_are_refused() {
    let mut h = Harness::new();
    h.call_with(
        "competency.create",
        json!({ "code": "OLD", "title": "Old competency title" }),
        &Opts::key("old"),
    )
    .unwrap();
    h.advance(31 * DAY);
    // A schema object the migrations do not produce: recovery writes nothing.
    h.conn()
        .execute_batch("CREATE INDEX zz_planted ON notes (title);")
        .unwrap();
    h.restart();
    assert_eq!(
        h.ctx().recovery_report().write_block_reason(),
        Some("schema_drift")
    );
    assert_eq!(idempotency_rows(&h), 1);
    assert_eq!(h.ctx().recovery_report().purged_idempotency_records, 0);
}
