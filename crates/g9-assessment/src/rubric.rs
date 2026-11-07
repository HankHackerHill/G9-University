//! Rubrics with anchored levels (handoff §9 "Official assessment rubric
//! (configurable baseline)"; ADR-0012; ARCHITECTURE.md Addendum A item 1;
//! defect register LRN-10; gate B01).
//!
//! A [`Rubric`] is a validated list of weighted criteria plus the anchored
//! scoring standard ([`RubricAnchors`]) it is graded with. Invariants enforced
//! by [`Rubric::new`] / [`Rubric::with_anchors`] / [`Rubric::validate`] and on
//! deserialization:
//!
//! - weights sum to exactly [`REQUIRED_TOTAL_WEIGHT`] (100),
//! - criterion codes are unique and well-formed, titles are non-empty,
//! - at least one criterion is a critical gate,
//! - every criterion has written anchors for each level 0..=4, in rubric order,
//! - the level → score mapping ([`LevelScale`]) is explicit, strictly
//!   increasing from 0 to 100, and agrees with the pass rule: level 1 is below
//!   the [`CRITERION_FLOOR_PERCENT`] floor, level 2 meets it, and level 3 on
//!   every criterion reaches the pass threshold.
//!
//! The anchors are serialized with the rubric, so they are stored with (and
//! covered by the checksum of) the `AssessmentVersion` that embeds the rubric.
//!
//! Every criterion maps to one [`LearningDimension`] so that grade decisions
//! can be turned into dimension-specific competency observations.

use g9_domain::hashing::sha256_json;
use g9_domain::{BaselineRubric, G9Error, LearningDimension, PASS_THRESHOLD};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Required sum of criterion weights.
pub const REQUIRED_TOTAL_WEIGHT: u32 = 100;

/// Maximum byte length of a criterion code.
pub const MAX_CODE_LEN: usize = 64;

/// Number of anchored levels (0..=4).
pub const LEVEL_COUNT: usize = 5;

/// Highest anchored level.
pub const MAX_LEVEL: u8 = 4;

/// Per-criterion floor (ADR-0012): every criterion must reach at least this
/// percentage of its weight, i.e. a criterion score of at least 50 on the
/// 0..=100 scale, or the attempt fails regardless of the weighted total.
pub const CRITERION_FLOOR_PERCENT: u8 = 50;

/// Target share (percent) of rubric weight that official blind practicals
/// should make machine-checkable (ADR-0012, LRN-11). A design target that is
/// reported by [`Rubric::meets_machine_checkable_target`], not enforced: the
/// baseline rubric reaches 45.
pub const MACHINE_CHECKABLE_TARGET_PERCENT: u8 = 60;

/// Standard-setting rationale recorded with the standard scale (LRN-10).
pub const STANDARD_SETTING_NOTE: &str = "Criterion-referenced expert judgment (defect register LRN-10, \
confirmed option a): anchored levels 0..4 map to 0, 25, 50, 80 and 100 percent of the criterion weight, so \
that level 3 (correct and complete) on every criterion totals exactly the pass threshold of 80 and level 2 \
(partially correct) meets the 50 percent per-criterion floor. The cut score is not empirically \
standard-set.";

/// Generic level descriptors from the anchored scale of LRN-10. Used for the
/// baseline rubric and for rubrics built with [`Rubric::new`]; authors
/// replace them with criterion-specific anchors through [`Rubric::with_anchors`].
pub const STANDARD_LEVEL_DESCRIPTORS: [&str; LEVEL_COUNT] = [
    "Absent: no relevant work for this criterion.",
    "Attempted but incorrect.",
    "Partially correct with material gaps.",
    "Correct and complete.",
    "Correct, complete and verified.",
];

