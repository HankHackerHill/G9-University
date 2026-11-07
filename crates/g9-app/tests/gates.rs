#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! g9-app gate tests (ARCHITECTURE.md §5, §14): every test drives the real
//! `dispatch` entry point over an in-memory (or temp-file) database with a fixed
//! clock and binding key, then asserts the final backend state through the
//! storage repositories.

mod common;

use common::*;
use g9_ai_adapter::{ModelRef, ScriptedRuntime};
use g9_domain::{
    Action, ActorId, ApprovalId, ApprovalState, AssessmentAttemptState, AssessmentId, AttemptId,
    BackupId, BackupState, Classification, CompetencyId, CurriculumVersionId, DataDomain,
    Difficulty, ErrorCategory, EvidenceId, IanaZone, InteractionMode, ManifestProvenance, ModelId,
    NoteId, ResourceKind, ResourceRef, Revision, SessionId, SessionState, UtcMillis,
};
use g9_storage::approvals_store;
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::competency as competency_repo;
use g9_storage::repos::curriculum as curriculum_repo;
use g9_storage::repos::evidence as evidence_repo;
use g9_storage::repos::knowledge as knowledge_repo;
use g9_storage::repos::sessions as sessions_repo;
use g9_storage::repos::ReadScope;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::sync::Arc;

fn learning() -> ReadScope {
    ReadScope::domain(DataDomain::Learning)
}

fn note_payload(title: &str, body: &str) -> Value {
    json!({ "title": title, "body_markdown": body, "classification": "PRIVATE", "domain": "LEARNING" })
}

fn pending(h: &Harness) -> Vec<approvals_store::ApprovalRequestRow> {
    approvals_store::list_by_state(h.conn(), ApprovalState::Pending).unwrap()
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

#[test]
fn pipeline_idempotent_replay_returns_same_outcome() {
    let mut h = Harness::new();
    let first = h
        .call_with(
            "notes.create",
            note_payload("t", "body one"),
            &Opts::key("k-1"),
        )
        .unwrap();
    assert_eq!(first["replayed"], false);
    let rows_after_first = h.audit_rows();
    let second = h
        .call_with(
            "notes.create",
            note_payload("t", "body one"),
            &Opts::key("k-1"),
        )
        .unwrap();
    assert_eq!(second["replayed"], true);
    assert_eq!(second["result"], first["result"]);
    assert_eq!(second["audit_ref"], first["audit_ref"]);
    assert_eq!(second["revision"], first["revision"]);
    // Final state: one note, no new audit row for the replay.
    assert_eq!(h.audit_rows(), rows_after_first);
    let notes = evidence_repo::list_notes_between(
        h.conn(),
        &learning(),
        UtcMillis::from_millis(0),
        UtcMillis::from_millis(T0 + 1),
    )
    .unwrap();
    assert_eq!(notes.len(), 1);
}

#[test]
fn pipeline_idempotency_conflict_on_different_payload() {
    let mut h = Harness::new();
    h.call_with(
        "notes.create",
        note_payload("t", "body one"),
        &Opts::key("k-1"),
    )
    .unwrap();
    let err = h
        .call_with(
            "notes.create",
            note_payload("t", "DIFFERENT"),
            &Opts::key("k-1"),
        )
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::Conflict);
    assert_eq!(err.code, "idempotency.key_reused");
    let notes = evidence_repo::list_notes_between(
        h.conn(),
        &learning(),
        UtcMillis::from_millis(0),
        UtcMillis::from_millis(T0 + 1),
    )
    .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body_markdown, "body one");
}

#[test]
fn pipeline_policy_deny_maps_to_unauthorized_and_logs_decision() {
    let mut h = Harness::new();
    let rows = h.audit_rows();
    let err = h
        .call(
            "deletion.request",
            json!({ "resource": { "kind": "ATTEMPT", "id": "attempt-x" }, "reason": "cleanup" }),
        )
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::Unauthorized);
    assert_eq!(err.code, "policy.denied");
    assert!(err.approval_id.is_none());
    // Final state: a DENY decision row, an audit row with UNAUTHORIZED, no approval request.
    let decisions = approvals_store::list_policy_decisions_for(
        h.conn(),
        &ResourceRef::new(ResourceKind::Attempt, "attempt-x"),
        10,
    )
    .unwrap();
    assert_eq!(decisions.len(), 1);
    assert!(matches!(
        decisions[0].decision,
        g9_domain::Decision::Deny { .. }
    ));
    assert_eq!(decisions[0].actor.id, operator_id());
    assert_eq!(h.audit_rows(), rows + 1);
    assert!(pending(&h).is_empty());
    let recent = h.call("audit.recent", json!({ "limit": 1 })).unwrap();
    assert_eq!(recent["result"]["entries"][0]["outcome"], "ERROR");
    assert_eq!(recent["result"]["entries"][0]["decision"], "DENY");
    assert_eq!(recent["result"]["chain_ok"], true);
}

fn export_payload(include_private: bool) -> Value {
    json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": include_private })
}

