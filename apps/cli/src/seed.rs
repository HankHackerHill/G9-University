//! Repository-path materialization for records that have **no IPC creation
//! path yet**, used only by `simulate-week` and `crash-drill`:
//!
//! | Record | Why there is no `dispatch` path | Same path as |
//! |---|---|---|
//! | `PLANNED` study sessions linked to the day's study blocks | `calendar.propose` with `FILL_CURRICULUM` is PROPOSED (`PRECONDITION_FAILED calendar.fill_curriculum.not_implemented`) | `g9_evaluation::fixtures::insert_planned_session` |
//! | one competency + the Saturday blind-practical definition, version and baseline rubric | catalogue import is PROPOSED | `g9_evaluation::fixtures::{seed_competencies, seed_assessments}` |
//!
//! Every write goes through the typed `g9-storage` repositories (no SQL here),
//! is reported as `SEEDED` in the step table (never as a pipeline outcome), and
//! is idempotent: existing rows are reused. Everything that *has* an IPC path
//! (session transitions, notes, attempts, grades, approvals, backups) goes
//! through [`g9_app::dispatch`].

use g9_assessment::{AssessmentDefinition, AssessmentKind, AssessmentVersion, Rubric};
use g9_competency::Competency;
use g9_curriculum::CivilDate;
use g9_domain::{
    ActorId, AssessmentId, Classification, CompetencyId, CurriculumId, DataDomain, Difficulty,
    DurationMs, EvidenceId, G9Error, IanaZone, InteractionMode, Revision, SessionId, SessionState,
    UtcMillis,
};
use g9_storage::repos::assessment::{self as assessment_repo, AssessmentDefinitionRow};
use g9_storage::repos::competency::{self as competency_repo, CompetencyRow};
use g9_storage::repos::curriculum::{
    self as curriculum_repo, CurriculumVersionRow, TrainingDayRow,
};
use g9_storage::repos::sessions::{self as sessions_repo, StudySessionRow};
use g9_storage::repos::ReadScope;
use g9_storage::Database;
use serde::Serialize;

/// Curriculum id of the program (`AppConfig::new` uses the same).
pub const CURRICULUM_ID: &str = "g9-ct9-480";
/// Id of the competency the simulated assessments evidence.
pub const SIM_COMPETENCY_ID: &str = "sim-core-competency";
/// Local start hour of a simulated training day.
pub const DAY_START_HOUR: u8 = 8;

/// What a materialization did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SeedReport {
    /// Rows inserted.
    pub created: usize,
    /// Rows that already existed and were reused.
    pub existing: usize,
    /// Ids involved (created or reused), in order.
    pub ids: Vec<String>,
}

fn internal(code: &str) -> G9Error {
    G9Error::internal_redacted(code)
}

/// `YYYY-MM-DD`.
pub fn format_date(d: CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// Parse `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Result<CivilDate, G9Error> {
    let bad = || G9Error::invalid_input("date.invalid", "dates are YYYY-MM-DD");
    let mut parts = s.trim().split('-');
    let year: i32 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let month: u8 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let day: u8 = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    if parts.next().is_some() {
        return Err(bad());
    }
    CivilDate::new(year, month, day).map_err(|_| bad())
}

/// Instant of `date` at `hour:00` local time in `zone`.
pub fn local_instant(date: CivilDate, hour: u8, zone: &IanaZone) -> Result<UtcMillis, G9Error> {
    let civil = g9_domain::CivilDateTime::new(date.year, date.month, date.day, hour, 0, 0)
        .map_err(|_| internal("cli.civil_time"))?;
    Ok(g9_calendar::resolve_local(&civil, zone)?.instant)
}

/// The curriculum version the backend reads (same rule as `g9-app`: the
/// current canonical version, else the newest import).
pub fn active_version(db: &Database) -> Result<Option<CurriculumVersionRow>, G9Error> {
    let conn = db.conn();
    let curriculum =
        CurriculumId::parse(CURRICULUM_ID).map_err(|_| internal("cli.curriculum_id"))?;
    if let Some(v) = curriculum_repo::current_version(conn, &curriculum)? {
        return Ok(Some(v));
    }
    let mut versions = curriculum_repo::list_versions(conn, &curriculum)?;
    versions.sort_by_key(|v| (v.imported_at, v.id.clone()));
    Ok(versions.pop())
}

