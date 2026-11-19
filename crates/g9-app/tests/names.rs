#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! A title, a file name and a label are names: one line of visible text
//! (security audit 2026-11-13).
//!
//! Event titles accepted spaces alone and control characters, and a file name
//! accepted U+202E, which shows `pin<U+202E>txt.exe` as `pinexe.txt`. Names are
//! shown as facts in lists, in the native approval confirmation and in exports,
//! so they are refused at the boundary, for every operation, before anything is
//! stored. Free text (a note's body, a reason, a task statement) is not a name.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};

const TXT: &str = "aGVsbG8="; // "hello"

/// One valid request per operation that stores a name, with the name left to the caller.
fn requests(name: &str) -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "notes.create",
            "title",
            json!({ "title": name, "body_markdown": "b", "classification": "INTERNAL", "domain": "LEARNING" }),
        ),
        (
            "calendar.create_event",
            "title",
            json!({ "title": name, "kind": "APPOINTMENT", "start": 1_800_000_000_000_i64, "end": 1_800_003_600_000_i64 }),
        ),
        (
            "competency.create",
            "title",
            json!({ "code": "NAME-1", "title": name }),
        ),
        (
            "evidence.attach_file",
            "filename",
            json!({ "filename": name, "media_type": "text/plain", "content_base64": TXT }),
        ),
        ("backup.create", "label", json!({ "label": name })),
    ]
}

/// A harness with a folder for attached files and one for backups.
fn harness() -> Harness {
    let dir = tempfile::Builder::new().prefix("g9nm").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["attachments", "backups", "scratch"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.attachment_dir = Some(r.join("attachments"));
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    });
    h.dir = Some(dir);
    h
}

fn rows(h: &Harness) -> i64 {
    [
        "notes",
        "calendar_events",
        "competencies",
        "attachments",
        "backup_manifests",
    ]
    .iter()
    .map(|t| {
        h.conn()
            .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    })
    .sum()
}

#[test]
fn a_name_that_is_blank_or_holds_a_hidden_character_is_refused_and_nothing_is_stored() {
    let hostile = [
        ("   ", "is empty or only spaces"),
        ("\u{00A0}", "is empty or only spaces"),
        ("pin\u{202E}txt.exe", "U+202E"),
        ("first line\nApprove: nothing", "U+000A"),
        ("tab\there", "U+0009"),
        ("bell\u{0007}", "U+0007"),
        ("zero\u{200B}width", "U+200B"),
        ("tag\u{E0041}", "U+E0041"),
        ("sep\u{2028}arator", "U+2028"),
        // Audit 2026-11-19, NAME-1: characters that render as nothing or as blank space.
        ("invoice\u{3164}.exe", "U+3164"),
        ("\u{2800}x", "U+2800"),
        ("a\u{FFFC}b", "U+FFFC"),
        ("a\u{034F}b", "U+034F"),
        ("a\u{E0100}b", "U+E0100"),
        ("a\u{FE0F}b", "U+FE0F"),
        ("report.pdf\u{3000}\u{3000}.exe", "U+3000"),
        ("report\u{00A0}final", "U+00A0"),
        ("report   final", "holds two or more spaces in a row"),
    ];
    let mut h = harness();
    let before = rows(&h);
    for (name, why) in hostile {
        for (op, field, payload) in requests(name) {
            let err = h.call(op, payload).unwrap_err();
            assert_eq!(
                err.category,
                ErrorCategory::InvalidInput,
                "{op} {name:?}: {err:?}"
            );
            assert_eq!(
                err.code, "input.name_not_printable",
                "{op} {name:?}: {err:?}"
            );
            let details = err.details.clone().unwrap_or(Value::Null);
            assert_eq!(details["field"], field, "{op} {name:?}: {err:?}");
            assert!(
                details["reason"].as_str().unwrap_or("").contains(why),
                "{op} {name:?}: {err:?}"
            );
            // The refusal does not echo the hostile text.
            assert!(
                !err.message.contains(name) && !details.to_string().contains(name),
                "{err:?}"
            );
        }
    }
    assert_eq!(rows(&h), before, "a refused name left a row behind");
}

#[test]
fn ordinary_names_in_any_script_are_accepted() {
    let mut h = harness();
    for name in [
        "Week 3 lab notes",
        "Überblick: Tag 1 (Entwurf)",
        "報告書 v2",
        "evidence-final_2.txt",
    ] {
        for (op, _, mut payload) in requests(name) {
            if op == "competency.create" {
                // The code is unique per competency.
                payload["code"] = json!(format!("N{}", rows(&h)));
            }
            let out = h.call(op, payload);
            assert!(out.is_ok(), "{op} {name:?}: {out:?}");
        }
    }
}

/// Audit 2026-11-19, P2A-03: a review prompt is shown as a row title and inside a sentence of
/// facts, so it may keep layout and spaces but not a hidden, blank or reordering character.
#[test]
fn a_review_prompt_with_a_hidden_character_is_refused_and_nothing_is_stored() {
    let mut h = harness();
    let before = rows(&h);
    for hostile in [
        "Recall the flag \u{202E}.tpircs",
        "zero\u{200B}width",
        "filler\u{3164}",
        "bell\u{0007}",
    ] {
        let err = h
            .call(
                "retention.create_item",
                json!({ "prompt": hostile, "answer": "a" }),
            )
            .unwrap_err();
        assert_eq!(err.category, ErrorCategory::InvalidInput, "{hostile:?}");
        assert_eq!(err.code, "input.name_not_printable", "{hostile:?}: {err:?}");
        assert_eq!(err.details.unwrap()["field"], "prompt");
    }
    assert_eq!(rows(&h), before, "nothing was stored");
    // Layout, a no-break space and a non-ASCII letter are fine in free text.
    h.call(
        "retention.create_item",
        json!({ "prompt": "What does\u{00A0}HSTS prevent?\nName one \u{fc}ber-case.", "answer": "a" }),
    )
    .unwrap();
}

#[test]
fn free_text_keeps_its_line_breaks() {
    let mut h = Harness::new();
    let out = h
        .call(
            "notes.create",
            json!({ "title": "Lab notes", "body_markdown": "line one\n\n\tindented\nline three", "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    assert!(out["result"]["note_id"].is_string(), "{out}");
}