#[test]
fn pipeline_approval_required_creates_request_and_returns_id() {
    let mut h = Harness::new();
    let err = h.call("export.request", export_payload(false)).unwrap_err();
    assert_eq!(err.category, ErrorCategory::ApprovalRequired);
    let id = err.approval_id.clone().unwrap();
    let row = approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap();
    assert_eq!(row.state, ApprovalState::Pending);
    assert_eq!(
        row.required_capability,
        Some(g9_domain::Capability::DataExport)
    );
    assert_eq!(row.requested_by, operator_id());
    assert_eq!(row.operation.action, Action::Export);
    assert_eq!(
        row.operation.parameters_sha256,
        g9_app::pipeline::parameters_sha256("export.request", &export_payload(false))
    );
    assert_eq!(row.expires_at.as_millis(), T0 + 15 * 60_000);
    // The decision row links the created request.
    let decisions =
        approvals_store::list_policy_decisions_for(h.conn(), &row.operation.resource, 10).unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].approval_id.as_ref(), Some(&id));
    assert!(matches!(
        decisions[0].decision,
        g9_domain::Decision::ApprovalRequired { .. }
    ));
    // The ledger mirrors the row; the UI list shows it.
    assert_eq!(
        h.ctx().approvals().get(&id).unwrap().state,
        ApprovalState::Pending
    );
    let list = h
        .call("approvals.list", json!({ "state": "PENDING" }))
        .unwrap();
    assert_eq!(list["result"]["approvals"][0]["approval_id"], json!(id));
    assert_eq!(
        list["result"]["approvals"][0]["operation_name"],
        "export.request"
    );
}

