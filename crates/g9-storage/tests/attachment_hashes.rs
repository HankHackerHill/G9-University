#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! STOR-2 (docs/release/path-to-10.md §5.2): backup verification hashes every
//! attachment file a snapshot references, in the store the caller names (the
//! application names `<backups>/attachments`, the store a restore copies from).
//! A damaged file fails verification exactly as a missing one does; both are
//! reported by code and count, never by path or content.

mod common;

use common::*;
use g9_domain::BackupId;
use g9_storage::backup::{create_snapshot, verify_snapshot, SnapshotRequest, VerifyOptions};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The fixture's one attachment holds these bytes (`common::populate`).
const BYTES: &[u8] = b"pcap-bytes";

struct Setup {
    _dir: tempfile::TempDir,
    snapshot: PathBuf,
    store: PathBuf,
    file: PathBuf,
    sha: String,
}

/// A snapshot of the populated database and a backup attachment store that
/// holds the fixture's attachment intact at its content address.
fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file(&dir.path().join("live.sqlite"));
    let fx = populate(db.conn());
    assert_eq!(fx.attachment_sha, sha("pcap-bytes"));
    let backups = dir.path().join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    let snapshot = backups.join("b1.sqlite");
    create_snapshot(
        db.conn(),
        &snapshot,
        &SnapshotRequest {
            backup_id: BackupId::parse("b1").unwrap(),
            requested_at: t(100),
            app_version: "0.1.0-test".into(),
        },
    )
    .unwrap();
    let store = backups.join("attachments");
    let file = store.join(&fx.attachment_storage_ref);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, BYTES).unwrap();
    Setup {
        _dir: dir,
        snapshot,
        store,
        file,
        sha: fx.attachment_sha,
    }
}

fn verify(s: &Setup) -> (bool, Value, Vec<String>) {
    let report = verify_snapshot(
        &s.snapshot,
        &VerifyOptions {
            attachment_root: Some(s.store.clone()),
        },
    )
    .unwrap();
    let ok = report.ok();
    let manifest = serde_json::to_value(&report.attachment_manifest).unwrap();
    (ok, manifest, report.errors)
}

/// No error line names a path, a digest or the bytes.
fn assert_metadata_only(errors: &[String], s: &Setup) {
    for line in errors {
        assert!(!line.contains(&s.sha), "a digest in an error: {line}");
        assert!(!line.contains("pcap"), "content in an error: {line}");
        assert!(!line.contains(&*s.store.to_string_lossy()), "{line}");
        if line.starts_with("backup.attachments_") {
            assert!(!line.contains('/'), "a path in an error: {line}");
        }
    }
}

fn flip_one_byte(path: &Path) {
    let mut bytes = std::fs::read(path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn an_intact_store_verifies() {
    let s = setup();
    let (ok, manifest, errors) = verify(&s);
    assert!(ok, "{errors:?}");
    assert_eq!(manifest["status"], "CHECKED");
    assert_eq!(manifest["expected"], 1);
    assert_eq!(manifest["missing"], serde_json::json!([]));
    assert_eq!(manifest["damaged"], 0);
}

#[test]
fn one_flipped_byte_in_the_backup_attachment_store_fails_verification() {
    let s = setup();
    flip_one_byte(&s.file);
    let (ok, manifest, errors) = verify(&s);
    assert!(!ok, "a damaged attachment copy passed verification");
    assert_eq!(manifest["damaged"], 1, "{manifest}");
    assert_eq!(manifest["missing"], serde_json::json!([]), "{manifest}");
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("backup.attachments_damaged: 1 ")),
        "{errors:?}"
    );
    assert_metadata_only(&errors, &s);
}

#[test]
fn a_missing_file_is_reported_by_code_and_count() {
    let s = setup();
    std::fs::remove_file(&s.file).unwrap();
    let (ok, manifest, errors) = verify(&s);
    assert!(!ok);
    assert_eq!(manifest["damaged"], 0, "{manifest}");
    assert_eq!(manifest["missing"].as_array().unwrap().len(), 1);
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("backup.attachments_missing: 1 ")),
        "{errors:?}"
    );
    assert_metadata_only(&errors, &s);
}

#[cfg(unix)]
#[test]
fn a_link_at_the_content_address_is_not_an_intact_copy() {
    // A restore copies only regular files out of the store (`copy_between`
    // refuses anything else), so a link, even to the right bytes, is not a copy
    // a restore can use.
    let s = setup();
    let elsewhere = s.store.parent().unwrap().join("elsewhere");
    std::fs::write(&elsewhere, BYTES).unwrap();
    std::fs::remove_file(&s.file).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &s.file).unwrap();
    let (ok, manifest, errors) = verify(&s);
    assert!(!ok, "a symlink passed as an attachment copy");
    assert_eq!(manifest["damaged"], 1, "{manifest}");
    assert_metadata_only(&errors, &s);
}

#[test]
fn a_report_stored_before_the_damaged_count_existed_still_reads() {
    // `verification_json` rows written by earlier builds have no `damaged`.
    let old = r#"{"status":"CHECKED","expected":2,"missing":[]}"#;
    let check: g9_storage::backup::AttachmentCheck = serde_json::from_str(old).unwrap();
    let value = serde_json::to_value(&check).unwrap();
    assert_eq!(value["damaged"], 0);
    assert_eq!(value["expected"], 2);
}

#[test]
fn a_schema_older_than_the_attachments_table_references_no_file() {
    // A pre-migration backup of a database at schema 1 to 4 is verified before
    // migration 0005 creates `attachments`: there is nothing to look up, and
    // the check must say so instead of failing on the missing table (REL-5;
    // on the unchanged code every such database stayed unmigrated under the
    // CLI and desktop layout, where an attachment store is always configured).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema-4.sqlite");
    {
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        g9_storage::Migrator::new("0.0.4", t(0))
            .migrate_up_to(&mut conn, 4)
            .unwrap();
    }
    let store = dir.path().join("attachments");
    std::fs::create_dir_all(&store).unwrap();
    let report = verify_snapshot(
        &path,
        &VerifyOptions {
            attachment_root: Some(store),
        },
    )
    .expect("verification must report, not fail");
    assert!(report.ok(), "{:?}", report.errors);
    let manifest = serde_json::to_value(&report.attachment_manifest).unwrap();
    assert_eq!(manifest["status"], "CHECKED");
    assert_eq!(manifest["expected"], 0);
    assert_eq!(
        g9_storage::attachment_store::referenced_digests_in(&path).unwrap(),
        Vec::<String>::new()
    );
}
