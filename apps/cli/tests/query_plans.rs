//! PERF-3: the query-plan gate.
//!
//! The SQL the repositories actually run is captured on the connection
//! (`g9_storage::diagnostics::StatementCapture`, SQLite's statement trace)
//! while a representative workload runs: the PERF-1 fixture builder at 1/25
//! of the budget sizes (every operation it uses, through `g9_app::dispatch`)
//! and then every read the screens issue and the writes the fixture does not
//! make (`g9_cli::perf::workload::{run_reads, run_other_writes}`). Each
//! distinct statement is planned with `EXPLAIN QUERY PLAN` (parameters
//! unbound). A step that reads a whole table ([`plans::ScanKind`]: a `SCAN`,
//! an automatic index, or a `SEARCH` that no constraint narrows) of a table
//! that is large in the PERF-1 fixture ([`LARGE_TABLES`]) or by expected
//! growth ([`GROWS_IN_USE`]) fails the gate unless [`ALLOWED`] lists the
//! statement with a reason. The capture leaves out only the tooling's own
//! reads, which carry `diagnostics::MARKER`; everything else the connection
//! ran is planned, the backup's per-table counts included.
//!
//! Why a reduced workload gives the full fixture's plans: the database holds
//! no planner statistics (`sqlite_stat*`; asserted below), so SQLite plans
//! from the schema alone. `plans_do_not_depend_on_row_counts` checks it for
//! every captured statement against an empty database; `examples/perf_plans.rs`
//! checks it against the full fixture (result in the PERF-3 report).
//!
//! Beyond the workload:
//!
//! | Check | Test |
//! |---|---|
//! | the SQL literals of g9-storage the workload does not reach, planned from the source (complete statements; a literal with a `format!` placeholder cannot be planned alone and stays uncovered) | `unreached_sql_literals_are_planned_from_the_source` |
//! | foreign-key lookups SQLite runs internally on a parent change: every foreign key of a large table has a child-side index | `every_foreign_key_of_a_large_table_has_a_lookup_index` |
//! | trigger programs (traced as one `-- TRIGGER` line each): no trigger body reads a table | `trigger_programs_read_no_table` |
//! | the gate bites: a dropped index, a query that stops using its index (`+session_id`) and a MIN/MAX lookup that stops using its index (`+note_id`) all fail it | `the_gate_fails_when_*` |
//! | which literals the workload reached, per file | `coverage_of_the_storage_sql_is_reported` (printed) |
//! | the tables the fixture leaves empty: classified by growth, written by no code, or listed as not classified | `only_listed_growth_tables_lack_a_fixture_writer` (printed) |
//! | no source but `diagnostics.rs` carries the marker the capture filters on | `only_the_diagnostics_module_writes_its_marker` |
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// `CliError` carries a `G9Error` by value (the closed IPC error contract).
#![allow(clippy::result_large_err)]

use g9_cli::perf::fixture::{self, FixtureOptions};
use g9_cli::perf::plans::{self, Allowed, Shape, StatementPlan, ANY_TABLE};
use g9_cli::perf::workload;
use g9_domain::{G9Error, UtcMillis};
use g9_storage::diagnostics::{self, StatementCapture};
use g9_storage::{Database, OpenOptions};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Scale of the workload's fixture.
const DIVISOR: u32 = 25;

/// Rows from which a table counts as large: the smallest size the budget
/// document lists (500 attempts).
const LARGE_ROWS: u64 = 500;

/// Tables with at least [`LARGE_ROWS`] rows in the PERF-1 fixture at the
/// budget sizes, with that count (`g9 perf-fixture --scratch --divisor 1`,
/// per-table rows of its checksum report, measured 2026-11-17 in the
/// sandbox).
const LARGE_TABLES: &[(&str, u64)] = &[
    ("policy_decisions", 50_073),
    ("audit_metadata", 50_000),
    ("domain_events", 49_499),
    ("idempotency_records", 49_499),
    ("review_attempts", 26_208),
    ("competency_observations", 20_000),
    ("session_events", 11_700),
    ("note_versions", 6_000),
    ("time_intervals", 5_000),
    ("criterion_results", 3_000),
    ("notes", 2_000),
    ("learning_episodes", 1_700),
    ("study_sessions", 1_700),
    ("competency_observation_evidence", 1_500),
    ("review_items", 1_000),
    ("rubric_criteria", 600),
    ("calendar_events", 512),
    ("approval_decisions", 501),
    ("approval_requests", 501),
    ("attempts", 500),
    ("exposure_log", 500),
    ("grade_decisions", 500),
];

/// Tables the PERF-1 fixture leaves empty that grow with use, classified as
/// large by expected growth (ASSUMED, not measured: no operation of the
/// fixture writes them), with the reason. The knowledge tables
/// (`knowledge_documents`, `knowledge_chunks`, `knowledge_claims`,
/// `claim_versions`) are not here: no code of this version writes them
/// (`only_listed_growth_tables_lack_a_fixture_writer` lists the tables that
/// have no rows in the fixture).
const GROWS_IN_USE: &[(&str, &str)] = &[
    (
        "evidence_items",
        "evidence.attach_file and the evidence of notes, sessions and attempts add rows with every piece of evidence (budget year: 500 attempts, 1,700 sessions, 2,000 notes); the fixture attaches none",
    ),
    (
        "attachments",
        "one row per file the operator attaches as evidence (evidence.attach_file); the fixture attaches none",
    ),
    (
        "attempt_evidence",
        "one row per evidence item linked to an attempt (500 attempts in the budget year); the fixture links none",
    ),
    (
        "artifact_links",
        "links from evidence items to the records they support; grows with evidence_items; the fixture makes none",
    ),
];

