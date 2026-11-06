//! Occurrence expansion, exhaustive pairwise conflict detection and hard
//! constraints (M03, ARCHITECTURE §7).
//!
//! `expand` resolves every event occurrence inside a range to instants;
//! `find_conflicts` reports every overlapping pair; `check_constraints`
//! evaluates event-level constraints (immutable appointments, protected rest
//! windows, daily capacity). Task-level constraints (assessment deadlines,
//! prerequisite readiness) are evaluated by [`crate::proposal::propose_schedule`].

use crate::civil::{add_civil_days, resolve_local, to_civil, Adjustment, CivilDate, DAY_MS};
use crate::error::CalendarError;
use crate::events::{CalendarEvent, EventKind};
use crate::recurrence::occurrences;
use g9_domain::{CalendarEventId, CivilDateTime, DurationMs, IanaZone, UtcMillis};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Reference to one occurrence of one event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OccurrenceRef {
    /// Event identifier.
    pub event_id: CalendarEventId,
    /// Occurrence sequence (0 for single events).
    pub occurrence_seq: u32,
}

/// An event occurrence resolved to instants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEvent {
    /// Event identifier.
    pub event_id: CalendarEventId,
    /// Occurrence sequence (0 for single events).
    pub occurrence_seq: u32,
    /// Start instant (inclusive).
    pub start: UtcMillis,
    /// End instant (exclusive).
    pub end: UtcMillis,
    /// Event kind.
    pub kind: EventKind,
    /// Immutable flag copied from the event.
    pub immutable: bool,
    /// Whether this is a date-only occurrence spanning local midnights.
    pub all_day: bool,
    /// DST adjustment applied to the start.
    pub adjustment: Adjustment,
}

impl ResolvedEvent {
    /// Hard constraints are never moved or overlapped by the scheduler.
    pub const fn is_hard_constraint(&self) -> bool {
        self.immutable || self.kind.is_rest()
    }

    /// Reference to this occurrence.
    pub fn occurrence_ref(&self) -> OccurrenceRef {
        OccurrenceRef {
            event_id: self.event_id.clone(),
            occurrence_seq: self.occurrence_seq,
        }
    }

    /// Span in milliseconds (zero if reversed, which `expand` never produces).
    pub fn span(&self) -> DurationMs {
        self.end
            .duration_since(self.start)
            .unwrap_or(DurationMs::ZERO)
    }

    /// Overlap with another resolved interval, if any (strictly positive).
    pub fn overlap_with(&self, other: &ResolvedEvent) -> Option<DurationMs> {
        overlap((self.start, self.end), (other.start, other.end))
    }
}

/// Strictly positive overlap between two half-open instant intervals.
pub fn overlap(a: (UtcMillis, UtcMillis), b: (UtcMillis, UtcMillis)) -> Option<DurationMs> {
    let start = a.0.max(b.0);
    let end = a.1.min(b.1);
    if end > start {
        end.duration_since(start)
    } else {
        None
    }
}

fn resolved_from(
    event: &CalendarEvent,
    seq: u32,
    civil: &CivilDateTime,
) -> Result<ResolvedEvent, CalendarError> {
    let start = resolve_local(civil, &event.zone)?;
    let end = match event.all_day_days() {
        Some(days) => {
            let next = add_civil_days(civil, days)?;
            resolve_local(&next, &event.zone)?.instant
        }
        None => start.instant.checked_add(event.duration)?,
    };
    Ok(ResolvedEvent {
        event_id: event.id.clone(),
        occurrence_seq: seq,
        start: start.instant,
        end,
        kind: event.kind,
        immutable: event.immutable,
        all_day: event.all_day,
        adjustment: start.adjustment,
    })
}

