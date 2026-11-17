#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-9: size bounds at the IPC boundary.
//!
//! Only a note's body was size-checked. A note title (create and revise), a
//! time-correction reason, a backup label, a skill code and a criterion code had
//! no length limit, `evidence_refs` and `criteria` no item limit, and dispatch no
//! payload byte cap: a multi-megabyte title was stored, listed and copied into
//! `idempotency_records`. Every one is now refused at the boundary,
//! `INVALID_INPUT`, before anything is stored.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

fn note_rows(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
        .unwrap()
}

fn assert_payload_invalid(h: &mut Harness, op: &str, payload: Value) {
    let refused = h.call(op, payload).unwrap_err();
    assert_eq!(
        refused.category,
        ErrorCategory::InvalidInput,
        "{op}: {refused:?}"
    );
    assert_eq!(refused.code, "ipc.payload_invalid", "{op}: {refused:?}");
    // The refusal never quotes the value it refused.
    let text = serde_json::to_string(&refused).unwrap();
    assert!(text.len() < 4_096, "{op}: refusal of {} bytes", text.len());
}

fn note(h: &mut Harness, title: &str) -> Value {
    h.call(
        "notes.create",
        json!({ "title": title, "body_markdown": "Body.", "classification": "INTERNAL", "domain": "LEARNING" }),
    )
    .unwrap()
}

#[test]
fn a_one_megabyte_note_title_is_refused_and_no_note_is_stored() {
    let mut h = Harness::new();
    assert_payload_invalid(
        &mut h,
        "notes.create",
        json!({ "title": "t".repeat(1_000_000), "body_markdown": "Body.", "classification": "INTERNAL", "domain": "LEARNING" }),
    );
    assert_eq!(note_rows(&h), 0);
    let idempotency: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM idempotency_records", [], |r| r.get(0))
        .unwrap();
    assert_eq!(idempotency, 0);
}

#[test]
fn a_note_title_of_200_characters_is_kept_and_201_is_refused_on_create_and_revise() {
    let mut h = Harness::new();
    // Characters, not bytes: 200 two-byte characters are accepted.
    let created = note(&mut h, &"é".repeat(200));
    let id = created["result"]["note_id"].as_str().unwrap().to_string();
    assert_payload_invalid(
        &mut h,
        "notes.create",
        json!({ "title": "t".repeat(201), "body_markdown": "Body.", "classification": "INTERNAL", "domain": "LEARNING" }),
    );
    assert_payload_invalid(
        &mut h,
        "notes.revise",
        json!({ "note_id": id, "title": "t".repeat(201), "body_markdown": "Body." }),
    );
    assert_eq!(note_rows(&h), 1);
    let title: String = h
        .conn()
        .query_row("SELECT title FROM notes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "é".repeat(200));
}

#[test]
fn reasons_labels_and_codes_have_a_length_limit() {
    let mut h = Harness::new();
    assert_payload_invalid(
        &mut h,
        "session.correct_time",
        json!({ "session_id": "session-1", "interval_id": "interval-1", "credited_ms": 0, "reason": "r".repeat(501) }),
    );
    assert_payload_invalid(&mut h, "backup.create", json!({ "label": "l".repeat(201) }));
    assert_payload_invalid(
        &mut h,
        "skills.invoke",
        json!({ "skill_code": "s".repeat(129), "input": {} }),
    );
    assert_payload_invalid(
        &mut h,
        "assessment.grade_provisional",
        json!({ "attempt_id": "attempt-1", "criteria": [
            { "criterion_code": "c".repeat(65), "score": 1, "source": "HUMAN_REVIEWER", "evidence_refs": [] }
        ] }),
    );
}

#[test]
fn evidence_lists_and_criteria_have_an_item_limit() {
    let mut h = Harness::new();
    let refs: Vec<String> = (0..101).map(|i| format!("evi-{i}")).collect();
    assert_payload_invalid(
        &mut h,
        "assessment.submit",
        json!({ "attempt_id": "attempt-1", "evidence_refs": refs }),
    );
    assert_payload_invalid(
        &mut h,
        "notes.create",
        json!({ "title": "T", "body_markdown": "Body.", "classification": "INTERNAL", "domain": "LEARNING", "evidence_refs": refs }),
    );
    assert_payload_invalid(
        &mut h,
        "assessment.grade_provisional",
        json!({ "attempt_id": "attempt-1", "criteria": [
            { "criterion_code": "c1", "score": 1, "source": "HUMAN_REVIEWER", "evidence_refs": refs }
        ] }),
    );
    let criteria: Vec<Value> = (0..65)
        .map(|i| json!({ "criterion_code": format!("c{i}"), "score": 1, "source": "HUMAN_REVIEWER", "evidence_refs": [] }))
        .collect();
    assert_payload_invalid(
        &mut h,
        "assessment.grade_provisional",
        json!({ "attempt_id": "attempt-1", "criteria": criteria }),
    );
    // The same evidence twice in one list is refused too (it would be linked twice).
    assert_payload_invalid(
        &mut h,
        "assessment.submit",
        json!({ "attempt_id": "attempt-1", "evidence_refs": ["evi-1", "evi-1"] }),
    );
}

#[test]
fn a_payload_over_the_byte_cap_is_refused_before_it_is_validated() {
    let mut h = Harness::new();
    // An unknown key would fail the schema; the size is checked first.
    let refused = h
        .call(
            "notes.create",
            json!({ "title": "T", "body_markdown": "b".repeat(26 * 1024 * 1024), "classification": "INTERNAL", "domain": "LEARNING", "unknown": true }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput, "{refused:?}");
    assert_eq!(refused.code, "ipc.payload_too_large", "{refused:?}");
    assert_eq!(note_rows(&h), 0);

    // Any other operation has a smaller cap.
    let refused = h
        .call(
            "calendar.list",
            json!({ "from": 0, "to": 1, "zone": "z".repeat(2 * 1024 * 1024) }),
        )
        .unwrap_err();
    assert_eq!(refused.code, "ipc.payload_too_large", "{refused:?}");
}

#[test]
fn a_note_body_at_its_byte_limit_still_fits_under_the_cap() {
    let mut h = Harness::new();
    // MAX_NOTE_BYTES of newlines: each serializes as two bytes.
    let body = "\n".repeat(usize::try_from(g9_evidence::MAX_NOTE_BYTES).unwrap());
    let created = h
        .call(
            "notes.create",
            json!({ "title": "Blank lines", "body_markdown": body, "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    assert!(created["result"]["note_id"].is_string());
}
