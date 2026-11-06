#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-retention (M08)
//!
//! Spaced retrieval for *declarative items*, practical transfer checks for
//! *competencies*, and confidence calibration. Contract: `ARCHITECTURE.md` §9,
//! handoff v6.0 §10 "Retrieval and retention", corrections C14 and §23 item 46,
//! ADR-0006, gate B02.
//!
//! Rules enforced here:
//! * The fixed [`BaselineSchedule`] is the default scheduler. FSRS is an
//!   optional cargo feature (`fsrs`) wrapping `rs-fsrs`, usable for
//!   [`ItemKind::DeclarativeFact`] items only.
//! * [`plan_reviews`] never drops work: items that do not fit the daily budget
//!   are returned as `deferred_overflow`.
//! * Practical transfer checks are scheduled by competency validity windows,
//!   never by memory curves, and live in a separate type ([`transfer`]).
//! * Calibration uses stated probabilities. A 0–5 self-rating is *not* a
//!   probability and has no implicit conversion here.
//! * No I/O, no clocks: callers pass `now`.

pub mod baseline;
pub mod calibration;
pub mod item;
pub mod scheduler;
pub mod transfer;

#[cfg(feature = "fsrs")]
pub mod fsrs;

pub use baseline::{BaselineSchedule, ItemState, ReviewOutcome};
pub use calibration::{brier_score, reliability_bins, CalibrationObservation, ReliabilityBin};
pub use item::{ItemKind, ReviewItem};
pub use scheduler::{plan_reviews, ItemScheduler, ReviewPlan};
pub use transfer::{schedule_transfer_check, PracticalTransferCheck, TransferStatus};

use g9_domain::{ErrorCategory, G9Error};
use serde::{Deserialize, Serialize};

/// Errors raised by this crate; each maps to one closed `ErrorCategory`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetentionError {
    /// Schedule offsets must be non-empty and strictly increasing.
    #[error("invalid schedule: {reason}")]
    InvalidSchedule { reason: String },
    /// The scheduler does not accept this item kind.
    #[error("scheduler {scheduler} accepts DECLARATIVE_FACT items only; item kind is {item_kind}")]
    NotDeclarative {
        scheduler: String,
        item_kind: String,
    },
    /// A state belongs to a different item.
    #[error("state item {state_item} does not match item {item}")]
    ItemMismatch { state_item: String, item: String },
    /// Persisted engine state could not be decoded.
    #[error("engine state for {scheduler} is invalid: {reason}")]
    InvalidEngineState { scheduler: String, reason: String },
    /// Budget or estimate is zero/invalid.
    #[error("invalid budget: {reason}")]
    InvalidBudget { reason: String },
}

impl RetentionError {
    pub fn category(&self) -> ErrorCategory {
        match self {
            RetentionError::InvalidSchedule { .. }
            | RetentionError::ItemMismatch { .. }
            | RetentionError::InvalidBudget { .. } => ErrorCategory::InvalidInput,
            RetentionError::NotDeclarative { .. } => ErrorCategory::PreconditionFailed,
            RetentionError::InvalidEngineState { .. } => ErrorCategory::DataCorruption,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            RetentionError::InvalidSchedule { .. } => "retention.schedule.invalid",
            RetentionError::NotDeclarative { .. } => "retention.scheduler.not_declarative",
            RetentionError::ItemMismatch { .. } => "retention.state.item_mismatch",
            RetentionError::InvalidEngineState { .. } => "retention.state.engine_invalid",
            RetentionError::InvalidBudget { .. } => "retention.plan.invalid_budget",
        }
    }
}

impl From<RetentionError> for G9Error {
    fn from(e: RetentionError) -> Self {
        let details = serde_json::to_value(&e).ok();
        let base = G9Error::new(e.category(), e.code(), e.to_string());
        match details {
            Some(d) => base.with_details(d),
            None => base,
        }
    }
}
