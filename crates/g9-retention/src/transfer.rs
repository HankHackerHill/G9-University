//! Practical transfer checks: re-verification of a *competency* on an unseen
//! task in a new context, scheduled by the competency's validity window.
//! Distinct from item reviews by type and by scheduling rule (gate B02).

use g9_domain::{CompetencyId, UtcMillis};
use serde::{Deserialize, Serialize};

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferStatus {
    Scheduled,
    Completed { passed: bool, at: UtcMillis },
    Skipped { reason: String, at: UtcMillis },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PracticalTransferCheck {
    /// Stable id (`transfer:<competency>:<due epoch ms>`).
    pub id: String,
    pub competency_id: CompetencyId,
    pub due_at: UtcMillis,
    /// What must differ from previously seen tasks (surface features, context, tooling).
    pub novelty_requirement: String,
    pub last_validated_at: Option<UtcMillis>,
    pub status: TransferStatus,
}

impl PracticalTransferCheck {
    pub fn is_due(&self, now: UtcMillis) -> bool {
        matches!(self.status, TransferStatus::Scheduled) && self.due_at <= now
    }

    pub fn complete(&mut self, passed: bool, at: UtcMillis) {
        self.status = TransferStatus::Completed { passed, at };
    }
}

/// Schedule the next transfer check at `last_validated_at + validity_window_days`
/// (or `now` when the competency was never validated).
pub fn schedule_transfer_check(
    competency_id: CompetencyId,
    validity_window_days: u32,
    last_validated_at: Option<UtcMillis>,
    novelty_requirement: impl Into<String>,
    now: UtcMillis,
) -> PracticalTransferCheck {
    let due_at = match last_validated_at {
        Some(v) => UtcMillis::from_millis(
            v.as_millis()
                .saturating_add(i64::from(validity_window_days) * DAY_MS),
        ),
        None => now,
    };
    PracticalTransferCheck {
        id: format!("transfer:{}:{}", competency_id, due_at.as_millis()),
        competency_id,
        due_at,
        novelty_requirement: novelty_requirement.into(),
        last_validated_at,
        status: TransferStatus::Scheduled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::{BaselineSchedule, ItemState, ReviewOutcome};
    use g9_domain::ReviewItemId;

    #[test]
    fn b02_transfer_checks_separate_from_item_reviews() {
        let now = UtcMillis::from_millis(100 * DAY_MS);
        let c = CompetencyId::parse("web.sqli").unwrap();
        let check = schedule_transfer_check(
            c.clone(),
            90,
            Some(UtcMillis::from_millis(50 * DAY_MS)),
            "new application, different DBMS",
            now,
        );
        assert_eq!(check.due_at.as_millis(), 140 * DAY_MS);
        assert!(!check.is_due(now));
        // An item review on the same competency advances on its own memory schedule and
        // has no effect on the transfer check: the two are different types with no link.
        let s = BaselineSchedule::default();
        let st = ItemState::new(
            ReviewItemId::parse("item-1").unwrap(),
            now,
            BaselineSchedule::NAME,
        );
        let st2 = s.next_after(&st, ReviewOutcome::Success, now).unwrap();
        assert_eq!(st2.due_at.as_millis(), 101 * DAY_MS);
        assert_eq!(
            check.due_at.as_millis(),
            140 * DAY_MS,
            "unchanged by item reviews"
        );
        let never = schedule_transfer_check(c, 90, None, "first unseen task", now);
        assert!(never.is_due(now));
    }
}