/// Tables that reach [`LARGE_ROWS`] / [`DIVISOR`] rows in the gate's reduced
/// fixture but stay below [`LARGE_ROWS`] at the budget sizes (the curriculum
/// does not scale with the divisor), with the full-size count (same
/// measurement as [`LARGE_TABLES`]).
const NOT_LARGE_AT_FULL_SIZE: &[(&str, u64)] = &[
    ("study_blocks", 408),
    ("resource_references", 256),
    ("daily_objectives", 216),
    ("deliverable_definitions", 72),
    ("training_days", 72),
];

/// The validation-eligibility query of `competency.projection`
/// (`crates/g9-storage/src/repos/competency.rs`, `WITH RECURSIVE ineligible`).
const ELIGIBILITY_SQL: &str = concat!(
    "WITH RECURSIVE ineligible(id) AS ( SELECT e.observation_id FROM ",
    "competency_observation_evidence e WHERE e.validation_eligible = 0 UNION SELECT o.id FROM",
    " competency_observations o JOIN ineligible i ON o.supersedes = i.id WHERE NOT EXISTS ",
    "(SELECT 1 FROM competency_observation_evidence x WHERE x.observation_id = o.id) ), ",
    "eligible_root(id) AS ( SELECT e.observation_id FROM competency_observation_evidence e ",
    "WHERE e.validation_eligible = 1 UNION SELECT o.id FROM competency_observations o JOIN ",
    "eligible_root r ON o.supersedes = r.id WHERE NOT EXISTS (SELECT 1 FROM ",
    "competency_observation_evidence x WHERE x.observation_id = o.id) ), unproven(id) AS ( ",
    "SELECT o.id FROM competency_observations o WHERE o.mode = 'EXAM' AND o.assistance = ",
    "'UNAIDED' AND o.dimension IN ('APPLICATION', 'TRANSFER') AND NOT EXISTS (SELECT 1 FROM ",
    "competency_observation_evidence x WHERE x.observation_id = o.id) AND o.id NOT IN (SELECT",
    " id FROM eligible_root) ) SELECT i.id FROM (SELECT id FROM ineligible UNION SELECT id ",
    "FROM unproven) i JOIN competency_observations o ON o.id = i.id WHERE o.competency_id = ",
    "?1 ORDER BY i.id",
);

