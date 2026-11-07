#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Regression tests for confirmed review findings against `g9-app` (each test
//! fails on the pre-fix code and passes after the fix). Every test drives the
//! real `dispatch` entry point through the shared harness.

mod common;

use common::*;
use g9_ai_adapter::{ModelRef, ScriptedRuntime};
use g9_domain::{
    ApprovalState, AttachmentId, BackupState, Classification, DataDomain, ErrorCategory, IanaZone,
    InteractionMode, ModelId, ReviewItemId, Revision, SessionId, SessionState, SourceKind,
    TrustClass, UtcMillis,
};
use g9_storage::approvals_store;
use g9_storage::repos::evidence as evidence_repo;
use g9_storage::repos::retention as retention_repo;
use g9_storage::repos::sessions as sessions_repo;
use g9_storage::repos::ReadScope;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

const MIN: i64 = 60_000;

fn learning() -> ReadScope {
    ReadScope::domain(DataDomain::Learning)
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

fn has_warning(outcome: &Value, prefix: &str) -> bool {
    warnings(outcome).iter().any(|w| w.starts_with(prefix))
}

fn insert_session(h: &Harness, id: &str) -> SessionId {
    let sid = SessionId::parse(id).unwrap();
    sessions_repo::insert_session(
        h.conn(),
        &sessions_repo::StudySessionRow {
            id: sid.clone(),
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            state: SessionState::Planned,
            mode: InteractionMode::Study,
            zone: IanaZone::utc(),
            planned_start_utc: Some(h.now()),
            planned_end_utc: None,
            planned_minutes: 90,
            started_at: None,
            ended_at: None,
            credited_ms: g9_domain::DurationMs::ZERO,
            uncertain_ms: g9_domain::DurationMs::ZERO,
            data_domain: DataDomain::Learning,
            classification: Classification::Internal,
            revision: Revision::INITIAL,
            created_at: h.now(),
            updated_at: h.now(),
        },
    )
    .unwrap();
    sid
}

fn session(id: &str) -> Value {
    json!({ "session_id": id })
}

fn review_item(n: usize, due_at: UtcMillis) -> retention_repo::ReviewItemRow {
    retention_repo::ReviewItemRow {
        id: ReviewItemId::parse(&format!("ri-{n:05}")).unwrap(),
        kind: retention_repo::ReviewItemKind::Declarative,
        competency_id: None,
        objective_id: None,
        prompt: format!("evi-prompt-{n}"),
        answer_ref: None,
        scheduler: retention_repo::SchedulerKind::Baseline,
        due_at,
        last_success_at: None,
        stage: 0,
        interval_days: 1,
        fsrs_state: None,
        state: retention_repo::ReviewItemState::Active,
        data_domain: DataDomain::Learning,
        classification: Classification::Internal,
        revision: Revision::INITIAL,
        created_at: due_at,
        updated_at: due_at,
    }
}

/// A file database at schema version 9 (every later migration pending).
fn database_at_version_9(path: &std::path::Path) -> g9_storage::Database {
    let options = g9_storage::OpenOptions::new("0.0.9").applied_at(UtcMillis::from_millis(T0));
    let mut db = g9_storage::Database::open(path, options.clone().no_auto_migrate()).unwrap();
    g9_storage::Migrator::new("0.0.9", UtcMillis::from_millis(T0))
        .migrate_up_to(db.conn_mut(), 9)
        .unwrap();
    drop(db);
    let db = g9_storage::Database::open(path, options).unwrap();
    assert_eq!(db.pending_migrations().first(), Some(&10));
    db
}

/// Every `(table.column, rows)` whose text contains `needle` (whole database).
fn tables_containing(conn: &rusqlite::Connection, needle: &str) -> Vec<String> {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut hits = Vec::new();
    for table in tables {
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
        let names: Vec<String> = stmt.column_names().map(str::to_string).collect();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for (i, name) in names.iter().enumerate() {
                if let Ok(rusqlite::types::ValueRef::Text(t)) = row.get_ref(i) {
                    if String::from_utf8_lossy(t).contains(needle) {
                        hits.push(format!("{table}.{name}"));
                    }
                }
            }
        }
    }
    hits
}

// ---------------------------------------------------------------------------
// sessions.rs:493 — recover only when no uncertain interval is uncorrected
// ---------------------------------------------------------------------------

