//! Exam integrity (ARCHITECTURE.md §8 "Sealed keys"; handoff §9 "Exam
//! integrity"; gate A07 exam leakage).
//!
//! - [`ExamContextPolicy`] decides whether a piece of content may enter model
//!   context given the interaction mode, its classification and its kind. Sealed
//!   keys (`Secret`) and answer keys never do; hints and reference solutions only
//!   where the mode allows teaching.
//! - [`HintPolicy`] delegates to `InteractionMode` (no hints in EXAM) and bounds
//!   the assistance level per mode.
//! - [`holdout_leakage_check`] intersects exam item hashes with the hashes of
//!   chunks assembled into a prompt; any intersection is a leak.

use crate::attempt::ExposureKind;
use g9_domain::{AssistanceLevel, Classification, G9Error, InteractionMode};
use std::collections::BTreeSet;

/// Integrity violation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntegrityError {
    /// Exam items were found in model context.
    #[error("holdout leakage: {} exam item hash(es) present in model context", hashes.len())]
    HoldoutLeakage {
        /// Normalized hashes found in both sets (sorted, unique).
        hashes: Vec<String>,
    },
    /// Requested assistance exceeds what the mode allows.
    #[error("assistance level {requested:?} exceeds the maximum {max:?} for mode {mode:?}")]
    AssistanceNotAllowed {
        /// Mode.
        mode: InteractionMode,
        /// Requested level.
        requested: AssistanceLevel,
        /// Maximum for the mode.
        max: AssistanceLevel,
    },
    /// Content may not enter model context.
    #[error("{kind:?} content classified {classification:?} is not allowed in model context in mode {mode:?}")]
    ContextNotAllowed {
        /// Mode.
        mode: InteractionMode,
        /// Classification.
        classification: Classification,
        /// Content kind.
        kind: ExposureKind,
    },
}

impl IntegrityError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            IntegrityError::HoldoutLeakage { .. } => "assessment.exam.holdout_leakage",
            IntegrityError::AssistanceNotAllowed { .. } => "assessment.exam.assistance_not_allowed",
            IntegrityError::ContextNotAllowed { .. } => "assessment.exam.context_not_allowed",
        }
    }
}

impl From<IntegrityError> for G9Error {
    fn from(e: IntegrityError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            IntegrityError::HoldoutLeakage { hashes } => G9Error::unauthorized(code, message)
                .with_details(serde_json::json!({ "hashes": hashes })),
            IntegrityError::AssistanceNotAllowed { .. }
            | IntegrityError::ContextNotAllowed { .. } => G9Error::unauthorized(code, message),
        }
    }
}

/// What may enter model context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExamContextPolicy;

impl ExamContextPolicy {
    /// Whether content of `kind` classified `classification` may be assembled
    /// into model context in `mode`.
    ///
    /// | rule | effect |
    /// |---|---|
    /// | `Private`/`Secret` classification | never (sealed keys are `Secret`) |
    /// | `AnswerKey` | never |
    /// | `Hint` | only where `mode.hints_allowed()` (never in EXAM) |
    /// | `ReferenceSolution` | only where `mode.teaching_allowed()` (never in EXAM) |
    /// | `Task` | yes (minimal exam context) |
    pub const fn allowed(
        mode: InteractionMode,
        classification: Classification,
        kind: ExposureKind,
    ) -> bool {
        if classification.requires_approval_for_disclosure() {
            return false;
        }
        match kind {
            ExposureKind::AnswerKey => false,
            ExposureKind::Hint => mode.hints_allowed(),
            ExposureKind::ReferenceSolution => mode.teaching_allowed(),
            ExposureKind::Task => true,
        }
    }

    /// [`ExamContextPolicy::allowed`] as a result.
    pub fn check(
        mode: InteractionMode,
        classification: Classification,
        kind: ExposureKind,
    ) -> Result<(), IntegrityError> {
        if Self::allowed(mode, classification, kind) {
            Ok(())
        } else {
            Err(IntegrityError::ContextNotAllowed {
                mode,
                classification,
                kind,
            })
        }
    }
}

/// Hint rules per mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HintPolicy;

impl HintPolicy {
    /// Delegates to `InteractionMode::hints_allowed` (Study and Lab only).
    pub const fn hints_allowed(mode: InteractionMode) -> bool {
        mode.hints_allowed()
    }

    /// Highest assistance level permitted in `mode`: `Shown` where hints are
    /// allowed, `Prompted` in Review (Socratic prompts, no method), `Unaided`
    /// everywhere else including EXAM.
    pub const fn max_assistance(mode: InteractionMode) -> AssistanceLevel {
        match mode {
            InteractionMode::Study | InteractionMode::Lab => AssistanceLevel::Shown,
            InteractionMode::Review => AssistanceLevel::Prompted,
            InteractionMode::Exam
            | InteractionMode::Executive
            | InteractionMode::Research
            | InteractionMode::Maintenance => AssistanceLevel::Unaided,
        }
    }

    /// Refuse `requested` when it exceeds [`HintPolicy::max_assistance`].
    pub fn check(mode: InteractionMode, requested: AssistanceLevel) -> Result<(), IntegrityError> {
        let max = Self::max_assistance(mode);
        if requested > max {
            Err(IntegrityError::AssistanceNotAllowed {
                mode,
                requested,
                max,
            })
        } else {
            Ok(())
        }
    }
}

fn normalize(hash: &str) -> String {
    hash.trim().to_ascii_lowercase()
}

/// Hashes present in both the exam item set and the context chunk set, after
/// trimming and lower-casing; sorted and de-duplicated. Empty means no leak.
pub fn holdout_leakage_check(
    exam_item_hashes: &[String],
    context_chunk_hashes: &[String],
) -> Vec<String> {
    let exam: BTreeSet<String> = exam_item_hashes
        .iter()
        .map(|h| normalize(h))
        .filter(|h| !h.is_empty())
        .collect();
    let leaked: BTreeSet<String> = context_chunk_hashes
        .iter()
        .map(|h| normalize(h))
        .filter(|h| exam.contains(h))
        .collect();
    leaked.into_iter().collect()
}

/// [`holdout_leakage_check`] as a result: `Err` when any hash leaked.
pub fn ensure_no_holdout_leakage(
    exam_item_hashes: &[String],
    context_chunk_hashes: &[String],
) -> Result<(), IntegrityError> {
    let hashes = holdout_leakage_check(exam_item_hashes, context_chunk_hashes);
    if hashes.is_empty() {
        Ok(())
    } else {
        Err(IntegrityError::HoldoutLeakage { hashes })
    }
}
