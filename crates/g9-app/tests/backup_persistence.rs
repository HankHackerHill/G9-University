#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Backups across process lifetimes (gate A08, ADR-0001): `backup_manifests` is
//! the source of truth, so a backup created by one `AppContext` is verified,
//! restore-tested and reported by the next one over the same database file.
//! Every "restart" here drops the context (closing the database) and opens the
//! file again with a new context; the in-memory backup map starts empty.

mod common;

use common::*;
use g9_domain::{BackupId, BackupState, ErrorCategory, Revision, UtcMillis};
use g9_storage::{Database, Migrator, OpenOptions};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::path::Path;

fn options() -> OpenOptions {
    OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0))
}

/// A harness over `<root>/g9.sqlite` with `<root>` as the approved root (backups
/// and scratch below it). Dropping it closes the database. `generation` gives
/// each lifetime its own request and idempotency id range.
fn lifetime(root: &Path, generation: u64) -> Harness {
    try_lifetime(root, generation).unwrap()
}

#[allow(clippy::result_large_err)] // test helper; G9Error is the API's error type
fn try_lifetime(root: &Path, generation: u64) -> Result<Harness, g9_domain::G9Error> {
    std::fs::create_dir_all(root.join("backups")).unwrap();
    std::fs::create_dir_all(root.join("scratch")).unwrap();
    let db = Database::open(root.join("g9.sqlite"), options()).unwrap();
    let r = root.to_path_buf();
    let mut h = Harness::with_database(db, |s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    })?;
    h.continue_ids_from(generation * 1_000);
    Ok(h)
}

fn tempdir(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(prefix).tempdir().unwrap()
}

