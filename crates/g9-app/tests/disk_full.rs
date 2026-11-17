#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! STOR-4 (docs/release/path-to-10.md §5.2; runbook "Disk-full and power-fail"):
//!
//! 1. A backup refuses to start when the backup directory's filesystem has less
//!    free space than `BACKUP_FREE_SPACE_FACTOR` × the database size. The
//!    free-space source is injected (`AppConfig::free_space`), so the refusal is
//!    proven without filling a disk.
//! 2. `SQLITE_FULL` during a real command, on a scratch database capped with
//!    `PRAGMA max_page_count`: the answer is `RETRYABLE_STORAGE
//!    storage.disk_full`, nothing is half written, the audit chain verifies, and
//!    the same request succeeds once there is room again.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use g9_storage::backup::{database_size_bytes, FreeSpaceSource, BACKUP_FREE_SPACE_FACTOR};
use rusqlite::Connection;
use serde_json::json;
use std::path::{Path, PathBuf};

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Harness over the data-directory layout with a fixed free-space reading.
fn harness_with_free_space(free: FreeSpaceSource) -> (Harness, PathBuf) {
    let dir = tempfile::Builder::new().prefix("g9full").tempdir().unwrap();
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
        s.config.free_space = free;
    });
    h.dir = Some(dir);
    (h, root)
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_default_threshold_is_the_runbook_proposal_of_twice_the_database_size() {
    assert_eq!(BACKUP_FREE_SPACE_FACTOR, 2);
    assert_eq!(
        g9_app::AppConfig::new("t").unwrap().free_space,
        FreeSpaceSource::Filesystem
    );
}

#[test]
fn backup_create_refuses_to_start_below_the_free_space_threshold() {
    // One byte short of the threshold of this very database.
    let probe = harness_with_free_space(FreeSpaceSource::Filesystem).0;
    let required = BACKUP_FREE_SPACE_FACTOR * database_size_bytes(probe.conn()).unwrap();
    drop(probe);
    let (mut h, root) = harness_with_free_space(FreeSpaceSource::Fixed(required - 1));
    assert_eq!(
        BACKUP_FREE_SPACE_FACTOR * database_size_bytes(h.conn()).unwrap(),
        required
    );
    let audit_before = h.audit_rows();

    let err = h.call("backup.create", json!({})).unwrap_err();
    assert_eq!(err.category, ErrorCategory::RetryableStorage, "{err:?}");
    assert_eq!(err.code, "backup.insufficient_space");
    let details = err.details.clone().unwrap();
    assert_eq!(details["required_bytes"], json!(required));
    assert_eq!(details["available_bytes"], json!(required - 1));
    assert!(!err.message.contains(&*root.to_string_lossy()), "{err:?}");

    // Nothing started: no snapshot, no partial file, no manifest row, no
    // success audit row; the refusal itself is audited and the chain verifies.
    assert!(entries(&root.join("backups")).is_empty());
    assert_eq!(count(h.conn(), "SELECT COUNT(*) FROM backup_manifests"), 0);
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM audit_metadata WHERE operation LIKE 'backup.create%' AND outcome = 'SUCCESS'"
        ),
        0
    );
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM audit_metadata WHERE operation = 'backup.create' AND outcome = 'RETRYABLE_STORAGE'"
        ),
        1
    );
    assert_eq!(h.audit_rows(), audit_before + 1);
    assert!(
        g9_storage::AuditChain::verify_chain(h.conn()).unwrap().ok,
        "the chain must verify after the refusal"
    );
}

#[test]
fn backup_create_starts_at_exactly_the_threshold() {
    let probe = harness_with_free_space(FreeSpaceSource::Filesystem).0;
    let required = BACKUP_FREE_SPACE_FACTOR * database_size_bytes(probe.conn()).unwrap();
    drop(probe);
    let (mut h, root) = harness_with_free_space(FreeSpaceSource::Fixed(required));
    let created = h.call("backup.create", json!({})).unwrap();
    assert_eq!(created["result"]["state"], "SNAPSHOT_CREATED", "{created}");
    assert_eq!(entries(&root.join("backups")).len(), 1);
}

#[test]
fn the_real_filesystem_reading_lets_a_backup_start_here() {
    // The default source reads statvfs on this host's temporary filesystem.
    let (mut h, root) = harness_with_free_space(FreeSpaceSource::Filesystem);
    let available = FreeSpaceSource::Filesystem
        .available_bytes(&root.join("backups"))
        .unwrap();
    if cfg!(unix) {
        assert!(available.is_some_and(|b| b > 0), "{available:?}");
    }
    let created = h.call("backup.create", json!({})).unwrap();
    assert_eq!(created["result"]["state"], "SNAPSHOT_CREATED", "{created}");
}

/// A file database at schema version 9 (every later migration pending).
fn database_at_version_9(path: &Path) -> g9_storage::Database {
    let options =
        g9_storage::OpenOptions::new("0.0.9").applied_at(g9_domain::UtcMillis::from_millis(T0));
    let mut db = g9_storage::Database::open(path, options.clone().no_auto_migrate()).unwrap();
    g9_storage::Migrator::new("0.0.9", g9_domain::UtcMillis::from_millis(T0))
        .migrate_up_to(db.conn_mut(), 9)
        .unwrap();
    drop(db);
    g9_storage::Database::open(path, options).unwrap()
}

