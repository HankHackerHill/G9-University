//! Optional FSRS scheduler (cargo feature `fsrs`) wrapping `rs-fsrs` 1.2.1 (MIT).
//! Declarative items only; desired retention is a *parameter* evaluated per
//! `docs/evaluations/fsrs-evaluation-protocol.md`, not a mandate (C14, ADR-0006).

use crate::baseline::{ItemState, ReviewOutcome};
use crate::item::ReviewItem;
use crate::scheduler::ItemScheduler;
use crate::RetentionError;
use g9_domain::UtcMillis;
use rs_fsrs::{Card, Parameters, Rating, FSRS};
use serde::{Deserialize, Serialize};

/// FSRS scheduler with explicit, recorded parameters.
#[derive(Debug, Clone)]
pub struct FsrsScheduler {
    params: Parameters,
}

/// Serializable snapshot of the parameters that produced a state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FsrsParams {
    pub request_retention: f64,
    pub maximum_interval: i32,
}

impl FsrsScheduler {
    pub const NAME: &'static str = "rs-fsrs-1.2.1";

    /// `request_retention` in (0, 1); 0.9 is the evaluation candidate default.
    pub fn new(request_retention: f64, maximum_interval_days: i32) -> Result<Self, RetentionError> {
        if !(request_retention > 0.0 && request_retention < 1.0) || maximum_interval_days <= 0 {
            return Err(RetentionError::InvalidSchedule {
                reason: "request_retention must be in (0,1) and maximum_interval positive".into(),
            });
        }
        let params = Parameters {
            request_retention,
            maximum_interval: maximum_interval_days,
            enable_fuzz: false,
            ..Parameters::default()
        };
        Ok(Self { params })
    }

    pub fn params(&self) -> FsrsParams {
        FsrsParams {
            request_retention: self.params.request_retention,
            maximum_interval: self.params.maximum_interval,
        }
    }
}

fn to_datetime(t: UtcMillis) -> Result<chrono::DateTime<chrono::Utc>, RetentionError> {
    t.to_datetime()
        .ok_or_else(|| RetentionError::InvalidEngineState {
            scheduler: FsrsScheduler::NAME.into(),
            reason: "timestamp out of range".into(),
        })
}

impl ItemScheduler for FsrsScheduler {
    fn next(
        &self,
        item: &ReviewItem,
        state: &ItemState,
        outcome: ReviewOutcome,
        now: UtcMillis,
    ) -> Result<ItemState, RetentionError> {
        if !item.kind.is_declarative() {
            return Err(RetentionError::NotDeclarative {
                scheduler: Self::NAME.into(),
                item_kind: format!("{:?}", item.kind),
            });
        }
        if state.item_id != item.id {
            return Err(RetentionError::ItemMismatch {
                state_item: state.item_id.to_string(),
                item: item.id.to_string(),
            });
        }
        let now_dt = to_datetime(now)?;
        let card: Card = match &state.engine_state {
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
                RetentionError::InvalidEngineState {
                    scheduler: Self::NAME.into(),
                    reason: e.to_string(),
                }
            })?,
            None => Card {
                due: now_dt,
                last_review: now_dt,
                ..Card::default()
            },
        };
        let rating = match outcome {
            ReviewOutcome::Success => Rating::Good,
            ReviewOutcome::Fail => Rating::Again,
        };
        let info = FSRS::new(self.params.clone()).next(card, now_dt, rating);
        let due_at = UtcMillis::from_datetime(info.card.due);
        let engine_state =
            serde_json::to_value(&info.card).map_err(|e| RetentionError::InvalidEngineState {
                scheduler: Self::NAME.into(),
                reason: e.to_string(),
            })?;
        Ok(ItemState {
            item_id: state.item_id.clone(),
            stage_index: state.stage_index + usize::from(outcome == ReviewOutcome::Success),
            last_reviewed_at: Some(now),
            due_at,
            lapses: state.lapses + u32::from(outcome == ReviewOutcome::Fail),
            successes: state.successes + u32::from(outcome == ReviewOutcome::Success),
            scheduler: Self::NAME.to_string(),
            engine_state: Some(engine_state),
        })
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemKind;
    use g9_domain::{CompetencyId, DataDomain, EvidenceId, ReviewItemId};

    fn item(kind: ItemKind) -> ReviewItem {
        ReviewItem {
            id: ReviewItemId::parse("item-1").unwrap(),
            competency_id: CompetencyId::parse("c").unwrap(),
            kind,
            prompt_ref: EvidenceId::parse("p").unwrap(),
            answer_ref: EvidenceId::parse("a").unwrap(),
            created_at: UtcMillis::from_millis(0),
            data_domain: DataDomain::Learning,
        }
    }

    #[test]
    fn fsrs_feature_schedules_declarative_only() {
        let s = FsrsScheduler::new(0.9, 365).unwrap();
        let now = UtcMillis::from_millis(1_700_000_000_000);
        let st = ItemState::new(
            ReviewItemId::parse("item-1").unwrap(),
            now,
            FsrsScheduler::NAME,
        );
        let err = s
            .next(&item(ItemKind::Procedure), &st, ReviewOutcome::Success, now)
            .unwrap_err();
        assert!(matches!(err, RetentionError::NotDeclarative { .. }));
        let next = s
            .next(
                &item(ItemKind::DeclarativeFact),
                &st,
                ReviewOutcome::Success,
                now,
            )
            .unwrap();
        assert!(next.due_at > now);
        assert!(next.engine_state.is_some());
        assert_eq!(next.scheduler, FsrsScheduler::NAME);
        let again = s
            .next(
                &item(ItemKind::DeclarativeFact),
                &next,
                ReviewOutcome::Success,
                next.due_at,
            )
            .unwrap();
        assert!(again.due_at > next.due_at);
        assert!(FsrsScheduler::new(1.5, 10).is_err());
    }
}