#[test]
fn pipeline_consumes_approval_once_and_rejects_replay() {
    let mut h = Harness::new();
    let err = h.call("export.request", export_payload(false)).unwrap_err();
    let id = err.approval_id.unwrap();
    h.decide(id.as_str(), "APPROVE");
    let row = approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap();
    assert_eq!(row.state, ApprovalState::Approved);
    assert_eq!(row.binding_mac.as_deref().map(str::len), Some(64));

    // A different parameter set cannot use the approval (operation hash binding).
    let mismatch = h
        .call_with(
            "export.request",
            export_payload(true),
            &Opts::approval(id.as_str()),
        )
        .unwrap_err();
    assert_eq!(mismatch.category, ErrorCategory::Unauthorized);
    assert_eq!(mismatch.code, "approval.operation_mismatch");
    assert_eq!(
        approvals_store::get_request(h.conn(), &id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Approved
    );

    let ok = h
        .call_with(
            "export.request",
            export_payload(false),
            &Opts::approval(id.as_str()),
        )
        .unwrap();
    assert_eq!(ok["result"]["state"], "REQUESTED");
    let row = approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap();
    assert_eq!(row.state, ApprovalState::Consumed);
    assert!(row.consumed_at.is_some());

    // Replay with a fresh idempotency key: single use.
    let replay = h
        .call_with(
            "export.request",
            export_payload(false),
            &Opts::approval(id.as_str()),
        )
        .unwrap_err();
    assert_eq!(replay.category, ErrorCategory::Unauthorized);
    assert_eq!(replay.code, "approval.already_consumed");
    let decisions =
        approvals_store::list_policy_decisions_for(h.conn(), &row.operation.resource, 20).unwrap();
    assert!(decisions.iter().any(|d| matches!(&d.decision,
        g9_domain::Decision::Deny { rule_id, .. } if rule_id == "approval.already_consumed")));
    assert_eq!(
        h.ctx().approvals().get(&id).unwrap().state,
        ApprovalState::Consumed
    );

    // An unknown approval id is refused without a dangling link.
    let unknown = h
        .call_with(
            "export.request",
            export_payload(false),
            &Opts::approval("no-such-approval"),
        )
        .unwrap_err();
    assert_eq!(unknown.category, ErrorCategory::NotFound);
}

#[test]
fn pipeline_stale_revision_rejected() {
    let mut h = Harness::new();
    let created = h.call("notes.create", note_payload("t", "v1")).unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    assert_eq!(created["revision"], 1);
    let err = h
        .call_with(
            "notes.revise",
            json!({ "note_id": note_id, "body_markdown": "v2" }),
            &Opts::revision(5),
        )
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::StaleRevision);
    assert_eq!(err.details.as_ref().unwrap()["actual"], 1);
    let note = evidence_repo::get_note(h.conn(), &learning(), &NoteId::parse(&note_id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(note.body_markdown, "v1");
    assert_eq!(note.revision, Revision::INITIAL);
    let ok = h
        .call_with(
            "notes.revise",
            json!({ "note_id": note_id, "body_markdown": "v2" }),
            &Opts::revision(1),
        )
        .unwrap();
    assert_eq!(ok["result"]["version_count"], 2);
    assert_eq!(ok["result"]["body_markdown"], "v2");
}

#[test]
fn pipeline_audit_chain_grows_and_verifies() {
    let mut h = Harness::new();
    let base = h.audit_rows();
    let a = h.call("notes.create", note_payload("a", "1")).unwrap();
    let _ = h.call("export.request", export_payload(false)).unwrap_err();
    let b = h.call("notes.create", note_payload("b", "2")).unwrap();
    // Reads do not append; every mutating command (accepted or refused) appends one row.
    h.call("system.status", json!({})).unwrap();
    assert_eq!(h.audit_rows(), base + 3);
    let report = g9_storage::AuditChain::verify_chain(h.conn()).unwrap();
    assert!(report.ok);
    let recent = h.call("audit.recent", json!({ "limit": 3 })).unwrap();
    let entries = recent["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["id"], b["audit_ref"]);
    assert_eq!(entries[2]["id"], a["audit_ref"]);
    assert_eq!(entries[0]["prev_hash"], entries[1]["row_hash"]);
    assert_eq!(entries[0]["actor_id"], "operator-1");
    assert_eq!(recent["result"]["chain_ok"], true);
}

// ---------------------------------------------------------------------------
// A01 curriculum
// ---------------------------------------------------------------------------

#[test]
fn a01_curriculum_import_rejects_invariant_violation_end_to_end() {
    let mut h = Harness::new();
    let mut bad = seed_manifest();
    bad["structure"]["weeks"] = json!(11);
    let err = h
        .call(
            "curriculum.import",
            json!({ "manifest": bad, "provenance": "PROPOSED_SEED" }),
        )
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::InvalidInput);
    assert_eq!(err.code, "curriculum.invariant_violated");
    assert!(!err.details.as_ref().unwrap()["violations"]
        .as_array()
        .unwrap()
        .is_empty());
    // Nothing was requested or stored.
    assert!(pending(&h).is_empty());
    let id = CurriculumVersionId::parse("g9-ct9-480:0.1.0-seed").unwrap();
    assert!(curriculum_repo::get_version(h.conn(), &id)
        .unwrap()
        .is_none());

    // Direct APPROVED_CANONICAL import is refused by the schema.
    let canon = h
        .call(
            "curriculum.import",
            json!({ "manifest": seed_manifest(), "provenance": "APPROVED_CANONICAL" }),
        )
        .unwrap_err();
    assert_eq!(canon.code, "ipc.payload_invalid");

    // A valid seed imports only with an approval and stays PROPOSED_SEED.
    let ok = h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    assert_eq!(ok["result"]["weeks"], 12);
    assert_eq!(ok["result"]["days"], 72);
    assert_eq!(ok["result"]["objectives"], 216);
    let row = curriculum_repo::get_version(h.conn(), &id)
        .unwrap()
        .unwrap();
    assert_eq!(row.provenance, ManifestProvenance::ProposedSeed);
    assert!(!row.is_current);
}

fn reconciled_candidate(original: &Value) -> Value {
    let mut candidate = original.clone();
    candidate["version"] = json!("0.2.0-reconciled");
    candidate["provenance"] = json!("RECONCILED_CANDIDATE");
    let original_m: g9_curriculum::Manifest = serde_json::from_value(original.clone()).unwrap();
    candidate["reconciliation"] = json!({
        "original_checksum": "0".repeat(64),
        "diff_checksum": "0".repeat(64),
        "approved_by": "operator-1",
        "approved_at": T0,
    });
    let cand_m: g9_curriculum::Manifest = serde_json::from_value(candidate.clone()).unwrap();
    let report = g9_curriculum::reconcile(&original_m, &cand_m).unwrap();
    candidate["reconciliation"]["original_checksum"] = json!(report.original_checksum.as_str());
    candidate["reconciliation"]["diff_checksum"] = json!(report.diff_checksum.as_str());
    candidate
}

#[test]
fn a01_promote_requires_reconciliation_and_approval() {
    let mut h = Harness::new();
    let original = seed_manifest();
    let candidate = reconciled_candidate(&original);
    let imported = h.approved(
        "curriculum.import",
        json!({ "manifest": candidate, "provenance": "RECONCILED_CANDIDATE" }),
        &Opts::default(),
    );
    let version_id = imported["result"]["curriculum_version_id"]
        .as_str()
        .unwrap()
        .to_string();
    let vid = CurriculumVersionId::parse(&version_id).unwrap();

    // A content-different original without a matching reconciliation: blocked before policy.
    let mut other = original.clone();
    other["weeks"][0]["days"][0]["objectives"][0]["text"] = json!("a different objective text");
    let pending_before = pending(&h).len();
    let blocked = h
        .call(
            "curriculum.promote",
            json!({ "curriculum_version_id": version_id, "candidate_manifest": candidate, "original_manifest": other }),
        )
        .unwrap_err();
    assert_eq!(blocked.category, ErrorCategory::PreconditionFailed);
    assert_eq!(blocked.code, "curriculum.promotion_blocked");
    assert_eq!(pending(&h).len(), pending_before);

    // A candidate that does not hash to the stored version is refused.
    let mut tampered = candidate.clone();
    tampered["name"] = json!("tampered");
    let mismatch = h
        .call(
            "curriculum.promote",
            json!({ "curriculum_version_id": version_id, "candidate_manifest": tampered, "original_manifest": original }),
        )
        .unwrap_err();
    assert_eq!(mismatch.code, "curriculum.candidate_checksum_mismatch");

    // Reconciled against the real original: approval required, then promoted.
    let promoted = h.approved(
        "curriculum.promote",
        json!({ "curriculum_version_id": version_id, "candidate_manifest": candidate, "original_manifest": original }),
        &Opts::default(),
    );
    assert_eq!(promoted["result"]["provenance"], "APPROVED_CANONICAL");
    assert_eq!(promoted["result"]["promotion_allowed"], true);
    let row = curriculum_repo::get_version(h.conn(), &vid)
        .unwrap()
        .unwrap();
    assert_eq!(row.provenance, ManifestProvenance::ApprovedCanonical);
    assert!(row.is_current);
    assert_eq!(row.approved_by, Some(operator_id()));
    let approval = row.approval_id.unwrap();
    assert_eq!(
        approvals_store::get_request(h.conn(), &approval)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Consumed
    );
}

// ---------------------------------------------------------------------------
// A03 restart recovery
// ---------------------------------------------------------------------------

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

#[test]
fn a03_restart_recovery_marks_sessions_interrupted() {
    let mut h = Harness::new();
    let sid = insert_session(&h, "session-a03");
    h.call("session.prepare", json!({ "session_id": "session-a03" }))
        .unwrap();
    let started = h
        .call("session.start", json!({ "session_id": "session-a03" }))
        .unwrap();
    assert_eq!(started["result"]["state"], "RUNNING");
    h.advance(25 * 60_000);
    let paused = h
        .call("session.pause", json!({ "session_id": "session-a03" }))
        .unwrap();
    assert_eq!(paused["result"]["credited_ms"], 25 * 60_000);
    h.call("session.resume", json!({ "session_id": "session-a03" }))
        .unwrap();
    h.advance(40 * 60_000);

    // Crash/suspend: the open interval is still open when the process restarts.
    h.restart();
    let report = h.ctx().recovery_report().clone();
    assert_eq!(report.interrupted_sessions, vec![sid.clone()]);
    assert_eq!(report.closed_intervals.len(), 1);
    let row = sessions_repo::get_session(h.conn(), &learning(), &sid)
        .unwrap()
        .unwrap();
    assert_eq!(row.state, SessionState::Interrupted);
    assert_eq!(
        row.credited_ms.as_millis(),
        25 * 60_000,
        "uncertain time is never credited"
    );
    assert_eq!(row.uncertain_ms.as_millis(), 40 * 60_000);
    let intervals = sessions_repo::list_intervals(h.conn(), &sid).unwrap();
    assert_eq!(
        intervals[1].record.continuity,
        g9_calendar::Continuity::Uncertain
    );
    assert_eq!(
        intervals[1].closed_by,
        Some(sessions_repo::ClosedBy::Recovery)
    );
    let events = g9_storage::events::list_since(h.conn(), 0, 100).unwrap();
    assert!(events
        .iter()
        .any(|e| e.event.header.kind == "session.interrupted"
            && e.event.header.actor.kind == g9_domain::ActorKind::System));

    // Resume is refused until the operator corrects the uncertain interval.
    let refused = h
        .call("session.resume", json!({ "session_id": "session-a03" }))
        .unwrap_err();
    assert_eq!(refused.code, "session.illegal_transition");
    let corrected = h
        .call(
            "session.correct_time",
            json!({ "session_id": "session-a03", "interval_id": intervals[1].id, "credited_ms": 30 * 60_000, "reason": "laptop suspended; 30 minutes of verified work" }),
        )
        .unwrap();
    assert_eq!(corrected["result"]["state"], "PAUSED");
    assert_eq!(corrected["result"]["credited_ms"], 55 * 60_000);
    assert_eq!(corrected["result"]["requires_correction"], false);
    // The new process lifetime can time again (fresh monotonic epoch).
    h.call("session.resume", json!({ "session_id": "session-a03" }))
        .unwrap();
    h.advance(5 * 60_000);
    let done = h
        .call("session.complete", json!({ "session_id": "session-a03" }))
        .unwrap();
    assert_eq!(done["result"]["state"], "COMPLETED");
    assert_eq!(done["result"]["credited_ms"], 60 * 60_000);
    let row = sessions_repo::get_session(h.conn(), &learning(), &sid)
        .unwrap()
        .unwrap();
    assert_eq!(row.credited_ms.as_millis(), 60 * 60_000);
}

// ---------------------------------------------------------------------------
// A04 finalization
// ---------------------------------------------------------------------------

fn seed_assessment(h: &Harness) -> AssessmentId {
    let conn = h.conn();
    let now = h.now();
    let competency = g9_competency::Competency {
        id: CompetencyId::parse("comp-boot").unwrap(),
        name: "Linux boot chain".into(),
        pack: "linux".into(),
        domain: DataDomain::Learning,
        description: "Recover a broken boot chain".into(),
        validity_window_days: 90,
        difficulty_band: (Difficulty::new(1).unwrap(), Difficulty::new(5).unwrap()),
        nice_refs: Vec::new(),
    };
    competency_repo::insert_competency(
        conn,
        &competency_repo::CompetencyRow::from_domain(&competency, "LINUX-BOOT", None, now),
    )
    .unwrap();
    let def = g9_assessment::AssessmentDefinition::new(
        AssessmentId::parse("ad-w1").unwrap(),
        "Week 1 blind practical",
        vec![competency.id.clone()],
        g9_assessment::AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )
    .unwrap();
    assessment_repo::insert_definition(
        conn,
        &assessment_repo::AssessmentDefinitionRow::from_domain(&def, "W1-BLIND", now).unwrap(),
    )
    .unwrap();
    let version = g9_assessment::AssessmentVersion {
        definition_id: def.id.clone(),
        version: 1,
        rubric: g9_assessment::Rubric::baseline(),
        task_ref: EvidenceId::parse("evi-task").unwrap(),
        sealed_key_ref: None,
        difficulty: Difficulty::new(3).unwrap(),
        equivalence_group: "W1".into(),
        created_at: now,
        checksum: String::new(),
    }
    .with_checksum()
    .unwrap();
    let row = assessment_repo::AssessmentVersionRow::from_domain(
        &version,
        "av-w1",
        "Recover the boot chain",
        None,
        operator_id(),
    )
    .unwrap();
    assessment_repo::insert_version(conn, &row).unwrap();
    let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
        &version.rubric,
        "rub-w1",
        "av-w1",
        "Baseline",
        "v1",
        now,
    );
    assessment_repo::insert_rubric(conn, &rubric, &criteria).unwrap();
    def.id
}

fn grade_payload(attempt: &str, source: &str) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| json!({ "criterion_code": c.code, "score": 4, "source": source, "evidence_refs": [] }))
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