/// Validation failure for a rubric or its anchors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RubricError {
    /// The rubric lists no criteria.
    #[error("rubric has no criteria")]
    Empty,
    /// Weights do not sum to [`REQUIRED_TOTAL_WEIGHT`].
    #[error("rubric weights sum to {actual}, expected {expected}")]
    WeightSum {
        /// Observed sum.
        actual: u32,
        /// Required sum.
        expected: u32,
    },
    /// Two criteria share a code.
    #[error("duplicate criterion code `{0}`")]
    DuplicateCode(String),
    /// The code at the given position is empty, too long, or uses characters
    /// outside `[A-Za-z0-9._-]`. The offending value is not echoed.
    #[error("criterion code at position {index} is empty, longer than {MAX_CODE_LEN} bytes, or contains characters outside [A-Za-z0-9._-]")]
    InvalidCode {
        /// Zero-based position of the criterion in the rubric.
        index: usize,
    },
    /// A criterion has an empty title.
    #[error("criterion `{0}` has an empty title")]
    EmptyTitle(String),
    /// No criterion is marked as a critical gate.
    #[error("rubric has no critical gate criterion")]
    NoCriticalGate,
    /// An anchored level outside 0..=4.
    #[error("anchored level must be within 0..={MAX_LEVEL}, got {0}")]
    InvalidLevel(u8),
    /// The level → score mapping violates a scale rule.
    #[error("invalid level scale: {0}")]
    InvalidLevelScale(&'static str),
    /// The anchors do not list exactly the rubric criteria, in rubric order.
    #[error("anchors at position {index} do not match the rubric criterion at that position")]
    AnchorsMismatch {
        /// Zero-based position.
        index: usize,
    },
    /// A criterion does not have exactly one non-empty descriptor per level.
    #[error("criterion `{0}` needs exactly one non-empty anchor descriptor per level 0..=4")]
    InvalidAnchorDescriptors(String),
    /// The standard-setting note is empty.
    #[error("standard-setting note is empty")]
    EmptyStandardSettingNote,
    /// The rubric could not be serialized for hashing.
    #[error("rubric could not be serialized for hashing")]
    Serialization,
}

impl RubricError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            RubricError::Empty => "assessment.rubric.empty",
            RubricError::WeightSum { .. } => "assessment.rubric.weight_sum",
            RubricError::DuplicateCode(_) => "assessment.rubric.duplicate_code",
            RubricError::InvalidCode { .. } => "assessment.rubric.invalid_code",
            RubricError::EmptyTitle(_) => "assessment.rubric.empty_title",
            RubricError::NoCriticalGate => "assessment.rubric.no_critical_gate",
            RubricError::InvalidLevel(_) => "assessment.rubric.invalid_level",
            RubricError::InvalidLevelScale(_) => "assessment.rubric.invalid_level_scale",
            RubricError::AnchorsMismatch { .. } => "assessment.rubric.anchors_mismatch",
            RubricError::InvalidAnchorDescriptors(_) => {
                "assessment.rubric.invalid_anchor_descriptors"
            }
            RubricError::EmptyStandardSettingNote => {
                "assessment.rubric.empty_standard_setting_note"
            }
            RubricError::Serialization => "assessment.rubric.serialization",
        }
    }
}

impl From<RubricError> for G9Error {
    fn from(e: RubricError) -> Self {
        let code = e.code();
        match e {
            RubricError::Serialization => G9Error::internal_redacted(code),
            other => G9Error::invalid_input(code, other.to_string()),
        }
    }
}

/// One rubric criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricCriterionDef {
    /// Stable code, e.g. `independent_execution`.
    pub code: String,
    /// Human-readable title.
    pub title: String,
    /// Weight in percent; all weights of a rubric sum to 100.
    pub weight: u8,
    /// Whether a deterministic checker may score this criterion automatically.
    pub machine_checkable: bool,
    /// Critical-gate criterion (handoff §9 "mandatory critical integrity/scope
    /// gates"). Under ADR-0012 a critical-gate criterion fails exactly when it
    /// misses the per-criterion floor; the binary integrity/scope gates are the
    /// enumerated `grading::GateFailure`s.
    pub critical_gate: bool,
    /// Learning dimension this criterion is evidence for.
    pub dimension: LearningDimension,
}

/// An anchored performance level 0..=4 (LRN-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct AnchoredLevel(u8);

