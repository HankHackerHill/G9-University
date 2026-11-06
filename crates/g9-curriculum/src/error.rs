//! Crate error type and its mapping onto the closed `G9Error` category set.
//! Messages never contain manifest content beyond identifiers and paths.

use crate::provenance::PromotionBlocked;
use crate::validate::ValidationReport;
use g9_domain::{ErrorCategory, G9Error};

/// Errors produced by this crate. Every variant maps to exactly one
/// [`ErrorCategory`] via `From<CurriculumError> for G9Error`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CurriculumError {
    /// The JSON text could not be parsed as a manifest (position only, no content).
    #[error("manifest could not be parsed: {0}")]
    Parse(String),
    /// The manifest parsed but failed validation; the full report is attached.
    #[error(
        "manifest failed validation: {} error(s), {} invariant violation(s)",
        .0.errors.len(),
        .0.violations.len()
    )]
    Invalid(Box<ValidationReport>),
    /// No day exists at the requested position.
    #[error("no training day at week {week}, day_of_week {day_of_week}")]
    DayNotFound {
        /// Requested 1-based week number.
        week: u32,
        /// Requested day of week (1 = Monday .. 6 = Saturday).
        day_of_week: u8,
    },
    /// `day_of_week` outside 1..=6.
    #[error("day_of_week {0} is outside 1..=6 (Mon..Sat)")]
    DayOfWeekOutOfRange(u8),
    /// A civil date that does not exist in the proleptic Gregorian calendar.
    #[error("invalid civil date {year:04}-{month:02}-{day:02}")]
    InvalidDate {
        /// Proleptic Gregorian year.
        year: i32,
        /// Month 1..=12.
        month: u8,
        /// Day of month.
        day: u8,
    },
    /// Program timelines start on a Monday (day_of_week 1).
    #[error("program start must be a Monday; got ISO weekday {weekday}")]
    StartNotMonday {
        /// ISO weekday of the supplied start (1 = Monday .. 7 = Sunday).
        weekday: u8,
    },
    /// Date arithmetic left the supported range.
    #[error("date arithmetic overflow")]
    DateOverflow,
    /// A checksum string is not 64 lowercase hex characters.
    #[error("invalid checksum: expected 64 lowercase hex characters")]
    InvalidChecksum,
    /// An identifier in a validated manifest could not be typed (should be
    /// unreachable after validation; reported, never panicked).
    #[error("identifier at {path} is not a valid {kind}")]
    InvalidId {
        /// Where in the manifest the identifier sits.
        path: String,
        /// Identifier kind (`DayId`, `ObjectiveId`, ...).
        kind: &'static str,
    },
    /// A minute count in a validated manifest is not a representable
    /// non-negative duration (should be unreachable after validation).
    #[error("minutes at {path} are not a valid duration")]
    InvalidDuration {
        /// Where in the manifest the minute count sits.
        path: String,
    },
    /// Promotion is blocked by the provenance gate.
    #[error("promotion blocked: {0}")]
    PromotionBlocked(#[from] PromotionBlocked),
    /// A manifest could not be serialized (should be unreachable for these types).
    #[error("manifest could not be serialized")]
    Serialization,
}

impl CurriculumError {
    /// The closed error category this variant maps to.
    pub fn category(&self) -> ErrorCategory {
        match self {
            CurriculumError::Parse(_)
            | CurriculumError::Invalid(_)
            | CurriculumError::DayOfWeekOutOfRange(_)
            | CurriculumError::InvalidDate { .. }
            | CurriculumError::StartNotMonday { .. }
            | CurriculumError::InvalidChecksum
            | CurriculumError::InvalidId { .. }
            | CurriculumError::InvalidDuration { .. } => ErrorCategory::InvalidInput,
            CurriculumError::DayNotFound { .. } => ErrorCategory::NotFound,
            CurriculumError::PromotionBlocked(_) => ErrorCategory::PreconditionFailed,
            CurriculumError::DateOverflow | CurriculumError::Serialization => {
                ErrorCategory::Internal
            }
        }
    }

    /// Stable machine-readable code (`curriculum.<...>`).
    pub fn code(&self) -> &'static str {
        match self {
            CurriculumError::Parse(_) => "curriculum.manifest.parse",
            CurriculumError::Invalid(_) => "curriculum.manifest.invalid",
            CurriculumError::DayNotFound { .. } => "curriculum.day.not_found",
            CurriculumError::DayOfWeekOutOfRange(_) => "curriculum.day_of_week.out_of_range",
            CurriculumError::InvalidDate { .. } => "curriculum.timeline.invalid_date",
            CurriculumError::StartNotMonday { .. } => "curriculum.timeline.start_not_monday",
            CurriculumError::DateOverflow => "curriculum.timeline.overflow",
            CurriculumError::InvalidChecksum => "curriculum.checksum.invalid",
            CurriculumError::InvalidId { .. } => "curriculum.id.invalid",
            CurriculumError::InvalidDuration { .. } => "curriculum.duration.invalid",
            CurriculumError::PromotionBlocked(_) => "curriculum.promotion_blocked",
            CurriculumError::Serialization => "curriculum.serialization",
        }
    }
}

impl From<CurriculumError> for G9Error {
    fn from(e: CurriculumError) -> Self {
        let category = e.category();
        let code = e.code();
        match e {
            CurriculumError::Invalid(report) => {
                let details = serde_json::to_value(&*report).ok();
                let err = G9Error::new(category, code, e_message(&report));
                match details {
                    Some(d) => err.with_details(d),
                    None => err,
                }
            }
            CurriculumError::PromotionBlocked(blocked) => {
                let message = blocked.to_string();
                let err = G9Error::new(category, code, message);
                match serde_json::to_value(&blocked).ok() {
                    Some(d) => err.with_details(d),
                    None => err,
                }
            }
            CurriculumError::DateOverflow | CurriculumError::Serialization => {
                G9Error::internal_redacted(code)
            }
            other => G9Error::new(category, code, other.to_string()),
        }
    }
}

fn e_message(report: &ValidationReport) -> String {
    format!(
        "manifest failed validation: {} error(s), {} invariant violation(s), {} warning(s)",
        report.errors.len(),
        report.violations.len(),
        report.warnings.len()
    )
}
