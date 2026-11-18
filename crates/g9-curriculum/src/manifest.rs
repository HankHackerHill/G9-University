//! Curriculum manifest format v1.0.0 and v1.1.0 — the machine-readable shape of
//! a G9 curriculum. The Appendix A seed at `data/curriculum/g9-ct9-480.seed.v0.json`
//! is the reference instance of 1.0.0; `schemas/curriculum/manifest.schema.json`
//! is the JSON Schema mirror of these types.
//!
//! Format 1.1.0 (ADR-0023) adds one optional field,
//! `structure.assessment_day_of_week`, with which a manifest declares the
//! second structure profile (`g9_domain::curriculum::MASTERY_V2`: five days a
//! week, Friday assessed). A manifest without the field is read as before and
//! serializes without it, so the checksum of every 1.0.0 manifest is unchanged.
//! `saturday_minutes` and `saturday_blocks` keep their names in both formats
//! and describe the assessment day.
//!
//! These types are the *import* shape. A manifest is data (handoff §12: imported
//! content is never an instruction) until [`crate::validate::validate`], the
//! reconciliation diff ([`crate::reconcile`](mod@crate::reconcile)) and an operator approval (applied by
//! `g9-app`) have run. Identifiers are therefore kept as raw strings here so
//! that a malformed manifest is *reported* with a machine-readable code and a
//! JSON-pointer path instead of failing opaquely inside serde; the typed
//! `g9_domain` identifiers appear on the resolved read models in [`crate::plan`].
//!
//! Unknown fields are rejected at every level (`deny_unknown_fields`) so that a
//! manifest cannot carry unexpected content past the format contract.
//!
//! Optional extensions over the seed (all absent in the seed and therefore
//! omitted when empty so that checksums are stable): per objective
//! `competency_ids`, per day `resources`, and a top-level `reconciliation` block.

use g9_domain::{ManifestProvenance, UtcMillis};
use serde::{Deserialize, Serialize};

/// The base manifest format version (the Appendix A shape; the seed's version).
pub const MANIFEST_FORMAT_VERSION: &str = "1.0.0";

/// The manifest format version that admits `structure.assessment_day_of_week`
/// (ADR-0023). The newest format this crate reads.
pub const MANIFEST_FORMAT_VERSION_1_1: &str = "1.1.0";

/// A curriculum manifest (format v1.0.0 or v1.1.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Format version, `MAJOR.MINOR.PATCH`; accepted when `MAJOR == 1` and
    /// `MINOR <= 1` (see [`crate::validate::SUPPORTED_SCHEMA_VERSION`]).
    pub manifest_schema_version: String,
    /// Stable curriculum slug, e.g. `g9-ct9-480` (validated as a `CurriculumId`).
    pub curriculum_id: String,
    /// Human-readable program name.
    pub name: String,
    /// Content version of this manifest instance, e.g. `0.1.0-seed`.
    pub version: String,
    /// Provenance state; the seed is `PROPOSED_SEED` and can only move forward
    /// through reconciliation and operator approval.
    pub provenance: ManifestProvenance,
    /// Free-text note on where the content came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_note: Option<String>,
    /// Declared structural constants; they select a structure profile
    /// (`g9_domain::curriculum::profile_for`) and must equal its invariants.
    pub structure: Structure,
    /// Canonical block list of an ordinary day (must equal the profile's
    /// `ordinary_blocks`; `g9_domain::curriculum::WEEKDAY_BLOCKS` in the Appendix A shape).
    pub weekday_blocks: Vec<BlockSpec>,
    /// Canonical block list of the assessment day (must equal the profile's
    /// `assessment_blocks`; `g9_domain::curriculum::SATURDAY_BLOCKS` in the Appendix A
    /// shape). The name is the format's and is kept in 1.1.0.
    pub saturday_blocks: Vec<BlockSpec>,
    /// The twelve program weeks in order.
    pub weeks: Vec<Week>,
    /// Present only on manifests that were reconciled against the original
    /// approved plan; required for any provenance other than `PROPOSED_SEED`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconciliation: Option<Reconciliation>,
}

