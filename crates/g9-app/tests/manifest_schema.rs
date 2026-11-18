#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `schemas/curriculum/manifest.schema.json` against real manifests of both structure profiles
//! (ADR-0023): the two manifests that exist (Appendix A seed and draft, format 1.0.0) and the
//! generated v2 manifests (`MASTERY_V2`, format 1.1.0) are valid, and each rule the schema can
//! express rejects the manifest that breaks it. Cross-field rules (day ids in order, block sums,
//! canonical block lists) are the validator's (`g9_curriculum::validate`), not the schema's.

#[path = "../../g9-curriculum/tests/common/mastery_v2.rs"]
mod v2;

use serde_json::{json, Value};

const SCHEMA: &str = include_str!("../../../schemas/curriculum/manifest.schema.json");
const SEED_V0: &str = include_str!("../../../data/curriculum/g9-ct9-480.seed.v0.json");
const DRAFT_V1: &str = include_str!("../../../data/curriculum/g9-ct9-480.draft.v1.json");

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    jsonschema::validator_for(&schema).expect("the manifest schema compiles")
}

fn accepts(v: &jsonschema::Validator, manifest: &Value) -> Result<(), Vec<String>> {
    let errors: Vec<String> = v
        .iter_errors(manifest)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn the_existing_and_the_generated_manifests_are_valid() {
    let v = validator();
    for (name, manifest) in [
        ("Appendix A seed", json_of(SEED_V0)),
        ("Appendix A draft", json_of(DRAFT_V1)),
        ("v2 seed", v2::seed()),
        ("v2 draft", v2::draft()),
    ] {
        assert_eq!(accepts(&v, &manifest), Ok(()), "{name}");
    }
    // The Appendix A shape in format 1.1.0, with the default written out or left out.
    let mut as_1_1 = json_of(SEED_V0);
    as_1_1["manifest_schema_version"] = json!("1.1.0");
    assert_eq!(accepts(&v, &as_1_1), Ok(()));
    as_1_1["structure"]["assessment_day_of_week"] = json!(6);
    assert_eq!(accepts(&v, &as_1_1), Ok(()));
}

#[test]
fn each_rule_the_schema_expresses_rejects_the_manifest_that_breaks_it() {
    let v = validator();
    type Mutation = fn(&mut Value);
    let cases: Vec<(&str, bool, Mutation)> = vec![
        // (name, mutate the v2 seed (true) or the Appendix A seed (false))
        ("the field in a 1.0.0 manifest", true, |m| {
            m["manifest_schema_version"] = json!("1.0.0")
        }),
        ("an unknown minor version", true, |m| {
            m["manifest_schema_version"] = json!("1.2.0")
        }),
        ("an unknown major version", true, |m| {
            m["manifest_schema_version"] = json!("2.0.0")
        }),
        ("assessment_day_of_week 4", true, |m| {
            m["structure"]["assessment_day_of_week"] = json!(4)
        }),
        ("assessment_day_of_week 7", true, |m| {
            m["structure"]["assessment_day_of_week"] = json!(7)
        }),
        ("a v2 manifest with Appendix A weekday minutes", true, |m| {
            m["structure"]["weekday_minutes"] = json!(420)
        }),
        ("a v2 manifest of six days a week", true, |m| {
            m["structure"]["days_per_week"] = json!(6)
        }),
        ("a v2 manifest with an Appendix A block list", true, |m| {
            m["weekday_blocks"] = json!([
                { "block": "recall", "minutes": 15 },
                { "block": "theory", "minutes": 45 },
                { "block": "guided_lab", "minutes": 120 },
                { "block": "independent_lab", "minutes": 120 },
                { "block": "defense_verification", "minutes": 60 },
                { "block": "evidence_reporting", "minutes": 60 },
            ])
        }),
        ("a v2 week of six days", true, |m| {
            let mut sixth = m["weeks"][0]["days"][4].clone();
            sixth["day_id"] = json!("D06");
            sixth["day_of_week"] = json!(6);
            m["weeks"][0]["days"].as_array_mut().unwrap().push(sixth);
        }),
        ("a v2 Saturday", true, |m| {
            m["weeks"][0]["days"][4]["day_of_week"] = json!(6)
        }),
        ("a v2 day of 420 minutes", true, |m| {
            m["weeks"][0]["days"][0]["planned_minutes"] = json!(420)
        }),
        ("a v2 day id past D60", true, |m| {
            m["weeks"][11]["days"][4]["day_id"] = json!("D61")
        }),
        ("an unknown key in structure", true, |m| {
            m["structure"]["assessment_day"] = json!(5)
        }),
        ("an Appendix A week of five days", false, |m| {
            m["weeks"][0]["days"].as_array_mut().unwrap().pop();
        }),
        ("an Appendix A weekday of 480 minutes", false, |m| {
            m["weeks"][0]["days"][0]["planned_minutes"] = json!(480)
        }),
        ("Appendix A weekday minutes declared as 480", false, |m| {
            m["structure"]["weekday_minutes"] = json!(480)
        }),
        (
            "an Appendix A manifest declaring the other assessment day",
            false,
            |m| {
                m["manifest_schema_version"] = json!("1.1.0");
                m["structure"]["assessment_day_of_week"] = json!(5);
            },
        ),
    ];
    for (name, on_v2, mutate) in cases {
        let mut manifest = if on_v2 { v2::seed() } else { json_of(SEED_V0) };
        mutate(&mut manifest);
        assert!(
            accepts(&v, &manifest).is_err(),
            "{name}: the schema accepted a manifest that breaks the rule"
        );
    }
}
