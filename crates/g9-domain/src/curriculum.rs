//! Curriculum structural contract and invariants (handoff §8, Appendix A
//! "Importer proof obligations", gate A01), and the structure profiles the
//! application accepts (ADR-0023).
//!
//! Two shapes exist. [`CT9_480`] is the Appendix A shape (12 weeks of six days,
//! Saturday assessment); [`CURRICULUM_INVARIANTS`], [`WEEKDAY_BLOCKS`] and
//! [`SATURDAY_BLOCKS`] are its constants and keep their names. [`MASTERY_V2`] is
//! the owner's v2.0 plan (12 weeks of five days, Friday assessment). A manifest
//! declares its shape in `structure`; [`profile_for`] selects the profile and a
//! declaration that matches neither is refused by the validator.

use serde::{Deserialize, Serialize};

/// Fixed structural invariants of one structure profile. The field names
/// `weekday_minutes` and `saturday_minutes` are those of the manifest format:
/// they hold the minutes of an ordinary day and of the assessment day (a
/// Saturday in [`CT9_480`], a Friday in [`MASTERY_V2`]).
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

/// Invariants of the Appendix A shape ([`CT9_480`]).
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

/// Weekday study blocks of the Appendix A shape (minutes) — must sum to 420.
pub const WEEKDAY_BLOCKS: [(&str, u32); 6] = [
    ("recall", 15),
    ("theory", 45),
    ("guided_lab", 120),
    ("independent_lab", 120),
    ("defense_verification", 60),
    ("evidence_reporting", 60),
];

/// Saturday blocks of the Appendix A shape (minutes) — must sum to 300.
pub const SATURDAY_BLOCKS: [(&str, u32); 4] = [
    ("blind_practical", 120),
    ("remediation", 60),
    ("report", 60),
    ("performance_review", 60),
];

/// Identifier of a structure profile (ADR-0023). The wire spelling is the one
/// stored in `curriculum_versions.invariants_json` and returned by
/// `curriculum.program`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StructureProfileId {
    /// The Appendix A shape: 12 weeks of six days, Saturday assessment.
    #[serde(rename = "CT9_480")]
    Ct9480,
    /// The owner's v2.0 plan: 12 weeks of five days, Friday assessment.
    #[serde(rename = "MASTERY_V2")]
    MasteryV2,
}

impl StructureProfileId {
    /// Every profile id, in declaration order.
    pub const ALL: [StructureProfileId; 2] =
        [StructureProfileId::Ct9480, StructureProfileId::MasteryV2];

    /// Wire spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            StructureProfileId::Ct9480 => "CT9_480",
            StructureProfileId::MasteryV2 => "MASTERY_V2",
        }
    }

    /// Parse the wire spelling; `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == s)
    }

    /// The profile this id names.
    pub const fn profile(self) -> &'static StructureProfile {
        match self {
            StructureProfileId::Ct9480 => &CT9_480,
            StructureProfileId::MasteryV2 => &MASTERY_V2,
        }
    }
}

impl std::fmt::Display for StructureProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One block of a profile's day: its storage code, its planned minutes and the
/// name it is shown under when no lesson sheet names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProfileBlock {
    /// Block code (`study_blocks.block_code`).
    pub code: &'static str,
    /// Planned minutes.
    pub minutes: u32,
    /// Default label of the block on this day type.
    pub label: &'static str,
}

/// A pause of the day layout: `minutes` without a block after block `after_block`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DayPause {
    /// 1-based position of the block the pause follows.
    pub after_block: u32,
    /// Length of the pause in minutes.
    pub minutes: u32,
}

/// A curriculum shape the application accepts (ADR-0023): the arithmetic, the
/// block list of an ordinary day and of the assessment day, which day of the
/// week is assessed, and where the planned day pauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StructureProfile {
    /// Profile id.
    pub id: StructureProfileId,
    /// Arithmetic invariants (`days_per_week` is the number of training days of a week).
    pub invariants: CurriculumInvariants,
    /// The assessed day of a week: 6 (Saturday) or 5 (Friday). It is the last
    /// training day of the week in both profiles.
    pub assessment_day_of_week: u8,
    /// Blocks of an ordinary day, in order; they sum to `invariants.weekday_minutes`.
    pub ordinary_blocks: &'static [ProfileBlock],
    /// Blocks of the assessment day, in order; they sum to `invariants.saturday_minutes`.
    pub assessment_blocks: &'static [ProfileBlock],
    /// Pauses of the day layout, the same on both day types.
    pub pauses: &'static [DayPause],
}

