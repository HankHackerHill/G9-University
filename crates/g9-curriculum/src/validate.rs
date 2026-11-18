//! Invariant validator (gate A01; handoff §8 "Structural contract" and Appendix A
//! "Importer proof obligations"; corrections C01/C02).
//!
//! [`validate`] never corrects a manifest: every deviation is reported with a
//! stable code (see [`codes`]), a JSON-pointer path (RFC 6901) into the manifest
//! and a short message. `errors` and `violations` block import; `warnings` do not.
//!
//! The counts that feed the arithmetic invariants are computed here and checked
//! with `g9_domain::StructureProfile::violations`, so the 12 × 6 × 3 / 28,800
//! contract (and the 12 × 5 × 3 / 28,800 contract of the second profile) has a
//! single source of truth in `g9-domain`.
//!
//! Structure profiles (ADR-0023). A manifest declares its shape in `structure`:
//! `days_per_week` and, in format 1.1.0, `assessment_day_of_week` (absent means
//! 6). [`select_profile`] maps the declaration to a `g9_domain` profile and
//! every structural check below uses that profile. A declaration that matches
//! no profile is `structure.mismatch`; the remaining checks then run against
//! the nearest profile so that the report stays readable:
//!
//! | Declaration | Profile the checks use |
//! |---|---|
//! | no `assessment_day_of_week` | `CT9_480` (every 1.0.0 manifest, checked exactly as before) |
//! | the field equals a profile's assessment day | that profile |
//! | else `days_per_week` equals a profile's | that profile |
//! | else | `CT9_480` |

use crate::checksum::Checksum;
use crate::error::CurriculumError;
use crate::manifest::{Block, BlockSpec, Day, Manifest, Reconciliation, ResourceKind, Structure};
use crate::provenance::provenance_label;
use g9_domain::curriculum::{profile_for, ProfileBlock, STRUCTURE_PROFILES};
use g9_domain::{
    ActorId, CompetencyId, CurriculumCounts, CurriculumId, CurriculumInvariants, DayOfWeek,
    InvariantViolation, ManifestProvenance, SchemaVersion, StructureProfile, StructureProfileId,
    CT9_480,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The newest manifest format a reader of this crate accepts (`1.1`: format
/// 1.0.x and 1.1.x manifests are read; ADR-0023).
pub const SUPPORTED_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 1);

/// The first format that admits `structure.assessment_day_of_week` (`1.1`).
pub const ASSESSMENT_DAY_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 1);

