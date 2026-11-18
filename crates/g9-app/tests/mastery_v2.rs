#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The second curriculum shape (ADR-0023, structure profile `MASTERY_V2`: 60 weekdays of 480
//! minutes, a scored Friday) through the command pipeline, next to the Appendix A shape:
//! `curriculum.validate`, `curriculum.import`, `curriculum.promote`, `curriculum.program`,
//! `curriculum.daily_plan` and `session.plan_day`.
//!
//! The v2 manifests are generated (`g9-curriculum/tests/common/mastery_v2.rs`): a structural
//! fixture with placeholder texts, not the owner's syllabus. Nothing here approves or promotes
//! anything outside the test database.

mod common;
#[path = "../../g9-curriculum/tests/common/mastery_v2.rs"]
mod v2;

use common::{seed_manifest, Harness, Opts, T0};
use g9_domain::{ErrorCategory, IanaZone, UtcMillis};
use serde_json::{json, Value};

/// 2026-11-02T00:00:00Z, the Monday the programs of these tests start on.
const MONDAY_MIDNIGHT: i64 = 1_793_577_600_000;
const MINUTE: i64 = 60_000;

fn import_v2(h: &mut Harness) -> Value {
    h.approved(
        "curriculum.import",
        json!({ "manifest": v2::draft(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    )
}

fn import_ct9(h: &mut Harness) -> Value {
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    )
}

fn v2_program() -> Harness {
    let mut h = Harness::new();
    import_v2(&mut h);
    h
}

fn set_start(h: &mut Harness, date: &str, day_start: &str) -> Value {
    h.call(
        "curriculum.set_program_start",
        json!({ "date": date, "day_start": day_start }),
    )
    .unwrap()
}

fn program(h: &mut Harness) -> Value {
    h.call("curriculum.program", json!({})).unwrap()["result"].clone()
}

fn plan(h: &mut Harness, date: &str) -> Value {
    h.call("curriculum.daily_plan", json!({ "date": date }))
        .unwrap()["result"]
        .clone()
}

fn field(blocks: &[Value], key: &str) -> Vec<String> {
    blocks
        .iter()
        .map(|b| b[key].as_str().unwrap().to_string())
        .collect()
}

fn minutes_of(blocks: &[Value]) -> Vec<i64> {
    blocks
        .iter()
        .map(|b| b["minutes"].as_i64().unwrap())
        .collect()
}

fn approval_requests(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM approval_requests", [], |r| r.get(0))
        .unwrap()
}

// ------------------------------------------------------------------ validate

/// The six blocks of an Appendix A weekday, as manifest JSON.
fn appendix_a_weekday_blocks() -> Value {
    json!([
        { "seq": 1, "block": "recall", "minutes": 15 },
        { "seq": 2, "block": "theory", "minutes": 45 },
        { "seq": 3, "block": "guided_lab", "minutes": 120 },
        { "seq": 4, "block": "independent_lab", "minutes": 120 },
        { "seq": 5, "block": "defense_verification", "minutes": 60 },
        { "seq": 6, "block": "evidence_reporting", "minutes": 60 },
    ])
}

#[allow(clippy::type_complexity)]
fn broken_rules() -> Vec<(&'static str, fn(&mut Value), &'static str)> {
    vec![
        (
            "six days in a week",
            |m| {
                let mut sixth = m["weeks"][0]["days"][4].clone();
                sixth["day_id"] = json!("D06");
                sixth["day_of_week"] = json!(6);
                m["weeks"][0]["days"].as_array_mut().unwrap().push(sixth);
            },
            "week.days.count",
        ),
        (
            "a Saturday",
            |m| m["weeks"][0]["days"][4]["day_of_week"] = json!(6),
            "day.day_of_week.out_of_range",
        ),
        (
            "a Sunday",
            |m| m["weeks"][0]["days"][4]["day_of_week"] = json!(7),
            "day.sunday_present",
        ),
        (
            "a 420-minute day",
            |m| m["weeks"][0]["days"][0]["planned_minutes"] = json!(420),
            "day.planned_minutes.mismatch",
        ),
        (
            "the assessment flag on a Thursday",
            |m| m["weeks"][0]["days"][3]["is_assessment_day"] = json!(true),
            "day.assessment_flag.mismatch",
        ),
        (
            "the Appendix A block list on a v2 day",
            |m| m["weeks"][0]["days"][0]["blocks"] = appendix_a_weekday_blocks(),
            "day.blocks.canonical_mismatch",
        ),
        (
            "the field in a 1.0.0 manifest",
            |m| m["manifest_schema_version"] = json!("1.0.0"),
            "structure.assessment_day_of_week.requires_format_1_1",
        ),
        (
            "an unknown format version",
            |m| m["manifest_schema_version"] = json!("1.2.0"),
            "manifest.schema_version.unsupported",
        ),
        (
            "assessment_day_of_week 4",
            |m| m["structure"]["assessment_day_of_week"] = json!(4),
            "structure.mismatch",
        ),
        (
            "wrong total minutes",
            |m| m["structure"]["total_minutes"] = json!(28_000),
            "structure.mismatch",
        ),
        (
            "the Appendix A day count declared",
            |m| m["structure"]["days_per_week"] = json!(6),
            "structure.mismatch",
        ),
    ]
}

#[test]
fn validate_accepts_the_v2_shape_and_names_each_rule_it_breaks() {
    let mut h = Harness::new();
    let manifest = v2::seed();
    let out = h
        .call("curriculum.validate", json!({ "manifest": manifest }))
        .unwrap();
    let r = &out["result"];
    assert_eq!(r["valid"], true, "{r}");
    assert_eq!(r["violations"], json!([]));
    assert_eq!(r["provenance"], "PROPOSED_SEED");
    assert_eq!(
        r["checksum_sha256"],
        g9_curriculum::json_checksum(&manifest).as_str()
    );
    // The enriched draft validates as well.
    let draft = h
        .call("curriculum.validate", json!({ "manifest": v2::draft() }))
        .unwrap();
    assert_eq!(draft["result"]["valid"], true);

    for (name, mutate, code) in broken_rules() {
        let mut m = v2::seed();
        mutate(&mut m);
        let out = h
            .call("curriculum.validate", json!({ "manifest": m }))
            .unwrap();
        let r = &out["result"];
        assert_eq!(r["valid"], false, "{name}");
        let codes: Vec<&str> = r["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["code"].as_str().unwrap())
            .collect();
        assert!(
            codes.contains(&code),
            "{name}: expected {code}, got {codes:?}"
        );
    }

    // A value that does not fit the field is a parse error, reported as one.
    let mut m = v2::seed();
    m["structure"]["assessment_day_of_week"] = json!(300);
    let out = h
        .call("curriculum.validate", json!({ "manifest": m }))
        .unwrap();
    assert_eq!(out["result"]["valid"], false);
    assert_eq!(
        out["result"]["violations"][0]["code"],
        "curriculum.manifest_unparseable"
    );
    assert!(out["result"]["violations"][0]["message"]
        .as_str()
        .unwrap()
        .contains("v1.1.0"));

    // The Appendix A seed is still valid, and still not a v2 manifest.
    let ct9 = h
        .call(
            "curriculum.validate",
            json!({ "manifest": seed_manifest() }),
        )
        .unwrap();
    assert_eq!(ct9["result"]["valid"], true);
    let mut ct9_with_field = seed_manifest();
    ct9_with_field["structure"]["assessment_day_of_week"] = json!(6);
    let refused = h
        .call("curriculum.validate", json!({ "manifest": ct9_with_field }))
        .unwrap();
    assert_eq!(refused["result"]["valid"], false);
    assert_eq!(
        refused["result"]["violations"][0]["code"],
        "structure.assessment_day_of_week.requires_format_1_1"
    );
}

// -------------------------------------------------------------------- import

#[test]
fn a_v2_manifest_imports_with_an_approval_and_is_stored_under_its_profile() {
    let mut h = Harness::new();
    // An invalid v2 manifest is refused before an approval is asked for.
    let mut bad = v2::draft();
    bad["weeks"][0]["days"][0]["planned_minutes"] = json!(420);
    let e = h
        .call(
            "curriculum.import",
            json!({ "manifest": bad, "provenance": "PROPOSED_SEED" }),
        )
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::InvalidInput);
    assert_eq!(e.code, "curriculum.invariant_violated");
    let violations = e.details.as_ref().unwrap()["violations"]
        .as_array()
        .unwrap();
    assert!(violations
        .iter()
        .any(|v| v["code"] == "day.planned_minutes.mismatch"));
    assert_eq!(approval_requests(&h), 0);

    let ok = import_v2(&mut h);
    let r = &ok["result"];
    assert_eq!(r["weeks"], 12);
    assert_eq!(r["days"], 60);
    assert_eq!(r["objectives"], 180);
    assert_eq!(r["provenance"], "PROPOSED_SEED");
    assert_eq!(
        r["curriculum_version_id"],
        format!("{}:{}", v2::CURRICULUM_ID, v2::DRAFT_VERSION)
    );
    let manifest: g9_curriculum::Manifest = serde_json::from_value(v2::draft()).unwrap();
    assert_eq!(
        r["checksum_sha256"],
        g9_curriculum::manifest_checksum(&manifest)
            .unwrap()
            .as_str()
    );

    // Stored as proposed, under the profile the manifest declares.
    let (provenance, is_current, profile, days): (String, i64, String, i64) = h
        .conn()
        .query_row(
            "SELECT provenance, is_current, json_extract(invariants_json, '$.profile'),
                    json_extract(invariants_json, '$.invariants.total_days')
               FROM curriculum_versions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (provenance.as_str(), is_current, profile.as_str(), days),
        ("PROPOSED_SEED", 0, "MASTERY_V2", 60)
    );
    let rows: Vec<(String, i64)> = ["training_days", "daily_objectives", "study_blocks"]
        .iter()
        .map(|t| {
            (
                (*t).to_string(),
                h.conn()
                    .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("training_days".to_string(), 60),
            ("daily_objectives".to_string(), 180),
            ("study_blocks".to_string(), 300)
        ]
    );

    // The same version cannot be imported twice; APPROVED_CANONICAL is never an import.
    let e = h
        .call(
            "curriculum.import",
            json!({ "manifest": v2::draft(), "provenance": "PROPOSED_SEED" }),
        )
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::Conflict);
    assert_eq!(e.code, "curriculum.version_exists");
    let e = h
        .call(
            "curriculum.import",
            json!({ "manifest": v2::draft(), "provenance": "APPROVED_CANONICAL" }),
        )
        .unwrap_err();
    assert_eq!(e.code, "ipc.payload_invalid");
}

fn reviewed_candidate(seed: &Value, draft: &Value) -> Value {
    let mut candidate = draft.clone();
    candidate["version"] = json!("2.0.0-rc1");
    candidate["provenance"] = json!("RECONCILED_CANDIDATE");
    candidate["reconciliation"] = json!({
        "original_checksum": "0".repeat(64),
        "diff_checksum": "0".repeat(64),
        "approved_by": "operator-1",
        "approved_at": T0,
    });
    let seed_m: g9_curriculum::Manifest = serde_json::from_value(seed.clone()).unwrap();
    let cand_m: g9_curriculum::Manifest = serde_json::from_value(candidate.clone()).unwrap();
    let report = g9_curriculum::reconcile(&seed_m, &cand_m).unwrap();
    assert_eq!(report.summary.changed, 60);
    candidate["reconciliation"]["original_checksum"] = json!(report.original_checksum.as_str());
    candidate["reconciliation"]["diff_checksum"] = json!(report.diff_checksum.as_str());
    candidate
}

#[test]
fn a_v2_candidate_reconciled_against_its_seed_is_promoted_with_an_approval() {
    let mut h = Harness::new();
    let (seed, draft) = (v2::seed(), v2::draft());
    let candidate = reviewed_candidate(&seed, &draft);

    // The unreviewed draft carries no reconciliation: it cannot be promoted, whatever it is.
    let imported = h.approved(
        "curriculum.import",
        json!({ "manifest": candidate, "provenance": "RECONCILED_CANDIDATE" }),
        &Opts::default(),
    );
    let version_id = imported["result"]["curriculum_version_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(version_id, "g9-cyber-mastery-480:2.0.0-rc1");

    // Promotion against a different original is blocked before an approval is asked for.
    let mut other = seed.clone();
    other["weeks"][0]["days"][0]["objectives"][0]["text"] = json!("a different objective text");
    let before = approval_requests(&h);
    let blocked = h
        .call(
            "curriculum.promote",
            json!({ "curriculum_version_id": version_id, "candidate_manifest": candidate, "original_manifest": other }),
        )
        .unwrap_err();
    assert_eq!(blocked.category, ErrorCategory::PreconditionFailed);
    assert_eq!(blocked.code, "curriculum.promotion_blocked");
    assert_eq!(approval_requests(&h), before);

    let promoted = h.approved(
        "curriculum.promote",
        json!({ "curriculum_version_id": version_id, "candidate_manifest": candidate, "original_manifest": seed }),
        &Opts::default(),
    );
    let r = &promoted["result"];
    assert_eq!(r["provenance"], "APPROVED_CANONICAL");
    assert_eq!(r["promotion_allowed"], true);
    assert_eq!(r["summary"]["changed"], 60);
    assert_eq!(r["summary"]["unchanged"], 0);
    assert_eq!(r["summary"]["metadata_changes"], json!([]));

    // It is now the current version, and the plan no longer says it is a proposal.
    let p = program(&mut h);
    assert_eq!(p["curriculum"]["provenance"], "APPROVED_CANONICAL");
    assert_eq!(p["curriculum"]["profile"], "MASTERY_V2");
    set_start(&mut h, "2026-11-02", "08:00");
    let out = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-06" }))
        .unwrap();
    assert_eq!(out["result"]["is_assessment_day"], true);
    let warnings = out.get("warnings").and_then(Value::as_array);
    assert!(
        warnings.is_none_or(|w| w.is_empty()),
        "canonical: nothing to label, got {warnings:?}"
    );
}

// ------------------------------------------------------------------- program

#[test]
fn the_program_reports_its_structure_profile_and_days_a_week() {
    let mut h = Harness::new();
    let before = program(&mut h);
    assert_eq!(before["curriculum"], Value::Null);

    import_v2(&mut h);
    let after = program(&mut h);
    let c = &after["curriculum"];
    assert_eq!(c["profile"], "MASTERY_V2");
    assert_eq!(c["days_per_week"], 5);
    assert_eq!(c["weeks"], 12);
    assert_eq!(c["days"], 60);
    assert_eq!(c["objectives"], 180);
    assert_eq!(c["planned_minutes"], 28_800);
    assert_eq!(c["version_label"], v2::DRAFT_VERSION);
    assert_eq!(c["provenance"], "PROPOSED_SEED");
    assert_eq!(
        c["version_id"],
        format!("{}:{}", v2::CURRICULUM_ID, v2::DRAFT_VERSION)
    );
    assert_eq!(c["name"], "G9 Cybersecurity Mastery v2.0 (480 hours)");
    assert_eq!(after["program_start"], Value::Null);

    let mut ct9 = Harness::new();
    import_ct9(&mut ct9);
    let c = &program(&mut ct9)["curriculum"];
    assert_eq!(c["profile"], "CT9_480");
    assert_eq!(c["days_per_week"], 6);
    assert_eq!(
        (c["days"].as_i64(), c["objectives"].as_i64()),
        (Some(72), Some(216))
    );
    assert_eq!(c["planned_minutes"], 28_800);
}

// ---------------------------------------------------------------- daily plan

const V2_ORDINARY_LABELS: [&str; 5] = [
    "Active recall",
    "Core lesson",
    "Tool practice",
    "Independent challenge",
    "Defense and documentation",
];
const V2_FRIDAY_LABELS: [&str; 5] = [
    "Active recall",
    "Knowledge exam",
    "Review and planning",
    "Scored practical",
    "Scoring and report",
];

#[test]
fn the_daily_plan_of_a_v2_monday_and_friday_names_the_blocks_of_the_profile() {
    let mut h = v2_program();
    set_start(&mut h, "2026-11-02", "08:00");

    let monday = plan(&mut h, "2026-11-02");
    assert_eq!(monday["day_id"], "D01");
    assert_eq!(monday["week_number"], 1);
    assert_eq!(monday["week_title"], v2::WEEK_TITLES[0]);
    assert_eq!(monday["unscheduled"], false);
    assert_eq!(monday["is_assessment_day"], false);
    assert_eq!(monday["planned_minutes"], 480);
    assert_eq!(monday["objectives"].as_array().unwrap().len(), 3);
    assert_eq!(monday["objectives"][0]["objective_id"], "D01-O1");
    assert!(monday["evidence_artifact"]
        .as_str()
        .unwrap()
        .starts_with("Evidence of D01"));
    let blocks = monday["blocks"].as_array().unwrap();
    assert_eq!(
        field(blocks, "block"),
        [
            "recall",
            "theory",
            "guided_lab",
            "independent_lab",
            "defense_verification"
        ]
    );
    assert_eq!(field(blocks, "label"), V2_ORDINARY_LABELS);
    assert_eq!(minutes_of(blocks), [30, 120, 120, 150, 60]);
    assert!(blocks.iter().all(|b| b["session_id"].is_null()));

    let thursday = plan(&mut h, "2026-11-05");
    assert_eq!(thursday["day_id"], "D04");
    assert_eq!(thursday["is_assessment_day"], false);

    let friday = plan(&mut h, "2026-11-06");
    assert_eq!(friday["day_id"], "D05");
    assert_eq!(friday["is_assessment_day"], true);
    assert_eq!(friday["planned_minutes"], 480);
    let blocks = friday["blocks"].as_array().unwrap();
    assert_eq!(
        field(blocks, "block"),
        [
            "recall",
            "theory",
            "performance_review",
            "blind_practical",
            "report"
        ]
    );
    assert_eq!(field(blocks, "label"), V2_FRIDAY_LABELS);
    assert_eq!(minutes_of(blocks), [30, 120, 120, 150, 60]);

    // The midpoint and the final day are Fridays of weeks 6 and 12.
    let midpoint = plan(&mut h, "2026-12-11");
    assert_eq!(
        (
            midpoint["day_id"].as_str(),
            midpoint["week_number"].as_i64()
        ),
        (Some("D30"), Some(6))
    );
    assert_eq!(midpoint["is_assessment_day"], true);
    let last = plan(&mut h, "2027-01-22");
    assert_eq!(
        (last["day_id"].as_str(), last["week_number"].as_i64()),
        (Some("D60"), Some(12))
    );
    assert_eq!(last["is_assessment_day"], true);

    // Every response above passed the response schema (the harness validates the outcome).
}

#[test]
fn a_saturday_of_a_v2_program_is_answered_exactly_as_a_sunday_is() {
    let mut h = v2_program();
    set_start(&mut h, "2026-11-02", "08:00");
    for (saturday, sunday, week) in [
        ("2026-11-07", "2026-11-08", 1),
        ("2026-12-12", "2026-12-13", 6),
        ("2027-01-23", "2027-01-24", 12),
    ] {
        let mut sat = plan(&mut h, saturday);
        let mut sun = plan(&mut h, sunday);
        assert_eq!(sat["date"], saturday);
        assert_eq!(sat["unscheduled"], true, "{saturday}");
        assert_eq!(sat["day_id"], Value::Null);
        assert_eq!(sat["week_number"], week);
        assert_eq!(sat["week_title"], v2::WEEK_TITLES[week as usize - 1]);
        assert_eq!(sat["is_assessment_day"], false);
        assert_eq!(sat["planned_minutes"], 0);
        assert_eq!(sat["evidence_artifact"], Value::Null);
        assert_eq!(sat["objectives"], json!([]));
        assert_eq!(sat["blocks"], json!([]));
        sat.as_object_mut().unwrap().remove("date");
        sun.as_object_mut().unwrap().remove("date");
        assert_eq!(sat, sun, "{saturday} is answered as {sunday}");
    }
    // After week 12 nothing is scheduled at all, as for the Appendix A shape.
    let after = plan(&mut h, "2027-01-25");
    assert_eq!(after["unscheduled"], true);
    assert_eq!(after["week_number"], Value::Null);
    let before = plan(&mut h, "2026-10-31");
    assert_eq!(before["unscheduled"], true);

    // Planning a day that has no training day is refused, a Saturday included.
    for date in ["2026-11-07", "2026-11-08", "2027-01-23"] {
        let e = h
            .call("session.plan_day", json!({ "date": date }))
            .unwrap_err();
        assert_eq!(e.category, ErrorCategory::NotFound, "{date}");
        assert_eq!(e.code, "curriculum.no_training_day", "{date}");
    }
    let sessions: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM study_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sessions, 0);
}

#[test]
fn the_appendix_a_shape_keeps_its_saturday_and_its_labels() {
    let mut h = Harness::new();
    import_ct9(&mut h);
    set_start(&mut h, "2026-11-02", "09:00");
    let tuesday = plan(&mut h, "2026-11-03");
    assert_eq!(tuesday["day_id"], "D02");
    assert_eq!(tuesday["is_assessment_day"], false);
    assert_eq!(
        field(tuesday["blocks"].as_array().unwrap(), "label"),
        [
            "recall",
            "theory",
            "guided lab",
            "independent lab",
            "defense verification",
            "evidence reporting"
        ]
    );
    let saturday = plan(&mut h, "2026-11-07");
    assert_eq!(saturday["day_id"], "D06");
    assert_eq!(saturday["is_assessment_day"], true);
    assert_eq!(saturday["planned_minutes"], 300);
    let blocks = saturday["blocks"].as_array().unwrap();
    assert_eq!(
        field(blocks, "label"),
        [
            "blind practical",
            "remediation",
            "report",
            "performance review"
        ]
    );
    assert_eq!(
        field(blocks, "block"),
        [
            "blind_practical",
            "remediation",
            "report",
            "performance_review"
        ]
    );
    // A Friday is an ordinary day there.
    let friday = plan(&mut h, "2026-11-06");
    assert_eq!(friday["is_assessment_day"], false);
    assert_eq!(friday["planned_minutes"], 420);
}

// ------------------------------------------------------------------ plan day

fn session_times(h: &Harness) -> Vec<(String, i64, i64, String)> {
    let mut stmt = h
        .conn()
        .prepare(
            "SELECT b.block_code, s.planned_start_utc, s.planned_end_utc, s.mode
               FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
              WHERE b.training_day_id = (SELECT id FROM training_days WHERE day_code = ?1)
              ORDER BY b.ordinal",
        )
        .unwrap();
    stmt.query_map(["D01"], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn events_on(h: &mut Harness, midnight: i64) -> Vec<Value> {
    h.call(
        "calendar.list",
        json!({ "from": midnight, "to": midnight + 86_400_000 }),
    )
    .unwrap()["result"]["events"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn a_v2_monday_is_laid_out_with_the_pauses_of_the_plan_and_titled_with_the_profile_labels() {
    let mut h = v2_program();
    set_start(&mut h, "2026-11-02", "08:00");
    let planned = h
        .call("session.plan_day", json!({ "date": "2026-11-02" }))
        .unwrap();
    let r = &planned["result"];
    assert_eq!(r["day_id"], "D01");
    let created = r["created"].as_array().unwrap();
    assert_eq!(created.len(), 5);
    assert_eq!(
        field(created, "block"),
        field(
            plan(&mut h, "2026-11-02")["blocks"].as_array().unwrap(),
            "block"
        )
    );
    assert!(r["already_planned"].as_array().unwrap().is_empty());

    // 08:00-08:30, 08:30-10:30, 10:45-12:45, 13:45-16:15, 16:15-17:15.
    let at = |h: i64, m: i64| MONDAY_MIDNIGHT + (h * 60 + m) * MINUTE;
    let expected = [
        ("recall", at(8, 0), at(8, 30), "REVIEW"),
        ("theory", at(8, 30), at(10, 30), "STUDY"),
        ("guided_lab", at(10, 45), at(12, 45), "LAB"),
        ("independent_lab", at(13, 45), at(16, 15), "LAB"),
        ("defense_verification", at(16, 15), at(17, 15), "LAB"),
    ];
    let got = session_times(&h);
    let want: Vec<(String, i64, i64, String)> = expected
        .iter()
        .map(|(b, s, e, m)| ((*b).to_string(), *s, *e, (*m).to_string()))
        .collect();
    assert_eq!(got, want);
    // A session lasts what its block does: the pauses belong to no block.
    for ((_, start, end, _), minutes) in got.iter().zip([30, 120, 120, 150, 60]) {
        assert_eq!(end - start, minutes * MINUTE);
    }
    assert_eq!(
        got[4].2 - got[0].1,
        (9 * 60 + 15) * MINUTE,
        "08:00 to 17:15"
    );
    for (i, s) in created.iter().enumerate() {
        assert_eq!(s["state"], "PLANNED");
        assert_eq!(s["block_seq"], i + 1);
    }

    // The calendar carries the same times under the labels of the profile.
    let events = events_on(&mut h, MONDAY_MIDNIGHT);
    assert_eq!(events.len(), 5);
    assert_eq!(
        field(&events, "title"),
        [
            "D01 Active recall",
            "D01 Core lesson",
            "D01 Tool practice",
            "D01 Independent challenge",
            "D01 Defense and documentation"
        ]
    );
    for (e, (_, start, end, _)) in events.iter().zip(&want) {
        assert_eq!(e["start"], *start);
        assert_eq!(e["end"], *end);
        assert_eq!(e["kind"], "STUDY_BLOCK");
        assert_eq!(e["day_id"], "D01");
    }
}

#[test]
fn a_v2_friday_is_titled_with_its_own_labels_and_the_scored_practical_is_an_assessment() {
    let mut h = v2_program();
    set_start(&mut h, "2026-11-02", "08:00");
    let friday_midnight = MONDAY_MIDNIGHT + 4 * 86_400_000;
    let planned = h
        .call("session.plan_day", json!({ "date": "2026-11-06" }))
        .unwrap();
    let created = planned["result"]["created"].as_array().unwrap();
    assert_eq!(planned["result"]["day_id"], "D05");
    assert_eq!(
        field(created, "block"),
        [
            "recall",
            "theory",
            "performance_review",
            "blind_practical",
            "report"
        ]
    );
    let minutes: i64 = created
        .iter()
        .map(|s| s["planned_ms"].as_i64().unwrap() / MINUTE)
        .sum();
    assert_eq!(minutes, 480);

    let events = events_on(&mut h, friday_midnight);
    assert_eq!(
        field(&events, "title"),
        [
            "D05 Active recall",
            "D05 Knowledge exam",
            "D05 Review and planning",
            "D05 Scored practical",
            "D05 Scoring and report"
        ]
    );
    let at = |h: i64, m: i64| friday_midnight + (h * 60 + m) * MINUTE;
    let spans: Vec<(i64, i64)> = events
        .iter()
        .map(|e| (e["start"].as_i64().unwrap(), e["end"].as_i64().unwrap()))
        .collect();
    assert_eq!(
        spans,
        [
            (at(8, 0), at(8, 30)),
            (at(8, 30), at(10, 30)),
            (at(10, 45), at(12, 45)),
            (at(13, 45), at(16, 15)),
            (at(16, 15), at(17, 15)),
        ]
    );
    let kinds = field(&events, "kind");
    assert_eq!(
        kinds,
        [
            "STUDY_BLOCK",
            "STUDY_BLOCK",
            "STUDY_BLOCK",
            "ASSESSMENT",
            "STUDY_BLOCK"
        ]
    );
    assert_eq!(
        events[3]["immutable"], true,
        "the scored practical cannot be moved"
    );
    assert!(events
        .iter()
        .enumerate()
        .all(|(i, e)| (e["immutable"] == true) == (i == 3)));

    // Only the scored practical runs in EXAM mode.
    let modes: Vec<String> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT s.mode FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
                  ORDER BY b.ordinal",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(modes, ["REVIEW", "STUDY", "REVIEW", "EXAM", "STUDY"]);
}

#[test]
fn planning_a_v2_day_again_keeps_the_pauses_for_the_block_it_adds() {
    let mut h = v2_program();
    set_start(&mut h, "2026-11-02", "08:00");
    h.call("session.plan_day", json!({ "date": "2026-11-02" }))
        .unwrap();
    // The session of the third block is lost (a deleted row stands for a block never planned).
    h.conn()
        .execute(
            "DELETE FROM study_sessions WHERE study_block_id IN
               (SELECT id FROM study_blocks WHERE block_code = 'guided_lab')",
            [],
        )
        .unwrap();
    h.conn()
        .execute(
            "DELETE FROM calendar_events WHERE title = 'D01 Tool practice'",
            [],
        )
        .unwrap();
    let mended = h
        .call("session.plan_day", json!({ "date": "2026-11-02" }))
        .unwrap();
    let created = mended["result"]["created"].as_array().unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["block"], "guided_lab");
    assert_eq!(
        mended["result"]["already_planned"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    // The skipped blocks still advance the clock, pauses included: 10:45, not 10:30.
    let at = |h: i64, m: i64| MONDAY_MIDNIGHT + (h * 60 + m) * MINUTE;
    let times = session_times(&h);
    let third = times.iter().find(|t| t.0 == "guided_lab").unwrap();
    assert_eq!((third.1, third.2), (at(10, 45), at(12, 45)));
    let fourth = times.iter().find(|t| t.0 == "independent_lab").unwrap();
    assert_eq!((fourth.1, fourth.2), (at(13, 45), at(16, 15)));
    let events = events_on(&mut h, MONDAY_MIDNIGHT);
    assert_eq!(events.len(), 5);
    assert!(events.iter().any(|e| e["title"] == "D01 Tool practice"));
}

// ------------------------------------------------------- two shapes, one database

#[test]
fn both_shapes_share_one_database_and_the_last_import_is_the_one_the_application_works_in() {
    let mut h = Harness::new();
    import_ct9(&mut h);
    let first = program(&mut h);
    assert_eq!(first["curriculum"]["profile"], "CT9_480");
    set_start(&mut h, "2026-11-02", "09:00");
    assert_eq!(
        plan(&mut h, "2026-11-07")["is_assessment_day"],
        true,
        "Saturday of the Appendix A shape"
    );

    // Importing the v2 plan adds a version of another curriculum.
    h.advance(60_000);
    import_v2(&mut h);
    let curricula: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM curricula", [], |r| r.get(0))
        .unwrap();
    assert_eq!(curricula, 2);
    let both: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM training_days", [], |r| r.get(0))
        .unwrap();
    assert_eq!(both, 72 + 60);
    let second = program(&mut h);
    assert_eq!(second["curriculum"]["profile"], "MASTERY_V2");
    assert_eq!(second["curriculum"]["days_per_week"], 5);
    // The Appendix A program had a start; the v2 program has none of its own yet.
    assert_eq!(second["program_start"], Value::Null);
    let e = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-06" }))
        .unwrap_err();
    assert_eq!(e.code, "curriculum.program_start_not_configured");

    let set = set_start(&mut h, "2026-11-02", "08:00");
    assert_eq!(set["result"]["curriculum_id"], v2::CURRICULUM_ID);
    let plans: Vec<(String, String)> = {
        let mut stmt = h
            .conn()
            .prepare("SELECT curriculum_id, day_start_minute || '' FROM program_plan ORDER BY curriculum_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        plans,
        vec![
            ("g9-ct9-480".to_string(), "540".to_string()),
            (v2::CURRICULUM_ID.to_string(), "480".to_string())
        ],
        "each curriculum keeps its own program plan"
    );
    let saturday = plan(&mut h, "2026-11-07");
    assert_eq!(
        saturday["unscheduled"], true,
        "a Saturday of the v2 program"
    );
    assert_eq!(plan(&mut h, "2026-11-06")["is_assessment_day"], true);
    assert_eq!(plan(&mut h, "2026-11-06")["planned_minutes"], 480);

    // The other way round: the Appendix A seed imported last is the active program.
    let mut h = Harness::new();
    import_v2(&mut h);
    h.advance(60_000);
    import_ct9(&mut h);
    assert_eq!(program(&mut h)["curriculum"]["profile"], "CT9_480");
    set_start(&mut h, "2026-11-02", "09:00");
    assert_eq!(plan(&mut h, "2026-11-07")["is_assessment_day"], true);
    assert_eq!(plan(&mut h, "2026-11-06")["planned_minutes"], 420);
    h.call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let titles: Vec<Value> = events_on(&mut h, MONDAY_MIDNIGHT + 86_400_000);
    assert_eq!(titles.len(), 6);
    assert_eq!(
        titles[2]["title"], "D02 guided lab",
        "Appendix A titles are unchanged"
    );
    // Back to back, no pauses.
    for pair in titles.windows(2) {
        assert_eq!(pair[0]["end"], pair[1]["start"]);
    }
}

#[test]
fn a_promoted_version_stays_the_program_when_another_shape_is_imported_later() {
    let mut h = Harness::new();
    let (seed, draft) = (v2::seed(), v2::draft());
    let candidate = reviewed_candidate(&seed, &draft);
    let imported = h.approved(
        "curriculum.import",
        json!({ "manifest": candidate, "provenance": "RECONCILED_CANDIDATE" }),
        &Opts::default(),
    );
    let version_id = imported["result"]["curriculum_version_id"]
        .as_str()
        .unwrap()
        .to_string();
    h.approved(
        "curriculum.promote",
        json!({ "curriculum_version_id": version_id, "candidate_manifest": candidate, "original_manifest": seed }),
        &Opts::default(),
    );
    // A proposal of the other shape, imported afterwards, does not displace the canonical one.
    h.advance(60_000);
    import_ct9(&mut h);
    let p = program(&mut h);
    assert_eq!(p["curriculum"]["profile"], "MASTERY_V2");
    assert_eq!(p["curriculum"]["provenance"], "APPROVED_CANONICAL");
    assert_eq!(p["curriculum"]["version_label"], "2.0.0-rc1");
}

// ------------------------------------------------------------ response schemas

/// A description of a wrong answer and the edit that makes an answer wrong that way.
type Refusal = (&'static str, fn(&mut Value));

/// The three fields ADR-0023 added to the answers of `curriculum.program` and
/// `curriculum.daily_plan` (`curriculum.profile`, `curriculum.days_per_week`, `blocks[].label`)
/// are declared and typed in the response schemas, but not listed in `required`: the interface
/// that was built before them (the desktop mock transport, whose parity test validates its
/// answers against these very files) has to keep validating until it emits them. The backend
/// always emits them, which the tests above assert for both shapes and which the pipeline
/// enforces by validating every answer against the schema.
///
/// When the interface emits the fields, list them in `required` in both schemas and turn the
/// "without them" half of this test into the opposite assertion.
#[test]
fn the_response_schemas_require_and_type_the_structure_fields() {
    let strip = |program: &mut Value, daily_plan: &mut Value| {
        let curriculum = program["curriculum"].as_object_mut().unwrap();
        curriculum.remove("profile").unwrap();
        curriculum.remove("days_per_week").unwrap();
        for block in daily_plan["blocks"].as_array_mut().unwrap() {
            block.as_object_mut().unwrap().remove("label").unwrap();
        }
    };
    for (shape, mut h, date) in [
        ("MASTERY_V2", v2_program(), "2026-11-06"),
        (
            "CT9_480",
            {
                let mut h = Harness::new();
                import_ct9(&mut h);
                h
            },
            "2026-11-07",
        ),
    ] {
        set_start(&mut h, "2026-11-02", "08:00");
        let full_program = program(&mut h);
        let full_plan = plan(&mut h, date);

        // What the backend sends validates, and carries the three fields.
        assert_eq!(full_program["curriculum"]["profile"], shape);
        assert!(full_program["curriculum"]["days_per_week"].is_u64());
        assert!(full_plan["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["label"].as_str().is_some_and(|l| !l.is_empty())));
        g9_app::schemas::validate_response("curriculum.program", &full_program).unwrap();
        g9_app::schemas::validate_response("curriculum.daily_plan", &full_plan).unwrap();

        // They are required: an answer without them is refused.
        let (mut old_program, mut old_plan) = (full_program.clone(), full_plan.clone());
        strip(&mut old_program, &mut old_plan);
        for (op, answer) in [
            ("curriculum.program", &old_program),
            ("curriculum.daily_plan", &old_plan),
        ] {
            assert!(
                g9_app::schemas::validate_response(op, answer).is_err(),
                "{shape}: {op} without the ADR-0023 fields was accepted"
            );
        }

        // Present, they are typed: a value outside the schema is still refused, and so is a
        // field the schema does not declare.
        let refused_programs: [Refusal; 4] = [
            ("an unknown profile", |p| {
                p["curriculum"]["profile"] = json!("CT9_420");
            }),
            ("seven days a week", |p| {
                p["curriculum"]["days_per_week"] = json!(7);
            }),
            ("no days a week", |p| {
                p["curriculum"]["days_per_week"] = json!(0);
            }),
            ("an undeclared field", |p| {
                p["curriculum"]["weeks_per_year"] = json!(12);
            }),
        ];
        for (what, mutate) in refused_programs {
            let mut p = full_program.clone();
            mutate(&mut p);
            assert!(
                g9_app::schemas::validate_response("curriculum.program", &p).is_err(),
                "{shape}: {what}"
            );
        }
        let refused_plans: [Refusal; 3] = [
            ("an empty label", |d| {
                d["blocks"][0]["label"] = json!("");
            }),
            ("a numeric label", |d| {
                d["blocks"][0]["label"] = json!(5);
            }),
            ("an undeclared block field", |d| {
                d["blocks"][0]["colour"] = json!("red");
            }),
        ];
        for (what, mutate) in refused_plans {
            let mut d = full_plan.clone();
            mutate(&mut d);
            assert!(
                g9_app::schemas::validate_response("curriculum.daily_plan", &d).is_err(),
                "{shape}: {what}"
            );
        }
    }
}

// ----------------------------------------------------- pauses in a zone with a DST change

fn v2_program_in(zone: &str) -> Harness {
    let mut h = Harness::with(|s| s.config.zone = IanaZone::parse(zone).unwrap());
    import_v2(&mut h);
    h
}

/// `(block code, start, end)` of the sessions of the training day `day_code`, in block order.
fn spans_of(h: &Harness, day_code: &str) -> Vec<(String, i64, i64)> {
    let mut stmt = h
        .conn()
        .prepare(
            "SELECT b.block_code, s.planned_start_utc, s.planned_end_utc
               FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
              WHERE b.training_day_id = (SELECT id FROM training_days WHERE day_code = ?1)
              ORDER BY b.ordinal",
        )
        .unwrap();
    stmt.query_map([day_code], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The wall clock of an instant in `zone`, as `HH:MM`.
fn local_hhmm(instant_ms: i64, zone: &str) -> String {
    let civil = g9_calendar::to_civil(
        UtcMillis::from_millis(instant_ms),
        &IanaZone::parse(zone).unwrap(),
    )
    .unwrap();
    format!("{:02}:{:02}", civil.hour, civil.minute)
}

/// `HH:MM-HH:MM` of every session, on the wall clock of `zone`.
fn local_spans(spans: &[(String, i64, i64)], zone: &str) -> Vec<String> {
    spans
        .iter()
        .map(|(_, start, end)| format!("{}-{}", local_hhmm(*start, zone), local_hhmm(*end, zone)))
        .collect()
}

/// What a v2 day is, whatever the wall clock does: each session lasts its block's minutes of
/// elapsed time, and the next one starts 0, 15, 60 and 0 elapsed minutes after the one before
/// (no overlap, no gap that is not a pause of the profile).
fn assert_elapsed_layout(spans: &[(String, i64, i64)]) {
    let lasts: Vec<i64> = spans.iter().map(|(_, s, e)| (e - s) / MINUTE).collect();
    assert_eq!(lasts, [30, 120, 120, 150, 60], "{spans:?}");
    assert!(spans.iter().all(|(_, s, e)| (e - s) % MINUTE == 0));
    let pauses: Vec<i64> = spans
        .windows(2)
        .map(|w| (w[1].1 - w[0].2) / MINUTE)
        .collect();
    assert_eq!(pauses, [0, 15, 60, 0], "{spans:?}");
}

fn plan_day_of(h: &mut Harness, date: &str) {
    h.call("session.plan_day", json!({ "date": date })).unwrap();
}

/// `(start, end, adjusted)` of the calendar events of training day `day_id`, by start.
fn event_spans(h: &mut Harness, day_id: &str, around: i64) -> Vec<(i64, i64, Value)> {
    let mut events = h
        .call(
            "calendar.list",
            json!({ "from": around - 2 * 86_400_000, "to": around + 2 * 86_400_000 }),
        )
        .unwrap()["result"]["events"]
        .as_array()
        .unwrap()
        .clone();
    events.retain(|e| e["day_id"] == day_id);
    events.sort_by_key(|e| e["start"].as_i64().unwrap());
    events
        .iter()
        .map(|e| {
            (
                e["start"].as_i64().unwrap(),
                e["end"].as_i64().unwrap(),
                e["adjusted"].clone(),
            )
        })
        .collect()
}

#[test]
fn a_v2_day_keeps_its_pauses_when_the_clocks_change_between_two_days() {
    // Europe/Berlin changes to summer time on Sunday 2027-03-28 (02:00 to 03:00): Friday
    // 2027-03-26 is CET (UTC+1), Monday 2027-03-29 is CEST (UTC+2). The day is 08:00 to 17:15
    // on the wall clock on both, 07:00Z and 06:00Z.
    let zone = "Europe/Berlin";
    let mut h = v2_program_in(zone);
    set_start(&mut h, "2027-03-22", "08:00");
    plan_day_of(&mut h, "2027-03-26");
    plan_day_of(&mut h, "2027-03-29");

    let local = [
        "08:00-08:30",
        "08:30-10:30",
        "10:45-12:45",
        "13:45-16:15",
        "16:15-17:15",
    ];
    let friday = spans_of(&h, "D05");
    let monday = spans_of(&h, "D06");
    assert_eq!(local_spans(&friday, zone), local);
    assert_eq!(local_spans(&monday, zone), local);
    // The same days in UTC, computed from the zone's rules and not by the code under test.
    let starts = |spans: &[(String, i64, i64)]| spans.iter().map(|s| s.1).collect::<Vec<_>>();
    assert_eq!(
        starts(&friday),
        [
            1_806_044_400_000,
            1_806_046_200_000,
            1_806_054_300_000,
            1_806_065_100_000,
            1_806_074_100_000
        ]
    );
    assert_eq!(
        starts(&monday),
        [
            1_806_300_000_000,
            1_806_301_800_000,
            1_806_309_900_000,
            1_806_320_700_000,
            1_806_329_700_000
        ]
    );
    assert_elapsed_layout(&friday);
    assert_elapsed_layout(&monday);

    // The calendar carries the same instants, and nothing was adjusted: 08:00 exists.
    let friday_events = event_spans(&mut h, "D05", friday[0].1);
    assert_eq!(
        friday_events.iter().map(|e| (e.0, e.1)).collect::<Vec<_>>(),
        friday.iter().map(|s| (s.1, s.2)).collect::<Vec<_>>()
    );
    assert!(friday_events.iter().all(|e| e.2.is_null()));
}

#[test]
fn a_v2_day_that_crosses_the_spring_forward_keeps_every_block_its_elapsed_minutes() {
    // Asia/Jerusalem moves to summer time on Friday 2027-03-26 at 02:00 (to 03:00). A day that
    // starts at 01:00 local has its second block begin at 01:30 and end at 04:30 on the wall
    // clock, three hours that hold two hours of study; the pauses are added to the instant,
    // not to the wall clock, so the third block starts 15 minutes after 04:30 and not at 04:45
    // of a clock that skipped an hour.
    let zone = "Asia/Jerusalem";
    let mut h = v2_program_in(zone);
    set_start(&mut h, "2027-03-22", "01:00");
    plan_day_of(&mut h, "2027-03-26");

    let friday = spans_of(&h, "D05");
    assert_eq!(
        friday.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
        [
            "recall",
            "theory",
            "performance_review",
            "blind_practical",
            "report"
        ]
    );
    assert_eq!(
        local_spans(&friday, zone),
        [
            "01:00-01:30",
            "01:30-04:30",
            "04:45-06:45",
            "07:45-10:15",
            "10:15-11:15"
        ]
    );
    // 23:00Z Thursday 2027-03-25 is 01:00 IST (UTC+2); the day ends at 08:15Z.
    assert_eq!(
        friday.iter().map(|s| s.1).collect::<Vec<_>>(),
        [
            1_806_015_600_000,
            1_806_017_400_000,
            1_806_025_500_000,
            1_806_036_300_000,
            1_806_045_300_000
        ]
    );
    assert_eq!(friday[4].2, 1_806_048_900_000);
    assert_elapsed_layout(&friday);
    // The wall clock of the second block shows 180 minutes, its length is 120.
    let second = &friday[1];
    let wall = |ms: i64| {
        let c = g9_calendar::to_civil(UtcMillis::from_millis(ms), &IanaZone::parse(zone).unwrap())
            .unwrap();
        i64::from(c.hour) * 60 + i64::from(c.minute)
    };
    assert_eq!(wall(second.2) - wall(second.1), 180);
    assert_eq!((second.2 - second.1) / MINUTE, 120);
    // 01:00 exists on that day: no adjustment is recorded.
    assert!(event_spans(&mut h, "D05", friday[0].1)
        .iter()
        .all(|e| e.2.is_null()));
}

#[test]
fn a_v2_day_that_starts_inside_the_spring_forward_gap_is_shifted_and_says_so() {
    // 02:30 does not exist on Friday 2027-03-26 in Asia/Jerusalem. The day starts at 03:30
    // (00:30Z), the first event records the adjustment, the pauses still follow.
    let zone = "Asia/Jerusalem";
    let mut h = v2_program_in(zone);
    set_start(&mut h, "2027-03-22", "02:30");
    plan_day_of(&mut h, "2027-03-26");

    let friday = spans_of(&h, "D05");
    assert_eq!(friday[0].1, 1_806_021_000_000);
    assert_eq!(
        local_spans(&friday, zone),
        [
            "03:30-04:00",
            "04:00-06:00",
            "06:15-08:15",
            "09:15-11:45",
            "11:45-12:45"
        ]
    );
    assert_elapsed_layout(&friday);
    let events = event_spans(&mut h, "D05", friday[0].1);
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].2, "GAP");
    assert!(events[1..].iter().all(|e| e.2.is_null()));
}

#[test]
fn a_v2_day_that_starts_in_the_autumn_fold_takes_the_earlier_instant_and_says_so() {
    // Africa/Cairo leaves summer time at the end of Thursday 2027-10-28 (24:00 back to 23:00),
    // so 23:30 that day happens twice. The day starts at the earlier one (20:30Z, UTC+3), the
    // first event records the overlap, and no block overlaps another in time.
    let zone = "Africa/Cairo";
    let mut h = v2_program_in(zone);
    set_start(&mut h, "2027-10-25", "23:30");
    plan_day_of(&mut h, "2027-10-28");

    let thursday = spans_of(&h, "D04");
    assert_eq!(thursday[0].1, 1_824_755_400_000);
    assert_elapsed_layout(&thursday);
    let events = event_spans(&mut h, "D04", thursday[0].1);
    assert_eq!(events.len(), 5);
    assert_eq!(events[0].2, "OVERLAP");
    assert!(events[1..].iter().all(|e| e.2.is_null()));
    assert_eq!(
        events.iter().map(|e| (e.0, e.1)).collect::<Vec<_>>(),
        thursday.iter().map(|s| (s.1, s.2)).collect::<Vec<_>>()
    );
}
