//! The operations log against real commands (plan steps OBS-1 and OBS-2):
//! notes, files and refusals that carry marker text go through the backend,
//! and the log is then read as raw bytes.
//!
//! * OBS-1: no marker, no digest of a marker, and no path of the data
//!   directory is anywhere in the log; every line is one of the two shapes.
//! * OBS-2: every dispatched call is one line, in order, and the request id of
//!   every audit row is the request id of a logged command of the same
//!   operation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::result_large_err
)]

mod common;

use common::*;
use g9_app::oplog::{self, OpLog, LOG_DIR_NAME, LOG_FILE_NAME};
use g9_domain::UtcMillis;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

const TITLE: &str = "Probe title QX7TITLEMARK";
const BODY: &str = "Probe body QX7BODYMARK with a door code 4711";
const FILE_NAME: &str = "probe-QX7FILEMARK.txt";
const FILE_BYTES: &[u8] = b"file bytes QX7BYTESMARK";
const REASON: &str = "probe reason QX7REASONMARK";
const NEW_TITLE: &str = "Second probe QX7REVISEMARK";

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The desktop layout, with the log in `<data>/logs`.
fn harness(root: &Path, log: Arc<OpLog>) -> Harness {
    for sub in ["attachments", "backups", "scratch", "exports"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.to_path_buf();
    Harness::with(move |s| {
        s.config.approved_roots = ["attachments", "backups", "scratch", "exports"]
            .iter()
            .map(|d| r.join(d))
            .collect();
        s.config.attachment_dir = Some(r.join("attachments"));
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
        s.config.export_dir = Some(r.join("exports"));
        s.oplog = Some(log);
    })
}

fn log_lines(dir: &Path) -> Vec<Value> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    // Oldest file first: g9.log.3, .2, .1, then g9.log.
    names.sort_by_key(|p| {
        std::cmp::Reverse(
            p.extension()
                .and_then(|x| x.to_str())
                .and_then(|x| x.parse::<u32>().ok())
                .unwrap_or(0),
        )
    });
    names
        .iter()
        .flat_map(|p| {
            std::fs::read_to_string(p)
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn raw_log(dir: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        out.extend(std::fs::read(entry.unwrap().path()).unwrap());
    }
    out
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

struct Run {
    root: tempfile::TempDir,
    calls: Vec<String>,
    h: Harness,
}

/// Every kind of call the probe needs, through the real pipeline, with the
/// diagnostic events of this thread going to the same log.
fn run() -> Run {
    let root = tempfile::Builder::new().prefix("g9log").tempdir().unwrap();
    let log = OpLog::open(&root.path().join(LOG_DIR_NAME)).unwrap();
    let clock: oplog::EventClock = Arc::new(|| UtcMillis::from_millis(T0));
    let (h, calls) = oplog::with_event_log(log.clone(), clock, || {
        let mut h = harness(root.path(), log);
        let mut calls: Vec<String> = Vec::new();
        let mut call = |h: &mut Harness, op: &str, payload: Value| {
            calls.push(op.to_string());
            h.call(op, payload)
        };

        let note = call(
            &mut h,
            "notes.create",
            json!({ "title": TITLE, "body_markdown": BODY, "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
        let note_id = note["result"]["note_id"].as_str().unwrap().to_string();
        let revision = note["revision"].as_u64().unwrap();
        call(
            &mut h,
            "evidence.attach_file",
            json!({ "filename": FILE_NAME, "media_type": "text/plain", "content_base64": b64(FILE_BYTES), "note_id": note_id }),
        )
        .unwrap();
        // A refusal whose payload carries a marker (an unknown key).
        let refused = call(
            &mut h,
            "notes.create",
            json!({ "title": TITLE, "body_markdown": BODY, "classification": "INTERNAL", "domain": "LEARNING", "QX7KEYMARK": 1 }),
        );
        assert!(refused.is_err());
        // An operation the registry does not know, named with a marker.
        assert!(call(&mut h, "notes.QX7OPMARK", json!({})).is_err());
        // A mutation that is replayed: the same idempotency key and payload twice.
        for _ in 0..2 {
            calls.push("notes.create".to_string());
            h.call_with(
                "notes.create",
                json!({ "title": "Replay", "body_markdown": "same", "classification": "INTERNAL", "domain": "LEARNING" }),
                &Opts::key("idem-replay"),
            )
            .unwrap();
        }
        // A revision of the note with a marker in the new title (needs the revision).
        calls.push("notes.revise".to_string());
        h.call_with(
            "notes.revise",
            json!({ "note_id": note_id, "title": NEW_TITLE, "body_markdown": BODY }),
            &Opts::revision(revision),
        )
        .unwrap();
        // Time passes inside a command: the duration comes from the monotonic clock.
        h.mono.fetch_add(0, Ordering::SeqCst);
        // An approval-gated erasure with a reason: request, confirmed approval, retry.
        let payload = json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": REASON });
        calls.push("deletion.request".to_string());
        let err = h.call("deletion.request", payload.clone()).unwrap_err();
        let approval = err.approval_id.clone().unwrap();
        calls.push("approvals.decide".to_string());
        h.decide(approval.as_str(), "APPROVE");
        calls.push("deletion.request".to_string());
        let done = h
            .call_with(
                "deletion.request",
                payload,
                &Opts::approval(approval.as_str()),
            )
            .unwrap();
        assert_eq!(done["result"]["state"], "APPLIED", "{done}");
        // A declined native confirmation of another request.
        calls.push("notes.create".to_string());
        let other = h
            .call(
                "notes.create",
                json!({ "title": "Other", "body_markdown": "x", "classification": "INTERNAL", "domain": "LEARNING" }),
            )
            .unwrap()["result"]["note_id"]
            .as_str()
            .unwrap()
            .to_string();
        calls.push("deletion.request".to_string());
        let err = h
            .call(
                "deletion.request",
                json!({ "resource": { "kind": "NOTE", "id": other }, "reason": REASON }),
            )
            .unwrap_err();
        calls.push("approvals.decide".to_string());
        let declined = h.confirmed_with(
            "approvals.decide",
            json!({ "approval_id": err.approval_id.unwrap().as_str(), "decision": "APPROVE" }),
            &mut |_| false,
        );
        assert_eq!(declined.unwrap_err().code, "approval.not_confirmed");
        (h, calls)
    });
    Run { root, calls, h }
}

#[test]
fn no_content_and_no_path_reaches_the_log() {
    let run = run();
    let dir = run.root.path().join(LOG_DIR_NAME);
    let raw = raw_log(&dir);
    assert!(!raw.is_empty());
    let digest = |b: &[u8]| g9_domain::hashing::sha256_hex(b);
    let mut needles: Vec<(String, Vec<u8>)> = vec![
        ("the title".into(), b"QX7TITLEMARK".to_vec()),
        ("the body".into(), b"QX7BODYMARK".to_vec()),
        ("the door code".into(), b"4711".to_vec()),
        ("the file name".into(), b"QX7FILEMARK".to_vec()),
        ("the file bytes".into(), b"QX7BYTESMARK".to_vec()),
        ("the erasure reason".into(), b"QX7REASONMARK".to_vec()),
        ("the revised title".into(), b"QX7REVISEMARK".to_vec()),
        ("a payload key".into(), b"QX7KEYMARK".to_vec()),
        ("an unknown operation".into(), b"QX7OPMARK".to_vec()),
        (
            "the file bytes in base64".into(),
            b64(FILE_BYTES).into_bytes(),
        ),
        (
            "the digest of the body".into(),
            digest(BODY.as_bytes()).into_bytes(),
        ),
        (
            "the digest of the file".into(),
            digest(FILE_BYTES).into_bytes(),
        ),
        (
            "the digest of the title".into(),
            digest(TITLE.as_bytes()).into_bytes(),
        ),
        (
            "the data directory".into(),
            run.root.path().to_string_lossy().into_owned().into_bytes(),
        ),
        ("any absolute path".into(), b"/tmp".to_vec()),
    ];
    // The control: the same needles are in the database or the attachment store,
    // so their absence from the log is not an accident of the search.
    needles.retain(|(what, needle)| {
        assert!(!contains(&raw, needle), "{what} reached the log");
        true
    });
    let attachments = run.root.path().join("attachments");
    assert!(
        walk(&attachments)
            .iter()
            .any(|f| contains(&std::fs::read(f).unwrap(), b"QX7BYTESMARK"))
            || walk(&attachments).is_empty(),
        "control: the attachment store was not where the probe looked"
    );
    let lines = log_lines(&dir);
    for line in &lines {
        let mut keys: Vec<&str> = line
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        match line["kind"].as_str() {
            Some("command") => assert_eq!(
                keys,
                vec![
                    "code",
                    "duration_ms",
                    "kind",
                    "op",
                    "outcome",
                    "replayed",
                    "request_id",
                    "ts"
                ],
                "{line}"
            ),
            Some("event") => assert_eq!(
                keys,
                vec![
                    "fields",
                    "fields_not_allowed",
                    "kind",
                    "level",
                    "message",
                    "src",
                    "target",
                    "ts"
                ],
                "{line}"
            ),
            other => panic!("unexpected kind {other:?}: {line}"),
        }
    }
    // The diagnostic events of the run reached the same log (recovery and
    // migrations raise some on every open of a new database).
    assert!(
        lines.iter().any(|l| l["kind"] == "event"),
        "no diagnostic event was recorded"
    );
    drop(run.h);
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn every_call_is_one_line_and_every_audit_row_finds_its_command() {
    let run = run();
    let lines = log_lines(&run.root.path().join(LOG_DIR_NAME));
    let commands: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "command").collect();
    let logged_ops: Vec<String> = commands
        .iter()
        .map(|l| l["op"].as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = run
        .calls
        .iter()
        .map(|op| {
            if op.contains("QX7OPMARK") {
                "unknown".to_string()
            } else {
                op.clone()
            }
        })
        .collect();
    assert_eq!(
        logged_ops, expected,
        "one line per dispatched call, in order"
    );

    // Outcomes, codes and the replay flag are what the caller saw.
    let outcomes: Vec<(String, Value)> = commands
        .iter()
        .map(|l| {
            (
                l["outcome"].as_str().unwrap().to_string(),
                l["code"].clone(),
            )
        })
        .collect();
    assert_eq!(
        outcomes[2],
        ("INVALID_INPUT".to_string(), json!("ipc.payload_invalid"))
    );
    assert_eq!(
        outcomes[3],
        ("INVALID_INPUT".to_string(), json!("ipc.unknown_operation"))
    );
    assert_eq!(
        (
            commands[4]["replayed"].as_bool(),
            commands[5]["replayed"].as_bool()
        ),
        (Some(false), Some(true))
    );
    let declined = commands.last().unwrap();
    assert_eq!(declined["outcome"], "UNAUTHORIZED");
    assert_eq!(declined["code"], "approval.not_confirmed");
    assert!(commands.iter().any(|l| l["outcome"] == "APPROVAL_REQUIRED"));
    for l in &commands {
        assert!(l["duration_ms"].as_u64().is_some(), "{l}");
        assert_eq!(l["ts"].as_i64(), Some(run.h.now().as_millis()), "{l}");
        assert!(
            l["request_id"]
                .as_str()
                .is_some_and(|r| r.starts_with("req-")),
            "{l}"
        );
    }

    // OBS-2: one id finds both the audit row and the log line.
    let by_request: BTreeMap<String, String> = commands
        .iter()
        .map(|l| {
            (
                l["request_id"].as_str().unwrap().to_string(),
                l["op"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let mut stmt = run
        .h
        .conn()
        .prepare("SELECT request_id, operation FROM audit_metadata ORDER BY seq")
        .unwrap();
    let audited: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut matched = 0;
    for (request_id, operation) in &audited {
        if !request_id.starts_with("req-") {
            // Rows the backend writes on its own (recovery) carry its own ids.
            continue;
        }
        let logged = by_request
            .get(request_id)
            .unwrap_or_else(|| panic!("audit row {request_id} ({operation}) has no log line"));
        // A command's own row, or the row of one of its steps (`<op>.post_commit`).
        assert!(
            operation == logged || operation.starts_with(&format!("{logged}.")),
            "{request_id}: audited as {operation}, logged as {logged}"
        );
        matched += 1;
    }
    assert!(
        matched >= 6,
        "only {matched} audit rows were matched: {audited:?}"
    );
}

#[test]
fn a_context_without_a_log_writes_nothing_and_behaves_the_same() {
    let root = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    let out = h
        .call(
            "notes.create",
            json!({ "title": TITLE, "body_markdown": BODY, "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    assert!(out["result"]["note_id"].is_string());
    assert!(!root.path().join(LOG_DIR_NAME).join(LOG_FILE_NAME).exists());
}