/// Whole-table reads of large tables that are accepted, each with its reason.
/// A reason that starts with `FINDING` is debt recorded by PERF-3, not a
/// judgement that the read is fine: each names the change that removes it.
const ALLOWED: &[Allowed] = &[
    Allowed {
        table: "time_intervals",
        sql: concat!(
            "SELECT id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity, ",
            "closed_by, revision, created_at, updated_at FROM time_intervals WHERE end_utc IS NULL ",
            "ORDER BY start_utc, id",
        ),
        reason: "idx_time_intervals_open is a partial index (WHERE end_utc IS NULL): the scan visits only open intervals, at most one per session in progress (recovery at open)",
        shape: None,
    },
    Allowed {
        table: "time_intervals",
        sql: "SELECT session_id FROM time_intervals WHERE end_utc IS NULL LIMIT 1",
        reason: "idx_time_intervals_open is a partial index (WHERE end_utc IS NULL): the scan visits only open intervals and stops at the first",
        shape: None,
    },
    Allowed {
        table: "audit_metadata",
        sql: "SELECT seq, row_hash FROM audit_metadata ORDER BY seq DESC LIMIT 1",
        reason: "seq is the INTEGER PRIMARY KEY (the rowid): the scan runs backwards in rowid order and stops at the first row (the chain head)",
        shape: None,
    },
    Allowed {
        table: "audit_metadata",
        sql: concat!(
            "SELECT seq, id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, actor_skill_id, ",
            "operation, resource_kind, resource_id, decision, outcome, request_id, trace_id, ",
            "details_sha256 FROM audit_metadata ORDER BY seq",
        ),
        reason: "AuditChain::verify_chain recomputes the whole hash chain, so it reads every row by design (recovery at open). FINDING PERF-F3: audit.recent runs it on every call too (crates/g9-app/src/commands/audit.rs), linear in the audit rows; an anchored or incremental verification is a g9-app/g9-storage change",
        shape: None,
    },
    Allowed {
        table: "study_sessions",
        sql: concat!(
            "SELECT id, training_day_id, objective_id, calendar_event_id, state, mode, zone, ",
            "planned_start_utc, planned_end_utc, planned_minutes, started_at, ended_at, credited_ms, ",
            "uncertain_ms, data_domain, classification, revision, created_at, updated_at, ",
            "study_block_id FROM study_sessions WHERE (?1 IS NULL OR state = ?1) AND (?2 = 1 OR ",
            "data_domain = ?3) ORDER BY planned_start_utc, created_at, id",
        ),
        reason: "FINDING PERF-F1: session.list reads every session through this statement even for one date (crates/g9-app/src/commands/sessions.rs filters the date in Rust) and builds a view of each session it keeps (intervals and corrections read twice, the study block once); a planned_start_utc range predicate can use the existing idx_study_sessions_planned (repository and handler change, no migration)",
        shape: None,
    },
    Allowed {
        table: "notes",
        sql: concat!(
            "SELECT n.id, n.title, n.objective_id, n.data_domain, n.classification, n.revision, ",
            "n.created_at, n.updated_at, length(CAST(n.body_markdown AS BLOB)) AS size_bytes, (SELECT",
            " COUNT(*) FROM note_versions v WHERE v.note_id = n.id) AS version_count FROM notes n ",
            "WHERE n.erased = 0 AND (?1 = 1 OR n.data_domain = ?2) ORDER BY n.updated_at DESC, n.id ",
            "DESC LIMIT ?3",
        ),
        reason: "FINDING PERF-F2: notes.list sorts every live note by updated_at before its LIMIT (the schema indexes notes.created_at, not updated_at); an index on notes (updated_at, id) needs a migration",
        shape: None,
    },
    Allowed {
        table: "competency_observation_evidence",
        sql: ELIGIBILITY_SQL,
        reason: "FINDING PERF-F4: competency.projection runs this once per competency, and its recursive sets read every evidence row (no index on validation_eligible) before the outer query filters by competency; filtering by competency inside the sets is a repository change",
        shape: None,
    },
    Allowed {
        table: "competency_observations",
        sql: ELIGIBILITY_SQL,
        reason: "FINDING PERF-F4: the `unproven` set reads every observation (no index on mode/assistance/dimension) once per competency before the outer query filters by competency_id, although idx_competency_observations_competency exists",
        shape: None,
    },
    Allowed {
        table: "evidence_items",
        sql: concat!(
            "SELECT id, kind, note_id, attachment_id, session_id, summary, content_sha256, trust, ",
            "provenance_json, data_domain, classification, invalidated_at, invalidation_reason, ",
            "revision, created_at, updated_at FROM evidence_items WHERE (?1 = 1 OR data_domain = ?2) ",
            "ORDER BY created_at DESC, id DESC LIMIT ?3",
        ),
        reason: "the walk of idx_evidence_items_created runs in the ORDER BY's order and stops at the LIMIT: it reads LIMIT rows when every domain is in scope, and further only past rows of other domains (repos/evidence.rs, the evidence list)",
        shape: None,
    },
    Allowed {
        table: "attachments",
        sql: "SELECT DISTINCT content_sha256 FROM attachments WHERE erased = 0 ORDER BY content_sha256",
        reason: "attachment_store::referenced_digests: backup.create copies, and backup verification hashes, every referenced attachment file, so it lists every digest once per backup by design (operator-initiated, not interactive)",
        shape: None,
    },
    Allowed {
        table: "attachments",
        sql: "SELECT EXISTS (SELECT 1 FROM attachments WHERE storage_ref = ?1 AND erased = 0 AND id <> ?2)",
        reason: "FINDING PERF-F5: the erasure of an attachment (deletion.execute, approval-gated) checks whether another row shares its content-addressed path, and attachments has no index on storage_ref, so it scans the table once per erased attachment; querying by content_sha256 (idx_attachments_sha) is a repository change, an index on storage_ref a migration",
        shape: None,
    },
    Allowed {
        table: "idempotency_records",
        sql: "SELECT MAX(created_at) FROM idempotency_records",
        reason: "g9_storage::idempotency::purge_cutoff (recovery at open): MAX of an indexed column is SQLite's min/max optimisation, one seek to the end of idx_idempotency_records_created_at, although the plan shows the index search without a constraint",
        shape: None,
    },
    Allowed {
        table: "idempotency_records",
        sql: concat!(
            "SELECT r.actor_id, r.idempotency_key, r.outcome_json FROM idempotency_records r ",
            "WHERE instr(r.outcome_json, ?2) > 0 AND EXISTS ( SELECT 1 FROM json_tree(r.outcome_json, '$.result') a ",
            "WHERE a.key = ?1 AND a.type = 'text' AND a.atom = ?2 ",
            "AND json_type(r.outcome_json, a.path || '.\"' || ?3 || '\"') IS NOT NULL) ",
            "ORDER BY r.created_at, r.actor_id, r.idempotency_key",
        ),
        reason: "g9_storage::idempotency::results_quoting (data-inventory gap G1): deleting a calendar event must redact its title from every stored outcome that quotes it, found by JSON structure so no copy is missed whichever operation stored it (one json_tree pass per record that contains the id; the self-join it replaced was quadratic, final security review ipc-3); it runs once per deletion, and the table holds at most RETENTION_DAYS (30) of records because recovery purges older ones at open. An indexed resource column would need a migration and could miss copies quoted by other operations",
        shape: None,
    },
    Allowed {
        table: ANY_TABLE,
        sql: "SELECT * FROM {name} WHERE {clause} ORDER BY rowid",
        reason: "g9_storage::export::collect: export.request (approval-gated, operator-initiated, one consistent read inside the command transaction) reads every row of the selected domains by design, including the parent rows its domain filters select through; not an interactive read. Shape::ExportSelect admits only export's own filter forms",
        shape: Some(Shape::ExportSelect),
    },
    Allowed {
        table: ANY_TABLE,
        sql: "SELECT COUNT(*) FROM \"{quoted}\"",
        reason: "g9_storage::migrations::table_counts, run by backup::create_snapshot (backup.create: operator-initiated, not interactive) on the live database: the per-table row counts the snapshot's relationship check compares; the Online Backup API copies every page of the database in the same command, so the counts add a read of the same order. Shape::QuotedTableCount admits one quoted table name and nothing else",
        shape: Some(Shape::QuotedTableCount),
    },
];

