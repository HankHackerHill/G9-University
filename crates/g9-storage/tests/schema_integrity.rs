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

fn stored_table_sql(path: &Path, table: &str) -> String {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn a_line_comment_stretched_over_a_check_constraint_fails_verification() {
    // Whitespace is not cosmetic in SQL: a newline ends a `--` comment. Replacing
    // the one newline after the comment above a CHECK lets the comment swallow the
    // constraint, while the text with whitespace collapsed stays identical. The
    // tampered file still opens and passes integrity_check, and then accepts rows
    // the genuine schema refuses: integrity_check validates CHECK constraints, so
    // without this the snapshot would carry them past every row-level check.
    let dir = tempfile::tempdir().unwrap();
    let path = genuine(dir.path(), "evil.sqlite");
    let sql = stored_table_sql(&path, "decisions");
    let comment = "-- a decision is DECIDED only by an operator (model output can only propose)\n";
    assert!(sql.contains(comment), "fixture drifted: {sql}");
    let stretched = sql.replacen(comment, &comment.replace('\n', " "), 1);
    assert_eq!(
        stretched.split_whitespace().collect::<Vec<_>>(),
        sql.split_whitespace().collect::<Vec<_>>(),
        "the tamper changes whitespace only"
    );
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch("PRAGMA writable_schema = ON;").unwrap();
        c.execute(
            "UPDATE sqlite_master SET sql = ?1 WHERE type = 'table' AND name = 'decisions'",
            [&stretched],
        )
        .unwrap();
    }

    // The constraint is gone: a DECIDED decision with no operator is accepted.
    const UNDECIDED: &str =
        "INSERT INTO decisions (id, title, context, state, trust, created_at, updated_at)
         VALUES ('d-1', 't', 'c', 'DECIDED', 'OPERATOR', 0, 0)";
    let honest = genuine(dir.path(), "honest.sqlite");
    assert!(
        Connection::open(&honest)
            .unwrap()
            .execute(UNDECIDED, [])
            .is_err(),
        "the genuine schema refuses a DECIDED decision without an operator"
    );
    Connection::open(&path)
        .unwrap()
        .execute(UNDECIDED, [])
        .unwrap();

    let (ok, errors) = errors_of(&path);
    assert!(!ok, "a snapshot with a stretched comment must not verify");
    assert!(
        errors.join("\n").contains("changed table \"decisions\""),
        "{errors:?}"
    );
}

#[test]
fn only_planner_statistics_are_exempt_from_the_comparison() {
    // What SQLite derives deterministically from the migrations (the indexes behind
    // PRIMARY KEY / UNIQUE constraints, the AUTOINCREMENT counter table) is part of
    // the expected schema. Only the statistics ANALYZE writes are exempt: they
    // steer query plans and never run code.
    let dir = tempfile::tempdir().unwrap();
    let analyzed = genuine(dir.path(), "analyzed.sqlite");
    tamper(&analyzed, "ANALYZE;");
    let (ok, errors) = errors_of(&analyzed);
    assert!(
        ok,
        "planner statistics alone must not fail verification: {errors:?}"
    );

    let path = genuine(dir.path(), "evil.sqlite");
    tamper(
        &path,
        "PRAGMA writable_schema = ON;
         UPDATE sqlite_master SET sql = 'CREATE TABLE sqlite_sequence(name,seq,planted)'
           WHERE type = 'table' AND name = 'sqlite_sequence';",
    );
    let (ok, errors) = errors_of(&path);
    assert!(!ok, "a changed sqlite_sequence must not verify");
    assert!(
        errors
            .join("\n")
            .contains("changed table \"sqlite_sequence\""),
        "{errors:?}"
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
