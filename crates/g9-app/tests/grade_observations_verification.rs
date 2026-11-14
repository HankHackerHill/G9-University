#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Independent verification of the grade-decision -> competency-observation
//! change (migration 0013, `assessment.approve_final`, `competency.projection`).
//! Every test here drives `g9_app::dispatch` (or a real file database that is
//! migrated by `g9-app` recovery) and was written against the claims of the
//! implementation report, not against its code:
//!
//! * the finalization is atomic: a failure after an observation was appended
//!   leaves nothing of the decision behind;
//! * migration 0013 applies to a database that is already at version 12 and holds
//!   assessment data, and the old rows keep deriving nothing;
//! * blindness takes earlier exposure of the same item into account (LRN-13).

mod common;

use common::*;
use g9_domain::{
    AssessmentAttemptState, AssessmentId, AssistanceLevel, AttemptId, Classification, CompetencyId,
    DataDomain, Difficulty, ErrorCategory, EvidenceId, InteractionMode, Revision, UtcMillis,
};
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::competency as competency_repo;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

const OBSERVATIONS_NOT_DERIVED: &str = "competency.observations_not_derived";

fn count(h: &Harness, table: &str) -> i64 {
    h.conn()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn levels_payload(attempt: &str, levels: [u8; 6]) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .zip(levels)
        .map(|(c, l)| {
            json!({ "criterion_code": c.code, "score": l, "source": "HUMAN_REVIEWER", "evidence_refs": [] })
        })
        .collect();
    json!({ "attempt_id": attempt, "criteria": criteria })
}

fn begin(h: &mut Harness, def: &AssessmentId) -> String {
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    begun["result"]["attempt_id"].as_str().unwrap().to_string()
}

/// submit .. review .. grade; returns the revision the finalization approval binds to.
fn submit_review_grade(h: &mut Harness, attempt: &str, levels: [u8; 6]) -> u64 {
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
    let graded = h
        .call(
            "assessment.grade_provisional",
            levels_payload(attempt, levels),
        )
        .unwrap();
    graded["revision"].as_u64().unwrap()
}

