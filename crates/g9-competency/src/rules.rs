//! Configurable evidence rules for state derivation (ARCHITECTURE.md §8
//! "Derivation (replayable fold)"; handoff §9: "Evidence thresholds/validation
//! periods are configurable per competency").

use crate::graph::Competency;
use crate::CompetencyError;
use g9_domain::hashing::sha256_json;
use serde::{Deserialize, Serialize};

/// Thresholds that drive the `CompetencyState` fold. Serializable so that a
/// projection can record exactly which rules produced it (`rules_hash`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DerivationRules {
    /// Minimum score of an assisted (prompted/hinted/guided) or unaided
    /// Application observation to reach `Guided`.
    pub guided_min_score: u8,
    /// Minimum score of an unaided Application observation to count toward `Independent`.
    pub independent_min_score: u8,
    /// Number of qualifying unaided Application observations required for `Independent`.
    pub independent_min_count: u8,
    /// Minimum task difficulty (1..=5 ordinal of `g9_domain::Difficulty`) for
    /// an observation to count toward `Independent`.
    pub independent_min_difficulty: u8,
    /// Qualifying observations must fall within this many days of each other.
    pub independent_window_days: u32,
    /// Minimum score of a blind-exam (EXAM mode, unaided, Application/Transfer)
    /// observation to count as validation evidence.
    pub validated_min_score: u8,
    /// `Regress` fires when this many most-recent unaided observations are all
    /// below `independent_min_score`.
    pub regress_window_count: u8,
    /// Override of the competency's validity window; `None` uses the competency's own.
    pub validity_window_days: Option<u32>,
}

impl Default for DerivationRules {
    fn default() -> Self {
        Self {
            guided_min_score: 60,
            independent_min_score: 70,
            independent_min_count: 2,
            independent_min_difficulty: 3,
            independent_window_days: 60,
            validated_min_score: 80,
            regress_window_count: 2,
            validity_window_days: None,
        }
    }
}

impl DerivationRules {
    /// Check internal consistency (score ranges, non-zero counts/windows,
    /// difficulty within the 1..=5 scale).
    pub fn validate(&self) -> Result<(), CompetencyError> {
        let invalid = |reason: String| CompetencyError::InvalidRules { reason };
        for (name, v) in [
            ("guided_min_score", self.guided_min_score),
            ("independent_min_score", self.independent_min_score),
            ("validated_min_score", self.validated_min_score),
        ] {
            if v > 100 {
                return Err(invalid(format!("{name} must be 0..=100, got {v}")));
            }
        }
        if self.independent_min_count == 0 {
            return Err(invalid(
                "independent_min_count must be at least 1".to_string(),
            ));
        }
        if !(1..=5).contains(&self.independent_min_difficulty) {
            return Err(invalid(format!(
                "independent_min_difficulty must be 1..=5, got {}",
                self.independent_min_difficulty
            )));
        }
        if self.independent_window_days == 0 {
            return Err(invalid(
                "independent_window_days must be at least 1".to_string(),
            ));
        }
        if self.regress_window_count == 0 {
            return Err(invalid(
                "regress_window_count must be at least 1".to_string(),
            ));
        }
        if self.validity_window_days == Some(0) {
            return Err(invalid(
                "validity_window_days override must be at least 1".to_string(),
            ));
        }
        Ok(())
    }

    /// Effective validity window: the override if set, else the competency's own.
    pub fn validity_window_days_for(&self, competency: &Competency) -> u32 {
        self.validity_window_days
            .unwrap_or(competency.validity_window_days)
    }

    /// Hex SHA-256 of the canonical JSON form of these rules; stored on every
    /// projection so that a replay can prove which rules it used.
    pub fn hash(&self) -> String {
        match serde_json::to_value(self) {
            Ok(v) => sha256_json(&v),
            // Serialization of a plain struct of integers cannot fail; the
            // fallback keeps the function total without panicking.
            Err(_) => String::from("unhashable-rules"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::competency;
    use pretty_assertions::assert_eq;

    #[test]
    fn defaults_match_architecture_and_hash_is_stable() {
        let r = DerivationRules::default();
        assert_eq!(r.guided_min_score, 60);
        assert_eq!(r.independent_min_score, 70);
        assert_eq!(r.independent_min_count, 2);
        assert_eq!(r.independent_min_difficulty, 3);
        assert_eq!(r.independent_window_days, 60);
        assert_eq!(r.validated_min_score, 80);
        assert_eq!(r.regress_window_count, 2);
        assert_eq!(r.validity_window_days, None);
        assert!(r.validate().is_ok());
        assert_eq!(r.hash(), DerivationRules::default().hash());
        assert_eq!(r.hash().len(), 64);
        let mut other = r.clone();
        other.validated_min_score = 85;
        assert_ne!(other.hash(), r.hash());

        let c = competency("c1");
        assert_eq!(r.validity_window_days_for(&c), 90);
        other.validity_window_days = Some(30);
        assert_eq!(other.validity_window_days_for(&c), 30);
    }

    #[test]
    fn rules_serde_round_trip_and_validation() {
        let r = DerivationRules::default();
        let json = serde_json::to_string(&r).unwrap();
        let back: DerivationRules = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
        let partial: DerivationRules =
            serde_json::from_str(r#"{"validated_min_score": 90}"#).unwrap();
        assert_eq!(partial.validated_min_score, 90);
        assert_eq!(partial.guided_min_score, 60);

        let mut bad = r.clone();
        bad.independent_min_difficulty = 6;
        assert!(bad.validate().is_err());
        let mut bad = r.clone();
        bad.guided_min_score = 101;
        assert!(bad.validate().is_err());
        let mut bad = r;
        bad.regress_window_count = 0;
        assert!(bad.validate().is_err());
    }
}
