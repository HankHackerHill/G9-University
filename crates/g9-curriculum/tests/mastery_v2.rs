//! The second structure profile, `MASTERY_V2` (ADR-0023; manifest format 1.1.0), next to the
//! Appendix A shape: validation, every rule of the profile, plans, reconciliation, and the proof
//! that the two manifests that exist keep their checksums.
//!
//! The v2 manifests here are generated (`common/mastery_v2.rs`): a structural fixture with
//! placeholder texts. Nothing here approves or promotes anything.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mastery_v2.rs"]
mod v2;

use g9_curriculum::*;
use g9_domain::{
    ErrorCategory, G9Error, ManifestProvenance, StructureProfileId, UtcMillis, CT9_480, MASTERY_V2,
};
use pretty_assertions::assert_eq;

const SEED_V0: &str = include_str!("../../../data/curriculum/g9-ct9-480.seed.v0.json");
const DRAFT_V1: &str = include_str!("../../../data/curriculum/g9-ct9-480.draft.v1.json");

fn v2_seed() -> Manifest {
    serde_json::from_value(v2::seed()).expect("the generated v2 seed parses")
}

fn v2_draft() -> Manifest {
    serde_json::from_value(v2::draft()).expect("the generated v2 draft parses")
}

fn ct9_seed() -> Manifest {
    parse_manifest(SEED_V0).expect("the Appendix A seed parses")
}

// ------------------------------------------------------------ valid manifests

#[test]
fn the_generated_v2_manifest_validates_against_the_v2_profile() {
    let m = v2_seed();
    let report = validate(&m);
    assert_eq!(report.errors, Vec::<ManifestError>::new());
    assert_eq!(report.violations, vec![]);
    assert_eq!(report.warnings, Vec::<ManifestWarning>::new());
    assert!(report.is_valid(), "{}", report.summary_line());

    assert_eq!(report.profile, StructureProfileId::MasteryV2);
    assert_eq!(report.invariants, MASTERY_V2.invariants);
    assert_eq!(
        select_profile(&m.structure).id,
        StructureProfileId::MasteryV2
    );
    let c = report.counts;
    assert_eq!(
        (c.weeks, c.days, c.objectives, c.total_minutes),
        (12, 60, 180, 28_800)
    );
    assert_eq!((c.weekday_days, c.saturday_days), (48, 12));

    // Structural facts, independent of the validator.
    assert_eq!(m.manifest_schema_version, MANIFEST_FORMAT_VERSION_1_1);
    assert_eq!(m.structure.assessment_day_of_week, Some(5));
    let ids: Vec<&str> = m.days().map(|(_, d)| d.day_id.as_str()).collect();
    let expected: Vec<String> = (1..=60).map(|i| format!("D{i:02}")).collect();
    assert_eq!(ids, expected);
    for (w, d) in m.days() {
        assert!((1..=5).contains(&d.day_of_week), "{}", d.day_id);
        assert_eq!(d.planned_minutes, 480, "{}", d.day_id);
        assert_eq!(d.is_assessment_day, d.day_of_week == 5, "{}", d.day_id);
        assert_eq!(d.blocks.len(), 5, "{}", d.day_id);
        assert_eq!(d.objectives.len(), 3, "{}", d.day_id);
        assert!((1..=12).contains(&w.week_number));
    }
    // Week w holds D(5w-4) to D(5w).
    assert_eq!(m.weeks[0].days[4].day_id, "D05");
    assert_eq!(m.weeks[1].days[0].day_id, "D06");
    assert_eq!(m.weeks[11].days[4].day_id, "D60");

    // The generator's block lists are the profile's (written out twice, compared here).
    let pairs = |blocks: &[g9_domain::ProfileBlock]| -> Vec<(&'static str, u32)> {
        blocks.iter().map(|b| (b.code, b.minutes)).collect()
    };
    assert_eq!(
        pairs(MASTERY_V2.ordinary_blocks),
        v2::WEEKDAY_BLOCKS.to_vec()
    );
    assert_eq!(
        pairs(MASTERY_V2.assessment_blocks),
        v2::FRIDAY_BLOCKS.to_vec()
    );

    // The draft (enriched) is valid as well, and loads through the one-step entry point.
    let draft = v2_draft();
    let draft_report = validate(&draft);
    assert!(draft_report.is_valid(), "{:?}", draft_report.errors);
    assert_eq!(draft_report.profile, StructureProfileId::MasteryV2);
    let text = serde_json::to_string(&v2::draft()).unwrap();
    let (loaded, loaded_report) = load_manifest(&text).expect("load_manifest accepts the v2 draft");
    assert_eq!(loaded, draft);
    assert_eq!(loaded_report, draft_report);

    // A valid v2 manifest is still only a proposal.
    assert_eq!(m.provenance, ManifestProvenance::ProposedSeed);
    assert!(!m.is_authoritative());
}

