//! Test generator: a valid manifest of the `MASTERY_V2` structure profile
//! (ADR-0023; manifest format 1.1.0): 12 weeks of five days, `D01` to `D60`,
//! three objectives a day, 480 minutes a day in five blocks, Friday assessed.
//!
//! The numbers and block lists are written out here, not read from
//! `g9_domain`, so that a test which validates this manifest checks the
//! application's constants against the ADR table and not against themselves.
//! The texts are generated placeholders ("W03 D2 objective 1 ..."): this is a
//! structural fixture, not the owner's syllabus, which is generated elsewhere
//! (`scripts/build_curriculum_v2.py`).
//!
//! Shared by the tests of several crates through `#[path]` (it needs only
//! `serde_json`): `crates/g9-curriculum/tests/mastery_v2.rs`,
//! `crates/g9-storage/src/repos/curriculum.rs` (unit tests),
//! `crates/g9-app/tests/mastery_v2.rs` and `apps/cli/tests/mastery_v2.rs`.
#![allow(dead_code)]

use serde_json::{json, Value};

/// Curriculum id of the owner's v2.0 plan.
pub const CURRICULUM_ID: &str = "g9-cyber-mastery-480";
/// Version label of the generated seed.
pub const SEED_VERSION: &str = "2.0.0-seed";
/// Version label of the generated (enriched) draft.
pub const DRAFT_VERSION: &str = "2.0.0-draft1";
/// The separator between a seed text and what a draft adds to it.
pub const ADDED: &str = " — ";

/// Blocks of an ordinary day (Monday to Thursday): 480 minutes.
pub const WEEKDAY_BLOCKS: [(&str, u32); 5] = [
    ("recall", 30),
    ("theory", 120),
    ("guided_lab", 120),
    ("independent_lab", 150),
    ("defense_verification", 60),
];

/// Blocks of the assessment day (Friday): 480 minutes.
pub const FRIDAY_BLOCKS: [(&str, u32); 5] = [
    ("recall", 30),
    ("theory", 120),
    ("performance_review", 120),
    ("blind_practical", 150),
    ("report", 60),
];

/// Week themes (placeholders in the order of the plan's sequence).
pub const WEEK_TITLES: [&str; 12] = [
    "Linux and the lab",
    "Networking",
    "Windows and Active Directory",
    "Web I",
    "Web II",
    "Infrastructure",
    "SOC",
    "DFIR",
    "Malware and reverse engineering",
    "Advanced Active Directory",
    "Cloud and AI",
    "Capstone",
];

fn block_specs(blocks: &[(&str, u32)]) -> Value {
    Value::Array(
        blocks
            .iter()
            .map(|(name, minutes)| json!({ "block": name, "minutes": minutes }))
            .collect(),
    )
}

fn day_blocks(blocks: &[(&str, u32)]) -> Value {
    Value::Array(
        blocks
            .iter()
            .enumerate()
            .map(|(i, (name, minutes))| json!({ "seq": i + 1, "block": name, "minutes": minutes }))
            .collect(),
    )
}

/// `D01` .. `D60`: week `w` (1-based) holds `D(5w-4)` .. `D(5w)`.
pub fn day_id(week: usize, day_of_week: usize) -> String {
    format!("D{:02}", (week - 1) * 5 + day_of_week)
}

/// A valid seed-like manifest of the v2 shape: no resources, no competency links.
pub fn manifest(curriculum_id: &str, version: &str) -> Value {
    let weeks: Vec<Value> = (1..=12usize)
        .map(|w| {
            let days: Vec<Value> = (1..=5usize)
                .map(|d| {
                    let id = day_id(w, d);
                    let friday = d == 5;
                    let objectives: Vec<Value> = (1..=3usize)
                        .map(|o| {
                            json!({
                                "seq": o,
                                "objective_id": format!("{id}-O{o}"),
                                "text": format!("W{w:02} D{d} objective {o} of {id}"),
                            })
                        })
                        .collect();
                    json!({
                        "day_id": id,
                        "day_of_week": d,
                        "planned_minutes": 480,
                        "objectives": objectives,
                        "evidence_artifact": format!("Evidence of {id}"),
                        "blocks": day_blocks(if friday { &FRIDAY_BLOCKS } else { &WEEKDAY_BLOCKS }),
                        "is_assessment_day": friday,
                    })
                })
                .collect();
            json!({ "week_number": w, "title": WEEK_TITLES[w - 1], "days": days })
        })
        .collect();
    json!({
        "manifest_schema_version": "1.1.0",
        "curriculum_id": curriculum_id,
        "name": "G9 Cybersecurity Mastery v2.0 (480 hours)",
        "version": version,
        "provenance": "PROPOSED_SEED",
        "provenance_note": "PROPOSED: generated test fixture of the MASTERY_V2 shape; not the owner's syllabus.",
        "structure": {
            "weeks": 12,
            "days_per_week": 5,
            "objectives_per_day": 3,
            "weekday_minutes": 480,
            "saturday_minutes": 480,
            "weekly_minutes": 2400,
            "total_minutes": 28800,
            "sunday": "unscheduled",
            "assessment_day_of_week": 5,
        },
        "weekday_blocks": block_specs(&WEEKDAY_BLOCKS),
        "saturday_blocks": block_specs(&FRIDAY_BLOCKS),
        "weeks": weeks,
    })
}

/// The generated seed (`g9-cyber-mastery-480`, `2.0.0-seed`).
pub fn seed() -> Value {
    manifest(CURRICULUM_ID, SEED_VERSION)
}

/// The generated draft: the seed with each objective text and each evidence
/// artifact kept as the start of its field and a criterion added after
/// [`ADDED`], one competency link on the first objective of a day, and two
/// resources a day (one external with a version, one internal).
pub fn draft() -> Value {
    let mut m = manifest(CURRICULUM_ID, DRAFT_VERSION);
    let weeks = m["weeks"].as_array_mut().into_iter().flatten();
    for (wi, week) in weeks.enumerate() {
        let days = week["days"].as_array_mut().into_iter().flatten();
        for (di, day) in days.enumerate() {
            let number = wi * 5 + di + 1;
            let objectives = day["objectives"].as_array_mut().into_iter().flatten();
            for (oi, objective) in objectives.enumerate() {
                let text = objective["text"].as_str().unwrap_or_default().to_string();
                objective["text"] = json!(format!("{text}{ADDED}shown by an observable result"));
                if oi == 0 {
                    objective["competency_ids"] = json!(["lab.authorization"]);
                }
            }
            let artifact = day["evidence_artifact"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            day["evidence_artifact"] = json!(format!(
                "{artifact}{ADDED}with the commands that produced it"
            ));
            day["resources"] = json!([
                {
                    "ref": format!("https://example.invalid/labs/day-{number:02}"),
                    "kind": "EXTERNAL",
                    "version": "2026-11",
                    "license_note": "placeholder of a test fixture",
                },
                {
                    "ref": format!("docs/curriculum/v2/WEEKS/W{:02}/DAY_{number:02}.md", wi + 1),
                    "kind": "INTERNAL",
                },
            ]);
        }
    }
    m
}