#[test]
fn a04_approve_final_requires_approval_and_operator_and_is_immutable_after() {
    let mut h = Harness::new();
    let def = seed_assessment(&h);
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    let attempt = begun["result"]["attempt_id"].as_str().unwrap().to_string();
    assert_eq!(begun["result"]["state"], "STARTED");
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();

    // Model-advisory grades can never finalize: refused before any approval request.
    let advisory = h
        .call(
            "assessment.grade_provisional",
            grade_payload(&attempt, "MODEL_ADVISORY"),
        )
        .unwrap();
    assert_eq!(advisory["result"]["state"], "GRADED_PROVISIONAL");
    let rev = advisory["revision"].as_u64().unwrap();
    let refused = h
        .call_with(
            "assessment.approve_final",
            json!({ "attempt_id": attempt }),
            &Opts::revision(rev),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::PreconditionFailed);
    assert!(pending(&h).is_empty());

    // Human-reviewed grades: approval required, then final.
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let graded = h
        .call(
            "assessment.grade_provisional",
            grade_payload(&attempt, "HUMAN_REVIEWER"),
        )
        .unwrap();
    let rev = graded["revision"].as_u64().unwrap();
    assert_eq!(graded["result"]["grade"]["final"], false);
    let finalized = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    assert_eq!(finalized["result"]["state"], "APPROVED_FINAL");
    assert_eq!(finalized["result"]["grade"]["final"], true);
    assert_eq!(finalized["result"]["grade"]["pass"], true);
    assert_eq!(
        finalized["result"]["grade"]["reliability_class"],
        "SELF_ADJUDICATED"
    );

    let aid = AttemptId::parse(&attempt).unwrap();
    let decision = assessment_repo::effective_final_decision(h.conn(), &aid)
        .unwrap()
        .unwrap();
    assert!(decision.is_final);
    assert_eq!(decision.decided_by, operator_id());
    assert!(decision.approval_id.is_some());
    let stored = assessment_repo::get_attempt(h.conn(), &learning(), &aid)
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, AssessmentAttemptState::ApprovedFinal);

    // The reviewer must be the operator (domain rule; the backend never assigns another actor).
    let model = g9_domain::Actor::model(ActorId::parse("model-1").unwrap());
    let rubric = g9_assessment::Rubric::baseline();
    let results: Vec<g9_assessment::CriterionResult> =
        assessment_repo::effective_criterion_results(h.conn(), &aid)
            .unwrap()
            .iter()
            .map(|r| r.to_domain().unwrap())
            .collect();
    let mut rebuilt = stored
        .to_attempt(
            &def,
            1,
            &operator_id(),
            vec![],
            Difficulty::new(3).unwrap(),
            vec![],
        )
        .unwrap();
    // Re-open a provisional copy in memory only to exercise the reviewer rule.
    rebuilt = serde_json::from_value({
        let mut v = serde_json::to_value(&rebuilt).unwrap();
        v["state"] = json!("GRADED_PROVISIONAL");
        v
    })
    .unwrap();
    assert!(g9_assessment::finalize(
        g9_domain::GradeDecisionId::parse("gd-x").unwrap(),
        &rebuilt,
        &rubric,
        &results,
        &model,
        h.now()
    )
    .is_err());

    // Immutable afterwards: every further change is denied by policy.
    let again = h
        .call_with(
            "assessment.approve_final",
            json!({ "attempt_id": attempt }),
            &Opts::revision(stored.revision.get()),
        )
        .unwrap_err();
    assert_eq!(again.category, ErrorCategory::Unauthorized);
    let regrade = h
        .call(
            "assessment.grade_provisional",
            grade_payload(&attempt, "HUMAN_REVIEWER"),
        )
        .unwrap_err();
    assert_eq!(regrade.category, ErrorCategory::Unauthorized);
    let after = assessment_repo::get_attempt(h.conn(), &learning(), &aid)
        .unwrap()
        .unwrap();
    assert_eq!(after.revision, stored.revision);
    assert_eq!(
        assessment_repo::list_grade_decisions(h.conn(), &aid)
            .unwrap()
            .len(),
        1
    );
}

