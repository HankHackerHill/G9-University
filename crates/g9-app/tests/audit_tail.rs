#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Open-time detection of a truncated audit tail (security audit 2026-11-12, LOW "deleting the
//! newest audit rows is not detected"): the probe was `g9 init` reporting `audit_chain_ok: true`
//! over a chain whose newest rows had been deleted. Recovery now treats the missing rows like a
//! broken link: nothing is written over them, every mutation is refused, reads still answer.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use rusqlite::Connection;
use serde_json::json;

fn note(h: &mut Harness, n: u32) {
    h.call(
        "notes.create",
        json!({ "title": format!("Note {n}"), "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
    )
    .unwrap();
}

/// A writer with access to the database file: drops the delete guard, removes the newest audit
/// rows and puts the guard back exactly as it was (the schema check still passes).
fn delete_audit_rows_after(conn: &Connection, keep: i64) {
    let guard: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_audit_metadata_no_delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute_batch("DROP TRIGGER trg_audit_metadata_no_delete")
        .unwrap();
    conn.execute("DELETE FROM audit_metadata WHERE seq > ?1", [keep])
        .unwrap();
    conn.execute_batch(&guard).unwrap();
}

#[test]
fn newest_audit_rows_deleted_while_the_app_was_closed_block_every_mutation_at_the_next_open() {
    let mut h = Harness::new();
    for n in 0..4 {
        note(&mut h, n);
    }
    let rows = i64::try_from(h.audit_rows()).unwrap();
    assert!(rows >= 4, "{rows}");
    assert!(
        h.ctx().recovery_report().write_block_reason().is_none(),
        "a genuine database opens writable"
    );

    delete_audit_rows_after(h.conn(), rows - 2);
    h.restart();

    let report = h.ctx().recovery_report().clone();
    assert!(!report.audit_chain_ok, "{report:?}");
    assert_eq!(report.write_block_reason(), Some("audit_chain_broken"));
    assert_eq!(report.audit_rows, u64::try_from(rows - 2).unwrap());
    assert_eq!(
        report.audit_first_bad_seq,
        Some(rows - 1),
        "the first missing position: {report:?}"
    );

    let refused = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::DataCorruption);
    assert_eq!(refused.code, "recovery.integrity_failed");

    // Reads still answer, and the audit view says the chain is not intact.
    let audit = h.call("audit.recent", json!({})).unwrap();
    assert_eq!(audit["result"]["chain_ok"], false, "{audit}");
}

#[test]
fn a_database_with_real_backups_and_later_commands_still_opens_writable() {
    let mut h = Harness::with_dirs();
    note(&mut h, 0);
    h.call("backup.create", json!({})).unwrap();
    h.advance(60_000);
    for n in 1..4 {
        note(&mut h, n);
    }
    h.call("backup.create", json!({})).unwrap();
    // The manifests carry anchors, and the rows they name are still there.
    let anchors: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM backup_manifests WHERE audit_anchor_seq IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(anchors, 2);

    h.restart();
    let report = h.ctx().recovery_report().clone();
    assert!(report.audit_chain_ok, "{report:?}");
    assert_eq!(report.write_block_reason(), None);
    note(&mut h, 9);
}

#[test]
fn deleting_the_rows_a_backup_anchors_is_caught_even_when_the_counter_was_reset() {
    let mut h = Harness::with_dirs();
    note(&mut h, 0);
    h.call("backup.create", json!({})).unwrap();
    for n in 1..4 {
        note(&mut h, n);
    }
    let anchor: i64 = h
        .conn()
        .query_row(
            "SELECT audit_anchor_seq FROM backup_manifests WHERE audit_anchor_seq IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();

    // Everything after the row before the anchor goes, and the counter is set to match.
    delete_audit_rows_after(h.conn(), anchor - 1);
    h.conn()
        .execute(
            "UPDATE sqlite_sequence SET seq = ?1 WHERE name = 'audit_metadata'",
            [anchor - 1],
        )
        .unwrap();
    h.restart();

    let report = h.ctx().recovery_report().clone();
    assert!(!report.audit_chain_ok, "{report:?}");
    assert_eq!(report.audit_first_bad_seq, Some(anchor));
    assert_eq!(report.write_block_reason(), Some("audit_chain_broken"));
}