#[test]
fn correct_time_keeps_interrupted_while_another_uncertain_interval_is_uncorrected() {
    let mut h = Harness::new();
    let sid = insert_session(&h, "session-two-gaps");
    h.call("session.prepare", session("session-two-gaps"))
        .unwrap();
    h.call("session.start", session("session-two-gaps"))
        .unwrap();
    h.advance(10 * MIN);
    h.restart();
    let i1 = sessions_repo::list_intervals(h.conn(), &sid).unwrap()[0]
        .id
        .clone();
    let first = h
        .call(
            "session.correct_time",
            json!({ "session_id": "session-two-gaps", "interval_id": i1, "credited_ms": 5 * MIN, "reason": "first gap" }),
        )
        .unwrap();
    assert_eq!(first["result"]["state"], "PAUSED");

    h.call("session.resume", session("session-two-gaps"))
        .unwrap();
    h.advance(10 * MIN);
    h.restart();
    let intervals = sessions_repo::list_intervals(h.conn(), &sid).unwrap();
    let i2 = intervals[1].id.clone();
    assert_eq!(
        intervals[1].record.continuity,
        g9_calendar::Continuity::Uncertain
    );

    // Re-correcting the already corrected I1 must not recover the session
    // while I2 is still uncorrected.
    let again = h
        .call(
            "session.correct_time",
            json!({ "session_id": "session-two-gaps", "interval_id": i1, "credited_ms": 6 * MIN, "reason": "first gap, revised" }),
        )
        .unwrap();
    assert_eq!(again["result"]["state"], "INTERRUPTED");
    assert_eq!(again["result"]["requires_correction"], true);
    assert_eq!(again["result"]["uncertain_ms"], 10 * MIN);
    let w = warnings(&again);
    assert!(
        w.iter()
            .any(|x| x.starts_with("session.uncorrected_uncertain_time") && x.contains(&i2)),
        "{w:?}"
    );
    let stored = sessions_repo::get_session(h.conn(), &learning(), &sid)
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, SessionState::Interrupted);
    let refused = h
        .call("session.resume", session("session-two-gaps"))
        .unwrap_err();
    assert_eq!(refused.code, "session.illegal_transition");

    // Correcting the last uncertain interval recovers the session.
    let done = h
        .call(
            "session.correct_time",
            json!({ "session_id": "session-two-gaps", "interval_id": i2, "credited_ms": 7 * MIN, "reason": "second gap" }),
        )
        .unwrap();
    assert_eq!(done["result"]["state"], "PAUSED");
    assert_eq!(done["result"]["requires_correction"], false);
    assert_eq!(done["result"]["credited_ms"], 13 * MIN);
}

// ---------------------------------------------------------------------------
// context.rs:439 — dual-clock check at stop (TEC-07, partial)
// ---------------------------------------------------------------------------

#[test]
fn pause_after_a_suspend_with_stopped_monotonic_clock_is_uncertain_and_interrupts() {
    let mut h = Harness::new();
    let sid = insert_session(&h, "session-suspend");
    h.call("session.prepare", session("session-suspend"))
        .unwrap();
    h.call("session.start", session("session-suspend")).unwrap();
    h.advance(10 * MIN);
    // Suspend for 8 h: the wall clock moves, the monotonic clock stands still.
    h.clock.fetch_add(8 * 60 * MIN, Ordering::SeqCst);
    let paused = h.call("session.pause", session("session-suspend")).unwrap();
    assert_eq!(paused["result"]["state"], "INTERRUPTED");
    assert_eq!(
        paused["result"]["credited_ms"], 0,
        "never credited as VERIFIED"
    );
    assert_eq!(paused["result"]["uncertain_ms"], 8 * 60 * MIN + 10 * MIN);
    assert_eq!(paused["result"]["requires_correction"], true);
    assert!(
        has_warning(&paused, "session.clock_discontinuity"),
        "{paused}"
    );
    let intervals = sessions_repo::list_intervals(h.conn(), &sid).unwrap();
    assert_eq!(
        intervals[0].record.continuity,
        g9_calendar::Continuity::Uncertain
    );
    assert!(intervals[0].record.end_mono_ms.is_none());

    // The operator's correction recovers it; time is credited only then.
    let corrected = h
        .call(
            "session.correct_time",
            json!({ "session_id": "session-suspend", "interval_id": intervals[0].id, "credited_ms": 10 * MIN, "reason": "laptop slept; 10 minutes of work" }),
        )
        .unwrap();
    assert_eq!(corrected["result"]["state"], "PAUSED");
    assert_eq!(corrected["result"]["credited_ms"], 10 * MIN);

    // Control: equal wall and monotonic progress stays VERIFIED.
    h.call("session.resume", session("session-suspend"))
        .unwrap();
    h.advance(5 * MIN);
    let normal = h.call("session.pause", session("session-suspend")).unwrap();
    assert_eq!(normal["result"]["state"], "PAUSED");
    assert_eq!(normal["result"]["credited_ms"], 15 * MIN);
    assert!(!has_warning(&normal, "session.clock_discontinuity"));
}