// ---------------------------------------------------------------------------
// A06 identity
// ---------------------------------------------------------------------------

#[test]
fn a06_ui_cannot_set_actor_identity() {
    let mut h = Harness::new();
    let rows = h.audit_rows();
    for actor in [
        json!({ "id": "evil", "kind": { "kind": "OPERATOR" } }),
        json!({ "id": "operator-1", "kind": { "kind": "OPERATOR" } }),
        json!({ "id": "skill", "kind": { "kind": "SKILL", "skill_id": "g9-session-director" } }),
    ] {
        let mut meta = h.meta("notes.create", &Opts::default());
        meta["actor"] = actor;
        let err = h
            .raw("notes.create", meta, note_payload("t", "b"))
            .unwrap_err();
        assert_eq!(err.category, ErrorCategory::InvalidInput);
        assert_eq!(err.code, "meta.actor_not_accepted");
    }
    assert_eq!(
        h.audit_rows(),
        rows,
        "a refused identity claim persists nothing"
    );
    // Every accepted command is attributed to the backend-assigned operator.
    let err = h.call("export.request", export_payload(false)).unwrap_err();
    let row = approvals_store::get_request(h.conn(), &err.approval_id.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(row.requested_by, operator_id());
    let recent = h.call("audit.recent", json!({ "limit": 1 })).unwrap();
    assert_eq!(recent["result"]["entries"][0]["actor_id"], "operator-1");
    assert_eq!(recent["result"]["entries"][0]["actor_kind"], "OPERATOR");
}

// ---------------------------------------------------------------------------
// A07 skill output
// ---------------------------------------------------------------------------

fn execute_output() -> Value {
    json!({
        "status": "COMPLETED",
        "result_schema_version": "1.0",
        "observations": [],
        "inferences": [],
        "evidence_refs": [],
        "confidence_limits": ["fixture"],
        "recommended_actions": [
            { "action": "EXECUTE", "resource": { "kind": "PROCESS", "id": "bash" },
              "rationale": "run the lab", "evidence_refs": [], "parameters": { "cmd": "id" } }
        ],
        "approval_requirements": [],
        "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 },
        "validation_results": [],
        "errors": [],
        "result": {
            "brief": { "day_id": "D15", "objectives": [], "due_reviews": [], "open_interventions": [], "risks": [] },
            "basis": { "evidence_refs": [], "unresolved_refs": [] }
        }
    })
}

#[test]
fn a07_skill_invoke_output_requesting_execute_is_rejected_and_nothing_persisted() {
    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    let out = execute_output();
    runtime.push_response(runtime.response(out.to_string(), Some(out)));
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime.clone();
    let mut h = Harness::with(|s| {
        s.runtime = Some(rt);
        s.skills = Some(registry);
    });
    let err = h
        .call(
            "skills.invoke",
            json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" } }),
        )
        .unwrap_err();
    assert_eq!(err.category, ErrorCategory::PreconditionFailed, "{err:?}");
    assert_eq!(err.code, "skills.output_rejected");
    let details = err.details.unwrap();
    assert!(
        details["failed_checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "output.recommended_actions"),
        "{details}"
    );
    assert_eq!(runtime.remaining(), 0, "the runtime was actually called");

    // Only metadata survives: the invocation row (ERROR, no output hash).
    let invocations = knowledge_repo::list_invocations_for_skill(
        h.conn(),
        &g9_domain::SkillId::parse("g9-session-director").unwrap(),
        10,
    )
    .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].state, g9_domain::SkillInvocationState::Error);
    assert!(invocations[0].output_sha256.is_none());
    assert_eq!(invocations[0].actor_id, operator_id());
    // Nothing the model asked for was executed or turned into a request.
    assert!(
        approvals_store::list_by_state(h.conn(), ApprovalState::Pending)
            .unwrap()
            .is_empty()
    );
    assert!(
        g9_storage::repos::retention::list_interventions(h.conn(), &learning(), None)
            .unwrap()
            .is_empty()
    );
    let events = g9_storage::events::list_since(h.conn(), 0, 100).unwrap();
    assert!(events
        .iter()
        .all(|e| !e.event.payload.to_string().contains("bash")));
}

