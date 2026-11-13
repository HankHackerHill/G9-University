#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `export.request` writes the export (it returned REQUESTED and wrote nothing):
//! approval-gated, the selected domains only, PRIVATE and SECRET rows only with
//! `include_private`, attachment bytes copied and hash-checked, manifest last.

mod common;

use common::*;
use g9_domain::ErrorCategory;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn harness() -> (Harness, PathBuf) {
    let dir = tempfile::Builder::new().prefix("g9ex").tempdir().unwrap();
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

fn note(h: &mut Harness, title: &str, classification: &str) -> String {
    h.call(
        "notes.create",
        json!({ "title": title, "body_markdown": format!("Body of {title}."), "classification": classification, "domain": "LEARNING" }),
    )
    .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn export(h: &mut Harness, format: &str, include_private: bool) -> Value {
    h.approved(
        "export.request",
        json!({ "domains": ["LEARNING"], "format": format, "include_private": include_private }),
        &Opts::default(),
    )
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn an_approved_json_export_holds_the_selected_rows_and_the_attachment_bytes() {
    let (mut h, root) = harness();
    let shared = note(&mut h, "Shared lab", "INTERNAL");
    note(&mut h, "Private diary", "PRIVATE");
    let bytes = b"capture summary";
    let attached = h
        .call(
            "evidence.attach_file",
            json!({ "note_id": shared, "filename": "summary.txt", "media_type": "text/plain", "content_base64": b64(bytes) }),
        )
        .unwrap();
    let sha = attached["result"]["sha256"].as_str().unwrap().to_string();

    let out = export(&mut h, "JSON", false);
    let r = &out["result"];
    assert_eq!(r["state"], "READY", "{out}");
    assert_eq!(r["attachments"], 1);
    let dir = PathBuf::from(r["location"].as_str().unwrap());
    assert!(dir.starts_with(std::fs::canonicalize(root.join("exports")).unwrap()));

    let data = read_json(&dir.join("data.json"));
    let notes = &data["tables"]["notes"];
    let title_col = notes["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c == "title")
        .unwrap();
    let titles: Vec<&str> = notes["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[title_col].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"Shared lab"));
    assert!(
        !titles.contains(&"Private diary"),
        "PRIVATE rows need include_private"
    );
    assert!(data["tables"].get("idempotency_records").is_none());
    assert!(
        data["tables"].get("audit_metadata").is_none(),
        "SYSTEM was not selected"
    );

    let copy = dir.join(format!("attachments/{}/{}/{}", &sha[0..2], &sha[2..4], sha));
    assert_eq!(std::fs::read(copy).unwrap(), bytes);

    let manifest_bytes = std::fs::read(dir.join("manifest.json")).unwrap();
    assert_eq!(
        g9_domain::hashing::sha256_hex(&manifest_bytes),
        r["manifest_sha256"].as_str().unwrap()
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    for file in manifest["files"].as_array().unwrap() {
        let on_disk = std::fs::read(dir.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            g9_domain::hashing::sha256_hex(&on_disk),
            file["sha256"].as_str().unwrap()
        );
    }
}

#[test]
fn include_private_exports_private_rows_as_csv() {
    let (mut h, _root) = harness();
    note(&mut h, "Private diary", "PRIVATE");
    note(&mut h, "=cmd|' /C calc'!A0", "INTERNAL");
    let out = export(&mut h, "CSV", true);
    let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
    let csv = std::fs::read_to_string(dir.join("notes.csv")).unwrap();
    assert!(csv.contains("Private diary"));
    assert!(
        csv.contains("'=cmd|"),
        "a cell that would start a formula is prefixed: {csv}"
    );
    assert!(dir.join("study_sessions.csv").is_file());
    assert!(!dir.join("data.json").exists());
}

#[test]
fn without_an_approval_no_export_is_written() {
    let (mut h, root) = harness();
    note(&mut h, "Shared lab", "INTERNAL");
    let pending = h
        .call(
            "export.request",
            json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false }),
        )
        .unwrap_err();
    assert_eq!(pending.category, ErrorCategory::ApprovalRequired);
    assert_eq!(std::fs::read_dir(root.join("exports")).unwrap().count(), 0);
}

#[test]
fn without_an_export_directory_the_approved_export_fails_and_the_approval_is_not_spent() {
    // The directory is checked after policy: the approval request comes first,
    // and the refusal rolls the command back, so the approval stays usable.
    let mut h = Harness::new();
    let payload = json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false });
    let pending = h.call("export.request", payload.clone()).unwrap_err();
    assert_eq!(pending.category, ErrorCategory::ApprovalRequired);
    let id = pending.approval_id.clone().unwrap();
    h.decide(id.as_str(), "APPROVE");
    let refused = h
        .call_with("export.request", payload, &Opts::approval(id.as_str()))
        .unwrap_err();
    assert_eq!(refused.code, "export.dir_not_configured", "{refused:?}");
    let state = g9_storage::approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap()
        .state;
    assert_eq!(state, g9_domain::ApprovalState::Approved, "not consumed");
}

#[test]
fn secret_rows_are_never_exported_even_with_include_private() {
    // docs/privacy/data-lifecycle.md "Exports (A10)": PRIVATE requires approval,
    // SECRET is never exported.
    let (mut h, _root) = harness();
    note(&mut h, "Private diary", "PRIVATE");
    let secret = h
        .call(
            "notes.create",
            json!({ "title": "Lab credentials", "body_markdown": "never leaves", "classification": "SECRET", "domain": "LEARNING" }),
        )
        .unwrap();
    assert!(secret["result"]["note_id"].is_string(), "{secret}");
    let out = export(&mut h, "JSON", true);
    let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
    let text = std::fs::read_to_string(dir.join("data.json")).unwrap();
    assert!(text.contains("Private diary"));
    assert!(
        !text.contains("Lab credentials"),
        "a SECRET row left in an export"
    );
    assert!(!text.contains("never leaves"));
}