// ---------------------------------------------------------------------------
// calendar.rs:394 — apply re-checks placements at use time
// ---------------------------------------------------------------------------

#[test]
fn calendar_apply_refuses_a_stale_proposal_that_overlaps_an_applied_event() {
    let mut h = Harness::new();
    for n in 0..5 {
        retention_repo::insert_review_item(
            h.conn(),
            &review_item(n, UtcMillis::from_millis(T0 - 1_000)),
        )
        .unwrap();
    }
    let range = json!({ "from": T0, "to": T0 + 86_400_000, "intent": "ADD_REVIEWS" });
    let a = h.call("calendar.propose", range.clone()).unwrap();
    let b = h.call("calendar.propose", range).unwrap();
    assert_eq!(a["result"]["feasible"], true);
    assert_eq!(b["result"]["feasible"], true);
    assert_eq!(
        a["result"]["changes"][0]["event"]["start"], b["result"]["changes"][0]["event"]["start"],
        "both proposals placed the same block"
    );
    let a_id = a["result"]["proposal_id"].as_str().unwrap().to_string();
    let b_id = b["result"]["proposal_id"].as_str().unwrap().to_string();

    // B's approval is granted while the calendar is still empty.
    let asked = h
        .call("calendar.apply", json!({ "proposal_id": b_id }))
        .unwrap_err();
    assert_eq!(asked.category, ErrorCategory::ApprovalRequired);
    let b_approval = asked.approval_id.unwrap();
    h.decide(b_approval.as_str(), "APPROVE");

    let applied = h.approved(
        "calendar.apply",
        json!({ "proposal_id": a_id }),
        &Opts::default(),
    );
    assert_eq!(applied["result"]["applied"], 1);

    // Use-time check: B now overlaps A's event and is refused; its approval is not spent.
    let stale = h
        .call_with(
            "calendar.apply",
            json!({ "proposal_id": b_id }),
            &Opts::approval(b_approval.as_str()),
        )
        .unwrap_err();
    assert_eq!(
        stale.category,
        ErrorCategory::PreconditionFailed,
        "{stale:?}"
    );
    assert_eq!(stale.code, "proposal.stale");
    assert_eq!(
        approvals_store::get_request(h.conn(), &b_approval)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Approved
    );
    let listed = h
        .call(
            "calendar.list",
            json!({ "from": T0, "to": T0 + 86_400_000 }),
        )
        .unwrap();
    assert_eq!(listed["result"]["events"].as_array().unwrap().len(), 1);
    // A stale proposal never yields a new approval request either.
    let pending_before = approvals_store::list_by_state(h.conn(), ApprovalState::Pending)
        .unwrap()
        .len();
    let again = h
        .call("calendar.apply", json!({ "proposal_id": b_id }))
        .unwrap_err();
    assert_eq!(again.code, "proposal.stale");
    assert_eq!(
        approvals_store::list_by_state(h.conn(), ApprovalState::Pending)
            .unwrap()
            .len(),
        pending_before
    );
}

// ---------------------------------------------------------------------------
// retention.rs:84 and :70 — start of day, no silent truncation
// ---------------------------------------------------------------------------

#[test]
fn retention_due_start_of_day_is_local_midnight_not_the_current_second() {
    let mut h = Harness::new();
    // T0 is 2026-11-22T08:30:00Z; the zone is UTC.
    let midnight = T0 - (8 * 60 + 30) * MIN;
    retention_repo::insert_review_item(
        h.conn(),
        &review_item(1, UtcMillis::from_millis(midnight + 10_000)),
    )
    .unwrap();
    let out = h
        .call("retention.due", json!({ "as_of": T0 + 45_000 }))
        .unwrap();
    assert_eq!(out["result"]["due"].as_array().unwrap().len(), 1, "{out}");
    assert_eq!(out["result"]["overdue"].as_array().unwrap().len(), 0);
}

