//! Assessment definitions and immutable versions (ARCHITECTURE.md §8 "Sealed
//! keys"; handoff §9 "Exam integrity").
//!
//! An [`AssessmentDefinition`] names what is assessed and for which
//! competencies. An [`AssessmentVersion`] pins the rubric, task, difficulty,
//! equivalence group and the sealed answer-key reference, and carries a content
//! checksum so that tampering with a stored version is detectable
//! (`DATA_CORRUPTION`). Versions are immutable; a changed task or rubric is a new
//! version number.

use crate::rubric::{Rubric, RubricError};
use g9_domain::hashing::sha256_json;
use g9_domain::{
    AssessmentId, AttachmentId, Classification, CompetencyId, DataDomain, Difficulty, EvidenceId,
    G9Error, InteractionMode, UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Kind of assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssessmentKind {
    /// Blind practical task on an unseen holdout.
    BlindPractical,
    /// Weekly official assessment.
    WeeklyAssessment,
    /// Diagnostic that measures baseline/calibration; not a blind holdout.
    DiagnosticCalibration,
    /// Transfer check in an unfamiliar context (competency validity window).
    TransferCheck,
    /// Integrated capstone.
    Capstone,
}

impl AssessmentKind {
    /// Every kind.
    pub const ALL: [AssessmentKind; 5] = [
        AssessmentKind::BlindPractical,
        AssessmentKind::WeeklyAssessment,
        AssessmentKind::DiagnosticCalibration,
        AssessmentKind::TransferCheck,
        AssessmentKind::Capstone,
    ];

    /// Official assessments are blind holdout tests: they must run in
    /// `InteractionMode::Exam` and are the only source of *validated* competency
    /// evidence. A diagnostic calibration measures baseline and confidence
    /// calibration and may run outside EXAM mode (INFERRED from handoff §3
    /// "baseline diagnostic" vs "blind holdout tests").
    pub const fn is_official(self) -> bool {
        !matches!(self, AssessmentKind::DiagnosticCalibration)
    }

    /// Interaction mode an attempt of this kind must run in, if constrained.
    pub const fn required_mode(self) -> Option<InteractionMode> {
        if self.is_official() {
            Some(InteractionMode::Exam)
        } else {
            None
        }
    }
}

/// Validation or integrity failure for a definition or version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    /// The definition title is empty.
    #[error("assessment title is empty")]
    EmptyTitle,
    /// The definition lists no competencies.
    #[error("assessment lists no competencies")]
    NoCompetencies,
    /// A competency is listed twice.
    #[error("competency {0} is listed twice")]
    DuplicateCompetency(CompetencyId),
    /// Version numbers start at 1.
    #[error("assessment version numbers start at 1")]
    VersionZero,
    /// The equivalence group is empty.
    #[error("equivalence group is empty")]
    EmptyEquivalenceGroup,
    /// The stored checksum does not match the version content.
    #[error("stored checksum of assessment {definition_id} v{version} does not match its content")]
    ChecksumMismatch {
        /// Definition the version belongs to.
        definition_id: AssessmentId,
        /// Version number.
        version: u32,
    },
    /// The version could not be serialized for hashing.
    #[error("assessment version could not be serialized for hashing")]
    Serialization,
    /// The embedded rubric is invalid.
    #[error(transparent)]
    Rubric(#[from] RubricError),
}

impl DefinitionError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            DefinitionError::EmptyTitle => "assessment.definition.empty_title",
            DefinitionError::NoCompetencies => "assessment.definition.no_competencies",
            DefinitionError::DuplicateCompetency(_) => "assessment.definition.duplicate_competency",
            DefinitionError::VersionZero => "assessment.version.zero",
            DefinitionError::EmptyEquivalenceGroup => "assessment.version.empty_equivalence_group",
            DefinitionError::ChecksumMismatch { .. } => "assessment.version.checksum_mismatch",
            DefinitionError::Serialization => "assessment.version.serialization",
            DefinitionError::Rubric(e) => e.code(),
        }
    }
}