/// The Appendix A shape. A label is the block code with underscores replaced by
/// spaces, which is what the application printed before profiles existed.
pub const CT9_480: StructureProfile = StructureProfile {
    id: StructureProfileId::Ct9480,
    invariants: CURRICULUM_INVARIANTS,
    assessment_day_of_week: 6,
    ordinary_blocks: &[
        ProfileBlock {
            code: WEEKDAY_BLOCKS[0].0,
            minutes: WEEKDAY_BLOCKS[0].1,
            label: "recall",
        },
        ProfileBlock {
            code: WEEKDAY_BLOCKS[1].0,
            minutes: WEEKDAY_BLOCKS[1].1,
            label: "theory",
        },
        ProfileBlock {
            code: WEEKDAY_BLOCKS[2].0,
            minutes: WEEKDAY_BLOCKS[2].1,
            label: "guided lab",
        },
        ProfileBlock {
            code: WEEKDAY_BLOCKS[3].0,
            minutes: WEEKDAY_BLOCKS[3].1,
            label: "independent lab",
        },
        ProfileBlock {
            code: WEEKDAY_BLOCKS[4].0,
            minutes: WEEKDAY_BLOCKS[4].1,
            label: "defense verification",
        },
        ProfileBlock {
            code: WEEKDAY_BLOCKS[5].0,
            minutes: WEEKDAY_BLOCKS[5].1,
            label: "evidence reporting",
        },
    ],
    assessment_blocks: &[
        ProfileBlock {
            code: SATURDAY_BLOCKS[0].0,
            minutes: SATURDAY_BLOCKS[0].1,
            label: "blind practical",
        },
        ProfileBlock {
            code: SATURDAY_BLOCKS[1].0,
            minutes: SATURDAY_BLOCKS[1].1,
            label: "remediation",
        },
        ProfileBlock {
            code: SATURDAY_BLOCKS[2].0,
            minutes: SATURDAY_BLOCKS[2].1,
            label: "report",
        },
        ProfileBlock {
            code: SATURDAY_BLOCKS[3].0,
            minutes: SATURDAY_BLOCKS[3].1,
            label: "performance review",
        },
    ],
    pauses: &[],
};

/// The owner's v2.0 plan (ADR-0023): 60 weekdays of 480 minutes in five blocks
/// (R 30, A 120, B 120, C 150, D 60), a scored Friday, and the plan's 08:00 to
/// 17:15 day (15 minutes after the second block, 60 after the third). The block
/// codes are existing storage codes; the label says what the block is in this plan.
pub const MASTERY_V2: StructureProfile = StructureProfile {
    id: StructureProfileId::MasteryV2,
    invariants: CurriculumInvariants {
        weeks: 12,
        days_per_week: 5,
        objectives_per_day: 3,
        weekday_minutes: 480,
        saturday_minutes: 480,
        weekly_minutes: 2_400,
        total_minutes: 28_800,
        total_days: 60,
        total_objectives: 180,
    },
    assessment_day_of_week: 5,
    ordinary_blocks: &[
        ProfileBlock {
            code: "recall",
            minutes: 30,
            label: "Active recall",
        },
        ProfileBlock {
            code: "theory",
            minutes: 120,
            label: "Core lesson",
        },
        ProfileBlock {
            code: "guided_lab",
            minutes: 120,
            label: "Tool practice",
        },
        ProfileBlock {
            code: "independent_lab",
            minutes: 150,
            label: "Independent challenge",
        },
        ProfileBlock {
            code: "defense_verification",
            minutes: 60,
            label: "Defense and documentation",
        },
    ],
    assessment_blocks: &[
        ProfileBlock {
            code: "recall",
            minutes: 30,
            label: "Active recall",
        },
        ProfileBlock {
            code: "theory",
            minutes: 120,
            label: "Knowledge exam",
        },
        ProfileBlock {
            code: "performance_review",
            minutes: 120,
            label: "Review and planning",
        },
        ProfileBlock {
            code: "blind_practical",
            minutes: 150,
            label: "Scored practical",
        },
        ProfileBlock {
            code: "report",
            minutes: 60,
            label: "Scoring and report",
        },
    ],
    pauses: &[
        DayPause {
            after_block: 2,
            minutes: 15,
        },
        DayPause {
            after_block: 3,
            minutes: 60,
        },
    ],
};