/// Training day of `date` for a program starting on `program_start` (Monday of
/// week 1); `None` on Sundays and outside the program.
pub fn training_day(
    db: &Database,
    version: &CurriculumVersionRow,
    program_start: CivilDate,
    date: CivilDate,
) -> Result<Option<TrainingDayRow>, G9Error> {
    let conn = db.conn();
    let index = date.to_days() - program_start.to_days();
    let week = index.div_euclid(7) + 1;
    let weekday = index.rem_euclid(7) + 1;
    if weekday > 6 || week < 1 {
        return Ok(None);
    }
    let weeks = curriculum_repo::list_weeks(conn, &version.id)?;
    let Some(week_row) = weeks.iter().find(|w| i64::from(w.week_number) == week) else {
        return Ok(None);
    };
    Ok(curriculum_repo::list_training_days(conn, &version.id)?
        .into_iter()
        .find(|d| d.week_id == week_row.id && i64::from(d.day_of_week.get()) == weekday))
}

/// Deterministic session id for a block of a simulated week.
pub fn session_id_for(week: u32, day_code: &str, ordinal: u32) -> String {
    format!("sim-w{week:02}-{day_code}-b{ordinal}")
}

/// Materialize one `PLANNED` session per study block of `day` (blocks that
/// already have a session are left alone), in one transaction: a failure
/// part-way leaves none of the day's new sessions (storage-repos-10).
pub fn materialize_sessions(
    db: &Database,
    week: u32,
    day: &TrainingDayRow,
    date: CivilDate,
    zone: &IanaZone,
    now: UtcMillis,
) -> Result<SeedReport, G9Error> {
    db.write_transaction(|conn| {
        let scope = ReadScope::domain(DataDomain::Learning);
        let existing = sessions_repo::list_sessions_for_day(conn, &scope, &day.id)?;
        let blocks = curriculum_repo::study_blocks(conn, &day.id)?;
        let mut report = SeedReport::default();
        let mut start = local_instant(date, DAY_START_HOUR, zone)?;
        for block in &blocks {
            let minutes = DurationMs::from_minutes_u32(block.minutes);
            let end = start
                .checked_add(minutes)
                .map_err(|_| internal("cli.time_overflow"))?;
            if let Some(found) = existing
                .iter()
                .find(|s| s.study_block_id.as_deref() == Some(block.id.as_str()))
            {
                report.existing += 1;
                report.ids.push(found.id.as_str().to_string());
            } else {
                let id_text = session_id_for(week, day.day_code.as_str(), block.ordinal);
                let id = SessionId::parse(&id_text).map_err(|_| internal("cli.session_id"))?;
                let row = StudySessionRow {
                    id: id.clone(),
                    training_day_id: Some(day.id.clone()),
                    objective_id: block.objective_id.clone(),
                    study_block_id: Some(block.id.clone()),
                    calendar_event_id: None,
                    state: SessionState::Planned,
                    mode: block.mode,
                    zone: zone.clone(),
                    planned_start_utc: Some(start),
                    planned_end_utc: Some(end),
                    planned_minutes: block.minutes,
                    started_at: None,
                    ended_at: None,
                    credited_ms: DurationMs::ZERO,
                    uncertain_ms: DurationMs::ZERO,
                    data_domain: DataDomain::Learning,
                    classification: Classification::Internal,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                };
                sessions_repo::insert_session(conn, &row)?;
                report.created += 1;
                report.ids.push(id_text);
            }
            // One minute between blocks.
            start = end
                .checked_add(DurationMs::from_minutes_u32(1))
                .map_err(|_| internal("cli.time_overflow"))?;
        }
        Ok(report)
    })
}

