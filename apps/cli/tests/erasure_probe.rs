#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! PRIV-3 (`docs/release/path-to-10.md` §5.15): the raw-byte erasure probe.
//!
//! After an approved erasure, no trace of the erased content is left in any file of the data
//! directory, read as bytes: the database and its `-wal` and `-shm`, the attachment store, the
//! backups taken after the erasure and their attachment copies, the exports, the operations log
//! (`logs/`), `restore-log.jsonl`, the quarantine and the scratch directory. The probe searches for
//! the erased title, body (with a four-digit secret in it), file name and file bytes, the base64
//! the file travelled in, and the SHA-256 hex of the title, body, secret, file name and file bytes.
//!
//! The only expected hits are in the snapshot taken before the erasure (S1) and in the attachment
//! copy that only S1 references: they predate the erasure by design (the runbook's retention
//! removes them; PRIV-4). Positive controls show that the same search finds every stored marker
//! before the erasure, that a note that was not erased is found after it in every place the erased
//! one is looked for, and that the search finds each needle at all.
//!
//! `an_approved_erasure_leaves_no_trace_outside_the_snapshot_taken_before_it` runs the real `g9`
//! binary, one process per command, on a real data directory (the shipped layout, logs included).
//! `the_live_wal_holds_no_erased_content_while_the_application_still_runs` keeps one process open
//! (as the desktop application does) and reads the files while its connection is still open.

mod common;

use common::{base64, g9};
use g9_cli::bootstrap::{open, BootOptions};
use g9_cli::client::Client;
use g9_cli::meta::MetaOptions;
use g9_domain::hashing::sha256_hex;
use g9_storage::{Database, OpenOptions};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

const TITLE: &str = "PRIV3-TITLE-qz71k-erased";
const SECRET: &str = "4817";
/// The erased body: longer than a database page, so that most of it lives in overflow pages,
/// which an erasure frees (a freed page keeps its bytes unless `secure_delete` zeroes it).
/// Markers at the start, in the middle (the secret) and at the end.
fn body() -> String {
    let filler = "Lab notes for the erased entry, nothing else of interest in this line. ";
    format!(
        "{BODY_START}: {}the door code is {SECRET}. {}{BODY_END}",
        filler.repeat(60),
        filler.repeat(60)
    )
}
const BODY_START: &str = "PRIV3-BODY-mw83j-erased";
const BODY_END: &str = "PRIV3-BODY-END-k5f1z-erased";
/// The secret in its context. The bare four digits are not searched: a time-ordered id or a
/// millisecond timestamp in any file can hold them by chance. The secret is covered by this
/// context and by its SHA-256 (ADR-0017: the digest of a four-digit code is the code).
const SECRET_IN_CONTEXT: &str = "the door code is 4817";
const FILENAME: &str = "priv3-fname-vx52h-erased.txt";
const FILE_BYTES: &str = "PRIV3-FILE-BYTES-ht94p-erased\n";
const REASON: &str = "PRIV3-REASON-ua36n";

/// A note and a file that are never erased: found after the erasure wherever the erased ones
/// are looked for, so their absence is not an accident of where the search looks.
const KEEP_TITLE: &str = "PRIV3-KEEP-TITLE-r4c8d";
const KEEP_BODY: &str = "PRIV3-KEEP-BODY-n2w6s";
const KEEP_FILE: &str = "PRIV3-KEEP-FILE-BYTES-p9t3e\n";

