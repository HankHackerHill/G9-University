#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The tail of the audit chain (security audit 2026-11-12, LOW "deleting the newest audit rows is
//! not detected although the backup anchors prove the rows existed"). Every link of the chain
//! verifies when the newest rows are simply gone, so `verify_chain` also checks what the chain
//! cannot say about itself: the sequence counter that `AUTOINCREMENT` keeps (the largest `seq`
//! ever allocated) and the rows that backup manifests name as anchors.

mod common;

use common::*;
use g9_domain::{BackupId, RequestId, ResourceKind, ResourceRef, TraceId};
use g9_storage::audit::{AuditChain, AuditDecision, AuditEntry, AuditOutcome};
use g9_storage::backup::{create_snapshot, record_manifest, SnapshotRequest};
use rusqlite::{params, Connection};

fn append(conn: &Connection, n: i64) {
    AuditChain::append(
        conn,
        &AuditEntry {
            occurred_at: t(n),
            actor: operator(),
            operation: "session.start".to_string(),
            resource: Some(ResourceRef::new(ResourceKind::StudySession, "sess-1")),
            decision: AuditDecision::Allow,
            outcome: AuditOutcome::Success,
            request_id: RequestId::parse(&format!("req-{n}")).unwrap(),
            trace_id: TraceId::parse("trace-1").unwrap(),
            details_sha256: Some(sha("details")),
        },
    )
    .unwrap();
}

/// A database whose chain holds exactly `rows` rows.
fn chain_of(rows: i64) -> g9_storage::Database {
    let db = open_mem();
    for n in 0..rows {
        append(db.conn(), n);
    }
    assert_eq!(count(db.conn(), "audit_metadata"), rows);
    db
}

/// What the audit describes: a writer with access to the database file drops the delete guard,
/// removes the newest rows and puts the guard back exactly as it was, so the schema still
/// matches the migrations.
fn delete_rows_after(conn: &Connection, keep: i64) {
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

fn set_counter(conn: &Connection, value: i64) {
    conn.execute(
        "UPDATE sqlite_sequence SET seq = ?1 WHERE name = 'audit_metadata'",
        [value],
    )
    .unwrap();
}

/// A manifest row that names `seq` and `hash` as the audit anchor.
fn record_anchor(conn: &Connection, id: &str, seq: i64, hash: &str) {
    conn.execute(
        "INSERT INTO backup_manifests (id, created_at, app_version, schema_version, page_count, sha256_of_file,
            state, path, audit_anchor_seq, audit_anchor_hash, revision, updated_at)
         VALUES (?1, 1, '0.1.0-test', 1, 1, ?2, 'SNAPSHOT_CREATED', '/backups/x.sqlite', ?3, ?4, 1, 1)",
        params![id, sha(id), seq, hash],
    )
    .unwrap();
}

