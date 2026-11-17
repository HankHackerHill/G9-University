#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! REL-5 (docs/release/path-to-10.md §5.16): a database written at every
//! earlier schema version, holding data, opens under this build the way the
//! application opens it (`Database::open` with `g9_app::database_options`,
//! then `AppContext::builder(..).open()`, whose recovery takes the verified
//! pre-migration backup, rehearses the migrations on a copy and migrates;
//! runbook "Migration"), with its data, its audit chain and a backup that
//! verifies and restores.
//!
//! The databases are stored files (`tests/upgrade_fixtures/schema-vNN.sqlite`,
//! written by `tests/upgrade_fixtures/generate.rs`): a stored file stands for
//! what an older build wrote, which a database regenerated from today's code
//! cannot. Every test works on a copy; the committed files are never opened.
//!
//! What the earlier tests covered (read before this file was written): the
//! storage gates migrate a version-5 database on a dry-run copy and a version-2
//! one on open (`crates/g9-storage/tests/gates.rs::a08_dry_run_migration_on_copy`),
//! a stepwise-migrated empty database verifies its schema
//! (`schema_integrity.rs`), and the application migrates an empty version-9
//! database after a verified backup (`gates.rs::recovery_migrates_only_after_a_verified_backup`,
//! `backup_persistence.rs`, `regressions.rs`) and a version-12 one holding
//! assessment rows (`grade_observations_verification.rs`). Versions 1 to 8,
//! 10, 11, 13 and 14 were never opened by the application, no upgrade started
//! from a stored file, and none carried attachment files.

mod common;

#[path = "upgrade_fixtures/generate.rs"]
mod generate;

use common::*;
use g9_domain::{BackupId, BackupState};
use g9_storage::backup::{verify_snapshot, AttachmentCheck, VerifyOptions};
use g9_storage::migrations::{table_counts, CURRENT_SCHEMA_VERSION};
use g9_storage::{AuditChain, Migrator};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every schema version a database can have been written at.
fn versions() -> std::ops::RangeInclusive<u32> {
    1..=CURRENT_SCHEMA_VERSION
}

fn sha_of(path: &Path) -> String {
    g9_storage::backup::sha256_of_file(path).unwrap()
}

/// Copy the committed fixture of `version` to `dest` (the only way any test
/// touches it).
fn copy_fixture(version: u32, dest: &Path) {
    let source = generate::fixture_dir().join(generate::file_name(version));
    assert!(
        source.is_file(),
        "no stored fixture for schema {version}: run the write_upgrade_fixtures example ({})",
        source.display()
    );
    std::fs::copy(&source, dest).unwrap();
}

/// What a closed database file holds, read from a scratch copy of it.
#[derive(Debug, Clone, PartialEq)]
struct Facts {
    version: u32,
    counts: BTreeMap<String, u64>,
    /// `(seq, id, row_hash)` of every audit row.
    audit: Vec<(i64, String, String)>,
}

fn facts_of(file: &Path, scratch: &Path) -> Facts {
    let copy = scratch.join("inspect.sqlite");
    let _ = std::fs::remove_file(&copy);
    std::fs::copy(file, &copy).unwrap();
    let facts = {
        let conn = Connection::open(&copy).unwrap();
        facts_of_conn(&conn)
    };
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", copy.display()));
    }
    facts
}

