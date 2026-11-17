//! Generator of the stored upgrade fixtures (REL-5, docs/release/path-to-10.md
//! §5.16): one small SQLite database per schema version under
//! `crates/g9-app/tests/upgrade_fixtures/schema-vNN.sqlite`.
//! The root `.gitignore` ignores `*.sqlite`; `upgrade_fixtures/.gitignore`
//! re-includes `schema-v*.sqlite`, so a new fixture is committed with the others.
//!
//! The fixture of version `k` is what a build whose newest migration was `k`
//! left on disk after some use: the migrations `0001..k` applied one at a time,
//! each recorded with the application version that introduced it
//! (`0.0.<n>`), and after each migration `n` the rows of [`write_rows`] for `n`,
//! written with raw SQL that is valid at version `n` (later migrations then
//! carry them forward, as they did for the owner's database), plus one audit
//! row per version through the hash chain (`audit_metadata` has not changed
//! since `0001`). Every value is fixed: ids, instants, bytes and the order of
//! statements, so the generator writes the same bytes on every run with this
//! SQLite (gate `the_generator_is_deterministic`).
//!
//! The committed files are the point. A test that regenerates the database
//! from today's code would prove only that today's migrations compose; a
//! stored file is what an older build wrote and keeps standing for it when the
//! code moves on. The generator therefore never overwrites a fixture: when a
//! migration `0016` is added, its fixture is written by the build that adds
//! it and the earlier files stay as they are (gate
//! `the_committed_fixtures_hold_what_the_generator_writes` compares their
//! content, not their bytes, so a newer SQLite does not force a rewrite).
//!
//! Write the missing fixtures (from the repository root):
//!
//! ```text
//! cargo run -p g9-app --example write_upgrade_fixtures -- crates/g9-app/tests/upgrade_fixtures
//! ```
#![allow(dead_code)]

use g9_domain::hashing::{sha256_hex, sha256_json};
use g9_domain::{Actor, ActorId, AuditRef, RequestId, ResourceKind, ResourceRef, TraceId};
use g9_domain::{LearningDimension, UtcMillis};
use g9_storage::audit::{AuditChain, AuditDecision, AuditEntry, AuditOutcome};
use g9_storage::{Database, Migrator, OpenOptions};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

/// 2026-01-01T00:00:00Z: the fixtures are dated before this build.
const BASE: i64 = 1_767_225_600_000;
const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;

/// Directory of the committed fixtures.
pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/upgrade_fixtures")
}

/// File name of the fixture of `version`.
pub fn file_name(version: u32) -> String {
    format!("schema-v{version:02}.sqlite")
}

/// Instant at which the rows of `version` were written.
pub fn at(version: u32) -> i64 {
    BASE + i64::from(version) * DAY
}

/// Application version of the build that introduced migration `version`.
pub fn app_version(version: u32) -> String {
    format!("0.0.{version}")
}

/// Bytes of the attachment the schema-5 rows reference; they live outside the
/// database, so the test places them in the live attachment store.
pub const ATTACHMENT_BYTES: &[u8] = b"g9 upgrade fixture: attachment written at schema 5\n";

/// Body of the schema-5 note.
pub const NOTE_BODY_05: &str = "Written at schema 5.\n\nThe upgrade must keep this text.";

/// Body of the schema-14 note.
pub const NOTE_BODY_14: &str = "Written at schema 14.";

/// Content address of [`ATTACHMENT_BYTES`] (`ab/cd/<sha256>`).
pub fn attachment_ref() -> String {
    let sha = sha256_hex(ATTACHMENT_BYTES);
    format!("{}/{}/{}", &sha[0..2], &sha[2..4], sha)
}

