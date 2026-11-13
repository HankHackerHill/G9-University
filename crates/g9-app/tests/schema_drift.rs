#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Open-time schema check (security audit 2026-11-12, MEDIUM "planted schema
//! objects"): when the live database's schema differs from what the embedded
//! migrations produce, recovery writes nothing and every mutation is refused,
//! exactly as for a broken audit chain.

mod common;

use common::*;
use g9_domain::{ErrorCategory, UtcMillis};
use g9_storage::{Database, OpenOptions};
use serde_json::json;

fn tampered_database(dir: &std::path::Path) -> Database {
    let path = dir.join("g9.sqlite");
    drop(
        Database::open(
            &path,
            OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0)),
        )
        .unwrap(),
    );
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch(
        "DROP TRIGGER trg_audit_metadata_no_delete;
         CREATE TRIGGER zz_eat_audit AFTER INSERT ON audit_metadata BEGIN DELETE FROM audit_metadata WHERE seq = NEW.seq; END;",
    )
    .unwrap();
    drop(c);
    Database::open(&path, OpenOptions::new("0.1.0-test")).unwrap()
}

#[test]
fn a_planted_trigger_in_the_live_database_blocks_every_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::with_database(tampered_database(dir.path()), |_| {}).unwrap();

    let report = h.ctx().recovery_report().clone();
    assert_eq!(
        report.write_block_reason(),
        Some("schema_drift"),
        "{report:?}"
    );
    assert!(
        format!("{report:?}").contains("zz_eat_audit"),
        "the report names the planted trigger: {report:?}"
    );

    let refused = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap_err();
    assert_eq!(
        refused.category,
        ErrorCategory::DataCorruption,
        "{refused:?}"
    );
    assert_eq!(refused.code, "recovery.integrity_failed");
}

#[test]
fn a_genuine_database_opens_writable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g9.sqlite");
    let db = Database::open(
        &path,
        OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0)),
    )
    .unwrap();
    let mut h = Harness::with_database(db, |_| {}).unwrap();
    let report = h.ctx().recovery_report().clone();
    assert_eq!(report.write_block_reason(), None, "{report:?}");
}
