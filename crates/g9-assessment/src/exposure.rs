//! Exam item variants, the exposure ledger and blind eligibility
//! (ARCHITECTURE.md Addendum A item 3; defect register LRN-13; handoff §9
//! "Exam integrity").
//!
//! For a single learner every exam item is consumed on first exposure. The
//! ledger records, per learner, every exposure of an item variant or of a
//! whole fixture family:
//!
//! | [`ExposureType`] | meaning | scope |
//! |---|---|---|
//! | `DISPLAYED` | the item was shown to the learner (task, hint, key, reference) | variant |
//! | `AI_CONTEXT` | the item entered model or skill context | variant |
//! | `PUBLIC_SOURCE_DECLARED` | the learner declared prior familiarity (public write-up, earlier course) | family |
//!
//! An item is blind-eligible for a learner iff ([`ExposureLedger::blind_eligibility`]):
//!
//! 1. its status is `SEALED`;
//! 2. the variant has no `DISPLAYED`/`AI_CONTEXT` entry for this learner (ever);
//! 3. the family has no `PUBLIC_SOURCE_DECLARED` entry for this learner;
//! 4. no sibling variant of the same family was `DISPLAYED`/`AI_CONTEXT` for this
//!    learner within [`SIBLING_VARIANT_COOLDOWN_DAYS`] before `now` (or later).
//!
//! A retake uses a different variant of equal declared difficulty that is
//! itself blind-eligible ([`check_retake_item`]); `Attempt::new_retake`
//! enforces the different-variant and difficulty rules on the attempt.
//! The ledger is append-only: it has no API that edits or removes an entry.

use crate::attempt::Attempt;
use crate::definition::AssessmentVersion;
use g9_domain::{
    Actor, ActorId, ActorKind, AssessmentId, Difficulty, G9Error, InteractionMode, UtcMillis,
};
use serde::{Deserialize, Serialize};

/// Minimum distance between the exposure of a sibling variant and a blind use
/// of another variant of the same fixture family (LRN-13 confirmation).
pub const SIBLING_VARIANT_COOLDOWN_DAYS: u32 = 28;

const DAY_MS: i64 = 86_400_000;

/// Maximum byte length of a family or variant code.
pub const MAX_ITEM_CODE_LEN: usize = 64;

/// Lifecycle status of an exam item variant (LRN-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamItemStatus {
    /// Being written.
    Draft,
    /// Reviewed, key not yet sealed.
    Reviewed,
    /// Key sealed; usable for blind assessment.
    Sealed,
    /// Consumed by an exposure.
    Exposed,
    /// Withdrawn from the bank.
    Retired,
}

/// Kind of exposure recorded in the ledger (LRN-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposureType {
    /// Shown to the learner.
    Displayed,
    /// Entered model or skill context.
    AiContext,
    /// The learner declared prior familiarity with the fixture family.
    PublicSourceDeclared,
}

impl ExposureType {
    /// Every type.
    pub const ALL: [ExposureType; 3] = [
        ExposureType::Displayed,
        ExposureType::AiContext,
        ExposureType::PublicSourceDeclared,
    ];

    /// Ledger type of an exposure to `audience`: operator → `DISPLAYED`,
    /// model or skill → `AI_CONTEXT`, system (deterministic checker) → none.
    pub const fn for_audience(audience: &Actor) -> Option<Self> {
        match audience.kind {
            ActorKind::Operator => Some(ExposureType::Displayed),
            ActorKind::Model | ActorKind::Skill { .. } => Some(ExposureType::AiContext),
            ActorKind::System => None,
        }
    }

    /// Whether the entry concerns one variant (`DISPLAYED`, `AI_CONTEXT`)
    /// rather than the whole family.
    pub const fn is_variant_scoped(self) -> bool {
        !matches!(self, ExposureType::PublicSourceDeclared)
    }
}