fn sha(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

/// What the erasure must remove, as `(label, needle)`. Every needle is ASCII, so it can be
/// searched in a lossy UTF-8 reading of a binary file without changing a match.
fn erased_needles() -> Vec<(String, String)> {
    let mut v = vec![
        ("title".to_string(), TITLE.to_string()),
        ("body".to_string(), BODY_START.to_string()),
        ("body end".to_string(), BODY_END.to_string()),
        (
            "secret in context".to_string(),
            SECRET_IN_CONTEXT.to_string(),
        ),
        ("file name".to_string(), FILENAME.to_string()),
        ("file bytes".to_string(), FILE_BYTES.trim_end().to_string()),
        ("file base64".to_string(), base64(FILE_BYTES.as_bytes())),
    ];
    for (label, value) in [
        ("title", TITLE),
        ("body", body().as_str()),
        ("secret", SECRET),
        ("file name", FILENAME),
        ("file bytes", FILE_BYTES),
    ] {
        let hex = sha(value);
        v.push((format!("sha256({label})"), hex.clone()));
        v.push((format!("SHA256({label})"), hex.to_uppercase()));
    }
    for (_, needle) in &v {
        assert!(needle.is_ascii() && needle.len() >= 16, "{needle}");
    }
    assert!(body().contains(SECRET_IN_CONTEXT) && body().len() > 8_000);
    v
}

/// The needles that the design stores before an erasure. The rest never reach the disk: the
/// base64 is decoded at the boundary, no digest of the title, the secret or the file name is
/// kept, and digests are stored in lower case.
const STORED_BEFORE: [&str; 8] = [
    "title",
    "body",
    "body end",
    "secret in context",
    "file name",
    "file bytes",
    "sha256(body)",
    "sha256(file bytes)",
];

// ---------------------------------------------------------------------------
// The byte search
// ---------------------------------------------------------------------------

/// Every regular file under `root`, as paths relative to it, sorted.
fn files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                walk(&path, root, out);
            } else if kind.is_file() {
                out.push(path.strip_prefix(root).unwrap().to_path_buf());
            } else {
                panic!("unexpected non-regular file {}", path.display());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// One reading of every file under a root: relative path and contents.
struct Snapshot {
    files: Vec<(String, String)>,
}

impl Snapshot {
    fn take(root: &Path) -> Self {
        let files = files(root)
            .into_iter()
            .map(|rel| {
                let bytes = std::fs::read(root.join(&rel)).unwrap();
                (
                    rel.to_string_lossy().into_owned(),
                    String::from_utf8_lossy(&bytes).into_owned(),
                )
            })
            .collect();
        Self { files }
    }

    /// Files whose bytes, or whose relative path, contain `needle`.
    fn hits(&self, needle: &str) -> BTreeSet<String> {
        self.files
            .iter()
            .filter(|(path, text)| path.contains(needle) || text.contains(needle))
            .map(|(path, _)| path.clone())
            .collect()
    }

    fn paths(&self) -> BTreeSet<String> {
        self.files.iter().map(|(p, _)| p.clone()).collect()
    }
}

fn is_live_database(path: &str) -> bool {
    matches!(path, "g9.sqlite" | "g9.sqlite-wal" | "g9.sqlite-shm")
}

// ---------------------------------------------------------------------------
// The data directory, driven through the `g9` binary
// ---------------------------------------------------------------------------

struct Env {
    _dir: tempfile::TempDir,
    data: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        Self { _dir: dir, data }
    }

    fn run(&self, args: &[&str]) -> (i32, Value, String) {
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--data-dir", self.data.to_str().unwrap(), "--no-skills"]);
        g9(&all)
    }

    fn call(&self, op: &str, payload: &Value) -> Value {
        let text = payload.to_string();
        let (code, out, err) = self.run(&["call", op, "--payload", &text]);
        assert_eq!(code, 0, "{op}: {out} {err}");
        out
    }

    /// `APPROVAL_REQUIRED`, `g9 approve`, the retry with the approval.
    fn approved(&self, op: &str, payload: &Value) -> Value {
        let text = payload.to_string();
        let (code, refused, _) = self.run(&["call", op, "--payload", &text]);
        assert_eq!(code, 2, "{op}: {refused}");
        assert_eq!(refused["category"], "APPROVAL_REQUIRED", "{refused}");
        let id = refused["approval_id"].as_str().unwrap().to_string();
        let (code, decided, _) = self.run(&["approve", &id]);
        assert_eq!(code, 0, "{decided}");
        let (code, out, err) = self.run(&["call", op, "--payload", &text, "--approval", &id]);
        assert_eq!(code, 0, "{op}: {out} {err}");
        out
    }

    fn note(&self, title: &str, body: &str) -> String {
        let out = self.call(
            "notes.create",
            &json!({ "title": title, "body_markdown": body, "classification": "PRIVATE", "domain": "LEARNING" }),
        );
        out["result"]["note_id"].as_str().unwrap().to_string()
    }

    fn attach(&self, note_id: &str, filename: &str, bytes: &str) -> Value {
        self.call(
            "evidence.attach_file",
            &json!({
                "note_id": note_id,
                "filename": filename,
                "media_type": "text/plain",
                "content_base64": base64(bytes.as_bytes()),
            }),
        )
    }

    /// A backup driven to RETAINED (create, verify, restore test). Returns its id.
    fn retained_backup(&self, label: &str) -> String {
        let created = self.call("backup.create", &json!({ "label": label }));
        let id = created["result"]["backup_id"].as_str().unwrap().to_string();
        let verified = self.call("backup.verify", &json!({ "backup_id": id }));
        assert_eq!(verified["result"]["state"], "CHECKED", "{verified}");
        let tested = self.call("backup.restore_test", &json!({ "backup_id": id }));
        assert_eq!(tested["result"]["state"], "RETAINED", "{tested}");
        id
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::take(&self.data)
    }
}