impl From<DefinitionError> for G9Error {
    fn from(e: DefinitionError) -> Self {
        let code = e.code();
        match e {
            DefinitionError::Rubric(inner) => inner.into(),
            DefinitionError::Serialization => G9Error::internal_redacted(code),
            DefinitionError::ChecksumMismatch {
                ref definition_id,
                version,
            } => G9Error::data_corruption(code, e.to_string()).with_details(serde_json::json!({
                "definition_id": definition_id.as_str(),
                "version": version,
            })),
            other => G9Error::invalid_input(code, other.to_string()),
        }
    }
}

/// What is assessed and for which competencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssessmentDefinition {
    /// Stable identifier.
    pub id: AssessmentId,
    /// Human-readable title.
    pub title: String,
    /// Competencies this assessment produces evidence for (non-empty, unique).
    pub competency_ids: Vec<CompetencyId>,
    /// Kind of assessment.
    pub kind: AssessmentKind,
    /// Privacy domain of the definition and of every attempt made against it.
    pub data_domain: DataDomain,
}

impl AssessmentDefinition {
    /// Validate and build a definition.
    pub fn new(
        id: AssessmentId,
        title: impl Into<String>,
        competency_ids: Vec<CompetencyId>,
        kind: AssessmentKind,
        data_domain: DataDomain,
    ) -> Result<Self, DefinitionError> {
        let def = Self {
            id,
            title: title.into(),
            competency_ids,
            kind,
            data_domain,
        };
        def.validate()?;
        Ok(def)
    }

    /// Re-check invariants: non-empty title, at least one competency, no
    /// duplicate competencies.
    pub fn validate(&self) -> Result<(), DefinitionError> {
        if self.title.trim().is_empty() {
            return Err(DefinitionError::EmptyTitle);
        }
        if self.competency_ids.is_empty() {
            return Err(DefinitionError::NoCompetencies);
        }
        let mut seen = BTreeSet::new();
        for c in &self.competency_ids {
            if !seen.insert(c) {
                return Err(DefinitionError::DuplicateCompetency(c.clone()));
            }
        }
        Ok(())
    }
}

/// Immutable version of an assessment: rubric, task, sealed key, difficulty.
///
/// Build it with the literal struct syntax and an empty `checksum`, then call
/// [`AssessmentVersion::with_checksum`]; load from storage and call
/// [`AssessmentVersion::verify_checksum`] before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssessmentVersion {
    /// Definition this version belongs to.
    pub definition_id: AssessmentId,
    /// Version number, starting at 1.
    pub version: u32,
    /// Rubric used to grade attempts of this version.
    pub rubric: Rubric,
    /// Evidence item holding the task statement shown to the learner.
    pub task_ref: EvidenceId,
    /// Content-addressed attachment holding the answer key / reference solution.
    /// Its classification is always [`AssessmentVersion::SEALED_KEY_CLASSIFICATION`].
    pub sealed_key_ref: Option<AttachmentId>,
    /// Task difficulty (1..=5).
    pub difficulty: Difficulty,
    /// Difficulty-equivalence group; retakes draw from the same group.
    pub equivalence_group: String,
    /// When this version was created.
    pub created_at: UtcMillis,
    /// Hex SHA-256 over every other field (canonical JSON).
    pub checksum: String,
}

/// Hashable content of a version (everything except the checksum itself).
#[derive(Serialize)]
struct VersionContent<'a> {
    definition_id: &'a AssessmentId,
    version: u32,
    rubric: &'a Rubric,
    task_ref: &'a EvidenceId,
    sealed_key_ref: &'a Option<AttachmentId>,
    difficulty: Difficulty,
    equivalence_group: &'a str,
    created_at: UtcMillis,
}

impl AssessmentVersion {
    /// Classification of sealed answer keys and reference solutions. Content at
    /// this level never enters model context (see `integrity::ExamContextPolicy`).
    pub const SEALED_KEY_CLASSIFICATION: Classification = Classification::Secret;

    /// Validate the version and fill in its checksum.
    pub fn with_checksum(mut self) -> Result<Self, DefinitionError> {
        self.validate()?;
        self.checksum = self.compute_checksum()?;
        Ok(self)
    }

