//! The second curriculum shape (ADR-0023, structure profile `MASTERY_V2`) through the `g9`
//! command-line driver: a v2 manifest file validates, imports, reconciles and promotes like the
//! Appendix A one, the Appendix A tooling (the default seed path, `simulate-week`, the
//! performance fixture) keeps its shape, and a version of the other shape is answered with a
//! `PRECONDITION_FAILED` error, never a panic.
//!
//! The v2 manifests are generated (`crates/g9-curriculum/tests/common/mastery_v2.rs`): a
//! structural fixture with placeholder texts, not the owner's syllabus. Each command runs in its
//! own process on a throwaway data directory; the test is the operator of a scratch installation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// `CliError` carries a `G9Error` by value (the closed IPC error contract).
#![allow(clippy::result_large_err)]

mod common;
#[path = "../../../crates/g9-curriculum/tests/common/mastery_v2.rs"]
mod v2;

use common::g9;
use g9_cli::bootstrap::{self, BootOptions};
use g9_cli::commands::simulate::{self, SimulateOptions};
use g9_cli::perf::fixture::{self, FixtureOptions};
use g9_cli::{seed, CliError};
use g9_domain::{ErrorCategory, IanaZone};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// 2026-11-16T08:00:00+01:00 (Monday).
const FIXED_ORIGIN_MS: i64 = 1_794_812_400_000;

struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    data: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let data = root.join("data");
        Self {
            _dir: dir,
            root,
            data,
        }
    }

    fn p(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn write(&self, name: &str, value: &Value) -> String {
        let path = self.p(name);
        fs::write(&path, serde_json::to_string(value).unwrap()).unwrap();
        path
    }

    /// One `g9` process on the scratch data directory; the program starts on Monday 2026-11-16.
    fn run(&self, args: &[&str]) -> (i32, Value, String) {
        let data = self.data.to_string_lossy().into_owned();
        let mut all: Vec<&str> = args.to_vec();
        all.extend([
            "--data-dir",
            &data,
            "--no-skills",
            "--program-start",
            "2026-11-16",
            "--zone",
            "Europe/Berlin",
        ]);
        g9(&all)
    }

    fn call(&self, op: &str, payload: &str) -> Value {
        let (code, out, stderr) = self.run(&["call", op, "--payload", payload]);
        assert_eq!(code, 0, "{op}: {out} {stderr}");
        out["result"].clone()
    }

    /// The approval round trip of one operation: refused with an approval id, approved by the
    /// operator, retried with it.
    fn with_approval(&self, op: &str, payload_file: &str) -> (i32, Value, String) {
        let (code, first, _) = self.run(&["call", op, "--payload-file", payload_file]);
        assert_eq!(code, 2, "{op}: {first}");
        assert_eq!(first["category"], "APPROVAL_REQUIRED", "{op}: {first}");
        let id = first["approval_id"].as_str().unwrap().to_string();
        let (code, decided, stderr) = self.run(&["approve", &id]);
        assert_eq!(code, 0, "approve {id}: {decided} {stderr}");
        self.run(&[
            "call",
            op,
            "--payload-file",
            payload_file,
            "--approval",
            &id,
        ])
    }

    fn plan(&self, date: &str) -> Value {
        self.call("curriculum.daily_plan", &format!(r#"{{"date":"{date}"}}"#))
    }

    fn import_seed(&self, manifest: Option<&str>) -> (i32, Value, String) {
        let mut args = vec!["import-seed"];
        if let Some(path) = manifest {
            args.extend(["--manifest", path]);
        }
        self.run(&args)
    }

    fn program(&self) -> Value {
        self.call("curriculum.program", "{}")["curriculum"].clone()
    }
}

fn boot(data: &Path) -> BootOptions {
    let mut boot = BootOptions::new(data.to_path_buf());
    boot.skills_dir = None;
    boot
}

fn simulate_options(data: &Path, manifest: PathBuf) -> SimulateOptions {
    SimulateOptions::new(
        boot(data),
        seed::parse_date("2026-11-16").unwrap(),
        1,
        Some(FIXED_ORIGIN_MS),
        manifest,
        IanaZone::parse("Europe/Berlin").unwrap(),
    )
    .unwrap()
}

/// A scripted week that did not pass is `CliError::Scenario(report)` (exit 2 with the report on
/// stdout), as for any failed week; the report is what a test reads.
fn failed_week(outcome: Result<Value, CliError>) -> Value {
    match outcome {
        Err(CliError::Scenario(report)) => report,
        Ok(report) => panic!("the week passed: {report}"),
        Err(other) => panic!("the week did not run: {other:?}"),
    }
}

fn failure_codes(report: &Value) -> Vec<String> {
    report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["decision"].as_str())
        .filter(|d| d.starts_with("FAIL:"))
        .map(str::to_string)
        .collect()
}

// ------------------------------------------------------- import and validate

#[test]
fn a_v2_manifest_file_validates_and_imports_through_the_import_seed_path() {
    let env = Env::new();
    let file = env.write("v2.draft.json", &v2::draft());

    // `curriculum.validate` on the file's content, as `g9 call` sends it.
    let request = env.write("validate.json", &json!({ "manifest": v2::draft() }));
    let (code, validated, stderr) =
        env.run(&["call", "curriculum.validate", "--payload-file", &request]);
    assert_eq!(code, 0, "{validated} {stderr}");
    assert_eq!(validated["result"]["valid"], true, "{validated}");
    assert_eq!(validated["result"]["violations"], json!([]));

    // `import-seed --manifest`: validation, then the approval round trip, in one command.
    let (code, imported, stderr) = env.import_seed(Some(&file));
    assert_eq!(code, 0, "{imported} {stderr}");
    assert_eq!(imported["already_imported"], false);
    assert_eq!(imported["provenance"], "PROPOSED_SEED");
    assert_eq!(
        imported["curriculum_version_id"],
        format!("{}:{}", v2::CURRICULUM_ID, v2::DRAFT_VERSION)
    );
    assert_eq!(
        (
            &imported["weeks"],
            &imported["days"],
            &imported["objectives"]
        ),
        (&json!(12), &json!(60), &json!(180))
    );
    assert!(
        imported["approval_id"].is_string(),
        "approved by the operator"
    );

    // Importing it again reports, from the manifest itself, what is there.
    let (code, again, stderr) = env.import_seed(Some(&file));
    assert_eq!(code, 0, "{again} {stderr}");
    assert_eq!(again["already_imported"], true);
    assert_eq!(
        (&again["weeks"], &again["days"], &again["objectives"]),
        (&json!(12), &json!(60), &json!(180))
    );

    // The program says which shape it is, and the plan follows it: Friday is the scored day, a
    // Saturday is a rest day (Monday 2026-11-16 starts week 1).
    let program = env.program();
    assert_eq!(program["profile"], "MASTERY_V2");
    assert_eq!(program["days_per_week"], 5);
    assert_eq!(program["days"], 60);
    assert_eq!(program["planned_minutes"], 28_800);
    let friday = env.plan("2026-11-20");
    assert_eq!(friday["day_id"], "D05");
    assert_eq!(friday["is_assessment_day"], true);
    assert_eq!(friday["blocks"][3]["label"], "Scored practical");
    let saturday = env.plan("2026-11-21");
    assert_eq!(saturday["unscheduled"], true);
    assert_eq!(saturday["blocks"], json!([]));
}

#[test]
fn an_invalid_v2_manifest_is_refused_with_the_validation_report_and_nothing_is_stored() {
    let env = Env::new();
    let mut bad = v2::draft();
    bad["weeks"][0]["days"][4]["is_assessment_day"] = json!(false);
    bad["structure"]["total_minutes"] = json!(28_000);
    let file = env.write("bad.json", &bad);
    let (code, refused, _) = env.import_seed(Some(&file));
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["category"], "INVALID_INPUT");
    assert_eq!(refused["code"], "curriculum.invariant_violated");
    let codes: Vec<&str> = refused["details"]["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"day.assessment_flag.mismatch"), "{codes:?}");
    assert!(codes.contains(&"structure.mismatch"), "{codes:?}");
    assert_eq!(env.program(), Value::Null, "nothing was imported");
}

#[test]
fn the_default_seed_path_and_the_scripted_tooling_stay_on_the_appendix_a_shape() {
    let seed_path = bootstrap::default_seed_manifest();
    assert!(
        seed_path.ends_with("data/curriculum/g9-ct9-480.seed.v0.json"),
        "{seed_path:?}"
    );
    let seed_json: Value = serde_json::from_str(&fs::read_to_string(&seed_path).unwrap()).unwrap();
    assert_eq!(seed_json["manifest_schema_version"], "1.0.0");
    assert!(seed_json["structure"]
        .get("assessment_day_of_week")
        .is_none());
    seed::require_appendix_a_manifest(&seed_json).unwrap();
    assert!(
        fixture::default_manifest().ends_with("data/curriculum/g9-ct9-480.draft.v1.json"),
        "the performance fixture is sized for the Appendix A draft"
    );
    let default = FixtureOptions::new(PathBuf::from("unused"), 50).unwrap();
    assert_eq!(default.manifest, fixture::default_manifest());

    // The v2 manifest is another shape: the helper says so with a precondition error.
    let err = seed::require_appendix_a_manifest(&v2::draft()).unwrap_err();
    assert_eq!(err.category, ErrorCategory::PreconditionFailed);
    assert_eq!(err.code, "cli.appendix_a_required");
    assert!(err.message.contains("MASTERY_V2"), "{}", err.message);
    // A manifest whose structure cannot be read is the validator's to report, not this check's.
    seed::require_appendix_a_manifest(&json!({})).unwrap();
    seed::require_appendix_a_manifest(&json!({ "structure": { "weeks": "twelve" } })).unwrap();
}

// -------------------------------------------------------- simulate-week, perf

#[test]
fn simulate_week_answers_a_v2_manifest_with_a_precondition_error_before_importing_it() {
    let env = Env::new();
    let file = env.write("v2.draft.json", &v2::draft());
    let report = failed_week(simulate::run(simulate_options(
        &env.data,
        PathBuf::from(&file),
    )));
    assert_eq!(report["result"], "FAIL", "{report}");
    assert_eq!(report["passed"], false);
    let step = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["op"] == "load:manifest")
        .unwrap_or_else(|| panic!("{report}"));
    assert_eq!(step["category"], "PRECONDITION_FAILED");
    assert_eq!(step["decision"], "FAIL: cli.appendix_a_required");
    assert!(step["state_after"]["message"]
        .as_str()
        .unwrap()
        .contains("Appendix A"));
    // Nothing was imported or seeded.
    assert_eq!(env.program(), Value::Null);
    let ops: Vec<&str> = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["op"].as_str())
        .collect();
    assert!(!ops.contains(&"curriculum.import") && !ops.contains(&"seed:sessions"));
}