/// Exam bank or ledger failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExposureError {
    /// A family or variant code is empty, too long or malformed.
    #[error("{field} is empty, longer than {MAX_ITEM_CODE_LEN} bytes, or contains characters outside [A-Za-z0-9._-]")]
    InvalidCode {
        /// Field name.
        field: &'static str,
    },
    /// The difficulty rationale is empty.
    #[error("declared difficulty needs a rationale")]
    EmptyDifficultyRationale,
    /// A variant-scoped exposure names no variant.
    #[error("a {0:?} exposure must name the variant")]
    MissingVariant(ExposureType),
    /// The retake candidate is the original variant.
    #[error("a retake must use a different variant than the original")]
    SameVariant,
    /// The retake candidate's declared difficulty differs.
    #[error("retake variant difficulty {candidate} differs from the original's {original}")]
    DifficultyMismatch {
        /// Original difficulty.
        original: u8,
        /// Candidate difficulty.
        candidate: u8,
    },
    /// The item is not blind-eligible for the learner.
    #[error("item is not blind-eligible: {} reason(s)", reasons.len())]
    NotBlindEligible {
        /// Every reason, in rule order.
        reasons: Vec<Ineligibility>,
    },
}

impl ExposureError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            ExposureError::InvalidCode { .. } => "assessment.exposure.invalid_code",
            ExposureError::EmptyDifficultyRationale => {
                "assessment.exposure.empty_difficulty_rationale"
            }
            ExposureError::MissingVariant(_) => "assessment.exposure.missing_variant",
            ExposureError::SameVariant => "assessment.exposure.retake_same_variant",
            ExposureError::DifficultyMismatch { .. } => {
                "assessment.exposure.retake_difficulty_mismatch"
            }
            ExposureError::NotBlindEligible { .. } => "assessment.exposure.not_blind_eligible",
        }
    }
}

impl From<ExposureError> for G9Error {
    fn from(e: ExposureError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            ExposureError::InvalidCode { .. }
            | ExposureError::EmptyDifficultyRationale
            | ExposureError::MissingVariant(_) => G9Error::invalid_input(code, message),
            ExposureError::SameVariant | ExposureError::DifficultyMismatch { .. } => {
                G9Error::precondition_failed(code, message)
            }
            ExposureError::NotBlindEligible { reasons } => {
                G9Error::precondition_failed(code, message)
                    .with_details(serde_json::json!({ "reasons": reasons }))
            }
        }
    }
}

fn check_code(field: &'static str, code: &str) -> Result<(), ExposureError> {
    let ok = !code.is_empty()
        && code.len() <= MAX_ITEM_CODE_LEN
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(ExposureError::InvalidCode { field })
    }
}

/// One exam item variant: an assessment version placed in a fixture family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExamItem {
    /// Definition the variant belongs to.
    pub definition_id: AssessmentId,
    /// Assessment version that realizes the variant.
    pub version: u32,
    /// Fixture family (variants of one family share the underlying lab fixture).
    pub fixture_family: String,
    /// Variant code within the family.
    pub variant: String,
    /// Declared difficulty (equals the version's difficulty).
    pub declared_difficulty: Difficulty,
    /// Why this difficulty was declared.
    pub difficulty_rationale: String,
    /// Lifecycle status.
    pub status: ExamItemStatus,
}

impl ExamItem {
    /// Describe `version` as a variant of `fixture_family`.
    pub fn for_version(
        version: &AssessmentVersion,
        fixture_family: impl Into<String>,
        variant: impl Into<String>,
        difficulty_rationale: impl Into<String>,
        status: ExamItemStatus,
    ) -> Result<Self, ExposureError> {
        let item = Self {
            definition_id: version.definition_id.clone(),
            version: version.version,
            fixture_family: fixture_family.into(),
            variant: variant.into(),
            declared_difficulty: version.difficulty,
            difficulty_rationale: difficulty_rationale.into(),
            status,
        };
        item.validate()?;
        Ok(item)
    }

    /// Re-check codes and rationale.
    pub fn validate(&self) -> Result<(), ExposureError> {
        check_code("fixture_family", &self.fixture_family)?;
        check_code("variant", &self.variant)?;
        if self.difficulty_rationale.trim().is_empty() {
            return Err(ExposureError::EmptyDifficultyRationale);
        }
        Ok(())
    }

