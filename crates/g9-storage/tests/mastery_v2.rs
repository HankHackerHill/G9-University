#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The second structure profile (ADR-0023) in storage: a 60-day `MASTERY_V2` version imports with
//! no migration and no DDL change, its profile travels in `invariants_json`, and both shapes live
//! side by side in one database.
//!
//! The v2 manifest is generated (`g9-curriculum/tests/common/mastery_v2.rs`): a structural fixture
//! with placeholder texts, not the owner's syllabus.

mod common;
#[path = "../../g9-curriculum/tests/common/mastery_v2.rs"]
mod v2;

use common::{count, open_mem, operator, t};
use g9_curriculum::{load_manifest, manifest_checksum, Manifest, ValidationReport};
use g9_domain::{
    Action, ApprovalId, Capability, CurriculumId, CurriculumVersionId, DataDomain, DayId,
    DayOfWeek, InteractionMode, ManifestProvenance, OperationDescriptor, ResourceKind, ResourceRef,
    Revision, Scope, StructureProfileId, UtcMillis, CT9_480, MASTERY_V2,
};
use g9_storage::approvals_store::{insert_request, NewApprovalRequest};
use g9_storage::repos::curriculum::{self as repo, NewCurriculumVersion, ProvenanceApproval};
use g9_storage::repos::RepoError;
use g9_storage::StorageError;
use rusqlite::Connection;

const SEED_V0: &str = include_str!("../../../data/curriculum/g9-ct9-480.seed.v0.json");

fn v2_manifest(draft: bool) -> (Manifest, ValidationReport) {
    let value = if draft { v2::draft() } else { v2::seed() };
    load_manifest(&serde_json::to_string(&value).unwrap()).expect("the generated v2 manifest loads")
}

fn import(
    conn: &Connection,
    id: &str,
    manifest: &Manifest,
    report: &ValidationReport,
) -> CurriculumVersionId {
    import_at(conn, id, manifest, report, t(0))
}

fn import_at(
    conn: &Connection,
    id: &str,
    manifest: &Manifest,
    report: &ValidationReport,
    at: UtcMillis,
) -> CurriculumVersionId {
    let checksum = manifest_checksum(manifest).unwrap();
    repo::insert_version(
        conn,
        &NewCurriculumVersion {
            id: CurriculumVersionId::parse(id).unwrap(),
            manifest,
            validation: report,
            provenance: manifest.provenance,
            checksum: &checksum,
            source_ref: "tests/mastery_v2".into(),
            imported_by: operator().id,
            imported_at: at,
        },
    )
    .unwrap()
}

/// An approval request to cite when a version becomes canonical.
fn approval(conn: &Connection, id: &str) -> ApprovalId {
    insert_request(
        conn,
        &NewApprovalRequest {
            id: ApprovalId::parse(id).unwrap(),
            operation: OperationDescriptor {
                action: Action::Approve,
                resource: ResourceRef::new(ResourceKind::CurriculumVersion, "x"),
                scope: Scope {
                    domain: DataDomain::Learning,
                    mode: InteractionMode::Maintenance,
                },
                parameters_sha256: common::sha("params"),
            },
            required_capability: Some(Capability::CurriculumPromote),
            requested_by: operator().id,
            requested_at: t(0),
            expires_at: t(15 * 60_000),
            expected_revision: Some(Revision::INITIAL),
        },
    )
    .unwrap()
    .id
}

fn promote(conn: &Connection, id: &CurriculumVersionId, approval_id: &str, at: i64) {
    repo::set_provenance(
        conn,
        id,
        Revision::INITIAL,
        ManifestProvenance::ReconciledCandidate,
        None,
        t(at),
    )
    .unwrap();
    repo::set_provenance(
        conn,
        id,
        Revision::new(2),
        ManifestProvenance::ApprovedCanonical,
        Some(&ProvenanceApproval {
            approval_id: approval(conn, approval_id),
            approved_by: operator().id,
            approved_at: t(at),
        }),
        t(at),
    )
    .unwrap();
}