/// Stable error and warning codes. `ALL_ERRORS` / `ALL_WARNINGS` are used by the
/// A01 gate test to prove that every code is reachable from a seed mutation.
pub mod codes {
    /// `manifest_schema_version` is not `1.0.x` or `1.1.x`.
    pub const SCHEMA_VERSION_UNSUPPORTED: &str = "manifest.schema_version.unsupported";
    /// `curriculum_id` is not a valid `CurriculumId` slug.
    pub const CURRICULUM_ID_INVALID: &str = "manifest.curriculum_id.invalid";
    /// `name` is blank.
    pub const NAME_EMPTY: &str = "manifest.name.empty";
    /// `version` is blank.
    pub const VERSION_EMPTY: &str = "manifest.version.empty";
    /// A `structure` field differs from the invariants of the selected
    /// profile, or the declaration matches no profile.
    pub const STRUCTURE_MISMATCH: &str = "structure.mismatch";
    /// `structure.assessment_day_of_week` in a manifest older than format 1.1.0.
    pub const STRUCTURE_ASSESSMENT_DAY_FORMAT: &str =
        "structure.assessment_day_of_week.requires_format_1_1";
    /// `structure.sunday` is not `unscheduled`.
    pub const STRUCTURE_SUNDAY: &str = "structure.sunday.not_unscheduled";
    /// `weekday_blocks` differs from the profile's ordinary-day block list.
    pub const WEEKDAY_BLOCKS_CANONICAL: &str = "blocks.weekday.canonical_mismatch";
    /// `saturday_blocks` differs from the profile's assessment-day block list.
    pub const SATURDAY_BLOCKS_CANONICAL: &str = "blocks.saturday.canonical_mismatch";
    /// Not exactly 12 weeks.
    pub const WEEK_COUNT: &str = "week.count";
    /// `week_number` is not its 1-based position.
    pub const WEEK_NUMBER_SEQUENCE: &str = "week.number.out_of_sequence";
    /// Week title is blank.
    pub const WEEK_TITLE_EMPTY: &str = "week.title.empty";
    /// A week does not have exactly the profile's days (6; 5 in `MASTERY_V2`).
    pub const WEEK_DAYS_COUNT: &str = "week.days.count";
    /// `day_of_week` outside the profile's training days (1..=6; 1..=5 in
    /// `MASTERY_V2`), other than 7.
    pub const DAY_OF_WEEK_RANGE: &str = "day.day_of_week.out_of_range";
    /// `day_of_week` is 7: Sunday is unscheduled.
    pub const DAY_SUNDAY_PRESENT: &str = "day.sunday_present";
    /// `day_of_week` does not strictly increase within the week.
    pub const DAY_OF_WEEK_ORDER: &str = "day.day_of_week.not_increasing";
    /// `day_id` is not the position-derived id (`D01`..`D72`; `D01`..`D60` in `MASTERY_V2`).
    pub const DAY_ID_MISMATCH: &str = "day.id.mismatch";
    /// `day_id` repeats an earlier day.
    pub const DAY_ID_DUPLICATE: &str = "day.id.duplicate";
    /// `planned_minutes` is not the profile's (420 weekday / 300 Saturday; 480 in `MASTERY_V2`).
    pub const DAY_PLANNED_MINUTES: &str = "day.planned_minutes.mismatch";
    /// A day does not have exactly 3 objectives.
    pub const DAY_OBJECTIVES_COUNT: &str = "day.objectives.count";
    /// `evidence_artifact` (deliverable) is blank.
    pub const DAY_EVIDENCE_EMPTY: &str = "day.evidence_artifact.empty";
    /// Block minutes do not sum to `planned_minutes`.
    pub const DAY_BLOCKS_SUM: &str = "day.blocks.sum_mismatch";
    /// Blocks differ from the canonical list for the day type.
    pub const DAY_BLOCKS_CANONICAL: &str = "day.blocks.canonical_mismatch";
    /// `is_assessment_day` is not "the profile's assessment day" (`day_of_week == 6`;
    /// `== 5` in `MASTERY_V2`).
    pub const DAY_ASSESSMENT_FLAG: &str = "day.assessment_flag.mismatch";
    /// Objective `seq` is not its 1-based position.
    pub const OBJECTIVE_SEQ: &str = "objective.seq.mismatch";
    /// `objective_id` is not `<day_id>-O<seq>`.
    pub const OBJECTIVE_ID_MISMATCH: &str = "objective.id.mismatch";
    /// `objective_id` repeats an earlier objective.
    pub const OBJECTIVE_ID_DUPLICATE: &str = "objective.id.duplicate";
    /// Objective text is blank.
    pub const OBJECTIVE_TEXT_EMPTY: &str = "objective.text.empty";
    /// A `competency_ids` entry is not a valid `CompetencyId` slug.
    pub const COMPETENCY_ID_INVALID: &str = "objective.competency_id.invalid";
    /// Block `seq` is not its 1-based position.
    pub const BLOCK_SEQ: &str = "block.seq.mismatch";
    /// A minute count (block, planned, canonical list) is <= 0.
    pub const DURATION_NON_POSITIVE: &str = "duration.non_positive";
    /// Resource `ref` is blank.
    pub const RESOURCE_REF_EMPTY: &str = "resource.ref.empty";
    /// An `EXTERNAL` resource has no `version`.
    pub const RESOURCE_VERSION_REQUIRED: &str = "resource.version.required_for_external";
    /// Provenance other than `PROPOSED_SEED` without a reconciliation block.
    pub const PROVENANCE_RECONCILIATION_REQUIRED: &str = "provenance.reconciliation_required";
    /// A reconciliation block field is missing, malformed or not a 64-hex checksum / actor id / positive instant.
    pub const RECONCILIATION_INCOMPLETE: &str = "reconciliation.incomplete";

    /// Every error code the validator can emit.
    pub const ALL_ERRORS: &[&str] = &[
        SCHEMA_VERSION_UNSUPPORTED,
        CURRICULUM_ID_INVALID,
        NAME_EMPTY,
        VERSION_EMPTY,
        STRUCTURE_MISMATCH,
        STRUCTURE_ASSESSMENT_DAY_FORMAT,
        STRUCTURE_SUNDAY,
        WEEKDAY_BLOCKS_CANONICAL,
        SATURDAY_BLOCKS_CANONICAL,
        WEEK_COUNT,
        WEEK_NUMBER_SEQUENCE,
        WEEK_TITLE_EMPTY,
        WEEK_DAYS_COUNT,
        DAY_OF_WEEK_RANGE,
        DAY_SUNDAY_PRESENT,
        DAY_OF_WEEK_ORDER,
        DAY_ID_MISMATCH,
        DAY_ID_DUPLICATE,
        DAY_PLANNED_MINUTES,
        DAY_OBJECTIVES_COUNT,
        DAY_EVIDENCE_EMPTY,
        DAY_BLOCKS_SUM,
        DAY_BLOCKS_CANONICAL,
        DAY_ASSESSMENT_FLAG,
        OBJECTIVE_SEQ,
        OBJECTIVE_ID_MISMATCH,
        OBJECTIVE_ID_DUPLICATE,
        OBJECTIVE_TEXT_EMPTY,
        COMPETENCY_ID_INVALID,
        BLOCK_SEQ,
        DURATION_NON_POSITIVE,
        RESOURCE_REF_EMPTY,
        RESOURCE_VERSION_REQUIRED,
        PROVENANCE_RECONCILIATION_REQUIRED,
        RECONCILIATION_INCOMPLETE,
    ];