#[test]
fn the_appendix_a_shape_is_still_valid_in_either_format() {
    // Format 1.0.0, as shipped.
    let seed = ct9_seed();
    let report = validate(&seed);
    assert!(report.is_valid());
    assert_eq!(report.profile, StructureProfileId::Ct9480);
    assert_eq!(report.invariants, CT9_480.invariants);
    assert_eq!(seed.structure.assessment_day_of_week, None);

    // The same content declared as format 1.1.0: the field is optional there too.
    let mut as_1_1 = ct9_seed();
    as_1_1.manifest_schema_version = "1.1.0".into();
    let report = validate(&as_1_1);
    assert!(report.is_valid(), "{:?}", report.errors);
    assert_eq!(report.profile, StructureProfileId::Ct9480);

    // With the default written out (6 is what its absence means).
    as_1_1.structure.assessment_day_of_week = Some(6);
    let report = validate(&as_1_1);
    assert!(report.is_valid(), "{:?}", report.errors);
    assert_eq!(report.profile, StructureProfileId::Ct9480);
    assert_eq!(report.counts.days, 72);
    assert_ne!(
        manifest_checksum(&as_1_1).unwrap(),
        manifest_checksum(&seed).unwrap(),
        "a written-out field is content: it changes the checksum"
    );
}

// ------------------------------------------------------------ every v2 rule

struct Case {
    name: &'static str,
    mutate: fn(&mut Manifest),
    code: &'static str,
    path: &'static str,
}