/// Every structure profile, in the order of [`StructureProfileId::ALL`].
pub const STRUCTURE_PROFILES: [&StructureProfile; 2] = [&CT9_480, &MASTERY_V2];

/// The assessment day of a manifest that does not declare one (format 1.0.0).
pub const DEFAULT_ASSESSMENT_DAY_OF_WEEK: u8 = 6;

/// The profile a manifest's `structure` declares: the one with this number of
/// training days a week and this assessment day (`None` means
/// [`DEFAULT_ASSESSMENT_DAY_OF_WEEK`]). `None` when no profile has both.
pub fn profile_for(
    days_per_week: u32,
    assessment_day_of_week: Option<u8>,
) -> Option<&'static StructureProfile> {
    let assessment = assessment_day_of_week.unwrap_or(DEFAULT_ASSESSMENT_DAY_OF_WEEK);
    STRUCTURE_PROFILES.into_iter().find(|p| {
        p.invariants.days_per_week == days_per_week && p.assessment_day_of_week == assessment
    })
}

impl StructureProfile {
    /// Training days of a week (6 or 5).
    pub const fn days_per_week(&self) -> u32 {
        self.invariants.days_per_week
    }

    /// Whether `day_of_week` (1 = Monday) is a training day of this profile.
    pub const fn is_training_day(&self, day_of_week: u8) -> bool {
        day_of_week >= 1 && (day_of_week as u32) <= self.invariants.days_per_week
    }

    /// Whether `day_of_week` is the assessed day of the week.
    pub const fn is_assessment_day(&self, day_of_week: u8) -> bool {
        day_of_week == self.assessment_day_of_week
    }

    /// Planned minutes of a training day by its day of week.
    pub const fn planned_minutes(&self, day_of_week: u8) -> u32 {
        if self.is_assessment_day(day_of_week) {
            self.invariants.saturday_minutes
        } else {
            self.invariants.weekday_minutes
        }
    }