// ---------------------------------------------------------------------------
// A08 backup
// ---------------------------------------------------------------------------

#[test]
fn a08_backup_commands_drive_state_machine_to_retained() {
    let mut h = Harness::with_dirs();
    h.call("notes.create", note_payload("before backup", "content"))
        .unwrap();
    let created = h
        .call("backup.create", json!({ "label": "nightly" }))
        .unwrap();
    assert_eq!(created["result"]["state"], "SNAPSHOT_CREATED");
    assert_eq!(created["result"]["integrity"], "UNKNOWN");
    assert_eq!(created["result"]["label"], "nightly");
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let bid = BackupId::parse(&id).unwrap();
    let snapshot = h
        .dir
        .as_ref()
        .unwrap()
        .path()
        .join("backups")
        .join(format!("{id}.sqlite"));
    assert!(snapshot.exists());

    let premature = h
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap_err();
    assert_eq!(premature.code, "backup.illegal_transition");

    let verified = h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    assert_eq!(verified["result"]["state"], "CHECKED");
    assert_eq!(verified["result"]["integrity"], "OK");
    let tested = h
        .call("backup.restore_test", json!({ "backup_id": id }))
        .unwrap();
    assert_eq!(tested["result"]["state"], "RETAINED");
    assert!(tested["result"]["restore_tested_at"].as_i64().is_some());
    assert_eq!(
        g9_storage::backup::manifest_state(h.conn(), &bid).unwrap(),
        Some(BackupState::Retained)
    );
    let scratch: Vec<_> = std::fs::read_dir(h.dir.as_ref().unwrap().path().join("scratch"))
        .unwrap()
        .collect();
    assert!(scratch.is_empty(), "the disposable restore copy is removed");
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["last_backup"]["state"], "RETAINED");
}

// ---------------------------------------------------------------------------
// A09 model down
// ---------------------------------------------------------------------------

#[test]
fn a09_all_deterministic_commands_work_with_null_runtime() {
    let mut h = Harness::new();
    insert_session(&h, "session-a09");
    let day = json!({ "from": 0, "to": T0 + 86_400_000 });
    for (op, payload) in [
        ("system.status", json!({})),
        (
            "curriculum.validate",
            json!({ "manifest": seed_manifest() }),
        ),
        ("calendar.list", day.clone()),
        ("session.list", json!({})),
        ("session.prepare", json!({ "session_id": "session-a09" })),
        ("session.start", json!({ "session_id": "session-a09" })),
        ("session.complete", json!({ "session_id": "session-a09" })),
        (
            "notes.create",
            note_payload("offline", "works without a model"),
        ),
        ("assessment.list", json!({})),
        ("competency.projection", json!({})),
        ("retention.due", json!({})),
        ("interventions.list", json!({})),
        ("approvals.list", json!({})),
        ("audit.recent", json!({})),
        (
            "calendar.propose",
            json!({ "from": T0, "to": T0 + 86_400_000, "intent": "ADD_REVIEWS" }),
        ),
    ] {
        let out = h.call(op, payload);
        assert!(out.is_ok(), "{op}: {out:?}");
    }
    let validate = h
        .call(
            "curriculum.validate",
            json!({ "manifest": seed_manifest() }),
        )
        .unwrap();
    assert_eq!(validate["result"]["valid"], true);
    // Skills need a runtime and a registry: an explicit, retryable refusal.
    let skills = h
        .call("skills.invoke", json!({ "skill_code": "T01", "input": {} }))
        .unwrap_err();
    assert_eq!(skills.category, ErrorCategory::ResourceUnavailable);

    // With a registry but no model, the orchestrator reports model-down honestly.
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let mut h = Harness::with(|s| s.skills = Some(registry));
    let down = h
        .call(
            "skills.invoke",
            json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" } }),
        )
        .unwrap_err();
    assert_eq!(
        down.category,
        ErrorCategory::ResourceUnavailable,
        "{down:?}"
    );
}

