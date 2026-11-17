#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! STOR-2 (docs/release/path-to-10.md §5.2): `backup.verify` and
//! `backup.restore_test` hash every attachment file the snapshot references in
//! `<backups>/attachments`, the store `g9 restore` copies the files back from.
//! A damaged copy fails the backup exactly as a missing one does, reported by
//! code and count; the live store `<data>/attachments` is not what decides
//! whether a backup restores, so it is not consulted.

mod common;

use common::*;
use g9_domain::BackupId;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const BYTES: &[u8] = b"lab capture bytes";

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn content_address(store: &Path, bytes: &[u8]) -> PathBuf {
    let s = g9_domain::hashing::sha256_hex(bytes);
    store.join(&s[0..2]).join(&s[2..4]).join(&s)
}

/// The data-directory layout of the CLI and the desktop bridge: attachments,
/// backups and scratch are each an approved root.
fn harness() -> (Harness, PathBuf) {
    let dir = tempfile::Builder::new().prefix("g9bab").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["attachments", "backups", "scratch"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = ["attachments", "backups", "scratch"]
            .iter()
            .map(|d| r.join(d))
            .collect();
        s.config.attachment_dir = Some(r.join("attachments"));
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    });
    h.dir = Some(dir);
    (h, root)
}

/// One attached file, then `backup.create`; returns the backup id.
fn attached_and_backed_up(h: &mut Harness, root: &Path) -> String {
    h.call(
        "evidence.attach_file",
        json!({ "filename": "capture.txt", "media_type": "text/plain", "content_base64": b64(BYTES) }),
    )
    .unwrap();
    let created = h.call("backup.create", json!({})).unwrap();
    assert!(
        created["warnings"].as_array().is_none_or(|w| w.is_empty()),
        "{created}"
    );
    let copy = content_address(&root.join("backups/attachments"), BYTES);
    assert_eq!(std::fs::read(&copy).unwrap(), BYTES);
    created["result"]["backup_id"].as_str().unwrap().to_string()
}

fn flip_one_byte(path: &Path) {
    let mut bytes = std::fs::read(path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(path, bytes).unwrap();
}

/// The stored verification report (`backup_manifests.verification_json`).
fn stored_report(h: &Harness, id: &str) -> (Value, Vec<String>) {
    let stored = g9_storage::backup::read_manifest(h.conn(), &BackupId::parse(id).unwrap())
        .unwrap()
        .unwrap();
    let report = stored.manifest.verification.unwrap();
    (
        serde_json::to_value(&report.attachment_manifest).unwrap(),
        report.errors,
    )
}

fn assert_no_path_or_digest(errors: &[String]) {
    let sha = g9_domain::hashing::sha256_hex(BYTES);
    for line in errors {
        assert!(!line.contains(&sha), "{line}");
        assert!(!line.contains("capture"), "{line}");
        if line.starts_with("backup.attachments_") {
            assert!(!line.contains('/'), "{line}");
        }
    }
}

#[test]
fn one_flipped_byte_under_backups_attachments_fails_backup_verify() {
    let (mut h, root) = harness();
    let id = attached_and_backed_up(&mut h, &root);
    flip_one_byte(&content_address(&root.join("backups/attachments"), BYTES));
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "FAILED", "{verified}");
    assert_eq!(verified["result"]["integrity"], "FAILED");
    let (manifest, errors) = stored_report(&h, &id);
    assert_eq!(manifest["damaged"], 1, "{manifest}");
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("backup.attachments_damaged: 1 ")),
        "{errors:?}"
    );
    assert_no_path_or_digest(&errors);
}

#[test]
fn a_copy_damaged_after_verification_fails_the_restore_test() {
    let (mut h, root) = harness();
    let id = attached_and_backed_up(&mut h, &root);
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED", "{verified}");
    flip_one_byte(&content_address(&root.join("backups/attachments"), BYTES));
    let tested = h
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "FAILED", "{tested}");
    assert_eq!(tested["result"]["integrity"], "FAILED");
}

#[test]
fn a_file_missing_from_the_backup_store_fails_even_when_the_live_store_has_it() {
    let (mut h, root) = harness();
    let id = attached_and_backed_up(&mut h, &root);
    std::fs::remove_file(content_address(&root.join("backups/attachments"), BYTES)).unwrap();
    assert!(content_address(&root.join("attachments"), BYTES).is_file());
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "FAILED", "{verified}");
    let (_, errors) = stored_report(&h, &id);
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("backup.attachments_missing: 1 ")),
        "{errors:?}"
    );
    assert_no_path_or_digest(&errors);
}

#[test]
fn the_live_store_does_not_decide_whether_a_backup_restores() {
    // `g9 restore` copies the files from `<backups>/attachments` into the live
    // store and replaces a damaged file there, so a backup whose own copies are
    // intact is good whatever the live store holds.
    let (mut h, root) = harness();
    let id = attached_and_backed_up(&mut h, &root);
    flip_one_byte(&content_address(&root.join("attachments"), BYTES));
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED", "{verified}");
    std::fs::remove_file(content_address(&root.join("attachments"), BYTES)).unwrap();
    let tested = h
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED", "{tested}");
    let (manifest, _) = stored_report(&h, &id);
    assert_eq!(manifest["expected"], 1);
    assert_eq!(manifest["damaged"], 0);
}