    /// The canonical block list of a day type.
    pub const fn blocks(&self, assessment_day: bool) -> &'static [ProfileBlock] {
        if assessment_day {
            self.assessment_blocks
        } else {
            self.ordinary_blocks
        }
    }

    /// The canonical block list of a training day by its day of week.
    pub const fn day_blocks(&self, day_of_week: u8) -> &'static [ProfileBlock] {
        self.blocks(self.is_assessment_day(day_of_week))
    }

    /// Ordinary (not assessed) training days of the program.
    pub const fn ordinary_days(&self) -> u32 {
        self.invariants.weeks * self.invariants.days_per_week.saturating_sub(1)
    }

    /// Assessment days of the program (one a week).
    pub const fn assessment_days(&self) -> u32 {
        self.invariants.weeks
    }

    /// The profile's label of a block code on a day type; `None` when the day
    /// type has no such block.
    pub fn label(&self, block_code: &str, assessment_day: bool) -> Option<&'static str> {
        self.blocks(assessment_day)
            .iter()
            .find(|b| b.code == block_code)
            .map(|b| b.label)
    }

    /// [`Self::label`], or the block code with underscores replaced by spaces
    /// for a code the day type does not list (a stored block that predates the
    /// profile, or a fixture).
    pub fn label_or_code(&self, block_code: &str, assessment_day: bool) -> String {
        match self.label(block_code, assessment_day) {
            Some(label) => label.to_string(),
            None => block_code.replace('_', " "),
        }
    }

    /// Minutes of pause after the block at 1-based position `block_seq` (0 when none).
    pub fn pause_after(&self, block_seq: u32) -> u32 {
        self.pauses
            .iter()
            .filter(|p| p.after_block == block_seq)
            .map(|p| p.minutes)
            .sum()
    }

    /// Every invariant of this profile that `counts` violates, in a stable
    /// order. Empty means the structure holds.
    pub fn violations(&self, counts: &CurriculumCounts) -> Vec<InvariantViolation> {
        let inv = self.invariants;
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
        check("curriculum.weeks", inv.weeks, counts.weeks);
        check("curriculum.days", inv.total_days, counts.days);
        check(
            "curriculum.objectives",
            inv.total_objectives,
            counts.objectives,
        );
        check(
            "curriculum.total_minutes",
            inv.total_minutes,
            counts.total_minutes,
        );
        check(
            "curriculum.weekday_days",
            self.ordinary_days(),
            counts.weekday_days,
        );
        check(
            "curriculum.saturday_days",
            self.assessment_days(),
            counts.saturday_days,
        );
        v
    }
}

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
///
/// The type admits every day a profile can train on. Which of them a given
/// curriculum uses, which one is assessed and how long a day lasts depend on
/// its [`StructureProfile`]; [`DayOfWeek::is_saturday`] and
/// [`DayOfWeek::planned_minutes`] answer for the Appendix A shape only.
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

    /// Planned minutes of this day in the Appendix A shape ([`CT9_480`]); use
    /// [`StructureProfile::planned_minutes`] for a version of another profile.
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