// ---------------------------------------------------------------------------
// A10 deletion
// ---------------------------------------------------------------------------

#[test]
fn a10_deletion_request_gated_and_tombstones_content() {
    let mut h = Harness::new();
    let body = "private reflection that must be erasable";
    let created = h
        .call("notes.create", note_payload("reflection", body))
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    let payload = json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "operator erasure request" });
    let err = h.call("deletion.request", payload.clone()).unwrap_err();
    assert_eq!(err.category, ErrorCategory::ApprovalRequired);
    assert!(!serde_json::to_string(&err).unwrap().contains(body));
    // Nothing erased before the approval is consumed.
    let nid = NoteId::parse(&note_id).unwrap();
    assert_eq!(
        evidence_repo::get_note(h.conn(), &learning(), &nid)
            .unwrap()
            .unwrap()
            .body_markdown,
        body
    );

    let id = err.approval_id.unwrap();
    h.decide(id.as_str(), "APPROVE");
    let done = h
        .call_with("deletion.request", payload, &Opts::approval(id.as_str()))
        .unwrap();
    assert_eq!(done["result"]["state"], "APPLIED");
    assert_eq!(
        done["result"]["tombstone_sha256"],
        json!(g9_domain::hashing::sha256_hex(body.as_bytes()))
    );
    let note = evidence_repo::get_note(h.conn(), &learning(), &nid)
        .unwrap()
        .unwrap();
    assert!(note.erased);
    assert_ne!(note.body_markdown, body);
    let request = g9_storage::deletion::get_request(
        h.conn(),
        done["result"]["deletion_id"].as_str().unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(request.state, g9_storage::deletion::DeletionState::Executed);
    assert_eq!(request.approval_id, Some(id.clone()));
    assert!(!request.report.unwrap().tombstones.is_empty());
    assert_eq!(
        approvals_store::get_request(h.conn(), &id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Consumed
    );
    let events = g9_storage::events::list_since(h.conn(), 0, 100).unwrap();
    assert!(events
        .iter()
        .all(|e| !e.event.payload.to_string().contains(body)));
}

// ---------------------------------------------------------------------------
// Dispatch and status
// ---------------------------------------------------------------------------

#[test]
fn dispatch_rejects_unknown_op_and_invalid_payload_against_schema() {
    let mut h = Harness::new();
    let rows = h.audit_rows();
    let unknown = h.call("nope.operation", json!({})).unwrap_err();
    assert_eq!(unknown.category, ErrorCategory::InvalidInput);
    assert_eq!(unknown.code, "ipc.unknown_operation");
    let later = h.call("teaching.propose_lesson", json!({})).unwrap_err();
    assert_eq!(later.category, ErrorCategory::PreconditionFailed);
    assert_eq!(later.code, "teaching.not_implemented");

    for bad in [
        json!({ "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING" }),
        json!({ "title": "t", "body_markdown": "b", "classification": "TOP", "domain": "LEARNING" }),
        json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "LEARNING", "extra": 1 }),
        json!("not an object"),
    ] {
        let err = h.call("notes.create", bad).unwrap_err();
        assert_eq!(err.code, "ipc.payload_invalid");
    }
    let mut meta = h.meta("notes.create", &Opts::default());
    meta["payload_schema_version"] = json!({ "major": 2, "minor": 0 });
    let version = h
        .raw("notes.create", meta, note_payload("t", "b"))
        .unwrap_err();
    assert_eq!(version.code, "ipc.schema_version_rejected");
    let mut meta = h.meta("notes.create", &Opts::default());
    meta["operation"] = json!("notes.revise");
    let mismatch = h
        .raw("notes.create", meta, note_payload("t", "b"))
        .unwrap_err();
    assert_eq!(mismatch.code, "meta.operation_mismatch");
    assert_eq!(h.audit_rows(), rows, "decode failures never reach storage");
}

#[test]
fn status_reports_model_unavailable_honestly() {
    let mut h = Harness::new();
    let s = h.call("system.status", json!({})).unwrap();
    let r = &s["result"];
    assert_eq!(
        r["model"],
        json!({ "available": false, "reason": "NO_RUNTIME_CONFIGURED" })
    );
    assert_eq!(r["network"], json!({ "egress_allowed": false }));
    assert_eq!(r["transport"], "tauri");
    assert_eq!(r["storage"]["state"], "READY");
    assert_eq!(
        r["storage"]["migrations_applied"],
        g9_storage::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(r["clock"]["now"], T0);
    assert_eq!(r["clock"]["monotonic_ok"], true);
    assert_eq!(r["pending_approvals"], 0);
    assert_eq!(r["last_backup"], Value::Null);
    let _ = h.call("export.request", export_payload(false)).unwrap_err();
    let s = h.call("system.status", json!({})).unwrap();
    assert_eq!(s["result"]["pending_approvals"], 1);

    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("local-7b").unwrap()),
        4096,
    ));
    let mut h = Harness::with(|s| s.runtime = Some(rt));
    let s = h.call("system.status", json!({})).unwrap();
    assert_eq!(
        s["result"]["model"],
        json!({ "available": true, "model_id": "local-7b" })
    );
}