/// Allowances for SQL literals the workload does not reach (planned from the
/// source by `unreached_sql_literals_are_planned_from_the_source`).
const ALLOWED_UNREACHED: &[Allowed] = &[
    Allowed {
        table: "attachments",
        sql: "SELECT storage_ref, content_sha256 FROM attachments WHERE erased = 0 ORDER BY id",
        reason: "backup::check_attachments (backup verification with an attachment root): checks that every referenced attachment file exists and hashes to its recorded SHA-256 (STOR-2), so it reads every attachment row by design; it runs on the snapshot's own connection (operator-initiated), which the gate's capture does not see",
        shape: None,
    },
    Allowed {
        table: "time_intervals",
        sql: "SELECT COUNT(*) FROM time_intervals WHERE end_utc > ?1",
        reason: "g9_storage::repos::sessions::open_interval runs it only for a backdated start (start_utc < now); g9-app always starts an interval at the current reading (crates/g9-app/src/commands/sessions.rs, session.start/resume), so no operation reaches it. Exposing a backdated start would need an index on time_intervals (end_utc), i.e. a migration",
        shape: None,
    },
];

struct Workload {
    plans: Vec<StatementPlan>,
    statements: BTreeMap<String, u64>,
    triggers: BTreeMap<String, u64>,
    tables: Vec<String>,
    /// Rows per table in the reduced fixture, before the extra workload.
    fixture_rows: BTreeMap<String, u64>,
    statistics: bool,
    after_fixture: usize,
}

fn workload() -> &'static Workload {
    static WORKLOAD: OnceLock<Workload> = OnceLock::new();
    WORKLOAD.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let options = FixtureOptions::new(dir.path().join("fixture"), DIVISOR).unwrap();
        let mut capture = None;
        let (report, mut client) = fixture::build(&options, &mut |db| {
            capture = Some(StatementCapture::install(db.conn_mut()).map_err(G9Error::from)?);
            Ok(())
        })
        .unwrap();
        assert_eq!(report.result, "PASS", "{:?}", report.mismatches);
        let fixture_rows = report
            .checksum
            .tables
            .iter()
            .map(|(t, d)| (t.clone(), d.rows))
            .collect();
        let after_fixture = workload::run_reads(&mut client).unwrap()
            + workload::run_other_writes(&mut client).unwrap();
        let capture = capture.unwrap();
        let statements = capture.statements();
        let db = client.ctx.database();
        Workload {
            plans: plans::plan_all(db, &statements).unwrap(),
            triggers: capture.triggers(),
            tables: diagnostics::user_tables(db.conn()).unwrap(),
            statistics: diagnostics::has_planner_statistics(db.conn()).unwrap(),
            fixture_rows,
            statements,
            after_fixture,
        }
    })
}

/// Tables a whole-table read of fails the gate: large in the PERF-1 fixture
/// ([`LARGE_TABLES`]) or by expected growth ([`GROWS_IN_USE`]).
fn large() -> Vec<&'static str> {
    LARGE_TABLES
        .iter()
        .map(|(t, _)| *t)
        .chain(GROWS_IN_USE.iter().map(|(t, _)| *t))
        .collect()
}

fn render(findings: &[plans::Finding]) -> String {
    findings
        .iter()
        .map(|f| format!("  {} {:?}: {}\n    {}", f.table, f.kind, f.detail, f.sql))
        .collect::<Vec<_>>()
        .join("\n")
}

fn storage_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/g9-storage/src")
}

