//! Read models over a validated manifest: the plan for one training day and the
//! calendar offsets of the whole program.
//!
//! Both functions validate the manifest first and refuse an invalid one
//! (`CurriculumError::Invalid`): a plan derived from a manifest that breaks the
//! structural contract would silently misreport minutes or objectives.
//!
//! Date arithmetic is chrono-free: offsets are whole days from a Monday start,
//! and civil dates are produced with the proleptic Gregorian day-count
//! algorithm (Howard Hinnant's `days_from_civil` / `civil_from_days`). Resolving
//! a civil date to an instant (zone, DST) is the calendar crate's job.

use crate::error::CurriculumError;
use crate::manifest::{Manifest, Resource};
use crate::validate::validate;
use g9_domain::{
    CivilDateTime, CompetencyId, CurriculumId, DayId, DayOfWeek, DurationMs, ManifestProvenance,
    ObjectiveId, TimeError,
};
use serde::{Deserialize, Serialize};

/// A civil calendar date (proleptic Gregorian), no time, no zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CivilDate {
    /// Proleptic Gregorian year.
    pub year: i32,
    /// Month 1..=12.
    pub month: u8,
    /// Day of month 1..=31.
    pub day: u8,
}

impl CivilDate {
    /// Validate and construct a date.
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CurriculumError> {
        let invalid = CurriculumError::InvalidDate { year, month, day };
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return Err(invalid);
        }
        Ok(Self { year, month, day })
    }

    /// Days since 1970-01-01 (negative before the epoch).
    pub fn to_days(self) -> i64 {
        days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        )
    }

    /// Date from days since 1970-01-01.
    pub fn from_days(days: i64) -> Result<Self, CurriculumError> {
        let (y, m, d) = civil_from_days(days);
        let year = i32::try_from(y).map_err(|_| CurriculumError::DateOverflow)?;
        let month = u8::try_from(m).map_err(|_| CurriculumError::DateOverflow)?;
        let day = u8::try_from(d).map_err(|_| CurriculumError::DateOverflow)?;
        Self::new(year, month, day)
    }

    /// Add whole days.
    pub fn add_days(self, days: u32) -> Result<Self, CurriculumError> {
        let total = self
            .to_days()
            .checked_add(i64::from(days))
            .ok_or(CurriculumError::DateOverflow)?;
        Self::from_days(total)
    }

    /// ISO weekday: 1 = Monday .. 7 = Sunday.
    pub fn iso_weekday(self) -> u8 {
        // 1970-01-01 was a Thursday (ISO 4).
        let w = (self.to_days() + 3).rem_euclid(7); // 0 = Monday
        u8::try_from(w + 1).unwrap_or(1)
    }

    /// Midnight of this date as a civil date-time (zone-less; see `g9-calendar`).
    pub fn to_civil_date_time(self) -> Result<CivilDateTime, TimeError> {
        CivilDateTime::new(self.year, self.month, self.day, 0, 0, 0)
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// One objective of a daily plan, with typed identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedObjective {
    /// Position within the day, 1..=3.
    pub seq: u32,
    /// Typed objective id, e.g. `D14-O2`.
    pub objective_id: ObjectiveId,
    /// Objective text as written in the manifest.
    pub text: String,
    /// Linked competency nodes (may be empty).
    pub competency_ids: Vec<CompetencyId>,
}

/// One study block of a daily plan with its offset from the start of the day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedBlock {
    /// Position within the day, 1-based.
    pub seq: u32,
    /// Block name from the canonical list.
    pub block: String,
    /// Planned length of the block.
    pub duration: DurationMs,
    /// Elapsed study time before this block starts.
    pub offset: DurationMs,
}

/// The resolved plan for one training day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyPlan {
    /// Typed curriculum id.
    pub curriculum_id: CurriculumId,
    /// Content version of the source manifest.
    pub manifest_version: String,
    /// Provenance of the source manifest; only `APPROVED_CANONICAL` plans may
    /// drive official records.
    pub provenance: ManifestProvenance,
    /// `provenance.is_authoritative()`, repeated so UI labels cannot miss it.
    pub authoritative: bool,
    /// 1-based week number.
    pub week_number: u32,
    /// Week theme.
    pub week_title: String,
    /// Typed day id, e.g. `D14`.
    pub day_id: DayId,
    /// Validated day of week.
    pub day_of_week: DayOfWeek,
    /// Planned study time for the day (420 or 300 minutes).
    pub planned: DurationMs,
    /// Saturday blind-practical day.
    pub is_assessment_day: bool,
    /// Deliverable for the day.
    pub evidence_artifact: String,
    /// The three primary objectives.
    pub objectives: Vec<PlannedObjective>,
    /// Study blocks in order with offsets.
    pub blocks: Vec<PlannedBlock>,
    /// Resource references attached to the day.
    pub resources: Vec<Resource>,
}