/// Materialize one standalone `PLANNED` session (crash drill), 45 minutes.
pub fn materialize_drill_session(
    db: &Database,
    zone: &IanaZone,
    now: UtcMillis,
) -> Result<SessionId, G9Error> {
    let conn = db.conn();
    let id_text = format!("drill-{}", g9_domain::EventId::new_v7());
    let id = SessionId::parse(&id_text).map_err(|_| internal("cli.session_id"))?;
    let row = StudySessionRow {
        id: id.clone(),
        training_day_id: None,
        objective_id: None,
        study_block_id: None,
        calendar_event_id: None,
        state: SessionState::Planned,
        mode: InteractionMode::Study,
        zone: zone.clone(),
        planned_start_utc: Some(now),
        planned_end_utc: None,
        planned_minutes: 45,
        started_at: None,
        ended_at: None,
        credited_ms: DurationMs::ZERO,
        uncertain_ms: DurationMs::ZERO,
        data_domain: DataDomain::Learning,
        classification: Classification::Internal,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    sessions_repo::insert_session(conn, &row)?;
    Ok(id)
}

/// The shared competency the simulated assessments evidence (the row only).
fn sim_competency(now: UtcMillis) -> Result<(CompetencyId, CompetencyRow), G9Error> {
    let id = CompetencyId::parse(SIM_COMPETENCY_ID).map_err(|_| internal("cli.competency_id"))?;
    let easiest = Difficulty::new(1).map_err(|_| internal("cli.difficulty"))?;
    let hardest = Difficulty::new(5).map_err(|_| internal("cli.difficulty"))?;
    let competency = Competency {
        id: id.clone(),
        name: "Simulated core competency".to_string(),
        pack: "simulation".to_string(),
        domain: DataDomain::Learning,
        description: "Synthetic competency evidenced by the simulated-week assessments; \
                      not part of a knowledge pack."
            .to_string(),
        validity_window_days: 90,
        difficulty_band: (easiest, hardest),
        nice_refs: Vec::new(),
    };
    competency.validate()?;
    let row = CompetencyRow::from_domain(&competency, "SIM-CORE", None, now);
    Ok((id, row))
}

/// Assessment definition id of the simulated week.
pub fn assessment_id_for(week: u32) -> String {
    format!("sim-w{week:02}-blind-practical")
}

/// Materialize the Saturday blind practical of `week`: competency (shared),
/// definition linked to `day`, version 1 with the baseline rubric. One
/// transaction (storage-repos-10): a definition is never left without its
/// version and rubric, which a rerun (it reuses an existing definition) could
/// not repair.
pub fn materialize_assessment(
    db: &Database,
    week: u32,
    day: &TrainingDayRow,
    operator: &ActorId,
    now: UtcMillis,
) -> Result<SeedReport, G9Error> {
    let (competency_id, competency_row) = sim_competency(now)?;
    let id_text = assessment_id_for(week);
    let id = AssessmentId::parse(&id_text).map_err(|_| internal("cli.assessment_id"))?;
    let definition = AssessmentDefinition::new(
        id.clone(),
        format!("Week {week} blind practical (simulated)"),
        vec![competency_id.clone()],
        AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )?;
    let mut row =
        AssessmentDefinitionRow::from_domain(&definition, &format!("SIM-W{week:02}-BLIND"), now)
            .map_err(G9Error::from)?;
    row.training_day_id = Some(day.id.clone());

    // Difficulty 3 is a fixture value declared by this seed for the simulated
    // practical, not a measurement. It is persisted with the version
    // (`assessment_versions.difficulty`): `assessment.approve_final` derives
    // competency observations only from a version that declares a difficulty.
    let difficulty = Difficulty::new(3).map_err(|_| internal("cli.difficulty"))?;
    let task_ref =
        EvidenceId::parse(&format!("{id_text}-task")).map_err(|_| internal("cli.evidence_id"))?;
    let version = AssessmentVersion {
        definition_id: id.clone(),
        version: 1,
        rubric: Rubric::baseline(),
        task_ref,
        sealed_key_ref: None,
        difficulty,
        equivalence_group: format!("sim-w{week:02}"),
        created_at: now,
        checksum: String::new(),
    }
    .with_checksum()?;
    let version_id = format!("{id_text}:v1");
    let version_row = g9_storage::repos::assessment::AssessmentVersionRow::from_domain(
        &version,
        &version_id,
        "Simulated blind practical: complete the week's lab on the unseen holdout and document \
         the evidence (synthetic task statement; no sealed key).",
        None,
        operator.clone(),
    )
    .map_err(G9Error::from)?;
    let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
        &version.rubric,
        &format!("{id_text}:rubric"),
        &version_id,
        "Baseline rubric",
        "v1",
        now,
    );

    db.write_transaction(|conn| {
        let scope = ReadScope::domain(DataDomain::Learning);
        let mut report = SeedReport::default();
        if competency_repo::get_competency(conn, &scope, &competency_id)?.is_some() {
            report.existing += 1;
        } else {
            competency_repo::insert_competency(conn, &competency_row)?;
            report.created += 1;
        }
        report.ids.push(competency_id.as_str().to_string());
        if assessment_repo::get_definition(conn, &scope, &id)?.is_some() {
            report.existing += 1;
            report.ids.push(id_text.clone());
            return Ok(report);
        }
        assessment_repo::insert_definition(conn, &row)?;
        assessment_repo::insert_version(conn, &version_row)?;
        assessment_repo::insert_rubric(conn, &rubric, &criteria)?;
        report.created += 1;
        report.ids.push(id_text.clone());
        Ok(report)
    })
}