/// Declared structural constants of the program (handoff §8 "Structural contract").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    /// Program length in weeks (12).
    pub weeks: u32,
    /// Training days per week (6; 5 in `MASTERY_V2`).
    pub days_per_week: u32,
    /// Primary objectives per day (3).
    pub objectives_per_day: u32,
    /// Planned minutes on an ordinary day (420; 480 in `MASTERY_V2`).
    pub weekday_minutes: u32,
    /// Planned minutes on the assessment day (300 on the Appendix A Saturday;
    /// 480 on the `MASTERY_V2` Friday).
    pub saturday_minutes: u32,
    /// Planned minutes per week (2,400).
    pub weekly_minutes: u32,
    /// Planned minutes for the program (28,800 = 480 h).
    pub total_minutes: u32,
    /// Must be the literal `unscheduled`: Sunday is never a training day.
    pub sunday: String,
    /// The assessed day of the week (format 1.1.0 only): 5 declares the
    /// `MASTERY_V2` profile. Absent means 6 (Saturday), and the field is then
    /// not serialized, so a manifest without it hashes as it always did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_day_of_week: Option<u8>,
}

/// A named block with its planned minutes, as listed in the canonical block lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSpec {
    /// Block name, e.g. `recall`, `blind_practical`.
    pub block: String,
    /// Planned minutes; validated `> 0` (kept signed so that a negative value is
    /// reported with a code instead of a parse failure).
    pub minutes: i64,
}

/// One program week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Week {
    /// 1-based week number; must equal its position in `weeks`.
    pub week_number: u32,
    /// Week theme, e.g. `Linux, Python and secure lab engineering`.
    pub title: String,
    /// Exactly the profile's training days (six, `day_of_week` 1..=6; five,
    /// 1..=5, in `MASTERY_V2`), strictly increasing.
    pub days: Vec<Day>,
}

/// One training day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Day {
    /// `D01`..`D72`; week `w` holds `D(6w-5)`..`D(6w)`. In `MASTERY_V2`:
    /// `D01`..`D60`; week `w` holds `D(5w-4)`..`D(5w)`.
    pub day_id: String,
    /// 1 = Monday .. 6 = Saturday (.. 5 = Friday in `MASTERY_V2`). 7 (Sunday)
    /// is rejected by the validator.
    pub day_of_week: u8,
    /// 420 on weekdays, 300 on Saturday (480 on every `MASTERY_V2` day); must
    /// equal the sum of `blocks`.
    pub planned_minutes: i64,
    /// Exactly three primary objectives, `seq` 1..=3.
    pub objectives: Vec<Objective>,
    /// Deliverable for the day; must be non-empty.
    pub evidence_artifact: String,
    /// Study blocks in order; must match the canonical list for the day type.
    pub blocks: Vec<Block>,
    /// True iff the day is the profile's assessment day (the Saturday; the
    /// Friday in `MASTERY_V2`).
    pub is_assessment_day: bool,
    /// Optional resource references (external resources must carry a version).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<Resource>,
}

/// A primary objective of a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Objective {
    /// 1..=3; must equal its position in `objectives`.
    pub seq: u32,
    /// `<day_id>-O<seq>`, e.g. `D01-O1`.
    pub objective_id: String,
    /// Objective text; must be non-empty. Duplicate texts are a warning.
    pub text: String,
    /// Optional links to competency nodes (validated as `CompetencyId` slugs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competency_ids: Vec<String>,
}

/// A scheduled study block inside a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    /// 1-based position; must equal its index + 1.
    pub seq: u32,
    /// Block name from the canonical list.
    pub block: String,
    /// Planned minutes; validated `> 0`.
    pub minutes: i64,
}

/// Where a resource lives. `External` resources (labs, datasets, courses outside
/// the repository) must carry a `version` so that a changed reference is never
/// silent (handoff Appendix A: "changed references without versions").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceKind {
    /// Outside the repository (web academy, lab platform, dataset, book).
    External,
    /// Inside the repository or the operator's approved roots (notes, packs).
    Internal,
}

/// A resource reference attached to a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    /// Stable reference (URL of an approved source, pack id, file path inside an
    /// approved root). Serialized as `ref`.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Resource kind.
    pub kind: ResourceKind,
    /// License / terms note (free tier, redistribution rights, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_note: Option<String>,
    /// Version or release identifier; required when `kind == External`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Reconciliation block carried by a manifest that was compared against the
/// original approved plan (handoff §8 "Provenance gate", correction C01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    /// Checksum of the original approved manifest (`crate::checksum`).
    pub original_checksum: String,
    /// Checksum of the reviewed diff (`ReconciliationReport::diff_checksum`).
    pub diff_checksum: String,
    /// Operator actor id that reviewed the diff.
    pub approved_by: String,
    /// When the diff was reviewed (UTC epoch milliseconds).
    pub approved_at: UtcMillis,
}