/// `(path relative to g9-storage/src, source)` of the `.rs` files of `dir`,
/// sorted; `diagnostics.rs` (the tooling's own statements, never captured by
/// design) left out.
fn sources(dir: &Path) -> Vec<(String, String)> {
    let root = storage_src();
    let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter(|p| !p.ends_with("diagnostics.rs"))
        .map(|p| {
            (
                p.strip_prefix(&root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn no_unindexed_scan_of_a_large_table_outside_the_allow_list() {
    let w = workload();
    assert!(!w.statistics, "the fixture holds planner statistics");
    assert!(
        w.plans.len() > 150,
        "only {} statements planned",
        w.plans.len()
    );
    for allowance in ALLOWED.iter().chain(ALLOWED_UNREACHED) {
        assert!(
            allowance.reason.len() > 40,
            "an allowance needs a reason: {}",
            allowance.sql
        );
    }
    let outcome = plans::gate(&w.plans, &w.tables, &large(), ALLOWED);
    println!(
        "query-plan gate: {} statements planned ({} operations dispatched after the fixture), {} allowed scans, {} scans of small tables, {} violations",
        w.plans.len(),
        w.after_fixture,
        outcome.allowed.len(),
        outcome.small.len(),
        outcome.violations.len(),
    );
    println!("small-table scans:\n{}", render(&outcome.small));
    println!("allowed scans:\n{}", render(&outcome.allowed));
    for plan in w.plans.iter().filter(|p| {
        outcome
            .allowed
            .iter()
            .chain(&outcome.violations)
            .any(|f| f.sql == p.sql)
    }) {
        println!(
            "plan ({} runs) of {}:\n    {}",
            plan.runs,
            plan.sql,
            plan.details.join("\n    ")
        );
    }
    println!("violations:\n{}", render(&outcome.violations));
    println!("unresolved:\n{}", render(&outcome.unresolved));
    assert!(
        outcome.unresolved.is_empty(),
        "scans of names that are not tables of the schema:\n{}",
        render(&outcome.unresolved)
    );
    assert!(
        outcome.violations.is_empty(),
        "whole-table reads of large tables (add an index through a migration, or allow-list with a reason):\n{}",
        render(&outcome.violations)
    );
    assert!(
        outcome.unused_allowances.is_empty(),
        "allowances that match no statement (remove or update them): {:?}",
        outcome.unused_allowances
    );
}

#[test]
fn the_large_table_list_matches_the_fixture() {
    let w = workload();
    for (table, rows) in LARGE_TABLES {
        assert!(
            w.tables.iter().any(|t| t == table),
            "{table} is not in the schema"
        );
        assert!(*rows >= LARGE_ROWS, "{table}: {rows}");
    }
    for (table, rows) in NOT_LARGE_AT_FULL_SIZE {
        assert!(*rows < LARGE_ROWS, "{table}: {rows}");
        assert!(!large().contains(table), "{table} is listed twice");
    }
    for (table, reason) in GROWS_IN_USE {
        assert!(
            w.tables.iter().any(|t| t == table),
            "{table} is not in the schema"
        );
        assert!(
            reason.len() > 40,
            "{table}: a growth classification needs a reason"
        );
        assert!(
            !LARGE_TABLES.iter().any(|(t, _)| t == table),
            "{table} is listed twice"
        );
        assert_eq!(
            w.fixture_rows.get(*table).copied().unwrap_or(0),
            0,
            "{table} has rows in the fixture now: classify it by its measured size"
        );
    }
    let mut all = large();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), LARGE_TABLES.len() + GROWS_IN_USE.len());
    // A table the reduced fixture fills at a rate that reaches LARGE_ROWS at
    // the budget sizes must be classified (a new table, or one that grew).
    let threshold = LARGE_ROWS.div_ceil(u64::from(DIVISOR));
    println!(
        "rows per table in the 1/{DIVISOR} fixture (threshold {threshold}): {:?}",
        w.fixture_rows
            .iter()
            .filter(|(_, n)| **n > 0)
            .collect::<Vec<_>>()
    );
    let unclassified: Vec<String> = w
        .fixture_rows
        .iter()
        .filter(|(_, rows)| **rows >= threshold)
        .filter(|(t, _)| {
            !large().contains(&t.as_str()) && !NOT_LARGE_AT_FULL_SIZE.iter().any(|(n, _)| n == t)
        })
        .map(|(t, rows)| format!("{t} ({rows} rows at 1/{DIVISOR})"))
        .collect();
    assert!(
        unclassified.is_empty(),
        "tables to classify as large or not (measure them with `g9 perf-fixture --scratch --divisor 1`): {unclassified:?}"
    );
}

/// The tables the PERF-1 fixture leaves empty, so that the gate's size
/// classification is visibly incomplete for them: each is either classified
/// by expected growth ([`GROWS_IN_USE`]) or printed as not classified (the
/// list is part of the PERF-3 report). A table that is written by no code
/// (no `INSERT INTO <table>` literal in g9-storage) cannot grow.
#[test]
fn only_listed_growth_tables_lack_a_fixture_writer() {
    let w = workload();
    let literals: Vec<String> = sources(&storage_src().join("repos"))
        .iter()
        .chain(sources(&storage_src()).iter())
        .flat_map(|(_, src)| plans::sql_literals(src))
        .map(|l| l.sql)
        .collect();
    let written = |table: &str| {
        literals.iter().any(|l| {
            l.starts_with(&format!("INSERT INTO {table} "))
                || l.starts_with(&format!("INSERT INTO {table}("))
                || l.starts_with(&format!("INSERT OR IGNORE INTO {table} "))
                || l.starts_with(&format!("INSERT OR REPLACE INTO {table} "))
        })
    };
    let empty: Vec<&String> = w
        .tables
        .iter()
        .filter(|t| w.fixture_rows.get(*t).copied().unwrap_or(0) == 0)
        .collect();
    let growth: Vec<&String> = empty
        .iter()
        .copied()
        .filter(|t| GROWS_IN_USE.iter().any(|(g, _)| g == t))
        .collect();
    let unwritten: Vec<&String> = empty.iter().copied().filter(|t| !written(t)).collect();
    let other: Vec<&String> = empty
        .iter()
        .copied()
        .filter(|t| written(t) && !growth.contains(t))
        .collect();
    println!(
        "tables without rows in the fixture: {}; classified by growth: {growth:?}; written by no SQL literal of g9-storage: {unwritten:?}; written, not classified (expected to stay below {LARGE_ROWS} rows a year, or written only by operations the fixture does not run): {other:?}",
        empty.len()
    );
    for (table, _) in GROWS_IN_USE {
        assert!(
            written(table),
            "{table} is written by no SQL literal of g9-storage"
        );
    }
    for table in [
        "knowledge_documents",
        "knowledge_chunks",
        "knowledge_claims",
        "claim_versions",
    ] {
        assert!(
            unwritten.iter().any(|t| *t == table),
            "{table} is written now: classify it (GROWS_IN_USE or a measured size)"
        );
    }
}

#[test]
fn plans_do_not_depend_on_row_counts() {
    let w = workload();
    let empty =
        Database::open_in_memory(OpenOptions::new("plans").applied_at(UtcMillis::from_millis(1)))
            .unwrap();
    let on_empty = plans::plan_all(&empty, &w.statements).unwrap();
    assert_eq!(on_empty.len(), w.plans.len());
    let differing: Vec<&str> = w
        .plans
        .iter()
        .zip(&on_empty)
        .filter(|(a, b)| a.details != b.details)
        .map(|(a, _)| a.sql.as_str())
        .collect();
    assert!(
        differing.is_empty(),
        "plans differ between the fixture and an empty database: {differing:#?}"
    );
}

/// A scratch database with the schema (no rows: plans do not depend on them,
/// see `plans_do_not_depend_on_row_counts`).
fn scratch() -> Database {
    Database::open_in_memory(OpenOptions::new("plans").applied_at(UtcMillis::from_millis(1)))
        .unwrap()
}

#[test]
fn the_gate_fails_when_an_index_is_dropped() {
    let w = workload();
    let scratch = scratch();
    let tables = diagnostics::user_tables(scratch.conn()).unwrap();
    let before = plans::gate(
        &plans::plan_all(&scratch, &w.statements).unwrap(),
        &tables,
        &large(),
        ALLOWED,
    );
    assert!(before.violations.is_empty(), "{:?}", before.violations);
    // The scratch database only: the mutation proves the gate bites.
    scratch
        .conn()
        .execute_batch("DROP INDEX idx_time_intervals_session")
        .unwrap();
    let after = plans::gate(
        &plans::plan_all(&scratch, &w.statements).unwrap(),
        &tables,
        &large(),
        ALLOWED,
    );
    let hits: Vec<&plans::Finding> = after
        .violations
        .iter()
        .filter(|v| v.table == "time_intervals")
        .collect();
    assert!(!hits.is_empty(), "{:?}", after.violations);
    println!(
        "with idx_time_intervals_session dropped, {} statements fail the gate:\n{}",
        hits.len(),
        hits.iter()
            .map(|f| format!("  {}\n    {}", f.detail, f.sql))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_gate_fails_when_a_query_stops_using_its_index() {
    let w = workload();
    let scratch = scratch();
    let tables = diagnostics::user_tables(scratch.conn()).unwrap();
    // A captured statement that finds its rows through an index ...
    let (sql, _) = w
        .statements
        .iter()
        .find(|(sql, _)| {
            plans::normalize(sql).contains("FROM time_intervals WHERE session_id = ?1")
        })
        .expect("the workload lists a session's intervals");
    // ... changed the way a careless edit would: `+column` keeps the result
    // and stops SQLite from using an index on the column.
    let changed =
        plans::normalize(sql).replacen("WHERE session_id = ?1", "WHERE +session_id = ?1", 1);
    assert_ne!(changed, plans::normalize(sql));
    let mut statements = BTreeMap::new();
    statements.insert(sql.clone(), 1);
    let before = plans::gate(
        &plans::plan_all(&scratch, &statements).unwrap(),
        &tables,
        &large(),
        ALLOWED,
    );
    assert!(before.violations.is_empty(), "{:?}", before.violations);
    statements.clear();
    statements.insert(changed, 1);
    let after = plans::gate(
        &plans::plan_all(&scratch, &statements).unwrap(),
        &tables,
        &large(),
        ALLOWED,
    );
    assert_eq!(after.violations.len(), 1, "{:?}", after.violations);
    assert_eq!(after.violations[0].table, "time_intervals");
    println!(
        "changed statement fails the gate: {}\n    {}",
        after.violations[0].detail, after.violations[0].sql
    );
}

/// The careless edit of `the_gate_fails_when_a_query_stops_using_its_index`
/// on a MIN/MAX lookup: SQLite still calls the step a `SEARCH` (the MIN/MAX
/// optimisation), but no constraint narrows it, so it walks the whole index
/// until a row passes the filter (`plans::scans` reports it as a full-index
/// read).
#[test]
fn the_gate_fails_when_a_max_lookup_stops_using_its_index() {
    let w = workload();
    let scratch = scratch();
    let tables = diagnostics::user_tables(scratch.conn()).unwrap();
    let (sql, _) = w
        .statements
        .iter()
        .find(|(sql, _)| {
            plans::normalize(sql)
                == "SELECT COALESCE(MAX(version_no), 0) FROM note_versions WHERE note_id = ?1"
        })
        .expect("notes.revise reads the latest version number");
    let changed = plans::normalize(sql).replacen("WHERE note_id", "WHERE +note_id", 1);
    let gate_of = |sql: &str| {
        let mut statements = BTreeMap::new();
        statements.insert(sql.to_string(), 1);
        plans::gate(
            &plans::plan_all(&scratch, &statements).unwrap(),
            &tables,
            &large(),
            ALLOWED,
        )
    };
    let before = gate_of(sql);
    assert!(before.violations.is_empty(), "{:?}", before.violations);
    let after = gate_of(&changed);
    assert_eq!(after.violations.len(), 1, "{:?}", after.violations);
    assert_eq!(after.violations[0].table, "note_versions");
    assert_eq!(after.violations[0].kind, plans::ScanKind::FullIndex);
    assert!(
        after.violations[0]
            .detail
            .starts_with("SEARCH note_versions USING COVERING INDEX")
            && !after.violations[0].detail.contains(" ("),
        "{}",
        after.violations[0].detail
    );
    println!(
        "changed MAX lookup fails the gate: {}\n    {}",
        after.violations[0].detail, after.violations[0].sql
    );
}

/// `.rs` files under `dir`, recursively, sorted.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The capture leaves out the tooling's own reads by `diagnostics::MARKER`
/// alone, so no statement of the application may carry that comment: only
/// `diagnostics.rs` writes it.
#[test]
fn only_the_diagnostics_module_writes_its_marker() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let marker = diagnostics::MARKER.trim();
    assert!(
        marker.starts_with("/*") && marker.ends_with("*/"),
        "{marker}"
    );
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates"))
        .unwrap()
        .filter_map(Result::ok)
    {
        files.extend(rust_files(&entry.path().join("src")));
    }
    files.extend(rust_files(&root.join("apps/cli/src")));
    files.extend(rust_files(&root.join("apps/desktop/src-tauri/src")));
    assert!(files.len() > 100, "{}", files.len());
    let marker_text = marker
        .trim_start_matches("/*")
        .trim_end_matches("*/")
        .trim();
    let carrying: Vec<String> = files
        .iter()
        .filter(|p| std::fs::read_to_string(p).unwrap().contains(marker_text))
        .map(|p| p.strip_prefix(&root).unwrap_or(p).display().to_string())
        .collect();
    assert_eq!(
        carrying,
        vec!["crates/g9-storage/src/diagnostics.rs".to_string()],
        "the diagnostics marker belongs to g9_storage::diagnostics only"
    );
}

#[test]
fn allowances_name_storage_sql() {
    let literals: Vec<(String, plans::SqlLiteral)> = sources(&storage_src().join("repos"))
        .into_iter()
        .chain(sources(&storage_src()))
        .flat_map(|(file, src)| {
            plans::sql_literals(&src)
                .into_iter()
                .map(move |l| (file.clone(), l))
        })
        .collect();
    for allowance in ALLOWED.iter().chain(ALLOWED_UNREACHED) {
        let template = plans::normalize(allowance.sql);
        let matching: Vec<String> = literals
            .iter()
            .filter(|(_, l)| {
                if allowance.is_template() {
                    // The template must be tight: it may cover one source literal only.
                    plans::literal_matches(&template, &l.sql)
                } else {
                    plans::literal_matches(&l.sql, &template)
                }
            })
            .map(|(file, l)| format!("{file}:{}", l.line))
            .collect();
        if allowance.is_template() {
            assert_eq!(
                matching.len(),
                1,
                "a template allowance must match exactly one SQL literal of g9-storage: {} matches {matching:?}",
                allowance.sql
            );
        } else {
            assert!(
                !matching.is_empty(),
                "an allowance names a statement that is no SQL literal of g9-storage: {}",
                allowance.sql
            );
        }
        println!("allowance {} -> {matching:?}", allowance.table);
    }
}

#[test]
fn trigger_programs_read_no_table() {
    let dir = storage_src().join("migrations");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut triggers = 0;
    let mut reading = Vec::new();
    for file in &files {
        for t in plans::trigger_bodies(&std::fs::read_to_string(file).unwrap()) {
            triggers += 1;
            if t.reads_a_table {
                reading.push(format!("{}: {}", file.display(), t.name));
            }
        }
    }
    println!(
        "{triggers} CREATE TRIGGER statements in {} migrations; trigger programs traced by the workload: {:?}",
        files.len(),
        workload().triggers
    );
    assert!(triggers > 50, "the trigger scan found {triggers}");
    assert!(
        reading.is_empty(),
        "trigger bodies that read a table (their statements are not captured; plan them by hand or extend the gate): {reading:?}"
    );
}

#[test]
fn coverage_of_the_storage_sql_is_reported() {
    let w = workload();
    let ran: Vec<String> = w.statements.keys().cloned().collect();
    let repos = plans::coverage(&sources(&storage_src().join("repos")), &ran);
    let other = plans::coverage(&sources(&storage_src()), &ran);
    println!(
        "coverage, crates/g9-storage/src/repos: {} of {} SQL literals reached",
        repos.reached, repos.literals
    );
    println!(
        "coverage, crates/g9-storage/src (other modules, diagnostics.rs excluded): {} of {} SQL literals reached",
        other.reached, other.literals
    );
    println!("unreached (repos):\n  {}", repos.unreached.join("\n  "));
    println!("unreached (other):\n  {}", other.unreached.join("\n  "));
    assert!(
        repos.literals > 100,
        "the literal scan found {}",
        repos.literals
    );
    assert!(repos.reached > 0 && other.reached > 0);
    let literals: Vec<String> = sources(&storage_src().join("repos"))
        .iter()
        .chain(sources(&storage_src()).iter())
        .flat_map(|(_, src)| plans::sql_literals(src))
        .map(|l| l.sql)
        .collect();
    let foreign: Vec<&String> = w
        .plans
        .iter()
        .map(|p| &p.sql)
        .filter(|sql| !literals.iter().any(|l| plans::literal_matches(l, sql)))
        .collect();
    println!(
        "statements planned that match no SQL literal of g9-storage/src (built at run time elsewhere): {}",
        foreign.len()
    );
    for sql in &foreign {
        println!("  {sql}");
    }
}

/// The statement capture (`g9-storage` feature `statement-trace`, rusqlite's
/// `trace`) stays out of production builds: it is defined in g9-storage's
/// `[features]` and enabled only from the `[dev-dependencies]` of this crate.
/// Cargo's resolver 2 does not unify dev-dependency features into a build
/// that does not compile tests or examples, so `cargo build -p g9-cli --bin
/// g9` and the desktop shell build without it.
#[test]
fn statement_trace_is_enabled_by_dev_dependencies_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manifests = vec![root.join("Cargo.toml")];
    for group in ["crates", "apps"] {
        for entry in std::fs::read_dir(root.join(group))
            .unwrap()
            .filter_map(Result::ok)
        {
            for candidate in [
                entry.path().join("Cargo.toml"),
                entry.path().join("src-tauri/Cargo.toml"),
            ] {
                if candidate.is_file() {
                    manifests.push(candidate);
                }
            }
        }
    }
    assert!(manifests.len() > 10, "{manifests:?}");
    let mut uses = Vec::new();
    for manifest in &manifests {
        let text = std::fs::read_to_string(manifest).unwrap();
        let mut section = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                section = trimmed.to_string();
            }
            if !trimmed.starts_with('#') && trimmed.contains("statement-trace") {
                let name = manifest
                    .strip_prefix(&root)
                    .unwrap_or(manifest)
                    .display()
                    .to_string();
                uses.push(format!("{name} {section}"));
            }
        }
    }
    uses.sort();
    assert_eq!(
        uses,
        vec![
            "apps/cli/Cargo.toml [dev-dependencies]".to_string(),
            "crates/g9-storage/Cargo.toml [features]".to_string(),
        ],
        "statement-trace may only be defined by g9-storage and enabled by a dev-dependency"
    );
}

/// A change of a parent row's key (or its delete) makes SQLite look up the
/// referencing rows of every child table; without an index that starts with
/// the foreign-key columns that lookup scans the child table. The statement
/// trace does not see these internal lookups, so they are checked from the
/// schema: no large table may be a child without such an index.
#[test]
fn every_foreign_key_of_a_large_table_has_a_lookup_index() {
    let db = scratch();
    let keys = diagnostics::foreign_keys(db.conn()).unwrap();
    assert!(keys.len() > 50, "{}", keys.len());
    let mut unindexed = Vec::new();
    for key in &keys {
        let lookups = diagnostics::lookup_indexes(db.conn(), &key.table).unwrap();
        let covered = lookups.iter().any(|cols| {
            cols.len() >= key.columns.len() && cols[..key.columns.len()] == key.columns[..]
        });
        if !covered {
            unindexed.push(format!(
                "{} ({}) -> {}",
                key.table,
                key.columns.join(", "),
                key.parent
            ));
        }
    }
    println!(
        "{} foreign keys; without a lookup index on the child side: {unindexed:?}",
        keys.len()
    );
    let large_unindexed: Vec<&String> = unindexed
        .iter()
        .filter(|u| large().iter().any(|t| u.starts_with(&format!("{t} ("))))
        .collect();
    assert!(
        large_unindexed.is_empty(),
        "foreign keys of large tables without a child-side index (a parent change scans the table; the index needs a migration): {large_unindexed:?}"
    );
}

/// The SQL literals of g9-storage that the workload does not reach, planned
/// from the source text (complete statements only: a literal with a
/// `format!` placeholder is completed at run time and cannot be planned
/// alone). Not a substitute for the workload, whose statements are what the
/// application actually runs; a scan found here is reported with the same
/// rules.
#[test]
fn unreached_sql_literals_are_planned_from_the_source() {
    let w = workload();
    let db = scratch();
    let tables = diagnostics::user_tables(db.conn()).unwrap();
    let ran: Vec<String> = w.statements.keys().map(|s| plans::normalize(s)).collect();
    let mut statements = BTreeMap::new();
    let mut templates = 0;
    let mut unpreparable = Vec::new();
    let mut literals = 0;
    for (file, src) in sources(&storage_src().join("repos"))
        .into_iter()
        .chain(sources(&storage_src()))
    {
        for literal in plans::sql_literals(&src) {
            literals += 1;
            if ran.iter().any(|r| plans::literal_matches(&literal.sql, r)) {
                continue;
            }
            if literal.sql.contains('{') {
                templates += 1;
                continue;
            }
            match diagnostics::explain_query_plan(db.conn(), &literal.sql) {
                Ok(_) => {
                    statements.insert(literal.sql.clone(), 0);
                }
                Err(_) => unpreparable.push(format!("{file}:{}: {}", literal.line, literal.sql)),
            }
        }
    }
    let planned = plans::plan_all(&db, &statements).unwrap();
    let outcome = plans::gate(&planned, &tables, &large(), ALLOWED_UNREACHED);
    println!(
        "{literals} SQL literals; not reached by the workload: {} planned from the source, {templates} with a placeholder (not plannable alone), {} that do not prepare alone",
        planned.len(),
        unpreparable.len()
    );
    println!("do not prepare alone:\n  {}", unpreparable.join("\n  "));
    println!(
        "scans of large tables (no allowance):\n{}",
        render(&outcome.violations)
    );
    println!("allowed:\n{}", render(&outcome.allowed));
    println!("unresolved:\n{}", render(&outcome.unresolved));
    assert!(literals > 250, "the literal scan found {literals}");
    assert!(
        unpreparable.is_empty(),
        "SQL literals that do not prepare against the schema: {unpreparable:?}"
    );
    assert!(
        outcome.unresolved.is_empty(),
        "scans of names that are not tables of the schema:\n{}",
        render(&outcome.unresolved)
    );
    assert!(
        outcome.violations.is_empty(),
        "whole-table reads of large tables in SQL the workload does not reach (index through a migration, or allow-list with a reason):\n{}",
        render(&outcome.violations)
    );
    assert!(
        outcome.unused_allowances.is_empty(),
        "allowances for unreached SQL that match nothing (reached now, or changed): {:?}",
        outcome.unused_allowances
    );
}
