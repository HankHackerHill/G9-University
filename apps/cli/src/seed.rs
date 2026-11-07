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
/// already have a session are left alone).
pub fn materialize_sessions(
    db: &Database,
    week: u32,
    day: &TrainingDayRow,
    date: CivilDate,
    zone: &IanaZone,
    now: UtcMillis,
) -> Result<SeedReport, G9Error> {
    let conn = db.conn();
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

fn ensure_competency(db: &Database, now: UtcMillis) -> Result<(CompetencyId, bool), G9Error> {
    let conn = db.conn();
    let id = CompetencyId::parse(SIM_COMPETENCY_ID).map_err(|_| internal("cli.competency_id"))?;
    let scope = ReadScope::domain(DataDomain::Learning);
    if competency_repo::get_competency(conn, &scope, &id)?.is_some() {
        return Ok((id, false));
    }
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
    competency_repo::insert_competency(
        conn,
        &CompetencyRow::from_domain(&competency, "SIM-CORE", None, now),
    )?;
    Ok((id, true))
}

/// Assessment definition id of the simulated week.
pub fn assessment_id_for(week: u32) -> String {
    format!("sim-w{week:02}-blind-practical")
}

/// Materialize the Saturday blind practical of `week`: competency (shared),
/// definition linked to `day`, version 1 with the baseline rubric.
pub fn materialize_assessment(
    db: &Database,
    week: u32,
    day: &TrainingDayRow,
    operator: &ActorId,
    now: UtcMillis,
) -> Result<SeedReport, G9Error> {
    let conn = db.conn();
    let mut report = SeedReport::default();
    let (competency_id, created) = ensure_competency(db, now)?;
    if created {
        report.created += 1;
    } else {
        report.existing += 1;
    }
    report.ids.push(competency_id.as_str().to_string());

    let id_text = assessment_id_for(week);
    let id = AssessmentId::parse(&id_text).map_err(|_| internal("cli.assessment_id"))?;
    let scope = ReadScope::domain(DataDomain::Learning);
    if assessment_repo::get_definition(conn, &scope, &id)?.is_some() {
        report.existing += 1;
        report.ids.push(id_text);
        return Ok(report);
    }
    let definition = AssessmentDefinition::new(
        id.clone(),
        format!("Week {week} blind practical (simulated)"),
        vec![competency_id],
        AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )?;
    let mut row =
        AssessmentDefinitionRow::from_domain(&definition, &format!("SIM-W{week:02}-BLIND"), now)
            .map_err(G9Error::from)?;
    row.training_day_id = Some(day.id.clone());
    assessment_repo::insert_definition(conn, &row)?;

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
    assessment_repo::insert_version(conn, &version_row)?;
    let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
        &version.rubric,
        &format!("{id_text}:rubric"),
        &version_id,
        "Baseline rubric",
        "v1",
        now,
    );
    assessment_repo::insert_rubric(conn, &rubric, &criteria)?;
    report.created += 1;
    report.ids.push(id_text);
    Ok(report)
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
}