#[test]
fn a_sixty_day_version_imports_into_the_unchanged_tables() {
    let db = open_mem();
    let conn = db.conn();
    let (manifest, report) = v2_manifest(false);
    assert!(report.is_valid());
    let id = import(conn, "cv-v2", &manifest, &report);

    assert_eq!(count(conn, "weeks"), 12);
    assert_eq!(count(conn, "training_days"), 60);
    assert_eq!(count(conn, "daily_objectives"), 180);
    assert_eq!(count(conn, "study_blocks"), 300, "five blocks a day");
    assert_eq!(count(conn, "deliverable_definitions"), 60);
    let minutes: i64 = conn
        .query_row("SELECT SUM(planned_minutes) FROM training_days", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(minutes, 28_800);

    // Weeks hold 2,400 minutes; day numbers run 1 to 60 in program order.
    let weeks = repo::list_weeks(conn, &id).unwrap();
    assert_eq!(weeks.len(), 12);
    assert!(weeks.iter().all(|w| w.planned_minutes == 2_400));
    let days = repo::list_training_days(conn, &id).unwrap();
    let numbers: Vec<u32> = days.iter().map(|d| d.day_number).collect();
    assert_eq!(numbers, (1..=60).collect::<Vec<u32>>());
    for d in &days {
        assert_eq!(d.planned_minutes, 480, "{}", d.day_code);
        assert!((1..=5).contains(&d.day_of_week.get()), "{}", d.day_code);
        // Week w holds D(5w-4) to D(5w): the code and the number agree.
        assert_eq!(d.day_code.as_str(), format!("D{:02}", d.day_number));
    }
    assert_eq!(days[59].day_code.as_str(), "D60");
    assert_eq!(days[59].day_of_week.get(), 5);

    // A Monday and a Friday: the codes of the ADR table, in the modes of the codes.
    let monday = &days[0];
    let friday = &days[4];
    let shape = |day: &g9_storage::repos::curriculum::TrainingDayRow| -> Vec<(String, u32, InteractionMode)> {
        repo::study_blocks(conn, &day.id)
            .unwrap()
            .into_iter()
            .map(|b| (b.block_code, b.minutes, b.mode))
            .collect()
    };
    let s = |code: &str, minutes, mode| (code.to_string(), minutes, mode);
    assert_eq!(
        shape(monday),
        vec![
            s("recall", 30, InteractionMode::Review),
            s("theory", 120, InteractionMode::Study),
            s("guided_lab", 120, InteractionMode::Lab),
            s("independent_lab", 150, InteractionMode::Lab),
            s("defense_verification", 60, InteractionMode::Lab),
        ]
    );
    assert_eq!(
        shape(friday),
        vec![
            s("recall", 30, InteractionMode::Review),
            s("theory", 120, InteractionMode::Study),
            s("performance_review", 120, InteractionMode::Review),
            s("blind_practical", 150, InteractionMode::Exam),
            s("report", 60, InteractionMode::Study),
        ]
    );
    // The scored practical is the only EXAM block of the week.
    let exam_blocks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM study_blocks WHERE mode = 'EXAM' AND training_day_id IN
               (SELECT id FROM training_days WHERE week_id = ?1)",
            [&weeks[0].id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exam_blocks, 1);
    // The Friday's evidence is an attempt submission, the others notes.
    assert_eq!(
        repo::deliverables(conn, &friday.id).unwrap()[0].evidence_kind,
        "ATTEMPT_SUBMISSION"
    );
    assert_eq!(
        repo::deliverables(conn, &monday.id).unwrap()[0].evidence_kind,
        "NOTE"
    );

    // Lookups by (week, day of week): a Friday exists, a Saturday does not.
    let objectives =
        |week, dow| repo::daily_objectives(conn, &id, week, DayOfWeek::new(dow).unwrap()).unwrap();
    assert_eq!(objectives(12, 5).len(), 3);
    assert_eq!(objectives(12, 5)[0].objective_code.as_str(), "D60-O1");
    assert!(objectives(12, 6).is_empty());
    assert_eq!(
        repo::training_day(conn, &id, &DayId::parse("D30").unwrap())
            .unwrap()
            .unwrap()
            .day_of_week
            .get(),
        5,
        "the midpoint is a Friday"
    );
    assert_eq!(repo::count_objectives(conn, &id).unwrap(), 180);
}

#[test]
fn the_stored_report_carries_the_profile_and_reads_back_as_it() {
    let db = open_mem();
    let conn = db.conn();
    let (manifest, report) = v2_manifest(false);
    let id = import(conn, "cv-v2", &manifest, &report);
    let row = repo::get_version(conn, &id).unwrap().unwrap();

    assert_eq!(row.invariants["profile"], "MASTERY_V2");
    assert_eq!(row.invariants["invariants"]["total_days"], 60);
    assert_eq!(row.invariants["invariants"]["days_per_week"], 5);
    assert_eq!(row.invariants["invariants"]["weekday_minutes"], 480);
    assert_eq!(row.invariants["counts"]["objectives"], 180);
    assert_eq!(row.structure_profile().unwrap(), &MASTERY_V2);
    assert_eq!(row.version_label, v2::SEED_VERSION);
    assert_eq!(
        row.manifest_sha256,
        manifest_checksum(&manifest).unwrap().as_str(),
        "the checksum covers the declared assessment day"
    );

    // The stored report is the validator's report, so it reads back as the same report.
    let back: ValidationReport = serde_json::from_value(row.invariants.clone()).unwrap();
    assert_eq!(back, report);
}

#[test]
fn both_shapes_live_side_by_side_in_one_database() {
    let db = open_mem();
    let conn = db.conn();
    let (ct9, ct9_report) = load_manifest(SEED_V0).unwrap();
    let ct9_id = import(conn, "cv-ct9", &ct9, &ct9_report);
    let (manifest, report) = v2_manifest(false);
    let v2_id = import(conn, "cv-v2", &manifest, &report);
    let (draft, draft_report) = v2_manifest(true);
    let v2_draft_id = import(conn, "cv-v2-draft", &draft, &draft_report);

    // Two curricula, three versions; every table holds the sum of the shapes.
    assert_eq!(count(conn, "curricula"), 2);
    assert_eq!(count(conn, "curriculum_versions"), 3);
    assert_eq!(count(conn, "training_days"), 72 + 60 + 60);
    assert_eq!(count(conn, "daily_objectives"), 216 + 180 + 180);
    assert_eq!(count(conn, "weeks"), 36);
    // Appendix A: 60 weekdays of six blocks and 12 Saturdays of four; v2: 60 days of five, twice.
    assert_eq!(
        count(conn, "study_blocks"),
        (60 * 6 + 12 * 4) + 2 * (60 * 5)
    );

    // Each version reads back as its own profile and holds its own tree.
    let row = |id: &CurriculumVersionId| repo::get_version(conn, id).unwrap().unwrap();
    assert_eq!(row(&ct9_id).structure_profile().unwrap(), &CT9_480);
    assert_eq!(row(&v2_id).structure_profile().unwrap(), &MASTERY_V2);
    assert_eq!(row(&v2_draft_id).structure_profile().unwrap(), &MASTERY_V2);
    assert_eq!(row(&ct9_id).invariants["profile"], "CT9_480");
    assert_eq!(row(&ct9_id).invariants["invariants"]["total_days"], 72);
    assert_eq!(repo::list_training_days(conn, &ct9_id).unwrap().len(), 72);
    assert_eq!(repo::list_training_days(conn, &v2_id).unwrap().len(), 60);
    assert_eq!(repo::count_objectives(conn, &ct9_id).unwrap(), 216);
    assert_eq!(repo::count_objectives(conn, &v2_id).unwrap(), 180);
    // The Appendix A tree still has its Saturday; the v2 tree has none.
    let saturdays = |id: &CurriculumVersionId| -> usize {
        repo::list_training_days(conn, id)
            .unwrap()
            .iter()
            .filter(|d| d.day_of_week.get() == 6)
            .count()
    };
    assert_eq!(saturdays(&ct9_id), 12);
    assert_eq!(saturdays(&v2_id), 0);

    // The versions of the two curricula are told apart by curriculum id.
    let cyber = CurriculumId::parse(v2::CURRICULUM_ID).unwrap();
    let ct9_curriculum = CurriculumId::parse("g9-ct9-480").unwrap();
    assert_eq!(repo::list_versions(conn, &cyber).unwrap().len(), 2);
    assert_eq!(repo::list_versions(conn, &ct9_curriculum).unwrap().len(), 1);

    // Provenance moves exactly as for the other shape: forward, with the candidate step first.
    assert!(matches!(
        repo::set_provenance(
            conn,
            &v2_draft_id,
            Revision::INITIAL,
            ManifestProvenance::ApprovedCanonical,
            None,
            t(10)
        ),
        Err(RepoError::Storage(StorageError::Precondition(_)))
    ));
    repo::set_provenance(
        conn,
        &v2_draft_id,
        Revision::INITIAL,
        ManifestProvenance::ReconciledCandidate,
        None,
        t(10),
    )
    .unwrap();
    assert!(repo::current_version(conn, &cyber).unwrap().is_none());
    // A version of another curriculum is untouched.
    assert_eq!(row(&ct9_id).provenance, ManifestProvenance::ProposedSeed);

    // Deleting one shape's non-canonical version leaves the other whole.
    repo::delete_version(conn, &v2_id, Revision::INITIAL).unwrap();
    assert_eq!(count(conn, "training_days"), 72 + 60);
    assert_eq!(repo::list_training_days(conn, &ct9_id).unwrap().len(), 72);
}

#[test]
fn a_report_for_another_profile_is_refused_before_anything_is_written() {
    let db = open_mem();
    let conn = db.conn();
    let (manifest, report) = v2_manifest(false);
    let (ct9, ct9_report) = load_manifest(SEED_V0).unwrap();

    // The v2 manifest with the Appendix A report, and the reverse.
    for (m, r) in [(&manifest, &ct9_report), (&ct9, &report)] {
        let checksum = manifest_checksum(m).unwrap();
        let err = repo::insert_version(
            conn,
            &NewCurriculumVersion {
                id: CurriculumVersionId::parse("cv-wrong").unwrap(),
                manifest: m,
                validation: r,
                provenance: m.provenance,
                checksum: &checksum,
                source_ref: "tests/mastery_v2".into(),
                imported_by: operator().id,
                imported_at: t(0),
            },
        )
        .unwrap_err();
        assert!(
            matches!(&err, RepoError::Storage(StorageError::Precondition(m)) if m.contains("profile")),
            "{err:?}"
        );
    }
    assert_eq!(count(conn, "curriculum_versions"), 0);
    assert_eq!(count(conn, "training_days"), 0);

    // An invalid report still blocks the import under its own profile.
    let mut broken = manifest.clone();
    broken.weeks[0].days[0].planned_minutes = 420;
    let broken_report = g9_curriculum::validate(&broken);
    assert_eq!(broken_report.profile, StructureProfileId::MasteryV2);
    assert!(!broken_report.is_valid());
    let checksum = manifest_checksum(&broken).unwrap();
    assert!(repo::insert_version(
        conn,
        &NewCurriculumVersion {
            id: CurriculumVersionId::parse("cv-broken").unwrap(),
            manifest: &broken,
            validation: &broken_report,
            provenance: broken.provenance,
            checksum: &checksum,
            source_ref: "tests/mastery_v2".into(),
            imported_by: operator().id,
            imported_at: t(0),
        },
    )
    .is_err());
    assert_eq!(count(conn, "curriculum_versions"), 0);
}

#[test]
fn a_stored_profile_that_is_not_known_is_refused_not_guessed() {
    let db = open_mem();
    let conn = db.conn();
    let (manifest, report) = v2_manifest(false);
    let id = import(conn, "cv-v2", &manifest, &report);
    let mut row = repo::get_version(conn, &id).unwrap().unwrap();

    // No `profile` key, or a null one: the shape every version had before ADR-0023.
    row.invariants = serde_json::json!({});
    assert_eq!(row.structure_profile().unwrap(), &CT9_480);
    row.invariants = serde_json::json!({ "profile": null });
    assert_eq!(row.structure_profile().unwrap(), &CT9_480);
    row.invariants = serde_json::json!({ "profile": "CT9_480" });
    assert_eq!(row.structure_profile().unwrap(), &CT9_480);

    // A profile this build does not know, or a value of another type, is data corruption.
    for bad in [
        serde_json::json!({ "profile": "MASTERY_V3" }),
        serde_json::json!({ "profile": "ct9_480" }),
        serde_json::json!({ "profile": 5 }),
        serde_json::json!({ "profile": ["MASTERY_V2"] }),
    ] {
        row.invariants = bad.clone();
        let err = row.structure_profile().unwrap_err();
        let g9: g9_domain::G9Error = err.into();
        assert_eq!(
            g9.category,
            g9_domain::ErrorCategory::DataCorruption,
            "{bad}"
        );
        assert!(
            !g9.message.contains("MASTERY_V3"),
            "no stored text in the error"
        );
    }
}

#[test]
fn the_active_version_is_the_latest_canonical_one_else_the_latest_import_across_curricula() {
    let db = open_mem();
    let conn = db.conn();
    assert!(repo::active_version(conn).unwrap().is_none());
    assert!(repo::list_all_versions(conn).unwrap().is_empty());
    assert!(repo::current_versions(conn).unwrap().is_empty());

    // Nothing canonical: the most recently imported version, whichever curriculum it belongs to.
    let (ct9, ct9_report) = load_manifest(SEED_V0).unwrap();
    let ct9_id = import_at(conn, "cv-ct9", &ct9, &ct9_report, t(10));
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, ct9_id);
    let (seed, seed_report) = v2_manifest(false);
    let seed_id = import_at(conn, "cv-v2-seed", &seed, &seed_report, t(20));
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, seed_id);
    let (draft, draft_report) = v2_manifest(true);
    let draft_id = import_at(conn, "cv-v2-draft", &draft, &draft_report, t(30));
    let order: Vec<&str> = ["cv-ct9", "cv-v2-seed", "cv-v2-draft"].to_vec();
    assert_eq!(
        repo::list_all_versions(conn)
            .unwrap()
            .iter()
            .map(|v| v.id.as_str().to_string())
            .collect::<Vec<_>>(),
        order,
        "oldest import first, both curricula"
    );
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, draft_id);

    // A promoted version beats a later import of either shape ...
    promote(conn, &ct9_id, "apr-ct9", 100);
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, ct9_id);
    assert_eq!(repo::current_versions(conn).unwrap().len(), 1);
    let (late, late_report) = {
        let mut m = draft.clone();
        m.version = "2.0.0-draft2".into();
        let r = g9_curriculum::validate(&m);
        (m, r)
    };
    import_at(conn, "cv-v2-draft2", &late, &late_report, t(200));
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, ct9_id);

    // ... and the most recently approved of two curricula wins.
    promote(conn, &draft_id, "apr-v2", 300);
    let current: Vec<String> = repo::current_versions(conn)
        .unwrap()
        .iter()
        .map(|v| v.id.as_str().to_string())
        .collect();
    assert_eq!(current, ["cv-v2-draft", "cv-ct9"]);
    assert_eq!(repo::active_version(conn).unwrap().unwrap().id, draft_id);
    assert_eq!(
        repo::active_version(conn)
            .unwrap()
            .unwrap()
            .structure_profile()
            .unwrap(),
        &MASTERY_V2
    );
}
