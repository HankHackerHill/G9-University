//! Official assessment rubric baseline and grade authority rules (handoff §9, C09).

use serde::{Deserialize, Serialize};

/// Baseline pass recommendation (percent). Mandatory critical integrity/scope
/// gates apply in addition and can fail an attempt regardless of score.
pub const PASS_THRESHOLD: u8 = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricCriterion {
    pub code: &'static str,
    pub title: &'static str,
    pub weight: u8,
    /// Whether a deterministic checker may score this criterion automatically.
    pub machine_checkable: bool,
    /// Whether failing this criterion fails the attempt regardless of total.
    pub critical_gate: bool,
}

/// The configurable baseline rubric. Weights sum to 100.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRubric;

impl BaselineRubric {
    pub const CRITERIA: [RubricCriterion; 6] = [
        RubricCriterion {
            code: "independent_execution",
            title: "Independent execution",
            weight: 25,
            machine_checkable: true,
            critical_gate: false,
        },
        RubricCriterion {
            code: "root_cause_reasoning",
            title: "Root-cause reasoning",
            weight: 20,
            machine_checkable: false,
            critical_gate: false,
        },
        RubricCriterion {
            code: "remediation_verification",
            title: "Remediation and verification",
            weight: 20,
            machine_checkable: true,
            critical_gate: false,
        },
        RubricCriterion {
            code: "detection_visibility",
            title: "Detection and visibility",
            weight: 15,
            machine_checkable: false,
            critical_gate: false,
        },
        RubricCriterion {
            code: "evidence_documentation",
            title: "Evidence/documentation",
            weight: 10,
            machine_checkable: false,
            critical_gate: false,
        },
        RubricCriterion {
            code: "scope_compliance_reporting",
            title: "Scope compliance and reporting",
            weight: 10,
            machine_checkable: false,
            critical_gate: true,
        },
    ];

    pub const fn total_weight() -> u32 {
        let mut i = 0;
        let mut sum = 0u32;
        while i < Self::CRITERIA.len() {
            sum += Self::CRITERIA[i].weight as u32;
            i += 1;
        }
        sum
    }
}

/// Who produced a criterion score. Only deterministic checkers and human
/// reviewers may contribute to a final grade; model scores are advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GradeSource {
    DeterministicChecker,
    HumanReviewer,
    ModelAdvisory,
}

impl GradeSource {
    pub const fn may_contribute_to_final(self) -> bool {
        matches!(
            self,
            GradeSource::DeterministicChecker | GradeSource::HumanReviewer
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubric_weights_sum_to_100() {
        assert_eq!(BaselineRubric::total_weight(), 100);
    }

    #[test]
    fn model_grades_are_never_final() {
        assert!(!GradeSource::ModelAdvisory.may_contribute_to_final());
        assert!(GradeSource::HumanReviewer.may_contribute_to_final());
    }
}
