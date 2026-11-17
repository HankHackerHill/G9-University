#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Gate tests for g9-storage (A04, A08, A10) plus structural schema checks.
//! Test names are greppable by gate id.

mod common;

use common::*;
use g9_domain::state::{ApprovalEvent, BackupEvent};
use g9_domain::{
    Action, ActorId, ApprovalId, ApprovalState, BackupId, BackupState, Classification, DataDomain,
    Decision, EventHeader, EventId, IdempotencyKey, InteractionMode, OperationDescriptor,
    PolicyDecisionId, ResourceKind, ResourceRef, Revision, SchemaVersion, Scope, TraceId,
    TrustClass, UtcMillis,
};
use g9_storage::approvals_store::{
    self, ApprovalDecisionKind, ApprovalDecisionRow, ApprovalUpdate, PolicyDecisionRow,
};
use g9_storage::audit::{AuditChain, GENESIS_PREV_HASH};
use g9_storage::backup::{
    create_snapshot, record_manifest, restore_test, restore_to_copy, vacuum_into,
    verify_and_advance, verify_snapshot, AttachmentCheck, BackupManifest, SnapshotRequest,
    VerifyOptions,
};
use g9_storage::deletion::{self, DeletionState, DeletionSubjectKind, NewDeletionRequest};
use g9_storage::events::{append_event, list_since, DomainEvent};
use g9_storage::idempotency::{self, IdempotencyCheck};
use g9_storage::migrations::{app_tables, table_counts, Migrator, CURRENT_SCHEMA_VERSION};
use g9_storage::{Database, StorageError};
use pretty_assertions::assert_eq;
use rusqlite::{params, Connection};

fn err_of<T: std::fmt::Debug>(r: Result<T, StorageError>) -> StorageError {
    match r {
        Ok(v) => panic!("expected an error, got {v:?}"),
        Err(e) => e,
    }
}

fn sql_err(r: rusqlite::Result<usize>) -> StorageError {
    match r {
        Ok(n) => panic!("expected an error, statement changed {n} row(s)"),
        Err(e) => e.into(),
    }
}

// ---------------------------------------------------------------- schema structure

#[test]
fn every_table_is_strict() {
    let db = open_mem();
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .unwrap();
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        rows.len() >= 80,
        "expected the full table set, found {}",
        rows.len()
    );
    for (name, sql) in rows {
        assert!(
            sql.trim_end().ends_with("STRICT"),
            "table {name} is not STRICT: {sql}"
        );
    }
}

#[test]
fn every_required_table_group_exists() {
    let db = open_mem();
    let tables = app_tables(db.conn()).unwrap();
    let required = [
        // curriculum
        "curricula",
        "curriculum_versions",
        "weeks",
        "training_days",
        "daily_objectives",
        "study_blocks",
        "resource_references",
        "objective_competencies",
        "deliverable_definitions",
        // calendar/time
        "calendar_events",
        "recurrence_rules",
        "schedule_proposals",
        "schedule_decisions",
        "study_sessions",
        "session_events",
        "time_intervals",
        "manual_time_corrections",
        // evidence
        "notes",
        "note_versions",
        "attachments",
        "artifact_links",
        "evidence_items",
        "source_provenance",
        "content_classifications",
        // assessment
        "assessment_definitions",
        "assessment_versions",
        "rubrics",
        "rubric_criteria",
        "attempts",
        "attempt_evidence",
        "criterion_results",
        "grade_decisions",
        "retake_links",
        "exposure_log",
        // competency
        "competencies",
        "competency_versions",
        "competency_edges",
        "competency_observations",
        "learner_hypotheses",
        "competency_state_snapshots",
        // retention/intervention
        "review_items",
        "review_attempts",
        "practical_transfer_checks",
        "learning_episodes",
        "interventions",
        "intervention_outcomes",
        "method_experiments",
        "calibration_observations",
        // knowledge/AI
        "source_registry",
        "knowledge_documents",
        "knowledge_chunks",
        "knowledge_claims",
        "claim_versions",
        "model_registry",
        "model_evals",
        "skill_registry",
        "skill_versions",
        "skill_invocations",
        "skill_evals",
        // business
        "goals",
        "projects",
        "milestones",
        "milestone_dependencies",
        "commitments",
        "execution_observations",
        "decisions",
        "decision_outcomes",
        "opt_in_reflections",
        // governance
        "approval_requests",
        "approval_decisions",
        "policy_decisions",
        "audit_metadata",
        "data_retention_rules",
        "deletion_requests",
        "deletion_tombstones",
        "backup_manifests",
        "schema_migrations",
        "idempotency_records",
        "domain_events",
        "app_settings",
    ];
    for t in required {
        assert!(tables.iter().any(|x| x == t), "missing table {t}");
    }
    let views: Vec<String> = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'view'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(views.contains(&"misconception_hypotheses".to_string()));
    assert!(views.contains(&"dependencies".to_string()));
}