/// Resolve every occurrence of every event that intersects `[range.0, range.1)`.
/// Output is sorted by `(start, event_id, occurrence_seq)`.
pub fn expand(
    events: &[CalendarEvent],
    range: (UtcMillis, UtcMillis),
) -> Result<Vec<ResolvedEvent>, CalendarError> {
    if range.0 >= range.1 {
        return Err(CalendarError::InvalidRange);
    }
    let mut out = Vec::new();
    for event in events {
        event.validate()?;
        match &event.rule {
            None => {
                let r = resolved_from(event, 0, &event.start)?;
                if r.start < range.1 && r.end > range.0 {
                    out.push(r);
                }
            }
            Some(rule) => {
                // Widen the start of the range by the longest possible span so
                // that occurrences starting before the range but ending inside
                // it are not missed.
                let max_span = match event.all_day_days() {
                    Some(days) => {
                        DurationMs::new(days.saturating_mul(DAY_MS).saturating_add(DAY_MS))
                            .unwrap_or(DurationMs::ZERO)
                    }
                    None => event.duration,
                };
                let widened = range.0.checked_sub(max_span)?;
                for occ in occurrences(rule, &event.start, &event.zone, (widened, range.1))? {
                    let r = resolved_from(event, occ.seq, &occ.civil)?;
                    if r.start < range.1 && r.end > range.0 {
                        out.push(r);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (a.start, &a.event_id, a.occurrence_seq).cmp(&(b.start, &b.event_id, b.occurrence_seq))
    });
    Ok(out)
}

/// Two occurrences that overlap in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Earlier-starting occurrence.
    pub a: OccurrenceRef,
    /// Later-starting occurrence.
    pub b: OccurrenceRef,
    /// Length of the overlap.
    pub overlap_ms: DurationMs,
    /// Whether at least one side is a hard constraint (immutable or rest).
    pub hard: bool,
}

/// Exhaustive pairwise overlap detection. The input is sorted by start; each
/// occurrence is compared with every later-starting occurrence that begins
/// before it ends, which covers every overlapping pair exactly once.
pub fn find_conflicts(resolved: &[ResolvedEvent]) -> Vec<Conflict> {
    let mut sorted: Vec<&ResolvedEvent> = resolved.iter().collect();
    sorted.sort_by(|a, b| {
        (a.start, &a.event_id, a.occurrence_seq).cmp(&(b.start, &b.event_id, b.occurrence_seq))
    });
    let mut out = Vec::new();
    for (i, a) in sorted.iter().enumerate() {
        for b in sorted.iter().skip(i + 1) {
            if b.start >= a.end {
                break;
            }
            if let Some(overlap_ms) = a.overlap_with(b) {
                out.push(Conflict {
                    a: a.occurrence_ref(),
                    b: b.occurrence_ref(),
                    overlap_ms,
                    hard: a.is_hard_constraint() || b.is_hard_constraint(),
                });
            }
        }
    }
    out
}

/// A daily window in local minutes since midnight, `[start_minute, end_minute)`.
/// `end_minute <= start_minute` means the window wraps past midnight
/// (e.g. 22:00 → 07:00 is `{ start_minute: 1320, end_minute: 420 }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DailyWindow {
    /// Start, minutes since local midnight (0..=1439).
    pub start_minute: u16,
    /// End, minutes since local midnight (0..=1440); `1440` means midnight.
    pub end_minute: u16,
}

impl DailyWindow {
    /// Validate bounds.
    pub fn validate(&self) -> Result<(), CalendarError> {
        if self.start_minute >= 1440 || self.end_minute > 1440 {
            return Err(CalendarError::InvalidConstraint(
                "daily window minutes must be within 0..=1440".into(),
            ));
        }
        if self.start_minute == self.end_minute {
            return Err(CalendarError::InvalidConstraint(
                "daily window is empty".into(),
            ));
        }
        Ok(())
    }

    /// Whether the window wraps past local midnight.
    pub const fn wraps(&self) -> bool {
        self.end_minute <= self.start_minute
    }

    /// The window's instants on a given local date.
    pub fn instants_on(
        &self,
        date: CivilDate,
        zone: &IanaZone,
    ) -> Result<(UtcMillis, UtcMillis), CalendarError> {
        self.validate()?;
        let start_civil = date.at(
            (self.start_minute / 60) as u8,
            (self.start_minute % 60) as u8,
            0,
        )?;
        let (end_date, end_minute) = if self.wraps() {
            (date.add_days(1)?, self.end_minute)
        } else {
            (date, self.end_minute)
        };
        let end_civil = if end_minute == 1440 {
            end_date.add_days(1)?.midnight()
        } else {
            end_date.at((end_minute / 60) as u8, (end_minute % 60) as u8, 0)?
        };
        Ok((
            resolve_local(&start_civil, zone)?.instant,
            resolve_local(&end_civil, zone)?.instant,
        ))
    }
}

/// Scheduling constraints. Event-level ones are checked by [`check_constraints`];
/// task-level ones by [`crate::proposal::propose_schedule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Constraint {
    /// Immutable events must not be overlapped by anything movable. (Two hard
    /// constraints overlapping each other is still reported by
    /// [`find_conflicts`] with `hard = true`, but is not a scheduling violation.)
    ImmutableAppointment,
    /// No workload inside the daily rest window (local time in `zone`).
    ProtectedRest {
        /// Rest window.
        daily_window: DailyWindow,
        /// Zone the window is expressed in.
        zone: IanaZone,
    },
    /// Workload per local day (by start instant) must not exceed `minutes`.
    DailyCapacity {
        /// Capacity in minutes.
        minutes: u32,
        /// Zone that defines the local day.
        zone: IanaZone,
    },
    /// A task must end by `deadline`.
    AssessmentDeadline {
        /// Task identifier.
        task_id: String,
        /// Deadline instant.
        deadline: UtcMillis,
    },
    /// `task` may only start after `requires` has been placed.
    PrerequisiteReady {
        /// Dependent task.
        task: String,
        /// Prerequisite task.
        requires: String,
    },
}