#[test]
fn retention_due_reports_every_due_item_beyond_one_thousand() {
    let mut h = Harness::new();
    let total = 1_005;
    for n in 0..total {
        let due = T0 - 1_000 - i64::try_from(total - n).unwrap();
        retention_repo::insert_review_item(h.conn(), &review_item(n, UtcMillis::from_millis(due)))
            .unwrap();
    }
    let out = h.call("retention.due", json!({})).unwrap();
    let count = |k: &str| out["result"][k].as_array().unwrap().len();
    assert_eq!(count("due") + count("overdue") + count("overflow"), total);
    let listed: Vec<&str> = ["due", "overdue", "overflow"]
        .iter()
        .flat_map(|k| out["result"][*k].as_array().unwrap())
        .map(|v| v["review_item_id"].as_str().unwrap())
        .collect();
    assert!(
        listed.contains(&"ri-01004"),
        "the latest-due item is reported"
    );
}

#[test]
fn retention_due_flags_procedural_items_shown_with_the_transfer_label() {
    let mut h = Harness::new();
    let mut row = review_item(7, UtcMillis::from_millis(T0 - 1_000));
    row.kind = retention_repo::ReviewItemKind::Procedural;
    retention_repo::insert_review_item(h.conn(), &row).unwrap();
    let out = h.call("retention.due", json!({})).unwrap();
    assert!(has_warning(&out, "retention.due.procedural_label"), "{out}");
}

// ---------------------------------------------------------------------------
// backup.rs:259 — in-memory backup state follows the committed database
// ---------------------------------------------------------------------------

#[test]
fn backup_rollback_keeps_the_in_memory_state_equal_to_the_database() {
    let mut h = Harness::with_dirs();
    let created = h.call("backup.create", json!({})).unwrap();
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let bid = g9_domain::BackupId::parse(&id).unwrap();
    // Inject a failure in pipeline step 7 (after the handler wrote the manifest).
    h.conn()
        .execute_batch(
            "CREATE TEMP TRIGGER inject_step7 BEFORE INSERT ON domain_events
             WHEN NEW.kind = 'backup.verified' BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )
        .unwrap();
    assert!(h.call("backup.verify", json!({ "backup_id": id })).is_err());
    assert_eq!(
        g9_storage::backup::manifest_state(h.conn(), &bid).unwrap(),
        Some(BackupState::SnapshotCreated)
    );
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"]["state"], "SNAPSHOT_CREATED");

    h.conn()
        .execute_batch("DROP TRIGGER inject_step7;")
        .unwrap();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
    assert_eq!(verified["result"]["revision"], 2);
}

// ---------------------------------------------------------------------------
// backup.rs:220 — the attachment manifest is checked, NotChecked never passes
// ---------------------------------------------------------------------------

fn insert_attachment(h: &Harness, storage_ref: &str) {
    evidence_repo::insert_attachment(
        h.conn(),
        &evidence_repo::AttachmentRow {
            id: AttachmentId::parse("att-1").unwrap(),
            note_id: None,
            original_filename: "lab.pcap".into(),
            media_type: "application/octet-stream".into(),
            size_bytes: 4,
            content_sha256: g9_domain::hashing::sha256_hex(b"pcap"),
            storage_ref: storage_ref.into(),
            source_kind: SourceKind::OperatorInput,
            trust: TrustClass::Operator,
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            erased: false,
            erased_at: None,
            erasure_request_id: None,
            revision: Revision::INITIAL,
            created_at: h.now(),
            updated_at: h.now(),
        },
    )
    .unwrap();
}

