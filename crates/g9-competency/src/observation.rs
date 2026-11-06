//! Dimension-specific competency observations (ARCHITECTURE.md §8
//! "Observations"; handoff §9 "Learning dimensions"; C10).
//!
//! Observations are append-only facts. A correction never edits a row: it is a
//! new observation whose `supersedes` points at the earlier one. Only evidence
//! with `TrustClass::Operator` or `TrustClass::SystemDeterministic` provenance
//! may affect competency state; model output is advisory only (C10, C21).

use crate::CompetencyError;
use g9_domain::{
    ActorId, AssistanceLevel, AttemptId, CompetencyId, Confidence, Difficulty, EpisodeId,
    InteractionMode, LearningDimension, ObservationId, Provenance, ReviewItemId, UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum score on an observation (percent).
pub const MAX_SCORE: u8 = 100;

/// Where the observed performance came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationSource {
    /// Graded assessment attempt (M07).
    Attempt(AttemptId),
    /// Learning episode inside a study/lab session (M04/M12).
    Episode(EpisodeId),
    /// Retention review attempt (M08).
    ReviewAttempt(ReviewItemId),
    /// Operator correction of an earlier observation; must set `supersedes`.
    ManualCorrection,
}

/// One observed performance on one competency in one learning dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyObservation {
    /// Observation identifier (unique, append-only).
    pub id: ObservationId,
    /// Competency the performance evidences.
    pub competency_id: CompetencyId,
    /// Which dimension was observed (recall / understanding / application / transfer).
    pub dimension: LearningDimension,
    /// Origin record.
    pub source: ObservationSource,
    /// Interaction mode in force when observed; only `Exam` can validate.
    pub mode: InteractionMode,
    /// Task difficulty on the competency's 1..=5 scale.
    pub difficulty: Difficulty,
    /// Assistance received; recorded, never erased (handoff §9).
    pub assistance: AssistanceLevel,
    /// Observed performance 0..=100.
    pub score: u8,
    /// Environment condition label (lab image, toolchain, network profile).
    pub environment: String,
    /// Human reviewer who finalized the underlying result, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<ActorId>,
    /// Where the observation came from and how far it may be trusted.
    pub provenance: Provenance,
    /// Stated measurement uncertainty: 0 = certain, 1 = no information.
    pub uncertainty: Confidence,
    /// When the performance was observed.
    pub observed_at: UtcMillis,
    /// Earlier observation of the same competency that this one corrects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<ObservationId>,
}

impl CompetencyObservation {
    /// Whether this observation is allowed to affect competency state (C10/C21):
    /// only operator or deterministic-system provenance is authoritative.
    pub fn is_authoritative(&self) -> bool {
        self.provenance.is_trusted_for_authority()
    }

    /// Whether this is an unaided performance (full independence weight).
    pub fn is_unaided(&self) -> bool {
        self.assistance == AssistanceLevel::Unaided
    }

    /// Whether this observation is blind-exam validation evidence: EXAM mode,
    /// unaided, in the Application or Transfer dimension.
    pub fn is_validation_evidence(&self) -> bool {
        self.mode == InteractionMode::Exam
            && self.is_unaided()
            && matches!(
                self.dimension,
                LearningDimension::Application | LearningDimension::Transfer
            )
    }

    /// Structural validation that needs no other observations: score range,
    /// trust class, self-reference, correction shape.
    pub fn validate(&self) -> Result<(), CompetencyError> {
        if self.score > MAX_SCORE {
            return Err(CompetencyError::InvalidObservation {
                reason: format!("score must be 0..={MAX_SCORE}, got {}", self.score),
            });
        }
        if !self.is_authoritative() {
            return Err(CompetencyError::UntrustedProvenance {
                trust: self.provenance.trust,
            });
        }
        if self.environment.trim().is_empty() {
            return Err(CompetencyError::InvalidObservation {
                reason: "environment label must not be empty".to_string(),
            });
        }
        if self.supersedes.as_ref() == Some(&self.id) {
            return Err(CompetencyError::InvalidObservation {
                reason: "an observation cannot supersede itself".to_string(),
            });
        }
        if self.source == ObservationSource::ManualCorrection && self.supersedes.is_none() {
            return Err(CompetencyError::InvalidObservation {
                reason: "a MANUAL_CORRECTION must name the observation it supersedes".to_string(),
            });
        }
        Ok(())
    }