/// Observed counts of a manifest, checked against the invariants of its
/// profile ([`StructureProfile::violations`]). `weekday_days` counts ordinary
/// days and `saturday_days` assessment days (Saturdays in [`CT9_480`], Fridays
/// in [`MASTERY_V2`]); the names are those of the stored validation reports.
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
    /// Every invariant of the Appendix A shape ([`CT9_480`]) that these counts
    /// violate, in a stable order. Empty means the structure holds.
    pub fn violations(&self) -> Vec<InvariantViolation> {
        CT9_480.violations(self)
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
        assert_eq!(
            WEEKDAY_BLOCKS.iter().map(|b| b.1).sum::<u32>(),
            i.weekday_minutes
        );
        assert_eq!(
            SATURDAY_BLOCKS.iter().map(|b| b.1).sum::<u32>(),
            i.saturday_minutes
        );
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
        let bad = CurriculumCounts {
            objectives: 72,
            ..ok
        };
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

    #[test]
    fn every_profile_is_arithmetically_consistent() {
        for p in STRUCTURE_PROFILES {
            let i = p.invariants;
            let id = p.id;
            assert_eq!(i.weeks * i.days_per_week, i.total_days, "{id}");
            assert_eq!(i.total_days * i.objectives_per_day, i.total_objectives);
            assert_eq!(
                (i.days_per_week - 1) * i.weekday_minutes + i.saturday_minutes,
                i.weekly_minutes,
                "{id}"
            );
            assert_eq!(i.weekly_minutes * i.weeks, i.total_minutes, "{id}");
            assert_eq!(i.total_minutes, 480 * 60, "{id}: 480 hours");
            assert_eq!(
                p.ordinary_blocks.iter().map(|b| b.minutes).sum::<u32>(),
                i.weekday_minutes,
                "{id}"
            );
            assert_eq!(
                p.assessment_blocks.iter().map(|b| b.minutes).sum::<u32>(),
                i.saturday_minutes,
                "{id}"
            );
            // The assessed day is the last training day of the week.
            assert_eq!(u32::from(p.assessment_day_of_week), i.days_per_week, "{id}");
            assert_eq!(p.ordinary_days() + p.assessment_days(), i.total_days);
            assert!(p.is_training_day(1) && p.is_training_day(p.assessment_day_of_week));
            assert!(!p.is_training_day(0) && !p.is_training_day(7));
            assert!(!p.is_training_day(p.assessment_day_of_week + 1));
            // A pause follows a block that exists and is not the last one.
            for pause in p.pauses {
                assert!(pause.minutes > 0, "{id}");
                for blocks in [p.ordinary_blocks, p.assessment_blocks] {
                    assert!(
                        pause.after_block >= 1 && (pause.after_block as usize) < blocks.len(),
                        "{id}"
                    );
                }
            }
            // Labels are non-empty and block codes are not repeated within a day.
            for blocks in [p.ordinary_blocks, p.assessment_blocks] {
                let mut codes: Vec<&str> = blocks.iter().map(|b| b.code).collect();
                assert!(blocks.iter().all(|b| !b.label.trim().is_empty()), "{id}");
                codes.sort_unstable();
                codes.dedup();
                assert_eq!(codes.len(), blocks.len(), "{id}");
            }
            assert_eq!(StructureProfileId::parse(id.as_str()), Some(id));
            assert_eq!(id.profile(), p);
        }
        assert_eq!(StructureProfileId::parse("ct9_480"), None);
        assert_eq!(StructureProfileId::parse(""), None);
        assert_eq!(
            STRUCTURE_PROFILES.map(|p| p.id),
            StructureProfileId::ALL,
            "the table and the id list name the same profiles in the same order"
        );
    }

    #[test]
    fn the_appendix_a_profile_is_the_existing_constants() {
        assert_eq!(CT9_480.invariants, CURRICULUM_INVARIANTS);
        let pairs = |blocks: &[ProfileBlock]| -> Vec<(&'static str, u32)> {
            blocks.iter().map(|b| (b.code, b.minutes)).collect()
        };
        assert_eq!(pairs(CT9_480.ordinary_blocks), WEEKDAY_BLOCKS.to_vec());
        assert_eq!(pairs(CT9_480.assessment_blocks), SATURDAY_BLOCKS.to_vec());
        assert_eq!(CT9_480.assessment_day_of_week, 6);
        assert!(CT9_480.pauses.is_empty());
        // The label is what the application printed before profiles existed.
        for blocks in [CT9_480.ordinary_blocks, CT9_480.assessment_blocks] {
            for b in blocks {
                assert_eq!(b.label, b.code.replace('_', " "), "{}", b.code);
            }
        }
        // Day arithmetic agrees with `DayOfWeek`.
        for d in 1..=6u8 {
            let dow = DayOfWeek::new(d).unwrap();
            assert_eq!(CT9_480.planned_minutes(d), dow.planned_minutes());
            assert_eq!(CT9_480.is_assessment_day(d), dow.is_saturday());
        }
        // `CurriculumCounts::violations` is the Appendix A check.
        let bad = CurriculumCounts {
            weeks: 12,
            days: 60,
            objectives: 180,
            total_minutes: 28_800,
            weekday_days: 48,
            saturday_days: 12,
        };
        assert_eq!(bad.violations(), CT9_480.violations(&bad));
        assert_eq!(bad.violations().len(), 3);
        assert!(MASTERY_V2.violations(&bad).is_empty());
    }

    #[test]
    fn the_v2_profile_is_the_adr_table() {
        let p = &MASTERY_V2;
        let i = p.invariants;
        assert_eq!((i.weeks, i.days_per_week, i.total_days), (12, 5, 60));
        assert_eq!((i.objectives_per_day, i.total_objectives), (3, 180));
        assert_eq!((i.weekday_minutes, i.saturday_minutes), (480, 480));
        assert_eq!((i.weekly_minutes, i.total_minutes), (2_400, 28_800));
        assert_eq!(p.assessment_day_of_week, 5);
        let pairs = |blocks: &[ProfileBlock]| -> Vec<(&'static str, u32, &'static str)> {
            blocks
                .iter()
                .map(|b| (b.code, b.minutes, b.label))
                .collect()
        };
        assert_eq!(
            pairs(p.ordinary_blocks),
            vec![
                ("recall", 30, "Active recall"),
                ("theory", 120, "Core lesson"),
                ("guided_lab", 120, "Tool practice"),
                ("independent_lab", 150, "Independent challenge"),
                ("defense_verification", 60, "Defense and documentation"),
            ]
        );
        assert_eq!(
            pairs(p.assessment_blocks),
            vec![
                ("recall", 30, "Active recall"),
                ("theory", 120, "Knowledge exam"),
                ("performance_review", 120, "Review and planning"),
                ("blind_practical", 150, "Scored practical"),
                ("report", 60, "Scoring and report"),
            ]
        );
        assert_eq!(p.day_blocks(1), p.ordinary_blocks);
        assert_eq!(p.day_blocks(5), p.assessment_blocks);
        assert_eq!(p.planned_minutes(4), 480);
        assert!(p.is_assessment_day(5) && !p.is_assessment_day(4));
        assert!(
            !p.is_training_day(6),
            "a Saturday is a rest day in the v2 plan"
        );

        // The same code has another name on the scored Friday.
        assert_eq!(p.label("theory", false), Some("Core lesson"));
        assert_eq!(p.label("theory", true), Some("Knowledge exam"));
        assert_eq!(p.label("guided_lab", true), None);
        assert_eq!(p.label_or_code("guided_lab", true), "guided lab");
        assert_eq!(p.label_or_code("blind_practical", true), "Scored practical");
        assert_eq!(CT9_480.label_or_code("guided_lab", false), "guided lab");

        // The plan's day: 15 minutes after the second block, 60 after the third.
        let pauses: Vec<u32> = (1..=5).map(|seq| p.pause_after(seq)).collect();
        assert_eq!(pauses, vec![0, 15, 60, 0, 0]);
        let day: u32 =
            p.ordinary_blocks.iter().map(|b| b.minutes).sum::<u32>() + pauses.iter().sum::<u32>();
        assert_eq!(day, 9 * 60 + 15, "08:00 to 17:15");
        assert_eq!((1..=6).map(|s| CT9_480.pause_after(s)).sum::<u32>(), 0);

        // Counts are checked against this profile's numbers.
        let ok = CurriculumCounts {
            weeks: 12,
            days: 60,
            objectives: 180,
            total_minutes: 28_800,
            weekday_days: 48,
            saturday_days: 12,
        };
        assert!(p.violations(&ok).is_empty());
        let codes: Vec<&str> = p
            .violations(&CurriculumCounts {
                days: 72,
                weekday_days: 60,
                ..ok
            })
            .iter()
            .map(|v| v.code)
            .collect();
        assert_eq!(codes, vec!["curriculum.days", "curriculum.weekday_days"]);
    }

    #[test]
    fn a_declared_structure_selects_one_profile_or_none() {
        // Absent means 6: every 1.0.0 manifest is the Appendix A shape.
        assert_eq!(
            profile_for(6, None).map(|p| p.id),
            Some(StructureProfileId::Ct9480)
        );
        assert_eq!(
            profile_for(6, Some(6)).map(|p| p.id),
            Some(StructureProfileId::Ct9480)
        );
        assert_eq!(
            profile_for(5, Some(5)).map(|p| p.id),
            Some(StructureProfileId::MasteryV2)
        );
        for (days, assessment) in [
            (5, None),
            (5, Some(4)),
            (5, Some(6)),
            (6, Some(5)),
            (7, Some(7)),
            (0, None),
            (4, Some(4)),
        ] {
            assert!(
                profile_for(days, assessment).is_none(),
                "{days} {assessment:?}"
            );
        }
    }

    #[test]
    fn profile_ids_serialize_as_their_wire_spelling() {
        assert_eq!(
            serde_json::to_value(StructureProfileId::ALL).unwrap(),
            serde_json::json!(["CT9_480", "MASTERY_V2"])
        );
        let back: StructureProfileId = serde_json::from_str("\"MASTERY_V2\"").unwrap();
        assert_eq!(back, StructureProfileId::MasteryV2);
        assert!(serde_json::from_str::<StructureProfileId>("\"MasteryV2\"").is_err());
        let json = serde_json::to_value(MASTERY_V2).unwrap();
        assert_eq!(json["id"], "MASTERY_V2");
        assert_eq!(json["invariants"]["total_days"], 60);
        assert_eq!(json["pauses"][1]["minutes"], 60);
    }
}