fn ct9_weekday_blocks() -> Vec<Block> {
    ct9_seed().weeks[0].days[0].blocks.clone()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "six days in a week",
            mutate: |m| {
                let mut sixth = m.weeks[0].days[4].clone();
                sixth.day_id = "D06".into();
                sixth.day_of_week = 6;
                m.weeks[0].days.push(sixth);
            },
            code: codes::WEEK_DAYS_COUNT,
            path: "/weeks/0/days",
        },
        Case {
            name: "four days in a week",
            mutate: |m| {
                m.weeks[3].days.pop();
            },
            code: codes::WEEK_DAYS_COUNT,
            path: "/weeks/3/days",
        },
        Case {
            name: "a Saturday",
            mutate: |m| m.weeks[0].days[4].day_of_week = 6,
            code: codes::DAY_OF_WEEK_RANGE,
            path: "/weeks/0/days/4/day_of_week",
        },
        Case {
            name: "a Sunday",
            mutate: |m| m.weeks[0].days[4].day_of_week = 7,
            code: codes::DAY_SUNDAY_PRESENT,
            path: "/weeks/0/days/4/day_of_week",
        },
        Case {
            name: "day of week 0",
            mutate: |m| m.weeks[0].days[0].day_of_week = 0,
            code: codes::DAY_OF_WEEK_RANGE,
            path: "/weeks/0/days/0/day_of_week",
        },
        Case {
            name: "days out of order",
            mutate: |m| m.weeks[2].days.swap(1, 2),
            code: codes::DAY_OF_WEEK_ORDER,
            path: "/weeks/2/days/2/day_of_week",
        },
        Case {
            name: "a 420-minute day",
            mutate: |m| m.weeks[0].days[0].planned_minutes = 420,
            code: codes::DAY_PLANNED_MINUTES,
            path: "/weeks/0/days/0/planned_minutes",
        },
        Case {
            name: "a 300-minute assessment day",
            mutate: |m| m.weeks[0].days[4].planned_minutes = 300,
            code: codes::DAY_PLANNED_MINUTES,
            path: "/weeks/0/days/4/planned_minutes",
        },
        Case {
            name: "the assessment flag on a Thursday",
            mutate: |m| m.weeks[0].days[3].is_assessment_day = true,
            code: codes::DAY_ASSESSMENT_FLAG,
            path: "/weeks/0/days/3/is_assessment_day",
        },
        Case {
            name: "no assessment flag on a Friday",
            mutate: |m| m.weeks[5].days[4].is_assessment_day = false,
            code: codes::DAY_ASSESSMENT_FLAG,
            path: "/weeks/5/days/4/is_assessment_day",
        },
        Case {
            name: "the Appendix A block list on a v2 day",
            mutate: |m| m.weeks[0].days[0].blocks = ct9_weekday_blocks(),
            code: codes::DAY_BLOCKS_CANONICAL,
            path: "/weeks/0/days/0/blocks",
        },
        Case {
            name: "the ordinary block list on a Friday",
            mutate: |m| {
                let ordinary = m.weeks[0].days[0].blocks.clone();
                m.weeks[0].days[4].blocks = ordinary;
            },
            code: codes::DAY_BLOCKS_CANONICAL,
            path: "/weeks/0/days/4/blocks",
        },
        Case {
            name: "v2 block minutes changed",
            mutate: |m| m.weeks[0].days[0].blocks[3].minutes = 120,
            code: codes::DAY_BLOCKS_SUM,
            path: "/weeks/0/days/0/blocks",
        },
        Case {
            name: "the Appendix A weekday list at the top",
            mutate: |m| m.weekday_blocks = ct9_seed().weekday_blocks,
            code: codes::WEEKDAY_BLOCKS_CANONICAL,
            path: "/weekday_blocks",
        },
        Case {
            name: "the Appendix A Saturday list at the top",
            mutate: |m| m.saturday_blocks = ct9_seed().saturday_blocks,
            code: codes::SATURDAY_BLOCKS_CANONICAL,
            path: "/saturday_blocks",
        },
        Case {
            name: "the field in a 1.0.0 manifest",
            mutate: |m| m.manifest_schema_version = "1.0.0".into(),
            code: codes::STRUCTURE_ASSESSMENT_DAY_FORMAT,
            path: "/structure/assessment_day_of_week",
        },
        Case {
            name: "an unknown minor version",
            mutate: |m| m.manifest_schema_version = "1.2.0".into(),
            code: codes::SCHEMA_VERSION_UNSUPPORTED,
            path: "/manifest_schema_version",
        },
        Case {
            name: "an unknown major version",
            mutate: |m| m.manifest_schema_version = "2.0.0".into(),
            code: codes::SCHEMA_VERSION_UNSUPPORTED,
            path: "/manifest_schema_version",
        },
        Case {
            name: "a version that is not MAJOR.MINOR.PATCH",
            mutate: |m| m.manifest_schema_version = "1.1".into(),
            code: codes::SCHEMA_VERSION_UNSUPPORTED,
            path: "/manifest_schema_version",
        },
        Case {
            name: "assessment_day_of_week 4",
            mutate: |m| m.structure.assessment_day_of_week = Some(4),
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/assessment_day_of_week",
        },
        Case {
            name: "assessment_day_of_week 6 on a five-day week",
            mutate: |m| m.structure.assessment_day_of_week = Some(6),
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/days_per_week",
        },
        Case {
            name: "the field removed from a five-day week",
            mutate: |m| m.structure.assessment_day_of_week = None,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/days_per_week",
        },
        Case {
            name: "six days a week declared",
            mutate: |m| m.structure.days_per_week = 6,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/days_per_week",
        },
        Case {
            name: "wrong total minutes",
            mutate: |m| m.structure.total_minutes = 28_000,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/total_minutes",
        },
        Case {
            name: "wrong weekly minutes",
            mutate: |m| m.structure.weekly_minutes = 2_100,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/weekly_minutes",
        },
        Case {
            name: "the Appendix A weekday minutes declared",
            mutate: |m| m.structure.weekday_minutes = 420,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/weekday_minutes",
        },
        Case {
            name: "the Appendix A Saturday minutes declared",
            mutate: |m| m.structure.saturday_minutes = 300,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/saturday_minutes",
        },
        Case {
            name: "eleven weeks declared",
            mutate: |m| m.structure.weeks = 11,
            code: codes::STRUCTURE_MISMATCH,
            path: "/structure/weeks",
        },
        Case {
            name: "Sunday scheduled",
            mutate: |m| m.structure.sunday = "optional".into(),
            code: codes::STRUCTURE_SUNDAY,
            path: "/structure/sunday",
        },
        Case {
            name: "the Appendix A day numbering (D07 opens week 2)",
            mutate: |m| m.weeks[1].days[0].day_id = "D07".into(),
            code: codes::DAY_ID_MISMATCH,
            path: "/weeks/1/days/0/day_id",
        },
        Case {
            name: "two objectives on a day",
            mutate: |m| {
                m.weeks[0].days[0].objectives.pop();
            },
            code: codes::DAY_OBJECTIVES_COUNT,
            path: "/weeks/0/days/0/objectives",
        },
        Case {
            name: "eleven weeks",
            mutate: |m| {
                m.weeks.pop();
            },
            code: codes::WEEK_COUNT,
            path: "/weeks",
        },
    ]
}

