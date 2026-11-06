//! Session timer ledger (M04, ARCHITECTURE §7 "Timer ledger", gate A03).
//!
//! Elapsed time is measured from a monotonic clock supplied by the caller
//! (`mono_ms`), never from wall-clock subtraction; wall instants are recorded
//! for display and correlation only. The ledger is append-only: intervals are
//! opened and closed, never edited; corrections are separate rows that
//! reference an interval and supersede its credit without overwriting it.
//!
//! Continuity: an interval closed normally is `VERIFIED`. An interval found
//! open when the process (re)starts is closed `UNCERTAIN` (suspend, crash or
//! reboot happened) and is **never credited** until the operator records a
//! [`ManualCorrection`]; the session is moved to `Interrupted` by the caller.
//!
//! Monotonic readings are only comparable within one process lifetime, so the
//! ledger tracks a `process_epoch` that `on_process_restart` increments.

use g9_domain::{Actor, DurationMs, SessionId, UtcMillis};
use serde::{Deserialize, Serialize};

/// Whether the elapsed time of an interval is trustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Continuity {
    /// Opened and closed within one process lifetime with monotonic readings.
    Verified,
    /// Closed at process restart; the elapsed time is unknown.
    Uncertain,
}

/// One focused-time interval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalRecord {
    /// Session the interval belongs to.
    pub session_id: SessionId,
    /// Wall-clock start (display only).
    pub start_wall: UtcMillis,
    /// Wall-clock end (display only); `None` while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_wall: Option<UtcMillis>,
    /// Monotonic reading at start.
    pub start_mono_ms: u64,
    /// Monotonic reading at end; `None` while open or when closed `Uncertain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_mono_ms: Option<u64>,
    /// Continuity class.
    pub continuity: Continuity,
    /// Process lifetime the monotonic readings belong to.
    #[serde(default)]
    pub process_epoch: u64,
}

impl IntervalRecord {
    /// Whether the interval is still open.
    pub const fn is_open(&self) -> bool {
        self.end_wall.is_none()
    }

    /// Elapsed time from monotonic readings (only for closed `Verified` intervals).
    pub fn mono_elapsed(&self) -> Option<DurationMs> {
        let end = self.end_mono_ms?;
        let delta = end.checked_sub(self.start_mono_ms)?;
        DurationMs::new(i64::try_from(delta).ok()?).ok()
    }

    /// Wall-clock estimate of the span (display only; zero if the clock went backwards).
    pub fn wall_estimate(&self) -> DurationMs {
        self.end_wall
            .and_then(|e| e.duration_since(self.start_wall))
            .unwrap_or(DurationMs::ZERO)
    }
}

/// An operator correction that supersedes the credit of one interval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCorrection {
    /// Correction identifier (unique in the ledger).
    pub id: String,
    /// Index of the corrected interval in [`Ledger::intervals`].
    pub interval_index: usize,
    /// Time the operator credits for the interval (may be zero).
    pub credited_ms: DurationMs,
    /// Operator-stated reason (non-empty).
    pub reason: String,
    /// Who recorded it (must be the operator).
    pub actor: Actor,
    /// When it was recorded.
    pub recorded_at: UtcMillis,
}

/// Credit summary for one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    /// Monotonic elapsed time of `Verified` intervals without a correction.
    pub verified_ms: DurationMs,
    /// Wall-clock estimate of `Uncertain` intervals without a correction.
    /// Never credited; shown so the operator can decide a correction.
    pub uncertain_ms: DurationMs,
    /// Sum of the latest correction per corrected interval.
    pub corrected_ms: DurationMs,
    /// `verified_ms + corrected_ms`.
    pub total_credited_ms: DurationMs,
}

/// Timer ledger errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimerError {
    /// An interval is already open (for this or another session).
    #[error("an interval is already open for session {session_id}")]
    AlreadyRunning {
        /// Session owning the open interval.
        session_id: SessionId,
    },
    /// Stop without a matching open interval.
    #[error("no open interval for session {session_id}")]
    NoOpenInterval {
        /// Session that was asked to stop.
        session_id: SessionId,
    },
    /// A monotonic reading is earlier than a previous one in the same process.
    #[error("monotonic clock regressed: {reading_ms} ms is earlier than {reference_ms} ms")]
    MonotonicRegression {
        /// The offending reading.
        reading_ms: u64,
        /// The earlier reference.
        reference_ms: u64,
    },
    /// Interval index out of range.
    #[error("interval index {0} does not exist")]
    IntervalNotFound(usize),
    /// Open intervals cannot be corrected.
    #[error("interval index {0} is still open and cannot be corrected")]
    IntervalOpen(usize),
    /// Corrections are operator-only.
    #[error("only the operator may record a manual time correction")]
    ActorNotOperator,
    /// Reason missing.
    #[error("a manual correction requires a non-empty reason")]
    EmptyReason,
    /// Correction id reused.
    #[error("duplicate correction id")]
    DuplicateCorrectionId,
    /// Arithmetic overflow.
    #[error("elapsed time overflow")]
    Overflow,
    /// The open interval belongs to an earlier process lifetime; it must be
    /// closed by `on_process_restart`, never as `Verified`.
    #[error(
        "interval was opened in process epoch {interval_epoch}, current epoch is {current_epoch}"
    )]
    EpochMismatch {
        /// Epoch the interval was opened in.
        interval_epoch: u64,
        /// Current ledger epoch.
        current_epoch: u64,
    },
}

