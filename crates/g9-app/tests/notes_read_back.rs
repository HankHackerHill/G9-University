#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Reading notes back (owner request 2026-11-14): `notes.list` shows what is stored without the
//! bodies, `notes.get` opens one note, and neither shows what was erased or what lies in another
//! domain than the scope of the request.

mod common;

use common::{Harness, Opts};
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

fn create(h: &mut Harness, title: &str, body: &str) -> String {
    h.call(
        "notes.create",
        json!({ "title": title, "body_markdown": body, "classification": "INTERNAL", "domain": "LEARNING" }),
    )
    .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn list(h: &mut Harness, payload: Value) -> Vec<Value> {
    h.call("notes.list", payload).unwrap()["result"]["notes"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn the_list_shows_the_newest_change_first_and_never_a_body() {
    let mut h = Harness::new();
    let first = create(&mut h, "First", "alpha");
    h.advance(1_000);
    let second = create(&mut h, "Second", "bravo bravo");
    h.advance(1_000);
    let third = create(&mut h, "Third", "charlie");
    h.advance(1_000);
    h.call_with(
        "notes.revise",
        json!({ "note_id": first, "title": "First, revised", "body_markdown": "alpha alpha alpha" }),
        &Opts::revision(1),
    )
    .unwrap();

    let notes = list(&mut h, json!({}));
    let order: Vec<&str> = notes
        .iter()
        .map(|n| n["note_id"].as_str().unwrap())
        .collect();
    assert_eq!(order, [first.as_str(), third.as_str(), second.as_str()]);
    assert_eq!(notes[0]["title"], "First, revised");
    assert_eq!(notes[0]["version_count"], 2);
    assert_eq!(notes[0]["revision"], 2);
    assert_eq!(notes[0]["size_bytes"], "alpha alpha alpha".len());
    assert_eq!(notes[2]["version_count"], 1);
    assert_eq!(notes[2]["classification"], "INTERNAL");
    assert_eq!(notes[2]["domain"], "LEARNING");
    assert_eq!(notes[2]["objective_id"], Value::Null);
    for n in &notes {
        assert!(
            n.get("body_markdown").is_none(),
            "a list must not carry bodies"
        );
        assert!(n.get("content_sha256").is_none());
    }
}

#[test]
fn the_limit_bounds_the_list_and_a_limit_of_zero_is_refused() {
    let mut h = Harness::new();
    for i in 0..3 {
        create(&mut h, &format!("Note {i}"), "x");
        h.advance(1_000);
    }
    assert_eq!(list(&mut h, json!({ "limit": 2 })).len(), 2);
    assert_eq!(list(&mut h, json!({})).len(), 3);
    let e = h.call("notes.list", json!({ "limit": 0 })).unwrap_err();
    assert_eq!(e.category, ErrorCategory::InvalidInput);
}

#[test]
fn opening_a_note_returns_its_body_and_unknown_ids_are_not_found() {
    let mut h = Harness::new();
    let id = create(
        &mut h,
        "Door code",
        "the code is on the second line\nsecond line",
    );
    let note = h.call("notes.get", json!({ "note_id": id })).unwrap();
    let r = &note["result"];
    assert_eq!(r["note_id"], id.as_str());
    assert_eq!(r["title"], "Door code");
    assert_eq!(
        r["body_markdown"],
        "the code is on the second line\nsecond line"
    );
    assert_eq!(r["version_count"], 1);
    assert_eq!(r["revision"], 1);

    let e = h
        .call("notes.get", json!({ "note_id": "note-does-not-exist" }))
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::NotFound);
    assert_eq!(e.code, "note.not_found");
}

#[test]
fn a_note_of_another_domain_is_neither_listed_nor_opened_from_this_scope() {
    let mut h = Harness::new();
    let learning = create(&mut h, "Learning note", "visible");
    let other = Opts {
        scope: Some(("SECURITY_SENSITIVE", "STUDY")),
        ..Opts::default()
    };
    let sensitive = h.approved(
        "notes.create",
        json!({ "title": "Scope letter", "body_markdown": "authorized targets", "classification": "SECRET", "domain": "SECURITY_SENSITIVE" }),
        &other,
    )["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();

    let here = list(&mut h, json!({}));
    assert_eq!(here.len(), 1);
    assert_eq!(here[0]["note_id"], learning.as_str());
    let there = h.call_with("notes.list", json!({}), &other).unwrap()["result"]["notes"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(there.len(), 1);
    assert_eq!(there[0]["note_id"], sensitive.as_str());
    assert_eq!(there[0]["classification"], "SECRET");

    // The other domain's note cannot be opened with the learning scope.
    let e = h
        .call("notes.get", json!({ "note_id": sensitive }))
        .unwrap_err();
    assert!(
        matches!(
            e.category,
            ErrorCategory::Unauthorized | ErrorCategory::NotFound
        ),
        "{e:?}"
    );
    // With its own scope it opens, body included.
    let opened = h
        .call_with("notes.get", json!({ "note_id": sensitive }), &other)
        .unwrap();
    assert_eq!(opened["result"]["body_markdown"], "authorized targets");
}

#[test]
fn an_erased_note_leaves_the_list_and_cannot_be_opened() {
    let mut h = Harness::new();
    let keep = create(&mut h, "Keep", "stays");
    h.advance(1_000);
    let gone = create(&mut h, "Door code", "pin-code-4711");
    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": gone }, "reason": "the door code changed" }),
        &Opts::default(),
    );
    assert_eq!(done["result"]["state"], "APPLIED", "{done}");

    let notes = list(&mut h, json!({}));
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["note_id"], keep.as_str());
    let e = h.call("notes.get", json!({ "note_id": gone })).unwrap_err();
    assert_eq!(e.category, ErrorCategory::NotFound);
    assert_eq!(e.code, "note.erased");
    // Nothing of the erased body is left to read through the new operations.
    let stored = serde_json::to_string(&h.call("notes.list", json!({})).unwrap()).unwrap();
    assert!(!stored.contains("4711"));
}
