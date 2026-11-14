#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! storage-core-3 (2026-11-10 review): a failed or interrupted migration dry
//! run left its copy in the scratch directory, and every later rehearsal then
//! refused with `storage.conflict`, so migrations stayed blocked until the file
//! was deleted by hand. A failed rehearsal now removes its copy; a copy left by
//! an interrupted one is replaced; anything that is not a regular file at the
//! copy path is still refused and left alone.

use g9_domain::UtcMillis;
use g9_storage::migrations::Migrator;
use g9_storage::StorageError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const T0: i64 = 1_767_225_600_000;

fn migrator() -> Migrator {
    Migrator::new("0.1.0-test", UtcMillis::from_millis(T0))
}

/// A database migrated to version 12, so migration 13 is pending.
fn at_version_12(dir: &Path) -> PathBuf {
    let path = dir.join("g9.sqlite");
    let mut c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    migrator().migrate_up_to(&mut c, 12).unwrap();
    path
}

fn copy_path(scratch: &Path) -> PathBuf {
    scratch.join("g9.dry-run.sqlite")
}

#[test]
fn a_failed_rehearsal_removes_its_copy_and_the_next_one_is_not_blocked() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let db = at_version_12(dir.path());
    // Migration 13 adds this column; with it already present the rehearsal fails.
    Connection::open(&db)
        .unwrap()
        .execute_batch("ALTER TABLE assessment_versions ADD COLUMN difficulty INTEGER;")
        .unwrap();

    let first = migrator().dry_run_on_copy(&db, scratch.path());
    assert!(first.is_err(), "the rehearsal must fail: {first:?}");
    assert!(
        !copy_path(scratch.path()).exists(),
        "a failed rehearsal must remove its copy"
    );
    let second = migrator().dry_run_on_copy(&db, scratch.path());
    assert!(
        !matches!(second, Err(StorageError::Conflict(_))),
        "the next rehearsal must not be blocked by a leftover: {second:?}"
    );
}

#[test]
fn a_copy_left_by_an_interrupted_rehearsal_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let db = at_version_12(dir.path());
    std::fs::write(copy_path(scratch.path()), b"half-written copy").unwrap();
    std::fs::write(scratch.path().join("g9.dry-run.sqlite-wal"), b"stale wal").unwrap();

    let dry = migrator()
        .dry_run_on_copy(&db, scratch.path())
        .expect("a stale copy must not block the rehearsal");
    assert!(dry.integrity_ok);
    // The database is at version 12: every later migration is rehearsed.
    assert_eq!(
        dry.report.applied.len(),
        (g9_storage::migrations::CURRENT_SCHEMA_VERSION - 12) as usize,
        "migrations 13 up to the current one rehearsed"
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_at_the_copy_path_is_refused_and_its_target_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let db = at_version_12(dir.path());
    let target = dir.path().join("precious.txt");
    std::fs::write(&target, b"keep me").unwrap();
    std::os::unix::fs::symlink(&target, copy_path(scratch.path())).unwrap();

    let refused = migrator().dry_run_on_copy(&db, scratch.path());
    assert!(
        matches!(refused, Err(StorageError::Conflict(_))),
        "{refused:?}"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"keep me");
}