impl AnchoredLevel {
    /// Every level, lowest first.
    pub const ALL: [AnchoredLevel; LEVEL_COUNT] = [
        AnchoredLevel(0),
        AnchoredLevel(1),
        AnchoredLevel(2),
        AnchoredLevel(3),
        AnchoredLevel(4),
    ];

    /// Validate a raw level.
    pub fn new(level: u8) -> Result<Self, RubricError> {
        if level <= MAX_LEVEL {
            Ok(Self(level))
        } else {
            Err(RubricError::InvalidLevel(level))
        }
    }

    /// Raw value.
    pub const fn get(self) -> u8 {
        self.0
    }

    fn index(self) -> usize {
        usize::from(self.0)
    }
}

impl TryFrom<u8> for AnchoredLevel {
    type Error = RubricError;
    fn try_from(level: u8) -> Result<Self, Self::Error> {
        Self::new(level)
    }
}

impl From<AnchoredLevel> for u8 {
    fn from(l: AnchoredLevel) -> u8 {
        l.0
    }
}

/// Explicit anchored level → criterion score (0..=100, percent of the
/// criterion weight) mapping. Serialized as the five scores, level 0 first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "[u8; LEVEL_COUNT]", into = "[u8; LEVEL_COUNT]")]
pub struct LevelScale {
    scores: [u8; LEVEL_COUNT],
}

impl LevelScale {
    /// The standard scale (LRN-10 confirmed option a): 0, 25, 50, 80, 100.
    pub const STANDARD: LevelScale = LevelScale {
        scores: [0, 25, 50, 80, 100],
    };

    /// Validate a mapping. Rules: level 0 → 0, level 4 → 100, strictly
    /// increasing, level 1 below the [`CRITERION_FLOOR_PERCENT`] floor, level 2
    /// at or above it, level 3 at or above `PASS_THRESHOLD` (so that the scale
    /// agrees with the cut score).
    pub fn new(scores: [u8; LEVEL_COUNT]) -> Result<Self, RubricError> {
        if scores[0] != 0 {
            return Err(RubricError::InvalidLevelScale("level 0 must map to 0"));
        }
        if scores[LEVEL_COUNT - 1] != 100 {
            return Err(RubricError::InvalidLevelScale("level 4 must map to 100"));
        }
        if scores.windows(2).any(|w| w[0] >= w[1]) {
            return Err(RubricError::InvalidLevelScale(
                "scores must be strictly increasing",
            ));
        }
        if scores[1] >= CRITERION_FLOOR_PERCENT {
            return Err(RubricError::InvalidLevelScale(
                "level 1 must be below the per-criterion floor",
            ));
        }
        if scores[2] < CRITERION_FLOOR_PERCENT {
            return Err(RubricError::InvalidLevelScale(
                "level 2 must meet the per-criterion floor",
            ));
        }
        if scores[3] < PASS_THRESHOLD {
            return Err(RubricError::InvalidLevelScale(
                "level 3 must reach the pass threshold",
            ));
        }
        Ok(Self { scores })
    }

    /// Score (0..=100) of a level.
    pub fn score_for(&self, level: AnchoredLevel) -> u8 {
        self.scores[level.index()]
    }

    /// Highest level whose mapped score does not exceed `score` (floor
    /// mapping; scores above 100 map to level 4).
    pub fn level_for(&self, score: u8) -> AnchoredLevel {
        AnchoredLevel::ALL
            .iter()
            .rev()
            .copied()
            .find(|l| self.score_for(*l) <= score)
            .unwrap_or(AnchoredLevel(0))
    }

    /// The five scores, level 0 first.
    pub const fn scores(&self) -> [u8; LEVEL_COUNT] {
        self.scores
    }
}

impl Default for LevelScale {
    fn default() -> Self {
        Self::STANDARD
    }
}

impl TryFrom<[u8; LEVEL_COUNT]> for LevelScale {
    type Error = RubricError;
    fn try_from(scores: [u8; LEVEL_COUNT]) -> Result<Self, Self::Error> {
        Self::new(scores)
    }
}

