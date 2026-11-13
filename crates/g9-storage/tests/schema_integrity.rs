#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Schema integrity (security audit 2026-11-12, MEDIUM "planted schema objects").
//!
//! A database file is data from outside the process: a backup handed to restore,
//! or the live file after anything else wrote to it. Its triggers run inside the
//! application's own transactions, so a dropped immutability trigger or a planted
//! one subverts audit, erasure and grades while every row-level check still
//! passes. The schema must equal what the embedded migrations produce at the
//! version the file records; anything else is refused.

use g9_domain::UtcMillis;
use g9_storage::backup::{verify_snapshot, VerifyOptions};
use g9_storage::migrations::Migrator;
use g9_storage::{restore_over_live, Database, OpenOptions, RestoreOptions};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const T0: i64 = 1_767_225_600_000;

/// A genuine database created by this build, closed, with no WAL sidecar left.
fn genuine(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let db = Database::open(
        &path,
        OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0)),
    )
    .unwrap();
    drop(db);
    let c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA journal_mode = DELETE;").unwrap();
    path
}

/// Edit the file the way an attacker with a SQLite shell would.
fn tamper(path: &Path, sql: &str) {
    let c = Connection::open(path).unwrap();
    c.execute_batch(sql).unwrap();
}

fn errors_of(path: &Path) -> (bool, Vec<String>) {
    let report = verify_snapshot(path, &VerifyOptions::default())
        .expect("verification must report, not fail");
    (report.ok(), report.errors)
}

const PLANT_AUDIT_EATER: &str = "DROP TRIGGER trg_audit_metadata_no_delete;
     CREATE TRIGGER zz_eat_audit AFTER INSERT ON audit_metadata BEGIN DELETE FROM audit_metadata WHERE seq = NEW.seq; END;";

#[test]
fn a_genuine_database_verifies_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "g9.sqlite");
    let (ok, errors) = errors_of(&path);
    assert!(ok, "{errors:?}");
}

#[test]
fn a_database_migrated_one_version_at_a_time_verifies_clean() {
    // No false positive: the stored SQL of a table altered by later migrations must
    // equal the SQL of the same table in a database created in one go.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stepwise.sqlite");
    {
        let mut c = Connection::open(&path).unwrap();
        c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let m = Migrator::new("0.1.0-test", UtcMillis::from_millis(T0));
        for target in 1..=g9_storage::migrations::CURRENT_SCHEMA_VERSION {
            m.migrate_up_to(&mut c, target).unwrap();
        }
    }
    let (ok, errors) = errors_of(&path);
    assert!(ok, "{errors:?}");
}

#[test]
fn a_dropped_immutability_trigger_and_a_planted_trigger_fail_verification() {
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "evil.sqlite");
    tamper(&path, PLANT_AUDIT_EATER);
    let (ok, errors) = errors_of(&path);
    assert!(!ok, "a snapshot with a planted trigger must not verify");
    let text = errors.join("\n");
    assert!(text.contains("zz_eat_audit"), "{text}");
    assert!(text.contains("trg_audit_metadata_no_delete"), "{text}");
}

#[test]
fn an_extra_table_fails_verification() {
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "evil.sqlite");
    tamper(&path, "CREATE TABLE zz_shadow (id TEXT, body TEXT);");
    let (ok, errors) = errors_of(&path);
    assert!(!ok);
    assert!(errors.join("\n").contains("zz_shadow"), "{errors:?}");
}

#[test]
fn a_trigger_hidden_behind_an_internal_name_fails_verification() {
    // `sqlite_` names are reserved, but a row written through writable_schema is
    // loaded as a working trigger: the check must not skip it by name.
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "evil.sqlite");
    tamper(
        &path,
        "PRAGMA writable_schema = ON;
         INSERT INTO sqlite_master (type, name, tbl_name, rootpage, sql)
           VALUES ('trigger', 'sqlite_hidden', 'notes', 0,
                   'CREATE TRIGGER sqlite_hidden AFTER INSERT ON notes BEGIN SELECT 1; END');
         PRAGMA writable_schema = OFF;",
    );
    let (ok, errors) = errors_of(&path);
    assert!(!ok);
    assert!(errors.join("\n").contains("sqlite_hidden"), "{errors:?}");
}

#[test]
fn a_table_name_with_a_double_quote_is_reported_not_an_internal_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "evil.sqlite");
    tamper(&path, r#"CREATE TABLE "we""ird" (x INTEGER);"#);
    let (ok, errors) = errors_of(&path);
    assert!(!ok);
    assert!(
        errors.join("\n").contains("we\\\"ird"),
        "names are reported escaped: {errors:?}"
    );
}

#[test]
fn restore_refuses_a_snapshot_with_a_planted_trigger_and_leaves_the_live_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let live = genuine(dir.path(), "g9.sqlite");
    let incoming = tempfile::tempdir().unwrap();
    let snapshot = genuine(incoming.path(), "snapshot.sqlite");
    tamper(&snapshot, PLANT_AUDIT_EATER);
    let before = std::fs::read(&live).unwrap();

    let result = restore_over_live(
        &snapshot,
        &live,
        &dir.path().join("quarantine"),
        &RestoreOptions {
            expect_sha256: None,
            now: UtcMillis::from_millis(T0),
        },
    );
    assert!(
        result.is_err(),
        "restore must refuse the snapshot: {result:?}"
    );
    assert_eq!(
        std::fs::read(&live).unwrap(),
        before,
        "the live database must be untouched"
    );
}
