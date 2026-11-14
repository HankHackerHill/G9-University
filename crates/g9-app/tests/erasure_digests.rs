#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! A digest of erasable content is content (ADR-0017, security audit 2026-11-13).
//!
//! The digest of a short text is the text: anyone who holds the SHA-256 of a
//! four-digit code recovers the code in ten thousand guesses. Erasure therefore
//! removes every digest of what it erases, and an export carries no digest of
//! erasable content, no text the operator wrote about an erasure and no approval
//! MAC. Each test scans every column of every table (or every file of the export)
//! for the digests, so a new place that stores one fails here.

mod common;

use common::*;
use g9_domain::hashing::sha256_hex;
use serde_json::json;
use std::path::{Path, PathBuf};

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn harness() -> (Harness, PathBuf) {
    let dir = tempfile::Builder::new().prefix("g9er").tempdir().unwrap();
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

/// Every `table.column` of the database with a value that contains `needle`.
fn holders(conn: &rusqlite::Connection, needle: &str) -> Vec<String> {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut found = Vec::new();
    for table in tables {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for column in columns {
            let n: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{table}\" WHERE instr(CAST(\"{column}\" AS TEXT), ?1) > 0"),
                    [needle],
                    |r| r.get(0),
                )
                .unwrap();
            if n > 0 {
                found.push(format!("{table}.{column}"));
            }
        }
    }
    found
}

/// Every file under `dir` whose bytes contain `needle`.
fn files_holding(dir: &Path, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains(needle) {
                found.push(path.file_name().unwrap().to_string_lossy().to_string());
            }
        }
    }
    found.sort();
    found
}

/// Bodies of the note below. Both contain letters that cannot occur in a hex digest or an id: a
/// bare "4712" occurs inside random hex strings now and then (this test failed once by chance).
const FIRST_BODY: &str = "pin-code-4711";
const SECOND_BODY: &str = "pin-code-4712";

/// A note with two versions, a file and both kinds of evidence; returns its id and the digests of everything erasable in it.
fn note_with_everything(h: &mut Harness, classification: &str) -> (String, Vec<String>) {
    let created = h
        .call(
            "notes.create",
            json!({ "title": "door code", "body_markdown": FIRST_BODY, "classification": classification, "domain": "LEARNING" }),
        )
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    h.call_with(
        "notes.revise",
        json!({ "note_id": note_id, "body_markdown": SECOND_BODY }),
        &Opts::revision(1),
    )
    .unwrap();
    let file = b"0042";
    for (op, payload) in [
        (
            "evidence.attach_file",
            json!({ "note_id": note_id, "filename": "code.txt", "media_type": "text/plain", "content_base64": b64(file) }),
        ),
        ("evidence.from_note", json!({ "note_id": note_id })),
    ] {
        // A SECRET or PRIVATE note is protected: the same call goes through its approval.
        if h.call(op, payload.clone()).is_err() {
            h.approved(op, payload, &Opts::default());
        }
    }
    let digests = vec![
        sha256_hex(FIRST_BODY.as_bytes()),
        sha256_hex(SECOND_BODY.as_bytes()),
        sha256_hex(file),
    ];
    (note_id, digests)
}

fn erase(h: &mut Harness, note_id: &str) {
    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "the door code changed" }),
        &Opts::default(),
    );
    assert_eq!(done["result"]["state"], "APPLIED", "{done}");
}

#[test]
fn erasing_a_note_leaves_no_digest_of_its_versions_or_files_in_any_table() {
    for classification in ["INTERNAL", "PRIVATE", "SECRET"] {
        let (mut h, _root) = harness();
        let (note_id, digests) = note_with_everything(&mut h, classification);
        erase(&mut h, &note_id);
        for d in &digests {
            assert_eq!(
                holders(h.conn(), d),
                Vec::<String>::new(),
                "{classification}: a digest of erased content is still stored"
            );
        }
        // The content itself is gone too (the versions held both bodies). The needle must not be
        // able to occur inside a hex digest or an id, or the check would fail by chance.
        assert_eq!(
            holders(h.conn(), SECOND_BODY),
            Vec::<String>::new(),
            "{classification}"
        );
    }
}