fn hash_at(conn: &Connection, seq: i64) -> String {
    conn.query_row(
        "SELECT row_hash FROM audit_metadata WHERE seq = ?1",
        [seq],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn an_intact_chain_with_anchors_verifies() {
    let db = chain_of(6);
    record_anchor(db.conn(), "bk-5", 5, &hash_at(db.conn(), 5));
    record_anchor(db.conn(), "bk-2", 2, &hash_at(db.conn(), 2));
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(report.ok, "{report:?}");
    assert_eq!(report.rows, 6);
    assert_eq!(report.first_bad_seq, None);
    // An empty chain with nothing allocated is intact too.
    let empty = open_mem();
    assert!(AuditChain::verify_chain(empty.conn()).unwrap().ok);
}

#[test]
fn deleting_the_newest_rows_is_detected_by_the_sequence_counter() {
    let db = chain_of(6);
    delete_rows_after(db.conn(), 4);
    assert_eq!(count(db.conn(), "audit_metadata"), 4);
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(
        !report.ok,
        "the four rows left link correctly; the missing two must still be reported: {report:?}"
    );
    assert_eq!(report.rows, 4);
    assert_eq!(report.first_bad_seq, Some(5), "the first missing position");
    let reason = report.first_bad_reason.unwrap();
    assert!(reason.contains("missing"), "{reason}");
}

#[test]
fn deleting_every_row_is_detected() {
    let db = chain_of(3);
    delete_rows_after(db.conn(), 0);
    assert_eq!(count(db.conn(), "audit_metadata"), 0);
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok, "{report:?}");
    assert_eq!(report.rows, 0);
    assert_eq!(report.first_bad_seq, Some(1));
}

#[test]
fn a_writer_that_also_resets_the_counter_is_caught_by_a_backup_anchor() {
    let db = chain_of(6);
    record_anchor(db.conn(), "bk-5", 5, &hash_at(db.conn(), 5));
    delete_rows_after(db.conn(), 3);
    set_counter(db.conn(), 3);
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(
        !report.ok,
        "the counter now agrees with the rows, the anchor does not: {report:?}"
    );
    assert_eq!(report.first_bad_seq, Some(5), "the anchored position");
    let reason = report.first_bad_reason.unwrap();
    assert!(reason.contains("bk-5"), "names the backup: {reason}");
}

#[test]
fn an_anchor_whose_row_carries_another_hash_is_reported() {
    let db = chain_of(4);
    record_anchor(db.conn(), "bk-wrong", 3, &sha("not the row at seq 3"));
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok, "{report:?}");
    assert_eq!(report.first_bad_seq, Some(3));
    assert!(report.first_bad_reason.unwrap().contains("bk-wrong"));
}

#[test]
fn a_manifest_with_half_an_anchor_is_reported() {
    let db = chain_of(2);
    db.conn()
        .execute(
            "INSERT INTO backup_manifests (id, created_at, app_version, schema_version, page_count, sha256_of_file,
                state, audit_anchor_seq, revision, updated_at)
             VALUES ('bk-half', 1, '0.1.0-test', 1, 1, ?1, 'REQUESTED', 2, 1, 1)",
            [sha("half")],
        )
        .unwrap();
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok, "{report:?}");
    assert!(report.first_bad_reason.unwrap().contains("bk-half"));
}

/// The limit of this check, stated so that it cannot be mistaken for more: the anchors and the
/// counter live in the same file as the rows. A writer that removes the rows, resets the counter
/// and rewrites or deletes the anchors leaves a chain that verifies. (ADR-0004: tamper evidence
/// against a same-user writer needs an anchor kept outside the file; PROPOSED, not built.)
#[test]
fn a_writer_that_also_rewrites_the_anchors_is_not_detected_which_is_the_documented_limit() {
    let db = chain_of(6);
    record_anchor(db.conn(), "bk-5", 5, &hash_at(db.conn(), 5));
    delete_rows_after(db.conn(), 3);
    set_counter(db.conn(), 3);
    db.conn()
        .execute("DELETE FROM backup_manifests WHERE id = 'bk-5'", [])
        .unwrap();
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(report.ok, "{report:?}");
    assert_eq!(report.rows, 3);
}

#[test]
fn a_real_snapshot_records_an_anchor_that_verifies_and_one_that_stops_matching_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let db = chain_of(5);
    let dest = dir.path().join("b1.sqlite");
    let request = SnapshotRequest {
        backup_id: BackupId::parse("b1").unwrap(),
        requested_at: t(100),
        app_version: "0.1.0-test".into(),
    };
    let manifest = create_snapshot(db.conn(), &dest, &request).unwrap();
    record_manifest(db.conn(), &manifest, t(101)).unwrap();
    assert_eq!(manifest.audit_anchor.as_ref().unwrap().seq, 5);
    assert!(AuditChain::verify_chain(db.conn()).unwrap().ok);

    // More rows later: the anchor still names a row that exists.
    append(db.conn(), 10);
    assert!(AuditChain::verify_chain(db.conn()).unwrap().ok);

    // The newest rows go, counter included; only the anchor of the real backup remains to say so.
    delete_rows_after(db.conn(), 4);
    set_counter(db.conn(), 4);
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok, "{report:?}");
    assert_eq!(report.first_bad_seq, Some(5));
}
