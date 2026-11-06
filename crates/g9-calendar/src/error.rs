//! Crate error type and its mapping to the closed `G9Error` category set.
//!
//! Messages never carry private content: they reference identifiers, indices
//! and instants only.

use crate::session::SessionError;
use crate::timer::TimerError;
use g9_domain::{G9Error, TimeError};

/// Errors produced by the calendar, recurrence, conflict and proposal modules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalendarError {
    /// The civil date-time cannot be represented (e.g. 30 February).
    #[error("invalid civil date-time: {0}")]
    InvalidCivil(String),
    /// An instant or duration left the representable range.
    #[error("instant or duration is outside the representable range")]
    OutOfRange,
    /// No valid local time was found within the scan horizon after a DST gap.
    #[error("no valid local time within {0} hours after a DST gap")]
    UnresolvableGap(u32),
    /// A recurrence rule is structurally invalid.
    #[error("invalid recurrence rule: {0}")]
    InvalidRule(String),
    /// An RRULE string is outside the supported subset or malformed.
    #[error("RRULE parse error: {0}")]
    RruleParse(String),
    /// A calendar event fails its invariants.
    #[error("invalid calendar event `{event_id}`: {reason}")]
    InvalidEvent {
        /// Event identifier.
        event_id: String,
        /// Why the event was refused.
        reason: String,
    },
    /// A `(start, end)` range is empty or reversed.
    #[error("invalid range: start must precede end")]
    InvalidRange,
    /// An availability window is empty or reversed.
    #[error("invalid availability window at index {0}: start must precede end")]
    InvalidWindow(usize),
    /// A constraint is structurally invalid (e.g. a daily window past 24:00).
    #[error("invalid constraint: {0}")]
    InvalidConstraint(String),
    /// A schedule task fails its invariants.
    #[error("invalid schedule task `{task_id}`: {reason}")]
    InvalidTask {
        /// Task identifier.
        task_id: String,
        /// Why the task was refused.
        reason: String,
    },
    /// Recurrence expansion scanned more candidates than the safety limit.
    #[error("recurrence expansion exceeded {0} candidates")]
    OccurrenceLimit(u32),
    /// A domain time primitive refused a value.
    #[error(transparent)]
    Time(#[from] TimeError),
    /// Timer ledger error.
    #[error(transparent)]
    Timer(#[from] TimerError),
    /// Session record error.
    #[error(transparent)]
    Session(#[from] SessionError),
}

impl CalendarError {
    /// Stable machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            CalendarError::InvalidCivil(_) => "calendar.civil_invalid",
            CalendarError::OutOfRange => "calendar.out_of_range",
            CalendarError::UnresolvableGap(_) => "calendar.gap_unresolvable",
            CalendarError::InvalidRule(_) => "calendar.rule_invalid",
            CalendarError::RruleParse(_) => "calendar.rrule_parse",
            CalendarError::InvalidEvent { .. } => "calendar.event_invalid",
            CalendarError::InvalidRange => "calendar.range_invalid",
            CalendarError::InvalidWindow(_) => "calendar.window_invalid",
            CalendarError::InvalidConstraint(_) => "calendar.constraint_invalid",
            CalendarError::InvalidTask { .. } => "calendar.task_invalid",
            CalendarError::OccurrenceLimit(_) => "calendar.occurrence_limit",
            CalendarError::Time(_) => "calendar.time_invalid",
            CalendarError::Timer(e) => e.code(),
            CalendarError::Session(e) => e.code(),
        }
    }
}

impl From<CalendarError> for G9Error {
    fn from(e: CalendarError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            CalendarError::Timer(inner) => inner.into(),
            CalendarError::Session(inner) => inner.into(),
            CalendarError::InvalidCivil(_)
            | CalendarError::OutOfRange
            | CalendarError::InvalidRule(_)
            | CalendarError::RruleParse(_)
            | CalendarError::InvalidEvent { .. }
            | CalendarError::InvalidRange
            | CalendarError::InvalidWindow(_)
            | CalendarError::InvalidConstraint(_)
            | CalendarError::InvalidTask { .. }
            | CalendarError::Time(_) => G9Error::invalid_input(code, message),
            CalendarError::UnresolvableGap(_) | CalendarError::OccurrenceLimit(_) => {
                G9Error::precondition_failed(code, message)
            }
        }
    }
}