fn facts_of_conn(conn: &Connection) -> Facts {
    let mut stmt = conn
        .prepare("SELECT seq, id, row_hash FROM audit_metadata ORDER BY seq")
        .unwrap();
    let audit = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    Facts {
        version: Migrator::schema_version(conn).unwrap(),
        counts: table_counts(conn).unwrap(),
        audit,
    }
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn text(conn: &Connection, sql: &str) -> Option<String> {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn int(conn: &Connection, sql: &str) -> Option<i64> {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn there_is_a_stored_fixture_for_every_schema_version() {
    let scratch = tempfile::tempdir().unwrap();
    for version in versions() {
        let copy = scratch.path().join(generate::file_name(version));
        copy_fixture(version, &copy);
        let facts = facts_of(&copy, scratch.path());
        assert_eq!(facts.version, version, "schema-v{version:02}");
        // One audit row per version, plus the rows written at each version.
        assert_eq!(facts.audit.len(), version as usize, "schema-v{version:02}");
        let conn = Connection::open(&copy).unwrap();
        for k in 1..=version {
            for (table, column, key) in generate::rows_written_at(k) {
                let n: i64 = conn
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
                        [key],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(n, 1, "schema-v{version:02}: {table}.{key}");
            }
        }
        assert!(AuditChain::verify_chain(&conn).unwrap().ok);
    }
}

/// The data-directory layout of the CLI and the desktop bridge (`<data>` with
/// `attachments`, `backups`, `scratch`, `exports`, each an approved root).
struct DataDir {
    _dir: tempfile::TempDir,
    root: PathBuf,
    data: PathBuf,
}

impl DataDir {
    fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("g9rel5").tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let data = root.join("data");
        for sub in ["attachments", "backups", "scratch", "exports"] {
            std::fs::create_dir_all(data.join(sub)).unwrap();
        }
        Self {
            _dir: dir,
            root,
            data,
        }
    }

    fn live(&self) -> PathBuf {
        self.data.join("g9.sqlite")
    }

    fn configure(&self) -> impl FnOnce(&mut Setup) {
        let data = self.data.clone();
        move |s: &mut Setup| {
            s.config.approved_roots = ["attachments", "backups", "scratch", "exports"]
                .iter()
                .map(|d| data.join(d))
                .collect();
            s.config.attachment_dir = Some(data.join("attachments"));
            s.config.backup_dir = Some(data.join("backups"));
            s.config.scratch_dir = Some(data.join("scratch"));
            s.config.export_dir = Some(data.join("exports"));
        }
    }
}

#[test]
fn every_stored_fixture_opens_under_this_build_migrated_with_its_data_its_chain_and_a_verified_backup(
) {
    for version in versions() {
        upgrade(version);
    }
}

fn upgrade(version: u32) {
    let label = format!("schema-v{version:02}");
    let committed = generate::fixture_dir().join(generate::file_name(version));
    let committed_sha = sha_of(&committed);
    let dir = DataDir::new();
    copy_fixture(version, &dir.live());
    let before = facts_of(&dir.live(), &dir.root);
    assert_eq!(before.version, version, "{label}");
    if version >= 5 {
        // The attachment bytes live outside the database, in the live store.
        let file = dir
            .data
            .join("attachments")
            .join(generate::attachment_ref());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, generate::ATTACHMENT_BYTES).unwrap();
    }

    // Open exactly as the application does.
    let db = g9_storage::Database::open(
        dir.live(),
        g9_app::database_options("0.1.0-test").applied_at(g9_domain::UtcMillis::from_millis(T0)),
    )
    .unwrap();
    let pending: Vec<u32> = (version + 1..=CURRENT_SCHEMA_VERSION).collect();
    assert_eq!(db.pending_migrations(), pending.as_slice(), "{label}");
    // The migration events of this open (the migrator says which database it
    // migrated: the rehearsal copy or the live file; `crates/g9-storage/src/migrations.rs`).
    g9_app::oplog::keep_call_sites_interested();
    let events = tempfile::tempdir().unwrap();
    let log_dir = events.path().join("logs");
    let log = g9_app::oplog::OpLog::open(&log_dir).unwrap();
    let clock: g9_app::oplog::EventClock =
        std::sync::Arc::new(|| g9_domain::UtcMillis::from_millis(T0));
    let mut h = g9_app::oplog::with_event_log(log, clock, || {
        Harness::with_database(db, dir.configure()).unwrap()
    });
    // Runbook "Migration" step 2 is on the path: every pending migration runs on
    // the rehearsal copy first, then on the live database, in version order.
    let expected: Vec<(String, u32)> = pending
        .iter()
        .map(|v| ("rehearsal".to_string(), *v))
        .chain(pending.iter().map(|v| ("live".to_string(), *v)))
        .collect();
    assert_eq!(
        migration_events(&log_dir),
        expected,
        "{label}: the rehearsal on a copy must migrate first, then the live database"
    );
    let report = h.ctx().recovery_report().clone();
    assert_eq!(report.migration_blocked_reason, None, "{label}: {report:?}");
    assert!(report.pending_migrations.is_empty(), "{label}: {report:?}");
    assert_eq!(report.migrated, pending, "{label}");
    assert!(report.schema_differences.is_empty(), "{label}: {report:?}");
    assert!(report.audit_chain_ok, "{label}: {report:?}");
    assert_eq!(report.audit_rows, u64::from(version), "{label}");
    assert_eq!(
        Migrator::schema_version(h.conn()).unwrap(),
        CURRENT_SCHEMA_VERSION,
        "{label}"
    );

    // Data: every row written at any version up to this one, its values, and
    // every table's count (the migrations add rows only to schema_migrations,
    // recovery only the pre-migration manifest).
    for k in 1..=version {
        for (table, column, key) in generate::rows_written_at(k) {
            let n: i64 = h
                .conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
                    [key],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{label}: {table}.{key} (written at schema {k})");
        }
    }
    check_values(h.conn(), version, &label);
    let after = facts_of_conn(h.conn());
    let migrated = u64::from(version < CURRENT_SCHEMA_VERSION);
    for (table, count) in &after.counts {
        let delta = match table.as_str() {
            "schema_migrations" => u64::from(CURRENT_SCHEMA_VERSION - version),
            "backup_manifests" => migrated,
            _ => 0,
        };
        assert_eq!(
            *count,
            before.counts.get(table).copied().unwrap_or(0) + delta,
            "{label}: rows of {table}"
        );
    }
    assert_eq!(
        after.audit, before.audit,
        "{label}: the chain is the stored one"
    );

    // The verified pre-migration backup (runbook "Migration" step 1) and the
    // rehearsal on a copy (step 2), which leaves nothing behind.
    assert!(
        entries(&dir.data.join("scratch")).is_empty(),
        "{label}: {:?}",
        entries(&dir.data.join("scratch"))
    );
    if version < CURRENT_SCHEMA_VERSION {
        let id = report.pre_migration_backup.clone().unwrap();
        check_pre_migration_backup(&mut h, &dir, &id, version, &before, &label);
    } else {
        assert_eq!(report.pre_migration_backup, None, "{label}");
        assert!(
            entries(&dir.data.join("backups")).is_empty(),
            "{label}: nothing to migrate, no backup"
        );
    }

    // The application reads the old rows and writes new ones.
    if version >= 5 {
        let note = h
            .call("notes.get", json!({ "note_id": "fx-note-05" }))
            .unwrap();
        assert_eq!(
            note["result"]["body_markdown"],
            generate::NOTE_BODY_05,
            "{label}"
        );
    }
    h.call(
        "notes.create",
        json!({ "title": "after the upgrade", "body_markdown": "written by this build", "classification": "PRIVATE", "domain": "LEARNING" }),
    )
    .unwrap();
    let chain = AuditChain::verify_chain(h.conn()).unwrap();
    assert!(chain.ok, "{label}: {chain:?}");
    drop(h);
    assert_eq!(
        sha_of(&committed),
        committed_sha,
        "{label}: the committed fixture changed"
    );
}

/// `(what, version)` of every `migration applied` event in an operations log.
fn migration_events(log_dir: &Path) -> Vec<(String, u32)> {
    let text = std::fs::read_to_string(log_dir.join(g9_app::oplog::LOG_FILE_NAME)).unwrap();
    text.lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|e| e["kind"] == "event" && e["message"] == "migration applied")
        .map(|e| {
            (
                e["fields"]["what"].as_str().unwrap().to_string(),
                e["fields"]["version"].as_str().unwrap().parse().unwrap(),
            )
        })
        .collect()
}