#[test]
fn before_an_erasure_no_immutable_table_holds_a_digest_of_a_note_or_its_files() {
    // Rows of these tables can never be changed, so a digest written there could never be erased.
    let (mut h, _root) = harness();
    let (_note_id, digests) = note_with_everything(&mut h, "INTERNAL");
    for d in &digests {
        let immutable: Vec<String> = holders(h.conn(), d)
            .into_iter()
            .filter(|c| {
                [
                    "domain_events.",
                    "source_provenance.",
                    "audit_metadata.",
                    "idempotency_records.",
                ]
                .iter()
                .any(|t| c.starts_with(t))
            })
            .collect();
        assert_eq!(
            immutable,
            Vec::<String>::new(),
            "a digest of erasable content is in a table that cannot be erased"
        );
    }
}

#[test]
fn the_deletion_result_and_its_tombstones_carry_no_digest_of_the_erased_content() {
    let (mut h, _root) = harness();
    let (note_id, digests) = note_with_everything(&mut h, "SECRET");
    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "the door code changed" }),
        &Opts::default(),
    );
    let tombstone = done["result"]["tombstone_sha256"].as_str().unwrap();
    assert_eq!(tombstone.len(), 64);
    assert!(
        !digests.iter().any(|d| d == tombstone),
        "the tombstone is a digest of the erased content"
    );
    // It identifies the tombstone row: the same value is stored for the note.
    let stored: String = h
        .conn()
        .query_row(
            "SELECT content_sha256_before FROM deletion_tombstones WHERE table_name = 'notes' AND row_id = ?1",
            [&note_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, tombstone);
}

#[test]
fn an_export_of_every_area_holds_no_digest_of_erased_content_no_erasure_reason_and_no_approval_mac()
{
    let (mut h, _root) = harness();
    let (note_id, digests) = note_with_everything(&mut h, "PRIVATE");
    erase(&mut h, &note_id);
    let mac: String = h
        .conn()
        .query_row(
            "SELECT binding_mac FROM approval_requests WHERE binding_mac IS NOT NULL LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    for format in ["JSON", "CSV"] {
        let out = h.approved(
            "export.request",
            json!({ "domains": ["LEARNING", "BUSINESS", "PERSONAL", "SECURITY_SENSITIVE", "SYSTEM"], "format": format, "include_private": true }),
            &Opts::default(),
        );
        assert_eq!(out["result"]["state"], "READY", "{out}");
        let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
        for d in &digests {
            assert_eq!(
                files_holding(&dir, d),
                Vec::<String>::new(),
                "{format}: digest of erased content in the export"
            );
        }
        assert_eq!(
            files_holding(&dir, "the door code changed"),
            Vec::<String>::new(),
            "{format}: erasure reason in the export"
        );
        assert_eq!(
            files_holding(&dir, &mac),
            Vec::<String>::new(),
            "{format}: approval MAC in the export"
        );
    }
}

#[test]
fn an_export_holds_no_digest_of_a_secret_note_that_was_never_erased() {
    // SECRET rows are never exported; neither is anything derived from them.
    let (mut h, _root) = harness();
    let (_note_id, digests) = note_with_everything(&mut h, "SECRET");
    let out = h.approved(
        "export.request",
        json!({ "domains": ["LEARNING", "SYSTEM"], "format": "JSON", "include_private": true }),
        &Opts::default(),
    );
    let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
    for d in &digests {
        assert_eq!(
            files_holding(&dir, d),
            Vec::<String>::new(),
            "digest of SECRET content in the export"
        );
    }
    assert_eq!(
        files_holding(&dir, "door code"),
        Vec::<String>::new(),
        "title of a SECRET note in the export"
    );
}

#[test]
fn a_reason_longer_than_the_limit_is_refused_before_anything_is_stored() {
    let (mut h, _root) = harness();
    let (note_id, _) = note_with_everything(&mut h, "INTERNAL");
    let err = h
        .call(
            "deletion.request",
            json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "x".repeat(501) }),
        )
        .unwrap_err();
    assert_eq!(
        err.category,
        g9_domain::ErrorCategory::InvalidInput,
        "{err:?}"
    );
    let n: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM deletion_requests", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}

// ---------------------------------------------------------------------------
// The bytes of an erased file leave the attachment folder, and only those.
// ---------------------------------------------------------------------------

/// Where the content-addressed store keeps `bytes` under `root`.
fn stored_at(root: &Path, bytes: &[u8]) -> PathBuf {
    let sha = sha256_hex(bytes);
    root.join("attachments")
        .join(&sha[0..2])
        .join(&sha[2..4])
        .join(&sha)
}