/// Content-addressed path of `bytes` under an attachment store (`ab/cd/<sha256>`).
fn store_path(prefix: &str, bytes: &str) -> String {
    let s = sha(bytes);
    format!("{prefix}/{}/{}/{s}", &s[0..2], &s[2..4])
}

/// Whether the database in `file` (inspected through a copy, so the file is not touched)
/// has a non-erased attachment row stored at `storage_ref`.
fn references_attachment(file: &Path, storage_digest: &str) -> bool {
    let scratch = tempfile::tempdir().unwrap();
    let copy = scratch.path().join("inspect.sqlite");
    std::fs::copy(file, &copy).unwrap();
    let db = Database::open(&copy, OpenOptions::new("cli-test").no_auto_migrate()).unwrap();
    let n: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM attachments WHERE erased = 0 AND (storage_ref LIKE '%' || ?1 || '%' OR content_sha256 = ?1)",
            [storage_digest],
            |r| r.get(0),
        )
        .unwrap();
    n > 0
}

#[test]
fn the_search_finds_every_needle_it_is_given() {
    // A planted file outside any data directory: each needle, alone, between binary bytes.
    let dir = tempfile::tempdir().unwrap();
    for (label, needle) in erased_needles() {
        let mut bytes = vec![0u8, 0xff, 0xfe, 0x80];
        bytes.extend_from_slice(needle.as_bytes());
        bytes.extend_from_slice(&[0xc3, 0x28, 0]);
        std::fs::write(dir.path().join("planted.bin"), &bytes).unwrap();
        let found = Snapshot::take(dir.path()).hits(&needle);
        assert_eq!(
            found,
            BTreeSet::from(["planted.bin".to_string()]),
            "{label}"
        );
    }
}

