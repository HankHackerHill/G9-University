#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Evidence the operator attaches: `evidence.attach_file` (bytes from the UI,
//! stored by content hash), `evidence.from_note`, `evidence.list`, and the
//! attachment bytes a backup carries.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn sha(bytes: &[u8]) -> String {
    g9_domain::hashing::sha256_hex(bytes)
}

/// The desktop layout: attachments, backups, scratch and exports are each an approved root.
fn harness() -> (Harness, PathBuf) {
    let dir = tempfile::Builder::new().prefix("g9ev").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["attachments", "backups", "scratch", "exports"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = ["attachments", "backups", "scratch", "exports"]
            .iter()
            .map(|d| r.join(d))
            .collect();
        s.config.attachment_dir = Some(r.join("attachments"));
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
        s.config.export_dir = Some(r.join("exports"));
    });
    h.dir = Some(dir);
    (h, root)
}

fn note(h: &mut Harness, title: &str) -> String {
    h.call(
        "notes.create",
        json!({ "title": title, "body_markdown": "Lab write-up.", "classification": "INTERNAL", "domain": "LEARNING" }),
    )
    .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[allow(clippy::result_large_err)]
fn attach(
    h: &mut Harness,
    note_id: Option<&str>,
    name: &str,
    media: &str,
    bytes: &[u8],
) -> Result<Value, g9_domain::G9Error> {
    let mut payload =
        json!({ "filename": name, "media_type": media, "content_base64": b64(bytes) });
    if let Some(n) = note_id {
        payload["note_id"] = json!(n);
    }
    h.call("evidence.attach_file", payload)
}

fn stored(root: &Path, bytes: &[u8]) -> PathBuf {
    let s = sha(bytes);
    root.join(format!("{}/{}/{}", &s[0..2], &s[2..4], s))
}

#[test]
fn a_file_is_stored_by_content_and_becomes_evidence_for_a_submission() {
    let (mut h, root) = harness();
    let n = note(&mut h, "Cache poisoning lab");
    let bytes = b"# Findings\n\nX-Forwarded-Host is unkeyed.\n";
    let first = attach(&mut h, Some(&n), "findings.md", "text/markdown", bytes).unwrap();
    let r = &first["result"];
    assert_eq!(r["sha256"], sha(bytes));
    assert_eq!(r["size_bytes"], bytes.len());
    assert_eq!(r["already_stored"], false);
    assert_eq!(r["note_id"], n.as_str());
    let path = stored(&root.join("attachments"), bytes);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    // Identical bytes are stored once; a second attachment row points at them.
    let again = attach(&mut h, Some(&n), "copy.md", "text/markdown", bytes).unwrap();
    assert_eq!(again["result"]["already_stored"], true);

    let listed = h.call("evidence.list", json!({ "note_id": n })).unwrap();
    let items = listed["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["kind"] == "ATTACHMENT"));
    assert!(items.iter().any(|i| i["filename"] == "findings.md"));

    // The evidence goes into an assessment submission.
    let assessment = seed_blind_practical(&h, 1, None);
    let attempt = h
        .call(
            "assessment.begin",
            json!({ "assessment_id": assessment.as_str() }),
        )
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    let evidence_id = r["evidence_id"].as_str().unwrap();
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [evidence_id] }),
    )
    .unwrap();
    let linked: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM attempt_evidence WHERE attempt_id = ?1 AND evidence_item_id = ?2",
            [attempt.as_str(), evidence_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(linked, 1);
}