impl TimerError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            TimerError::AlreadyRunning { .. } => "timer.already_running",
            TimerError::NoOpenInterval { .. } => "timer.no_open_interval",
            TimerError::MonotonicRegression { .. } => "timer.monotonic_regression",
            TimerError::IntervalNotFound(_) => "timer.interval_not_found",
            TimerError::IntervalOpen(_) => "timer.interval_open",
            TimerError::ActorNotOperator => "timer.actor_not_operator",
            TimerError::EmptyReason => "timer.empty_reason",
            TimerError::DuplicateCorrectionId => "timer.duplicate_correction_id",
            TimerError::Overflow => "timer.overflow",
            TimerError::EpochMismatch { .. } => "timer.epoch_mismatch",
        }
    }
}

/// Append-only interval ledger with corrections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// Intervals in creation order.
    pub intervals: Vec<IntervalRecord>,
    /// Corrections in creation order.
    pub corrections: Vec<ManualCorrection>,
    /// Current process lifetime; incremented by [`Ledger::on_process_restart`].
    #[serde(default)]
    pub process_epoch: u64,
}

impl Ledger {
    /// Empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// The open interval, if any.
    pub fn open_interval(&self) -> Option<(usize, &IntervalRecord)> {
        self.intervals.iter().enumerate().find(|(_, i)| i.is_open())
    }

