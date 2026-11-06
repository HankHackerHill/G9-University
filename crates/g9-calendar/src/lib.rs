#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-calendar
//!
//! Civil time, recurrence, conflict checking, schedule proposals (M03) and the
//! session timer ledger plus session record (M04). Pure: no I/O, no clock
//! reads, no randomness; every instant and monotonic reading is supplied by
//! the caller. Depends only on `g9-domain`, `chrono` and `chrono-tz`.
//!
//! | Module | Contract |
//! |---|---|
//! | [`civil`] | `resolve_local` (DST gap → shift forward, overlap → earlier offset), `to_civil`, `add_civil_days` |
//! | [`recurrence`] | `RecurrenceRule` (FREQ=DAILY/WEEKLY/MONTHLY, INTERVAL, BYDAY, UNTIL/COUNT), `parse_rrule`/`to_rrule`, `occurrences` |
//! | [`events`] | `CalendarEvent` with civil start + IANA zone, all-day flag, optional rule |
//! | [`conflicts`] | `expand`, exhaustive pairwise `find_conflicts`, `Constraint`, `check_constraints` |
//! | [`proposal`] | deterministic greedy `propose_schedule`; `Infeasible` lists every violation |
//! | [`timer`] | append-only `Ledger`; monotonic elapsed; `UNCERTAIN` on restart; corrections supersede |
//! | [`session`] | `SessionRecord` over `SessionState`; operator-driven; recovery gated on corrections |
//!
//! Gates: A02 (recurrence/DST/conflicts/protected rest) and A03 (timer
//! discontinuity) are covered by `tests/gates.rs` (`a02_*`, `a03_*`).
//!
//! Nothing here is authoritative on its own: proposals and corrections are
//! applied through `g9-app` commands after `g9-policy` evaluation.

pub mod civil;
pub mod conflicts;
pub mod error;
pub mod events;
pub mod proposal;
pub mod recurrence;
pub mod session;
pub mod timer;

pub use civil::{
    add_civil_days, civil_weekday, resolve_local, to_civil, Adjustment, CivilDate, Resolved,
    Weekday,
};
pub use conflicts::{
    check_constraints, expand, find_conflicts, Conflict, Constraint, ConstraintKind,
    ConstraintViolation, DailyWindow, OccurrenceRef, ResolvedEvent,
};
pub use error::CalendarError;
pub use events::{CalendarEvent, EventKind};
pub use proposal::{
    propose_schedule, Feasibility, Placement, PriorityClass, ScheduleProposal, ScheduleTask,
    Unplaced, UnplacedReason,
};
pub use recurrence::{occurrences, parse_rrule, to_rrule, Frequency, Occurrence, RecurrenceRule};
pub use session::{SessionError, SessionEventRow, SessionRecord};
pub use timer::{Continuity, Credit, IntervalRecord, Ledger, ManualCorrection, TimerError};