    /// Full validation against the already-recorded observations of the same
    /// competency: structural rules plus the `supersedes` contract (target
    /// exists, same competency, not observed later than this correction, not
    /// already superseded, and this id is new).
    pub fn validate_against(
        &self,
        existing: &[CompetencyObservation],
    ) -> Result<(), CompetencyError> {
        self.validate()?;
        if existing.iter().any(|o| o.id == self.id) {
            return Err(CompetencyError::InvalidObservation {
                reason: format!("observation id {} already exists", self.id),
            });
        }
        if let Some(target_id) = &self.supersedes {
            let target = existing
                .iter()
                .find(|o| &o.id == target_id)
                .ok_or_else(|| CompetencyError::SupersedesUnknown {
                    id: target_id.to_string(),
                })?;
            if target.competency_id != self.competency_id {
                return Err(CompetencyError::SupersedesInvalid {
                    id: target_id.to_string(),
                    reason: "it belongs to a different competency".to_string(),
                });
            }
            if target.observed_at > self.observed_at {
                return Err(CompetencyError::SupersedesInvalid {
                    id: target_id.to_string(),
                    reason: "it was observed later than the correction".to_string(),
                });
            }
            if existing
                .iter()
                .any(|o| o.supersedes.as_ref() == Some(target_id))
            {
                return Err(CompetencyError::SupersedesInvalid {
                    id: target_id.to_string(),
                    reason: "it is already superseded; correct the newest record instead"
                        .to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Partition `observations` for one competency into the retained set used by
/// derivation. Dropped: observations of other competencies, non-authoritative
/// provenance, out-of-range scores, and every observation that some other
/// observation in the input supersedes. The result is sorted by
/// `(observed_at, id)` so that callers see a canonical order.
pub fn retained_for(
    competency_id: &CompetencyId,
    observations: &[CompetencyObservation],
) -> Vec<CompetencyObservation> {
    let superseded: BTreeMap<&ObservationId, ()> = observations
        .iter()
        .filter(|o| &o.competency_id == competency_id && o.is_authoritative())
        .filter_map(|o| o.supersedes.as_ref())
        .map(|id| (id, ()))
        .collect();
    let mut retained: Vec<CompetencyObservation> = observations
        .iter()
        .filter(|o| &o.competency_id == competency_id)
        .filter(|o| o.is_authoritative())
        .filter(|o| o.score <= MAX_SCORE)
        .filter(|o| !superseded.contains_key(&o.id))
        .cloned()
        .collect();
    retained.sort_by(|a, b| {
        a.observed_at
            .cmp(&b.observed_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    retained.dedup_by(|a, b| a.id == b.id);
    retained
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{day, Obs};
    use g9_domain::{ErrorCategory, G9Error, TrustClass};
    use pretty_assertions::assert_eq;

    #[test]
    fn a05_model_sourced_observation_rejected() {
        let model = Obs::app("o1", "c1", 95, day(0))
            .trust(TrustClass::ModelOutput)
            .build();
        let err = model.validate().unwrap_err();
        assert_eq!(
            err,
            CompetencyError::UntrustedProvenance {
                trust: TrustClass::ModelOutput
            }
        );
        let g9: G9Error = err.into();
        assert_eq!(g9.category, ErrorCategory::Unauthorized);
        assert_eq!(g9.code, "competency.observation.untrusted_provenance");
        assert!(g9.message.contains("OPERATOR or SYSTEM_DETERMINISTIC"));

        for t in [
            TrustClass::ApprovedReference,
            TrustClass::ImportedDocument,
            TrustClass::SkillPackage,
            TrustClass::Unknown,
        ] {
            assert!(
                Obs::app("o2", "c1", 95, day(0))
                    .trust(t)
                    .build()
                    .validate()
                    .is_err(),
                "{t:?}"
            );
        }
        assert!(Obs::app("o3", "c1", 95, day(0))
            .trust(TrustClass::Operator)
            .build()
            .validate()
            .is_ok());
        assert!(Obs::app("o4", "c1", 95, day(0)).build().validate().is_ok());

        // Defense in depth: the retained set used by derivation drops it too.
        let retained = retained_for(&model.competency_id, std::slice::from_ref(&model));
        assert!(retained.is_empty());
    }

    #[test]
    fn observation_structural_validation() {
        let mut bad = Obs::app("o1", "c1", 50, day(0)).build();
        bad.score = 101;
        assert!(matches!(
            bad.validate().unwrap_err(),
            CompetencyError::InvalidObservation { .. }
        ));

        let mut bad = Obs::app("o1", "c1", 50, day(0)).build();
        bad.supersedes = Some(bad.id.clone());
        assert!(bad.validate().is_err());

        let mut bad = Obs::app("o1", "c1", 50, day(0)).build();
        bad.source = ObservationSource::ManualCorrection;
        assert!(bad.validate().is_err());

        let mut bad = Obs::app("o1", "c1", 50, day(0)).build();
        bad.environment = "  ".to_string();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn supersedes_must_reference_existing_earlier_same_competency_observation() {
        let original = Obs::app("o1", "c1", 50, day(1)).build();
        let other = Obs::app("x1", "c2", 50, day(1)).build();
        let existing = vec![original.clone(), other];

        let ok = Obs::app("o2", "c1", 80, day(1)).supersedes("o1").build();
        assert!(ok.validate_against(&existing).is_ok());

        let unknown = Obs::app("o3", "c1", 80, day(2)).supersedes("ghost").build();
        let err = unknown.validate_against(&existing).unwrap_err();
        assert!(matches!(err, CompetencyError::SupersedesUnknown { .. }));
        assert_eq!(G9Error::from(err).category, ErrorCategory::NotFound);

        let wrong_competency = Obs::app("o4", "c1", 80, day(2)).supersedes("x1").build();
        assert!(matches!(
            wrong_competency.validate_against(&existing).unwrap_err(),
            CompetencyError::SupersedesInvalid { .. }
        ));

        let too_early = Obs::app("o5", "c1", 80, day(0)).supersedes("o1").build();
        assert!(matches!(
            too_early.validate_against(&existing).unwrap_err(),
            CompetencyError::SupersedesInvalid { .. }
        ));

        let duplicate_id = Obs::app("o1", "c1", 80, day(2)).build();
        assert!(duplicate_id.validate_against(&existing).is_err());

        let mut chain = existing.clone();
        chain.push(ok);
        let second_correction = Obs::app("o6", "c1", 90, day(3)).supersedes("o1").build();
        assert!(matches!(
            second_correction.validate_against(&chain).unwrap_err(),
            CompetencyError::SupersedesInvalid { .. }
        ));
    }

    #[test]
    fn observation_serde_round_trip() {
        let o = Obs::app("o1", "c1", 77, day(0))
            .mode(InteractionMode::Exam)
            .assistance(AssistanceLevel::Hinted)
            .supersedes("o0")
            .build();
        let json = serde_json::to_value(&o).unwrap();
        assert_eq!(json["dimension"], "APPLICATION");
        assert_eq!(json["source"]["kind"], "MANUAL_CORRECTION");
        assert_eq!(json["assistance"], "HINTED");
        let back: CompetencyObservation = serde_json::from_value(json).unwrap();
        assert_eq!(back, o);
    }
}