fn note_with_file(h: &mut Harness, title: &str, bytes: &[u8]) -> String {
    let created = h
        .call(
            "notes.create",
            json!({ "title": title, "body_markdown": "b", "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    h.call(
        "evidence.attach_file",
        json!({ "note_id": note_id, "filename": "f.txt", "media_type": "text/plain", "content_base64": b64(bytes) }),
    )
    .unwrap();
    note_id
}

#[test]
fn erasing_a_note_removes_its_file_from_the_folder_and_leaves_every_other_file() {
    let (mut h, root) = harness();
    let shared = b"the same bytes in two notes";
    let only_a = b"bytes only note A has";
    let other = b"bytes of a note that is kept";
    let a = note_with_file(&mut h, "A", shared);
    h.call(
        "evidence.attach_file",
        json!({ "note_id": a, "filename": "second.txt", "media_type": "text/plain", "content_base64": b64(only_a) }),
    )
    .unwrap();
    let b = note_with_file(&mut h, "B", shared);
    let _kept = note_with_file(&mut h, "kept", other);
    // A file the application did not put there, and a file of an earlier database
    // (after a restore the folder can hold files this database does not reference).
    let foreign = root.join("attachments").join("README.txt");
    std::fs::write(&foreign, b"not ours").unwrap();
    let earlier = stored_at(&root, b"referenced only by a quarantined database");
    std::fs::create_dir_all(earlier.parent().unwrap()).unwrap();
    std::fs::write(&earlier, b"referenced only by a quarantined database").unwrap();
    for f in [
        stored_at(&root, shared),
        stored_at(&root, only_a),
        stored_at(&root, other),
    ] {
        assert!(f.is_file(), "{} is stored", f.display());
    }

    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": a }, "reason": "test" }),
        &Opts::default(),
    );
    assert_eq!(done["result"]["state"], "APPLIED", "{done}");
    let warnings = done["warnings"].to_string();
    assert!(!warnings.contains("not_purged"), "{done}");
    assert!(
        !stored_at(&root, only_a).exists(),
        "the erased note's own file is still in the folder"
    );
    assert!(
        stored_at(&root, shared).is_file(),
        "note B still uses these bytes"
    );
    assert!(stored_at(&root, other).is_file());
    assert!(
        foreign.is_file() && earlier.is_file(),
        "a file this erasure did not own was removed"
    );

    // The last note that used the shared bytes: now they go too.
    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": b }, "reason": "test" }),
        &Opts::default(),
    );
    assert_eq!(done["result"]["state"], "APPLIED", "{done}");
    assert!(
        !stored_at(&root, shared).exists(),
        "the shared bytes outlived their last note"
    );
    assert!(stored_at(&root, other).is_file());
    assert!(
        foreign.is_file() && earlier.is_file(),
        "a file this erasure did not own was removed"
    );
}

#[test]
fn an_erasure_without_an_attachment_folder_says_the_bytes_were_not_removed() {
    // The database knows the file; this process was not given the folder.
    let mut h = Harness::new();
    let created = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": "b", "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    let sha = sha256_hex(b"x");
    h.conn()
        .execute(
            "INSERT INTO attachments (id, note_id, original_filename, media_type, size_bytes, content_sha256, storage_ref,
                source_kind, trust, data_domain, classification, created_at, updated_at)
             SELECT 'att-1', id, 'x.txt', 'text/plain', 1, ?2, ?3, 'ATTACHMENT', 'IMPORTED_DOCUMENT', data_domain,
                classification, created_at, created_at FROM notes WHERE id = ?1",
            rusqlite::params![note_id, sha, format!("{}/{}/{}", &sha[0..2], &sha[2..4], sha)],
        )
        .unwrap();
    let done = h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "test" }),
        &Opts::default(),
    );
    assert_eq!(done["result"]["state"], "APPLIED", "{done}");
    assert!(
        done["warnings"]
            .to_string()
            .contains("deletion.attachment_bytes_not_purged"),
        "{done}"
    );
}

// ---------------------------------------------------------------------------
// Pass C of the audit found two more places that held a digest of erasable text.
// ---------------------------------------------------------------------------