/// `(table, key column, key)` of every row written at `version`, the audit
/// row excluded (`audit_id`).
pub fn rows_written_at(version: u32) -> &'static [(&'static str, &'static str, &'static str)] {
    match version {
        1 => &[
            ("app_settings", "key", "fixture.written_at_schema_1"),
            ("domain_events", "id", "fx-event-01"),
        ],
        2 => &[
            ("curricula", "id", "fx-curriculum"),
            ("curriculum_versions", "id", "fx-cv-02"),
            ("weeks", "id", "fx-week-02"),
            ("training_days", "id", "fx-day-02"),
            ("daily_objectives", "id", "fx-obj-02"),
            ("study_blocks", "id", "fx-block-02"),
        ],
        3 => &[
            ("competencies", "id", "fx-comp-03"),
            ("competency_versions", "id", "fx-compv-03"),
        ],
        4 => &[
            ("calendar_events", "id", "fx-cal-04"),
            ("study_sessions", "id", "fx-session-04"),
            ("session_events", "id", "fx-sev-04"),
            ("time_intervals", "id", "fx-interval-04"),
        ],
        5 => &[
            ("notes", "id", "fx-note-05"),
            ("note_versions", "id", "fx-nv-05"),
            ("attachments", "id", "fx-att-05"),
            ("evidence_items", "id", "fx-evidence-05"),
        ],
        6 => &[
            ("assessment_definitions", "id", "fx-adef-06"),
            ("assessment_versions", "id", "fx-aver-06"),
            ("rubrics", "id", "fx-rubric-06"),
            ("rubric_criteria", "id", "fx-crit-06"),
            ("attempts", "id", "fx-attempt-06"),
            ("criterion_results", "id", "fx-cres-06"),
            ("grade_decisions", "id", "fx-grade-06"),
        ],
        7 => &[
            ("review_items", "id", "fx-review-07"),
            ("interventions", "id", "fx-intervention-07"),
        ],
        8 => &[("skill_registry", "skill_id", "fx-skill-08")],
        9 => &[
            ("goals", "id", "fx-goal-09"),
            ("projects", "id", "fx-project-09"),
        ],
        10 => &[
            ("study_sessions", "id", "fx-session-10"),
            ("interventions", "id", "fx-intervention-10"),
        ],
        11 => &[
            ("study_sessions", "id", "fx-session-11"),
            ("time_intervals", "id", "fx-interval-11"),
        ],
        12 => &[
            ("assessment_versions", "id", "fx-aver-12"),
            ("rubrics", "id", "fx-rubric-12"),
            ("rubric_criteria", "id", "fx-crit-12"),
        ],
        13 => &[("assessment_versions", "id", "fx-aver-13")],
        14 => &[
            ("notes", "id", "fx-note-14"),
            ("note_versions", "id", "fx-nv-14"),
        ],
        15 => &[("program_plan", "curriculum_id", "fx-curriculum")],
        _ => &[],
    }
}

/// Id of the audit row written at `version`.
pub fn audit_id(version: u32) -> String {
    format!("fx-audit-{version:02}")
}

fn err(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context}: {e}")
}

/// Write the fixture of `version` at `path`, which must not exist.
pub fn write_fixture(path: &Path, version: u32) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} exists; fixtures are never overwritten",
            path.display()
        ));
    }
    // The application's own open profile (WAL, foreign keys, secure_delete),
    // without migrating: each migration is applied by the step that wrote it.
    let mut db = Database::open(
        path,
        OpenOptions::new(app_version(1))
            .applied_at(UtcMillis::from_millis(at(1)))
            .no_auto_migrate(),
    )
    .map_err(|e| err("open", e))?;
    for k in 1..=version {
        Migrator::new(app_version(k), UtcMillis::from_millis(at(k)))
            .migrate_up_to(db.conn_mut(), k)
            .map_err(|e| err(&format!("migrate to {k}"), e))?;
        let tx = db.conn_mut().transaction().map_err(|e| err("begin", e))?;
        write_rows(&tx, k).map_err(|e| err(&format!("rows of version {k}"), e))?;
        write_audit_row(&tx, k)?;
        tx.commit().map_err(|e| err("commit", e))?;
    }
    // Closing the last connection checkpoints the WAL and removes the
    // sidecars: the fixture is one self-contained file, as a stopped
    // application leaves it.
    db.into_connection()
        .close()
        .map_err(|(_, e)| err("close", e))
}

