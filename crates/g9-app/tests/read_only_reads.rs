#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-3 (2026-11-10 review): when recovery refused writes (a broken audit chain, a drifted
//! schema, unapplied migrations) storage is `READ_ONLY`, yet every read still committed a
//! `policy_decisions` row into the database recovery said must not be written. Reads in that
//! state are evaluated by policy as before, and their decision is not persisted.
//!
//! Every read-only operation of the registry (`g9_app::ops::OPERATIONS`, `mutating == false`) is
//! driven over a `READ_ONLY` database, and the full content of every table (and the schema) is
//! compared before and after, so a read handler that writes anything is caught, not only the
//! pipeline's decision row. A read operation added without a payload below fails the test.

mod common;

use common::*;
use g9_app::ops::OPERATIONS;
use g9_domain::UtcMillis;
use g9_storage::{Database, OpenOptions};
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;

const DAY: i64 = 86_400_000;

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// The schema and every row of every table, each table's rows sorted (a content fingerprint).
fn contents(conn: &Connection) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
        .unwrap();
    let schema = stmt
        .query_map([], |r| {
            Ok(format!(
                "schema {:?} {:?} {:?} {:?}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    out.extend(schema);
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for table in tables {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{}\"", table.replace('"', "\"\"")))
            .unwrap();
        let columns = stmt.column_count();
        let mut rows: Vec<String> = stmt
            .query_map([], |r| {
                let mut cells = Vec::with_capacity(columns);
                for i in 0..columns {
                    cells.push(match r.get_ref(i)? {
                        ValueRef::Null => "NULL".to_string(),
                        ValueRef::Integer(n) => n.to_string(),
                        ValueRef::Real(f) => f.to_string(),
                        ValueRef::Text(t) => format!("{:?}", String::from_utf8_lossy(t)),
                        ValueRef::Blob(b) => format!("blob{b:?}"),
                    });
                }
                Ok(cells.join("|"))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.sort();
        out.push(format!("table {table}: {} rows", rows.len()));
        out.extend(rows.into_iter().map(|row| format!("{table} {row}")));
    }
    out
}

/// The harness configuration of the desktop layout under `root` (so `backup.list` has a
/// directory to read).
fn dirs(root: &Path) -> impl FnOnce(&mut Setup) {
    let root = root.to_path_buf();
    move |s| {
        std::fs::create_dir_all(root.join("backups")).unwrap();
        std::fs::create_dir_all(root.join("scratch")).unwrap();
        s.config.approved_roots = vec![root.clone()];
        s.config.backup_dir = Some(root.join("backups"));
        s.config.scratch_dir = Some(root.join("scratch"));
    }
}

/// What the reads need (written while the database is still writable): the seed imported, the
/// program started and a note. Returns the note id.
fn prepare(h: &mut Harness) -> String {
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    h.call(
        "curriculum.set_program_start",
        json!({ "date": "2026-11-23" }),
    )
    .unwrap();
    let mut note = String::new();
    for n in 0..4 {
        let created = h
            .call(
                "notes.create",
                json!({ "title": format!("Note {n}"), "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
            )
            .unwrap();
        note = created["result"]["note_id"].as_str().unwrap().to_string();
    }
    note
}

/// A valid payload for every read-only operation (`None`: no fixture yet).
fn read_payload(op: &str, note: &str) -> Option<Value> {
    Some(match op {
        "system.status"
        | "curriculum.program"
        | "session.list"
        | "assessment.list"
        | "competency.projection"
        | "retention.due"
        | "interventions.list"
        | "approvals.list"
        | "audit.recent"
        | "evidence.list"
        | "notes.list"
        | "backup.list" => json!({}),
        "curriculum.validate" => json!({ "manifest": seed_manifest() }),
        "curriculum.daily_plan" => json!({ "date": "2026-11-24" }),
        "calendar.list" => json!({ "from": T0, "to": T0 + DAY }),
        "notes.get" => json!({ "note_id": note }),
        _ => return None,
    })
}

/// Every read-only operation of the registry, each answered; returns how many ran.
fn read_everything(h: &mut Harness, note: &str) -> usize {
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(
        status["result"]["storage"]["state"], "READ_ONLY",
        "{status}"
    );
    let mut ran = 0;
    for spec in OPERATIONS.iter().filter(|s| !s.mutating) {
        let payload = read_payload(spec.name, note).unwrap_or_else(|| {
            panic!(
                "read operation {} has no payload in read_only_reads.rs",
                spec.name
            )
        });
        let out = h.call(spec.name, payload);
        assert!(out.is_ok(), "{} over READ_ONLY: {out:?}", spec.name);
        ran += 1;
    }
    // A scope other than the default one, in EXAM mode.
    h.call_with(
        "notes.list",
        json!({}),
        &Opts {
            scope: Some(("SECURITY_SENSITIVE", "EXAM")),
            ..Opts::default()
        },
    )
    .unwrap();
    ran
}

/// Drop the delete guard, remove the newest audit rows, put the guard back exactly.
fn delete_audit_rows_after(conn: &Connection, keep: i64) {
    let guard: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_audit_metadata_no_delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute_batch("DROP TRIGGER trg_audit_metadata_no_delete")
        .unwrap();
    conn.execute("DELETE FROM audit_metadata WHERE seq > ?1", [keep])
        .unwrap();
    conn.execute_batch(&guard).unwrap();
}

#[test]
fn reads_over_a_broken_audit_chain_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::with(dirs(dir.path()));
    let note = prepare(&mut h);
    let rows = count(h.conn(), "SELECT COUNT(*) FROM audit_metadata");
    delete_audit_rows_after(h.conn(), rows - 1);
    h.restart();
    assert_eq!(
        h.ctx().recovery_report().write_block_reason(),
        Some("audit_chain_broken")
    );

    let before = contents(h.conn());
    let decisions = count(h.conn(), "SELECT COUNT(*) FROM policy_decisions");
    let ran = read_everything(&mut h, &note);
    assert_eq!(ran, 16, "every read-only operation of the registry ran");
    assert_eq!(
        count(h.conn(), "SELECT COUNT(*) FROM policy_decisions"),
        decisions,
        "a READ_ONLY database is not written by reads"
    );
    assert_eq!(
        contents(h.conn()),
        before,
        "no table changed while READ_ONLY"
    );
}

#[test]
fn reads_over_a_drifted_schema_fire_no_planted_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("g9.sqlite");
    let db = Database::open(
        &path,
        OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0)),
    )
    .unwrap();
    let mut h = Harness::with_database(db, dirs(dir.path())).unwrap();
    let note = prepare(&mut h);
    drop(h.ctx.take().unwrap().into_database());
    drop(h);
    let c = Connection::open(&path).unwrap();
    c.execute_batch(
        "CREATE TABLE zz_spy (n INTEGER);
         CREATE TRIGGER zz_spy_on_decisions AFTER INSERT ON policy_decisions BEGIN INSERT INTO zz_spy VALUES (1); END;",
    )
    .unwrap();
    drop(c);
    let db = Database::open(&path, OpenOptions::new("0.1.0-test")).unwrap();
    let mut h = Harness::with_database(db, dirs(dir.path())).unwrap();
    h.continue_ids_from(10_000);
    assert_eq!(
        h.ctx().recovery_report().write_block_reason(),
        Some("schema_drift")
    );

    let before = contents(h.conn());
    let ran = read_everything(&mut h, &note);
    assert_eq!(ran, 16);
    assert_eq!(
        count(h.conn(), "SELECT COUNT(*) FROM zz_spy"),
        0,
        "the planted trigger never ran"
    );
    assert_eq!(
        contents(h.conn()),
        before,
        "no table changed while READ_ONLY"
    );
}

#[test]
fn reads_on_a_writable_database_still_persist_their_decision() {
    let mut h = Harness::new();
    let before = count(h.conn(), "SELECT COUNT(*) FROM policy_decisions");
    h.call("session.list", json!({})).unwrap();
    assert_eq!(
        count(h.conn(), "SELECT COUNT(*) FROM policy_decisions"),
        before + 1
    );
}
