#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The interaction mode is derived by the backend for every mutating operation (security audit
//! 2026-11-12, LOW "interaction mode is client-declared for 33 of 35 operations"; TEC-25, SEC-10):
//! while an EXAM attempt is `STARTED`, a command is evaluated in EXAM whatever mode the client
//! declared. Before, only `skills.invoke` and `session.correct_time` did this, so a client that
//! declared STUDY during an open exam was evaluated in STUDY by every other operation.
//!
//! What the policy denies in EXAM is the policy's own business (`g9-policy` rules); these tests
//! use one rule that exists today, "personal records are written in STUDY, REVIEW and EXECUTIVE
//! modes", to show that the mode the policy sees is the backend's.

mod common;

use common::*;
use g9_domain::{ErrorCategory, G9Error};
use serde_json::{json, Value};

/// Begin a blind attempt (STARTED, mode EXAM) and return its id.
fn open_exam(h: &mut Harness) -> String {
    let def = seed_blind_practical(h, 1, None);
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    assert_eq!(begun["result"]["mode"], "EXAM");
    begun["result"]["attempt_id"].as_str().unwrap().to_string()
}

/// A personal note, declared (PERSONAL, STUDY) the way a window in STUDY mode declares it.
#[allow(clippy::result_large_err)] // test helper; G9Error is the API's error type
fn personal_note(h: &mut Harness, title: &str) -> Result<Value, G9Error> {
    h.call_with(
        "notes.create",
        json!({ "title": title, "body_markdown": "private", "classification": "PRIVATE", "domain": "PERSONAL" }),
        &Opts {
            scope: Some(("PERSONAL", "STUDY")),
            ..Opts::default()
        },
    )
}

fn notes_titled(h: &Harness, title: &str) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE title = ?1",
            [title],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn a_write_is_evaluated_in_exam_while_an_exam_is_open_whatever_mode_the_client_declares() {
    let mut h = Harness::new();
    personal_note(&mut h, "before the exam").unwrap();

    let attempt = open_exam(&mut h);
    let refused = personal_note(&mut h, "during the exam").unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Unauthorized, "{refused:?}");
    assert_eq!(
        notes_titled(&h, "during the exam"),
        0,
        "nothing was written"
    );

    // The decision that was stored names the mode the backend evaluated, not the one declared.
    let (mode, decision): (String, String) = h
        .conn()
        .query_row(
            "SELECT scope_mode, decision FROM policy_decisions ORDER BY decided_at DESC, rowid DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((mode.as_str(), decision.as_str()), ("EXAM", "DENY"));

    // Once the attempt is submitted the declared mode applies again.
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    personal_note(&mut h, "after the exam").unwrap();
    assert_eq!(notes_titled(&h, "after the exam"), 1);
}

#[test]
fn no_open_exam_means_the_declared_mode_stands() {
    let mut h = Harness::new();
    // A completed attempt does not lock anything.
    let attempt = open_exam(&mut h);
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    personal_note(&mut h, "one").unwrap();
    personal_note(&mut h, "two").unwrap();
}

/// The backup operations copy and verify the database; they cannot reveal or change an exam. The
/// policy allows system writes only in MAINTENANCE, so forcing EXAM on them would let an exam
/// attempt that was left open block every backup. They are evaluated as declared.
#[test]
fn backups_keep_working_while_an_exam_is_open() {
    let mut h = Harness::with_dirs();
    open_exam(&mut h);
    let made = h.call("backup.create", json!({})).unwrap();
    let id = made["result"]["backup_id"].as_str().unwrap().to_string();
    h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    h.call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    let listed = h.call("backup.list", json!({})).unwrap();
    assert_eq!(listed["result"]["backups"].as_array().unwrap().len(), 1);
}

#[test]
fn reads_answer_while_an_exam_is_open() {
    let mut h = Harness::new();
    open_exam(&mut h);
    h.call("system.status", json!({})).unwrap();
    h.call("audit.recent", json!({})).unwrap();
    h.call("notes.list", json!({})).unwrap();
}