fn check_pre_migration_backup(
    h: &mut Harness,
    dir: &DataDir,
    id: &BackupId,
    version: u32,
    before: &Facts,
    label: &str,
) {
    let stored = g9_storage::backup::read_manifest(h.conn(), id)
        .unwrap()
        .unwrap();
    let manifest = stored.manifest;
    assert_eq!(manifest.state, BackupState::Checked, "{label}");
    assert_eq!(manifest.schema_version, version, "{label}");
    let snapshot = dir
        .data
        .join("backups")
        .join(format!("{}.pre-migration.sqlite", id.as_str()));
    assert_eq!(
        manifest.path.as_deref(),
        Some(snapshot.as_path()),
        "{label}"
    );
    assert_eq!(manifest.sha256_of_file, sha_of(&snapshot), "{label}");
    // The snapshot is the database as it was before the migration.
    let snap = facts_of(&snapshot, &dir.root);
    assert_eq!(snap.version, version, "{label}");
    assert_eq!(snap.counts, before.counts, "{label}");
    assert_eq!(snap.audit, before.audit, "{label}");
    let head = before.audit.last().unwrap();
    let anchor = manifest.audit_anchor.clone().unwrap();
    assert_eq!(
        (anchor.seq, anchor.row_hash.as_str()),
        (head.0, head.2.as_str()),
        "{label}"
    );
    let report = manifest.verification.clone().unwrap();
    assert!(report.ok(), "{label}: {report:?}");
    let expected_attachments = u64::from(version >= 5);
    assert_eq!(
        report.attachment_manifest,
        AttachmentCheck::Checked {
            expected: expected_attachments,
            missing: Vec::new(),
            damaged: 0
        },
        "{label}"
    );
    if version >= 5 {
        // The bytes travelled with the backup (STOR-2) and verify there.
        let copy = dir
            .data
            .join("backups/attachments")
            .join(generate::attachment_ref());
        assert_eq!(
            std::fs::read(copy).unwrap(),
            generate::ATTACHMENT_BYTES,
            "{label}"
        );
    }
    let reverified = verify_snapshot(
        &snapshot,
        &VerifyOptions {
            attachment_root: Some(dir.data.join("backups/attachments")),
        },
    )
    .unwrap();
    assert!(reverified.ok(), "{label}: {reverified:?}");
    // The backup of the old version restores under this build.
    let tested = h
        .call("backup.restore_test", json!({ "backup_id": id.as_str() }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED", "{label}: {tested}");
}

/// Values written at old versions and the columns later migrations added to
/// those rows: the defaults and the one data migration (0010 copies
/// `interventions.trust` into `origin_trust`).
fn check_values(conn: &Connection, version: u32, label: &str) {
    if version >= 4 {
        assert_eq!(
            text(
                conn,
                "SELECT state FROM study_sessions WHERE id = 'fx-session-04'"
            )
            .as_deref(),
            Some("COMPLETED"),
            "{label}"
        );
        assert_eq!(
            text(
                conn,
                "SELECT continuity FROM time_intervals WHERE id = 'fx-interval-04'"
            )
            .as_deref(),
            Some("VERIFIED"),
            "{label}"
        );
    }
    if version >= 5 {
        assert_eq!(
            text(
                conn,
                "SELECT body_markdown FROM notes WHERE id = 'fx-note-05'"
            )
            .as_deref(),
            Some(generate::NOTE_BODY_05),
            "{label}"
        );
        assert_eq!(
            text(
                conn,
                "SELECT body_markdown FROM note_versions WHERE id = 'fx-nv-05'"
            )
            .as_deref(),
            Some(generate::NOTE_BODY_05),
            "{label}"
        );
        assert_eq!(
            text(
                conn,
                "SELECT storage_ref FROM attachments WHERE id = 'fx-att-05'"
            ),
            Some(generate::attachment_ref()),
            "{label}"
        );
    }
    if version >= 6 {
        assert_eq!(
            text(
                conn,
                "SELECT reliability_class FROM grade_decisions WHERE id = 'fx-grade-06'"
            )
            .as_deref(),
            Some("SELF_ADJUDICATED"),
            "{label}: 0010 default"
        );
        assert_eq!(
            int(
                conn,
                "SELECT difficulty FROM assessment_versions WHERE id = 'fx-aver-06'"
            ),
            None,
            "{label}: 0013 never guesses a difficulty"
        );
        assert_eq!(
            text(
                conn,
                "SELECT anchors_json FROM rubrics WHERE id = 'fx-rubric-06'"
            ),
            None,
            "{label}: 0012"
        );
        assert_eq!(
            text(
                conn,
                "SELECT state FROM attempts WHERE id = 'fx-attempt-06'"
            )
            .as_deref(),
            Some("APPROVED_FINAL"),
            "{label}"
        );
    }
    if version >= 7 {
        assert_eq!(
            text(
                conn,
                "SELECT origin_trust FROM interventions WHERE id = 'fx-intervention-07'"
            )
            .as_deref(),
            Some("MODEL_OUTPUT"),
            "{label}: 0010 copied the proposer's trust"
        );
        assert_eq!(
            text(
                conn,
                "SELECT steps_json FROM interventions WHERE id = 'fx-intervention-07'"
            )
            .as_deref(),
            Some("[]"),
            "{label}"
        );
    }
    if version >= 10 {
        assert_eq!(
            text(
                conn,
                "SELECT study_block_id FROM study_sessions WHERE id = 'fx-session-10'"
            )
            .as_deref(),
            Some("fx-block-02"),
            "{label}"
        );
    }
    if version >= 12 {
        let anchors = text(
            conn,
            "SELECT anchors_json FROM rubrics WHERE id = 'fx-rubric-12'",
        )
        .unwrap();
        assert_eq!(
            text(
                conn,
                "SELECT anchors_sha256 FROM rubrics WHERE id = 'fx-rubric-12'"
            ),
            Some(g9_domain::hashing::sha256_hex(anchors.as_bytes())),
            "{label}"
        );
    }
    if version >= 13 {
        assert_eq!(
            int(
                conn,
                "SELECT difficulty FROM assessment_versions WHERE id = 'fx-aver-13'"
            ),
            Some(3),
            "{label}"
        );
    }
    if version >= 15 {
        assert_eq!(
            text(
                conn,
                "SELECT start_monday FROM program_plan WHERE curriculum_id = 'fx-curriculum'"
            )
            .as_deref(),
            Some("2026-01-05"),
            "{label}"
        );
    }
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// Every row of every table and every schema object, as text, sorted.
fn dump(path: &Path) -> Vec<String> {
    let conn = Connection::open(path).unwrap();
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap();
    let objects: Vec<(String, String, String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (kind, name, table, sql) in &objects {
        out.push(format!("schema {kind} {name} {table} {sql:?}"));
        if kind != "table" {
            continue;
        }
        let quoted = name.replace('"', "\"\"");
        let mut rows = conn
            .prepare(&format!("SELECT * FROM \"{quoted}\" ORDER BY rowid"))
            .unwrap();
        let width = rows.column_count();
        let mut cursor = rows.query([]).unwrap();
        while let Some(row) = cursor.next().unwrap() {
            let values: Vec<String> = (0..width)
                .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                .collect();
            out.push(format!("row {name} {}", values.join(" | ")));
        }
    }
    out
}

#[test]
fn the_generator_is_deterministic() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for version in versions() {
        let pa = a.path().join(generate::file_name(version));
        let pb = b.path().join(generate::file_name(version));
        generate::write_fixture(&pa, version).unwrap();
        generate::write_fixture(&pb, version).unwrap();
        assert_eq!(
            std::fs::read(&pa).unwrap(),
            std::fs::read(&pb).unwrap(),
            "schema-v{version:02}: two runs wrote different bytes"
        );
        // One self-contained file: no sidecar left beside it.
        assert_eq!(entries(a.path()).len(), version as usize);
        // It refuses to overwrite.
        assert!(generate::write_fixture(&pa, version).is_err());
    }
}

#[test]
fn the_committed_fixtures_hold_what_the_generator_writes() {
    // Content, not bytes: a newer SQLite may lay the same rows out differently,
    // and the committed files must keep standing for the older builds.
    let scratch = tempfile::tempdir().unwrap();
    for version in versions() {
        let committed = scratch.path().join(format!("committed-{version}.sqlite"));
        copy_fixture(version, &committed);
        let fresh = scratch.path().join(format!("fresh-{version}.sqlite"));
        generate::write_fixture(&fresh, version).unwrap();
        assert_eq!(dump(&committed), dump(&fresh), "schema-v{version:02}");
    }
}