impl From<TimerError> for G9Error {
    fn from(e: TimerError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            TimerError::AlreadyRunning { .. } | TimerError::DuplicateCorrectionId => {
                G9Error::conflict(code, message)
            }
            TimerError::NoOpenInterval { .. }
            | TimerError::IntervalOpen(_)
            | TimerError::EpochMismatch { .. } => G9Error::precondition_failed(code, message),
            TimerError::MonotonicRegression { .. }
            | TimerError::EmptyReason
            | TimerError::Overflow => G9Error::invalid_input(code, message),
            TimerError::IntervalNotFound(_) => G9Error::not_found(code, message),
            TimerError::ActorNotOperator => G9Error::unauthorized(code, message),
        }
    }
}

impl From<SessionError> for G9Error {
    fn from(e: SessionError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            SessionError::Transition(t) => G9Error::precondition_failed(code, message)
                .with_details(serde_json::json!({ "from": t.from, "event": t.event })),
            SessionError::ActorNotPermitted { .. } => G9Error::unauthorized(code, message),
            SessionError::UncorrectedUncertainTime { .. }
            | SessionError::RecoverRequiresLedgerCheck => {
                G9Error::precondition_failed(code, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{ErrorCategory, SessionId, TransitionError};

    #[test]
    fn calendar_errors_map_to_closed_categories() {
        let cases: Vec<(CalendarError, ErrorCategory)> = vec![
            (
                CalendarError::InvalidCivil("x".into()),
                ErrorCategory::InvalidInput,
            ),
            (CalendarError::OutOfRange, ErrorCategory::InvalidInput),
            (
                CalendarError::UnresolvableGap(48),
                ErrorCategory::PreconditionFailed,
            ),
            (
                CalendarError::InvalidRule("x".into()),
                ErrorCategory::InvalidInput,
            ),
            (
                CalendarError::RruleParse("x".into()),
                ErrorCategory::InvalidInput,
            ),
            (CalendarError::InvalidRange, ErrorCategory::InvalidInput),
            (CalendarError::InvalidWindow(0), ErrorCategory::InvalidInput),
            (
                CalendarError::OccurrenceLimit(1),
                ErrorCategory::PreconditionFailed,
            ),
            (
                CalendarError::Time(TimeError::UnknownZone),
                ErrorCategory::InvalidInput,
            ),
            (
                CalendarError::Timer(TimerError::ActorNotOperator),
                ErrorCategory::Unauthorized,
            ),
            (
                CalendarError::Timer(TimerError::AlreadyRunning {
                    session_id: SessionId::parse("s1").unwrap(),
                }),
                ErrorCategory::Conflict,
            ),
            (
                CalendarError::Timer(TimerError::IntervalNotFound(3)),
                ErrorCategory::NotFound,
            ),
            (
                CalendarError::Timer(TimerError::EpochMismatch {
                    interval_epoch: 0,
                    current_epoch: 1,
                }),
                ErrorCategory::PreconditionFailed,
            ),
            (
                CalendarError::InvalidTask {
                    task_id: "t".into(),
                    reason: "r".into(),
                },
                ErrorCategory::InvalidInput,
            ),
            (
                CalendarError::Session(SessionError::Transition(TransitionError {
                    machine: "session",
                    from: "CLOSED".into(),
                    event: "START".into(),
                })),
                ErrorCategory::PreconditionFailed,
            ),
            (
                CalendarError::Session(SessionError::ActorNotPermitted {
                    event: "START".into(),
                    actor_kind: "MODEL".into(),
                }),
                ErrorCategory::Unauthorized,
            ),
        ];
        for (err, cat) in cases {
            let g: G9Error = err.clone().into();
            assert_eq!(g.category, cat, "{err:?}");
            assert!(
                g.code.starts_with("calendar.")
                    || g.code.starts_with("timer.")
                    || g.code.starts_with("session."),
                "{}",
                g.code
            );
            let json = serde_json::to_value(&g).unwrap();
            assert!(json.get("backtrace").is_none());
        }
    }
}