impl Constraint {
    /// Kind label for violations and traces.
    pub const fn kind(&self) -> ConstraintKind {
        match self {
            Constraint::ImmutableAppointment => ConstraintKind::ImmutableAppointment,
            Constraint::ProtectedRest { .. } => ConstraintKind::ProtectedRest,
            Constraint::DailyCapacity { .. } => ConstraintKind::DailyCapacity,
            Constraint::AssessmentDeadline { .. } => ConstraintKind::AssessmentDeadline,
            Constraint::PrerequisiteReady { .. } => ConstraintKind::PrerequisiteReady,
        }
    }

    /// Validate structural fields.
    pub fn validate(&self) -> Result<(), CalendarError> {
        match self {
            Constraint::ProtectedRest { daily_window, .. } => daily_window.validate(),
            Constraint::DailyCapacity { minutes, .. } if *minutes == 0 => Err(
                CalendarError::InvalidConstraint("daily capacity must be >= 1 minute".into()),
            ),
            Constraint::AssessmentDeadline { task_id, .. } if !is_slug(task_id) => Err(
                CalendarError::InvalidConstraint("assessment deadline needs a slug task id".into()),
            ),
            Constraint::PrerequisiteReady { task, requires }
                if !is_slug(task) || !is_slug(requires) || task == requires =>
            {
                Err(CalendarError::InvalidConstraint(
                    "prerequisite edge needs two distinct slug task ids".into(),
                ))
            }
            _ => Ok(()),
        }
    }
}

/// Constraint category of a violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintKind {
    /// See [`Constraint::ImmutableAppointment`].
    ImmutableAppointment,
    /// See [`Constraint::ProtectedRest`].
    ProtectedRest,
    /// See [`Constraint::DailyCapacity`].
    DailyCapacity,
    /// See [`Constraint::AssessmentDeadline`] (or a task's own deadline).
    AssessmentDeadline,
    /// See [`Constraint::PrerequisiteReady`] (or a task's own prerequisites).
    PrerequisiteReady,
    /// No availability window can hold the task.
    Availability,
}

/// A violated constraint, always reported, never dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintViolation {
    /// Which constraint.
    pub constraint: ConstraintKind,
    /// Task id or `event_id#seq` the violation is about.
    pub subject: String,
    /// Deterministic, secret-free explanation.
    pub detail: String,
}

fn subject_of(r: &ResolvedEvent) -> String {
    format!("{}#{}", r.event_id, r.occurrence_seq)
}