impl From<LevelScale> for [u8; LEVEL_COUNT] {
    fn from(s: LevelScale) -> Self {
        s.scores
    }
}

/// Written anchors for one criterion: one descriptor per level 0..=4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionAnchors {
    /// Criterion code (must match the rubric criterion at the same position).
    pub code: String,
    /// Descriptors for levels 0..=4, level 0 first.
    pub descriptors: Vec<String>,
}

impl CriterionAnchors {
    /// The generic [`STANDARD_LEVEL_DESCRIPTORS`] for `code`.
    pub fn standard(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            descriptors: STANDARD_LEVEL_DESCRIPTORS
                .iter()
                .map(|d| (*d).to_string())
                .collect(),
        }
    }

    /// Descriptor of a level.
    pub fn descriptor(&self, level: AnchoredLevel) -> Option<&str> {
        self.descriptors.get(level.index()).map(String::as_str)
    }
}

/// The anchored scoring standard of a rubric: level scale, standard-setting
/// note and per-criterion anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricAnchors {
    /// Level → score mapping.
    pub scale: LevelScale,
    /// Why this mapping and cut score were chosen (LRN-10).
    pub standard_setting_note: String,
    /// One entry per rubric criterion, in rubric order.
    pub criteria: Vec<CriterionAnchors>,
}

impl RubricAnchors {
    /// Standard scale, standard note and generic descriptors for `criteria`.
    pub fn standard_for(criteria: &[RubricCriterionDef]) -> Self {
        Self {
            scale: LevelScale::STANDARD,
            standard_setting_note: STANDARD_SETTING_NOTE.to_string(),
            criteria: criteria
                .iter()
                .map(|c| CriterionAnchors::standard(c.code.clone()))
                .collect(),
        }
    }

    /// Check the anchors against the rubric criteria.
    pub fn validate_for(&self, criteria: &[RubricCriterionDef]) -> Result<(), RubricError> {
        // Re-validate the scale: a hand-built struct literal could bypass serde.
        LevelScale::new(self.scale.scores())?;
        if self.standard_setting_note.trim().is_empty() {
            return Err(RubricError::EmptyStandardSettingNote);
        }
        let n = criteria.len().max(self.criteria.len());
        for index in 0..n {
            let (Some(c), Some(a)) = (criteria.get(index), self.criteria.get(index)) else {
                return Err(RubricError::AnchorsMismatch { index });
            };
            if c.code != a.code {
                return Err(RubricError::AnchorsMismatch { index });
            }
            if a.descriptors.len() != LEVEL_COUNT
                || a.descriptors.iter().any(|d| d.trim().is_empty())
            {
                return Err(RubricError::InvalidAnchorDescriptors(c.code.clone()));
            }
        }
        Ok(())
    }

    /// Anchors of a criterion.
    pub fn for_criterion(&self, code: &str) -> Option<&CriterionAnchors> {
        self.criteria.iter().find(|a| a.code == code)
    }
}

/// Unvalidated rubric shape; [`Rubric`] deserializes through it so that an
/// invalid rubric can never be loaded. A rubric serialized without anchors
/// (before ADR-0012) loads with the standard anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricSpec {
    /// Criteria in display order.
    pub criteria: Vec<RubricCriterionDef>,
    /// Anchored scoring standard; `None` selects [`RubricAnchors::standard_for`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchors: Option<RubricAnchors>,
}

/// A validated rubric. Construct with [`Rubric::new`], [`Rubric::with_anchors`]
/// or [`Rubric::baseline`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RubricSpec")]
pub struct Rubric {
    criteria: Vec<RubricCriterionDef>,
    anchors: RubricAnchors,
}

impl TryFrom<RubricSpec> for Rubric {
    type Error = RubricError;

    fn try_from(spec: RubricSpec) -> Result<Self, Self::Error> {
        match spec.anchors {
            Some(anchors) => Rubric::with_anchors(spec.criteria, anchors),
            None => Rubric::new(spec.criteria),
        }
    }
}