#[test]
fn backup_with_attachment_rows_is_never_checked_without_the_attachment_check() {
    // No attachment directory configured: the rows cannot be checked.
    let mut h = Harness::with_dirs();
    insert_attachment(&h, "ab/cd/missing");
    let created = h.call("backup.create", json!({})).unwrap();
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "FAILED");
    assert_eq!(verified["result"]["integrity"], "FAILED");

    // Configured attachment directory, file missing: FAILED.
    let dir = tempfile::Builder::new().prefix("g9att").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["backups", "scratch", "attachments/ab/cd"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
        s.config.attachment_dir = Some(r.join("attachments"));
    });
    insert_attachment(&h, "ab/cd/blob");
    let created = h.call("backup.create", json!({})).unwrap();
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "FAILED");

    // File present: CHECKED, then RETAINED.
    std::fs::write(root.join("attachments/ab/cd/blob"), b"pcap").unwrap();
    let created = h.call("backup.create", json!({})).unwrap();
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
    assert_eq!(verified["result"]["integrity"], "OK");
    let tested = h
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED");
}

// ---------------------------------------------------------------------------
// recovery.rs:160 — no migration without the mandatory dry run
// ---------------------------------------------------------------------------

#[test]
fn recovery_blocks_migration_of_a_file_database_without_a_scratch_directory() {
    let dir = tempfile::Builder::new().prefix("g9mig").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("backups")).unwrap();
    let path = root.join("live.sqlite");
    let r = root.clone();
    let mut h = Harness::with_database(database_at_version_9(&path), |s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        // scratch_dir deliberately unset (AppConfig::new default)
    })
    .unwrap();
    let report = h.ctx().recovery_report().clone();
    assert!(report.migrated.is_empty(), "{report:?}");
    assert_eq!(report.pending_migrations.first(), Some(&10));
    assert!(report
        .migration_blocked_reason
        .as_deref()
        .unwrap_or_default()
        .contains("scratch"));
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["storage"]["state"], "READ_ONLY");

    // A scratch directory outside the approved roots is refused too.
    let outside = tempfile::Builder::new().prefix("g9out").tempdir().unwrap();
    let o = outside.path().to_path_buf();
    drop(h);
    let r = root.clone();
    let mut h = Harness::with_database(
        g9_storage::Database::open(
            &path,
            g9_storage::OpenOptions::new("0.0.9").applied_at(UtcMillis::from_millis(T0)),
        )
        .unwrap(),
        |s| {
            s.config.approved_roots = vec![r.clone()];
            s.config.backup_dir = Some(r.join("backups"));
            s.config.scratch_dir = Some(o.clone());
        },
    )
    .unwrap();
    let report = h.ctx().recovery_report().clone();
    assert!(report.migrated.is_empty(), "{report:?}");
    assert!(std::fs::read_dir(&o).unwrap().next().is_none());
}

// ---------------------------------------------------------------------------
// pipeline.rs:362 — READ_ONLY never points to a restore that does not exist
// ---------------------------------------------------------------------------

#[test]
fn read_only_refusal_does_not_direct_the_operator_to_a_missing_restore() {
    let dir = tempfile::Builder::new().prefix("g9ro").tempdir().unwrap();
    let path = dir.path().join("blocked.sqlite");
    let mut h = Harness::with_database(database_at_version_9(&path), |_| {}).unwrap();
    let refused = h
        .call(
            "notes.create",
            json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
        )
        .unwrap_err();
    assert_eq!(refused.code, "recovery.integrity_failed");
    assert!(!refused.message.contains("until the operator restores"));
    assert!(refused.message.contains("not implemented"));
    let details = refused.details.unwrap();
    assert_eq!(details["live_restore"], "NOT_IMPLEMENTED");
    assert_eq!(details["reason"], "pending_migrations");
}

// ---------------------------------------------------------------------------
// status.rs:58 — last_backup never silently reports "no backup"
// ---------------------------------------------------------------------------

#[test]
fn status_warns_when_backups_from_an_earlier_process_are_not_read() {
    let mut h = Harness::with_dirs();
    let fresh = h.call("system.status", json!({})).unwrap();
    assert!(!has_warning(
        &fresh,
        "system.status.last_backup_process_scoped"
    ));
    let created = h.call("backup.create", json!({})).unwrap();
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    h.call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    h.restart();
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"], Value::Null);
    assert!(
        has_warning(&status, "system.status.last_backup_process_scoped: 1 "),
        "{status}"
    );
}

// ---------------------------------------------------------------------------
// pipeline.rs:389 — allowed reads persist their policy decision
// ---------------------------------------------------------------------------

