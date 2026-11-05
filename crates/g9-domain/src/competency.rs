//! Evidence-centered competency vocabulary (handoff §9, C10). Competence is
//! tracked per learning dimension with difficulty, assistance, recency and
//! uncertainty — never as a single unqualified mastery float.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningDimension {
    /// Unaided retrieval of facts/procedures.
    Recall,
    /// Mechanistic explanation.
    Understanding,
    /// Independent task execution.
    Application,
    /// Solving an unfamiliar task/context.
    Transfer,
}

impl LearningDimension {
    pub const ALL: [LearningDimension; 4] = [
        LearningDimension::Recall,
        LearningDimension::Understanding,
        LearningDimension::Application,
        LearningDimension::Transfer,
    ];
}

/// Assistance received during an observed performance. Recorded, never erased;
/// it adjusts the independence weight of the evidence (handoff §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssistanceLevel {
    /// No hints, references restricted per mode.
    Unaided,
    /// A nudge/question that did not reveal method or answer.
    Prompted,
    /// A tiered hint revealed part of the method.
    Hinted,
    /// Guided step by step.
    Guided,
    /// Solution shown/demonstrated.
    Shown,
}

impl AssistanceLevel {
    /// Weight of the evidence toward *independent* capability, in [0, 1].
    /// Shown/guided performances remain legitimate learning evidence but carry
    /// no independence weight.
    pub const fn independence_weight(self) -> f64 {
        match self {
            AssistanceLevel::Unaided => 1.0,
            AssistanceLevel::Prompted => 0.75,
            AssistanceLevel::Hinted => 0.5,
            AssistanceLevel::Guided => 0.0,
            AssistanceLevel::Shown => 0.0,
        }
    }
}

/// Task difficulty on a 1..=5 ordinal scale (defined per competency pack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Difficulty(u8);

impl Difficulty {
    pub fn new(d: u8) -> Result<Self, String> {
        if (1..=5).contains(&d) {
            Ok(Self(d))
        } else {
            Err(format!("difficulty must be 1..=5, got {d}"))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Difficulty {
    type Error = String;
    fn try_from(d: u8) -> Result<Self, Self::Error> {
        Self::new(d)
    }
}

impl From<Difficulty> for u8 {
    fn from(d: Difficulty) -> u8 {
        d.0
    }
}

/// A probability-like confidence in [0, 1]. Used for model/skill confidence
/// limits and for learner calibration where the learner states a probability
/// (a 0–5 self-rating is NOT a probability and must be mapped explicitly).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Confidence(f64);

impl Confidence {
    pub fn new(p: f64) -> Result<Self, String> {
        if p.is_finite() && (0.0..=1.0).contains(&p) {
            Ok(Self(p))
        } else {
            Err(format!("confidence must be within [0,1], got {p}"))
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }

    /// Brier score contribution for a binary outcome.
    pub fn brier(self, outcome: bool) -> f64 {
        let o = if outcome { 1.0 } else { 0.0 };
        (self.0 - o).powi(2)
    }
}

impl TryFrom<f64> for Confidence {
    type Error = String;
    fn try_from(p: f64) -> Result<Self, Self::Error> {
        Self::new(p)
    }
}

impl From<Confidence> for f64 {
    fn from(c: Confidence) -> f64 {
        c.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_bounds_and_brier() {
        assert!(Confidence::new(1.01).is_err());
        assert!(Confidence::new(f64::NAN).is_err());
        let c = Confidence::new(0.8).unwrap();
        assert!((c.brier(true) - 0.04).abs() < 1e-12);
        assert!((c.brier(false) - 0.64).abs() < 1e-12);
    }

    #[test]
    fn assistance_weights_are_monotone() {
        let w: Vec<f64> = [
            AssistanceLevel::Unaided,
            AssistanceLevel::Prompted,
            AssistanceLevel::Hinted,
            AssistanceLevel::Guided,
            AssistanceLevel::Shown,
        ]
        .iter()
        .map(|a| a.independence_weight())
        .collect();
        assert!(w.windows(2).all(|p| p[0] >= p[1]));
    }
}