fn warnings(outcome: &Value) -> Vec<String> {
    outcome["warnings"]
        .as_array()
        .map(|w| w.iter().map(|x| x.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

fn nothing_of_the_decision_survives(h: &Harness, attempt: &str) {
    assert_eq!(
        count(h, "grade_decisions"),
        0,
        "no grade decision row survives"
    );
    assert_eq!(count(h, "competency_observations"), 0, "no observation");
    assert_eq!(
        count(h, "competency_observation_evidence"),
        0,
        "no evidence row"
    );
    let stored = assessment_repo::get_attempt(
        h.conn(),
        &g9_storage::ReadScope::cross_domain(DataDomain::Learning),
        &AttemptId::parse(attempt).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(stored.state, AssessmentAttemptState::GradedProvisional);
    assert_eq!(stored.finalized_at, None);
}

/// A failure after the first observation row was appended (its evidence row is
/// refused) rolls the whole finalization back: no decision, no observation, no
/// evidence, the attempt is still GRADED_PROVISIONAL, and the same approval can
/// finalize once the cause is gone, producing exactly one set of observations.
#[test]
fn a_failure_after_the_observation_append_leaves_nothing_of_the_decision() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let attempt = begin(&mut h, &def);
    let rev = submit_review_grade(&mut h, &attempt, [4; 6]);
    h.conn()
        .execute_batch(
            "CREATE TRIGGER force_evidence_failure BEFORE INSERT ON competency_observation_evidence
             BEGIN SELECT RAISE(ABORT, 'forced failure after the observation append'); END;",
        )
        .unwrap();
    let payload = json!({ "attempt_id": attempt });
    let opts = Opts::revision(rev);
    let refused = h
        .call_with("assessment.approve_final", payload.clone(), &opts)
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::ApprovalRequired);
    let approval = refused.approval_id.clone().unwrap();
    h.decide(approval.as_str(), "APPROVE");
    let with_approval = Opts {
        approval_id: Some(approval.as_str().to_string()),
        ..opts
    };
    let failed = h
        .call_with("assessment.approve_final", payload.clone(), &with_approval)
        .unwrap_err();
    assert!(
        !matches!(failed.category, ErrorCategory::ApprovalRequired),
        "{failed:?}"
    );
    nothing_of_the_decision_survives(&h, &attempt);

    // the approval was not spent: with the cause removed the same approval finalizes
    h.conn()
        .execute_batch("DROP TRIGGER force_evidence_failure")
        .unwrap();
    let done = h
        .call_with("assessment.approve_final", payload, &with_approval)
        .unwrap();
    assert_eq!(done["result"]["state"], "APPROVED_FINAL");
    assert_eq!(count(&h, "grade_decisions"), 1);
    assert_eq!(count(&h, "competency_observations"), 3);
    assert_eq!(count(&h, "competency_observation_evidence"), 3);
}

/// The same, with the failure on the SECOND evidence row: the first observation
/// and its evidence row (already written inside the transaction) are rolled back
/// with the rest, so no half set of observations is ever visible.
#[test]
fn a_failure_on_a_later_evidence_row_rolls_back_the_rows_already_written() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let attempt = begin(&mut h, &def);
    let rev = submit_review_grade(&mut h, &attempt, [4; 6]);
    h.conn()
        .execute_batch(
            "CREATE TRIGGER force_second_evidence_failure BEFORE INSERT ON competency_observation_evidence
             WHEN (SELECT COUNT(*) FROM competency_observation_evidence) >= 1
             BEGIN SELECT RAISE(ABORT, 'forced failure on the second evidence row'); END;",
        )
        .unwrap();
    let payload = json!({ "attempt_id": attempt });
    let opts = Opts::revision(rev);
    let refused = h
        .call_with("assessment.approve_final", payload.clone(), &opts)
        .unwrap_err();
    let approval = refused.approval_id.clone().unwrap();
    h.decide(approval.as_str(), "APPROVE");
    let failed = h
        .call_with(
            "assessment.approve_final",
            payload,
            &Opts {
                approval_id: Some(approval.as_str().to_string()),
                ..opts
            },
        )
        .unwrap_err();
    assert!(
        !matches!(failed.category, ErrorCategory::ApprovalRequired),
        "{failed:?}"
    );
    nothing_of_the_decision_survives(&h, &attempt);
}

// ---------------------------------------------------------------------------
// migration 0013 on a database that is already at version 12 and holds data
// ---------------------------------------------------------------------------

fn at(ms: i64) -> UtcMillis {
    UtcMillis::from_millis(ms)
}

/// Rows a released build (schema version 12) holds: a competency, a blind
/// practical definition with ONE version that has no difficulty column yet,
/// its rubric, a finalized attempt with its final grade decision, and an attempt
/// that is still STARTED.
fn seed_version_12(conn: &rusqlite::Connection) {
    let long_ago = at(T0 - 7 * 86_400_000);
    let competency = g9_competency::Competency {
        id: CompetencyId::parse(COMPETENCY).unwrap(),
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
        &competency_repo::CompetencyRow::from_domain(&competency, "LINUX-BOOT", None, long_ago),
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
        &assessment_repo::AssessmentDefinitionRow::from_domain(&def, "W1-BLIND", long_ago).unwrap(),
    )
    .unwrap();
    // The version row as version 12 wrote it: the column `difficulty` does not exist yet.
    let task = "Recover the boot chain (variant 1)";
    conn.execute(
        "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, created_by, created_at)
         VALUES ('av-w1-1', 'ad-w1', '1', ?1, ?2, 'operator-1', ?3)",
        rusqlite::params![
            task,
            g9_domain::hashing::sha256_hex(task.as_bytes()),
            long_ago.as_millis()
        ],
    )
    .unwrap();
    let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
        &g9_assessment::Rubric::baseline(),
        "rub-w1-1",
        "av-w1-1",
        "Baseline",
        "v1",
        long_ago,
    );
    assessment_repo::insert_rubric(conn, &rubric, &criteria).unwrap();

    let attempt = |id: &str, state: AssessmentAttemptState, finalized: Option<UtcMillis>| {
        assessment_repo::AttemptRow {
            id: AttemptId::parse(id).unwrap(),
            assessment_version_id: "av-w1-1".into(),
            session_id: None,
            retake_of: None,
            state,
            mode: InteractionMode::Exam,
            assistance_level: AssistanceLevel::Unaided,
            hint_count: 0,
            started_at: Some(long_ago),
            submitted_at: finalized,
            finalized_at: finalized,
            submission_sha256: None,
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: long_ago,
            updated_at: long_ago,
        }
    };
    assessment_repo::insert_attempt(
        conn,
        &attempt(
            "attempt-v12-final",
            AssessmentAttemptState::ApprovedFinal,
            Some(long_ago),
        ),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO grade_decisions (id, attempt_id, total_score, passed, is_final, decided_by, decided_at, created_at, updated_at)
         VALUES ('gd-v12-final', 'attempt-v12-final', 100, 1, 1, 'operator-1', ?1, ?1, ?1)",
        rusqlite::params![long_ago.as_millis()],
    )
    .unwrap();
    assessment_repo::insert_attempt(
        conn,
        &attempt("attempt-v12-open", AssessmentAttemptState::Started, None),
    )
    .unwrap();
}