/// Resolve the plan for `week_number` / `day_of_week` from a valid manifest.
pub fn daily_plan(
    manifest: &Manifest,
    week_number: u32,
    day_of_week: u8,
) -> Result<DailyPlan, CurriculumError> {
    validate(manifest).into_result()?;
    let dow = DayOfWeek::new(day_of_week)
        .map_err(|_| CurriculumError::DayOfWeekOutOfRange(day_of_week))?;
    let (week, day) =
        manifest
            .find_day(week_number, day_of_week)
            .ok_or(CurriculumError::DayNotFound {
                week: week_number,
                day_of_week,
            })?;
    let path = format!("week {week_number} day_of_week {day_of_week}");
    let curriculum_id =
        CurriculumId::parse(&manifest.curriculum_id).map_err(|_| CurriculumError::InvalidId {
            path: "/curriculum_id".to_string(),
            kind: CurriculumId::KIND,
        })?;
    let day_id = DayId::parse(&day.day_id).map_err(|_| CurriculumError::InvalidId {
        path: path.clone(),
        kind: DayId::KIND,
    })?;
    let planned = minutes_to_duration(day.planned_minutes, &path)?;

    let mut objectives = Vec::with_capacity(day.objectives.len());
    for o in &day.objectives {
        let objective_id =
            ObjectiveId::parse(&o.objective_id).map_err(|_| CurriculumError::InvalidId {
                path: format!("{path} objective {}", o.seq),
                kind: ObjectiveId::KIND,
            })?;
        let mut competency_ids = Vec::with_capacity(o.competency_ids.len());
        for c in &o.competency_ids {
            competency_ids.push(CompetencyId::parse(c).map_err(|_| {
                CurriculumError::InvalidId {
                    path: format!("{path} objective {} competency_ids", o.seq),
                    kind: CompetencyId::KIND,
                }
            })?);
        }
        objectives.push(PlannedObjective {
            seq: o.seq,
            objective_id,
            text: o.text.clone(),
            competency_ids,
        });
    }

    let mut blocks = Vec::with_capacity(day.blocks.len());
    let mut offset = DurationMs::ZERO;
    for b in &day.blocks {
        let duration = minutes_to_duration(b.minutes, &path)?;
        blocks.push(PlannedBlock {
            seq: b.seq,
            block: b.block.clone(),
            duration,
            offset,
        });
        offset = offset
            .checked_add(duration)
            .map_err(|_| CurriculumError::DateOverflow)?;
    }

    Ok(DailyPlan {
        curriculum_id,
        manifest_version: manifest.version.clone(),
        provenance: manifest.provenance,
        authoritative: manifest.is_authoritative(),
        week_number: week.week_number,
        week_title: week.title.clone(),
        day_id,
        day_of_week: dow,
        planned,
        is_assessment_day: day.is_assessment_day,
        evidence_artifact: day.evidence_artifact.clone(),
        objectives,
        blocks,
        resources: day.resources.clone(),
    })
}

fn minutes_to_duration(minutes: i64, path: &str) -> Result<DurationMs, CurriculumError> {
    minutes
        .checked_mul(60_000)
        .and_then(|ms| DurationMs::new(ms).ok())
        .ok_or_else(|| CurriculumError::InvalidDuration {
            path: path.to_string(),
        })
}

/// One day of the program timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineDay {
    /// Typed day id.
    pub day_id: DayId,
    /// 1-based week number.
    pub week_number: u32,
    /// Validated day of week.
    pub day_of_week: DayOfWeek,
    /// Whole days after the program start (Monday of week 1 = 0).
    pub offset_days: u32,
    /// Calendar date of the day (`start + offset_days`).
    pub date: CivilDate,
    /// Planned study time for the day.
    pub planned: DurationMs,
    /// Saturday blind-practical day.
    pub is_assessment_day: bool,
}

/// Calendar offsets for every training day of the program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramTimeline {
    /// Typed curriculum id.
    pub curriculum_id: CurriculumId,
    /// Provenance of the source manifest.
    pub provenance: ManifestProvenance,
    /// Monday of week 1.
    pub start: CivilDate,
    /// Last training day (Saturday of the final week).
    pub end: CivilDate,
    /// Calendar days spanned, start and end inclusive.
    pub span_days: u32,
    /// Every training day in program order.
    pub days: Vec<TimelineDay>,
}