#[test]
fn each_broken_v2_rule_is_refused_with_its_code_and_path() {
    for case in cases() {
        let mut m = v2_seed();
        (case.mutate)(&mut m);
        let report = validate(&m);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.code == case.code && e.path == case.path),
            "{}: expected `{}` at {}; got {:?}",
            case.name,
            case.code,
            case.path,
            report
                .errors
                .iter()
                .map(|e| format!("{} {}", e.code, e.path))
                .collect::<Vec<_>>()
        );
        assert!(!report.is_valid(), "{}: must block import", case.name);
        // The report maps to the closed category set, as for the Appendix A shape.
        let err: G9Error = report.clone().into_result().unwrap_err().into();
        assert_eq!(err.category, ErrorCategory::InvalidInput, "{}", case.name);
        assert_eq!(err.code, "curriculum.manifest.invalid", "{}", case.name);
        for e in &report.errors {
            assert!(e.path.starts_with('/'), "{}: {e:?}", case.name);
            assert!(
                codes::ALL_ERRORS.contains(&e.code.as_str()),
                "{}: `{}` is not in the code table",
                case.name,
                e.code
            );
        }
    }
}

#[test]
fn a_structure_that_matches_no_profile_is_one_mismatch_not_a_cascade() {
    // The declaration is nearest to the v2 profile: the rest of the manifest is checked
    // against it, so the report names the one field that is wrong.
    let mut m = v2_seed();
    m.structure.assessment_day_of_week = Some(4);
    let report = validate(&m);
    assert_eq!(report.profile, StructureProfileId::MasteryV2);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].code, codes::STRUCTURE_MISMATCH);
    assert_eq!(report.errors[0].path, "/structure/assessment_day_of_week");
    assert_eq!(report.errors[0].message, "expected 5, declared 4");
    assert!(report.violations.is_empty());

    let mut m = v2_seed();
    m.structure.days_per_week = 6;
    let report = validate(&m);
    assert_eq!(report.profile, StructureProfileId::MasteryV2);
    assert_eq!(report.error_codes(), vec![codes::STRUCTURE_MISMATCH]);
    assert_eq!(report.errors[0].message, "expected 5, declared 6");

    // The field in a 1.0.0 manifest: the one error says which format it needs.
    let mut m = v2_seed();
    m.manifest_schema_version = "1.0.0".into();
    let report = validate(&m);
    assert_eq!(
        report.error_codes(),
        vec![codes::STRUCTURE_ASSESSMENT_DAY_FORMAT]
    );
    assert!(report.errors[0].message.contains("1.1"), "{report:?}");

    // An unsupported version is reported as that and nothing else is added for the field.
    let mut m = v2_seed();
    m.manifest_schema_version = "3.0.0".into();
    assert_eq!(
        validate(&m).error_codes(),
        vec![codes::SCHEMA_VERSION_UNSUPPORTED]
    );

    // Without the field a five-day manifest is read as the Appendix A shape and fails it:
    // absent means Saturday, in 1.1.0 as in 1.0.0.
    let mut m = v2_seed();
    m.structure.assessment_day_of_week = None;
    let report = validate(&m);
    assert_eq!(report.profile, StructureProfileId::Ct9480);
    assert!(report.has_error(codes::STRUCTURE_MISMATCH));
    assert!(report.has_error(codes::WEEK_DAYS_COUNT));
    assert!(report.has_error(codes::DAY_PLANNED_MINUTES));
    assert!(!report.violations.is_empty());

    // A v2 manifest is never valid under the Appendix A profile and the reverse.
    assert!(!CT9_480.violations(&validate(&v2_seed()).counts).is_empty());
    assert!(!MASTERY_V2
        .violations(&validate(&ct9_seed()).counts)
        .is_empty());
}

