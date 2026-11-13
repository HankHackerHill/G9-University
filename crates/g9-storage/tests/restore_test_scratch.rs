#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! storage-core-2 (2026-11-10 review): `restore_test` drove an intact, verified
//! backup to the terminal FAILED state when its scratch copy already existed
//! (left by an interrupted test) or when the copy failed for an environmental
//! reason (missing scratch directory, disk full, I/O). Only a failure that says
//! something about the backup's content may fail it; a leftover copy is our
//! own scratch file and is replaced.

mod common;

use common::*;
use g9_domain::{BackupId, BackupState};
use g9_storage::backup::{
    create_snapshot, restore_test, verify_and_advance, BackupManifest, SnapshotRequest,
    VerifyOptions,
};
use std::path::Path;

fn checked_backup(dir: &Path) -> BackupManifest {
    let db = open_file(&dir.join("live.sqlite"));
    populate(db.conn());
    let path = dir.join("b1.sqlite");
    let mut manifest = create_snapshot(
        db.conn(),
        &path,
        &SnapshotRequest {
            backup_id: BackupId::parse("b1").unwrap(),
            requested_at: t(100),
            app_version: "0.1.0-test".into(),
        },
    )
    .unwrap();
    let report = verify_and_advance(&mut manifest, &VerifyOptions::default()).unwrap();
    assert!(report.ok(), "{:?}", report.errors);
    assert_eq!(manifest.state, BackupState::Checked);
    manifest
}

#[test]
fn a_leftover_restore_test_copy_does_not_fail_a_good_backup() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let mut manifest = checked_backup(dir.path());
    std::fs::write(
        scratch.path().join("b1.restore-test.sqlite"),
        b"left by an interrupted restore test",
    )
    .unwrap();

    restore_test(
        &mut manifest,
        scratch.path(),
        &VerifyOptions::default(),
        t(200),
    )
    .expect("a leftover scratch copy must not fail the restore test");
    assert_eq!(manifest.state, BackupState::RestoreTested);
}

#[test]
fn an_environmental_failure_leaves_the_backup_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut manifest = checked_backup(dir.path());
    let missing_scratch = dir.path().join("unmounted-scratch");

    let e = restore_test(
        &mut manifest,
        &missing_scratch,
        &VerifyOptions::default(),
        t(200),
    )
    .unwrap_err();
    assert_ne!(e.code(), "storage.corrupt", "{e:?}");
    assert_eq!(
        manifest.state,
        BackupState::Checked,
        "a scratch directory that cannot be written says nothing about the backup"
    );
}
