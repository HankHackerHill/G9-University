//! Calendar events (M03). An event stores operator intent as civil time plus
//! an IANA zone; occurrences are resolved to instants by [`crate::conflicts::expand`].

use crate::civil::DAY_MS;
use crate::error::CalendarError;
use crate::recurrence::RecurrenceRule;
use g9_domain::{CalendarEventId, CivilDateTime, DataDomain, DurationMs, IanaZone};
use serde::{Deserialize, Serialize};

/// What an event represents. Drives conflict severity and schedule priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    /// External appointment (usually immutable).
    Appointment,
    /// Protected rest; always a hard constraint.
    ProtectedRest,
    /// Curriculum study block.
    StudyBlock,
    /// Assessment window or deadline.
    Assessment,
    /// Retention review slot.
    Review,
    /// Scheduled intervention.
    Intervention,
    /// Business commitment (M14).
    Commitment,
}

impl EventKind {
    /// Whether the event consumes learner workload (counts toward daily capacity
    /// and must not sit inside protected rest).
    pub const fn is_workload(self) -> bool {
        matches!(
            self,
            EventKind::StudyBlock
                | EventKind::Assessment
                | EventKind::Review
                | EventKind::Intervention
        )
    }

    /// Whether the event is rest.
    pub const fn is_rest(self) -> bool {
        matches!(self, EventKind::ProtectedRest)
    }
}

/// A calendar event (single or recurring).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    /// Identifier.
    pub id: CalendarEventId,
    /// Operator-facing title (private content; never placed in errors).
    pub title: String,
    /// Kind.
    pub kind: EventKind,
    /// Immutable events are hard constraints: the scheduler never moves or
    /// overlaps them.
    pub immutable: bool,
    /// Civil start (for all-day events the time part must be 00:00:00).
    pub start: CivilDateTime,
    /// Duration (for all-day events: a positive multiple of 24 h, counted in
    /// local days).
    pub duration: DurationMs,
    /// Zone the civil start is expressed in.
    pub zone: IanaZone,
    /// Date-only event spanning whole local days.
    pub all_day: bool,
    /// Recurrence rule, if the event repeats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<RecurrenceRule>,
    /// Privacy domain of the record.
    pub data_domain: DataDomain,
}

impl CalendarEvent {
    /// Check the event's invariants.
    pub fn validate(&self) -> Result<(), CalendarError> {
        let refuse = |reason: &str| CalendarError::InvalidEvent {
            event_id: self.id.to_string(),
            reason: reason.to_string(),
        };
        if self.start.to_naive().is_none() {
            return Err(refuse("start is not a valid civil date-time"));
        }
        if self.all_day {
            if self.start.hour != 0 || self.start.minute != 0 || self.start.second != 0 {
                return Err(refuse(
                    "all-day events are date-only: start time must be 00:00:00",
                ));
            }
            let ms = self.duration.as_millis();
            if ms < DAY_MS || ms % DAY_MS != 0 {
                return Err(refuse(
                    "all-day events need a duration that is a positive multiple of 24 h",
                ));
            }
        }
        if let Some(rule) = &self.rule {
            rule.validate()?;
        }
        Ok(())
    }

    /// Hard constraints are never moved or overlapped by the scheduler.
    pub const fn is_hard_constraint(&self) -> bool {
        self.immutable || self.kind.is_rest()
    }

    /// Number of local days spanned by an all-day event (`None` for timed events).
    pub fn all_day_days(&self) -> Option<i64> {
        self.all_day
            .then(|| (self.duration.as_millis() / DAY_MS).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recurrence::{Frequency, RecurrenceRule};

    fn base() -> CalendarEvent {
        CalendarEvent {
            id: CalendarEventId::parse("ev-1").unwrap(),
            title: "t".into(),
            kind: EventKind::StudyBlock,
            immutable: false,
            start: CivilDateTime::new(2026, 3, 2, 9, 0, 0).unwrap(),
            duration: DurationMs::from_minutes_u32(60),
            zone: IanaZone::parse("Europe/Berlin").unwrap(),
            all_day: false,
            rule: None,
            data_domain: DataDomain::Learning,
        }
    }

    #[test]
    fn validate_all_day_rules() {
        let mut e = base();
        e.all_day = true;
        assert!(e.validate().is_err(), "time part must be zero");
        e.start = CivilDateTime::new(2026, 3, 2, 0, 0, 0).unwrap();
        assert!(e.validate().is_err(), "duration must be whole days");
        e.duration = DurationMs::new(DAY_MS * 2).unwrap();
        assert!(e.validate().is_ok());
        assert_eq!(e.all_day_days(), Some(2));
        assert_eq!(base().all_day_days(), None);
    }

    #[test]
    fn validate_rule_and_hard_constraint() {
        let mut e = base();
        e.rule = Some(RecurrenceRule {
            freq: Frequency::Daily,
            interval: 0,
            by_day: vec![],
            until: None,
            count: None,
        });
        assert!(e.validate().is_err());
        let mut rest = base();
        rest.kind = EventKind::ProtectedRest;
        assert!(rest.is_hard_constraint());
        let mut imm = base();
        imm.immutable = true;
        assert!(imm.is_hard_constraint());
        assert!(!base().is_hard_constraint());
        let json = serde_json::to_value(&imm).unwrap();
        assert_eq!(json["kind"], "STUDY_BLOCK");
        assert_eq!(json["data_domain"], "LEARNING");
        let back: CalendarEvent = serde_json::from_value(json).unwrap();
        assert_eq!(back, imm);
    }

    #[test]
    fn error_does_not_echo_title() {
        let mut e = base();
        e.title = "private appointment with Dr X".into();
        e.all_day = true;
        let err = e.validate().unwrap_err();
        assert!(!err.to_string().contains("Dr X"));
    }
}