/// Every value of `table.column` of the live database, for columns that hold a digest.
fn stored_values(conn: &rusqlite::Connection, table: &str, column: &str) -> Vec<String> {
    conn.prepare(&format!(
        "SELECT \"{column}\" FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"
    ))
    .unwrap()
    .query_map([], |r| r.get::<_, String>(0))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

#[test]
fn an_export_holds_no_hash_of_a_request_that_carried_a_secret_and_no_value_that_could_confirm_one()
{
    // The hash of the whole request ({operation, payload}) is a guess-confirmation oracle: a
    // four-digit body is recovered from it in 10,000 tries when the title is guessable. The
    // policy, approval and audit ledgers keep such hashes; an export carries none of them, and
    // none of the chain values a guess could be checked against either.
    let (mut h, _root) = harness();
    let secret = json!({ "title": "pin", "body_markdown": "4711", "classification": "SECRET", "domain": "LEARNING" });
    h.call("notes.create", secret.clone()).unwrap();
    // An approval-gated request too: its approval carries a parameters hash and an operation descriptor.
    let (note_id, _) = note_with_everything(&mut h, "INTERNAL");
    erase(&mut h, &note_id);

    let request_hash = g9_app::pipeline::parameters_sha256("notes.create", &secret);
    let mut oracle_values = vec![request_hash.clone()];
    for (table, column) in [
        ("policy_decisions", "parameters_sha256"),
        ("approval_requests", "parameters_sha256"),
        ("approval_decisions", "parameters_sha256"),
        ("audit_metadata", "details_sha256"),
        ("audit_metadata", "prev_hash"),
        ("audit_metadata", "row_hash"),
    ] {
        let values = stored_values(h.conn(), table, column);
        assert!(
            !values.is_empty(),
            "{table}.{column} holds nothing: the check below would prove nothing"
        );
        oracle_values.extend(values);
    }
    assert!(
        stored_values(h.conn(), "policy_decisions", "parameters_sha256").contains(&request_hash),
        "the guess-confirmation oracle is not what this test assumes"
    );
    let descriptor: String = h
        .conn()
        .query_row(
            "SELECT operation_json FROM approval_requests LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();

    for format in ["JSON", "CSV"] {
        let out = h.approved(
            "export.request",
            json!({ "domains": ["LEARNING", "BUSINESS", "PERSONAL", "SECURITY_SENSITIVE", "SYSTEM"], "format": format, "include_private": true }),
            &Opts::default(),
        );
        assert_eq!(out["result"]["state"], "READY", "{out}");
        let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
        for value in &oracle_values {
            assert_eq!(
                files_holding(&dir, value),
                Vec::<String>::new(),
                "{format}: a request or ledger hash is in the export"
            );
        }
        assert_eq!(
            files_holding(&dir, &descriptor),
            Vec::<String>::new(),
            "{format}: an approval's operation descriptor is in the export"
        );
    }
}

#[test]
fn an_erased_submission_note_leaves_no_digest_of_its_text_in_the_attempt() {
    // `assessment.submit` stored the operator's notes as an erasable note, and the attempt (an
    // evidentiary row that is never erased) kept the hash of {evidence_refs, notes}: with the
    // evidence ids in the export, a dictionary attack on the erased text.
    let (mut h, _root) = harness();
    seed_blind_practical(&h, 1, Some(3));
    let attempt = h
        .call("assessment.begin", json!({ "assessment_id": "ad-w1" }))
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    let text = "4711";
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [], "notes": text }),
    )
    .unwrap();
    let note_id: String = h
        .conn()
        .query_row(
            "SELECT e.note_id FROM attempt_evidence ae JOIN evidence_items e ON e.id = ae.evidence_item_id WHERE ae.attempt_id = ?1",
            [&attempt],
            |r| r.get(0),
        )
        .unwrap();
    erase(&mut h, &note_id);

    let guesses = [
        sha256_hex(text.as_bytes()),
        g9_domain::hashing::sha256_json(&json!({ "evidence_refs": [], "notes": text })),
    ];
    for guess in &guesses {
        assert_eq!(
            holders(h.conn(), guess),
            Vec::<String>::new(),
            "a digest of the erased submission text is still stored"
        );
    }
    let out = h.approved(
        "export.request",
        json!({ "domains": ["LEARNING", "SYSTEM"], "format": "JSON", "include_private": true }),
        &Opts::default(),
    );
    let dir = PathBuf::from(out["result"]["location"].as_str().unwrap());
    for guess in &guesses {
        assert_eq!(
            files_holding(&dir, guess),
            Vec::<String>::new(),
            "a digest of the erased submission text is in the export"
        );
    }
}