#[test]
fn approval_expiry_is_swept_on_open_and_cannot_be_consumed() {
    let mut h = Harness::new();
    let err = h.call("export.request", export_payload(false)).unwrap_err();
    let id: ApprovalId = err.approval_id.unwrap();
    h.decide(id.as_str(), "APPROVE");
    h.advance(16 * 60_000);
    let late = h
        .call_with(
            "export.request",
            export_payload(false),
            &Opts::approval(id.as_str()),
        )
        .unwrap_err();
    assert_eq!(late.code, "approval.expired");
    assert_eq!(
        approvals_store::get_request(h.conn(), &id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Expired
    );
    // A pending request past expiry is moved to EXPIRED by the open-time sweep.
    let err = h.call("export.request", export_payload(true)).unwrap_err();
    let pending_id = err.approval_id.unwrap();
    h.advance(16 * 60_000);
    h.restart();
    assert!(h
        .ctx()
        .recovery_report()
        .expired_approvals
        .contains(&pending_id));
    assert_eq!(
        approvals_store::get_request(h.conn(), &pending_id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Expired
    );
}

// ---------------------------------------------------------------------------
// Recovery: migrations only after a verified backup
// ---------------------------------------------------------------------------

/// Migration versions after 9 embedded in this build (pending on a v9 database).
fn after_version_9() -> Vec<u32> {
    g9_storage::EMBEDDED_MIGRATIONS
        .iter()
        .map(|m| m.version)
        .filter(|v| *v > 9)
        .collect()
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
    assert_eq!(db.pending_migrations(), after_version_9().as_slice());
    assert_eq!(db.pending_migrations().first(), Some(&10));
    db
}

#[test]
fn recovery_migrates_only_after_a_verified_backup() {
    let dir = tempfile::Builder::new().prefix("g9mig").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("backups")).unwrap();
    std::fs::create_dir_all(root.join("scratch")).unwrap();

    // Without a backup directory the schema stays unmigrated and writes are refused.
    let blocked_path = root.join("blocked.sqlite");
    let mut blocked = Harness::with_database(database_at_version_9(&blocked_path), |_| {}).unwrap();
    let report = blocked.ctx().recovery_report().clone();
    assert_eq!(report.pending_migrations, after_version_9());
    assert!(report.migration_blocked_reason.is_some());
    let status = blocked.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["storage"]["state"], "READ_ONLY");
    let refused = blocked
        .call("notes.create", note_payload("t", "b"))
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::DataCorruption);
    assert_eq!(refused.code, "recovery.integrity_failed");

    // With an approved backup directory: snapshot, verify, dry run, then migrate.
    let path = root.join("live.sqlite");
    let r = root.clone();
    let mut h = Harness::with_database(database_at_version_9(&path), |s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    })
    .unwrap();
    let report = h.ctx().recovery_report().clone();
    assert_eq!(report.migrated, after_version_9());
    assert!(report.pending_migrations.is_empty());
    let backup = report.pre_migration_backup.unwrap();
    assert_eq!(
        g9_storage::backup::manifest_state(h.conn(), &backup).unwrap(),
        Some(BackupState::Checked)
    );
    assert!(root
        .join("backups")
        .join(format!("{}.pre-migration.sqlite", backup.as_str()))
        .exists());
    let status = h.call("system.status", json!({})).unwrap();
    assert_eq!(status["result"]["storage"]["state"], "READY");
    assert_eq!(status["result"]["storage"]["wal"], true);
    h.call("notes.create", note_payload("t", "after migration"))
        .unwrap();
}

// ---------------------------------------------------------------------------
// Calendar apply and daily plan
// ---------------------------------------------------------------------------

#[test]
fn calendar_apply_requires_schedule_apply_approval() {
    let mut h = Harness::new();
    let proposal = h
        .call(
            "calendar.propose",
            json!({ "from": T0, "to": T0 + 86_400_000, "intent": "ADD_REVIEWS" }),
        )
        .unwrap();
    assert_eq!(proposal["result"]["feasible"], true);
    let id = proposal["result"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_string();
    let unsupported = h
        .call(
            "calendar.propose",
            json!({ "from": T0, "to": T0 + 86_400_000, "intent": "FILL_CURRICULUM" }),
        )
        .unwrap_err();
    assert_eq!(unsupported.code, "calendar.fill_curriculum.not_implemented");
    let applied = h.approved(
        "calendar.apply",
        json!({ "proposal_id": id }),
        &Opts::default(),
    );
    assert_eq!(applied["result"]["applied"], 0);
    let row = g9_storage::repos::calendar::get_proposal(
        h.conn(),
        &g9_domain::ScheduleProposalId::parse(&id).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        row.state,
        g9_storage::repos::calendar::ProposalState::Applied
    );
    let decisions = g9_storage::repos::calendar::list_decisions(h.conn(), &row.id).unwrap();
    assert_eq!(decisions.len(), 1);
    assert!(decisions[0].approval_id.is_some());
    // An applied proposal cannot be applied again.
    let again = h
        .call("calendar.apply", json!({ "proposal_id": id }))
        .unwrap_err();
    assert_ne!(again.category, ErrorCategory::ApprovalRequired);
}

#[test]
fn daily_plan_reads_imported_structure_and_labels_non_canonical() {
    let mut h = Harness::with(|s| {
        s.config.program_start = Some(g9_curriculum::CivilDate::new(2026, 11, 2).unwrap());
    });
    let none = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap_err();
    assert_eq!(none.code, "curriculum.not_imported");
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    let plan = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap();
    let r = &plan["result"];
    assert_eq!(r["day_id"], "D02");
    assert_eq!(r["week_number"], 1);
    assert_eq!(r["planned_minutes"], 420);
    assert_eq!(r["objectives"].as_array().unwrap().len(), 3);
    assert!(!r["blocks"].as_array().unwrap().is_empty());
    assert!(plan["warnings"][0]
        .as_str()
        .unwrap()
        .starts_with("curriculum.non_canonical"));
    let sunday = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-08" }))
        .unwrap();
    assert_eq!(sunday["result"]["unscheduled"], true);
    let saturday = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-07" }))
        .unwrap();
    assert_eq!(saturday["result"]["planned_minutes"], 300);
    assert_eq!(saturday["result"]["is_assessment_day"], true);
}