fn write_audit_row(conn: &Connection, version: u32) -> Result<(), String> {
    let id = AuditRef::parse(&audit_id(version)).map_err(|e| err("audit id", e))?;
    let operator = ActorId::parse("operator-1").map_err(|e| err("actor", e))?;
    let entry = AuditEntry {
        occurred_at: UtcMillis::from_millis(at(version) + HOUR),
        actor: Actor::operator(operator),
        operation: format!("fixture.schema_{version:02}"),
        resource: Some(ResourceRef::new(
            ResourceKind::AppSetting,
            format!("fixture.schema_{version:02}"),
        )),
        decision: AuditDecision::Allow,
        outcome: AuditOutcome::Success,
        request_id: RequestId::parse(&format!("fx-request-{version:02}"))
            .map_err(|e| err("request id", e))?,
        trace_id: TraceId::parse(&format!("fx-trace-{version:02}"))
            .map_err(|e| err("trace id", e))?,
        details_sha256: Some(sha256_hex(format!("fixture schema {version}").as_bytes())),
    };
    AuditChain::append_with_id(conn, id, &entry).map_err(|e| err("audit row", e))?;
    Ok(())
}

/// The rows written right after migration `version` was applied. Raw SQL:
/// every column named here exists at that version.
pub fn write_rows(conn: &Connection, version: u32) -> rusqlite::Result<()> {
    let t = at(version);
    match version {
        1 => {
            conn.execute(
                "INSERT INTO app_settings (key, value_json, data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fixture.written_at_schema_1', '{\"schema\":1}', 'SYSTEM', 'INTERNAL', 1, ?1, ?1)",
                params![t],
            )?;
            let payload = serde_json::json!({ "schema": 1 });
            conn.execute(
                "INSERT INTO domain_events (id, kind, occurred_at, actor_id, actor_kind, actor_skill_id, trust, schema_version,
                    correlation_id, content_sha256, payload_json, payload_sha256, data_domain, classification)
                 VALUES ('fx-event-01', 'fixture.written', ?1, 'operator-1', 'OPERATOR', NULL, 'OPERATOR', '1.0',
                    'fx-correlation-01', NULL, ?2, ?3, 'SYSTEM', 'INTERNAL')",
                params![t, payload.to_string(), sha256_json(&payload)],
            )?;
        }
        2 => {
            conn.execute(
                "INSERT INTO curricula (id, title, description, data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fx-curriculum', 'Upgrade fixture curriculum', NULL, 'LEARNING', 'INTERNAL', 1, ?1, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO curriculum_versions (id, curriculum_id, version_label, provenance, manifest_sha256, source_ref,
                    invariants_json, imported_by, imported_at, approved_by, approved_at, approval_id, is_current, revision,
                    created_at, updated_at)
                 VALUES ('fx-cv-02', 'fx-curriculum', 'fixture-v02', 'PROPOSED_SEED', ?2, 'fixture://schema-02', '{}',
                    'operator-1', ?1, NULL, NULL, NULL, 0, 1, ?1, ?1)",
                params![t, sha256_hex(b"fixture manifest, schema 2")],
            )?;
            conn.execute(
                "INSERT INTO weeks (id, curriculum_version_id, week_number, title, theme, planned_minutes, created_at)
                 VALUES ('fx-week-02', 'fx-cv-02', 1, 'Week 1', NULL, 480, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week,
                    planned_minutes, title, created_at)
                 VALUES ('fx-day-02', 'fx-week-02', 'fx-cv-02', 'D01', 1, 1, 480, 'Day 1', ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO daily_objectives (id, training_day_id, curriculum_version_id, objective_code, ordinal, statement,
                    success_criteria, dimension, created_at)
                 VALUES ('fx-obj-02', 'fx-day-02', 'fx-cv-02', 'W01-D01-O1', 1, 'Explain the fixture.', NULL, 'RECALL', ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO study_blocks (id, training_day_id, objective_id, ordinal, block_code, minutes, mode, created_at)
                 VALUES ('fx-block-02', 'fx-day-02', 'fx-obj-02', 1, 'recall', 30, 'STUDY', ?1)",
                params![t],
            )?;
        }
        3 => {
            conn.execute(
                "INSERT INTO competencies (id, code, title, description, domain_area, validity_days, rules_json, data_domain,
                    classification, revision, created_at, updated_at)
                 VALUES ('fx-comp-03', 'FX-COMP-03', 'Fixture competency', NULL, 'fixture', 90, '{}', 'LEARNING',
                    'INTERNAL', 1, ?1, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO competency_versions (id, competency_id, version_label, definition_json, created_by, created_at)
                 VALUES ('fx-compv-03', 'fx-comp-03', 'v1', '{}', 'operator-1', ?1)",
                params![t],
            )?;
        }
        4 => {
            let start = t + 9 * HOUR;
            let end = start + 1_800_000;
            conn.execute(
                "INSERT INTO calendar_events (id, title, kind, civil_start, civil_end, zone, start_utc, end_utc, all_day,
                    immutable, protected, source, training_day_id, objective_id, data_domain, classification, revision,
                    created_at, updated_at)
                 VALUES ('fx-cal-04', 'Recall block', 'STUDY_BLOCK', '2026-01-05T09:00:00', '2026-01-05T09:30:00', 'UTC',
                    ?2, ?3, 0, 0, 0, 'OPERATOR', 'fx-day-02', 'fx-obj-02', 'LEARNING', 'PRIVATE', 1, ?1, ?1)",
                params![t, start, end],
            )?;
            conn.execute(
                "INSERT INTO study_sessions (id, training_day_id, objective_id, calendar_event_id, state, mode, zone,
                    planned_start_utc, planned_end_utc, planned_minutes, started_at, ended_at, credited_ms, uncertain_ms,
                    data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fx-session-04', 'fx-day-02', 'fx-obj-02', 'fx-cal-04', 'COMPLETED', 'STUDY', 'UTC', ?2, ?3, 30,
                    ?2, ?3, 1800000, 0, 'LEARNING', 'PRIVATE', 3, ?1, ?1)",
                params![t, start, end],
            )?;
            conn.execute(
                "INSERT INTO session_events (id, session_id, event, from_state, to_state, occurred_at, actor_id, actor_kind,
                    details_json, created_at)
                 VALUES ('fx-sev-04', 'fx-session-04', 'COMPLETE', 'RUNNING', 'COMPLETED', ?2, 'operator-1', 'OPERATOR',
                    NULL, ?1)",
                params![t, end],
            )?;
            conn.execute(
                "INSERT INTO time_intervals (id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity,
                    closed_by, revision, created_at, updated_at)
                 VALUES ('fx-interval-04', 'fx-session-04', ?2, ?3, 1000, 1801000, 'VERIFIED', 'OPERATOR', 2, ?1, ?1)",
                params![t, start, end],
            )?;
        }
        5 => {
            let body_sha = sha256_hex(NOTE_BODY_05.as_bytes());
            conn.execute(
                "INSERT INTO notes (id, title, body_markdown, body_sha256, kind, session_id, objective_id, source_kind, trust,
                    data_domain, classification, erased, erased_at, erasure_request_id, revision, created_at, updated_at)
                 VALUES ('fx-note-05', 'Note written at schema 5', ?2, ?3, 'JOURNAL', 'fx-session-04', 'fx-obj-02',
                    'OPERATOR_INPUT', 'OPERATOR', 'LEARNING', 'PRIVATE', 0, NULL, NULL, 1, ?1, ?1)",
                params![t, NOTE_BODY_05, body_sha],
            )?;
            conn.execute(
                "INSERT INTO note_versions (id, note_id, version_no, title, body_markdown, body_sha256, author_id, erased,
                    created_at)
                 VALUES ('fx-nv-05', 'fx-note-05', 1, 'Note written at schema 5', ?2, ?3, 'operator-1', 0, ?1)",
                params![t, NOTE_BODY_05, body_sha],
            )?;
            conn.execute(
                "INSERT INTO attachments (id, note_id, original_filename, media_type, size_bytes, content_sha256, storage_ref,
                    source_kind, trust, data_domain, classification, erased, erased_at, erasure_request_id, revision,
                    created_at, updated_at)
                 VALUES ('fx-att-05', 'fx-note-05', 'capture.txt', 'text/plain', ?2, ?3, ?4, 'ATTACHMENT', 'OPERATOR',
                    'LEARNING', 'PRIVATE', 0, NULL, NULL, 1, ?1, ?1)",
                params![
                    t,
                    i64::try_from(ATTACHMENT_BYTES.len()).unwrap_or(i64::MAX),
                    sha256_hex(ATTACHMENT_BYTES),
                    attachment_ref()
                ],
            )?;
            conn.execute(
                "INSERT INTO evidence_items (id, kind, note_id, attachment_id, session_id, summary, content_sha256, trust,
                    provenance_json, data_domain, classification, invalidated_at, invalidation_reason, revision,
                    created_at, updated_at)
                 VALUES ('fx-evidence-05', 'ATTACHMENT', 'fx-note-05', 'fx-att-05', 'fx-session-04', 'Attached file',
                    ?2, 'OPERATOR', '{\"source\":\"fixture\"}', 'LEARNING', 'PRIVATE', NULL, NULL, 1, ?1, ?1)",
                params![t, sha256_hex(ATTACHMENT_BYTES)],
            )?;
        }
        6 => {
            let task = "Find the planted beacon in the capture.";
            conn.execute(
                "INSERT INTO assessment_definitions (id, code, title, kind, objective_id, training_day_id, competency_id,
                    mode, data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fx-adef-06', 'FX-A-06', 'Fixture practical', 'BLIND_PRACTICAL', 'fx-obj-02', 'fx-day-02',
                    'fx-comp-03', 'EXAM', 'LEARNING', 'INTERNAL', 1, ?1, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256,
                    sealed_key_ref, sealed_key_sha256, key_classification, pass_threshold, created_by, created_at)
                 VALUES ('fx-aver-06', 'fx-adef-06', 'v1', ?2, ?3, NULL, NULL, 'SECRET', 80, 'operator-1', ?1)",
                params![t, task, sha256_hex(task.as_bytes())],
            )?;
            conn.execute(
                "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, created_at)
                 VALUES ('fx-rubric-06', 'fx-aver-06', 'Fixture rubric', 'v1', 100, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO rubric_criteria (id, rubric_id, code, title, ordinal, weight, machine_checkable, critical_gate,
                    created_at)
                 VALUES ('fx-crit-06', 'fx-rubric-06', 'independent_execution', 'Independent execution', 1, 100, 0, 0, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO attempts (id, assessment_version_id, session_id, retake_of, state, mode, assistance_level,
                    hint_count, started_at, submitted_at, finalized_at, submission_sha256, data_domain, classification,
                    revision, created_at, updated_at)
                 VALUES ('fx-attempt-06', 'fx-aver-06', NULL, NULL, 'APPROVED_FINAL', 'EXAM', 'UNAIDED', 0, ?1, ?2, ?3, ?4,
                    'LEARNING', 'PRIVATE', 5, ?1, ?3)",
                params![t, t + HOUR, t + 2 * HOUR, sha256_hex(b"fixture submission")],
            )?;
            conn.execute(
                "INSERT INTO criterion_results (id, attempt_id, criterion_id, score, source, evidence_refs, rationale,
                    scored_by, scored_at, supersedes, created_at)
                 VALUES ('fx-cres-06', 'fx-attempt-06', 'fx-crit-06', 90, 'HUMAN_REVIEWER', '[]', NULL, 'operator-1', ?1,
                    NULL, ?1)",
                params![t + 2 * HOUR],
            )?;
            conn.execute(
                "INSERT INTO grade_decisions (id, attempt_id, total_score, passed, is_final, decided_by, decided_at,
                    approval_id, rationale, criterion_results_sha256, supersedes, revision, created_at, updated_at)
                 VALUES ('fx-grade-06', 'fx-attempt-06', 90, 1, 1, 'operator-1', ?1, NULL, NULL, NULL, NULL, 1, ?1, ?1)",
                params![t + 2 * HOUR],
            )?;
        }
        7 => {
            conn.execute(
                "INSERT INTO review_items (id, kind, competency_id, objective_id, prompt, answer_ref, scheduler, due_at,
                    last_success_at, stage, interval_days, fsrs_state_json, state, data_domain, classification, revision,
                    created_at, updated_at)
                 VALUES ('fx-review-07', 'DECLARATIVE', 'fx-comp-03', 'fx-obj-02', 'What does the fixture prove?', NULL,
                    'BASELINE', ?2, NULL, 0, 1, NULL, 'ACTIVE', 'LEARNING', 'INTERNAL', 1, ?1, ?1)",
                params![t, t + DAY],
            )?;
            // Before 0010 `trust` held the proposer's trust; 0010 copies it into
            // `origin_trust` (the one data migration of the set).
            conn.execute(
                "INSERT INTO interventions (id, hypothesis_id, competency_id, kind, description, state, proposed_by,
                    proposed_at, trust, approval_id, scheduled_for, applied_at, invalidated_at, invalidation_reason,
                    data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fx-intervention-07', NULL, 'fx-comp-03', 'REMEDIATION', 'Repeat the lab with a hint budget',
                    'PROPOSED', 'model-local', ?1, 'MODEL_OUTPUT', NULL, NULL, NULL, NULL, NULL, 'LEARNING', 'PRIVATE', 1,
                    ?1, ?1)",
                params![t],
            )?;
        }
        8 => {
            conn.execute(
                "INSERT INTO skill_registry (skill_id, code, family, title, package_dir, current_version_label, state,
                    revision, created_at, updated_at)
                 VALUES ('fx-skill-08', 'K99', 'KNOWLEDGE', 'Fixture skill', 'fixture/k99', NULL, 'REGISTERED', 1, ?1, ?1)",
                params![t],
            )?;
        }
        9 => {
            conn.execute(
                "INSERT INTO goals (id, title, description, horizon, priority, state, parent_goal_id, target_date_civil,
                    zone, owner_id, data_domain, classification, revision, created_at, updated_at)
                 VALUES ('fx-goal-09', 'Finish the program', NULL, 'QUARTER', 2, 'ACTIVE', NULL, '2026-03-31', 'UTC',
                    'operator-1', 'BUSINESS', 'PRIVATE', 1, ?1, ?1)",
                params![t],
            )?;
            conn.execute(
                "INSERT INTO projects (id, goal_id, title, description, state, starts_at, ends_at, data_domain,
                    classification, revision, created_at, updated_at)
                 VALUES ('fx-project-09', 'fx-goal-09', 'Capstone', NULL, 'PLANNED', NULL, NULL, 'BUSINESS', 'PRIVATE', 1,
                    ?1, ?1)",
                params![t],
            )?;
        }
        10 => {
            conn.execute(
                "INSERT INTO study_sessions (id, training_day_id, objective_id, calendar_event_id, state, mode, zone,
                    planned_start_utc, planned_end_utc, planned_minutes, started_at, ended_at, credited_ms, uncertain_ms,
                    data_domain, classification, revision, created_at, updated_at, study_block_id)
                 VALUES ('fx-session-10', 'fx-day-02', 'fx-obj-02', NULL, 'PLANNED', 'STUDY', 'UTC', ?2, ?3, 30, NULL,
                    NULL, 0, 0, 'LEARNING', 'PRIVATE', 1, ?1, ?1, 'fx-block-02')",
                params![t, t + 9 * HOUR, t + 9 * HOUR + 1_800_000],
            )?;
            conn.execute(
                "INSERT INTO interventions (id, hypothesis_id, competency_id, kind, description, state, proposed_by,
                    proposed_at, trust, approval_id, scheduled_for, applied_at, invalidated_at, invalidation_reason,
                    data_domain, classification, revision, created_at, updated_at, origin_trust, reviewed_by,
                    reviewed_at, steps_json, time_budget_ms, acceptance)
                 VALUES ('fx-intervention-10', NULL, 'fx-comp-03', 'PRACTICE', 'Two timed drills', 'PROPOSED',
                    'operator-1', ?1, 'OPERATOR', NULL, NULL, NULL, NULL, NULL, 'LEARNING', 'PRIVATE', 1, ?1, ?1,
                    'OPERATOR', NULL, NULL, '[\"drill one\",\"drill two\"]', 1200000, 'both drills under ten minutes')",
                params![t],
            )?;
        }
        11 => {
            let start = t + 10 * HOUR;
            let end = start + 2_400_000;
            conn.execute(
                "INSERT INTO study_sessions (id, training_day_id, objective_id, calendar_event_id, state, mode, zone,
                    planned_start_utc, planned_end_utc, planned_minutes, started_at, ended_at, credited_ms, uncertain_ms,
                    data_domain, classification, revision, created_at, updated_at, study_block_id)
                 VALUES ('fx-session-11', 'fx-day-02', 'fx-obj-02', NULL, 'COMPLETED', 'LAB', 'UTC', ?2, ?3, 40, ?2, ?3,
                    2400000, 0, 'LEARNING', 'PRIVATE', 3, ?1, ?1, 'fx-block-02')",
                params![t, start, end],
            )?;
            conn.execute(
                "INSERT INTO time_intervals (id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity,
                    closed_by, revision, created_at, updated_at)
                 VALUES ('fx-interval-11', 'fx-session-11', ?2, ?3, 5000, 2405000, 'VERIFIED', 'OPERATOR', 2, ?1, ?1)",
                params![t, start, end],
            )?;
        }
        12 => {
            let task = "Explain the beacon to a reviewer.";
            conn.execute(
                "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256,
                    sealed_key_ref, sealed_key_sha256, key_classification, pass_threshold, created_by, created_at)
                 VALUES ('fx-aver-12', 'fx-adef-06', 'v2', ?2, ?3, NULL, NULL, 'SECRET', 80, 'operator-1', ?1)",
                params![t, task, sha256_hex(task.as_bytes())],
            )?;
            let anchors = rubric_anchors_json();
            conn.execute(
                "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, created_at,
                    anchors_json, anchors_sha256)
                 VALUES ('fx-rubric-12', 'fx-aver-12', 'Anchored rubric', 'v1', 100, ?1, ?2, ?3)",
                params![t, anchors, sha256_hex(anchors.as_bytes())],
            )?;
            conn.execute(
                "INSERT INTO rubric_criteria (id, rubric_id, code, title, ordinal, weight, machine_checkable, critical_gate,
                    created_at)
                 VALUES ('fx-crit-12', 'fx-rubric-12', 'independent_execution', 'Independent execution', 1, 100, 0, 0, ?1)",
                params![t],
            )?;
        }
        13 => {
            let task = "Write the incident timeline.";
            conn.execute(
                "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256,
                    sealed_key_ref, sealed_key_sha256, key_classification, pass_threshold, created_by, created_at,
                    difficulty)
                 VALUES ('fx-aver-13', 'fx-adef-06', 'v3', ?2, ?3, NULL, NULL, 'SECRET', 80, 'operator-1', ?1, 3)",
                params![t, task, sha256_hex(task.as_bytes())],
            )?;
        }
        14 => {
            let body_sha = sha256_hex(NOTE_BODY_14.as_bytes());
            conn.execute(
                "INSERT INTO notes (id, title, body_markdown, body_sha256, kind, session_id, objective_id, source_kind, trust,
                    data_domain, classification, erased, erased_at, erasure_request_id, revision, created_at, updated_at)
                 VALUES ('fx-note-14', 'Note written at schema 14', ?2, ?3, 'REFLECTION', NULL, NULL, 'OPERATOR_INPUT',
                    'OPERATOR', 'LEARNING', 'INTERNAL', 0, NULL, NULL, 1, ?1, ?1)",
                params![t, NOTE_BODY_14, body_sha],
            )?;
            conn.execute(
                "INSERT INTO note_versions (id, note_id, version_no, title, body_markdown, body_sha256, author_id, erased,
                    created_at)
                 VALUES ('fx-nv-14', 'fx-note-14', 1, 'Note written at schema 14', ?2, ?3, 'operator-1', 0, ?1)",
                params![t, NOTE_BODY_14, body_sha],
            )?;
        }
        15 => {
            conn.execute(
                "INSERT INTO program_plan (curriculum_id, start_monday, day_start_minute, revision, created_at, updated_at)
                 VALUES ('fx-curriculum', '2026-01-05', 540, 1, ?1, ?1)",
                params![t],
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// The anchored scoring standard of the schema-12 rubric, serialized as the
/// application stores it (`rubrics.anchors_json`, migration 0012).
fn rubric_anchors_json() -> String {
    let criterion = g9_assessment::RubricCriterionDef {
        code: "independent_execution".to_string(),
        title: "Independent execution".to_string(),
        weight: 100,
        machine_checkable: false,
        critical_gate: false,
        dimension: LearningDimension::Application,
    };
    let anchors = g9_assessment::RubricAnchors::standard_for(&[criterion]);
    serde_json::to_string(&anchors).unwrap_or_default()
}