#[test]
fn allowed_reads_persist_a_policy_decision_row() {
    let mut h = Harness::new();
    let count = |h: &Harness| -> i64 {
        h.conn()
            .query_row(
                "SELECT COUNT(*) FROM policy_decisions WHERE decision = 'ALLOW' AND action = 'READ'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let before = count(&h);
    h.call("session.list", json!({})).unwrap();
    h.call("competency.projection", json!({})).ok();
    h.call("retention.due", json!({})).unwrap();
    assert!(count(&h) >= before + 2, "{} -> {}", before, count(&h));
}

// ---------------------------------------------------------------------------
// pipeline.rs:797 — erased note content leaves no copy in idempotency_records
// ---------------------------------------------------------------------------

#[test]
fn a10_erasure_leaves_no_copy_in_any_table_and_replay_returns_no_erased_content() {
    let mut h = Harness::new();
    let body = "PLUGH-sentinel-body private text";
    let title = "XYZZY-sentinel-title";
    let create = json!({ "title": title, "body_markdown": body, "classification": "PRIVATE", "domain": "LEARNING" });
    let created = h
        .call_with("notes.create", create.clone(), &Opts::key("note-key-1"))
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    assert_eq!(created["result"]["body_markdown"], body);
    let revised_body = "PLUGH-sentinel-revision";
    h.call_with(
        "notes.revise",
        json!({ "note_id": note_id, "body_markdown": revised_body }),
        &Opts::key("note-key-2"),
    )
    .unwrap();
    // Before erasure: an idempotent replay rebuilds the view from storage.
    let replay = h
        .call_with("notes.create", create.clone(), &Opts::key("note-key-1"))
        .unwrap();
    // The idempotency records never held the content.
    let idem: Vec<String> = tables_containing(h.conn(), "PLUGH-sentinel")
        .into_iter()
        .chain(tables_containing(h.conn(), "XYZZY-sentinel"))
        .filter(|t| t.starts_with("idempotency_records"))
        .collect();
    assert!(idem.is_empty(), "{idem:?}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["note_id"], json!(note_id));
    assert!(has_warning(&replay, "idempotency.replay_rebuilt"));

    let payload = json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "erase" });
    let done = h.approved("deletion.request", payload, &Opts::default());
    assert_eq!(done["result"]["state"], "APPLIED");
    for needle in ["PLUGH-sentinel", "XYZZY-sentinel"] {
        let hits = tables_containing(h.conn(), needle);
        assert!(hits.is_empty(), "{needle} still stored in {hits:?}");
    }
    let replay = h
        .call_with("notes.create", create, &Opts::key("note-key-1"))
        .unwrap();
    assert_eq!(replay["replayed"], true);
    let text = replay.to_string();
    assert!(!text.contains("PLUGH-sentinel"), "{text}");
    assert!(!text.contains("XYZZY-sentinel"), "{text}");
}

#[test]
fn skill_output_is_not_kept_for_idempotent_replay() {
    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    let out = valid_brief_output("SENTINEL-MODEL-RISK");
    runtime.push_response(runtime.response(out.to_string(), Some(out)));
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime.clone();
    let mut h = Harness::with(|s| {
        s.runtime = Some(rt);
        s.skills = Some(registry);
    });
    let payload = json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" } });
    let first = h
        .call_with("skills.invoke", payload.clone(), &Opts::key("skill-key-1"))
        .unwrap();
    assert!(first.to_string().contains("SENTINEL-MODEL-RISK"));
    assert!(tables_containing(h.conn(), "SENTINEL-MODEL-RISK").is_empty());
    let replay = h
        .call_with("skills.invoke", payload, &Opts::key("skill-key-1"))
        .unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["output"], Value::Null);
    assert!(has_warning(&replay, "idempotency.content_not_retained"));
    assert_eq!(runtime.remaining(), 0, "the replay did not call the model");
}

/// A schema-valid T01 output (no recommended actions) carrying `risk`.
pub fn valid_brief_output(risk: &str) -> Value {
    json!({
        "status": "COMPLETED",
        "result_schema_version": "1.0",
        "observations": [],
        "inferences": [],
        "evidence_refs": [],
        "confidence_limits": ["fixture"],
        "recommended_actions": [],
        "approval_requirements": [],
        "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 },
        "validation_results": [],
        "errors": [],
        "result": {
            "brief": { "day_id": "D15", "objectives": [], "due_reviews": [], "open_interventions": [], "risks": [risk] },
            "basis": { "evidence_refs": [], "unresolved_refs": [] }
        }
    })
}
