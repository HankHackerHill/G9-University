//! Fixed baseline review schedule (default): offsets in days after the last
//! successful recall; a failure resets to the first offset.

use crate::RetentionError;
use g9_domain::{DurationMs, ReviewItemId, UtcMillis};
use serde::{Deserialize, Serialize};

const DAY_MS: i64 = 86_400_000;

/// Outcome of one review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewOutcome {
    Success,
    Fail,
}

/// Per-item scheduling state, scheduler-agnostic. `engine_state` carries an
/// optional scheduler-specific blob (e.g. an FSRS card) as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemState {
    pub item_id: ReviewItemId,
    /// Index into the baseline offsets (stage reached).
    pub stage_index: usize,
    pub last_reviewed_at: Option<UtcMillis>,
    pub due_at: UtcMillis,
    pub lapses: u32,
    pub successes: u32,
    /// Name of the scheduler that produced this state.
    pub scheduler: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_state: Option<serde_json::Value>,
}

impl ItemState {
    /// Initial state: due immediately.
    pub fn new(item_id: ReviewItemId, due_at: UtcMillis, scheduler: &str) -> Self {
        Self {
            item_id,
            stage_index: 0,
            last_reviewed_at: None,
            due_at,
            lapses: 0,
            successes: 0,
            scheduler: scheduler.to_string(),
            engine_state: None,
        }
    }

    /// How overdue the item is at `now` (zero when not yet due).
    pub fn overdue(&self, now: UtcMillis) -> DurationMs {
        now.duration_since(self.due_at).unwrap_or(DurationMs::ZERO)
    }

    pub fn is_due(&self, now: UtcMillis) -> bool {
        self.due_at <= now
    }
}

/// The configurable fixed schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineSchedule {
    /// Days after a successful review until the next one, per stage.
    pub offsets_days: Vec<u32>,
    /// Stage index to return to after a failure.
    pub fail_reset_index: usize,
}

impl Default for BaselineSchedule {
    fn default() -> Self {
        Self {
            offsets_days: vec![1, 3, 7, 21, 60],
            fail_reset_index: 0,
        }
    }
}

impl BaselineSchedule {
    pub const NAME: &'static str = "baseline-fixed-v1";

    pub fn validate(&self) -> Result<(), RetentionError> {
        if self.offsets_days.is_empty() {
            return Err(RetentionError::InvalidSchedule {
                reason: "offsets must not be empty".into(),
            });
        }
        if self.offsets_days.contains(&0) {
            return Err(RetentionError::InvalidSchedule {
                reason: "offsets must be positive".into(),
            });
        }
        if !self.offsets_days.windows(2).all(|w| w[0] < w[1]) {
            return Err(RetentionError::InvalidSchedule {
                reason: "offsets must be strictly increasing".into(),
            });
        }
        if self.fail_reset_index >= self.offsets_days.len() {
            return Err(RetentionError::InvalidSchedule {
                reason: "fail_reset_index out of range".into(),
            });
        }
        Ok(())
    }

    /// Next state after an outcome at `now`. Success advances one stage (capped
    /// at the last offset); failure resets to `fail_reset_index` and counts a lapse.
    pub fn next_after(
        &self,
        state: &ItemState,
        outcome: ReviewOutcome,
        now: UtcMillis,
    ) -> Result<ItemState, RetentionError> {
        self.validate()?;
        let last = self.offsets_days.len() - 1;
        // Success: the interval is the offset of the stage just completed, then the
        // stage advances (capped). Failure: back to the reset stage and its offset.
        let (stage, offset_days, lapses, successes) = match outcome {
            ReviewOutcome::Success => {
                let current = state.stage_index.min(last);
                (
                    (current + 1).min(last),
                    self.offsets_days[current],
                    state.lapses,
                    state.successes + 1,
                )
            }
            ReviewOutcome::Fail => (
                self.fail_reset_index,
                self.offsets_days[self.fail_reset_index],
                state.lapses + 1,
                state.successes,
            ),
        };
        let due_at = UtcMillis::from_millis(
            now.as_millis()
                .saturating_add(i64::from(offset_days) * DAY_MS),
        );
        Ok(ItemState {
            item_id: state.item_id.clone(),
            stage_index: stage,
            last_reviewed_at: Some(now),
            due_at,
            lapses,
            successes,
            scheduler: Self::NAME.to_string(),
            engine_state: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(day: i64) -> UtcMillis {
        UtcMillis::from_millis(day * DAY_MS)
    }

    #[test]
    fn baseline_intervals_monotone_and_reset_on_fail() {
        let s = BaselineSchedule::default();
        let id = ReviewItemId::parse("item-1").unwrap();
        let mut st = ItemState::new(id, t(0), BaselineSchedule::NAME);
        let mut prev_gap = 0i64;
        let mut now = t(0);
        for _ in 0..6 {
            st = s.next_after(&st, ReviewOutcome::Success, now).unwrap();
            let gap = st.due_at.as_millis() - now.as_millis();
            assert!(gap >= prev_gap, "intervals must be monotone");
            prev_gap = gap;
            now = st.due_at;
        }
        assert_eq!(st.stage_index, 4, "capped at the last stage");
        assert_eq!(prev_gap, 60 * DAY_MS, "steady state uses the last offset");
        let failed = s.next_after(&st, ReviewOutcome::Fail, now).unwrap();
        assert_eq!(failed.stage_index, 0);
        assert_eq!(failed.lapses, 1);
        assert_eq!(failed.due_at.as_millis() - now.as_millis(), DAY_MS);
    }

    #[test]
    fn invalid_schedules_rejected() {
        assert!(BaselineSchedule {
            offsets_days: vec![],
            fail_reset_index: 0
        }
        .validate()
        .is_err());
        assert!(BaselineSchedule {
            offsets_days: vec![3, 1],
            fail_reset_index: 0
        }
        .validate()
        .is_err());
        assert!(BaselineSchedule {
            offsets_days: vec![1, 2],
            fail_reset_index: 5
        }
        .validate()
        .is_err());
        assert!(BaselineSchedule {
            offsets_days: vec![0, 1],
            fail_reset_index: 0
        }
        .validate()
        .is_err());
    }
}