/// Compute per-day calendar offsets from a Monday start date. Week `w`, day of
/// week `d` lands on offset `7·(w−1) + (d−1)`; Sundays are never used.
pub fn program_timeline(
    manifest: &Manifest,
    start: CivilDate,
) -> Result<ProgramTimeline, CurriculumError> {
    validate(manifest).into_result()?;
    let weekday = start.iso_weekday();
    if weekday != 1 {
        return Err(CurriculumError::StartNotMonday { weekday });
    }
    let curriculum_id =
        CurriculumId::parse(&manifest.curriculum_id).map_err(|_| CurriculumError::InvalidId {
            path: "/curriculum_id".to_string(),
            kind: CurriculumId::KIND,
        })?;
    let mut days = Vec::new();
    for (wi, week) in manifest.weeks.iter().enumerate() {
        let week_index = u32::try_from(wi).map_err(|_| CurriculumError::DateOverflow)?;
        for day in &week.days {
            let dow = DayOfWeek::new(day.day_of_week)
                .map_err(|_| CurriculumError::DayOfWeekOutOfRange(day.day_of_week))?;
            let offset_days = week_index
                .checked_mul(7)
                .and_then(|w| w.checked_add(u32::from(dow.get()) - 1))
                .ok_or(CurriculumError::DateOverflow)?;
            let path = format!("week {} day_of_week {}", week.week_number, day.day_of_week);
            days.push(TimelineDay {
                day_id: DayId::parse(&day.day_id).map_err(|_| CurriculumError::InvalidId {
                    path: path.clone(),
                    kind: DayId::KIND,
                })?,
                week_number: week.week_number,
                day_of_week: dow,
                offset_days,
                date: start.add_days(offset_days)?,
                planned: minutes_to_duration(day.planned_minutes, &path)?,
                is_assessment_day: day.is_assessment_day,
            });
        }
    }
    let (end, span_days) = match days.last() {
        Some(last) => (last.date, last.offset_days.saturating_add(1)),
        None => (start, 0),
    };
    Ok(ProgramTimeline {
        curriculum_id,
        provenance: manifest.provenance,
        start,
        end,
        span_days,
        days,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date_round_trips_and_weekdays() {
        let epoch = CivilDate::new(1970, 1, 1).unwrap();
        assert_eq!(epoch.to_days(), 0);
        assert_eq!(epoch.iso_weekday(), 4); // Thursday
        let d = CivilDate::new(2026, 11, 6).unwrap();
        assert_eq!(d.iso_weekday(), 5); // Friday
        assert_eq!(CivilDate::from_days(d.to_days()).unwrap(), d);
        assert_eq!(CivilDate::new(2026, 1, 5).unwrap().iso_weekday(), 1); // Monday
        assert_eq!(CivilDate::new(1969, 12, 31).unwrap().to_days(), -1);
        assert_eq!(CivilDate::new(1969, 12, 31).unwrap().iso_weekday(), 3);
        assert_eq!(CivilDate::new(2000, 3, 1).unwrap().to_days(), 11_017);
    }

    #[test]
    fn add_days_crosses_leap_day_and_year_end() {
        let d = CivilDate::new(2028, 2, 28).unwrap().add_days(1).unwrap();
        assert_eq!(d, CivilDate::new(2028, 2, 29).unwrap());
        let d = CivilDate::new(2027, 2, 28).unwrap().add_days(1).unwrap();
        assert_eq!(d, CivilDate::new(2027, 3, 1).unwrap());
        let d = CivilDate::new(2026, 12, 31).unwrap().add_days(1).unwrap();
        assert_eq!(d, CivilDate::new(2027, 1, 1).unwrap());
        assert!(CivilDate::new(2027, 2, 29).is_err());
        assert!(CivilDate::new(2026, 13, 1).is_err());
        assert!(CivilDate::new(2026, 4, 31).is_err());
        assert!(CivilDate::new(1900, 2, 29).is_err());
        assert!(CivilDate::new(2000, 2, 29).is_ok());
    }

    #[test]
    fn minutes_conversion_rejects_overflow_and_negatives() {
        assert_eq!(
            minutes_to_duration(420, "p").unwrap(),
            DurationMs::from_minutes_u32(420)
        );
        assert!(minutes_to_duration(-1, "p").is_err());
        assert!(minutes_to_duration(i64::MAX, "p").is_err());
    }
}