#[test]
fn simulate_week_answers_a_data_directory_whose_program_is_v2_with_a_precondition_error() {
    let env = Env::new();
    let file = env.write("v2.draft.json", &v2::draft());
    let (code, imported, stderr) = env.import_seed(Some(&file));
    assert_eq!(code, 0, "{imported} {stderr}");

    // The scripted week finds a program there (the probe passes), then refuses its shape.
    let report = failed_week(simulate::run(simulate_options(
        &env.data,
        bootstrap::default_seed_manifest(),
    )));
    assert_eq!(report["result"], "FAIL", "{report}");
    let step = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["op"] == "resolve:curriculum")
        .unwrap_or_else(|| panic!("{report}"));
    assert_eq!(step["category"], "PRECONDITION_FAILED");
    assert_eq!(step["decision"], "FAIL: cli.appendix_a_required");
    assert_eq!(failure_codes(&report), ["FAIL: cli.appendix_a_required"]);
    let ops: Vec<&str> = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["op"].as_str())
        .collect();
    assert!(
        !ops.contains(&"seed:sessions") && !ops.contains(&"session.start"),
        "no session was seeded or started: {ops:?}"
    );

    // Through the binary it is an exit 2 with the report, as for any failed week.
    let (code, out, _) = env.run(&[
        "simulate-week",
        "--scratch",
        "--start-date",
        "2026-11-16",
        "--fixed-clock",
        &FIXED_ORIGIN_MS.to_string(),
    ]);
    assert_eq!(code, 2, "{out}");
    assert_eq!(out["result"], "FAIL");
}

#[test]
fn the_performance_fixture_refuses_a_v2_manifest_before_writing_a_data_directory() {
    let env = Env::new();
    let file = env.write("v2.draft.json", &v2::draft());
    let mut options = FixtureOptions::new(env.data.clone(), 50).unwrap();
    options.manifest = PathBuf::from(&file);
    let err = match fixture::build(&options, &mut |_| Ok(())) {
        Ok(_) => panic!("a v2 manifest must not build the fixture"),
        Err(e) => e,
    };
    let CliError::Backend(e) = err else {
        panic!("expected a backend error, got {err:?}");
    };
    assert_eq!(e.category, ErrorCategory::PreconditionFailed);
    assert_eq!(e.code, "cli.appendix_a_required");
    assert!(
        !env.data.join(bootstrap::DATABASE_FILE).exists(),
        "no database was created"
    );
}