#[test]
fn the_arithmetic_violations_are_the_v2_numbers() {
    let mut m = v2_seed();
    m.weeks[11].days.pop(); // drop D60, the capstone Friday
    let report = validate(&m);
    let got: Vec<(&str, &str, &str)> = report
        .violations
        .iter()
        .map(|v| (v.code, v.expected.as_str(), v.actual.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("curriculum.days", "60", "59"),
            ("curriculum.objectives", "180", "177"),
            ("curriculum.total_minutes", "28800", "28320"),
            ("curriculum.saturday_days", "12", "11"),
        ]
    );
    assert!(report.has_error(codes::WEEK_DAYS_COUNT));

    // A Monday planned as an assessment-length day of the other shape: the day count
    // holds, the minutes do not.
    let mut m = v2_seed();
    m.weeks[0].days[0].planned_minutes = 420;
    let report = validate(&m);
    let codes_: Vec<&str> = report.violations.iter().map(|v| v.code).collect();
    assert_eq!(codes_, vec!["curriculum.total_minutes"]);
    assert_eq!(report.counts.total_minutes, 28_740);

    // The report survives storage: the profile travels with it and the violations
    // are recomputed against that profile.
    let json = serde_json::to_string(&report).unwrap();
    let back: ValidationReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, report);
    assert_eq!(back.profile, StructureProfileId::MasteryV2);
}

// ---------------------------------------------------------------- checksums

/// Checksums of the two manifests in `data/curriculum/`, computed with the code as it was
/// before format 1.1.0 (`g9_curriculum::{manifest_checksum, content_checksum}`, 2026-11-18).
/// `structure.assessment_day_of_week` is absent in both and is not serialized, so the new
/// field must not move them: an approval or a reconciliation block bound to one of these
/// values stays bound.
const PINNED: [(&str, &str, &str, &str); 2] = [
    (
        "g9-ct9-480.seed.v0.json",
        SEED_V0,
        "ccdaefb13b260c27a52a652d42493888c7e9553e845df9d869d7e86f9c5e4ebf",
        "40c16f01d78d3318fe75c9bd4865fc435930a948ddde24e6a60473980c71f0c9",
    ),
    (
        "g9-ct9-480.draft.v1.json",
        DRAFT_V1,
        "cb8595b74a6e500ee5d87451c30bb3a728f734f3a40e26cb5d15ff864b3f16f1",
        "e7e67e7b16a34c0f272eaacd9f5c4568e7e1f2b49c647653523ac050fbb259ac",
    ),
];

/// `reconcile(seed, draft).diff_checksum` of those two files, computed the same way.
const PINNED_SEED_TO_DRAFT_DIFF: &str =
    "6591d562031f6251cb363d84f06b0da7de245f9433d7a4dfd8b72bbe386566e3";

#[test]
fn the_two_existing_manifests_keep_their_checksums() {
    for (name, text, manifest_sum, content_sum) in PINNED {
        let m = parse_manifest(text).unwrap();
        assert_eq!(m.structure.assessment_day_of_week, None, "{name}");
        assert_eq!(
            manifest_checksum(&m).unwrap().as_str(),
            manifest_sum,
            "{name}"
        );
        assert_eq!(
            content_checksum(&m).unwrap().as_str(),
            content_sum,
            "{name}"
        );
        // The file's own canonical form hashes to the same value: reading it into the
        // types and writing it back adds no key.
        assert_eq!(
            json_text_checksum(text).unwrap().as_str(),
            manifest_sum,
            "{name}"
        );
        let written = serde_json::to_string(&m).unwrap();
        assert!(!written.contains("assessment_day_of_week"), "{name}");
        // Byte for byte: the canonical JSON of the typed manifest is that of the file.
        let from_types = g9_domain::hashing::canonical_json(&serde_json::to_value(&m).unwrap());
        let from_file = g9_domain::hashing::canonical_json(
            &serde_json::from_str::<serde_json::Value>(text).unwrap(),
        );
        assert_eq!(from_types, from_file, "{name}");
    }
    let seed = parse_manifest(SEED_V0).unwrap();
    let draft = parse_manifest(DRAFT_V1).unwrap();
    assert_eq!(
        reconcile(&seed, &draft).unwrap().diff_checksum.as_str(),
        PINNED_SEED_TO_DRAFT_DIFF
    );
}