    /// Objective text (trimmed, case-insensitive) repeats an earlier objective.
    pub const WARN_OBJECTIVE_TEXT_DUPLICATE: &str = "objective.text.duplicate";
    /// The same competency id is listed twice on one objective.
    pub const WARN_COMPETENCY_ID_DUPLICATE: &str = "objective.competency_id.duplicate";
    /// A `PROPOSED_SEED` manifest carries a reconciliation block (no effect).
    pub const WARN_RECONCILIATION_IGNORED: &str = "provenance.reconciliation_ignored";

    /// Every warning code the validator can emit.
    pub const ALL_WARNINGS: &[&str] = &[
        WARN_OBJECTIVE_TEXT_DUPLICATE,
        WARN_COMPETENCY_ID_DUPLICATE,
        WARN_RECONCILIATION_IGNORED,
    ];
}

/// A blocking manifest defect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestError {
    /// One of [`codes::ALL_ERRORS`].
    pub code: String,
    /// JSON pointer into the manifest, e.g. `/weeks/2/days/1/objectives/0/text`.
    pub path: String,
    /// Short, content-free explanation (identifiers and expected values only).
    pub message: String,
}

/// A non-blocking manifest observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestWarning {
    /// One of [`codes::ALL_WARNINGS`].
    pub code: String,
    /// JSON pointer into the manifest.
    pub path: String,
    /// Short explanation.
    pub message: String,
}

/// Result of [`validate`]. Import is allowed only when [`ValidationReport::is_valid`].
///
/// `violations` is derived from `counts` and the profile
/// (`StructureProfile::violations`); the manual `Deserialize` recomputes it,
/// because `InvariantViolation::code` is a `&'static str` that cannot be
/// deserialized from owned input. `invariants` is the profile's and is
/// recomputed as well. A report written before profiles existed has no
/// `profile` and reads as `CT9_480`, the only shape there was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    /// The structure profile the manifest was checked against (ADR-0023).
    pub profile: StructureProfileId,
    /// The invariants of that profile (what `counts` was held to).
    pub invariants: CurriculumInvariants,
    /// Observed counts (weeks, days, objectives, minutes, ordinary/assessment days).
    pub counts: CurriculumCounts,
    /// Arithmetic invariant violations derived from `counts` (`g9-domain`).
    pub violations: Vec<InvariantViolation>,
    /// Blocking structural defects.
    pub errors: Vec<ManifestError>,
    /// Non-blocking observations.
    pub warnings: Vec<ManifestWarning>,
}

impl<'de> Deserialize<'de> for ValidationReport {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(default)]
            profile: Option<StructureProfileId>,
            counts: CurriculumCounts,
            #[serde(default)]
            errors: Vec<ManifestError>,
            #[serde(default)]
            warnings: Vec<ManifestWarning>,
        }
        let w = Wire::deserialize(deserializer)?;
        let mut report = ValidationReport::for_profile(
            w.profile.unwrap_or(StructureProfileId::Ct9480).profile(),
            w.counts,
        );
        report.errors = w.errors;
        report.warnings = w.warnings;
        Ok(report)
    }
}

impl ValidationReport {
    /// A report of `counts` checked against `profile`, with no errors and no
    /// warnings: `invariants` and `violations` are the profile's.
    pub fn for_profile(profile: &StructureProfile, counts: CurriculumCounts) -> Self {
        ValidationReport {
            profile: profile.id,
            invariants: profile.invariants,
            violations: profile.violations(&counts),
            counts,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// The structure profile the manifest was checked against.
    pub fn structure_profile(&self) -> &'static StructureProfile {
        self.profile.profile()
    }

    /// True when there are no errors and no invariant violations.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.violations.is_empty()
    }

    /// Whether an error with this code was reported.
    pub fn has_error(&self, code: &str) -> bool {
        self.errors.iter().any(|e| e.code == code)
    }

    /// Whether a warning with this code was reported.
    pub fn has_warning(&self, code: &str) -> bool {
        self.warnings.iter().any(|w| w.code == code)
    }

    /// Sorted, de-duplicated error codes.
    pub fn error_codes(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.errors.iter().map(|e| e.code.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Convert into a `Result`: `Err(CurriculumError::Invalid)` unless valid.
    pub fn into_result(self) -> Result<ValidationReport, CurriculumError> {
        if self.is_valid() {
            Ok(self)
        } else {
            Err(CurriculumError::Invalid(Box::new(self)))
        }
    }

    /// One-line summary for logs and CLI output.
    pub fn summary_line(&self) -> String {
        format!(
            "profile={} weeks={} days={} objectives={} total_minutes={} errors={} violations={} warnings={}",
            self.profile,
            self.counts.weeks,
            self.counts.days,
            self.counts.objectives,
            self.counts.total_minutes,
            self.errors.len(),
            self.violations.len(),
            self.warnings.len()
        )
    }
}

#[derive(Default)]
struct Reporter {
    errors: Vec<ManifestError>,
    warnings: Vec<ManifestWarning>,
}

impl Reporter {
    fn error(&mut self, code: &'static str, path: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ManifestError {
            code: code.to_string(),
            path: path.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, code: &'static str, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(ManifestWarning {
            code: code.to_string(),
            path: path.into(),
            message: message.into(),
        });
    }
}