#[test]
fn both_shapes_in_one_data_directory_and_the_scripted_week_follows_the_active_one() {
    // Appendix A first, then v2: the program in use is v2, and the week is refused.
    let env = Env::new();
    let v2_file = env.write("v2.draft.json", &v2::draft());
    let (code, out, stderr) = env.import_seed(None);
    assert_eq!(code, 0, "{out} {stderr}");
    assert_eq!(env.program()["profile"], "CT9_480");
    let (code, out, stderr) = env.import_seed(Some(&v2_file));
    assert_eq!(code, 0, "{out} {stderr}");
    assert_eq!(env.program()["profile"], "MASTERY_V2");
    let report = failed_week(simulate::run(simulate_options(
        &env.data,
        bootstrap::default_seed_manifest(),
    )));
    assert_eq!(report["result"], "FAIL");
    assert_eq!(failure_codes(&report), ["FAIL: cli.appendix_a_required"]);

    // v2 first, then Appendix A: the program in use is Appendix A, and the week passes.
    let env = Env::new();
    let v2_file = env.write("v2.draft.json", &v2::draft());
    let (code, out, stderr) = env.import_seed(Some(&v2_file));
    assert_eq!(code, 0, "{out} {stderr}");
    let (code, out, stderr) = env.import_seed(None);
    assert_eq!(code, 0, "{out} {stderr}");
    let program = env.program();
    assert_eq!(program["profile"], "CT9_480");
    assert_eq!(program["days_per_week"], 6);
    let report = simulate::run(simulate_options(
        &env.data,
        bootstrap::default_seed_manifest(),
    ))
    .unwrap();
    assert_eq!(report["result"], "PASS", "{report}");
}

// ---------------------------------------------------------- reconcile, promote

#[test]
fn a_v2_pair_reconciles_and_its_reviewed_candidate_is_promoted_through_two_approvals() {
    let env = Env::new();
    let seed_file = env.write("v2.seed.json", &v2::seed());
    let draft_file = env.write("v2.draft.json", &v2::draft());

    // The report alone: every day is enriched, nothing differs in structure.
    let (code, report, stderr) = g9(&[
        "reconcile",
        "--original",
        &seed_file,
        "--candidate",
        &draft_file,
    ]);
    assert_eq!(code, 0, "{report} {stderr}");
    assert_eq!(report["days"]["changed"], 60, "{report}");
    assert_eq!(report["days"]["unchanged"], 0);
    assert_eq!(report["metadata_changes"], json!([]));
    assert_eq!(report["promotion_allowed"], false);

    // The operator names the review; the candidate and both payloads are written.
    let target = env.p("rc1.json");
    let (code, written, stderr) = g9(&[
        "reconcile",
        "--original",
        &seed_file,
        "--candidate",
        &draft_file,
        "--write-candidate",
        &target,
        "--candidate-version",
        "2.0.0-rc1",
        "--approved-by",
        "local-operator",
        "--approved-at",
        "1763000000000",
    ]);
    assert_eq!(code, 0, "{written} {stderr}");
    assert_eq!(written["written"]["binding"], "MATCHED");

    let (code, init, stderr) = env.run(&["init"]);
    assert_eq!(code, 0, "{init} {stderr}");
    let import_file = env.p("rc1.import.json");
    let promote_file = env.p("rc1.promote.json");
    let (code, imported, stderr) = env.with_approval("curriculum.import", &import_file);
    assert_eq!(code, 0, "{imported} {stderr}");
    assert_eq!(imported["result"]["provenance"], "RECONCILED_CANDIDATE");
    assert_eq!(
        imported["result"]["curriculum_version_id"],
        "g9-cyber-mastery-480:2.0.0-rc1"
    );
    assert_eq!(imported["result"]["days"], 60);
    let (code, promoted, stderr) = env.with_approval("curriculum.promote", &promote_file);
    assert_eq!(code, 0, "{promoted} {stderr}");
    assert_eq!(promoted["result"]["provenance"], "APPROVED_CANONICAL");
    assert_eq!(promoted["result"]["summary"]["changed"], 60);

    // The plan no longer says it is a proposal.
    let (code, plan, stderr) = env.run(&[
        "call",
        "curriculum.daily_plan",
        "--payload",
        r#"{"date":"2026-11-20"}"#,
    ]);
    assert_eq!(code, 0, "{plan} {stderr}");
    assert_eq!(plan["result"]["is_assessment_day"], true);
    assert!(
        plan["warnings"]
            .as_array()
            .is_none_or(std::vec::Vec::is_empty),
        "{plan}"
    );
}
