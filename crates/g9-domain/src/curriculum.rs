//! Curriculum structural contract and invariants (handoff §8, Appendix A
//! "Importer proof obligations", gate A01).

use serde::{Deserialize, Serialize};

/// Fixed structural invariants of the G9-CT9-480 program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurriculumInvariants {
    pub weeks: u32,
    pub days_per_week: u32,
    pub objectives_per_day: u32,
    pub weekday_minutes: u32,
    pub saturday_minutes: u32,
    pub weekly_minutes: u32,
    pub total_minutes: u32,
    pub total_days: u32,
    pub total_objectives: u32,
}

pub const CURRICULUM_INVARIANTS: CurriculumInvariants = CurriculumInvariants {
    weeks: 12,
    days_per_week: 6,
    objectives_per_day: 3,
    weekday_minutes: 420,
    saturday_minutes: 300,
    weekly_minutes: 2_400,
    total_minutes: 28_800,
    total_days: 72,
    total_objectives: 216,
};

/// Weekday study blocks (minutes) — must sum to 420.
pub const WEEKDAY_BLOCKS: [(&str, u32); 6] = [
    ("recall", 15),
    ("theory", 45),
    ("guided_lab", 120),
    ("independent_lab", 120),
    ("defense_verification", 60),
    ("evidence_reporting", 60),
];

/// Saturday blocks (minutes) — must sum to 300.
pub const SATURDAY_BLOCKS: [(&str, u32); 4] = [
    ("blind_practical", 120),
    ("remediation", 60),
    ("report", 60),
    ("performance_review", 60),
];

/// Provenance status of a curriculum manifest. Only `ApprovedCanonical` may
/// drive official scheduling and assessment records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManifestProvenance {
    /// Appendix A seed or any unreconciled manifest.
    ProposedSeed,
    /// Reconciled against the original approved plan; diff produced; awaiting approval.
    ReconciledCandidate,
    /// Operator-approved authoritative manifest.
    ApprovedCanonical,
}

impl ManifestProvenance {
    pub const fn is_authoritative(self) -> bool {
        matches!(self, ManifestProvenance::ApprovedCanonical)
    }
}

/// Planned training day of week: 1 = Monday … 6 = Saturday. Sunday is unscheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct DayOfWeek(u8);

impl DayOfWeek {
    pub fn new(d: u8) -> Result<Self, InvariantViolation> {
        if (1..=6).contains(&d) {
            Ok(Self(d))
        } else {
            Err(InvariantViolation {
                code: "day_of_week.out_of_range",
                expected: "1..=6 (Mon..Sat)".to_string(),
                actual: d.to_string(),
            })
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn is_saturday(self) -> bool {
        self.0 == 6
    }

    pub const fn planned_minutes(self) -> u32 {
        if self.is_saturday() {
            CURRICULUM_INVARIANTS.saturday_minutes
        } else {
            CURRICULUM_INVARIANTS.weekday_minutes
        }
    }
}

impl TryFrom<u8> for DayOfWeek {
    type Error = InvariantViolation;
    fn try_from(d: u8) -> Result<Self, Self::Error> {
        Self::new(d)
    }
}

impl From<DayOfWeek> for u8 {
    fn from(d: DayOfWeek) -> u8 {
        d.0
    }
}

/// A violated invariant, reported (never silently corrected) by the importer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: expected {expected}, actual {actual}")]
pub struct InvariantViolation {
    pub code: &'static str,
    pub expected: String,
    pub actual: String,
}

/// Observed counts of a manifest, checked against [`CURRICULUM_INVARIANTS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CurriculumCounts {
    pub weeks: u32,
    pub days: u32,
    pub objectives: u32,
    pub total_minutes: u32,
    pub weekday_days: u32,
    pub saturday_days: u32,
}

impl CurriculumCounts {
    /// Every violated invariant, in a stable order. Empty means the structure holds.
    pub fn violations(&self) -> Vec<InvariantViolation> {
        let inv = CURRICULUM_INVARIANTS;
        let mut v = Vec::new();
        let mut check = |code: &'static str, expected: u32, actual: u32| {
            if expected != actual {
                v.push(InvariantViolation {
                    code,
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                });
            }
        };
        check("curriculum.weeks", inv.weeks, self.weeks);
        check("curriculum.days", inv.total_days, self.days);
        check("curriculum.objectives", inv.total_objectives, self.objectives);
        check("curriculum.total_minutes", inv.total_minutes, self.total_minutes);
        check("curriculum.weekday_days", inv.weeks * 5, self.weekday_days);
        check("curriculum.saturday_days", inv.weeks, self.saturday_days);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariants_are_arithmetically_consistent() {
        let i = CURRICULUM_INVARIANTS;
        assert_eq!(i.weeks * i.days_per_week, i.total_days);
        assert_eq!(i.total_days * i.objectives_per_day, i.total_objectives);
        assert_eq!(5 * i.weekday_minutes + i.saturday_minutes, i.weekly_minutes);
        assert_eq!(i.weekly_minutes * i.weeks, i.total_minutes);
        assert_eq!(i.total_minutes, 480 * 60);
        assert_eq!(WEEKDAY_BLOCKS.iter().map(|b| b.1).sum::<u32>(), i.weekday_minutes);
        assert_eq!(SATURDAY_BLOCKS.iter().map(|b| b.1).sum::<u32>(), i.saturday_minutes);
    }

    #[test]
    fn counts_detect_violations() {
        let ok = CurriculumCounts {
            weeks: 12,
            days: 72,
            objectives: 216,
            total_minutes: 28_800,
            weekday_days: 60,
            saturday_days: 12,
        };
        assert!(ok.violations().is_empty());
        let bad = CurriculumCounts { objectives: 72, ..ok };
        let v = bad.violations();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].code, "curriculum.objectives");
    }

    #[test]
    fn day_of_week_bounds() {
        assert!(DayOfWeek::new(0).is_err());
        assert!(DayOfWeek::new(7).is_err());
        assert_eq!(DayOfWeek::new(6).unwrap().planned_minutes(), 300);
        assert_eq!(DayOfWeek::new(1).unwrap().planned_minutes(), 420);
    }
}
