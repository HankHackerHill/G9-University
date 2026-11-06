//! Rubrics (handoff §9 "Official assessment rubric (configurable baseline)";
//! gate B01).
//!
//! A [`Rubric`] is a validated list of weighted criteria. Invariants enforced by
//! [`Rubric::new`] / [`Rubric::validate`] and on deserialization:
//!
//! - weights sum to exactly [`REQUIRED_TOTAL_WEIGHT`] (100),
//! - criterion codes are unique and well-formed,
//! - at least one criterion is a critical gate (its failure fails the attempt
//!   regardless of the weighted total).
//!
//! Every criterion maps to one [`LearningDimension`] so that grade decisions
//! can be turned into dimension-specific competency observations.

use g9_domain::hashing::sha256_json;
use g9_domain::{BaselineRubric, G9Error, LearningDimension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Required sum of criterion weights.
pub const REQUIRED_TOTAL_WEIGHT: u32 = 100;

/// Maximum byte length of a criterion code.
pub const MAX_CODE_LEN: usize = 64;

/// Validation failure for a rubric.
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
    /// Whether failing this criterion fails the attempt regardless of total.
    pub critical_gate: bool,
    /// Learning dimension this criterion is evidence for.
    pub dimension: LearningDimension,
}

/// Unvalidated rubric shape; [`Rubric`] deserializes through it so that an
/// invalid rubric can never be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricSpec {
    /// Criteria in display order.
    pub criteria: Vec<RubricCriterionDef>,
}

/// A validated rubric. Construct with [`Rubric::new`] or [`Rubric::baseline`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RubricSpec")]
pub struct Rubric {
    criteria: Vec<RubricCriterionDef>,
}

impl TryFrom<RubricSpec> for Rubric {
    type Error = RubricError;

    fn try_from(spec: RubricSpec) -> Result<Self, Self::Error> {
        Rubric::new(spec.criteria)
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
    /// Validate and wrap a list of criteria.
    pub fn new(criteria: Vec<RubricCriterionDef>) -> Result<Self, RubricError> {
        let rubric = Self { criteria };
        rubric.validate()?;
        Ok(rubric)
    }

    /// Re-check every invariant (weights sum to 100, unique well-formed codes,
    /// non-empty titles, at least one critical gate).
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
        Ok(())
    }

    /// The configurable baseline rubric from `g9_domain::BaselineRubric` with the
    /// fixed criterion → dimension mapping of [`Rubric::baseline_dimension`].
    ///
    /// The baseline is known to satisfy every invariant (covered by the B01 test);
    /// it is built directly so that this constructor cannot fail.
    pub fn baseline() -> Self {
        let criteria = BaselineRubric::CRITERIA
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
        Self { criteria }
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

    /// Hex SHA-256 of the canonical JSON form of the rubric; used to pin the
    /// rubric inside an assessment version checksum.
    pub fn checksum(&self) -> Result<String, RubricError> {
        let value = serde_json::to_value(self).map_err(|_| RubricError::Serialization)?;
        Ok(sha256_json(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
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
    fn invalid_code_is_not_echoed() {
        let mut criteria = Rubric::baseline().criteria().to_vec();
        criteria[0].code = "has space".to_string();
        let err = Rubric::new(criteria).unwrap_err();
        assert_eq!(err, RubricError::InvalidCode { index: 0 });
        assert!(!err.to_string().contains("has space"));
    }
}