    /// Whether `other` is the same variant: same family and variant code, or
    /// the same assessment version.
    pub fn same_variant(&self, other: &ExamItem) -> bool {
        (self.fixture_family == other.fixture_family && self.variant == other.variant)
            || (self.definition_id == other.definition_id && self.version == other.version)
    }
}

/// One immutable ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureEntry {
    /// Learner whose blindness the exposure affects.
    pub learner: ActorId,
    /// Fixture family.
    pub fixture_family: String,
    /// Variant; required for `DISPLAYED`/`AI_CONTEXT`, optional (ignored) for
    /// `PUBLIC_SOURCE_DECLARED`, which covers the whole family.
    pub variant: Option<String>,
    /// Exposure kind.
    pub exposure: ExposureType,
    /// Interaction mode in force.
    pub mode: InteractionMode,
    /// Actor that caused or declared the exposure.
    pub recorded_by: ActorId,
    /// When.
    pub at: UtcMillis,
}

impl ExposureEntry {
    /// Re-check codes and the variant requirement.
    pub fn validate(&self) -> Result<(), ExposureError> {
        check_code("fixture_family", &self.fixture_family)?;
        match (&self.variant, self.exposure.is_variant_scoped()) {
            (Some(v), _) => check_code("variant", v),
            (None, true) => Err(ExposureError::MissingVariant(self.exposure)),
            (None, false) => Ok(()),
        }
    }
}

/// Why an item is not blind-eligible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Ineligibility {
    /// The item is not `SEALED`.
    NotSealed {
        /// Current status.
        status: ExamItemStatus,
    },
    /// The variant itself was exposed to the learner.
    VariantExposed {
        /// Exposure kind.
        exposure: ExposureType,
        /// When.
        at: UtcMillis,
    },
    /// The learner declared prior familiarity with the family.
    FamilyDeclaredFamiliar {
        /// When.
        at: UtcMillis,
    },
    /// A sibling variant was exposed inside the cooldown window.
    SiblingVariantExposed {
        /// Sibling variant code.
        variant: String,
        /// When.
        at: UtcMillis,
    },
}

/// Outcome of the blind-eligibility rule.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlindEligibility {
    /// Every reason the item is ineligible, in rule order; empty = eligible.
    pub reasons: Vec<Ineligibility>,
}

impl BlindEligibility {
    /// Whether no rule failed.
    pub fn is_eligible(&self) -> bool {
        self.reasons.is_empty()
    }
}

/// Append-only per-learner exposure ledger. Serialized as the list of
/// entries; deserialization validates every entry.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<ExposureEntry>", into = "Vec<ExposureEntry>")]
pub struct ExposureLedger {
    entries: Vec<ExposureEntry>,
}

impl TryFrom<Vec<ExposureEntry>> for ExposureLedger {
    type Error = ExposureError;
    fn try_from(entries: Vec<ExposureEntry>) -> Result<Self, Self::Error> {
        Self::from_entries(entries)
    }
}

impl From<ExposureLedger> for Vec<ExposureEntry> {
    fn from(ledger: ExposureLedger) -> Self {
        ledger.entries
    }
}

