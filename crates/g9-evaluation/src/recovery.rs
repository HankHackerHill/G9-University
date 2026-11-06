//! Recovery drill (gates A08 backup/restore/recovery and A03 timer discontinuity).
//!
//! Sequence (one file database in a disposable directory, WAL journal):
//!
//! | # | Step | Boundary |
//! |---|---|---|
//! | 1 | seed the week-3 learner history; start a focus session; leave an approval pending | `dispatch` + fixture repositories |
//! | 2 | `backup.create` → `backup.verify` → `backup.restore_test` | `dispatch` (Online Backup API, checks on the copy, restore into the scratch root) |
//! | 3 | verify the snapshot file independently | `g9_storage::backup::verify_snapshot` |
//! | 4 | crash: `BEGIN IMMEDIATE`, write a row, image the files on disk while the transaction is open, then drop the connection without `COMMIT`/`ROLLBACK` | `Database::begin_immediate` + file copy |
//! | 5 | reopen the crash image as a new process (monotonic clock restarts) | `AppContext::builder(..).open()` → `g9_app::recovery` |
//! | 6 | recovery: open interval closed `UNCERTAIN`, session `INTERRUPTED`, stale approval `EXPIRED`, audit chain verified | read back through `g9-storage` |
//! | 7 | the application keeps working; the chain still verifies | `dispatch` |
//!
//! The crash image is the main database file plus its `-wal` file copied
//! while the writer's transaction is still open — the bytes a power cut or a
//! killed process leaves behind. The uncommitted row must not survive it.

use crate::fixtures::{self, SeedReport};
use crate::harness::{CallOpts, Harness, HarnessOptions, MINUTE_MS};
use crate::probes;
use crate::report::ScenarioReport;
use crate::scenarios::check_step;
use g9_domain::{ApprovalState, DataDomain, G9Error, ResourceKind, ResourceRef};
use g9_storage::backup::{verify_snapshot, VerifyOptions};
use g9_storage::repos::sessions as sessions_repo;
use g9_storage::Database;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Session that is running (open interval) when the crash happens.
pub const RUNNING_SESSION: &str = "ses-w03-d15-2";
/// Session row written inside the transaction that never commits.
pub const UNCOMMITTED_SESSION: &str = "ses-uncommitted-at-crash";
/// File name of the live database inside the drill directory.
pub const DB_FILE: &str = "g9-mentor.sqlite";