#[test]
fn a_v2_manifest_hashes_its_declared_assessment_day() {
    let m = v2_seed();
    let from_types = manifest_checksum(&m).unwrap();
    let from_value = json_checksum(&v2::seed());
    assert_eq!(
        from_types, from_value,
        "serde round-trip keeps the canonical form of a 1.1.0 manifest"
    );
    assert!(serde_json::to_string(&m)
        .unwrap()
        .contains("\"assessment_day_of_week\":5"));
    // The field is content: dropping it or changing it changes both checksums.
    let mut other = m.clone();
    other.structure.assessment_day_of_week = Some(4);
    assert_ne!(manifest_checksum(&other).unwrap(), from_types);
    assert_ne!(
        content_checksum(&other).unwrap(),
        content_checksum(&m).unwrap()
    );
    other.structure.assessment_day_of_week = None;
    assert_ne!(manifest_checksum(&other).unwrap(), from_types);
    // Relabelling (version, provenance) keeps the content checksum, as for the other shape.
    let mut relabelled = m.clone();
    relabelled.version = "2.0.0-rc1".into();
    assert_eq!(
        content_checksum(&relabelled).unwrap(),
        content_checksum(&m).unwrap()
    );
    assert_ne!(manifest_checksum(&relabelled).unwrap(), from_types);
}

// ------------------------------------------------------------------- plans

#[test]
fn the_daily_plan_of_a_v2_monday_and_friday() {
    let m = v2_seed();
    let monday = daily_plan(&m, 1, 1).unwrap();
    assert_eq!(monday.day_id.as_str(), "D01");
    assert_eq!(monday.curriculum_id.as_str(), v2::CURRICULUM_ID);
    assert_eq!(monday.manifest_version, v2::SEED_VERSION);
    assert_eq!(monday.planned.as_whole_minutes(), 480);
    assert!(!monday.is_assessment_day);
    assert!(!monday.authoritative);
    let names: Vec<&str> = monday.blocks.iter().map(|b| b.block.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "recall",
            "theory",
            "guided_lab",
            "independent_lab",
            "defense_verification"
        ]
    );
    let offsets: Vec<i64> = monday
        .blocks
        .iter()
        .map(|b| b.offset.as_whole_minutes())
        .collect();
    assert_eq!(offsets, vec![0, 30, 150, 270, 420], "study time, no pauses");
    let total: i64 = monday
        .blocks
        .iter()
        .map(|b| b.duration.as_whole_minutes())
        .sum();
    assert_eq!(total, 480);

    let friday = daily_plan(&m, 3, 5).unwrap();
    assert_eq!(friday.day_id.as_str(), "D15");
    assert_eq!(friday.week_title, v2::WEEK_TITLES[2]);
    assert_eq!(friday.planned.as_whole_minutes(), 480);
    assert!(friday.is_assessment_day);
    let names: Vec<&str> = friday.blocks.iter().map(|b| b.block.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "recall",
            "theory",
            "performance_review",
            "blind_practical",
            "report"
        ]
    );
    assert_eq!(daily_plan(&m, 12, 5).unwrap().day_id.as_str(), "D60");

    // No Saturday in this shape; 7 and 0 are outside every shape.
    assert!(matches!(
        daily_plan(&m, 1, 6),
        Err(CurriculumError::DayNotFound {
            week: 1,
            day_of_week: 6
        })
    ));
    let g9: G9Error = daily_plan(&m, 1, 6).unwrap_err().into();
    assert_eq!(g9.category, ErrorCategory::NotFound);
    assert_eq!(g9.code, "curriculum.day.not_found");
    assert!(matches!(
        daily_plan(&m, 1, 7),
        Err(CurriculumError::DayOfWeekOutOfRange(7))
    ));
    assert!(matches!(
        daily_plan(&m, 13, 1),
        Err(CurriculumError::DayNotFound { .. })
    ));

    // An invalid v2 manifest yields no plan at all.
    let mut broken = v2_seed();
    broken.weeks[0].days[0].planned_minutes = 420;
    assert!(matches!(
        daily_plan(&broken, 2, 1),
        Err(CurriculumError::Invalid(_))
    ));

    // Serializable read model, as for the other shape.
    let json = serde_json::to_value(&friday).unwrap();
    assert_eq!(json["day_id"], "D15");
    let back: DailyPlan = serde_json::from_value(json).unwrap();
    assert_eq!(back, friday);
}