#[test]
fn an_approved_erasure_leaves_no_trace_outside_the_snapshot_taken_before_it() {
    let env = Env::new();
    let (code, init, _) = env.run(&["init"]);
    assert_eq!(code, 0, "{init}");

    // A note that stays and a note that is erased, each with a file.
    let keep = env.note(KEEP_TITLE, KEEP_BODY);
    env.attach(&keep, "priv3-keep.txt", KEEP_FILE);
    let note = env.note(TITLE, &body());
    let attached = env.attach(&note, FILENAME, FILE_BYTES);
    assert_eq!(attached["result"]["sha256"], json!(sha(FILE_BYTES)));

    // S1: before the erasure.
    let s1 = env.retained_backup("before the erasure");
    let s1_snapshot = format!("backups/{s1}.sqlite");
    let s1_copy = store_path("backups/attachments", FILE_BYTES);

    // ---- positive controls: the search finds what is stored before the erasure ----
    let before = env.snapshot();
    let needles = erased_needles();
    for (label, needle) in &needles {
        let hits = before.hits(needle);
        if !STORED_BEFORE.contains(&label.as_str()) {
            assert!(
                hits.is_empty(),
                "{label} is never stored, but was found in {hits:?}"
            );
            continue;
        }
        if label == "file bytes" {
            assert!(
                hits.contains(&store_path("attachments", FILE_BYTES)) && hits.contains(&s1_copy),
                "positive control: {label} in the store and its backup copy, found in {hits:?}"
            );
        } else {
            assert!(
                hits.iter().any(|p| is_live_database(p)) && hits.contains(&s1_snapshot),
                "positive control: {label} in the live database and in S1, found in {hits:?}"
            );
        }
    }

    // ---- the approved erasure ----
    let erased = env.approved(
        "deletion.request",
        &json!({ "resource": { "kind": "NOTE", "id": note }, "reason": REASON }),
    );
    assert_eq!(erased["result"]["state"], "APPLIED", "{erased}");
    assert!(
        erased["warnings"].as_array().is_none_or(Vec::is_empty),
        "{erased}"
    );

    // S2: after the erasure, retained. Then an approved export, then an offline restore of S2
    // over the live database (the old one goes to quarantine/, the restore writes
    // restore-log.jsonl).
    let s2 = env.retained_backup("after the erasure");
    let export = env.approved(
        "export.request",
        &json!({ "domains": ["LEARNING", "SYSTEM"], "format": "JSON", "include_private": true }),
    );
    assert_eq!(export["result"]["state"], "READY", "{export}");
    let (code, restored, err) = env.run(&[
        "restore",
        "--backup-id",
        &s2,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{restored} {err}");
    assert_eq!(restored["restored"], true);

    // ---- the probe ----
    let after = env.snapshot();
    let all = after.paths();
    for expected in [
        "g9.sqlite",
        "restore-log.jsonl",
        "logs/g9.log",
        s1_snapshot.as_str(),
        &format!("backups/{s2}.sqlite"),
    ] {
        assert!(all.contains(expected), "{expected} missing from {all:#?}");
    }
    for prefix in ["exports/", "quarantine/"] {
        assert!(
            all.iter().any(|p| p.starts_with(prefix)),
            "no file under {prefix}: {all:#?}"
        );
    }
    // Positive controls after the erasure: every place searched holds live content.
    for (what, needle) in [("kept title", KEEP_TITLE), ("kept body", KEEP_BODY)] {
        let hits = after.hits(needle);
        assert!(
            hits.contains("g9.sqlite")
                && hits.contains(&format!("backups/{s2}.sqlite"))
                && hits.iter().any(|p| p.starts_with("exports/"))
                && hits.iter().any(|p| p.starts_with("quarantine/")),
            "positive control: {what} in the live database, S2, the export and the quarantine, found in {hits:?}"
        );
    }
    let kept_file = after.hits(KEEP_FILE.trim_end());
    assert!(
        kept_file.contains(&store_path("attachments", KEEP_FILE))
            && kept_file.contains(&store_path("backups/attachments", KEEP_FILE))
            && kept_file.iter().any(|p| p.starts_with("exports/")),
        "positive control: the kept file in the store, its backup copy and the export, found in {kept_file:?}"
    );
    assert!(
        after
            .hits(&sha(KEEP_FILE))
            .contains(&format!("backups/{s2}.sqlite")),
        "positive control: a stored digest is found in S2"
    );
    assert!(
        !after.hits("deletion.request").is_empty()
            && after.hits("deletion.request").contains("logs/g9.log"),
        "positive control: the operations log is written and searched"
    );
    assert!(after.hits(&s2).contains("restore-log.jsonl"));

    // The erased content: only in what S1 holds.
    let belongs_to_s1 = |p: &str| p == s1_snapshot || p == s1_copy;
    let mut residue = Vec::new();
    for (label, needle) in &needles {
        for path in after.hits(needle) {
            if !belongs_to_s1(&path) {
                residue.push(format!("{label} in {path}"));
            }
        }
    }
    assert!(
        residue.is_empty(),
        "erased content outside the pre-erasure snapshot S1 {s1}: {residue:#?}"
    );

    // What remains is S1 and the attachment copy only S1 references, listed by id.
    let remaining: BTreeSet<String> = needles
        .iter()
        .flat_map(|(_, needle)| after.hits(needle))
        .collect();
    assert_eq!(
        remaining,
        BTreeSet::from([s1_snapshot.clone(), s1_copy.clone()]),
        "the pre-erasure snapshot S1 {s1} ({s1_snapshot}) and its attachment copy ({s1_copy}) hold the erased content by design: they were taken before the erasure and leave with backup retention"
    );
    // The list the plan asks for (PRIV-3: "snapshots taken before the erasure are
    // listed by id"), printed on success: `cargo test --test erasure_probe -- --nocapture`.
    eprintln!(
        "PRIV-3: erased content remains only in the pre-erasure backup {s1}: {s1_snapshot} and {s1_copy}"
    );
    let digest = sha(FILE_BYTES);
    assert!(references_attachment(&env.data.join(&s1_snapshot), &digest));
    assert!(!references_attachment(
        &env.data.join(format!("backups/{s2}.sqlite")),
        &digest
    ));
    assert!(!references_attachment(&env.data.join("g9.sqlite"), &digest));

    // The deletion reason is a governance record (`deletion_requests.reason`, SYSTEM domain):
    // kept in the database and the snapshots taken after the request, never exported, never
    // logged, absent from S1 (which predates it).
    let reason = after.hits(REASON);
    assert!(reason.contains("g9.sqlite"), "{reason:?}");
    for path in &reason {
        assert!(
            is_live_database(path)
                || path == &format!("backups/{s2}.sqlite")
                || path.starts_with("quarantine/"),
            "the deletion reason reached {path}"
        );
    }
}

/// The desktop application keeps one connection open for hours: the erasure must not leave the
/// erased pages in the write-ahead log of the running process (SEC-09: the erasure checkpoints
/// with TRUNCATE after its commit).
#[test]
fn the_live_wal_holds_no_erased_content_while_the_application_still_runs() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let mut boot = BootOptions::new(data.clone());
    boot.skills_dir = None;
    let booted = open(&boot).unwrap();
    let mut client = Client::new(booted.ctx);

    let note = client
        .call(
            "notes.create",
            json!({ "title": TITLE, "body_markdown": body(), "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string();
    client
        .call(
            "evidence.attach_file",
            json!({ "note_id": note, "filename": FILENAME, "media_type": "text/plain", "content_base64": base64(FILE_BYTES.as_bytes()) }),
        )
        .unwrap();

    // Positive control, connection open: the note is in the write-ahead log.
    let before = Snapshot::take(&data);
    for needle in [TITLE, SECRET_IN_CONTEXT, FILENAME] {
        let hits = before.hits(needle);
        assert!(
            hits.contains("g9.sqlite-wal"),
            "positive control: {needle} in the live WAL, found in {hits:?}"
        );
    }

    let (_, erased) = client
        .run_approved(
            "deletion.request",
            json!({ "resource": { "kind": "NOTE", "id": note }, "reason": REASON }),
            &MetaOptions::default(),
        )
        .unwrap();
    assert_eq!(erased["result"]["state"], "APPLIED", "{erased}");

    // Still open: nothing of the note in any file.
    let after = Snapshot::take(&data);
    assert!(
        after.paths().contains("g9.sqlite-wal"),
        "{:?}",
        after.paths()
    );
    let mut residue = Vec::new();
    for (label, needle) in erased_needles() {
        for path in after.hits(&needle) {
            residue.push(format!("{label} in {path}"));
        }
    }
    assert!(
        residue.is_empty(),
        "erased content while the process still runs: {residue:#?}"
    );
    drop(client);
}