/// A file database at schema version 12 holding [`seed_version_12`] rows.
fn database_at_version_12(path: &std::path::Path) -> g9_storage::Database {
    let options = g9_storage::OpenOptions::new("0.0.12").applied_at(at(T0));
    let mut db = g9_storage::Database::open(path, options.clone().no_auto_migrate()).unwrap();
    g9_storage::Migrator::new("0.0.12", at(T0))
        .migrate_up_to(db.conn_mut(), 12)
        .unwrap();
    assert!(db
        .conn()
        .prepare("SELECT difficulty FROM assessment_versions")
        .is_err());
    seed_version_12(db.conn());
    drop(db);
    let db = g9_storage::Database::open(path, options).unwrap();
    assert_eq!(db.pending_migrations(), &[13, 14]);
    db
}

#[test]
fn migration_0013_applies_to_a_version_12_database_that_holds_assessment_data() {
    let dir = tempfile::Builder::new()
        .prefix("g9mig12")
        .tempdir()
        .unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("backups")).unwrap();
    std::fs::create_dir_all(root.join("scratch")).unwrap();
    let path = root.join("live.sqlite");
    let r = root.clone();
    let mut h = Harness::with_database(database_at_version_12(&path), |s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.backup_dir = Some(r.join("backups"));
        s.config.scratch_dir = Some(r.join("scratch"));
    })
    .unwrap();
    let report = h.ctx().recovery_report().clone();
    assert!(
        report.migration_blocked_reason.is_none(),
        "migration was blocked: {report:?}"
    );
    assert!(report.pending_migrations.is_empty(), "{report:?}");
    assert_eq!(report.migrated, vec![13, 14], "{report:?}");
    assert!(report.pre_migration_backup.is_some());

    // schema: the registry row carries the embedded checksum; the database is sound
    let (version, name): (i64, String) = h
        .conn()
        .query_row(
            "SELECT version, name FROM schema_migrations ORDER BY version DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((version, name.as_str()), (14, "0014_erasure_digests"));
    let applied_13: i64 = h
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 13 AND name = '0013_grade_observations'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(applied_13, 1);
    let integrity: String = h
        .conn()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let violations: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);

    // old data: the version declared no difficulty (never defaulted), the old final
    // decision and attempt are intact and have no observation or evidence row
    let version = assessment_repo::get_version(h.conn(), "av-w1-1")
        .unwrap()
        .unwrap();
    assert_eq!(version.difficulty, None);
    assert_eq!(count(&h, "grade_decisions"), 1);
    assert_eq!(count(&h, "competency_observations"), 0);
    assert_eq!(count(&h, "competency_observation_evidence"), 0);
    let projection = h
        .call(
            "competency.projection",
            json!({ "competency_id": COMPETENCY }),
        )
        .unwrap();
    assert_eq!(
        projection["result"]["projections"][0]["state"],
        "UNASSESSED"
    );
    assert_eq!(projection["result"]["projections"][0]["provisional"], false);

    // an attempt that was open before the migration finalizes after it: it derives
    // nothing and says why (the version has no declared difficulty)
    let rev = submit_review_grade(&mut h, "attempt-v12-open", [4; 6]);
    let outcome = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": "attempt-v12-open" }),
        &Opts::revision(rev),
    );
    assert_eq!(outcome["result"]["state"], "APPROVED_FINAL");
    let warned: Vec<String> = warnings(&outcome)
        .into_iter()
        .filter(|w| w.starts_with(OBSERVATIONS_NOT_DERIVED))
        .collect();
    assert_eq!(warned.len(), 1, "{:?}", warnings(&outcome));
    assert!(
        warned[0].contains("declares no difficulty"),
        "{}",
        warned[0]
    );
    assert_eq!(count(&h, "competency_observations"), 0);
    assert_eq!(count(&h, "competency_observation_evidence"), 0);

    // a version authored after the migration declares its difficulty and derives:
    // the next attempt (a retake of the latest terminal attempt) runs on it
    let version = g9_assessment::AssessmentVersion {
        definition_id: AssessmentId::parse("ad-w1").unwrap(),
        version: 2,
        rubric: g9_assessment::Rubric::baseline(),
        task_ref: EvidenceId::parse("evi-task-2").unwrap(),
        sealed_key_ref: None,
        difficulty: Difficulty::new(4).unwrap(),
        equivalence_group: "W1".into(),
        created_at: h.now(),
        checksum: String::new(),
    }
    .with_checksum()
    .unwrap();
    assessment_repo::insert_version(
        h.conn(),
        &assessment_repo::AssessmentVersionRow::from_domain(
            &version,
            "av-w1-2",
            "Recover the boot chain (variant 2)",
            None,
            operator_id(),
        )
        .unwrap(),
    )
    .unwrap();
    let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
        &version.rubric,
        "rub-w1-2",
        "av-w1-2",
        "Baseline",
        "v1",
        h.now(),
    );
    assessment_repo::insert_rubric(h.conn(), &rubric, &criteria).unwrap();
    h.advance(60_000);
    let second = begin(&mut h, &AssessmentId::parse("ad-w1").unwrap());
    let rev = submit_review_grade(&mut h, &second, [4; 6]);
    let outcome = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": second }),
        &Opts::revision(rev),
    );
    assert!(
        warnings(&outcome)
            .iter()
            .all(|w| !w.starts_with(OBSERVATIONS_NOT_DERIVED)),
        "{:?}",
        warnings(&outcome)
    );
    assert_eq!(count(&h, "competency_observations"), 3);
    assert_eq!(count(&h, "competency_observation_evidence"), 3);
    let derived =
        competency_repo::observations_for_attempt(h.conn(), &AttemptId::parse(&second).unwrap())
            .unwrap();
    assert!(derived.iter().all(|o| o.difficulty.get() == 4));
}