    /// Intervals of one session with their indices.
    pub fn intervals_for<'a>(
        &'a self,
        session: &'a SessionId,
    ) -> impl Iterator<Item = (usize, &'a IntervalRecord)> + 'a {
        self.intervals
            .iter()
            .enumerate()
            .filter(move |(_, i)| &i.session_id == session)
    }

    /// Latest correction for an interval index.
    pub fn latest_correction(&self, interval_index: usize) -> Option<&ManualCorrection> {
        self.corrections
            .iter()
            .rev()
            .find(|c| c.interval_index == interval_index)
    }

    /// Whether the session has `Uncertain` intervals without a correction.
    pub fn has_uncorrected_uncertain(&self, session: &SessionId) -> bool {
        self.intervals_for(session).any(|(idx, i)| {
            i.continuity == Continuity::Uncertain
                && !i.is_open()
                && self.latest_correction(idx).is_none()
        })
    }

    fn last_mono_in_epoch(&self) -> Option<u64> {
        self.intervals
            .iter()
            .filter(|i| i.process_epoch == self.process_epoch)
            .filter_map(|i| i.end_mono_ms.or(Some(i.start_mono_ms)))
            .max()
    }

    /// Open an interval. Refused while any interval is open (one focused
    /// interval at a time; overlapping credit is impossible by construction)
    /// or when the monotonic reading regresses within the current process.
    pub fn start(
        &mut self,
        session: SessionId,
        wall: UtcMillis,
        mono_ms: u64,
    ) -> Result<usize, TimerError> {
        if let Some((_, open)) = self.open_interval() {
            return Err(TimerError::AlreadyRunning {
                session_id: open.session_id.clone(),
            });
        }
        if let Some(reference) = self.last_mono_in_epoch() {
            if mono_ms < reference {
                return Err(TimerError::MonotonicRegression {
                    reading_ms: mono_ms,
                    reference_ms: reference,
                });
            }
        }
        self.intervals.push(IntervalRecord {
            session_id: session,
            start_wall: wall,
            end_wall: None,
            start_mono_ms: mono_ms,
            end_mono_ms: None,
            continuity: Continuity::Verified,
            process_epoch: self.process_epoch,
        });
        Ok(self.intervals.len() - 1)
    }

    /// Close the session's open interval as `Verified`.
    pub fn stop(
        &mut self,
        session: &SessionId,
        wall: UtcMillis,
        mono_ms: u64,
    ) -> Result<usize, TimerError> {
        let idx = self
            .intervals
            .iter()
            .position(|i| i.is_open() && &i.session_id == session)
            .ok_or_else(|| TimerError::NoOpenInterval {
                session_id: session.clone(),
            })?;
        let interval_epoch = self.intervals[idx].process_epoch;
        if interval_epoch != self.process_epoch {
            return Err(TimerError::EpochMismatch {
                interval_epoch,
                current_epoch: self.process_epoch,
            });
        }
        let start_mono = self.intervals[idx].start_mono_ms;
        if mono_ms < start_mono {
            return Err(TimerError::MonotonicRegression {
                reading_ms: mono_ms,
                reference_ms: start_mono,
            });
        }
        if i64::try_from(mono_ms - start_mono).is_err() {
            return Err(TimerError::Overflow);
        }
        let record = &mut self.intervals[idx];
        record.end_wall = Some(wall);
        record.end_mono_ms = Some(mono_ms);
        record.continuity = Continuity::Verified;
        Ok(idx)
    }

    /// Called once at process start: closes every open interval as
    /// `Uncertain` (no monotonic end), starts a new process epoch and returns
    /// the affected session ids. The caller must transition those sessions
    /// with `SessionEvent::Interrupt`.
    pub fn on_process_restart(&mut self, now_wall: UtcMillis) -> Vec<SessionId> {
        let mut affected = Vec::new();
        for record in self.intervals.iter_mut().filter(|i| i.is_open()) {
            record.end_wall = Some(now_wall);
            record.end_mono_ms = None;
            record.continuity = Continuity::Uncertain;
            affected.push(record.session_id.clone());
        }
        self.process_epoch = self.process_epoch.saturating_add(1);
        affected
    }

    /// Record an operator correction for a closed interval.
    pub fn correct(&mut self, correction: ManualCorrection) -> Result<usize, TimerError> {
        if !correction.actor.is_operator() {
            return Err(TimerError::ActorNotOperator);
        }
        if correction.reason.trim().is_empty() {
            return Err(TimerError::EmptyReason);
        }
        if self.corrections.iter().any(|c| c.id == correction.id) {
            return Err(TimerError::DuplicateCorrectionId);
        }
        let record = self
            .intervals
            .get(correction.interval_index)
            .ok_or(TimerError::IntervalNotFound(correction.interval_index))?;
        if record.is_open() {
            return Err(TimerError::IntervalOpen(correction.interval_index));
        }
        self.corrections.push(correction);
        Ok(self.corrections.len() - 1)
    }

    /// Credit summary for a session. Uncertain time is never credited without
    /// a correction; open intervals are not credited until closed.
    pub fn credited(&self, session: &SessionId) -> Credit {
        let mut verified: i64 = 0;
        let mut uncertain: i64 = 0;
        let mut corrected: i64 = 0;
        for (idx, record) in self.intervals_for(session) {
            if record.is_open() {
                continue;
            }
            match self.latest_correction(idx) {
                Some(c) => corrected = corrected.saturating_add(c.credited_ms.as_millis()),
                None => match record.continuity {
                    Continuity::Verified => {
                        verified = verified.saturating_add(
                            record
                                .mono_elapsed()
                                .unwrap_or(DurationMs::ZERO)
                                .as_millis(),
                        );
                    }
                    Continuity::Uncertain => {
                        uncertain = uncertain.saturating_add(record.wall_estimate().as_millis());
                    }
                },
            }
        }
        let d = |v: i64| DurationMs::new(v.max(0)).unwrap_or(DurationMs::ZERO);
        Credit {
            verified_ms: d(verified),
            uncertain_ms: d(uncertain),
            corrected_ms: d(corrected),
            total_credited_ms: d(verified.saturating_add(corrected)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ActorId;
    use pretty_assertions::assert_eq;

    fn sid(s: &str) -> SessionId {
        SessionId::parse(s).unwrap()
    }

    fn wall(ms: i64) -> UtcMillis {
        UtcMillis::from_millis(ms)
    }

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("op").unwrap())
    }

    #[test]
    fn start_stop_credits_mono_delta() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1_000), 10_000).unwrap();
        l.stop(&s, wall(61_000), 70_000).unwrap();
        let c = l.credited(&s);
        assert_eq!(c.verified_ms, DurationMs::new(60_000).unwrap());
        assert_eq!(c.total_credited_ms, DurationMs::new(60_000).unwrap());
        assert_eq!(c.uncertain_ms, DurationMs::ZERO);
    }

    #[test]
    fn stop_without_open_and_duplicate_start() {
        let mut l = Ledger::new();
        let s = sid("s1");
        assert_eq!(
            l.stop(&s, wall(1), 1).unwrap_err(),
            TimerError::NoOpenInterval {
                session_id: s.clone()
            }
        );
        l.start(s.clone(), wall(1), 1).unwrap();
        assert_eq!(
            l.start(s.clone(), wall(2), 2).unwrap_err(),
            TimerError::AlreadyRunning {
                session_id: s.clone()
            }
        );
        assert_eq!(
            l.start(sid("s2"), wall(2), 2).unwrap_err(),
            TimerError::AlreadyRunning {
                session_id: s.clone()
            }
        );
        assert_eq!(l.intervals.len(), 1);
    }

    #[test]
    fn monotonic_regression_rejected_within_epoch_but_not_across_restart() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1), 5_000).unwrap();
        assert!(matches!(
            l.stop(&s, wall(2), 4_000).unwrap_err(),
            TimerError::MonotonicRegression { .. }
        ));
        l.stop(&s, wall(2), 6_000).unwrap();
        assert!(matches!(
            l.start(s.clone(), wall(3), 5_500).unwrap_err(),
            TimerError::MonotonicRegression { .. }
        ));
        l.on_process_restart(wall(10));
        // After a restart the monotonic clock legitimately starts over.
        l.start(s.clone(), wall(11), 100).unwrap();
        l.stop(&s, wall(12), 200).unwrap();
        assert_eq!(l.credited(&s).verified_ms, DurationMs::new(1_100).unwrap());
    }

    #[test]
    fn corrections_require_operator_reason_and_closed_interval() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1), 1).unwrap();
        let base = ManualCorrection {
            id: "c1".into(),
            interval_index: 0,
            credited_ms: DurationMs::new(1).unwrap(),
            reason: "x".into(),
            actor: operator(),
            recorded_at: wall(5),
        };
        assert_eq!(
            l.correct(base.clone()).unwrap_err(),
            TimerError::IntervalOpen(0)
        );
        l.stop(&s, wall(2), 2).unwrap();
        let model = ManualCorrection {
            actor: Actor::model(ActorId::parse("m").unwrap()),
            ..base.clone()
        };
        assert_eq!(l.correct(model).unwrap_err(), TimerError::ActorNotOperator);
        let skill = ManualCorrection {
            actor: Actor::skill(
                ActorId::parse("sk").unwrap(),
                g9_domain::SkillId::parse("g9-x").unwrap(),
            ),
            ..base.clone()
        };
        assert_eq!(l.correct(skill).unwrap_err(), TimerError::ActorNotOperator);
        let empty = ManualCorrection {
            reason: "  ".into(),
            ..base.clone()
        };
        assert_eq!(l.correct(empty).unwrap_err(), TimerError::EmptyReason);
        let missing = ManualCorrection {
            interval_index: 7,
            ..base.clone()
        };
        assert_eq!(
            l.correct(missing).unwrap_err(),
            TimerError::IntervalNotFound(7)
        );
        l.correct(base.clone()).unwrap();
        assert_eq!(
            l.correct(base).unwrap_err(),
            TimerError::DuplicateCorrectionId
        );
        // The original interval row is untouched; the correction supersedes its credit.
        assert_eq!(l.intervals[0].end_mono_ms, Some(2));
        let c = l.credited(&s);
        assert_eq!(c.verified_ms, DurationMs::ZERO);
        assert_eq!(c.corrected_ms, DurationMs::new(1).unwrap());
        assert_eq!(c.total_credited_ms, DurationMs::new(1).unwrap());
    }

    #[test]
    fn latest_correction_wins() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1), 1).unwrap();
        l.stop(&s, wall(2), 2).unwrap();
        for (id, ms) in [("c1", 100), ("c2", 50)] {
            l.correct(ManualCorrection {
                id: id.into(),
                interval_index: 0,
                credited_ms: DurationMs::new(ms).unwrap(),
                reason: "r".into(),
                actor: operator(),
                recorded_at: wall(3),
            })
            .unwrap();
        }
        assert_eq!(
            l.credited(&s).total_credited_ms,
            DurationMs::new(50).unwrap()
        );
        assert_eq!(l.corrections.len(), 2);
    }

    #[test]
    fn interval_from_previous_epoch_cannot_be_closed_verified() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1), 1).unwrap();
        // Simulate a persisted ledger whose epoch moved on without closing the interval.
        l.process_epoch += 1;
        assert_eq!(
            l.stop(&s, wall(2), 2).unwrap_err(),
            TimerError::EpochMismatch {
                interval_epoch: 0,
                current_epoch: 1
            }
        );
        assert_eq!(l.on_process_restart(wall(3)), vec![s.clone()]);
        assert_eq!(l.credited(&s).total_credited_ms, DurationMs::ZERO);
    }

    #[test]
    fn ledger_serializes_roundtrip() {
        let mut l = Ledger::new();
        let s = sid("s1");
        l.start(s.clone(), wall(1), 1).unwrap();
        l.on_process_restart(wall(9));
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"UNCERTAIN\""));
        let back: Ledger = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.process_epoch, 1);
    }
}