#[test]
fn the_v2_timeline_runs_monday_to_friday_for_twelve_weeks() {
    let m = v2_seed();
    let start = CivilDate::new(2026, 1, 5).unwrap(); // a Monday
    let t = program_timeline(&m, start).unwrap();
    assert_eq!(t.days.len(), 60);
    assert_eq!(t.days[0].day_id.as_str(), "D01");
    assert_eq!(t.days[0].date, start);
    assert_eq!(t.days[4].day_id.as_str(), "D05");
    assert_eq!(t.days[4].offset_days, 4);
    assert_eq!(t.days[4].date, CivilDate::new(2026, 1, 9).unwrap());
    assert!(t.days[4].is_assessment_day);
    // Week 2 opens on the next Monday: the weekend is skipped.
    assert_eq!(t.days[5].day_id.as_str(), "D06");
    assert_eq!(t.days[5].offset_days, 7);
    assert_eq!(t.days[5].date, CivilDate::new(2026, 1, 12).unwrap());
    // Day 30 (the midpoint) and day 60 (the final) are Fridays.
    assert_eq!(t.days[29].day_id.as_str(), "D30");
    assert_eq!(t.days[29].date.iso_weekday(), 5);
    assert!(t.days[29].is_assessment_day);
    let last = &t.days[59];
    assert_eq!(last.day_id.as_str(), "D60");
    assert_eq!(last.offset_days, 7 * 11 + 4);
    assert_eq!(last.date, CivilDate::new(2026, 3, 27).unwrap());
    assert_eq!(last.date.iso_weekday(), 5);
    assert_eq!(t.end, last.date);
    assert_eq!(t.span_days, 82);
    assert!(
        t.days.iter().all(|d| d.offset_days % 7 < 5),
        "no Saturday or Sunday offsets"
    );
    assert!(t
        .days
        .iter()
        .all(|d| d.date.iso_weekday() == d.day_of_week.get()));
    assert_eq!(t.days.iter().filter(|d| d.is_assessment_day).count(), 12);
    let total: i64 = t.days.iter().map(|d| d.planned.as_whole_minutes()).sum();
    assert_eq!(total, 28_800);

    assert_eq!(
        program_timeline(&m, CivilDate::new(2026, 1, 6).unwrap()).unwrap_err(),
        CurriculumError::StartNotMonday { weekday: 2 }
    );
}

// --------------------------------------------------------------- reconcile

/// The only per-day fields in which a draft may differ from its seed.
fn is_enrichment_field(field: &str) -> bool {
    field == "evidence_artifact"
        || field == "resources"
        || (0..3).any(|i| {
            field == format!("objectives[{i}].text")
                || field == format!("objectives[{i}].competency_ids")
        })
}