// ---------------------------------------------------------------------------
// blindness takes the item's earlier exposure into account (LRN-13)
// ---------------------------------------------------------------------------

/// The evidence rows of the observations derived from one attempt.
fn evidence_of(h: &Harness, attempt: &str) -> Vec<competency_repo::ObservationEvidenceRow> {
    let derived: Vec<_> =
        competency_repo::observations_for_attempt(h.conn(), &AttemptId::parse(attempt).unwrap())
            .unwrap()
            .into_iter()
            .map(|o| o.id)
            .collect();
    competency_repo::observation_evidence_for_competency(
        h.conn(),
        &CompetencyId::parse(COMPETENCY).unwrap(),
    )
    .unwrap()
    .into_iter()
    .filter(|e| derived.contains(&e.observation_id))
    .collect()
}

fn finalize(h: &mut Harness, def: &AssessmentId) -> (String, Value) {
    let attempt = begin(h, def);
    let rev = submit_review_grade(h, &attempt, [4; 6]);
    let outcome = h.approved(
        "assessment.approve_final",
        json!({ "attempt_id": attempt }),
        &Opts::revision(rev),
    );
    (attempt, outcome)
}

/// `assessment.begin` tells the operator that a retake on the only variant "is no
/// longer blind-eligible" (LRN-13: an item is consumed on its first exposure). The
/// immutable evidence rows must say the same: the first attempt is blind, the
/// retake on the same, already shown, variant is not.
/// FAILS ON THE IMPLEMENTATION UNDER TEST: the evidence of the retake said
/// `blind = true` because only the retake's own exposure rows were read.
#[test]
fn a_retake_on_an_already_shown_variant_records_non_blind_evidence() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let (first, _) = finalize(&mut h, &def);
    assert!(evidence_of(&h, &first).iter().all(|e| e.blind));
    h.advance(60_000);
    let (second, outcome) = finalize(&mut h, &def);
    assert!(
        warnings(&outcome)
            .iter()
            .all(|w| !w.starts_with(OBSERVATIONS_NOT_DERIVED)),
        "{:?}",
        warnings(&outcome)
    );
    let evidence = evidence_of(&h, &second);
    assert_eq!(evidence.len(), 3);
    assert!(
        evidence.iter().all(|e| !e.blind && !e.validation_eligible),
        "the variant was shown to the operator for the first attempt: {evidence:?}"
    );
}