/// Criterion codes of the baseline rubric (for `assessment.grade_provisional`).
pub fn baseline_criteria() -> Vec<String> {
    Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| c.code.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_and_reject_garbage() {
        let d = parse_date("2026-11-16").unwrap();
        assert_eq!(format_date(d), "2026-11-16");
        assert_eq!(d.iso_weekday(), 1);
        assert!(parse_date("2026-13-01").is_err());
        assert!(parse_date("2026-11-16-1").is_err());
        assert!(parse_date("yesterday").is_err());
    }

    #[test]
    fn local_instant_resolves_in_zone() {
        let zone = IanaZone::parse("Europe/Berlin").unwrap();
        let d = parse_date("2026-11-16").unwrap();
        // 08:00 CET = 07:00Z.
        assert_eq!(
            local_instant(d, 8, &zone).unwrap(),
            UtcMillis::from_millis(1_794_812_400_000)
        );
        assert_eq!(
            local_instant(d, 8, &IanaZone::utc()).unwrap(),
            UtcMillis::from_millis(1_794_816_000_000)
        );
    }

    #[test]
    fn ids_are_deterministic_and_well_formed() {
        assert_eq!(session_id_for(1, "D01", 3), "sim-w01-D01-b3");
        assert_eq!(assessment_id_for(12), "sim-w12-blind-practical");
        assert!(SessionId::parse(&session_id_for(1, "D01", 3)).is_ok());
        assert_eq!(
            baseline_criteria().len(),
            Rubric::baseline().criteria().len()
        );
        assert!(baseline_criteria().contains(&"independent_execution".to_string()));
    }

    /// storage-repos-10: the definition, version and rubric of a simulated
    /// assessment are written in one transaction. When the rubric insert failed
    /// (here: its id is already taken), the definition and version stayed, a
    /// rerun found the definition and returned early, and the week never got a
    /// version or rubric (`assessment.no_version` at `assessment.begin`).
    #[test]
    fn a_failed_assessment_materialization_leaves_no_definition_or_version() {
        use crate::commands::import_seed;
        let dir = tempfile::tempdir().unwrap();
        let mut boot = crate::bootstrap::BootOptions::new(dir.path().join("data"));
        boot.skills_dir = None;
        boot.zone = Some(IanaZone::utc());
        boot.clock = Some(crate::clock::SimClock::new(UtcMillis::from_millis(
            1_794_812_400_000,
        )));
        let booted = crate::bootstrap::open(&boot).unwrap();
        let mut client = crate::client::Client::new(booted.ctx);
        let manifest =
            import_seed::load_manifest(&crate::bootstrap::default_seed_manifest()).unwrap();
        import_seed::import(&mut client, &manifest, &mut |_, _| {}).unwrap();
        let now = client.now();
        let db = client.ctx.database();
        let version = active_version(db).unwrap().unwrap();
        let saturday = curriculum_repo::list_training_days(db.conn(), &version.id)
            .unwrap()
            .into_iter()
            .find(|d| d.day_of_week.is_saturday())
            .unwrap();
        let operator = ActorId::parse("operator-1").unwrap();
        // Week 2 materializes; its version then holds a rubric under the id week 1 uses.
        materialize_assessment(db, 2, &saturday, &operator, now).unwrap();
        let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
            &Rubric::baseline(),
            &format!("{}:rubric", assessment_id_for(1)),
            &format!("{}:v1", assessment_id_for(2)),
            "Taken",
            "taken",
            now,
        );
        assessment_repo::insert_rubric(db.conn(), &rubric, &criteria).unwrap();

        let err = materialize_assessment(db, 1, &saturday, &operator, now).unwrap_err();
        assert_eq!(err.code, "storage.unique_violation", "{err:?}");
        let rows = |table: &str, id: String| -> i64 {
            db.conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"),
                    [id],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(rows("assessment_definitions", assessment_id_for(1)), 0);
        assert_eq!(
            rows(
                "assessment_versions",
                format!("{}:v1", assessment_id_for(1))
            ),
            0
        );
        assert_eq!(rows("assessment_definitions", assessment_id_for(2)), 1);
    }
}
