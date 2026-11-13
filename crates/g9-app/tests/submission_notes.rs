#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-5 (2026-11-10 review): `assessment.submit` accepted `notes` and kept only
//! their hash, so the text the operator typed was lost. The text is now stored
//! as a note (`source_kind = ASSESSMENT_SUBMISSION`), wrapped in an evidence
//! item and linked to the attempt as SUBMISSION evidence, all in the submitting
//! transaction. As a note it stays erasable through `deletion.request`.

mod common;

use common::*;
use serde_json::json;

fn begun_attempt(h: &mut Harness) -> String {
    seed_blind_practical(h, 1, Some(3));
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": "ad-w1" }))
        .unwrap();
    begun["result"]["attempt_id"].as_str().unwrap().to_string()
}

fn note_count(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn notes_typed_at_submission_are_stored_and_linked_to_the_attempt() {
    let mut h = Harness::new();
    let attempt = begun_attempt(&mut h);
    let text = "Ran out of time on part 3; the parser handles CRLF only.";
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [], "notes": text }),
    )
    .unwrap();

    let (body, source, role, kind): (String, String, String, String) = h
        .conn()
        .query_row(
            "SELECT n.body_markdown, n.source_kind, ae.role, e.kind
               FROM attempt_evidence ae
               JOIN evidence_items e ON e.id = ae.evidence_item_id
               JOIN notes n ON n.id = e.note_id
              WHERE ae.attempt_id = ?1",
            [&attempt],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("the submission notes must be reachable from the attempt");
    assert_eq!(body, text);
    assert_eq!(source, "ASSESSMENT_SUBMISSION");
    assert_eq!(role, "SUBMISSION");
    assert_eq!(kind, "NOTE");
}

#[test]
fn a_submission_without_notes_creates_no_note() {
    let mut h = Harness::new();
    let attempt = begun_attempt(&mut h);
    let before = note_count(&h);
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [], "notes": "   " }),
    )
    .unwrap();
    assert_eq!(note_count(&h), before, "blank notes store nothing");
}