fn io(code: &str, e: &std::io::Error) -> G9Error {
    G9Error::resource_unavailable(code, format!("host filesystem: {}", e.kind()))
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Copy the database file and its `-wal` (when present) into `dir`: the bytes
/// on disk at this instant. The `-shm` index is not copied; SQLite rebuilds it
/// from the WAL on the next open. Returns the image path and whether a WAL
/// file was part of the image.
pub fn crash_image(db_path: &Path, dir: &Path) -> Result<(PathBuf, bool), G9Error> {
    std::fs::create_dir_all(dir).map_err(|e| io("evaluation.crash_image", &e))?;
    let name = db_path
        .file_name()
        .ok_or_else(|| G9Error::internal_redacted("evaluation.crash_image"))?;
    let image = dir.join(name);
    std::fs::copy(db_path, &image).map_err(|e| io("evaluation.crash_image", &e))?;
    let wal = sidecar(db_path, "-wal");
    let has_wal = wal.is_file();
    if has_wal {
        std::fs::copy(&wal, sidecar(&image, "-wal"))
            .map_err(|e| io("evaluation.crash_image", &e))?;
    }
    Ok((image, has_wal))
}

/// Metadata of the committed learner data (compared before and after the crash).
fn committed_state(h: &Harness, seed: &SeedReport) -> Result<Value, G9Error> {
    let mut notes = BTreeMap::new();
    for (alias, id) in &seed.notes {
        notes.insert(alias.clone(), probes::note(h, id)?);
    }
    let mut attempts = BTreeMap::new();
    for (alias, id) in &seed.attempts {
        attempts.insert(alias.clone(), probes::attempt(h, id)?);
    }
    let mut sessions = BTreeMap::new();
    for id in seed.sessions.keys() {
        sessions.insert(id.clone(), probes::session(h, id)?);
    }
    Ok(json!({ "notes": notes, "attempts": attempts, "sessions": sessions }))
}

fn backup_view(out: &crate::harness::StepOutcome) -> Value {
    let v = out.value();
    json!({
        "state": v["state"],
        "integrity": v["integrity"],
        "restore_tested": !v["restore_tested_at"].is_null(),
    })
}

fn scratch_entries(h: &Harness) -> Result<usize, G9Error> {
    let dir = h
        .scratch_dir()
        .ok_or_else(|| G9Error::internal_redacted("evaluation.scratch_dir"))?;
    Ok(std::fs::read_dir(dir)
        .map_err(|e| io("evaluation.scratch", &e))?
        .count())
}

/// A08-01: the recovery drill (see the module docs for the sequence).
pub fn recovery_drill() -> Result<ScenarioReport, G9Error> {
    let mut r = ScenarioReport::new(
        "A08-01",
        "A03, A08",
        "populated database: backup → verify → restore test → crash mid-transaction → reopen → open session INTERRUPTED → audit chain verifies",
    );
    let work = tempfile::Builder::new()
        .prefix("g9-eval-a08-")
        .tempdir()
        .map_err(|e| io("evaluation.tempdir", &e))?;
    let live_dir = work.path().join("live");
    std::fs::create_dir_all(&live_dir).map_err(|e| io("evaluation.tempdir", &e))?;
    let db_path = live_dir.join(DB_FILE);
    let mut h = Harness::with_options(HarnessOptions {
        database_path: Some(db_path.clone()),
        ..HarnessOptions::with_host_dirs()
    })?;
    r.check_eq(
        "file database runs the WAL profile",
        "wal".to_string(),
        h.db()?.profile().journal_mode.to_ascii_lowercase(),
    );

    // 1. Populated database: week-3 history, a running focus session, a pending approval.
    let (_fx, seed) = fixtures::seeded_week03(&mut h)?;
    r.boundary_step(
        "harness:seed week03-learner-history",
        "SEEDED",
        "OK",
        None,
        json!({ "sessions": seed.sessions, "attempt_states": seed.attempt_states, "observations": seed.observations.len() }),
    );
    fixtures::insert_planned_session(&h, RUNNING_SESSION, 45, h.now(), DataDomain::Learning)?;
    h.call("session.prepare", json!({ "session_id": RUNNING_SESSION }))?;
    let running = |h: &Harness| probes::session(h, RUNNING_SESSION);
    let out = h.record(
        &mut r,
        "session.start",
        json!({ "session_id": RUNNING_SESSION }),
        &CallOpts::default(),
        running,
    );
    check_step(&mut r, "start the focus session", &out, "ALLOW", "OK", None);
    r.check_eq(
        "session RUNNING with one open interval",
        (
            json!("RUNNING"),
            json!([{ "continuity": "VERIFIED", "closed_by": null, "open": true }]),
        ),
        {
            let s = probes::session(&h, RUNNING_SESSION)?;
            (s["state"].clone(), s["intervals"].clone())
        },
    );
    h.advance(25 * MINUTE_MS);
    let before_crash = committed_state(&h, &seed)?;

    // 2. Backup → verify → restore test, through the application.
    let out = h.record(
        &mut r,
        "backup.create",
        json!({ "label": "a08-drill" }),
        &CallOpts::default(),
        |_| Ok(json!({})),
    );
    check_step(&mut r, "backup.create", &out, "ALLOW", "OK", None);
    r.check_eq(
        "snapshot created",
        json!("SNAPSHOT_CREATED"),
        out.value()["state"].clone(),
    );
    let backup_id = out.value()["backup_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let backup_file = h
        .backup_dir()
        .ok_or_else(|| G9Error::internal_redacted("evaluation.backup_dir"))?
        .join(format!("{backup_id}.sqlite"));
    let out = h.record(
        &mut r,
        "backup.verify",
        json!({ "backup_id": backup_id }),
        &CallOpts::default(),
        |_| Ok(json!({})),
    );
    check_step(&mut r, "backup.verify", &out, "ALLOW", "OK", None);
    r.check_eq(
        "verification on the copy passed",
        json!({ "state": "CHECKED", "integrity": "OK", "restore_tested": false }),
        backup_view(&out),
    );
    let out = h.record(
        &mut r,
        "backup.restore_test",
        json!({ "backup_id": backup_id }),
        &CallOpts::default(),
        |h| Ok(json!({ "scratch_entries": scratch_entries(h)? })),
    );
    check_step(&mut r, "backup.restore_test", &out, "ALLOW", "OK", None);
    r.check_eq(
        "restore into a disposable copy passed; backup retained",
        json!({ "state": "RETAINED", "integrity": "OK", "restore_tested": true }),
        backup_view(&out),
    );
    r.check_eq(
        "disposable restore copy removed from the scratch root",
        0,
        scratch_entries(&h)?,
    );

    // 3. Independent verification of the snapshot file.
    let report = verify_snapshot(&backup_file, &VerifyOptions::default()).map_err(G9Error::from)?;
    r.boundary_step(
        "storage:verify_snapshot (backup file)",
        if report.ok() { "ACCEPTED" } else { "REFUSED" },
        "OK",
        None,
        json!({
            "integrity_check_ok": report.integrity_check_ok,
            "foreign_key_check_ok": report.foreign_key_check_ok,
            "migration_checksums_ok": report.migration_checksums_ok,
            "audit_chain_ok": report.audit_chain_ok,
            "study_sessions": report.table_counts.get("study_sessions"),
            "notes": report.table_counts.get("notes"),
        }),
    );
    r.check_eq("snapshot file verifies", true, report.ok());
    r.check_eq(
        "snapshot holds the populated rows (4 sessions, 2 notes)",
        (Some(4), Some(2)),
        (
            report.table_counts.get("study_sessions").copied(),
            report.table_counts.get("notes").copied(),
        ),
    );

    // A request left pending across the restart (expiry default 15 minutes).
    let out = h.record(
        &mut r,
        "export.request",
        json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false }),
        &CallOpts::default(),
        probes::approval_counts,
    );
    check_step(
        &mut r,
        "export left awaiting approval",
        &out,
        "APPROVAL_REQUIRED",
        "APPROVAL_REQUIRED",
        Some("policy.approval_required"),
    );
    let stale_approval = out.approval_id().unwrap_or_default();
    let audit_before_crash = probes::audit(&h)?;
    r.check_eq(
        "audit chain verifies before the crash",
        json!(true),
        audit_before_crash["ok"].clone(),
    );
    let events_before_crash = probes::event_kinds(&h)?.len();

    // 4. Crash: a write transaction is open when the process dies.
    let mut db = h.take_database()?;
    let crash_dir = work.path().join("crash-image");
    let (image, wal_in_image) = {
        let tx = db.begin_immediate().map_err(G9Error::from)?;
        let row = fixtures::planned_session_row(
            UNCOMMITTED_SESSION,
            30,
            h.now(),
            DataDomain::Learning,
            h.now(),
        )?;
        sessions_repo::insert_session(&tx, &row)?;
        let visible_inside =
            sessions_repo::get_session(&tx, &probes::inspect(DataDomain::Learning), &row.id)?
                .is_some();
        let image = crash_image(&db_path, &crash_dir)?;
        r.boundary_step(
            "harness:crash (BEGIN IMMEDIATE + uncommitted insert; files imaged; connection dropped without COMMIT/ROLLBACK)",
            "CRASHED",
            "OK",
            None,
            json!({ "uncommitted_row_visible_inside_tx": visible_inside, "wal_in_image": image.1 }),
        );
        r.check_eq(
            "uncommitted row was written inside the open transaction",
            true,
            visible_inside,
        );
        // The process dies: no COMMIT and no ROLLBACK statement is issued.
        std::mem::forget(tx);
        image
    };
    drop(db);
    r.note(format!(
        "Crash image: main database file{} copied while the write transaction was open; the -shm index is rebuilt by SQLite on open.",
        if wal_in_image { " and -wal file" } else { "" }
    ));

    // 5. Reopen the crash image as a new process 20 minutes later.
    h.advance(20 * MINUTE_MS);
    let reopened = Database::open(&image, h.open_options()).map_err(G9Error::from)?;
    h.reopen(reopened)?;
    let recovery = h
        .recovery_report()
        .ok_or_else(|| G9Error::internal_redacted("evaluation.recovery_report"))?;
    r.boundary_step(
        "harness:reopen crash image (AppContext open runs g9_app::recovery)",
        "RECOVERED",
        "OK",
        None,
        serde_json::to_value(&recovery).unwrap_or(Value::Null),
    );

    // 6. What recovery did, read back from storage.
    r.check_eq(
        "recovery: no pending migration, nothing blocked",
        (Vec::<u32>::new(), None),
        (
            recovery.pending_migrations.clone(),
            recovery.migration_blocked_reason.clone(),
        ),
    );
    r.check_eq(
        "recovery: audit chain verified at open over every committed row",
        (true, audit_before_crash["rows"].clone()),
        (recovery.audit_chain_ok, json!(recovery.audit_rows)),
    );
    r.check_eq(
        "recovery: exactly the running session interrupted",
        vec![RUNNING_SESSION.to_string()],
        recovery
            .interrupted_sessions
            .iter()
            .map(|s| s.as_str().to_string())
            .collect::<Vec<_>>(),
    );
    r.check_eq(
        "recovery: one interval closed",
        1,
        recovery.closed_intervals.len(),
    );
    r.check_eq(
        "recovery: the stale approval expired",
        vec![stale_approval.clone()],
        recovery
            .expired_approvals
            .iter()
            .map(|a| a.as_str().to_string())
            .collect::<Vec<_>>(),
    );
    let session = probes::session(&h, RUNNING_SESSION)?;
    r.check_eq(
        "session INTERRUPTED; interval closed UNCERTAIN by recovery",
        (
            json!("INTERRUPTED"),
            json!([{ "continuity": "UNCERTAIN", "closed_by": "RECOVERY", "open": false }]),
        ),
        (session["state"].clone(), session["intervals"].clone()),
    );
    r.check_that(
        "uncertain time is never credited (credited 0, uncertain > 0)",
        "credited_ms == 0 and uncertain_ms > 0",
        json!({ "credited_ms": session["credited_ms"], "uncertain_ms": session["uncertain_ms"] }),
        session["credited_ms"] == json!(0)
            && session["uncertain_ms"].as_i64().is_some_and(|u| u > 0),
    );
    r.check_eq(
        "the uncommitted row did not survive the crash",
        json!({ "exists": false }),
        probes::session(&h, UNCOMMITTED_SESSION)?,
    );
    r.check_eq(
        "committed learner data identical after the crash",
        before_crash,
        committed_state(&h, &seed)?,
    );
    r.check_eq(
        "stale approval stored EXPIRED",
        Some(ApprovalState::Expired),
        probes::approval_state(&h, &stale_approval)?,
    );
    let audit_after = probes::audit(&h)?;
    r.check_eq(
        "audit chain verifies after recovery (+1 interrupt row, +1 expiry row)",
        json!({
            "ok": true,
            "rows": audit_before_crash["rows"].as_u64().unwrap_or(0) + 2,
            "first_bad_seq": null,
        }),
        audit_after,
    );
    let kinds = probes::event_kinds(&h)?;
    r.check_eq(
        "one session.interrupted event appended",
        vec!["session.interrupted".to_string()],
        kinds
            .get(events_before_crash..)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
    );
    r.check_eq(
        "recovery's policy decision persisted (system actor, ALLOW)",
        Some("ALLOW".to_string()),
        probes::latest_policy_decision(
            &h,
            &ResourceRef::new(ResourceKind::StudySession, RUNNING_SESSION),
        )?
        .map(|(k, _, _)| k),
    );

    // 7. The application keeps working after recovery.
    let out = h.record(
        &mut r,
        "system.status",
        json!({}),
        &CallOpts::default(),
        |_| Ok(json!({})),
    );
    check_step(
        &mut r,
        "status after recovery",
        &out,
        "NO_AUDIT_ROW",
        "OK",
        None,
    );
    r.check_eq(
        "storage READY after recovery",
        json!("READY"),
        out.value()["storage"]["state"].clone(),
    );
    let out = h.record(
        &mut r,
        "session.list",
        json!({}),
        &CallOpts::default(),
        |_| Ok(json!({})),
    );
    r.check_eq(
        "session.list reports the interrupted session",
        Some(json!("INTERRUPTED")),
        out.value()["sessions"].as_array().and_then(|a| {
            a.iter()
                .find(|s| s["session_id"] == json!(RUNNING_SESSION))
                .map(|s| s["state"].clone())
        }),
    );
    let out = h.record(
        &mut r,
        "notes.create",
        json!({ "title": "after recovery", "body_markdown": "synthetic note written after the crash", "classification": "PRIVATE", "domain": "LEARNING" }),
        &CallOpts::default(),
        probes::audit,
    );
    check_step(&mut r, "mutation after recovery", &out, "ALLOW", "OK", None);
    r.check_eq(
        "audit chain still verifies after new writes",
        json!(true),
        probes::audit(&h)?["ok"].clone(),
    );
    let out = h.record(
        &mut r,
        "backup.verify",
        json!({ "backup_id": backup_id }),
        &CallOpts::default(),
        |_| Ok(json!({})),
    );
    check_step(
        &mut r,
        "verify the pre-crash backup from the new process",
        &out,
        "NOT_EVALUATED",
        "PRECONDITION_FAILED",
        Some("backup.manifest_unavailable"),
    );
    r.note("Gap (g9-app/g9-storage): backup.verify and backup.restore_test only know backups created by the same process (no manifest reader); after the restart the pre-crash backup is verified at the storage boundary (verify_snapshot) above, not through IPC.");
    Ok(r.finish())
}