#[test]
fn every_foreign_key_declares_an_explicit_delete_action() {
    let db = open_mem();
    let mut total = 0;
    for table in app_tables(db.conn()).unwrap() {
        let mut stmt = db
            .conn()
            .prepare(&format!("PRAGMA foreign_key_list(\"{table}\")"))
            .unwrap();
        let rows: Vec<(String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(6)?,
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for (target, from, on_delete) in rows {
            total += 1;
            assert!(
                matches!(on_delete.as_str(), "RESTRICT" | "CASCADE" | "SET NULL"),
                "{table}.{from} -> {target}: on_delete = {on_delete}"
            );
            // CASCADE is only allowed inside the curriculum version's structural tree.
            if on_delete == "CASCADE" {
                assert!(
                    matches!(
                        table.as_str(),
                        "weeks"
                            | "training_days"
                            | "daily_objectives"
                            | "study_blocks"
                            | "resource_references"
                            | "deliverable_definitions"
                            | "objective_competencies"
                    ),
                    "CASCADE outside the curriculum tree: {table}.{from} -> {target}"
                );
            }
        }
    }
    assert!(total > 100, "expected many foreign keys, found {total}");
}

#[test]
fn foreign_keys_enforced() {
    let db = open_mem();
    let e = sql_err(db.conn().execute(
        "INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week, planned_minutes, title, created_at)
         VALUES ('d-x', 'no-such-week', 'no-such-version', 'D01', 1, 1, 420, 'x', 1)",
        [],
    ));
    assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
    let fk: i64 = db
        .conn()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn wal_mode_on_file_db() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal.sqlite");
    let db = open_file(&path);
    assert_eq!(db.profile().journal_mode.to_lowercase(), "wal");
    assert!(db.profile().foreign_keys);
    assert!(!db.profile().trusted_schema);
    assert_eq!(db.profile().synchronous, 1);
    assert_eq!(db.profile().busy_timeout_ms, 5000);
    assert_eq!(db.profile().temp_store, 2);
    assert_eq!(db.profile().secure_delete, 1);
    db.conn()
        .execute("INSERT INTO app_settings (key, value_json, created_at, updated_at) VALUES ('k', '1', 1, 1)", [])
        .unwrap();
    assert!(
        path.with_extension("sqlite-wal").exists(),
        "WAL file should exist while the connection is open"
    );
    let cp = db.checkpoint_truncate().unwrap();
    assert_eq!(cp.busy, 0);
    assert_eq!(
        db.migration_report().unwrap().schema_version_after,
        CURRENT_SCHEMA_VERSION
    );
    assert_count(
        db.conn(),
        "schema_migrations",
        i64::from(CURRENT_SCHEMA_VERSION),
    );
}

// ---------------------------------------------------------------- A04 immutability

#[test]
fn a04_trigger_blocks_final_grade_update() {
    let db = open_mem();
    let fx = populate(db.conn());
    let e = sql_err(db.conn().execute(
        "UPDATE grade_decisions SET total_score = 10, passed = 0 WHERE id = ?1",
        params![fx.grade_final_id],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    let e = sql_err(db.conn().execute(
        "DELETE FROM grade_decisions WHERE id = ?1",
        params![fx.grade_final_id],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    // a superseding decision is the only way to correct
    db.conn()
        .execute(
            "INSERT INTO grade_decisions (id, attempt_id, total_score, passed, is_final, decided_by, decided_at, supersedes, created_at, updated_at)
             VALUES ('gd-2', 'attempt-final', 85, 1, 1, 'operator-1', ?1, 'gd-1', ?1, ?1)",
            params![T0 + 10],
        )
        .unwrap();
    let score: i64 = db
        .conn()
        .query_row(
            "SELECT total_score FROM grade_decisions WHERE id = 'gd-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(score, 90);
}

#[test]
fn a04_trigger_blocks_finalized_attempt_update() {
    let db = open_mem();
    let fx = populate(db.conn());
    let e = sql_err(db.conn().execute(
        "UPDATE attempts SET state = 'STARTED', finalized_at = NULL WHERE id = ?1",
        params![fx.attempt_final_id],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    // non-final attempts may still progress
    db.conn()
        .execute(
            "UPDATE attempts SET state = 'SUBMITTED', submitted_at = ?2, revision = revision + 1, updated_at = ?2 WHERE id = ?1",
            params![fx.attempt_open_id, T0 + 5],
        )
        .unwrap();
    let e = sql_err(db.conn().execute(
        "DELETE FROM attempts WHERE id = ?1",
        params![fx.attempt_open_id],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
}

#[test]
fn a04_trigger_blocks_audit_row_mutation() {
    let db = open_mem();
    populate(db.conn());
    let e = sql_err(db.conn().execute(
        "UPDATE audit_metadata SET operation = 'x' WHERE seq = 1",
        [],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    let e = sql_err(
        db.conn()
            .execute("DELETE FROM audit_metadata WHERE seq = 1", []),
    );
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    assert!(AuditChain::verify_chain(db.conn()).unwrap().ok);
}

#[test]
fn a04_immutable_tables_reject_update_and_delete() {
    let db = open_mem();
    let fx = populate(db.conn());
    approvals_store::insert_decision(
        db.conn(),
        &ApprovalDecisionRow {
            id: "dec-1".into(),
            approval_id: fx.approval_id.clone(),
            decision: ApprovalDecisionKind::Approved,
            decided_by: ActorId::parse("operator-1").unwrap(),
            decided_at: t(1),
            reason: None,
            parameters_sha256: sha("params"),
            expected_revision: Some(Revision::new(1)),
            binding_mac: Some(sha("mac")),
        },
    )
    .unwrap();
    approvals_store::insert_policy_decision(
        db.conn(),
        &PolicyDecisionRow {
            id: PolicyDecisionId::parse("pd-1").unwrap(),
            decided_at: t(1),
            actor: operator(),
            operation: OperationDescriptor {
                action: Action::Finalize,
                resource: ResourceRef::new(ResourceKind::GradeDecision, "gd-1"),
                scope: Scope {
                    domain: DataDomain::Learning,
                    mode: InteractionMode::Review,
                },
                parameters_sha256: sha("params"),
            },
            decision: Decision::Allow {
                rule_id: "R-ALLOW".into(),
            },
            approval_id: Some(fx.approval_id.clone()),
            request_id: None,
            trace_id: None,
        },
    )
    .unwrap();
    let cases = [
        (
            "policy_decisions",
            "UPDATE policy_decisions SET rule_id = 'x' WHERE id = 'pd-1'",
            "DELETE FROM policy_decisions WHERE id = 'pd-1'",
        ),
        (
            "approval_decisions",
            "UPDATE approval_decisions SET decision = 'REJECTED' WHERE id = 'dec-1'",
            "DELETE FROM approval_decisions WHERE id = 'dec-1'",
        ),
        (
            "competency_observations",
            "UPDATE competency_observations SET score = 1 WHERE id = 'obs-1'",
            "DELETE FROM competency_observations WHERE id = 'obs-1'",
        ),
        (
            "criterion_results",
            "UPDATE criterion_results SET score = 1 WHERE id = 'cr-1'",
            "DELETE FROM criterion_results WHERE id = 'cr-1'",
        ),
        (
            "domain_events",
            "UPDATE domain_events SET kind = 'x' WHERE id = 'evt-1'",
            "DELETE FROM domain_events WHERE id = 'evt-1'",
        ),
    ];
    for (table, upd, del) in cases {
        let e = sql_err(db.conn().execute(upd, []));
        assert!(
            matches!(e, StorageError::Immutable(_)),
            "{table} update: {e:?}"
        );
        let e = sql_err(db.conn().execute(del, []));
        assert!(
            matches!(e, StorageError::Immutable(_)),
            "{table} delete: {e:?}"
        );
    }
}

#[test]
fn a04_trigger_blocks_terminal_approval_update() {
    let db = open_mem();
    let fx = populate(db.conn());
    let approved = approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(1),
        &ApprovalUpdate {
            event: ApprovalEvent::Approve,
            actor: Some(ActorId::parse("operator-1").unwrap()),
            at: t(1),
            binding_mac: Some(sha("mac")),
        },
    )
    .unwrap();
    assert_eq!(approved.state, ApprovalState::Approved);
    let consumed = approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        approved.revision,
        &ApprovalUpdate {
            event: ApprovalEvent::Consume,
            actor: Some(ActorId::parse("operator-1").unwrap()),
            at: t(2),
            binding_mac: None,
        },
    )
    .unwrap();
    assert_eq!(consumed.state, ApprovalState::Consumed);
    assert_eq!(consumed.consumed_at, Some(t(2)));
    // raw attempt to resurrect a consumed token
    let e = sql_err(db.conn().execute(
        "UPDATE approval_requests SET state = 'APPROVED', consumed_at = NULL WHERE id = ?1",
        params![fx.approval_id.as_str()],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    let e = sql_err(db.conn().execute(
        "DELETE FROM approval_requests WHERE id = ?1",
        params![fx.approval_id.as_str()],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
}

#[test]
fn a04_model_output_cannot_be_competency_observation() {
    let db = open_mem();
    populate(db.conn());
    let e = sql_err(db.conn().execute(
        "INSERT INTO competency_observations (id, competency_id, dimension, attempt_id, difficulty, assistance, score, environment, mode,
            provenance_json, trust, observed_at, created_at)
         VALUES ('obs-model', 'comp-1', 'APPLICATION', 'attempt-final', 3, 'UNAIDED', 100, 'chat', 'STUDY', '{}', 'MODEL_OUTPUT', 1, 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
}

// ---------------------------------------------------------------- audit chain

#[test]
fn audit_chain_detects_tamper() {
    let db = open_mem();
    populate(db.conn());
    let clean = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(clean.ok);
    assert_eq!(clean.rows, 3);
    let head = clean.head_hash.clone().unwrap();

    // A crafted row inserted through a raw statement (bypassing the API) with a wrong prev_hash.
    db.conn()
        .execute(
            "INSERT INTO audit_metadata (id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, operation, decision, outcome, request_id, trace_id)
             VALUES ('forged-1', ?1, ?2, 1, 'attacker', 'MODEL', 'grade.finalize', 'ALLOW', 'SUCCESS', 'r', 't')",
            params![GENESIS_PREV_HASH, sha("whatever")],
        )
        .unwrap();
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok);
    assert_eq!(report.first_bad_seq, Some(4));
    assert_eq!(report.head_hash.as_deref(), Some(head.as_str()));
    assert!(report.first_bad_reason.unwrap().contains("prev_hash"));
}

#[test]
fn audit_chain_detects_wrong_row_hash_even_with_valid_prev_hash() {
    let db = open_mem();
    populate(db.conn());
    let (_, head) = AuditChain::head(db.conn()).unwrap().unwrap();
    db.conn()
        .execute(
            "INSERT INTO audit_metadata (id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, operation, decision, outcome, request_id, trace_id)
             VALUES ('forged-2', ?1, ?2, 1, 'attacker', 'MODEL', 'grade.finalize', 'ALLOW', 'SUCCESS', 'r', 't')",
            params![head, sha("not-the-real-hash")],
        )
        .unwrap();
    let report = AuditChain::verify_chain(db.conn()).unwrap();
    assert!(!report.ok);
    assert_eq!(report.first_bad_seq, Some(4));
    assert!(report.first_bad_reason.unwrap().contains("row_hash"));
}

#[test]
fn audit_chain_links_from_genesis_and_round_trips() {
    let db = open_mem();
    populate(db.conn());
    let first_prev: String = db
        .conn()
        .query_row(
            "SELECT prev_hash FROM audit_metadata WHERE seq = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(first_prev, GENESIS_PREV_HASH);
    let id: String = db
        .conn()
        .query_row("SELECT id FROM audit_metadata WHERE seq = 2", [], |r| {
            r.get(0)
        })
        .unwrap();
    let (record, entry) = AuditChain::get(db.conn(), &id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(record.seq, 2);
    assert_eq!(entry.operation, "session.start");
    assert_eq!(entry.actor, operator());
    assert_eq!(
        entry.resource,
        Some(ResourceRef::new(ResourceKind::StudySession, "sess-1"))
    );
}

// ---------------------------------------------------------------- deletion semantics

#[test]
fn deletion_restrict_on_evidentiary_rows() {
    let db = open_mem();
    let fx = populate(db.conn());
    // attempts are protected by trigger; their parents by RESTRICT
    let e = sql_err(
        db.conn()
            .execute("DELETE FROM assessment_versions WHERE id = 'av-1'", []),
    );
    assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
    let e = sql_err(
        db.conn()
            .execute("DELETE FROM competencies WHERE id = 'comp-1'", []),
    );
    assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
    // The session is referenced by a finalized attempt through an informational SET NULL link; the
    // A04 trigger freezes the finalized attempt row entirely, so even the SET NULL is refused.
    let e = sql_err(db.conn().execute(
        "DELETE FROM study_sessions WHERE id = ?1",
        params![fx.session_id],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    assert_count(db.conn(), "study_sessions", 1);
    let e = sql_err(
        db.conn()
            .execute("DELETE FROM notes WHERE id = ?1", params![fx.note_id]),
    );
    assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
    let e = sql_err(
        db.conn()
            .execute("DELETE FROM rubric_criteria WHERE id = 'rc-1'", []),
    );
    assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
    assert_count(db.conn(), "attempts", 2);
    assert_count(db.conn(), "criterion_results", 1);
}

#[test]
fn curriculum_cascade_stays_inside_the_version() {
    let db = open_mem();
    let fx = populate(db.conn());
    db.conn()
        .execute(
            "DELETE FROM curriculum_versions WHERE id = ?1",
            params![fx.curriculum_version_id],
        )
        .unwrap();
    assert_count(db.conn(), "weeks", 0);
    assert_count(db.conn(), "training_days", 0);
    assert_count(db.conn(), "daily_objectives", 0);
    assert_count(db.conn(), "study_blocks", 0);
    assert_count(db.conn(), "objective_competencies", 0);
    // informational links were nulled, evidence rows survived
    let day: Option<String> = db
        .conn()
        .query_row(
            "SELECT training_day_id FROM study_sessions WHERE id = 'sess-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(day, None);
    assert_count(db.conn(), "curricula", 1);
    assert_count(db.conn(), "attempts", 2);
    assert_count(db.conn(), "notes", 1);
}

#[test]
fn curriculum_delete_version_refuses_a_version_that_sessions_and_evidence_link_to() {
    let db = open_mem();
    let fx = populate(db.conn());
    let id = g9_domain::CurriculumVersionId::parse(&fx.curriculum_version_id).unwrap();
    let r = g9_storage::repos::curriculum::delete_version(db.conn(), &id, Revision::INITIAL);
    match r {
        Err(g9_storage::repos::RepoError::Storage(StorageError::ForeignKey(m))) => {
            for link in [
                "study_sessions.training_day_id",
                "study_sessions.objective_id",
                "calendar_events.training_day_id",
                "notes.objective_id",
                "assessment_definitions.objective_id",
            ] {
                assert!(m.contains(link), "{link} missing from {m}");
            }
        }
        other => panic!("expected a RESTRICT-style refusal, got {other:?}"),
    }
    // nothing was deleted and no link was nulled
    assert_count(db.conn(), "curriculum_versions", 1);
    assert_count(db.conn(), "training_days", 1);
    let links: (Option<String>, Option<String>) = db
        .conn()
        .query_row(
            "SELECT training_day_id, objective_id FROM study_sessions WHERE id = 'sess-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        links,
        (Some(fx.day_id.clone()), Some(fx.objective_id.clone()))
    );
}

#[test]
fn a10_deletion_request_redacts_and_tombstones() {
    let mut db = open_mem();
    let fx = populate(db.conn());
    let req = deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-1".into(),
            subject_kind: DeletionSubjectKind::Note,
            subject_id: fx.note_id.clone(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(10),
            reason: Some("operator request".into()),
            approval_id: Some(fx.approval_id.clone()),
        },
    )
    .unwrap();
    assert_eq!(req.state, DeletionState::Requested);

    let report = deletion::execute_erasure(
        db.conn_mut(),
        "del-1",
        &ActorId::parse("operator-1").unwrap(),
        t(20),
    )
    .unwrap();

    // content redacted in place; its digest replaced (ADR-0017); rows still exist
    let content_sha = sha("# Lab log\nkernel boots via GRUB");
    let (title, body, sha_now, erased, erasure_id): (String, String, String, i64, Option<String>) = db
        .conn()
        .query_row(
            "SELECT title, body_markdown, body_sha256, erased, erasure_request_id FROM notes WHERE id = ?1",
            params![fx.note_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(title, "[REDACTED]");
    assert_eq!(body, "");
    assert_eq!(
        sha_now,
        deletion::tombstone_digest("del-1", "notes", &fx.note_id, t(20))
    );
    assert_ne!(sha_now, content_sha);
    assert_eq!(erased, 1);
    assert_eq!(erasure_id.as_deref(), Some("del-1"));
    let (vbody, verased): (String, i64) = db
        .conn()
        .query_row(
            "SELECT body_markdown, erased FROM note_versions WHERE id = ?1",
            params![fx.note_version_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((vbody.as_str(), verased), ("", 1));
    let (aname, aref, aerased): (String, String, i64) = db
        .conn()
        .query_row(
            "SELECT original_filename, storage_ref, erased FROM attachments WHERE id = ?1",
            params![fx.attachment_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (aname.as_str(), aref.as_str(), aerased),
        ("[REDACTED]", "", 1)
    );

    // a tombstone's digest identifies the tombstone (request, table, row, time) and is
    // never a digest of what was erased: the digest of a short text is the text
    assert_count(db.conn(), "deletion_tombstones", 3);
    let stones: Vec<(String, String, String)> = db
        .conn()
        .prepare(
            "SELECT table_name, row_id, content_sha256_before FROM deletion_tombstones ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for (table, row_id) in [
        ("notes", &fx.note_id),
        ("note_versions", &fx.note_version_id),
        ("attachments", &fx.attachment_id),
    ] {
        let expected = deletion::tombstone_digest("del-1", table, row_id, t(20));
        assert!(
            stones.contains(&(table.to_string(), row_id.clone(), expected)),
            "{table}: {stones:?}"
        );
    }
    for (_, _, digest) in &stones {
        assert!(
            digest != &content_sha && digest != &fx.attachment_sha,
            "a tombstone holds a digest of erased content"
        );
    }
    // and no column of the three rows holds one either
    let leftover: i64 = db
        .conn()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM notes WHERE body_sha256 IN (?1, ?2))
                  + (SELECT COUNT(*) FROM note_versions WHERE body_sha256 IN (?1, ?2))
                  + (SELECT COUNT(*) FROM attachments WHERE content_sha256 IN (?1, ?2) OR instr(storage_ref, ?2) > 0)",
            params![content_sha, fx.attachment_sha],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leftover, 0);
    assert_eq!(report.tombstones.len(), 3);
    assert_eq!(
        report.attachment_storage_refs_to_purge,
        vec![fx.attachment_storage_ref.clone()]
    );

    // derived rows invalidated, not deleted
    let (inv_at, inv_reason): (Option<i64>, Option<String>) = db
        .conn()
        .query_row(
            "SELECT invalidated_at, invalidation_reason FROM evidence_items WHERE id = ?1",
            params![fx.evidence_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(inv_at, Some(T0 + 20));
    assert_eq!(inv_reason.as_deref(), Some("deletion_request:del-1"));
    let hyp_inv: Option<i64> = db
        .conn()
        .query_row(
            "SELECT invalidated_at FROM learner_hypotheses WHERE id = ?1",
            params![fx.hypothesis_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        hyp_inv,
        Some(T0 + 20),
        "hypothesis citing the note must be invalidated"
    );
    assert!(report
        .invalidated_derived
        .iter()
        .any(|r| r.table_name == "evidence_items"));
    assert!(report
        .invalidated_derived
        .iter()
        .any(|r| r.table_name == "learner_hypotheses"));

    // request is EXECUTED, terminal and immutable; evidentiary rows untouched
    let done = deletion::get_request(db.conn(), "del-1").unwrap().unwrap();
    assert_eq!(done.state, DeletionState::Executed);
    assert_eq!(done.executed_at, Some(t(20)));
    // The stored report is the returned one without the files to remove: their paths
    // are digests of the erased content, which no table keeps (ADR-0017).
    let stored = done.report.unwrap();
    assert!(stored.attachment_storage_refs_to_purge.is_empty());
    assert_eq!(
        stored,
        deletion::ErasureReport {
            attachment_storage_refs_to_purge: Vec::new(),
            ..report.clone()
        }
    );
    let e = sql_err(db.conn().execute(
        "UPDATE deletion_requests SET state = 'REQUESTED' WHERE id = 'del-1'",
        [],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    let e = sql_err(db.conn().execute("DELETE FROM deletion_tombstones", []));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
    assert_count(db.conn(), "attempts", 2);
    assert_count(db.conn(), "audit_metadata", 3);
    assert!(AuditChain::verify_chain(db.conn()).unwrap().ok);

    // a second request on an erased subject is refused
    let e = err_of(deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-2".into(),
            subject_kind: DeletionSubjectKind::Note,
            subject_id: fx.note_id.clone(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(30),
            reason: None,
            approval_id: None,
        },
    ));
    assert!(matches!(e, StorageError::Conflict(_)), "{e:?}");
}

#[test]
fn a10_erasure_never_lists_bytes_a_live_attachment_still_uses() {
    let mut db = open_mem();
    let fx = populate(db.conn());
    // identical bytes → identical content-addressed path, shared by att-1 and att-2
    db.conn()
        .execute(
            "INSERT INTO attachments (id, note_id, original_filename, media_type, size_bytes, content_sha256, storage_ref,
                source_kind, trust, created_at, updated_at)
             VALUES ('att-2', NULL, 'copy.pcap', 'application/vnd.tcpdump.pcap', 10, ?1, ?2, 'OPERATOR_INPUT', 'OPERATOR', ?3, ?3)",
            params![fx.attachment_sha, fx.attachment_storage_ref, T0],
        )
        .unwrap();
    let erase = |db: &mut Database, req: &str, subject: &str, at: i64| {
        deletion::create_request(
            db.conn(),
            &NewDeletionRequest {
                id: req.into(),
                subject_kind: DeletionSubjectKind::Attachment,
                subject_id: subject.into(),
                requested_by: ActorId::parse("operator-1").unwrap(),
                requested_at: t(at),
                reason: None,
                approval_id: Some(fx.approval_id.clone()),
            },
        )
        .unwrap();
        deletion::execute_erasure(
            db.conn_mut(),
            req,
            &ActorId::parse("operator-1").unwrap(),
            t(at + 1),
        )
        .unwrap()
    };

    // att-2 still uses the bytes: erasing att-1 must not hand the path out for purging
    let first = erase(&mut db, "del-a1", &fx.attachment_id, 10);
    assert_eq!(first.tombstones.len(), 1);
    assert!(
        first.attachment_storage_refs_to_purge.is_empty(),
        "{first:?}"
    );
    let live: (String, i64) = db
        .conn()
        .query_row(
            "SELECT storage_ref, erased FROM attachments WHERE id = 'att-2'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(live, (fx.attachment_storage_ref.clone(), 0));

    // the last erasure of those bytes lists the path exactly once
    let last = erase(&mut db, "del-a2", "att-2", 20);
    assert_eq!(
        last.attachment_storage_refs_to_purge,
        vec![fx.attachment_storage_ref.clone()]
    );
}

#[test]
fn a10_approved_requests_whose_erasure_did_not_run_are_listed_until_the_subject_is_erased() {
    let mut db = open_mem();
    let fx = populate(db.conn());
    let operator = ActorId::parse("operator-1").unwrap();
    deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-1".into(),
            subject_kind: DeletionSubjectKind::Note,
            subject_id: fx.note_id.clone(),
            requested_by: operator.clone(),
            requested_at: t(10),
            reason: None,
            approval_id: Some(fx.approval_id.clone()),
        },
    )
    .unwrap();
    // A request without a spent approval is not listed (nothing was approved and spent yet).
    deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-unapproved".into(),
            subject_kind: DeletionSubjectKind::OptInReflection,
            subject_id: fx.reflection_id.clone(),
            requested_by: operator.clone(),
            requested_at: t(11),
            reason: None,
            approval_id: None,
        },
    )
    .unwrap();
    assert!(deletion::list_approved_unexecuted(db.conn())
        .unwrap()
        .is_empty());
    let approved = approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(1),
        &ApprovalUpdate {
            event: ApprovalEvent::Approve,
            actor: Some(operator.clone()),
            at: t(1),
            binding_mac: Some(sha("mac")),
        },
    )
    .unwrap();
    assert!(
        deletion::list_approved_unexecuted(db.conn())
            .unwrap()
            .is_empty(),
        "approved but not consumed"
    );
    approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        approved.revision,
        &ApprovalUpdate {
            event: ApprovalEvent::Consume,
            actor: Some(operator.clone()),
            at: t(2),
            binding_mac: None,
        },
    )
    .unwrap();
    let listed: Vec<String> = deletion::list_approved_unexecuted(db.conn())
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(listed, vec!["del-1".to_string()]);

    deletion::execute_erasure(db.conn_mut(), "del-1", &operator, t(20)).unwrap();
    assert!(deletion::list_approved_unexecuted(db.conn())
        .unwrap()
        .is_empty());
}

#[test]
fn a10_reflection_erasure_and_rejected_requests() {
    let mut db = open_mem();
    let fx = populate(db.conn());
    // Wellbeing scores are personal content of the reflection (storage-core-11).
    db.conn()
        .execute(
            "UPDATE opt_in_reflections SET energy_level = 1, focus_level = 2 WHERE id = ?1",
            params![fx.reflection_id],
        )
        .unwrap();
    deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-r".into(),
            subject_kind: DeletionSubjectKind::OptInReflection,
            subject_id: fx.reflection_id.clone(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(1),
            reason: None,
            approval_id: None,
        },
    )
    .unwrap();
    let report = deletion::execute_erasure(
        db.conn_mut(),
        "del-r",
        &ActorId::parse("operator-1").unwrap(),
        t(2),
    )
    .unwrap();
    assert_eq!(report.tombstones.len(), 1);
    let erased: (String, Option<i64>, Option<i64>) = db
        .conn()
        .query_row(
            "SELECT body, energy_level, focus_level FROM opt_in_reflections WHERE id = ?1",
            params![fx.reflection_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(erased, (String::new(), None, None));
    // executing twice is refused
    let e = err_of(deletion::execute_erasure(
        db.conn_mut(),
        "del-r",
        &ActorId::parse("operator-1").unwrap(),
        t(3),
    ));
    assert!(matches!(e, StorageError::Precondition(_)), "{e:?}");
    // reject path
    deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-a".into(),
            subject_kind: DeletionSubjectKind::Attachment,
            subject_id: fx.attachment_id.clone(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(1),
            reason: None,
            approval_id: None,
        },
    )
    .unwrap();
    let rejected = deletion::reject_request(
        db.conn(),
        "del-a",
        &ActorId::parse("operator-1").unwrap(),
        t(4),
    )
    .unwrap();
    assert_eq!(rejected.state, DeletionState::Rejected);
    let erased: i64 = db
        .conn()
        .query_row(
            "SELECT erased FROM attachments WHERE id = ?1",
            params![fx.attachment_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(erased, 0);
    // unknown subject
    let e = err_of(deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-x".into(),
            subject_kind: DeletionSubjectKind::Note,
            subject_id: "nope".into(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(1),
            reason: None,
            approval_id: None,
        },
    ));
    assert!(matches!(e, StorageError::NotFound(_)), "{e:?}");
}

#[test]
fn a10_study_session_erasure_is_explicitly_not_implemented() {
    let mut db = open_mem();
    let fx = populate(db.conn());
    deletion::create_request(
        db.conn(),
        &NewDeletionRequest {
            id: "del-s".into(),
            subject_kind: DeletionSubjectKind::StudySession,
            subject_id: fx.session_id.clone(),
            requested_by: ActorId::parse("operator-1").unwrap(),
            requested_at: t(1),
            reason: None,
            approval_id: None,
        },
    )
    .unwrap();
    let e = err_of(deletion::execute_erasure(
        db.conn_mut(),
        "del-s",
        &ActorId::parse("operator-1").unwrap(),
        t(2),
    ));
    assert_eq!(e, StorageError::NotImplemented("deletion.study_session"));
    let g: g9_domain::G9Error = e.into();
    assert_eq!(g.code, "deletion.study_session.not_implemented");
    assert_eq!(g.category, g9_domain::ErrorCategory::PreconditionFailed);
}

// ---------------------------------------------------------------- idempotency

#[test]
fn idempotency_roundtrip() {
    let db = open_mem();
    let actor = ActorId::parse("operator-1").unwrap();
    let key = IdempotencyKey::parse("cmd-42").unwrap();
    let request = serde_json::json!({ "operation": "session.start", "session_id": "sess-1" });
    let hash = idempotency::request_hash(&request);
    assert_eq!(
        idempotency::check(db.conn(), &actor, &key, &hash).unwrap(),
        IdempotencyCheck::Miss
    );
    let outcome = serde_json::json!({ "result": { "state": "RUNNING" }, "revision": 2 });
    idempotency::store(db.conn(), &actor, &key, &hash, &outcome, t(1)).unwrap();
    // same request → replay with the stored outcome
    match idempotency::check(db.conn(), &actor, &key, &hash).unwrap() {
        IdempotencyCheck::Replay(stored) => {
            assert_eq!(stored.outcome_json, outcome);
            assert_eq!(stored.request_sha256, hash);
            assert_eq!(stored.created_at, t(1));
        }
        other => panic!("{other:?}"),
    }
    // storing the same thing again is a no-op
    idempotency::store(db.conn(), &actor, &key, &hash, &outcome, t(2)).unwrap();
    // different request under the same key → conflict
    let other_hash =
        idempotency::request_hash(&serde_json::json!({ "operation": "session.cancel" }));
    assert!(matches!(
        idempotency::check(db.conn(), &actor, &key, &other_hash).unwrap(),
        IdempotencyCheck::Conflict { .. }
    ));
    let e = err_of(idempotency::store(
        db.conn(),
        &actor,
        &key,
        &other_hash,
        &outcome,
        t(3),
    ));
    assert!(matches!(e, StorageError::Conflict(_)));
    // different actor, same key → independent
    let other_actor = ActorId::parse("skill-1").unwrap();
    assert_eq!(
        idempotency::check(db.conn(), &other_actor, &key, &hash).unwrap(),
        IdempotencyCheck::Miss
    );
    // retention purge
    assert_eq!(idempotency::purge_older_than(db.conn(), t(1)).unwrap(), 0);
    assert_eq!(idempotency::purge_older_than(db.conn(), t(2)).unwrap(), 1);
    assert_eq!(idempotency::lookup(db.conn(), &actor, &key).unwrap(), None);
}

// ---------------------------------------------------------------- events

#[test]
fn events_append_list_and_refuse_secret() {
    let db = open_mem();
    populate(db.conn());
    let stored = list_since(db.conn(), 0, 10).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].event.header.kind, "assessment.attempt.finalized");
    assert_eq!(stored[0].event.header.schema_version, SchemaVersion::V1);
    assert_eq!(
        stored[0].payload_sha256,
        g9_domain::hashing::sha256_json(&stored[0].event.payload)
    );
    let secret = DomainEvent {
        header: EventHeader {
            event_id: EventId::parse("evt-secret").unwrap(),
            kind: "assessment.key.exposed".into(),
            occurred_at: t(1),
            actor: operator(),
            trust: TrustClass::SystemDeterministic,
            schema_version: SchemaVersion::V1,
            correlation_id: TraceId::parse("trace-2").unwrap(),
            content_sha256: None,
        },
        payload: serde_json::json!({ "key": "do-not-store" }),
        data_domain: DataDomain::Learning,
        classification: Classification::Secret,
    };
    let e = err_of(append_event(db.conn(), &secret));
    assert!(matches!(e, StorageError::InvalidInput(_)));
    assert_count(db.conn(), "domain_events", 1);
}

// ---------------------------------------------------------------- approvals store

#[test]
fn approvals_store_transitions_check_revision_and_machine() {
    let db = open_mem();
    let fx = populate(db.conn());
    let op = ActorId::parse("operator-1").unwrap();
    // stale revision refused
    let e = err_of(approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(7),
        &ApprovalUpdate {
            event: ApprovalEvent::Approve,
            actor: Some(op.clone()),
            at: t(1),
            binding_mac: None,
        },
    ));
    assert!(matches!(e, StorageError::StaleRevision { .. }), "{e:?}");
    // illegal edge refused (Pending + Consume)
    let e = err_of(approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(1),
        &ApprovalUpdate {
            event: ApprovalEvent::Consume,
            actor: Some(op.clone()),
            at: t(1),
            binding_mac: None,
        },
    ));
    assert!(matches!(e, StorageError::InvalidTransition(_)), "{e:?}");
    // approve requires an actor
    let e = err_of(approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(1),
        &ApprovalUpdate {
            event: ApprovalEvent::Approve,
            actor: None,
            at: t(1),
            binding_mac: None,
        },
    ));
    assert!(matches!(e, StorageError::InvalidInput(_)), "{e:?}");
    let approved = approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(1),
        &ApprovalUpdate {
            event: ApprovalEvent::Approve,
            actor: Some(op.clone()),
            at: t(1),
            binding_mac: Some(sha("mac")),
        },
    )
    .unwrap();
    assert_eq!(approved.revision, Revision::new(2));
    assert_eq!(approved.binding_mac, Some(sha("mac")));
    assert_eq!(approved.decided_by, Some(op.clone()));
    assert_eq!(
        approvals_store::list_by_state(db.conn(), ApprovalState::Approved)
            .unwrap()
            .len(),
        1
    );
    // consume once ...
    let consumed = approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(2),
        &ApprovalUpdate {
            event: ApprovalEvent::Consume,
            actor: Some(op.clone()),
            at: t(2),
            binding_mac: None,
        },
    )
    .unwrap();
    assert_eq!(consumed.state, ApprovalState::Consumed);
    // ... never twice
    let e = err_of(approvals_store::transition_request(
        db.conn(),
        &fx.approval_id,
        Revision::new(3),
        &ApprovalUpdate {
            event: ApprovalEvent::Consume,
            actor: Some(op),
            at: t(3),
            binding_mac: None,
        },
    ));
    assert!(matches!(e, StorageError::InvalidTransition(_)), "{e:?}");
}

#[test]
fn approvals_expire_due_and_policy_decisions_list() {
    let db = open_mem();
    let fx = populate(db.conn());
    assert!(approvals_store::expire_due(db.conn(), t(14 * 60_000))
        .unwrap()
        .is_empty());
    let expired = approvals_store::expire_due(db.conn(), t(15 * 60_000)).unwrap();
    assert_eq!(expired, vec![fx.approval_id.clone()]);
    let row = approvals_store::get_request(db.conn(), &fx.approval_id)
        .unwrap()
        .unwrap();
    assert_eq!(row.state, ApprovalState::Expired);
    assert_eq!(row.revision, Revision::new(2));
    let resource = ResourceRef::new(ResourceKind::Attempt, "attempt-open");
    for (i, decision) in [
        Decision::Deny {
            rule_id: "R-DENY-DEFAULT".into(),
            reason: "no rule matched".into(),
        },
        Decision::ApprovalRequired {
            rule_id: "R-FINALIZE".into(),
            reason: "finalization needs GradeFinalize".into(),
            required_capability: g9_domain::Capability::GradeFinalize,
        },
    ]
    .into_iter()
    .enumerate()
    {
        approvals_store::insert_policy_decision(
            db.conn(),
            &PolicyDecisionRow {
                id: PolicyDecisionId::parse(&format!("pd-{i}")).unwrap(),
                decided_at: t(i as i64),
                actor: g9_domain::Actor::model(ActorId::parse("model").unwrap()),
                operation: OperationDescriptor {
                    action: Action::Finalize,
                    resource: resource.clone(),
                    scope: Scope {
                        domain: DataDomain::Learning,
                        mode: InteractionMode::Exam,
                    },
                    parameters_sha256: sha("p"),
                },
                decision,
                approval_id: None,
                request_id: Some(g9_domain::RequestId::parse("req").unwrap()),
                trace_id: Some(TraceId::parse("trace").unwrap()),
            },
        )
        .unwrap();
    }
    let listed = approvals_store::list_policy_decisions_for(db.conn(), &resource, 10).unwrap();
    assert_eq!(listed.len(), 2);
    assert!(matches!(
        listed[0].decision,
        Decision::ApprovalRequired { .. }
    ));
    assert!(matches!(listed[1].decision, Decision::Deny { .. }));
    assert_eq!(listed[0].actor.kind, g9_domain::ActorKind::Model);
}

// ---------------------------------------------------------------- A08 backup / migration

#[test]
fn a08_snapshot_verify_restore_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let live_path = dir.path().join("live.sqlite");
    let db = open_file(&live_path);
    let fx = populate(db.conn());
    let live_counts = table_counts(db.conn()).unwrap();
    let live_head = AuditChain::head(db.conn()).unwrap().unwrap();

    // attachment store with the one fixture file present
    let att_root = dir.path().join("attachments");
    let att_path = att_root.join(&fx.attachment_storage_ref);
    std::fs::create_dir_all(att_path.parent().unwrap()).unwrap();
    std::fs::write(&att_path, b"pcap-bytes").unwrap();
    let verify = VerifyOptions {
        attachment_root: Some(att_root.clone()),
    };

    let backup_path = dir.path().join("backups").join("b1.sqlite");
    std::fs::create_dir_all(backup_path.parent().unwrap()).unwrap();
    let request = SnapshotRequest {
        backup_id: BackupId::parse("b1").unwrap(),
        requested_at: t(100),
        app_version: "0.1.0-test".into(),
    };
    let mut manifest = create_snapshot(db.conn(), &backup_path, &request).unwrap();
    assert_eq!(manifest.state, BackupState::SnapshotCreated);
    assert_eq!(manifest.schema_version, CURRENT_SCHEMA_VERSION);
    assert!(manifest.page_count > 0);
    assert_eq!(manifest.audit_anchor.as_ref().unwrap().seq, live_head.0);
    assert_eq!(
        manifest.audit_anchor.as_ref().unwrap().row_hash,
        live_head.1
    );
    assert!(backup_path.is_file());
    assert!(!backup_path.with_extension("partial").exists());

    let report = verify_and_advance(&mut manifest, &verify).unwrap();
    assert!(report.ok(), "{report:?}");
    assert!(
        report.integrity_check_ok
            && report.foreign_key_check_ok
            && report.migration_checksums_ok
            && report.audit_chain_ok
    );
    assert_eq!(report.table_counts, live_counts);
    assert_eq!(
        report.attachment_manifest,
        AttachmentCheck::Checked {
            expected: 1,
            missing: vec![],
            damaged: 0
        }
    );
    assert_eq!(manifest.state, BackupState::Checked);

    let restored = restore_test(&mut manifest, dir.path(), &verify, t(200)).unwrap();
    assert_eq!(manifest.state, BackupState::RestoreTested);
    assert_eq!(manifest.restore_tested_at, Some(t(200)));
    assert!(restored.is_file());
    let restored_db = Database::open(&restored, opts().no_auto_migrate()).unwrap();
    assert_eq!(table_counts(restored_db.conn()).unwrap(), live_counts);
    assert!(AuditChain::verify_chain(restored_db.conn()).unwrap().ok);
    assert_eq!(
        Migrator::schema_version(restored_db.conn()).unwrap(),
        CURRENT_SCHEMA_VERSION
    );
    drop(restored_db);

    manifest.retain().unwrap();
    assert_eq!(manifest.state, BackupState::Retained);
    record_manifest(db.conn(), &manifest, t(300)).unwrap();
    assert_eq!(
        g9_storage::backup::manifest_state(db.conn(), &request.backup_id).unwrap(),
        Some(BackupState::Retained)
    );

    // original untouched: counts identical (plus the manifest row), chain intact, snapshot hash unchanged
    let mut expected = live_counts.clone();
    *expected.get_mut("backup_manifests").unwrap() += 1;
    assert_eq!(table_counts(db.conn()).unwrap(), expected);
    assert!(AuditChain::verify_chain(db.conn()).unwrap().ok);
    assert_eq!(
        g9_storage::backup::sha256_of_file(&backup_path).unwrap(),
        manifest.sha256_of_file
    );

    // a missing attachment file is reported, never silently passed
    std::fs::remove_file(&att_path).unwrap();
    let report = verify_snapshot(&backup_path, &verify).unwrap();
    assert!(!report.ok());
    assert_eq!(
        report.attachment_manifest,
        AttachmentCheck::Checked {
            expected: 1,
            missing: vec![fx.attachment_storage_ref.clone()],
            damaged: 0
        }
    );
    // without an attachment root the manifest is explicitly NOT checked
    let report = verify_snapshot(&backup_path, &VerifyOptions::default()).unwrap();
    assert_eq!(report.attachment_manifest, AttachmentCheck::NotChecked);
    assert!(report.ok());
}

#[test]
fn a08_snapshot_refuses_overwrite_and_live_path() {
    let dir = tempfile::tempdir().unwrap();
    let live_path = dir.path().join("live.sqlite");
    let db = open_file(&live_path);
    populate(db.conn());
    let request = SnapshotRequest {
        backup_id: BackupId::parse("b2").unwrap(),
        requested_at: t(1),
        app_version: "0.1.0-test".into(),
    };
    let e = err_of(create_snapshot(db.conn(), &live_path, &request));
    assert!(matches!(e, StorageError::InvalidInput(_)), "{e:?}");
    let dest = dir.path().join("b2.sqlite");
    let created = create_snapshot(db.conn(), &dest, &request).unwrap();
    let e = err_of(create_snapshot(db.conn(), &dest, &request));
    assert!(matches!(e, StorageError::Conflict(_)), "{e:?}");
    let e = err_of(restore_to_copy(&dest, &dest));
    assert!(
        matches!(e, StorageError::Conflict(_) | StorageError::InvalidInput(_)),
        "{e:?}"
    );
    // storage-core-5: verified against the manifest create_snapshot returned
    // (the hash of the file it wrote), the untampered snapshot reaches CHECKED.
    let mut untampered = created.clone();
    verify_and_advance(&mut untampered, &VerifyOptions::default()).unwrap();
    assert_eq!(untampered.state, BackupState::Checked);
    // One flipped byte that no SQLite check reads (the low byte of the header's
    // user_version, offset 60..64): only the recorded hash can tell.
    let mut bytes = std::fs::read(&dest).unwrap();
    bytes[63] ^= 0x01;
    std::fs::write(&dest, &bytes).unwrap();
    assert!(
        verify_snapshot(&dest, &VerifyOptions::default())
            .unwrap()
            .ok(),
        "the flip is invisible to every check but the hash"
    );
    let mut tampered = created.clone();
    let e = err_of(verify_and_advance(&mut tampered, &VerifyOptions::default()));
    assert!(
        matches!(&e, StorageError::Verification(errs)
            if errs == &vec!["snapshot file hash differs from the manifest".to_string()]),
        "{e:?}"
    );
    assert_eq!(tampered.state, BackupState::Failed);
}

fn snapshot_request(id: &str) -> SnapshotRequest {
    SnapshotRequest {
        backup_id: BackupId::parse(id).unwrap(),
        requested_at: t(100),
        app_version: "0.1.0-test".into(),
    }
}

#[test]
fn a08_restore_test_refuses_a_snapshot_replaced_after_verification() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file(&dir.path().join("live.sqlite"));
    populate(db.conn());
    let s0_path = dir.path().join("s0.sqlite");
    let s0 = create_snapshot(db.conn(), &s0_path, &snapshot_request("s0")).unwrap();
    // the live database changes between the two snapshots
    record_manifest(db.conn(), &s0, t(150)).unwrap();
    let s1_path = dir.path().join("s1.sqlite");
    let mut s1 = create_snapshot(db.conn(), &s1_path, &snapshot_request("s1")).unwrap();
    verify_and_advance(&mut s1, &VerifyOptions::default()).unwrap();
    assert_eq!(s1.state, BackupState::Checked);

    // an older, internally consistent database replaces the verified file
    std::fs::remove_file(&s1_path).unwrap();
    std::fs::copy(&s0_path, &s1_path).unwrap();
    assert_ne!(
        g9_storage::backup::sha256_of_file(&s1_path).unwrap(),
        s1.sha256_of_file
    );
    assert!(verify_snapshot(&s1_path, &VerifyOptions::default())
        .unwrap()
        .ok());

    let scratch = dir.path().join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let e = err_of(restore_test(
        &mut s1,
        &scratch,
        &VerifyOptions::default(),
        t(200),
    ));
    assert!(
        matches!(&e, StorageError::Verification(errs) if errs.iter().any(|m| m.contains("hash differs"))),
        "{e:?}"
    );
    assert_eq!(s1.state, BackupState::Failed);
    assert!(s1.restore_tested_at.is_none());
    assert!(s1.retain().is_err(), "a failed set can never be RETAINED");
    // the replaced bytes were never restored
    assert!(!scratch.join("s1.restore-test.sqlite").exists());
}

#[test]
fn a08_restore_test_refuses_an_audit_anchor_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file(&dir.path().join("live.sqlite"));
    populate(db.conn());
    let path = dir.path().join("b-anchor.sqlite");
    let mut manifest = create_snapshot(db.conn(), &path, &snapshot_request("b-anchor")).unwrap();
    verify_and_advance(&mut manifest, &VerifyOptions::default()).unwrap();
    // the manifest claims a chain head the snapshot does not contain
    manifest.audit_anchor = Some(g9_storage::backup::AuditAnchor {
        seq: 999_999,
        row_hash: "0".repeat(64),
    });
    let e = err_of(restore_test(
        &mut manifest,
        dir.path(),
        &VerifyOptions::default(),
        t(200),
    ));
    assert!(
        matches!(&e, StorageError::Verification(errs) if errs.iter().any(|m| m.contains("audit chain head"))),
        "{e:?}"
    );
    assert_eq!(manifest.state, BackupState::Failed);
}

#[test]
fn a08_backup_state_machine_requires_restore_test_before_retain() {
    let request = SnapshotRequest {
        backup_id: BackupId::parse("b3").unwrap(),
        requested_at: t(1),
        app_version: "0.1.0-test".into(),
    };
    let mut m = BackupManifest::requested(&request);
    assert!(m.retain().is_err());
    m.transition(BackupEvent::Snapshot).unwrap();
    assert!(m.retain().is_err());
    m.transition(BackupEvent::Check).unwrap();
    assert!(m.retain().is_err());
    m.transition(BackupEvent::TestRestore).unwrap();
    m.retain().unwrap();
    assert_eq!(m.state, BackupState::Retained);
    m.fail("x").unwrap_err();
    let mut f = BackupManifest::requested(&request);
    f.fail("disk full").unwrap();
    assert_eq!(f.state, BackupState::Failed);
    assert_eq!(f.failure_reason.as_deref(), Some("disk full"));
}

#[test]
fn a08_vacuum_into_export_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file(&dir.path().join("live.sqlite"));
    populate(db.conn());
    let dest = dir.path().join("export.sqlite");
    vacuum_into(db.conn(), &dest).unwrap();
    let report = verify_snapshot(&dest, &VerifyOptions::default()).unwrap();
    assert!(report.ok(), "{report:?}");
    assert_eq!(report.table_counts, table_counts(db.conn()).unwrap());
    let e = err_of(vacuum_into(db.conn(), &dest));
    assert!(matches!(e, StorageError::Conflict(_)));
}

/// `<path><suffix>` (SQLite's sidecar naming: `-wal`, `-shm`).
fn sidecar(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    std::path::PathBuf::from(name)
}

#[test]
fn a08_crash_mid_transaction_rolls_back() {
    // storage-core-6: a crash image, not a clean close. Closing a connection
    // rolls its transaction back, so the reopen never saw a WAL holding
    // uncommitted frames. Here the database file and its -wal are copied while
    // the transaction is open (the bytes a killed process leaves), the
    // connection is never closed, and the copy is opened.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crash.sqlite");
    let image = dir.path().join("image").join("crash.sqlite");
    std::fs::create_dir_all(image.parent().unwrap()).unwrap();
    let conn = open_file(&path).into_connection();
    // An empty WAL, then one committed row that exists only in the WAL.
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
    assert_eq!(std::fs::metadata(sidecar(&path, "-wal")).unwrap().len(), 0);
    conn.execute(
        "INSERT INTO app_settings (key, value_json, created_at, updated_at) VALUES ('committed', '1', 1, 1)",
        [],
    )
    .unwrap();
    let committed_wal = std::fs::metadata(sidecar(&path, "-wal")).unwrap().len();
    assert!(committed_wal > 0);
    // A small page cache makes the open transaction spill its pages into the
    // WAL as uncommitted frames.
    conn.pragma_update(None, "cache_size", 10).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    for i in 0..200 {
        conn.execute(
            "INSERT INTO app_settings (key, value_json, created_at, updated_at)
             VALUES (?1, '\"' || hex(zeroblob(2000)) || '\"', 1, 1)",
            params![format!("uncommitted-{i}")],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('c', 't', 1, 1)",
        [],
    )
    .unwrap();
    assert!(!conn.is_autocommit());
    std::fs::copy(&path, &image).unwrap();
    std::fs::copy(sidecar(&path, "-wal"), sidecar(&image, "-wal")).unwrap();
    let image_wal = std::fs::metadata(sidecar(&image, "-wal")).unwrap().len();
    assert!(
        image_wal > committed_wal,
        "the image holds uncommitted WAL frames ({image_wal} > {committed_wal} bytes)"
    );
    // The process dies: no COMMIT, no ROLLBACK, no close.
    std::mem::forget(conn);

    let db = open_file(&image);
    let keys: Vec<String> = db
        .conn()
        .prepare("SELECT key FROM app_settings ORDER BY key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        keys,
        ["committed"],
        "committed WAL frames replayed, the rest discarded"
    );
    assert_count(db.conn(), "curricula", 0);
    assert_eq!(
        g9_storage::migrations::integrity_check(db.conn()).unwrap(),
        vec!["ok".to_string()]
    );
    assert_eq!(
        g9_storage::migrations::foreign_key_check(db.conn()).unwrap(),
        0
    );
    assert_eq!(Migrator::verify_checksums(db.conn()).unwrap().ok, true);
}

#[test]
fn a08_migration_checksum_tamper_detected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tamper.sqlite");
    drop(open_file(&path));
    {
        let raw = Connection::open(&path).unwrap();
        raw.execute(
            "UPDATE schema_migrations SET sha256 = ?1 WHERE version = 3",
            params!["f".repeat(64)],
        )
        .unwrap();
    }
    let e = err_of(Database::open(&path, opts()));
    match e {
        StorageError::ChecksumMismatch { version, .. } => assert_eq!(version, 3),
        other => panic!("{other:?}"),
    }
    let g: g9_domain::G9Error = e.into();
    assert_eq!(g.category, g9_domain::ErrorCategory::DataCorruption);
    // even with auto-migrate off the open is refused
    assert!(matches!(
        err_of(Database::open(&path, opts().no_auto_migrate())),
        StorageError::ChecksumMismatch { .. }
    ));
}

#[test]
fn a08_schema_newer_than_app_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.sqlite");
    drop(open_file(&path));
    {
        let raw = Connection::open(&path).unwrap();
        raw.execute(
            "INSERT INTO schema_migrations (version, name, sha256, applied_at, app_version) VALUES (?1, '0099_future', ?2, 1, '9.9.9')",
            params![i64::from(CURRENT_SCHEMA_VERSION) + 1, "a".repeat(64)],
        )
        .unwrap();
    }
    let e = err_of(Database::open(&path, opts()));
    assert!(
        matches!(e, StorageError::SchemaNewerThanApp { db_version, .. } if db_version == CURRENT_SCHEMA_VERSION + 1),
        "{e:?}"
    );
    let g: g9_domain::G9Error = e.into();
    assert_eq!(g.category, g9_domain::ErrorCategory::PreconditionFailed);
}

#[test]
fn a08_dry_run_migration_on_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("partial.sqlite");
    {
        let mut db = Database::open(&path, opts().no_auto_migrate()).unwrap();
        let m = Migrator::new("0.1.0-test", t(0));
        let r = m.migrate_up_to(db.conn_mut(), 5).unwrap();
        assert_eq!(r.schema_version_after, 5);
        db.conn()
            .execute(
                "INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('c', 't', 1, 1)",
                [],
            )
            .unwrap();
        db.checkpoint_truncate().unwrap();
    }
    let scratch = dir.path().join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let m = Migrator::new("0.1.0-test", t(1));
    let report = m.dry_run_on_copy(&path, &scratch).unwrap();
    assert_eq!(report.report.schema_version_before, 5);
    assert_eq!(report.report.schema_version_after, CURRENT_SCHEMA_VERSION);
    assert_eq!(
        report.report.applied.len() as u32,
        CURRENT_SCHEMA_VERSION - 5
    );
    assert!(report.integrity_ok);
    assert_eq!(report.foreign_key_violations, 0);
    assert_eq!(report.table_counts.get("curricula"), Some(&1));
    assert!(report.table_counts.contains_key("goals"));
    assert!(report.copy_path.starts_with(&scratch));
    // the live file is still at version 5
    let live = Database::open(&path, opts().no_auto_migrate()).unwrap();
    assert_eq!(Migrator::schema_version(live.conn()).unwrap(), 5);
    assert!(!app_tables(live.conn())
        .unwrap()
        .iter()
        .any(|t| t == "goals"));
    drop(live);
    // a second dry run replaces the copy the first one left: it is our own
    // scratch file, and refusing it blocked every later migration (storage-core-3;
    // a symlink or other non-file at that path is still refused, dry_run_cleanup.rs)
    let again = m.dry_run_on_copy(&path, &scratch).unwrap();
    assert!(again.integrity_ok);
    // the default policy never migrates an existing database behind the operator's back:
    // the open succeeds, reports the pending versions, and leaves the schema at 5
    let mut guarded = open_file(&path);
    let pending: Vec<u32> = (6..=CURRENT_SCHEMA_VERSION).collect();
    assert_eq!(guarded.pending_migrations(), pending.as_slice());
    assert!(guarded.migration_report().is_none());
    assert_eq!(Migrator::schema_version(guarded.conn()).unwrap(), 5);
    // after backup + dry run the application migrates explicitly
    let report = guarded.migrate_pending().unwrap();
    assert_eq!(report.schema_version_before, 5);
    assert_eq!(report.schema_version_after, CURRENT_SCHEMA_VERSION);
    assert!(guarded.pending_migrations().is_empty());
    drop(guarded);
    // a fresh database is migrated on open under the same default policy
    let fresh = open_file(&dir.path().join("fresh.sqlite"));
    assert!(fresh.pending_migrations().is_empty());
    assert_eq!(fresh.migration_report().unwrap().schema_version_before, 0);
    assert_eq!(
        Migrator::schema_version(fresh.conn()).unwrap(),
        CURRENT_SCHEMA_VERSION
    );
    // and MigrationPolicy::Always migrates an existing database on open
    let path2 = dir.path().join("always.sqlite");
    {
        let mut db = Database::open(&path2, opts().no_auto_migrate()).unwrap();
        Migrator::new("0.1.0-test", t(0))
            .migrate_up_to(db.conn_mut(), 2)
            .unwrap();
    }
    let always = Database::open(&path2, opts().auto_migrate()).unwrap();
    assert_eq!(always.migration_report().unwrap().schema_version_before, 2);
    assert_eq!(
        Migrator::schema_version(always.conn()).unwrap(),
        CURRENT_SCHEMA_VERSION
    );
}

// ---------------------------------------------------------------- 0008 / 0009 checks

#[test]
fn knowledge_ai_checks_mirror_domain_rules() {
    let db = open_mem();
    populate(db.conn());
    // skill family must match the code prefix
    let e = sql_err(db.conn().execute(
        "INSERT INTO skill_registry (skill_id, code, family, title, package_dir, created_at, updated_at)
         VALUES ('g9-bad', 'K01', 'TEACHING', 'x', 'x', 1, 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // an EXAM-mode invocation never carries SECRET data
    let e = sql_err(db.conn().execute(
        "INSERT INTO skill_invocations (id, skill_id, skill_version_id, actor_id, actor_kind, intent, scope_domain, scope_mode,
            data_classification, state, input_sha256, idempotency_key, trace_id, requested_at, created_at, updated_at)
         VALUES ('inv-1', 'g9-session-director', 'sv-1', 'model', 'MODEL', 'hint', 'LEARNING', 'EXAM', 'SECRET', 'REQUESTED', ?1, 'k', 't', 1, 1, 1)",
        params![sha("in")],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // a COMPLETED invocation needs a validated output
    let e = sql_err(db.conn().execute(
        "INSERT INTO skill_invocations (id, skill_id, skill_version_id, actor_id, actor_kind, intent, scope_domain, scope_mode,
            data_classification, state, input_sha256, idempotency_key, trace_id, requested_at, created_at, updated_at)
         VALUES ('inv-2', 'g9-session-director', 'sv-1', 'model', 'MODEL', 'hint', 'LEARNING', 'STUDY', 'INTERNAL', 'COMPLETED', ?1, 'k', 't', 1, 1, 1)",
        params![sha("in")],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // source trust is derived from state
    let e = sql_err(db.conn().execute(
        "UPDATE source_registry SET trust = 'APPROVED_REFERENCE' WHERE id = 'src-1'",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // model weights need a sha256 checksum
    let e = sql_err(db.conn().execute(
        "INSERT INTO model_registry (model_id, display_name, architecture, weights_source, license, quantization, checksum, context_length,
            tokenizer_template, runtime_version, created_at, updated_at)
         VALUES ('m2', 'x', 'x', 'x', 'x', 'x', 'short', 1, 'x', 'x', 1, 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // claim versions are immutable
    db.conn()
        .execute(
            "INSERT INTO knowledge_claims (id, topic, created_at, updated_at) VALUES ('claim-1', 'boot', 1, 1)",
            [],
        )
        .unwrap();
    db.conn()
        .execute(
            "INSERT INTO claim_versions (id, claim_id, version_no, statement, trust, created_by, created_at)
             VALUES ('clv-1', 'claim-1', 1, 'GRUB loads the kernel', 'IMPORTED_DOCUMENT', 'operator-1', 1)",
            [],
        )
        .unwrap();
    let e = sql_err(db.conn().execute(
        "UPDATE claim_versions SET statement = 'x' WHERE id = 'clv-1'",
        [],
    ));
    assert!(matches!(e, StorageError::Immutable(_)), "{e:?}");
}

#[test]
fn business_tables_isolate_domain_and_keep_decisions_honest() {
    let db = open_mem();
    populate(db.conn());
    let domain: String = db
        .conn()
        .query_row(
            "SELECT data_domain FROM goals WHERE id = 'goal-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(domain, "BUSINESS");
    let domain: String = db
        .conn()
        .query_row(
            "SELECT data_domain FROM opt_in_reflections WHERE id = 'refl-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(domain, "PERSONAL");
    // model output can only propose decisions
    let e = sql_err(db.conn().execute(
        "INSERT INTO decisions (id, title, context, state, decided_by, decided_at, chosen_option, trust, created_at, updated_at)
         VALUES ('dec-m', 'x', 'x', 'DECIDED', 'model', 1, 'A', 'MODEL_OUTPUT', 1, 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // execution observations are never model output
    let e = sql_err(db.conn().execute(
        "INSERT INTO execution_observations (id, project_id, metric, value, unit, observed_at, source_kind, trust, provenance_json, created_at)
         VALUES ('xo-1', 'proj-1', 'throughput', 3.0, 'tasks/week', 1, 'MODEL_RESPONSE', 'MODEL_OUTPUT', '{}', 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
    // the compatibility view resolves
    let n: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM dependencies", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
    // milestone dependency cannot be self-referential
    let e = sql_err(db.conn().execute(
        "INSERT INTO milestone_dependencies (id, from_milestone_id, to_milestone_id, kind, created_at) VALUES ('dep-1', 'ms-1', 'ms-1', 'BLOCKS', 1)",
        [],
    ));
    assert!(matches!(e, StorageError::CheckViolation(_)), "{e:?}");
}

#[test]
fn open_profile_applies_to_in_memory_and_file_alike() {
    let db = open_mem();
    assert_eq!(db.path(), None);
    let fk: i64 = db
        .conn()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
    let ts: i64 = db
        .conn()
        .query_row("PRAGMA trusted_schema", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ts, 0);
    assert_eq!(
        db.migration_report().unwrap().applied.len() as u32,
        CURRENT_SCHEMA_VERSION
    );
    let applied = Migrator::applied(db.conn()).unwrap();
    assert!(applied
        .iter()
        .all(|m| m.applied_at == UtcMillis::from_millis(T0) && m.app_version == "0.1.0-test"));
    let _ = ApprovalId::parse("x").unwrap();
}

#[test]
fn every_foreign_key_column_is_index_covered() {
    let db = open_mem();
    let mut uncovered = Vec::new();
    for table in app_tables(db.conn()).unwrap() {
        let fk_cols: Vec<String> = db
            .conn()
            .prepare(&format!("PRAGMA foreign_key_list(\"{table}\")"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        if fk_cols.is_empty() {
            continue;
        }
        // first column of every index (explicit indexes, UNIQUE constraints and the primary key)
        let mut leading: Vec<String> = db
            .conn()
            .prepare(&format!("PRAGMA index_list(\"{table}\")"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .filter_map(|idx| {
                db.conn()
                    .query_row(&format!("PRAGMA index_info(\"{idx}\")"), [], |r| {
                        r.get::<_, String>(2)
                    })
                    .ok()
            })
            .collect();
        let pk_first: Option<String> = db
            .conn()
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(5)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .find(|(_, pk)| *pk == 1)
            .map(|(name, _)| name);
        leading.extend(pk_first);
        for col in fk_cols {
            if !leading.contains(&col) {
                uncovered.push(format!("{table}.{col}"));
            }
        }
    }
    assert!(
        uncovered.is_empty(),
        "foreign key columns without a leading index: {uncovered:?}"
    );
}