/// Identifier rule for caller-supplied task ids (same alphabet as
/// `g9_domain` typed ids) so that ids can be echoed in errors and traces
/// without leaking private text.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// Evaluate event-level constraints over resolved occurrences.
pub fn check_constraints(
    resolved: &[ResolvedEvent],
    constraints: &[Constraint],
) -> Result<Vec<ConstraintViolation>, CalendarError> {
    let mut out = Vec::new();
    for c in constraints {
        c.validate()?;
        match c {
            Constraint::ImmutableAppointment => {
                for conflict in find_conflicts(resolved) {
                    let find =
                        |occ: &OccurrenceRef| resolved.iter().find(|r| r.occurrence_ref() == *occ);
                    let (Some(a), Some(b)) = (find(&conflict.a), find(&conflict.b)) else {
                        continue;
                    };
                    let movable_hits_immutable = (a.immutable && !b.is_hard_constraint())
                        || (b.immutable && !a.is_hard_constraint());
                    if movable_hits_immutable {
                        out.push(ConstraintViolation {
                            constraint: ConstraintKind::ImmutableAppointment,
                            subject: format!(
                                "{}#{} and {}#{}",
                                conflict.a.event_id,
                                conflict.a.occurrence_seq,
                                conflict.b.event_id,
                                conflict.b.occurrence_seq
                            ),
                            detail: format!(
                                "immutable appointment overlapped for {} minutes",
                                conflict.overlap_ms.as_whole_minutes()
                            ),
                        });
                    }
                }
            }
            Constraint::ProtectedRest { daily_window, zone } => {
                for r in resolved.iter().filter(|r| r.kind.is_workload()) {
                    if let Some(ov) = rest_overlap(r, daily_window, zone)? {
                        out.push(ConstraintViolation {
                            constraint: ConstraintKind::ProtectedRest,
                            subject: subject_of(r),
                            detail: format!(
                                "workload overlaps protected rest by {} minutes",
                                ov.as_whole_minutes()
                            ),
                        });
                    }
                }
            }
            Constraint::DailyCapacity { minutes, zone } => {
                let mut per_day: BTreeMap<CivilDate, i64> = BTreeMap::new();
                for r in resolved.iter().filter(|r| r.kind.is_workload()) {
                    let day = CivilDate::of(&to_civil(r.start, zone)?);
                    *per_day.entry(day).or_insert(0) += r.span().as_whole_minutes();
                }
                for (day, used) in per_day {
                    if used > i64::from(*minutes) {
                        out.push(ConstraintViolation {
                            constraint: ConstraintKind::DailyCapacity,
                            subject: day.to_string(),
                            detail: format!("{used} workload minutes exceed capacity {minutes}"),
                        });
                    }
                }
            }
            Constraint::AssessmentDeadline { .. } | Constraint::PrerequisiteReady { .. } => {
                // Task-level; evaluated by the proposal module.
            }
        }
    }
    Ok(out)
}

/// Overlap of a resolved occurrence with the rest window on any local day it
/// touches (the day before the start through the day of the end).
pub(crate) fn rest_overlap(
    r: &ResolvedEvent,
    window: &DailyWindow,
    zone: &IanaZone,
) -> Result<Option<DurationMs>, CalendarError> {
    rest_overlap_span((r.start, r.end), window, zone)
}