#[test]
fn a_pre_migration_backup_below_the_threshold_is_refused_and_nothing_migrates() {
    let dir = tempfile::Builder::new()
        .prefix("g9fullmig")
        .tempdir()
        .unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["backups", "scratch"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.clone();
    let mut h = Harness::with_database(database_at_version_9(&root.join("g9.sqlite")), |s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
        s.config.free_space = FreeSpaceSource::Fixed(0);
    })
    .unwrap();
    let report = h.ctx().recovery_report().clone();
    assert!(report.migrated.is_empty(), "{report:?}");
    assert_eq!(report.pending_migrations.first(), Some(&10));
    assert!(
        report
            .migration_blocked_reason
            .as_deref()
            .unwrap_or_default()
            .contains("backup.insufficient_space"),
        "{report:?}"
    );
    assert!(report.pre_migration_backup.is_none());
    assert!(entries(&root.join("backups")).is_empty());
    assert!(entries(&root.join("scratch")).is_empty());
}

// ---------------------------------------------------------------------------
// SQLITE_FULL through g9_app::dispatch
// ---------------------------------------------------------------------------

fn note(title: &str, body: &str) -> serde_json::Value {
    json!({ "title": title, "body_markdown": body, "classification": "PRIVATE", "domain": "LEARNING" })
}

#[test]
fn sqlite_full_answers_retryable_storage_and_leaves_nothing_half_written() {
    let dir = tempfile::Builder::new()
        .prefix("g9sqlfull")
        .tempdir()
        .unwrap();
    let path = dir.path().join("scratch.sqlite");
    let db = g9_storage::Database::open(
        &path,
        g9_app::database_options("0.1.0-test").applied_at(g9_domain::UtcMillis::from_millis(T0)),
    )
    .unwrap();
    let mut h = Harness::with_database(db, |_| {}).unwrap();
    h.call("notes.create", note("before", "a small note"))
        .unwrap();
    let notes_before = count(h.conn(), "SELECT COUNT(*) FROM notes");
    let audit_before = h.audit_rows();

    // The body is written twice (notes and note_versions). The cap leaves room
    // for one copy and for the small rows of a recorded refusal, not for both
    // copies: the transaction fails after its first write. (SQLite itself rolls
    // the whole transaction back on SQLITE_FULL here; the pipeline never
    // commits a failed handler either. Both are covered by the assertions.)
    let page_size = count(h.conn(), "PRAGMA page_size");
    let pages = count(h.conn(), "PRAGMA page_count");
    let body = "x".repeat(400_000);
    let body_pages = i64::try_from(body.len()).unwrap() / page_size + 1;
    let cap = pages + body_pages * 3 / 2;
    let set: i64 = h
        .conn()
        .query_row(&format!("PRAGMA max_page_count = {cap}"), [], |r| r.get(0))
        .unwrap();
    assert_eq!(set, cap);

    let large = note("large", &body);
    let err = h
        .call_with("notes.create", large.clone(), &Opts::key("large-note"))
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::RetryableStorage, "{err:?}");
    assert_eq!(err.code, "storage.disk_full");

    // Nothing half written.
    assert_eq!(count(h.conn(), "SELECT COUNT(*) FROM notes"), notes_before);
    assert_eq!(
        count(h.conn(), "SELECT COUNT(*) FROM note_versions"),
        notes_before
    );
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM notes WHERE title = 'large' OR length(body_markdown) > 1000"
        ),
        0
    );
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM idempotency_records WHERE idempotency_key = 'large-note'"
        ),
        0
    );
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM domain_events WHERE kind = 'note.created'"
        ),
        notes_before
    );
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM audit_metadata WHERE operation = 'notes.create' AND outcome = 'SUCCESS'"
        ),
        notes_before
    );
    // The refusal is recorded (decision and audit row fit under the cap).
    assert_eq!(
        count(
            h.conn(),
            "SELECT COUNT(*) FROM audit_metadata WHERE operation = 'notes.create' AND outcome = 'RETRYABLE_STORAGE'"
        ),
        1
    );
    assert_eq!(h.audit_rows(), audit_before + 1);
    let chain = g9_storage::AuditChain::verify_chain(h.conn()).unwrap();
    assert!(chain.ok, "{chain:?}");
    assert_eq!(
        g9_storage::migrations::integrity_check(h.conn()).unwrap(),
        vec!["ok".to_string()]
    );

    // Space is back: the same request (same idempotency key) and a small one succeed.
    let _: i64 = h
        .conn()
        .query_row("PRAGMA max_page_count = 1073741823", [], |r| r.get(0))
        .unwrap();
    let retried = h
        .call_with("notes.create", large, &Opts::key("large-note"))
        .unwrap();
    assert_eq!(retried["replayed"], false, "{retried}");
    h.call("notes.create", note("after", "space is back"))
        .unwrap();
    assert_eq!(
        count(h.conn(), "SELECT COUNT(*) FROM notes"),
        notes_before + 2
    );
    let chain = g9_storage::AuditChain::verify_chain(h.conn()).unwrap();
    assert!(chain.ok, "{chain:?}");
}