/// An answer key that was shown to the operator before the attempt began (study
/// session, no attempt attached) makes the attempt non-blind.
/// FAILS ON THE IMPLEMENTATION UNDER TEST: only exposure rows tied to the attempt
/// itself were read.
#[test]
fn a_key_shown_before_the_attempt_began_makes_it_non_blind() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    assessment_repo::append_exposure(
        h.conn(),
        &assessment_repo::ExposureLogRow {
            id: "ex-key-study".into(),
            assessment_version_id: "av-w1-1".into(),
            attempt_id: None,
            exposed_to: assessment_repo::ExposedTo::Operator,
            actor_id: operator_id(),
            mode: InteractionMode::Study,
            content_kind: assessment_repo::ExposureContentKind::Key,
            content_sha256: "0".repeat(64),
            reason: Some("test: key read while studying".into()),
            exposed_at: at(T0 - 86_400_000),
            created_at: at(T0 - 86_400_000),
        },
    )
    .unwrap();
    let (attempt, _) = finalize(&mut h, &def);
    let evidence = evidence_of(&h, &attempt);
    assert_eq!(evidence.len(), 3);
    assert!(
        evidence.iter().all(|e| !e.blind && !e.validation_eligible),
        "{evidence:?}"
    );
}

/// Negative control for the two tests above: a first attempt on a variant nothing
/// was ever shown of stays blind.
#[test]
fn a_first_attempt_on_an_unexposed_variant_stays_blind() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    let (attempt, _) = finalize(&mut h, &def);
    let evidence = evidence_of(&h, &attempt);
    assert_eq!(evidence.len(), 3);
    assert!(evidence.iter().all(|e| e.blind), "{evidence:?}");
}

/// An assessment that is linked to no competency derives nothing and says so (the
/// second explicit "not derived" reason; no test exercised it).
#[test]
fn an_assessment_linked_to_no_competency_derives_nothing_and_warns() {
    let mut h = Harness::new();
    let def = seed_blind_practical(&h, 1, Some(3));
    h.conn()
        .execute("UPDATE assessment_definitions SET competency_id = NULL", [])
        .unwrap();
    let (_, outcome) = finalize(&mut h, &def);
    assert_eq!(outcome["result"]["state"], "APPROVED_FINAL");
    let warned: Vec<String> = warnings(&outcome)
        .into_iter()
        .filter(|w| w.starts_with(OBSERVATIONS_NOT_DERIVED))
        .collect();
    assert_eq!(warned.len(), 1, "{:?}", warnings(&outcome));
    assert!(
        warned[0].contains("linked to no competency"),
        "{}",
        warned[0]
    );
    assert_eq!(count(&h, "competency_observations"), 0);
    assert_eq!(count(&h, "competency_observation_evidence"), 0);
}