fn warnings(outcome: &Value) -> Vec<String> {
    outcome["warnings"]
        .as_array()
        .map(|w| {
            w.iter()
                .map(|x| x.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn create(h: &mut Harness, label: Option<&str>) -> String {
    let payload = match label {
        Some(label) => json!({ "label": label }),
        None => json!({}),
    };
    let created = h.call("backup.create", payload).unwrap();
    assert_eq!(created["result"]["state"], "SNAPSHOT_CREATED");
    assert_eq!(created["result"]["revision"], 1);
    created["result"]["backup_id"].as_str().unwrap().to_string()
}

#[test]
fn a_backup_created_in_one_lifetime_is_verified_and_retained_in_the_next() {
    let dir = tempdir("g9persist");
    let root = dir.path();

    let mut first = lifetime(root, 1);
    first
        .call("notes.create", json!({ "title": "t", "body_markdown": "before the backup", "classification": "PRIVATE", "domain": "LEARNING" }))
        .unwrap();
    let id = create(&mut first, Some("nightly"));
    let status = first.call("system.status", json!({})).unwrap();
    // Created but not yet verified or restore-tested: not a "last backup".
    assert_eq!(status["result"]["last_backup"], Value::Null);
    drop(first);

    // Second lifetime: the manifest is read from the database.
    let mut second = lifetime(root, 2);
    assert!(second.ctx().database().path().is_some());
    let verified = second
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
    assert_eq!(verified["result"]["integrity"], "OK");
    assert_eq!(verified["result"]["revision"], 2);
    // The label is not persisted; the size comes from the file's metadata.
    assert_eq!(verified["result"]["label"], Value::Null);
    let file = root.join("backups").join(format!("{id}.sqlite"));
    assert_eq!(
        verified["result"]["size_bytes"],
        json!(std::fs::metadata(&file).unwrap().len())
    );
    let tested = second
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED");
    assert_eq!(tested["result"]["revision"], 3);
    assert!(tested["result"]["restore_tested_at"].as_i64().is_some());
    let status = second.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"]["backup_id"], json!(id));
    assert_eq!(status["result"]["last_backup"]["state"], "RETAINED");
    // The disposable restore copy was removed.
    assert!(std::fs::read_dir(root.join("scratch"))
        .unwrap()
        .next()
        .is_none());
    drop(second);

    // Third lifetime: the retained backup is still the last backup.
    let mut third = lifetime(root, 3);
    let status = third.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"]["backup_id"], json!(id));
    assert_eq!(status["result"]["last_backup"]["state"], "RETAINED");
    assert!(warnings(&status).is_empty(), "{status}");
    // A retained backup cannot be verified again (illegal transition, no state change).
    let again = third
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap_err();
    assert_eq!(again.code, "backup.illegal_transition");
    let stored = g9_storage::backup::read_manifest(third.conn(), &BackupId::parse(&id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored.manifest.state, BackupState::Retained);
    assert_eq!(stored.revision, Revision::new(3));
}

#[test]
fn an_unknown_backup_id_is_not_found_after_a_reopen() {
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut first = lifetime(root, 1);
    let id = create(&mut first, None);
    drop(first);

    let mut second = lifetime(root, 2);
    let other = BackupId::new_v7();
    assert_ne!(other.as_str(), id);
    for op in ["backup.verify", "backup.restore_test"] {
        let err = second
            .call(op, json!({ "backup_id": other.as_str() }))
            .unwrap_err();
        assert_eq!(err.category, ErrorCategory::NotFound, "{op}");
        assert_eq!(err.code, "backup.not_found", "{op}");
    }
    // The known one is still there and untouched by the refusals.
    let verified = second
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(verified["result"]["revision"], 2);
}

#[test]
fn revision_conflicts_are_checked_against_the_stored_revision() {
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut first = lifetime(root, 1);
    let id = create(&mut first, None);
    first
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap();
    drop(first);

    let mut second = lifetime(root, 2);
    let stale = second
        .call_with(
            "backup.restore_test",
            json!({ "backup_id": id }),
            &Opts::revision(1),
        )
        .unwrap_err();
    assert_eq!(stale.category, ErrorCategory::StaleRevision, "{stale:?}");
    assert_eq!(stale.code, "revision.stale");
    assert_eq!(
        stale.details.as_ref().unwrap(),
        &json!({ "expected": 1, "actual": 2 })
    );
    // Nothing moved.
    let bid = BackupId::parse(&id).unwrap();
    assert_eq!(
        g9_storage::backup::manifest_state(second.conn(), &bid).unwrap(),
        Some(BackupState::Checked)
    );
    // The stored revision is accepted and the update lines up with it.
    let tested = second
        .call_with(
            "backup.restore_test",
            json!({ "backup_id": id }),
            &Opts::revision(2),
        )
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED");
    assert_eq!(tested["result"]["revision"], 3);
    let stored = g9_storage::backup::read_manifest(second.conn(), &bid)
        .unwrap()
        .unwrap();
    assert_eq!(stored.revision, Revision::new(3));
}

#[test]
fn another_context_advancing_a_backup_is_seen_not_overwritten() {
    // One process at a time holds the data (app-4): while a context lives, a
    // second one over the same file is refused, so the CLI and the desktop shell
    // can no longer advance a backup behind each other's back. Across process
    // lifetimes the stored manifest decides.
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut first = lifetime(root, 1);
    let id = create(&mut first, Some("kept label"));
    let refused = try_lifetime(root, 2)
        .err()
        .expect("a second context over the same file must be refused");
    assert_eq!(refused.code, "app.data_in_use", "{refused:?}");
    drop(first);
    let mut b = lifetime(root, 2);
    b.call("backup.verify", json!({ "backup_id": id })).unwrap();
    b.call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    drop(b);
    let mut a = lifetime(root, 3);

    let stale = a
        .call_with(
            "backup.verify",
            json!({ "backup_id": id }),
            &Opts::revision(1),
        )
        .unwrap_err();
    assert_eq!(stale.category, ErrorCategory::StaleRevision, "{stale:?}");
    // Without an expected revision the verify is an illegal transition from the
    // stored RETAINED state; the RETAINED manifest is never downgraded.
    let illegal = a
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap_err();
    assert_eq!(illegal.code, "backup.illegal_transition");
    let stored = g9_storage::backup::read_manifest(a.conn(), &BackupId::parse(&id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored.manifest.state, BackupState::Retained);
    assert_eq!(stored.revision, Revision::new(3));
}

/// A file database at schema version 9 (every later migration pending).
fn database_at_version_9(path: &Path) -> Database {
    let options = OpenOptions::new("0.0.9").applied_at(UtcMillis::from_millis(T0));
    let mut db = Database::open(path, options.clone().no_auto_migrate()).unwrap();
    Migrator::new("0.0.9", UtcMillis::from_millis(T0))
        .migrate_up_to(db.conn_mut(), 9)
        .unwrap();
    drop(db);
    let db = Database::open(path, options).unwrap();
    assert_eq!(db.pending_migrations().first(), Some(&10));
    db
}

#[test]
fn the_pre_migration_backup_of_a_version_9_database_is_restore_tested_after_a_reopen() {
    let dir = tempdir("g9persist9");
    let root = dir.path();
    std::fs::create_dir_all(root.join("backups")).unwrap();
    std::fs::create_dir_all(root.join("scratch")).unwrap();
    let r = root.to_path_buf();
    let configure = move |s: &mut Setup| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    };

    // Lifetime 1: recovery takes the mandatory pre-migration backup (CHECKED) and migrates.
    let mut first =
        Harness::with_database(database_at_version_9(&root.join("g9.sqlite")), &configure).unwrap();
    let report = first.ctx().recovery_report().clone();
    let id = report.pre_migration_backup.clone().unwrap();
    assert!(!report.migrated.is_empty(), "{report:?}");
    let status = first.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"], Value::Null);
    drop(first);

    // Lifetime 2: a current-schema open; the old backup is restore-tested through IPC.
    let db = Database::open(root.join("g9.sqlite"), options()).unwrap();
    assert!(db.pending_migrations().is_empty());
    let mut second = Harness::with_database(db, &configure).unwrap();
    second.continue_ids_from(1_000);
    let tested = second
        .call("backup.restore_test", json!({ "backup_id": id.as_str() }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED");
    assert_eq!(tested["result"]["revision"], 2);
    let status = second.call("system.status", json!({})).unwrap();
    assert_eq!(
        status["result"]["last_backup"]["backup_id"],
        json!(id.as_str())
    );
    assert_eq!(status["result"]["last_backup"]["state"], "RETAINED");
}

#[test]
fn a_manifest_path_outside_the_approved_roots_is_refused() {
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut h = lifetime(root, 1);
    let id = create(&mut h, None);
    let file = root.join("backups").join(format!("{id}.sqlite"));
    h.conn()
        .execute(
            "UPDATE backup_manifests SET path = '/etc/hostname' WHERE id = ?1",
            [&id],
        )
        .unwrap();
    for op in ["backup.verify", "backup.restore_test"] {
        let err = h.call(op, json!({ "backup_id": id })).unwrap_err();
        assert_eq!(err.category, ErrorCategory::Unauthorized, "{op}");
        assert_eq!(err.code, "backup.manifest_path_outside_approved_roots");
    }
    // Put the recorded path back and the same backup verifies.
    h.conn()
        .execute(
            "UPDATE backup_manifests SET path = ?2 WHERE id = ?1",
            [&id, file.to_str().unwrap()],
        )
        .unwrap();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
}

#[test]
fn a_manifest_with_a_relative_path_is_refused_and_the_backup_is_not_marked_failed() {
    // `ApprovedRoots::contains` is lexical and accepts a relative path (it is
    // placed under the primary root), but the verification would open that path
    // relative to the process directory: an arbitrary file, and a FAILED manifest.
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut h = lifetime(root, 1);
    let id = create(&mut h, None);
    let file = root.join("backups").join(format!("{id}.sqlite"));
    let stored = |h: &Harness| -> (String, i64, Option<String>) {
        h.conn()
            .query_row(
                "SELECT state, revision, failure_reason FROM backup_manifests WHERE id = ?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
    };
    for relative in ["victim.sqlite", "backups/x.sqlite", "./backups/x.sqlite"] {
        h.conn()
            .execute(
                "UPDATE backup_manifests SET path = ?2 WHERE id = ?1",
                [&id, relative],
            )
            .unwrap();
        for op in ["backup.verify", "backup.restore_test"] {
            let err = h.call(op, json!({ "backup_id": id })).unwrap_err();
            assert_eq!(err.category, ErrorCategory::Unauthorized, "{relative} {op}");
            assert_eq!(err.code, "backup.manifest_path_outside_approved_roots");
        }
        // Nothing moved: not FAILED, same revision, no failure reason.
        assert_eq!(
            stored(&h),
            ("SNAPSHOT_CREATED".to_string(), 1, None),
            "{relative}"
        );
    }
    h.conn()
        .execute(
            "UPDATE backup_manifests SET path = ?2 WHERE id = ?1",
            [&id, file.to_str().unwrap()],
        )
        .unwrap();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
}

#[test]
fn a_manifest_whose_snapshot_file_was_deleted_fails_cleanly_and_stays_failed() {
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut first = lifetime(root, 1);
    let unverified = create(&mut first, None);
    let checked = create(&mut first, None);
    first
        .call("backup.verify", json!({ "backup_id": checked }))
        .unwrap();
    drop(first);
    for id in [&unverified, &checked] {
        std::fs::remove_file(root.join("backups").join(format!("{id}.sqlite"))).unwrap();
    }

    let mut second = lifetime(root, 2);
    // verify of a deleted snapshot: the manifest becomes FAILED, no panic.
    let verified = second
        .call("backup.verify", json!({ "backup_id": unverified }))
        .unwrap();
    assert_eq!(verified["result"]["state"], "FAILED");
    assert_eq!(verified["result"]["integrity"], "FAILED");
    assert_eq!(verified["result"]["size_bytes"], Value::Null);
    // restore_test of a CHECKED backup whose file vanished: FAILED as well.
    let tested = second
        .call("backup.restore_test", json!({ "backup_id": checked }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "FAILED");
    // FAILED is terminal: neither operation moves it again, and it is not "last".
    for id in [&unverified, &checked] {
        for op in ["backup.verify", "backup.restore_test"] {
            let err = second.call(op, json!({ "backup_id": id })).unwrap_err();
            assert_eq!(err.category, ErrorCategory::Unauthorized, "{op}");
        }
        let stored =
            g9_storage::backup::read_manifest(second.conn(), &BackupId::parse(id).unwrap())
                .unwrap()
                .unwrap();
        assert_eq!(stored.manifest.state, BackupState::Failed);
    }
    let status = second.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"], Value::Null);
    assert!(warnings(&status).is_empty(), "{status}");
}

#[test]
fn a_manifest_row_that_cannot_be_decoded_fails_closed_and_is_reported_by_status() {
    let dir = tempdir("g9persist");
    let root = dir.path();
    let mut h = lifetime(root, 1);
    let id = create(&mut h, None);
    h.conn()
        .execute_batch(&format!(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE backup_manifests SET state = 'SOMETHING_ELSE' WHERE id = '{id}';
             PRAGMA ignore_check_constraints = OFF;"
        ))
        .unwrap();
    let err = h
        .call("backup.verify", json!({ "backup_id": id }))
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::DataCorruption, "{err:?}");
    assert_eq!(err.code, "storage.row_decode");
    // The status stays available, reports no backup and says why.
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"], Value::Null);
    assert!(
        warnings(&status)
            .iter()
            .any(|w| w.starts_with("system.status.last_backup_unreadable")
                && w.contains("storage.row_decode")),
        "{status}"
    );
}