#[test]
fn executables_archives_and_mismatched_types_are_refused_and_nothing_is_stored() {
    let (mut h, root) = harness();
    let elf = b"\x7fELF\x02\x01\x01\x00payload";
    let refused = attach(&mut h, None, "tool", "text/plain", elf).unwrap_err();
    assert_eq!(refused.code, "attachment.refused", "{refused:?}");
    let zip = b"PK\x03\x04rest";
    let refused = attach(&mut h, None, "a.csv", "text/csv", zip).unwrap_err();
    assert_eq!(refused.code, "attachment.refused");
    let fake_png = b"not a png at all";
    let mismatch = attach(&mut h, None, "shot.png", "image/png", fake_png).unwrap_err();
    assert_eq!(mismatch.code, "attachment.content_mismatch");
    // Not on the allow-list: refused by the request schema.
    let schema = attach(&mut h, None, "a.zip", "application/zip", b"x").unwrap_err();
    assert_eq!(schema.category, ErrorCategory::InvalidInput);
    let bad = h
        .call(
            "evidence.attach_file",
            json!({ "filename": "a.txt", "media_type": "text/plain", "content_base64": "%%%%" }),
        )
        .unwrap_err();
    assert_eq!(bad.code, "attachment.content_invalid");
    let path_name = attach(&mut h, None, "../../etc/passwd", "text/plain", b"x").unwrap_err();
    assert_eq!(path_name.code, "attachment.filename_invalid");

    let entries = std::fs::read_dir(root.join("attachments")).unwrap().count();
    assert_eq!(entries, 0, "nothing reached the store");
    let rows: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM attachments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn a_file_on_a_security_sensitive_note_needs_an_approval() {
    let (mut h, root) = harness();
    let scope = Opts {
        scope: Some(("SECURITY_SENSITIVE", "STUDY")),
        ..Opts::default()
    };
    let n = h.approved(
        "notes.create",
        json!({ "title": "Scope letter", "body_markdown": "Authorized targets.", "classification": "SECRET", "domain": "SECURITY_SENSITIVE" }),
        &scope,
    )["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();
    let bytes = b"%PDF-1.7\n...";
    let payload = json!({ "note_id": n, "filename": "letter.pdf", "media_type": "application/pdf", "content_base64": b64(bytes) });
    let attached = h.approved("evidence.attach_file", payload, &scope);
    assert_eq!(attached["result"]["note_id"], n.as_str());
    assert!(stored(&root.join("attachments"), bytes).is_file());
    let (domain, class): (String, String) = h
        .conn()
        .query_row(
            "SELECT data_domain, classification FROM attachments WHERE id = ?1",
            [attached["result"]["attachment_id"].as_str().unwrap()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (domain.as_str(), class.as_str()),
        ("SECURITY_SENSITIVE", "SECRET")
    );
}

#[test]
fn a_typed_note_becomes_evidence() {
    let (mut h, _root) = harness();
    let n = note(&mut h, "Reflection");
    let made = h
        .call("evidence.from_note", json!({ "note_id": n }))
        .unwrap();
    assert_eq!(made["result"]["kind"], "NOTE");
    assert_eq!(made["result"]["summary"], "Reflection");
    let recent = h.call("evidence.list", json!({})).unwrap();
    assert!(recent["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["evidence_id"] == made["result"]["evidence_id"]));
    let missing = h
        .call("evidence.from_note", json!({ "note_id": "note-missing" }))
        .unwrap_err();
    assert_eq!(missing.category, ErrorCategory::NotFound);
}

#[test]
fn without_an_attachment_directory_nothing_is_attached() {
    let mut h = Harness::new();
    let refused = h
        .call(
            "evidence.attach_file",
            json!({ "filename": "a.txt", "media_type": "text/plain", "content_base64": b64(b"x") }),
        )
        .unwrap_err();
    assert_eq!(refused.code, "attachment.store_not_configured");
}

#[test]
fn a_backup_carries_the_attachment_bytes() {
    let (mut h, root) = harness();
    let bytes = b"evidence bytes";
    attach(&mut h, None, "e.txt", "text/plain", bytes).unwrap();
    let created = h.call("backup.create", json!({})).unwrap();
    assert!(
        created["warnings"].as_array().is_none_or(|w| w.is_empty()),
        "{created}"
    );
    let copy = stored(&root.join("backups/attachments"), bytes);
    assert_eq!(std::fs::read(copy).unwrap(), bytes);
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
}