impl ExposureLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild a ledger from stored entries, validating each.
    pub fn from_entries(entries: Vec<ExposureEntry>) -> Result<Self, ExposureError> {
        for e in &entries {
            e.validate()?;
        }
        Ok(Self { entries })
    }

    /// Every entry, in append order.
    pub fn entries(&self) -> &[ExposureEntry] {
        &self.entries
    }

    /// Append one entry.
    pub fn record(&mut self, entry: ExposureEntry) -> Result<(), ExposureError> {
        entry.validate()?;
        self.entries.push(entry);
        Ok(())
    }

    /// Append an exposure of `item` (variant-scoped for `DISPLAYED` and
    /// `AI_CONTEXT`, family-scoped for `PUBLIC_SOURCE_DECLARED`).
    pub fn record_item(
        &mut self,
        item: &ExamItem,
        learner: &ActorId,
        exposure: ExposureType,
        mode: InteractionMode,
        recorded_by: &ActorId,
        at: UtcMillis,
    ) -> Result<(), ExposureError> {
        self.record(ExposureEntry {
            learner: learner.clone(),
            fixture_family: item.fixture_family.clone(),
            variant: exposure.is_variant_scoped().then(|| item.variant.clone()),
            exposure,
            mode,
            recorded_by: recorded_by.clone(),
            at,
        })
    }

    /// Append one entry per exposure record of `attempt` on `item`, typed by
    /// audience ([`ExposureType::for_audience`]); exposures to the system are
    /// skipped. Returns the number of entries appended.
    pub fn record_attempt_exposures(
        &mut self,
        item: &ExamItem,
        attempt: &Attempt,
    ) -> Result<usize, ExposureError> {
        let mut appended = 0;
        for record in attempt.exposure() {
            if let Some(exposure) = ExposureType::for_audience(&record.actor) {
                self.record_item(
                    item,
                    attempt.learner(),
                    exposure,
                    attempt.mode(),
                    &record.actor.id,
                    record.at,
                )?;
                appended += 1;
            }
        }
        Ok(appended)
    }

    /// Evaluate the LRN-13 blind-eligibility rule for `learner` at `now`.
    pub fn blind_eligibility(
        &self,
        item: &ExamItem,
        learner: &ActorId,
        now: UtcMillis,
    ) -> BlindEligibility {
        let mut reasons = Vec::new();
        if item.status != ExamItemStatus::Sealed {
            reasons.push(Ineligibility::NotSealed {
                status: item.status,
            });
        }
        let cooldown_ms = i64::from(SIBLING_VARIANT_COOLDOWN_DAYS).saturating_mul(DAY_MS);
        let mine = self
            .entries
            .iter()
            .filter(|e| &e.learner == learner && e.fixture_family == item.fixture_family);
        for e in mine {
            match (e.exposure, e.variant.as_deref()) {
                (ExposureType::PublicSourceDeclared, _) => {
                    reasons.push(Ineligibility::FamilyDeclaredFamiliar { at: e.at });
                }
                (exposure, Some(v)) if v == item.variant => {
                    reasons.push(Ineligibility::VariantExposed { exposure, at: e.at });
                }
                (_, Some(v)) => {
                    let since = now.as_millis().saturating_sub(e.at.as_millis());
                    if since < cooldown_ms {
                        reasons.push(Ineligibility::SiblingVariantExposed {
                            variant: v.to_string(),
                            at: e.at,
                        });
                    }
                }
                // A variant-scoped entry without a variant cannot be appended.
                (_, None) => {}
            }
        }
        BlindEligibility { reasons }
    }
}

/// Whether `item` is blind-eligible for `learner` at `now` under `ledger`.
pub fn blind_eligible(
    item: &ExamItem,
    ledger: &ExposureLedger,
    learner: &ActorId,
    now: UtcMillis,
) -> bool {
    ledger.blind_eligibility(item, learner, now).is_eligible()
}

/// Check a retake candidate (LRN-13): a different variant than `original`, of
/// equal declared difficulty, that is itself blind-eligible for `learner`.
pub fn check_retake_item(
    original: &ExamItem,
    candidate: &ExamItem,
    ledger: &ExposureLedger,
    learner: &ActorId,
    now: UtcMillis,
) -> Result<(), ExposureError> {
    if original.same_variant(candidate) {
        return Err(ExposureError::SameVariant);
    }
    if original.declared_difficulty != candidate.declared_difficulty {
        return Err(ExposureError::DifficultyMismatch {
            original: original.declared_difficulty.get(),
            candidate: candidate.declared_difficulty.get(),
        });
    }
    let eligibility = ledger.blind_eligibility(candidate, learner, now);
    if eligibility.is_eligible() {
        Ok(())
    } else {
        Err(ExposureError::NotBlindEligible {
            reasons: eligibility.reasons,
        })
    }
}