/// Validate a manifest against every importer proof obligation. Never mutates
/// or corrects; reports everything it finds in document order.
pub fn validate(manifest: &Manifest) -> ValidationReport {
    let mut r = Reporter::default();
    let profile = select_profile(&manifest.structure);
    check_header(manifest, &mut r);
    check_structure(&manifest.structure, profile, &mut r);
    check_block_list(
        "/weekday_blocks",
        &manifest.weekday_blocks,
        profile.ordinary_blocks,
        codes::WEEKDAY_BLOCKS_CANONICAL,
        &mut r,
    );
    check_block_list(
        "/saturday_blocks",
        &manifest.saturday_blocks,
        profile.assessment_blocks,
        codes::SATURDAY_BLOCKS_CANONICAL,
        &mut r,
    );
    let counts = check_weeks(manifest, profile, &mut r);
    check_provenance(manifest, &mut r);
    let mut report = ValidationReport::for_profile(profile, counts);
    report.errors = r.errors;
    report.warnings = r.warnings;
    report
}

/// The structure profile a manifest's `structure` is checked against: the one
/// it declares (`g9_domain::curriculum::profile_for`), or the nearest one when
/// the declaration matches none (see the module documentation; the mismatch
/// itself is reported by [`validate`] as `structure.mismatch`).
pub fn select_profile(structure: &Structure) -> &'static StructureProfile {
    if let Some(profile) = profile_for(structure.days_per_week, structure.assessment_day_of_week) {
        return profile;
    }
    let Some(declared) = structure.assessment_day_of_week else {
        return &CT9_480;
    };
    STRUCTURE_PROFILES
        .into_iter()
        .find(|p| p.assessment_day_of_week == declared)
        .or_else(|| {
            STRUCTURE_PROFILES
                .into_iter()
                .find(|p| p.days_per_week() == structure.days_per_week)
        })
        .unwrap_or(&CT9_480)
}

/// Parse `MAJOR.MINOR.PATCH` into a [`SchemaVersion`] (patch is ignored).
pub fn parse_schema_version(s: &str) -> Option<SchemaVersion> {
    let mut parts = s.split('.');
    let major: u16 = parts.next()?.parse().ok()?;
    let minor: u16 = parts.next()?.parse().ok()?;
    let _patch: u16 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SchemaVersion::new(major, minor))
}

fn check_header(m: &Manifest, r: &mut Reporter) {
    match parse_schema_version(&m.manifest_schema_version) {
        Some(v) if SUPPORTED_SCHEMA_VERSION.accepts(v) => {
            // The field that declares the second profile exists from 1.1.0 on:
            // a 1.0.x manifest that carries it is refused, not reinterpreted.
            if m.structure.assessment_day_of_week.is_some()
                && !v.accepts(ASSESSMENT_DAY_SCHEMA_VERSION)
            {
                r.error(
                    codes::STRUCTURE_ASSESSMENT_DAY_FORMAT,
                    "/structure/assessment_day_of_week",
                    format!(
                        "assessment_day_of_week needs manifest format {ASSESSMENT_DAY_SCHEMA_VERSION}; this manifest declares {v}"
                    ),
                );
            }
        }
        _ => r.error(
            codes::SCHEMA_VERSION_UNSUPPORTED,
            "/manifest_schema_version",
            format!("expected a MAJOR.MINOR.PATCH version accepted by {SUPPORTED_SCHEMA_VERSION}"),
        ),
    }
    if let Err(e) = CurriculumId::parse(&m.curriculum_id) {
        r.error(codes::CURRICULUM_ID_INVALID, "/curriculum_id", e.reason);
    }
    if m.name.trim().is_empty() {
        r.error(codes::NAME_EMPTY, "/name", "name must not be empty");
    }
    if m.version.trim().is_empty() {
        r.error(
            codes::VERSION_EMPTY,
            "/version",
            "version must not be empty",
        );
    }
}

fn check_structure(s: &Structure, profile: &StructureProfile, r: &mut Reporter) {
    let inv = profile.invariants;
    let fields: [(&str, u32, u32); 7] = [
        ("weeks", inv.weeks, s.weeks),
        ("days_per_week", inv.days_per_week, s.days_per_week),
        (
            "objectives_per_day",
            inv.objectives_per_day,
            s.objectives_per_day,
        ),
        ("weekday_minutes", inv.weekday_minutes, s.weekday_minutes),
        ("saturday_minutes", inv.saturday_minutes, s.saturday_minutes),
        ("weekly_minutes", inv.weekly_minutes, s.weekly_minutes),
        ("total_minutes", inv.total_minutes, s.total_minutes),
    ];
    for (name, expected, actual) in fields {
        if expected != actual {
            r.error(
                codes::STRUCTURE_MISMATCH,
                format!("/structure/{name}"),
                format!("expected {expected}, declared {actual}"),
            );
        }
    }
    // Absent means the default assessment day; `select_profile` picks CT9_480
    // then, so only a declared value can differ from the profile's.
    if let Some(declared) = s.assessment_day_of_week {
        if declared != profile.assessment_day_of_week {
            r.error(
                codes::STRUCTURE_MISMATCH,
                "/structure/assessment_day_of_week",
                format!(
                    "expected {}, declared {declared}",
                    profile.assessment_day_of_week
                ),
            );
        }
    }
    if s.sunday != "unscheduled" {
        r.error(
            codes::STRUCTURE_SUNDAY,
            "/structure/sunday",
            "Sunday must be declared `unscheduled`",
        );
    }
}