/// Overlap of an instant span with the rest window on any local day it touches.
pub(crate) fn rest_overlap_span(
    span: (UtcMillis, UtcMillis),
    window: &DailyWindow,
    zone: &IanaZone,
) -> Result<Option<DurationMs>, CalendarError> {
    let first = CivilDate::of(&to_civil(span.0, zone)?).add_days(-1)?;
    let last = CivilDate::of(&to_civil(span.1, zone)?);
    let mut day = first;
    let mut total: i64 = 0;
    while day <= last {
        let (ws, we) = window.instants_on(day, zone)?;
        if let Some(ov) = overlap(span, (ws, we)) {
            total = total.saturating_add(ov.as_millis());
        }
        day = day.add_days(1)?;
    }
    if total > 0 {
        Ok(Some(DurationMs::new(total)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::DataDomain;
    use pretty_assertions::assert_eq;

    fn ev(id: &str, kind: EventKind, start: CivilDateTime, minutes: u32) -> CalendarEvent {
        CalendarEvent {
            id: CalendarEventId::parse(id).unwrap(),
            title: id.into(),
            kind,
            immutable: false,
            start,
            duration: DurationMs::from_minutes_u32(minutes),
            zone: IanaZone::utc(),
            all_day: false,
            rule: None,
            data_domain: DataDomain::Learning,
        }
    }

    fn ms(h: i64) -> UtcMillis {
        UtcMillis::from_millis(h * 3_600_000)
    }

    #[test]
    fn expand_includes_occurrence_starting_before_range() {
        // 1970-01-01 23:00 UTC, 2 hours long; range starts at 1970-01-02 00:00.
        let e = ev(
            "late",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 23, 0, 0).unwrap(),
            120,
        );
        let r = expand(std::slice::from_ref(&e), (ms(24), ms(48))).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start, ms(23));
        assert_eq!(r[0].end, ms(25));
        // Recurring daily version: occurrence of day 1 (23:00) must also appear.
        let mut rec = e;
        rec.rule = Some(
            crate::recurrence::RecurrenceRule::new(crate::recurrence::Frequency::Daily, 1).unwrap(),
        );
        let r = expand(&[rec], (ms(24), ms(48))).unwrap();
        assert_eq!(
            r.iter().map(|x| x.start).collect::<Vec<_>>(),
            vec![ms(23), ms(47)]
        );
        assert_eq!(r[0].occurrence_seq, 0);
        assert_eq!(r[1].occurrence_seq, 1);
    }

    #[test]
    fn expand_rejects_reversed_range_and_invalid_events() {
        let e = ev(
            "x",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 1, 0, 0).unwrap(),
            60,
        );
        assert_eq!(
            expand(std::slice::from_ref(&e), (ms(2), ms(1))).unwrap_err(),
            CalendarError::InvalidRange
        );
        let mut bad = e;
        bad.all_day = true;
        assert!(matches!(
            expand(&[bad], (ms(0), ms(1))).unwrap_err(),
            CalendarError::InvalidEvent { .. }
        ));
    }

    #[test]
    fn find_conflicts_is_exhaustive_and_touching_is_not_overlap() {
        let a = ev(
            "a",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 1, 0, 0).unwrap(),
            120,
        );
        let b = ev(
            "b",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 2, 0, 0).unwrap(),
            120,
        );
        let c = ev(
            "c",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 3, 0, 0).unwrap(),
            60,
        );
        let d = ev(
            "d",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 1, 30, 0).unwrap(),
            180,
        );
        let r = expand(&[a, b, c, d], (ms(0), ms(10))).unwrap();
        let conflicts = find_conflicts(&r);
        let pairs: Vec<(String, String)> = conflicts
            .iter()
            .map(|c| (c.a.event_id.to_string(), c.b.event_id.to_string()))
            .collect();
        // a[1,3) b[2,4) c[3,4) d[1:30,4:30): a-d, a-b, d-b, d-c, b-c; a-c touch only.
        assert_eq!(
            pairs,
            vec![
                ("a".into(), "d".into()),
                ("a".into(), "b".into()),
                ("d".into(), "b".into()),
                ("d".into(), "c".into()),
                ("b".into(), "c".into()),
            ]
        );
        assert!(conflicts.iter().all(|c| !c.hard));
        assert_eq!(conflicts[1].overlap_ms, DurationMs::from_minutes_u32(60));
    }

    #[test]
    fn daily_window_instants_and_wrap() {
        let zone = IanaZone::utc();
        let w = DailyWindow {
            start_minute: 1320,
            end_minute: 420,
        };
        assert!(w.wraps());
        let (s, e) = w
            .instants_on(CivilDate::new(1970, 1, 1).unwrap(), &zone)
            .unwrap();
        assert_eq!(s, ms(22));
        assert_eq!(e, ms(31));
        let w = DailyWindow {
            start_minute: 0,
            end_minute: 1440,
        };
        let (s, e) = w
            .instants_on(CivilDate::new(1970, 1, 2).unwrap(), &zone)
            .unwrap();
        assert_eq!((s, e), (ms(24), ms(48)));
        assert!(DailyWindow {
            start_minute: 1440,
            end_minute: 10
        }
        .validate()
        .is_err());
        assert!(DailyWindow {
            start_minute: 10,
            end_minute: 10
        }
        .validate()
        .is_err());
    }

    #[test]
    fn daily_capacity_violation_reported_per_day() {
        let a = ev(
            "a",
            EventKind::StudyBlock,
            CivilDateTime::new(1970, 1, 1, 8, 0, 0).unwrap(),
            240,
        );
        let b = ev(
            "b",
            EventKind::Review,
            CivilDateTime::new(1970, 1, 1, 13, 0, 0).unwrap(),
            240,
        );
        let rest = ev(
            "r",
            EventKind::ProtectedRest,
            CivilDateTime::new(1970, 1, 1, 22, 0, 0).unwrap(),
            540,
        );
        let r = expand(&[a, b, rest], (ms(0), ms(48))).unwrap();
        let v = check_constraints(
            &r,
            &[Constraint::DailyCapacity {
                minutes: 420,
                zone: IanaZone::utc(),
            }],
        )
        .unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].constraint, ConstraintKind::DailyCapacity);
        assert_eq!(v[0].subject, "1970-01-01");
        assert!(check_constraints(
            &r,
            &[Constraint::DailyCapacity {
                minutes: 0,
                zone: IanaZone::utc()
            }]
        )
        .is_err());
    }
}
