//! Scheduler abstraction and bounded daily review planning.

use crate::baseline::{BaselineSchedule, ItemState, ReviewOutcome};
use crate::item::ReviewItem;
use crate::RetentionError;
use g9_domain::{DurationMs, ReviewItemId, UtcMillis};
use serde::{Deserialize, Serialize};

/// A review scheduler. Implementations must be pure given `now`.
pub trait ItemScheduler: Send + Sync {
    /// Compute the next state for `item` after `outcome` at `now`.
    fn next(
        &self,
        item: &ReviewItem,
        state: &ItemState,
        outcome: ReviewOutcome,
        now: UtcMillis,
    ) -> Result<ItemState, RetentionError>;
    /// Stable scheduler name recorded on every state and review attempt.
    fn name(&self) -> &'static str;
}

impl ItemScheduler for BaselineSchedule {
    fn next(
        &self,
        item: &ReviewItem,
        state: &ItemState,
        outcome: ReviewOutcome,
        now: UtcMillis,
    ) -> Result<ItemState, RetentionError> {
        if state.item_id != item.id {
            return Err(RetentionError::ItemMismatch {
                state_item: state.item_id.to_string(),
                item: item.id.to_string(),
            });
        }
        self.next_after(state, outcome, now)
    }

    fn name(&self) -> &'static str {
        BaselineSchedule::NAME
    }
}

/// Result of planning a review session inside a time budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewPlan {
    /// Items to review now, most overdue first.
    pub scheduled: Vec<ReviewItemId>,
    /// Due items that did not fit the budget — reported, never dropped.
    pub deferred_overflow: Vec<ReviewItemId>,
    pub budget_used_ms: i64,
    pub budget_ms: i64,
}

/// Plan which due items fit in `budget` given `estimated_per_item`. Ordering:
/// most overdue first, then lower stage first (fragile items), then id.
pub fn plan_reviews(
    states: &[ItemState],
    budget: DurationMs,
    estimated_per_item: DurationMs,
    now: UtcMillis,
) -> Result<ReviewPlan, RetentionError> {
    if estimated_per_item.as_millis() <= 0 {
        return Err(RetentionError::InvalidBudget {
            reason: "estimated time per item must be positive".into(),
        });
    }
    let mut due: Vec<&ItemState> = states.iter().filter(|s| s.is_due(now)).collect();
    due.sort_by(|a, b| {
        b.overdue(now)
            .cmp(&a.overdue(now))
            .then_with(|| a.stage_index.cmp(&b.stage_index))
            .then_with(|| a.item_id.cmp(&b.item_id))
    });
    let mut used: i64 = 0;
    let mut scheduled = Vec::new();
    let mut deferred = Vec::new();
    for s in due {
        if used + estimated_per_item.as_millis() <= budget.as_millis() {
            used += estimated_per_item.as_millis();
            scheduled.push(s.item_id.clone());
        } else {
            deferred.push(s.item_id.clone());
        }
    }
    Ok(ReviewPlan {
        scheduled,
        deferred_overflow: deferred,
        budget_used_ms: used,
        budget_ms: budget.as_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;
    fn t(day: i64) -> UtcMillis {
        UtcMillis::from_millis(day * DAY_MS)
    }
    fn st(id: &str, due_day: i64, stage: usize) -> ItemState {
        let mut s = ItemState::new(
            ReviewItemId::parse(id).unwrap(),
            t(due_day),
            BaselineSchedule::NAME,
        );
        s.stage_index = stage;
        s
    }

    #[test]
    fn plan_reports_overflow_never_drops() {
        let states = vec![st("a", 1, 0), st("b", 2, 0), st("c", 3, 0), st("d", 9, 0)];
        let plan = plan_reviews(
            &states,
            DurationMs::from_minutes_u32(4),
            DurationMs::from_minutes_u32(2),
            t(5),
        )
        .unwrap();
        assert_eq!(plan.scheduled.len(), 2);
        assert_eq!(
            plan.deferred_overflow.len(),
            1,
            "c is due but deferred; d is not due"
        );
        assert_eq!(plan.scheduled.len() + plan.deferred_overflow.len(), 3);
        assert_eq!(plan.budget_used_ms, 4 * 60_000);
    }

    #[test]
    fn due_ordering_by_overdue() {
        let states = vec![
            st("late", 1, 3),
            st("later", 0, 4),
            st("fresh", 5, 0),
            st("tie", 1, 1),
        ];
        let plan = plan_reviews(
            &states,
            DurationMs::from_minutes_u32(60),
            DurationMs::from_minutes_u32(1),
            t(5),
        )
        .unwrap();
        let ids: Vec<&str> = plan.scheduled.iter().map(|i| i.as_str()).collect();
        assert_eq!(ids, vec!["later", "tie", "late", "fresh"]);
    }

    #[test]
    fn zero_estimate_rejected() {
        assert!(plan_reviews(&[], DurationMs::ZERO, DurationMs::ZERO, t(0)).is_err());
    }
}