fn code_is_well_formed(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_CODE_LEN
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

impl Rubric {
    /// Validate and wrap a list of criteria with the standard anchors.
    pub fn new(criteria: Vec<RubricCriterionDef>) -> Result<Self, RubricError> {
        let anchors = RubricAnchors::standard_for(&criteria);
        Self::with_anchors(criteria, anchors)
    }

    /// Validate and wrap a list of criteria with explicit anchors.
    pub fn with_anchors(
        criteria: Vec<RubricCriterionDef>,
        anchors: RubricAnchors,
    ) -> Result<Self, RubricError> {
        let rubric = Self { criteria, anchors };
        rubric.validate()?;
        Ok(rubric)
    }

    /// Re-check every invariant (weights sum to 100, unique well-formed codes,
    /// non-empty titles, at least one critical gate, valid anchors and scale).
    pub fn validate(&self) -> Result<(), RubricError> {
        if self.criteria.is_empty() {
            return Err(RubricError::Empty);
        }
        let mut seen = BTreeSet::new();
        for (index, c) in self.criteria.iter().enumerate() {
            if !code_is_well_formed(&c.code) {
                return Err(RubricError::InvalidCode { index });
            }
            if c.title.trim().is_empty() {
                return Err(RubricError::EmptyTitle(c.code.clone()));
            }
            if !seen.insert(c.code.as_str()) {
                return Err(RubricError::DuplicateCode(c.code.clone()));
            }
        }
        let actual = self.total_weight();
        if actual != REQUIRED_TOTAL_WEIGHT {
            return Err(RubricError::WeightSum {
                actual,
                expected: REQUIRED_TOTAL_WEIGHT,
            });
        }
        if !self.criteria.iter().any(|c| c.critical_gate) {
            return Err(RubricError::NoCriticalGate);
        }
        self.anchors.validate_for(&self.criteria)
    }

    /// The configurable baseline rubric from `g9_domain::BaselineRubric` with the
    /// fixed criterion → dimension mapping of [`Rubric::baseline_dimension`] and
    /// the standard anchors.
    ///
    /// The baseline is known to satisfy every invariant (covered by the B01 test);
    /// it is built directly so that this constructor cannot fail.
    pub fn baseline() -> Self {
        let criteria: Vec<RubricCriterionDef> = BaselineRubric::CRITERIA
            .iter()
            .map(|c| RubricCriterionDef {
                code: c.code.to_string(),
                title: c.title.to_string(),
                weight: c.weight,
                machine_checkable: c.machine_checkable,
                critical_gate: c.critical_gate,
                dimension: Self::baseline_dimension(c.code)
                    .unwrap_or(LearningDimension::Application),
            })
            .collect();
        let anchors = RubricAnchors::standard_for(&criteria);
        Self { criteria, anchors }
    }

    /// Dimension mapping of the baseline criteria (handoff §9):
    ///
    /// | code | dimension |
    /// |---|---|
    /// | `independent_execution` | Application |
    /// | `root_cause_reasoning` | Understanding |
    /// | `remediation_verification` | Application |
    /// | `detection_visibility` | Transfer |
    /// | `evidence_documentation` | Understanding |
    /// | `scope_compliance_reporting` | Application |
    pub fn baseline_dimension(code: &str) -> Option<LearningDimension> {
        match code {
            "independent_execution" => Some(LearningDimension::Application),
            "root_cause_reasoning" => Some(LearningDimension::Understanding),
            "remediation_verification" => Some(LearningDimension::Application),
            "detection_visibility" => Some(LearningDimension::Transfer),
            "evidence_documentation" => Some(LearningDimension::Understanding),
            "scope_compliance_reporting" => Some(LearningDimension::Application),
            _ => None,
        }
    }

    /// Criteria in display order.
    pub fn criteria(&self) -> &[RubricCriterionDef] {
        &self.criteria
    }

    /// Anchored scoring standard.
    pub fn anchors(&self) -> &RubricAnchors {
        &self.anchors
    }

    /// Level → score mapping of this rubric.
    pub fn level_scale(&self) -> LevelScale {
        self.anchors.scale
    }

    /// Score (0..=100) for a raw anchored level; refuses levels above 4.
    pub fn score_for_level(&self, level: u8) -> Result<u8, RubricError> {
        Ok(self.anchors.scale.score_for(AnchoredLevel::new(level)?))
    }

    /// Anchored level of a score (highest level whose mapped score ≤ `score`).
    pub fn level_for_score(&self, score: u8) -> AnchoredLevel {
        self.anchors.scale.level_for(score)
    }

    /// Look up a criterion by code.
    pub fn criterion(&self, code: &str) -> Option<&RubricCriterionDef> {
        self.criteria.iter().find(|c| c.code == code)
    }

    /// Dimension of a criterion, if the code exists.
    pub fn dimension_of(&self, code: &str) -> Option<LearningDimension> {
        self.criterion(code).map(|c| c.dimension)
    }

    /// Criteria marked as critical gates.
    pub fn critical_gates(&self) -> impl Iterator<Item = &RubricCriterionDef> {
        self.criteria.iter().filter(|c| c.critical_gate)
    }

    /// Sum of all weights.
    pub fn total_weight(&self) -> u32 {
        self.criteria.iter().map(|c| u32::from(c.weight)).sum()
    }

    /// Sum of the weights of machine-checkable criteria.
    pub fn machine_checkable_weight(&self) -> u32 {
        self.criteria
            .iter()
            .filter(|c| c.machine_checkable)
            .map(|c| u32::from(c.weight))
            .sum()
    }

    /// Whether at least [`MACHINE_CHECKABLE_TARGET_PERCENT`] of the weight is
    /// machine-checkable (design target for official blind practicals).
    pub fn meets_machine_checkable_target(&self) -> bool {
        self.machine_checkable_weight() * 100
            >= u32::from(MACHINE_CHECKABLE_TARGET_PERCENT) * self.total_weight()
    }

    /// Hex SHA-256 of the canonical JSON form of the rubric (criteria and
    /// anchors); used to pin the rubric inside an assessment version checksum.
    pub fn checksum(&self) -> Result<String, RubricError> {
        let value = serde_json::to_value(self).map_err(|_| RubricError::Serialization)?;
        Ok(sha256_json(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn baseline_criteria() -> Vec<RubricCriterionDef> {
        Rubric::baseline().criteria().to_vec()
    }

    #[test]
    fn baseline_matches_domain_constants() {
        let r = Rubric::baseline();
        assert_eq!(r.criteria().len(), BaselineRubric::CRITERIA.len());
        assert_eq!(r.total_weight(), BaselineRubric::total_weight());
        for (ours, theirs) in r.criteria().iter().zip(BaselineRubric::CRITERIA.iter()) {
            assert_eq!(ours.code, theirs.code);
            assert_eq!(ours.weight, theirs.weight);
            assert_eq!(ours.critical_gate, theirs.critical_gate);
            assert_eq!(ours.machine_checkable, theirs.machine_checkable);
            assert_eq!(Some(ours.dimension), Rubric::baseline_dimension(&ours.code));
        }
    }

    #[test]
    fn b01_rubric_weights_must_sum_100_and_baseline_valid() {
        let baseline = Rubric::baseline();
        assert!(baseline.validate().is_ok());
        assert_eq!(baseline.total_weight(), REQUIRED_TOTAL_WEIGHT);
        assert_eq!(Rubric::new(baseline_criteria()).unwrap(), baseline);

        // weights below and above 100
        let mut low = baseline_criteria();
        low[0].weight -= 1;
        assert_eq!(
            Rubric::new(low).unwrap_err(),
            RubricError::WeightSum {
                actual: 99,
                expected: 100
            }
        );
        let mut high = baseline_criteria();
        high[0].weight += 1;
        assert_eq!(
            Rubric::new(high).unwrap_err(),
            RubricError::WeightSum {
                actual: 101,
                expected: 100
            }
        );
        // duplicate code
        let mut dup = baseline_criteria();
        dup[1].code = dup[0].code.clone();
        assert!(matches!(
            Rubric::new(dup).unwrap_err(),
            RubricError::DuplicateCode(_)
        ));
        // no critical gate
        let mut no_gate = baseline_criteria();
        for c in &mut no_gate {
            c.critical_gate = false;
        }
        assert_eq!(
            Rubric::new(no_gate).unwrap_err(),
            RubricError::NoCriticalGate
        );
        // empty rubric and empty title
        assert_eq!(Rubric::new(vec![]).unwrap_err(), RubricError::Empty);
        let mut untitled = baseline_criteria();
        untitled[2].title = "  ".into();
        assert!(matches!(
            Rubric::new(untitled).unwrap_err(),
            RubricError::EmptyTitle(_)
        ));

        // deserialization goes through validation
        let mut bad = serde_json::to_value(&baseline).unwrap();
        bad["criteria"][0]["weight"] = serde_json::json!(24);
        assert!(serde_json::from_value::<Rubric>(bad).is_err());
        let round: Rubric =
            serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        assert_eq!(round, baseline);
    }

    #[test]
    fn rubric_without_anchors_loads_with_standard_anchors() {
        let legacy =
            serde_json::json!({ "criteria": serde_json::to_value(baseline_criteria()).unwrap() });
        let r: Rubric = serde_json::from_value(legacy).unwrap();
        assert_eq!(r, Rubric::baseline());
        assert_eq!(r.level_scale(), LevelScale::STANDARD);
    }

    #[test]
    fn checksum_is_stable_and_order_sensitive() {
        let a = Rubric::baseline();
        let b = Rubric::baseline();
        assert_eq!(a.checksum().unwrap(), b.checksum().unwrap());
        let mut reversed = a.criteria().to_vec();
        reversed.reverse();
        let c = Rubric::new(reversed).unwrap();
        assert_ne!(a.checksum().unwrap(), c.checksum().unwrap());
    }

    #[test]
    fn checksum_covers_anchors() {
        let base = Rubric::baseline();
        let mut anchors = base.anchors().clone();
        anchors.criteria[0].descriptors[4] = "Boot chain restored and verified by reboot.".into();
        let custom = Rubric::with_anchors(base.criteria().to_vec(), anchors).unwrap();
        assert_ne!(base.checksum().unwrap(), custom.checksum().unwrap());
    }

    #[test]
    fn invalid_code_is_not_echoed() {
        let mut criteria = baseline_criteria();
        criteria[0].code = "has space".to_string();
        let err = Rubric::new(criteria).unwrap_err();
        assert_eq!(err, RubricError::InvalidCode { index: 0 });
        assert!(!err.to_string().contains("has space"));
    }

    #[test]
    fn standard_level_scale_is_explicit_and_agrees_with_the_cut() {
        let s = LevelScale::STANDARD;
        assert_eq!(s.scores(), [0, 25, 50, 80, 100]);
        let mapped: Vec<u8> = AnchoredLevel::ALL.iter().map(|l| s.score_for(*l)).collect();
        assert_eq!(mapped, vec![0, 25, 50, 80, 100]);
        // level 2 meets the floor, level 1 does not; level 3 reaches the cut
        assert!(s.score_for(AnchoredLevel::new(2).unwrap()) >= CRITERION_FLOOR_PERCENT);
        assert!(s.score_for(AnchoredLevel::new(1).unwrap()) < CRITERION_FLOOR_PERCENT);
        assert_eq!(s.score_for(AnchoredLevel::new(3).unwrap()), PASS_THRESHOLD);
        // reverse (floor) mapping
        for (score, level) in [
            (0, 0),
            (24, 0),
            (25, 1),
            (49, 1),
            (50, 2),
            (79, 2),
            (80, 3),
            (99, 3),
            (100, 4),
            (255, 4),
        ] {
            assert_eq!(s.level_for(score).get(), level, "score {score}");
        }
        assert_eq!(Rubric::baseline().score_for_level(3).unwrap(), 80);
        assert_eq!(
            Rubric::baseline().score_for_level(5).unwrap_err(),
            RubricError::InvalidLevel(5)
        );
        assert!(AnchoredLevel::new(5).is_err());
        assert!(serde_json::from_str::<AnchoredLevel>("7").is_err());
    }

    #[test]
    fn level_scale_rules_are_enforced() {
        for bad in [
            [1, 25, 50, 80, 100],
            [0, 25, 50, 80, 99],
            [0, 50, 50, 80, 100],
            [0, 50, 60, 80, 100],
            [0, 25, 45, 80, 100],
            [0, 25, 50, 75, 100],
        ] {
            assert!(
                matches!(LevelScale::new(bad), Err(RubricError::InvalidLevelScale(_))),
                "{bad:?}"
            );
            assert!(serde_json::from_value::<LevelScale>(serde_json::json!(bad)).is_err());
        }
        assert!(LevelScale::new([0, 20, 55, 85, 100]).is_ok());
    }

    #[test]
    fn anchors_are_stored_with_the_rubric_and_validated() {
        let base = Rubric::baseline();
        let json = serde_json::to_value(&base).unwrap();
        assert_eq!(
            json["anchors"]["scale"],
            serde_json::json!([0, 25, 50, 80, 100])
        );
        assert_eq!(
            json["anchors"]["criteria"][0]["descriptors"]
                .as_array()
                .unwrap()
                .len(),
            LEVEL_COUNT
        );
        assert!(!base.anchors().standard_setting_note.is_empty());
        assert_eq!(
            base.anchors()
                .for_criterion("root_cause_reasoning")
                .and_then(|a| a.descriptor(AnchoredLevel::new(3).unwrap())),
            Some("Correct and complete.")
        );

        // wrong order / missing / extra criteria
        let mut swapped = base.anchors().clone();
        swapped.criteria.swap(0, 1);
        assert_eq!(
            Rubric::with_anchors(base.criteria().to_vec(), swapped).unwrap_err(),
            RubricError::AnchorsMismatch { index: 0 }
        );
        let mut missing = base.anchors().clone();
        missing.criteria.pop();
        assert_eq!(
            Rubric::with_anchors(base.criteria().to_vec(), missing).unwrap_err(),
            RubricError::AnchorsMismatch { index: 5 }
        );
        // four descriptors, or an empty one
        let mut short = base.anchors().clone();
        short.criteria[2].descriptors.pop();
        assert!(matches!(
            Rubric::with_anchors(base.criteria().to_vec(), short).unwrap_err(),
            RubricError::InvalidAnchorDescriptors(_)
        ));
        let mut blank = base.anchors().clone();
        blank.criteria[2].descriptors[1] = " ".into();
        assert!(matches!(
            Rubric::with_anchors(base.criteria().to_vec(), blank).unwrap_err(),
            RubricError::InvalidAnchorDescriptors(_)
        ));
        let mut no_note = base.anchors().clone();
        no_note.standard_setting_note = String::new();
        assert_eq!(
            Rubric::with_anchors(base.criteria().to_vec(), no_note).unwrap_err(),
            RubricError::EmptyStandardSettingNote
        );
    }

    #[test]
    fn machine_checkable_weight_is_reported_against_the_target() {
        let base = Rubric::baseline();
        // independent_execution 25 + remediation_verification 20
        assert_eq!(base.machine_checkable_weight(), 45);
        assert!(!base.meets_machine_checkable_target());
        let mut criteria = base.criteria().to_vec();
        criteria[3].machine_checkable = true; // detection_visibility 15 -> 60
        assert!(Rubric::new(criteria)
            .unwrap()
            .meets_machine_checkable_target());
    }

    #[test]
    fn rubric_errors_map_to_closed_categories() {
        let e: G9Error = RubricError::NoCriticalGate.into();
        assert_eq!(e.category, g9_domain::ErrorCategory::InvalidInput);
        let e: G9Error = RubricError::Serialization.into();
        assert_eq!(e.category, g9_domain::ErrorCategory::Internal);
    }
}