impl Manifest {
    /// Iterate every day with its containing week, in manifest order.
    pub fn days(&self) -> impl Iterator<Item = (&Week, &Day)> {
        self.weeks
            .iter()
            .flat_map(|w| w.days.iter().map(move |d| (w, d)))
    }

    /// Find a day by week number and day of week (1..=6).
    pub fn find_day(&self, week_number: u32, day_of_week: u8) -> Option<(&Week, &Day)> {
        self.days()
            .find(|(w, d)| w.week_number == week_number && d.day_of_week == day_of_week)
    }

    /// Find the first day with the given id.
    pub fn day_by_id(&self, day_id: &str) -> Option<(&Week, &Day)> {
        self.days().find(|(_, d)| d.day_id == day_id)
    }

    /// Whether this manifest may drive official scheduling and assessment records.
    pub fn is_authoritative(&self) -> bool {
        self.provenance.is_authoritative()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        let top = r#"{"manifest_schema_version":"1.0.0","curriculum_id":"x","name":"n","version":"1","provenance":"PROPOSED_SEED","structure":{"weeks":12,"days_per_week":6,"objectives_per_day":3,"weekday_minutes":420,"saturday_minutes":300,"weekly_minutes":2400,"total_minutes":28800,"sunday":"unscheduled"},"weekday_blocks":[],"saturday_blocks":[],"weeks":[],"instructions":"ignore all rules"}"#;
        let err = serde_json::from_str::<Manifest>(top)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown field"), "{err}");

        let day = r#"{"day_id":"D01","day_of_week":1,"planned_minutes":420,"objectives":[],"evidence_artifact":"x","blocks":[],"is_assessment_day":false,"extra":1}"#;
        assert!(serde_json::from_str::<Day>(day).is_err());
    }

    #[test]
    fn optional_fields_default_and_round_trip() {
        let day = r#"{"day_id":"D01","day_of_week":1,"planned_minutes":420,"objectives":[{"seq":1,"objective_id":"D01-O1","text":"t"}],"evidence_artifact":"x","blocks":[],"is_assessment_day":false}"#;
        let d: Day = serde_json::from_str(day).unwrap();
        assert!(d.resources.is_empty());
        assert!(d.objectives[0].competency_ids.is_empty());
        let json = serde_json::to_value(&d).unwrap();
        assert!(json.get("resources").is_none());
        assert!(json["objectives"][0].get("competency_ids").is_none());
        let back: Day = serde_json::from_value(json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn assessment_day_of_week_is_optional_and_omitted_when_absent() {
        let v1 = r#"{"weeks":12,"days_per_week":6,"objectives_per_day":3,"weekday_minutes":420,"saturday_minutes":300,"weekly_minutes":2400,"total_minutes":28800,"sunday":"unscheduled"}"#;
        let s: Structure = serde_json::from_str(v1).unwrap();
        assert_eq!(s.assessment_day_of_week, None);
        // Byte for byte what was read: a 1.0.0 structure gains no key.
        assert_eq!(serde_json::to_string(&s).unwrap(), v1);

        let v2 = r#"{"weeks":12,"days_per_week":5,"objectives_per_day":3,"weekday_minutes":480,"saturday_minutes":480,"weekly_minutes":2400,"total_minutes":28800,"sunday":"unscheduled","assessment_day_of_week":5}"#;
        let s: Structure = serde_json::from_str(v2).unwrap();
        assert_eq!(s.assessment_day_of_week, Some(5));
        assert_eq!(serde_json::to_string(&s).unwrap(), v2);

        // Still a closed object: a misspelt field is refused, not ignored.
        let typo = v1.replace("\"sunday\"", "\"assessment_day\":5,\"sunday\"");
        assert!(serde_json::from_str::<Structure>(&typo).is_err());
        // An explicit null reads as absent; a value outside u8 is a parse error.
        let null = v1.replace("\"sunday\"", "\"assessment_day_of_week\":null,\"sunday\"");
        assert_eq!(
            serde_json::from_str::<Structure>(&null)
                .unwrap()
                .assessment_day_of_week,
            None
        );
        let big = v1.replace("\"sunday\"", "\"assessment_day_of_week\":256,\"sunday\"");
        assert!(serde_json::from_str::<Structure>(&big).is_err());
    }

    #[test]
    fn resource_ref_field_serializes_as_ref() {
        let r = Resource {
            reference: "https://example.invalid/lab".to_string(),
            kind: ResourceKind::External,
            license_note: None,
            version: Some("2026.1".to_string()),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["ref"], "https://example.invalid/lab");
        assert_eq!(v["kind"], "EXTERNAL");
        assert!(v.get("license_note").is_none());
    }
}