#[test]
fn reconcile_of_a_v2_seed_and_an_enriched_v2_draft() {
    use ManifestProvenance::*;
    let (seed, draft) = (v2_seed(), v2_draft());

    // The draft keeps each seed text as the start of its field.
    for ((_, sd), (_, dd)) in seed.days().zip(draft.days()) {
        assert_eq!(sd.day_id, dd.day_id);
        assert_eq!(sd.blocks, dd.blocks, "{}", sd.day_id);
        for (so, dobj) in sd.objectives.iter().zip(&dd.objectives) {
            assert!(
                dobj.text.starts_with(&format!("{}{}", so.text, v2::ADDED)),
                "{}",
                so.objective_id
            );
        }
        assert!(dd.evidence_artifact.starts_with(&format!(
            "{}{}",
            sd.evidence_artifact,
            v2::ADDED
        )));
    }

    let report = reconcile(&seed, &draft).unwrap();
    assert!(report.curriculum_id_matches);
    assert_eq!(report.original_version, v2::SEED_VERSION);
    assert_eq!(report.candidate_version, v2::DRAFT_VERSION);
    assert_eq!(report.per_day.len(), 60);
    assert_eq!(report.summary.changed, 60);
    assert_eq!(report.summary.unchanged, 0);
    assert_eq!(report.summary.missing_in_candidate, 0);
    assert_eq!(report.summary.added_in_candidate, 0);
    assert!(
        report.summary.metadata_changes.is_empty(),
        "same name, same structure (the assessment day included), same block lists: {:?}",
        report.summary.metadata_changes
    );
    for d in &report.per_day {
        let DayDiffStatus::Changed { fields } = &d.status else {
            panic!("{} is not CHANGED", d.day_id);
        };
        assert!(
            fields.iter().all(|f| is_enrichment_field(f)),
            "{}: {fields:?}",
            d.day_id
        );
    }
    assert!(report.human_readable.contains("60 changed"));

    // Promotion is blocked until the diff is reviewed, exactly as for the other shape.
    assert_eq!(report.binding, ReconciliationBinding::Absent);
    assert!(!report.promotion_allowed);
    assert_eq!(
        can_promote(&report),
        Err(PromotionBlocked::UnreconciledDifferences {
            changed: 60,
            missing: 0,
            added: 0,
            metadata: 0,
        })
    );

    let mut reviewed = draft.clone();
    reviewed.provenance = ReconciledCandidate;
    reviewed.version = "2.0.0-rc1".into();
    reviewed.reconciliation = Some(Reconciliation {
        original_checksum: report.original_checksum.as_str().to_string(),
        diff_checksum: report.diff_checksum.as_str().to_string(),
        approved_by: "operator".to_string(),
        approved_at: UtcMillis::from_millis(1_763_000_000_000),
    });
    assert!(validate(&reviewed).is_valid());
    let bound = reconcile(&seed, &reviewed).unwrap();
    assert_eq!(bound.diff_checksum, report.diff_checksum);
    assert_eq!(bound.binding, ReconciliationBinding::Matched);
    assert!(bound.promotion_allowed);
    assert_eq!(
        check_transition(ReconciledCandidate, ApprovedCanonical, Some(&bound)),
        Ok(())
    );

    // An edit after the review is caught.
    let mut edited = reviewed.clone();
    edited.weeks[6].days[4].objectives[0]
        .text
        .push_str(" (edited)");
    let r = reconcile(&seed, &edited).unwrap();
    assert!(matches!(
        r.binding,
        ReconciliationBinding::DiffChecksumMismatch { .. }
    ));
    assert!(!r.promotion_allowed);

    // Identical manifests: sixty unchanged days.
    let same = reconcile(&seed, &seed).unwrap();
    assert_eq!(same.summary.unchanged, 60);
    assert!(!same.summary.has_content_diffs());
    assert!(same.promotion_allowed);
}

#[test]
fn reconcile_compares_the_declared_assessment_day() {
    let seed = v2_seed();

    // Only the new field differs: it is a structure change, reported as one.
    let mut other_day = v2_seed();
    other_day.structure.assessment_day_of_week = Some(4);
    let r = reconcile(&seed, &other_day).unwrap();
    assert_eq!(r.summary.metadata_changes, vec!["structure".to_string()]);
    assert_eq!(r.summary.unchanged, 60);
    assert!(r.summary.has_content_diffs());
    assert!(!r.promotion_allowed);
    assert!(r.human_readable.contains("metadata changes: structure"));

    let mut without = v2_seed();
    without.structure.assessment_day_of_week = None;
    let r = reconcile(&seed, &without).unwrap();
    assert_eq!(r.summary.metadata_changes, vec!["structure".to_string()]);
    assert_ne!(r.original_checksum, r.candidate_checksum);

    // The two shapes against each other: another curriculum, another structure, and the
    // days that share an id differ; never promotable.
    let r = reconcile(&ct9_seed(), &seed).unwrap();
    assert!(!r.curriculum_id_matches);
    assert!(r.summary.metadata_changes.iter().any(|f| f == "structure"));
    assert!(r
        .summary
        .metadata_changes
        .iter()
        .any(|f| f == "weekday_blocks"));
    assert_eq!(r.summary.missing_in_candidate, 12, "D61 to D72");
    assert_eq!(r.summary.unchanged, 0);
    assert!(!r.promotion_allowed);
    assert_eq!(can_promote(&r), Err(PromotionBlocked::CurriculumIdMismatch));
}
