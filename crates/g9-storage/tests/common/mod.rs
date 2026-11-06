//! Shared fixtures for the gate tests: deterministic timestamps and a populated
//! database spanning every table group.
#![allow(dead_code)]

use g9_domain::hashing::sha256_hex;
use g9_domain::{
    Action, Actor, ActorId, ApprovalId, Classification, DataDomain, EventHeader, EventId,
    IdempotencyKey, InteractionMode, OperationDescriptor, RequestId, ResourceKind, ResourceRef,
    SchemaVersion, Scope, TraceId, TrustClass, UtcMillis,
};
use g9_storage::approvals_store::{insert_request, NewApprovalRequest};
use g9_storage::audit::{AuditChain, AuditDecision, AuditEntry, AuditOutcome};
use g9_storage::events::{append_event, DomainEvent};
use g9_storage::idempotency;
use g9_storage::{Database, OpenOptions};
use rusqlite::Connection;
use std::path::Path;

/// 2025-11-06T00:00:00Z.
pub const T0: i64 = 1_762_387_200_000;

pub fn t(offset_ms: i64) -> UtcMillis {
    UtcMillis::from_millis(T0 + offset_ms)
}

pub fn sha(s: &str) -> String {
    sha256_hex(s.as_bytes())
}

pub fn opts() -> OpenOptions {
    OpenOptions::new("0.1.0-test").applied_at(t(0))
}

pub fn open_mem() -> Database {
    Database::open_in_memory(opts()).expect("open in memory")
}

pub fn open_file(path: &Path) -> Database {
    Database::open(path, opts()).expect("open file db")
}

pub fn operator() -> Actor {
    Actor::operator(ActorId::parse("operator-1").unwrap())
}

pub fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// Ids created by [`populate`].
#[derive(Debug, Clone)]
pub struct Fixture {
    pub note_id: String,
    pub note_version_id: String,
    pub attachment_id: String,
    pub attachment_storage_ref: String,
    pub attachment_sha: String,
    pub evidence_id: String,
    pub hypothesis_id: String,
    pub reflection_id: String,
    pub session_id: String,
    pub attempt_final_id: String,
    pub attempt_open_id: String,
    pub grade_final_id: String,
    pub criterion_result_id: String,
    pub observation_id: String,
    pub approval_id: ApprovalId,
    pub curriculum_version_id: String,
    pub week_id: String,
    pub day_id: String,
    pub objective_id: String,
}