fn canonical_description(canonical: &[ProfileBlock]) -> String {
    canonical
        .iter()
        .map(|b| format!("{} {}", b.code, b.minutes))
        .collect::<Vec<_>>()
        .join(", ")
}

fn check_block_list(
    path: &str,
    list: &[BlockSpec],
    canonical: &[ProfileBlock],
    code: &'static str,
    r: &mut Reporter,
) {
    for (i, b) in list.iter().enumerate() {
        if b.minutes <= 0 {
            r.error(
                codes::DURATION_NON_POSITIVE,
                format!("{path}/{i}/minutes"),
                format!("block `{}` has non-positive minutes {}", b.block, b.minutes),
            );
        }
    }
    let matches = list.len() == canonical.len()
        && list
            .iter()
            .zip(canonical.iter())
            .all(|(b, c)| b.block == c.code && b.minutes == i64::from(c.minutes));
    if !matches {
        r.error(
            code,
            path,
            format!("expected exactly [{}]", canonical_description(canonical)),
        );
    }
}

struct SeenIds {
    day_ids: BTreeMap<String, String>,
    objective_ids: BTreeMap<String, String>,
    objective_texts: BTreeMap<String, String>,
}

fn check_weeks(m: &Manifest, profile: &StructureProfile, r: &mut Reporter) -> CurriculumCounts {
    let inv = profile.invariants;
    let mut counts = CurriculumCounts::default();
    let mut seen = SeenIds {
        day_ids: BTreeMap::new(),
        objective_ids: BTreeMap::new(),
        objective_texts: BTreeMap::new(),
    };
    if m.weeks.len() != inv.weeks as usize {
        r.error(
            codes::WEEK_COUNT,
            "/weeks",
            format!("expected {} weeks, found {}", inv.weeks, m.weeks.len()),
        );
    }
    for (wi, week) in m.weeks.iter().enumerate() {
        let wpath = format!("/weeks/{wi}");
        counts.weeks = counts.weeks.saturating_add(1);
        let expected_number = u32::try_from(wi + 1).unwrap_or(u32::MAX);
        if week.week_number != expected_number {
            r.error(
                codes::WEEK_NUMBER_SEQUENCE,
                format!("{wpath}/week_number"),
                format!(
                    "expected week_number {expected_number}, found {}",
                    week.week_number
                ),
            );
        }
        if week.title.trim().is_empty() {
            r.error(
                codes::WEEK_TITLE_EMPTY,
                format!("{wpath}/title"),
                "week title must not be empty",
            );
        }
        if week.days.len() != inv.days_per_week as usize {
            r.error(
                codes::WEEK_DAYS_COUNT,
                format!("{wpath}/days"),
                format!(
                    "expected {} days, found {}",
                    inv.days_per_week,
                    week.days.len()
                ),
            );
        }
        let mut prev_dow: Option<u8> = None;
        for (di, day) in week.days.iter().enumerate() {
            let dpath = format!("{wpath}/days/{di}");
            counts.days = counts.days.saturating_add(1);
            check_day(
                day,
                profile,
                wi,
                di,
                &dpath,
                prev_dow,
                &mut seen,
                &mut counts,
                r,
            );
            prev_dow = Some(day.day_of_week);
        }
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn check_day(
    day: &Day,
    profile: &StructureProfile,
    wi: usize,
    di: usize,
    dpath: &str,
    prev_dow: Option<u8>,
    seen: &mut SeenIds,
    counts: &mut CurriculumCounts,
    r: &mut Reporter,
) {
    let inv = profile.invariants;
    // Day of week: a training day of the profile (`None` otherwise; the checks
    // that depend on the day type are then skipped, the error is reported once).
    let dow: Option<u8> = match day.day_of_week {
        7 => {
            r.error(
                codes::DAY_SUNDAY_PRESENT,
                format!("{dpath}/day_of_week"),
                "Sunday (7) is unscheduled and must not appear",
            );
            None
        }
        d => match DayOfWeek::new(d) {
            Ok(dow) if profile.is_training_day(dow.get()) => Some(dow.get()),
            Ok(dow) => {
                r.error(
                    codes::DAY_OF_WEEK_RANGE,
                    format!("{dpath}/day_of_week"),
                    format!(
                        "day_of_week {} is not a training day of profile {}: expected 1..={}",
                        dow.get(),
                        profile.id,
                        profile.days_per_week()
                    ),
                );
                None
            }
            Err(v) => {
                r.error(
                    codes::DAY_OF_WEEK_RANGE,
                    format!("{dpath}/day_of_week"),
                    v.to_string(),
                );
                None
            }
        },
    };
    if let Some(p) = prev_dow {
        if day.day_of_week <= p {
            r.error(
                codes::DAY_OF_WEEK_ORDER,
                format!("{dpath}/day_of_week"),
                format!("day_of_week {} does not follow {p}", day.day_of_week),
            );
        }
    }
    // Day id: position-derived, so a wrong week_number does not cascade.
    let expected_id = format!("D{:02}", wi * inv.days_per_week as usize + di + 1);
    if day.day_id != expected_id {
        r.error(
            codes::DAY_ID_MISMATCH,
            format!("{dpath}/day_id"),
            format!("expected `{expected_id}`, found `{}`", day.day_id),
        );
    }
    if let Some(first) = seen.day_ids.insert(day.day_id.clone(), dpath.to_string()) {
        r.error(
            codes::DAY_ID_DUPLICATE,
            format!("{dpath}/day_id"),
            format!("day id `{}` already used at {first}", day.day_id),
        );
    }
    // Planned minutes.
    if day.planned_minutes <= 0 {
        r.error(
            codes::DURATION_NON_POSITIVE,
            format!("{dpath}/planned_minutes"),
            format!("planned_minutes {} is not positive", day.planned_minutes),
        );
    }
    if let Some(d) = dow {
        let expected = i64::from(profile.planned_minutes(d));
        if day.planned_minutes != expected {
            r.error(
                codes::DAY_PLANNED_MINUTES,
                format!("{dpath}/planned_minutes"),
                format!(
                    "expected {expected} for day_of_week {d}, found {}",
                    day.planned_minutes
                ),
            );
        }
        let assessed = profile.is_assessment_day(d);
        if assessed {
            counts.saturday_days = counts.saturday_days.saturating_add(1);
        } else {
            counts.weekday_days = counts.weekday_days.saturating_add(1);
        }
        if day.is_assessment_day != assessed {
            r.error(
                codes::DAY_ASSESSMENT_FLAG,
                format!("{dpath}/is_assessment_day"),
                format!("is_assessment_day must be {assessed} on day_of_week {d}"),
            );
        }
    }
    let planned_u32 = u32::try_from(day.planned_minutes.max(0)).unwrap_or(u32::MAX);
    counts.total_minutes = counts.total_minutes.saturating_add(planned_u32);

    // Objectives.
    if day.objectives.len() != inv.objectives_per_day as usize {
        r.error(
            codes::DAY_OBJECTIVES_COUNT,
            format!("{dpath}/objectives"),
            format!(
                "expected {} objectives, found {}",
                inv.objectives_per_day,
                day.objectives.len()
            ),
        );
    }
    for (oi, obj) in day.objectives.iter().enumerate() {
        let opath = format!("{dpath}/objectives/{oi}");
        counts.objectives = counts.objectives.saturating_add(1);
        let expected_seq = u32::try_from(oi + 1).unwrap_or(u32::MAX);
        if obj.seq != expected_seq {
            r.error(
                codes::OBJECTIVE_SEQ,
                format!("{opath}/seq"),
                format!("expected seq {expected_seq}, found {}", obj.seq),
            );
        }
        let expected_oid = format!("{}-O{expected_seq}", day.day_id);
        if obj.objective_id != expected_oid {
            r.error(
                codes::OBJECTIVE_ID_MISMATCH,
                format!("{opath}/objective_id"),
                format!("expected `{expected_oid}`, found `{}`", obj.objective_id),
            );
        }
        if let Some(first) = seen
            .objective_ids
            .insert(obj.objective_id.clone(), opath.clone())
        {
            r.error(
                codes::OBJECTIVE_ID_DUPLICATE,
                format!("{opath}/objective_id"),
                format!(
                    "objective id `{}` already used at {first}",
                    obj.objective_id
                ),
            );
        }
        let text = obj.text.trim();
        if text.is_empty() {
            r.error(
                codes::OBJECTIVE_TEXT_EMPTY,
                format!("{opath}/text"),
                "objective text must not be empty",
            );
        } else {
            let key = text.to_lowercase();
            if let Some(first) = seen.objective_texts.insert(key, opath.clone()) {
                r.warn(
                    codes::WARN_OBJECTIVE_TEXT_DUPLICATE,
                    format!("{opath}/text"),
                    format!("objective text duplicates {first}"),
                );
            }
        }
        let mut seen_comp: BTreeMap<&str, usize> = BTreeMap::new();
        for (ci, cid) in obj.competency_ids.iter().enumerate() {
            let cpath = format!("{opath}/competency_ids/{ci}");
            if let Err(e) = CompetencyId::parse(cid) {
                r.error(codes::COMPETENCY_ID_INVALID, cpath.clone(), e.reason);
            }
            if let Some(first) = seen_comp.insert(cid.as_str(), ci) {
                r.warn(
                    codes::WARN_COMPETENCY_ID_DUPLICATE,
                    cpath,
                    format!("competency id repeats entry {first} of this objective"),
                );
            }
        }
    }

    // Deliverable.
    if day.evidence_artifact.trim().is_empty() {
        r.error(
            codes::DAY_EVIDENCE_EMPTY,
            format!("{dpath}/evidence_artifact"),
            "every day needs a deliverable",
        );
    }

    // Blocks.
    check_blocks(day, profile, dow, dpath, r);

    // Resources.
    for (ri, res) in day.resources.iter().enumerate() {
        let rpath = format!("{dpath}/resources/{ri}");
        if res.reference.trim().is_empty() {
            r.error(
                codes::RESOURCE_REF_EMPTY,
                format!("{rpath}/ref"),
                "resource ref must not be empty",
            );
        }
        let versioned = res.version.as_deref().is_some_and(|v| !v.trim().is_empty());
        if res.kind == ResourceKind::External && !versioned {
            r.error(
                codes::RESOURCE_VERSION_REQUIRED,
                format!("{rpath}/version"),
                "external resources must carry a version",
            );
        }
    }
}

fn check_blocks(
    day: &Day,
    profile: &StructureProfile,
    dow: Option<u8>,
    dpath: &str,
    r: &mut Reporter,
) {
    let mut sum: i64 = 0;
    for (bi, b) in day.blocks.iter().enumerate() {
        let bpath = format!("{dpath}/blocks/{bi}");
        let expected_seq = u32::try_from(bi + 1).unwrap_or(u32::MAX);
        if b.seq != expected_seq {
            r.error(
                codes::BLOCK_SEQ,
                format!("{bpath}/seq"),
                format!("expected seq {expected_seq}, found {}", b.seq),
            );
        }
        if b.minutes <= 0 {
            r.error(
                codes::DURATION_NON_POSITIVE,
                format!("{bpath}/minutes"),
                format!("block `{}` has non-positive minutes {}", b.block, b.minutes),
            );
        }
        sum = sum.saturating_add(b.minutes);
    }
    if sum != day.planned_minutes {
        r.error(
            codes::DAY_BLOCKS_SUM,
            format!("{dpath}/blocks"),
            format!(
                "blocks sum to {sum}, planned_minutes is {}",
                day.planned_minutes
            ),
        );
    }
    if let Some(d) = dow {
        let canonical = profile.day_blocks(d);
        if !blocks_match(&day.blocks, canonical) {
            r.error(
                codes::DAY_BLOCKS_CANONICAL,
                format!("{dpath}/blocks"),
                format!("expected exactly [{}]", canonical_description(canonical)),
            );
        }
    }
}

fn blocks_match(blocks: &[Block], canonical: &[ProfileBlock]) -> bool {
    blocks.len() == canonical.len()
        && blocks
            .iter()
            .zip(canonical.iter())
            .all(|(b, c)| b.block == c.code && b.minutes == i64::from(c.minutes))
}

fn check_provenance(m: &Manifest, r: &mut Reporter) {
    match (m.provenance, &m.reconciliation) {
        (ManifestProvenance::ProposedSeed, None) => {}
        (ManifestProvenance::ProposedSeed, Some(rec)) => {
            r.warn(
                codes::WARN_RECONCILIATION_IGNORED,
                "/reconciliation",
                "a PROPOSED_SEED manifest carries a reconciliation block; it has no effect until provenance is RECONCILED_CANDIDATE",
            );
            check_reconciliation(rec, r);
        }
        (_, None) => r.error(
            codes::PROVENANCE_RECONCILIATION_REQUIRED,
            "/provenance",
            format!(
                "provenance {} requires a complete reconciliation block",
                provenance_label(m.provenance)
            ),
        ),
        (_, Some(rec)) => check_reconciliation(rec, r),
    }
}

fn check_reconciliation(rec: &Reconciliation, r: &mut Reporter) {
    if Checksum::parse(&rec.original_checksum).is_err() {
        r.error(
            codes::RECONCILIATION_INCOMPLETE,
            "/reconciliation/original_checksum",
            "original_checksum must be a 64-character lowercase hex SHA-256",
        );
    }
    if Checksum::parse(&rec.diff_checksum).is_err() {
        r.error(
            codes::RECONCILIATION_INCOMPLETE,
            "/reconciliation/diff_checksum",
            "diff_checksum must be a 64-character lowercase hex SHA-256",
        );
    }
    if let Err(e) = ActorId::parse(&rec.approved_by) {
        r.error(
            codes::RECONCILIATION_INCOMPLETE,
            "/reconciliation/approved_by",
            format!("approved_by must be an actor id: {}", e.reason),
        );
    }
    if rec.approved_at.as_millis() <= 0 {
        r.error(
            codes::RECONCILIATION_INCOMPLETE,
            "/reconciliation/approved_at",
            "approved_at must be a positive UTC epoch millisecond instant",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_parsing() {
        assert_eq!(
            parse_schema_version("1.0.0"),
            Some(SchemaVersion::new(1, 0))
        );
        assert_eq!(
            parse_schema_version("1.0.9"),
            Some(SchemaVersion::new(1, 0))
        );
        assert_eq!(parse_schema_version("1.0"), None);
        assert_eq!(parse_schema_version("1.0.0.0"), None);
        assert_eq!(parse_schema_version("x.y.z"), None);
        assert!(SUPPORTED_SCHEMA_VERSION.accepts(SchemaVersion::new(1, 0)));
        // ADR-0023: format 1.1 (one optional field) is read; 1.2 is the first refused minor.
        assert!(SUPPORTED_SCHEMA_VERSION.accepts(SchemaVersion::new(1, 1)));
        assert!(!SUPPORTED_SCHEMA_VERSION.accepts(SchemaVersion::new(1, 2)));
        assert!(!SUPPORTED_SCHEMA_VERSION.accepts(SchemaVersion::new(2, 0)));
        assert!(!SUPPORTED_SCHEMA_VERSION.accepts(SchemaVersion::new(0, 1)));
        assert_eq!(
            parse_schema_version(crate::manifest::MANIFEST_FORMAT_VERSION_1_1),
            Some(ASSESSMENT_DAY_SCHEMA_VERSION)
        );
        assert_eq!(
            parse_schema_version(crate::manifest::MANIFEST_FORMAT_VERSION),
            Some(SchemaVersion::V1)
        );
    }

    fn structure(days_per_week: u32, assessment_day_of_week: Option<u8>) -> Structure {
        Structure {
            weeks: 12,
            days_per_week,
            objectives_per_day: 3,
            weekday_minutes: 420,
            saturday_minutes: 300,
            weekly_minutes: 2_400,
            total_minutes: 28_800,
            sunday: "unscheduled".to_string(),
            assessment_day_of_week,
        }
    }

    #[test]
    fn profile_selection_is_exact_or_nearest() {
        use StructureProfileId::{Ct9480, MasteryV2};
        let cases = [
            // Exact declarations.
            ((6, None), Ct9480),
            ((6, Some(6)), Ct9480),
            ((5, Some(5)), MasteryV2),
            // Without the field the manifest is the Appendix A shape, whatever else it says.
            ((5, None), Ct9480),
            ((9, None), Ct9480),
            // With it: the assessment day decides first, then the days a week.
            ((6, Some(5)), MasteryV2),
            ((5, Some(6)), Ct9480),
            ((5, Some(4)), MasteryV2),
            ((6, Some(4)), Ct9480),
            ((3, Some(2)), Ct9480),
        ];
        for ((days, assessment), expected) in cases {
            assert_eq!(
                select_profile(&structure(days, assessment)).id,
                expected,
                "{days} {assessment:?}"
            );
        }
    }

    #[test]
    fn a_report_without_a_profile_reads_as_the_appendix_a_shape() {
        // The shape of every report stored before ADR-0023.
        let old = r#"{"counts":{"weeks":12,"days":72,"objectives":216,"total_minutes":28800,"weekday_days":60,"saturday_days":12},"violations":[],"errors":[],"warnings":[]}"#;
        let report: ValidationReport = serde_json::from_str(old).unwrap();
        assert_eq!(report.profile, StructureProfileId::Ct9480);
        assert_eq!(report.invariants, CT9_480.invariants);
        assert!(report.is_valid());

        // A v2 report recomputes its violations against the v2 numbers.
        let counts = CurriculumCounts {
            weeks: 12,
            days: 60,
            objectives: 180,
            total_minutes: 28_800,
            weekday_days: 48,
            saturday_days: 12,
        };
        let v2 = ValidationReport::for_profile(&g9_domain::MASTERY_V2, counts);
        assert!(v2.is_valid());
        let json = serde_json::to_value(&v2).unwrap();
        assert_eq!(json["profile"], "MASTERY_V2");
        assert_eq!(json["invariants"]["total_days"], 60);
        assert_eq!(json["invariants"]["days_per_week"], 5);
        let back: ValidationReport = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, v2);
        // The same counts under the other profile are three violations: the
        // profile is part of what the report says.
        let mut relabelled = json;
        relabelled["profile"] = serde_json::json!("CT9_480");
        let as_ct9: ValidationReport = serde_json::from_value(relabelled).unwrap();
        assert_eq!(as_ct9.violations.len(), 3);
        assert_eq!(
            as_ct9.invariants, CT9_480.invariants,
            "never the stored copy"
        );
        assert!(v2
            .summary_line()
            .starts_with("profile=MASTERY_V2 weeks=12 days=60"));
    }

    #[test]
    fn report_round_trips_through_json_recomputing_violations() {
        let counts = CurriculumCounts {
            weeks: 12,
            days: 71,
            objectives: 213,
            total_minutes: 28_380,
            weekday_days: 59,
            saturday_days: 12,
        };
        let report = ValidationReport {
            profile: StructureProfileId::Ct9480,
            invariants: CT9_480.invariants,
            counts,
            violations: counts.violations(),
            errors: vec![ManifestError {
                code: codes::WEEK_DAYS_COUNT.to_string(),
                path: "/weeks/11/days".to_string(),
                message: "expected 6 days, found 5".to_string(),
            }],
            warnings: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        let back: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
        assert_eq!(back.violations.len(), 4);
        assert!(!back.is_valid());
    }

    #[test]
    fn code_tables_are_unique_and_disjoint() {
        let mut all: Vec<&str> = codes::ALL_ERRORS.to_vec();
        all.extend_from_slice(codes::ALL_WARNINGS);
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "duplicate validator code");
    }
}