    /// Re-check invariants: version ≥ 1, non-empty equivalence group, valid rubric.
    pub fn validate(&self) -> Result<(), DefinitionError> {
        if self.version == 0 {
            return Err(DefinitionError::VersionZero);
        }
        if self.equivalence_group.trim().is_empty() {
            return Err(DefinitionError::EmptyEquivalenceGroup);
        }
        self.rubric.validate()?;
        Ok(())
    }

    /// Compute the checksum over the version content (excluding `checksum`).
    pub fn compute_checksum(&self) -> Result<String, DefinitionError> {
        let content = VersionContent {
            definition_id: &self.definition_id,
            version: self.version,
            rubric: &self.rubric,
            task_ref: &self.task_ref,
            sealed_key_ref: &self.sealed_key_ref,
            difficulty: self.difficulty,
            equivalence_group: &self.equivalence_group,
            created_at: self.created_at,
        };
        let value = serde_json::to_value(&content).map_err(|_| DefinitionError::Serialization)?;
        Ok(sha256_json(&value))
    }

    /// Verify the stored checksum against the content.
    pub fn verify_checksum(&self) -> Result<(), DefinitionError> {
        if self.compute_checksum()? == self.checksum {
            Ok(())
        } else {
            Err(DefinitionError::ChecksumMismatch {
                definition_id: self.definition_id.clone(),
                version: self.version,
            })
        }
    }

    /// Whether this version has a sealed answer key.
    pub fn has_sealed_key(&self) -> bool {
        self.sealed_key_ref.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ErrorCategory;

    fn def(
        competencies: Vec<CompetencyId>,
        title: &str,
    ) -> Result<AssessmentDefinition, DefinitionError> {
        AssessmentDefinition::new(
            AssessmentId::parse("ad-1").unwrap(),
            title,
            competencies,
            AssessmentKind::BlindPractical,
            DataDomain::Learning,
        )
    }

    fn version(n: u32, group: &str) -> AssessmentVersion {
        AssessmentVersion {
            definition_id: AssessmentId::parse("ad-1").unwrap(),
            version: n,
            rubric: Rubric::baseline(),
            task_ref: EvidenceId::parse("evi-task").unwrap(),
            sealed_key_ref: None,
            difficulty: Difficulty::new(3).unwrap(),
            equivalence_group: group.into(),
            created_at: UtcMillis::from_millis(0),
            checksum: String::new(),
        }
    }

    #[test]
    fn definition_and_version_invariants() {
        let c = CompetencyId::parse("comp-1").unwrap();
        assert!(def(vec![c.clone()], "Week 1").is_ok());
        assert_eq!(
            def(vec![c.clone()], " ").unwrap_err(),
            DefinitionError::EmptyTitle
        );
        assert_eq!(
            def(vec![], "Week 1").unwrap_err(),
            DefinitionError::NoCompetencies
        );
        assert!(matches!(
            def(vec![c.clone(), c], "Week 1").unwrap_err(),
            DefinitionError::DuplicateCompetency(_)
        ));
        assert_eq!(
            version(0, "g").with_checksum().unwrap_err(),
            DefinitionError::VersionZero
        );
        assert_eq!(
            version(1, " ").with_checksum().unwrap_err(),
            DefinitionError::EmptyEquivalenceGroup
        );
        let v = version(1, "g").with_checksum().unwrap();
        assert_eq!(v.checksum.len(), 64);
        assert!(v.verify_checksum().is_ok());
        assert!(!v.has_sealed_key());
        let mut tampered = v.clone();
        tampered.difficulty = Difficulty::new(4).unwrap();
        let err = tampered.verify_checksum().unwrap_err();
        assert_eq!(G9Error::from(err).category, ErrorCategory::DataCorruption);
        assert_eq!(
            AssessmentVersion::SEALED_KEY_CLASSIFICATION,
            Classification::Secret
        );
    }

    #[test]
    fn official_kinds_require_exam_mode() {
        for kind in AssessmentKind::ALL {
            let expected =
                (kind != AssessmentKind::DiagnosticCalibration).then_some(InteractionMode::Exam);
            assert_eq!(kind.required_mode(), expected, "{kind:?}");
            assert_eq!(kind.is_official(), expected.is_some());
        }
    }
}