/// Populate rows across every table group (curriculum, calendar/time, evidence,
/// assessment, competency, retention, knowledge/AI, business, governance).
pub fn populate(conn: &Connection) -> Fixture {
    let now = T0;
    let e = |sql: &str, p: &[&dyn rusqlite::ToSql]| {
        conn.execute(sql, p)
            .unwrap_or_else(|err| panic!("{sql}: {err}"));
    };

    e("INSERT INTO app_settings (key, value_json, created_at, updated_at) VALUES ('ui.theme', '\"dark\"', ?1, ?1)", &[&now]);

    // curriculum
    e("INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('cur-1', 'G9 CT9 480', ?1, ?1)", &[&now]);
    e(
        "INSERT INTO curriculum_versions (id, curriculum_id, version_label, provenance, manifest_sha256, source_ref, invariants_json,
            imported_by, imported_at, created_at, updated_at)
         VALUES ('cv-1', 'cur-1', 'v0', 'PROPOSED_SEED', ?2, 'data/curriculum/seed.json', '{}', 'operator-1', ?1, ?1, ?1)",
        &[&now, &sha("manifest")],
    );
    e(
        "INSERT INTO weeks (id, curriculum_version_id, week_number, title, planned_minutes, created_at) VALUES ('w-1', 'cv-1', 1, 'Week 1', 2400, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week, planned_minutes, title, created_at)
         VALUES ('d-1', 'w-1', 'cv-1', 'D01', 1, 1, 420, 'Day 1', ?1)",
        &[&now],
    );
    e(
        "INSERT INTO daily_objectives (id, training_day_id, curriculum_version_id, objective_code, ordinal, statement, created_at)
         VALUES ('o-1', 'd-1', 'cv-1', 'D01-O1', 1, 'Explain the Linux boot process', ?1)",
        &[&now],
    );
    e(
        "INSERT INTO study_blocks (id, training_day_id, objective_id, ordinal, block_code, minutes, mode, created_at)
         VALUES ('sb-1', 'd-1', 'o-1', 1, 'recall', 15, 'STUDY', ?1)",
        &[&now],
    );

    // competency
    e(
        "INSERT INTO competencies (id, code, title, domain_area, created_at, updated_at) VALUES ('comp-1', 'LINUX-BOOT', 'Linux boot', 'linux', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO objective_competencies (objective_id, competency_id, dimension, created_at) VALUES ('o-1', 'comp-1', 'UNDERSTANDING', ?1)",
        &[&now],
    );
    e(
        "INSERT INTO learner_hypotheses (id, competency_id, statement, kind, supporting_refs, contradicting_refs, confidence_low,
            confidence_high, status, provenance_json, trust, created_at, updated_at)
         VALUES ('hyp-1', 'comp-1', 'confuses initrd with initramfs', 'MISCONCEPTION', '[\"note-1\"]', '[]', 0.2, 0.6, 'PROPOSED',
            '{\"source_kind\":\"MODEL_RESPONSE\"}', 'MODEL_OUTPUT', ?1, ?1)",
        &[&now],
    );

    // calendar / time
    e(
        "INSERT INTO calendar_events (id, title, kind, civil_start, civil_end, zone, start_utc, end_utc, all_day, immutable, protected,
            source, training_day_id, objective_id, created_at, updated_at)
         VALUES ('ev-1', 'Day 1 study', 'STUDY_BLOCK', '2025-11-06T09:00:00', '2025-11-06T16:00:00', 'Europe/Berlin', ?1, ?2, 0, 0, 0,
            'SCHEDULER', 'd-1', 'o-1', ?1, ?1)",
        &[&now, &(now + 7 * 3_600_000)],
    );
    e(
        "INSERT INTO study_sessions (id, training_day_id, objective_id, calendar_event_id, state, mode, zone, planned_minutes, created_at, updated_at)
         VALUES ('sess-1', 'd-1', 'o-1', 'ev-1', 'RUNNING', 'STUDY', 'Europe/Berlin', 420, ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO session_events (id, session_id, event, from_state, to_state, occurred_at, actor_id, actor_kind, created_at)
         VALUES ('se-1', 'sess-1', 'START', 'READY', 'RUNNING', ?1, 'operator-1', 'OPERATOR', ?1)",
        &[&now],
    );
    e(
        "INSERT INTO time_intervals (id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity, closed_by, created_at, updated_at)
         VALUES ('ti-1', 'sess-1', ?1, ?2, 1000, 601000, 'VERIFIED', 'OPERATOR', ?1, ?1)",
        &[&now, &(now + 600_000)],
    );

    // evidence
    let body = "# Lab log\nkernel boots via GRUB";
    e(
        "INSERT INTO notes (id, title, body_markdown, body_sha256, kind, session_id, objective_id, source_kind, trust, created_at, updated_at)
         VALUES ('note-1', 'Lab log', ?2, ?3, 'LAB_LOG', 'sess-1', 'o-1', 'OPERATOR_INPUT', 'OPERATOR', ?1, ?1)",
        &[&now, &body, &sha(body)],
    );
    e(
        "INSERT INTO note_versions (id, note_id, version_no, title, body_markdown, body_sha256, author_id, created_at)
         VALUES ('nv-1', 'note-1', 1, 'Lab log', ?2, ?3, 'operator-1', ?1)",
        &[&now, &body, &sha(body)],
    );
    let att_sha = sha("pcap-bytes");
    let storage_ref = format!("{}/{}/{}", &att_sha[0..2], &att_sha[2..4], att_sha);
    e(
        "INSERT INTO attachments (id, note_id, original_filename, media_type, size_bytes, content_sha256, storage_ref, source_kind, trust,
            created_at, updated_at)
         VALUES ('att-1', 'note-1', 'capture.pcap', 'application/vnd.tcpdump.pcap', 10, ?2, ?3, 'OPERATOR_INPUT', 'OPERATOR', ?1, ?1)",
        &[&now, &att_sha, &storage_ref],
    );
    e(
        "INSERT INTO evidence_items (id, kind, note_id, attachment_id, session_id, summary, content_sha256, trust, provenance_json, created_at, updated_at)
         VALUES ('evi-1', 'NOTE', 'note-1', NULL, 'sess-1', 'lab log for day 1', ?2, 'OPERATOR', '{\"source_kind\":\"NOTE\"}', ?1, ?1)",
        &[&now, &sha(body)],
    );

    // assessment
    e(
        "INSERT INTO assessment_definitions (id, code, title, kind, objective_id, training_day_id, competency_id, created_at, updated_at)
         VALUES ('ad-1', 'W1-BLIND', 'Week 1 blind practical', 'BLIND_PRACTICAL', 'o-1', 'd-1', 'comp-1', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, sealed_key_ref, sealed_key_sha256,
            created_by, created_at)
         VALUES ('av-1', 'ad-1', 'v1', 'Recover the boot chain', ?2, 'keys/av-1', ?3, 'operator-1', ?1)",
        &[&now, &sha("task"), &sha("key")],
    );
    e(
        "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, created_at) VALUES ('rub-1', 'av-1', 'Baseline', 'v1', 100, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO rubric_criteria (id, rubric_id, code, title, ordinal, weight, machine_checkable, critical_gate, created_at)
         VALUES ('rc-1', 'rub-1', 'SCOPE', 'Scope compliance', 1, 100, 0, 1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO attempts (id, assessment_version_id, session_id, state, mode, started_at, submitted_at, finalized_at, submission_sha256, created_at, updated_at)
         VALUES ('attempt-final', 'av-1', 'sess-1', 'APPROVED_FINAL', 'EXAM', ?1, ?2, ?3, ?4, ?1, ?3)",
        &[&now, &(now + 1000), &(now + 2000), &sha("submission")],
    );
    e(
        "INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at) VALUES ('attempt-open', 'av-1', 'STARTED', 'EXAM', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO criterion_results (id, attempt_id, criterion_id, score, source, scored_by, scored_at, created_at)
         VALUES ('cr-1', 'attempt-final', 'rc-1', 90, 'HUMAN_REVIEWER', 'operator-1', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO grade_decisions (id, attempt_id, total_score, passed, is_final, decided_by, decided_at, created_at, updated_at)
         VALUES ('gd-1', 'attempt-final', 90, 1, 1, 'operator-1', ?1, ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO retake_links (original_attempt_id, retake_attempt_id, reason, created_by, created_at)
         VALUES ('attempt-final', 'attempt-open', 'retake after validity window', 'operator-1', ?1)",
        &[&now],
    );

    // retention / observations
    e(
        "INSERT INTO competency_observations (id, competency_id, dimension, attempt_id, difficulty, assistance, score, environment, mode,
            reviewer_id, provenance_json, trust, observed_at, created_at)
         VALUES ('obs-1', 'comp-1', 'APPLICATION', 'attempt-final', 3, 'UNAIDED', 90, 'lab-vm', 'EXAM', 'operator-1',
            '{\"source_kind\":\"ASSESSMENT_SUBMISSION\"}', 'OPERATOR', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO review_items (id, kind, competency_id, objective_id, prompt, due_at, created_at, updated_at)
         VALUES ('ri-1', 'DECLARATIVE', 'comp-1', 'o-1', 'What loads the kernel?', ?1, ?1, ?1)",
        &[&now],
    );

    // knowledge / AI
    e(
        "INSERT INTO source_registry (id, domain_or_resource, resource_class, allowed_data, frequency, license, version, state, trust, created_at, updated_at)
         VALUES ('src-1', 'owasp.org/www-project-web-security-testing-guide', 'STANDARD', 'html', 'manual', 'CC-BY-SA-4.0', '4.2',
            'UNTRUSTED', 'UNKNOWN', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO knowledge_documents (id, source_id, title, version, media_type, byte_len, content_sha256, fetched_at, fetched_by, trust, created_at, updated_at)
         VALUES ('doc-1', 'src-1', 'WSTG 4.2', '4.2', 'text/html', 1234, ?2, ?1, 'operator-1', 'IMPORTED_DOCUMENT', ?1, ?1)",
        &[&now, &sha("doc")],
    );
    e(
        "INSERT INTO model_registry (model_id, display_name, architecture, weights_source, license, quantization, checksum, context_length,
            tokenizer_template, runtime_version, created_at, updated_at)
         VALUES ('model-1', 'Local 8B', 'llama', 'local://weights', 'Apache-2.0', 'Q4_K_M', ?2, 8192, 'chatml', 'llama.cpp-b4000', ?1, ?1)",
        &[&now, &sha("weights")],
    );
    e(
        "INSERT INTO skill_registry (skill_id, code, family, title, package_dir, created_at, updated_at)
         VALUES ('g9-session-director', 'T01', 'TEACHING', 'Session director', '.agents/skills/g9-session-director', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO skill_versions (id, skill_id, version_label, package_sha256, skill_md_sha256, input_schema_sha256, output_schema_sha256,
            budget_json, registered_by, registered_at, created_at)
         VALUES ('sv-1', 'g9-session-director', '1.0.0', ?2, ?3, ?4, ?5, '{\"max_tokens\":2048,\"max_seconds\":30,\"max_tool_calls\":0}',
            'operator-1', ?1, ?1)",
        &[&now, &sha("pkg"), &sha("skill.md"), &sha("in"), &sha("out")],
    );

    // business
    e(
        "INSERT INTO goals (id, title, horizon, state, owner_id, created_at, updated_at) VALUES ('goal-1', 'Pass CT9 capstone', 'PROGRAM', 'ACTIVE', 'operator-1', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO projects (id, goal_id, title, state, created_at, updated_at) VALUES ('proj-1', 'goal-1', 'Week 1 lab build', 'ACTIVE', ?1, ?1)",
        &[&now],
    );
    e(
        "INSERT INTO milestones (id, project_id, title, state, ordinal, created_at, updated_at) VALUES ('ms-1', 'proj-1', 'Lab VM ready', 'PLANNED', 1, ?1, ?1)",
        &[&now],
    );
    let refl = "felt focused after the recall block";
    e(
        "INSERT INTO opt_in_reflections (id, session_id, recorded_at, body, body_sha256, created_at, updated_at)
         VALUES ('refl-1', 'sess-1', ?1, ?2, ?3, ?1, ?1)",
        &[&now, &refl, &sha(refl)],
    );

    // governance via the crate API
    let approval = insert_request(
        conn,
        &NewApprovalRequest {
            id: ApprovalId::parse("apr-1").unwrap(),
            operation: OperationDescriptor {
                action: Action::Finalize,
                resource: ResourceRef::new(ResourceKind::GradeDecision, "gd-1"),
                scope: Scope {
                    domain: DataDomain::Learning,
                    mode: InteractionMode::Review,
                },
                parameters_sha256: sha("params"),
            },
            required_capability: Some(g9_domain::Capability::GradeFinalize),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(0),
            expires_at: t(15 * 60_000),
            expected_revision: Some(g9_domain::Revision::new(1)),
        },
    )
    .unwrap();
    for i in 0..3 {
        AuditChain::append(
            conn,
            &AuditEntry {
                occurred_at: t(i),
                actor: operator(),
                operation: "session.start".to_string(),
                resource: Some(ResourceRef::new(ResourceKind::StudySession, "sess-1")),
                decision: AuditDecision::Allow,
                outcome: AuditOutcome::Success,
                request_id: RequestId::parse(&format!("req-{i}")).unwrap(),
                trace_id: TraceId::parse("trace-1").unwrap(),
                details_sha256: Some(sha("details")),
            },
        )
        .unwrap();
    }
    append_event(
        conn,
        &DomainEvent {
            header: EventHeader {
                event_id: EventId::parse("evt-1").unwrap(),
                kind: "assessment.attempt.finalized".to_string(),
                occurred_at: t(5),
                actor: operator(),
                trust: TrustClass::Operator,
                schema_version: SchemaVersion::V1,
                correlation_id: TraceId::parse("trace-1").unwrap(),
                content_sha256: None,
            },
            payload: serde_json::json!({ "attempt_id": "attempt-final", "grade_decision_id": "gd-1" }),
            data_domain: DataDomain::Learning,
            classification: Classification::Internal,
        },
    )
    .unwrap();
    idempotency::store(
        conn,
        &ActorId::parse("operator-1").unwrap(),
        &IdempotencyKey::parse("idem-1").unwrap(),
        &sha("request"),
        &serde_json::json!({ "result": { "session_id": "sess-1" }, "revision": 2 }),
        t(0),
    )
    .unwrap();

    Fixture {
        note_id: "note-1".into(),
        note_version_id: "nv-1".into(),
        attachment_id: "att-1".into(),
        attachment_storage_ref: storage_ref,
        attachment_sha: att_sha,
        evidence_id: "evi-1".into(),
        hypothesis_id: "hyp-1".into(),
        reflection_id: "refl-1".into(),
        session_id: "sess-1".into(),
        attempt_final_id: "attempt-final".into(),
        attempt_open_id: "attempt-open".into(),
        grade_final_id: "gd-1".into(),
        criterion_result_id: "cr-1".into(),
        observation_id: "obs-1".into(),
        approval_id: approval.id,
        curriculum_version_id: "cv-1".into(),
        week_id: "w-1".into(),
        day_id: "d-1".into(),
        objective_id: "o-1".into(),
    }
}

/// Convenience: the row count of a table must equal `expected`.
pub fn assert_count(conn: &Connection, table: &str, expected: i64) {
    assert_eq!(count(conn, table), expected, "table {table}");
}
