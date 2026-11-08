//! Integration tests: the scripted week against a disposable data directory
//! with a fixed clock, and the two-step crash drill through the `g9` binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{assert_no_key_material, g9};
use g9_cli::bootstrap::{self, BootOptions};
use g9_cli::commands::simulate::{self, SimulateOptions};
use g9_cli::seed;
use g9_domain::IanaZone;
use serde_json::Value;
use std::path::Path;

/// 2026-11-16T08:00:00+01:00 (Monday).
const FIXED_ORIGIN_MS: i64 = 1_794_812_400_000;

fn options(data_dir: &Path, fixed_clock: Option<i64>) -> SimulateOptions {
    let mut boot = BootOptions::new(data_dir.to_path_buf());
    boot.skills_dir = None;
    SimulateOptions::new(
        boot,
        seed::parse_date("2026-11-16").unwrap(),
        1,
        fixed_clock,
        bootstrap::default_seed_manifest(),
        IanaZone::parse("Europe/Berlin").unwrap(),
    )
    .unwrap()
}

fn steps_of(report: &Value) -> Vec<Value> {
    report["steps"].as_array().cloned().unwrap_or_default()
}

#[test]
fn simulated_week_passes_and_is_reproducible_with_a_fixed_clock() {
    let dir = tempfile::tempdir().unwrap();
    let report = simulate::run(options(&dir.path().join("a"), Some(FIXED_ORIGIN_MS))).unwrap();
    assert_eq!(report["result"], "PASS", "{report}");
    assert_eq!(report["passed"], true);
    assert_eq!(report["complete"], true);
    assert_eq!(report["failures"], serde_json::json!([]));
    assert_eq!(report["not_run"], serde_json::json!([]));
    assert_no_key_material(
        &dir.path().join("a").join("binding.key"),
        &[&serde_json::to_string(&report).unwrap()],
    );
    assert_eq!(report["clock"]["mode"], "fixed");
    assert_eq!(report["clock"]["ids"], "sequential");
    let steps = steps_of(&report);
    let ops: Vec<&str> = steps.iter().filter_map(|s| s["op"].as_str()).collect();
    for expected in [
        "boot",
        "curriculum.validate",
        "curriculum.import",
        "approvals.decide",
        "seed:sessions",
        "seed:assessment",
        "curriculum.daily_plan",
        "session.prepare",
        "session.start",
        "session.pause",
        "session.resume",
        "session.complete",
        "notes.create",
        "assessment.begin",
        "assessment.submit",
        "assessment.review_evidence",
        "assessment.grade_provisional",
        "assessment.approve_final",
        "retention.due",
        "competency.projection",
        "backup.create",
        "backup.verify",
        "backup.restore_test",
        "audit.recent",
    ] {
        assert!(ops.contains(&expected), "missing step {expected}");
    }
    // The first import is refused APPROVAL_REQUIRED, the retry succeeds.
    let imports: Vec<&Value> = steps
        .iter()
        .filter(|s| s["op"] == "curriculum.import")
        .collect();
    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0]["category"], "APPROVAL_REQUIRED");
    assert_eq!(imports[1]["decision"], "OK");
    // Six days, 34 blocks, one pause per day, every credited time equals the plan.
    assert_eq!(
        ops.iter()
            .filter(|o| **o == "curriculum.daily_plan")
            .count(),
        6
    );
    assert_eq!(ops.iter().filter(|o| **o == "session.complete").count(), 34);
    assert_eq!(ops.iter().filter(|o| **o == "session.pause").count(), 6);
    assert_eq!(ops.iter().filter(|o| **o == "notes.create").count(), 6);
    let final_state = steps
        .iter()
        .rev()
        .find(|s| s["op"] == "assessment.approve_final")
        .unwrap();
    assert_eq!(final_state["state_after"]["state"], "APPROVED_FINAL");
    assert_eq!(final_state["state_after"]["grade_final"], true);
    let restore = steps
        .iter()
        .find(|s| s["op"] == "backup.restore_test")
        .unwrap();
    assert_eq!(restore["state_after"]["state"], "RETAINED");
    assert_eq!(restore["state_after"]["integrity"], "OK");
    // The Saturday grade reached the competency model: the finalized attempt's
    // observations are in the projection's evidence_refs. The operator grades
    // their own attempt (SELF_ADJUDICATED), so the projection is provisional and
    // never VALIDATED.
    let projection = steps
        .iter()
        .rev()
        .find(|s| s["op"] == "competency.projection")
        .unwrap();
    assert_eq!(projection["decision"], "OK");
    assert!(projection["state_after"]["projections"].as_u64().unwrap() >= 1);
    assert_eq!(
        projection["state_after"]["evidence_refs"], 3,
        "one observation per assessed dimension (understanding, application, transfer)"
    );
    assert_eq!(projection["state_after"]["provisional"], 1);
    let check_named = |name: &str| {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("missing check {name}"))
            .clone()
    };
    let included =
        check_named("competency.projection includes the observations of the finalized attempt");
    assert_eq!(included["passed"], true, "{included}");
    assert_eq!(included["detail"]["attempt_observations"], 3);
    assert_eq!(included["detail"]["evidence_refs"], 3);
    assert_eq!(
        included["detail"]["attempt_observations_in_evidence_refs"],
        true
    );
    let honest =
        check_named("self-adjudicated grade does not validate (provisional, no VALIDATED)");
    assert_eq!(honest["passed"], true, "{honest}");
    assert_eq!(honest["detail"]["validated"], false);
    assert_eq!(honest["detail"]["provisional"], true);
    let audit = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "audit chain verifies and grew")
        .unwrap();
    assert_eq!(audit["passed"], true);
    assert!(
        audit["detail"]["rows_after"].as_u64().unwrap()
            > audit["detail"]["rows_at_open"].as_u64().unwrap()
    );

    // Determinism: a second fresh run yields the same step table (ids are sequential,
    // the clock is fixed; backend-generated ids live only in `artifacts`).
    let again = simulate::run(options(&dir.path().join("b"), Some(FIXED_ORIGIN_MS))).unwrap();
    assert_eq!(
        steps_of(&again),
        steps,
        "step tables differ between fixed-clock runs"
    );
    assert_eq!(again["clock"], report["clock"]);
    // The whole report differs only in the data directory and the backend ids.
    let mut differing: Vec<&String> = report
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, v)| again.get(k.as_str()) != Some(*v))
        .map(|(k, _)| k)
        .collect();
    differing.sort();
    assert_eq!(differing, ["artifacts", "data_dir"], "{differing:?}");
}

#[test]
fn a_not_run_step_makes_the_week_incomplete_not_pass() {
    let none: Vec<String> = Vec::new();
    let not_run =
        vec!["step 9: calendar.propose -> calendar.fill_curriculum.not_implemented".into()];
    let failed =
        vec!["step 3: session.start -> PRECONDITION_FAILED session.illegal_transition".into()];
    assert_eq!(simulate::verdict(&none, &none), "PASS");
    assert_eq!(simulate::verdict(&none, &not_run), "INCOMPLETE");
    assert_eq!(simulate::verdict(&failed, &not_run), "FAIL");
}

#[test]
fn rerunning_the_same_week_in_the_same_data_dir_fails_with_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    simulate::run(options(dir.path(), Some(FIXED_ORIGIN_MS))).unwrap();
    let err = simulate::run(options(dir.path(), Some(FIXED_ORIGIN_MS))).unwrap_err();
    match err {
        g9_cli::CliError::Scenario(report) => {
            assert_eq!(report["result"], "FAIL");
            let failures = report["failures"].as_array().unwrap();
            assert!(
                failures
                    .iter()
                    .any(|f| f.as_str().unwrap().contains("session.illegal_transition")),
                "{failures:?}"
            );
            assert!(
                failures
                    .iter()
                    .any(|f| f.as_str().unwrap().contains("fresh --data-dir")),
                "{failures:?}"
            );
            assert_eq!(
                report["steps"][1]["decision"], "PRESENT",
                "curriculum already imported"
            );
            // The first run imported the seed in-process (12 extra ids), so the
            // re-run's keys are offset and nothing is replayed: the sessions are
            // really re-attempted and refused by the state machine.
            assert!(
                !report["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s["decision"] == "FAIL: idempotency.replayed"),
                "{report}"
            );
        }
        other => panic!("unexpected {other:?}"),
    }
}

/// When the seed was imported by another process (as `g9 import-seed` does),
/// two fixed-clock runs on the same directory issue identical sequential
/// idempotency keys. The backend then replays the stored outcomes instead of
/// executing anything; the scenario must report that as a failure at the first
/// mutating step, not pass its checks on replayed results.
#[test]
fn rerunning_with_aligned_ids_is_a_replay_failure_not_a_pass() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut boot = BootOptions::new(dir.path().to_path_buf());
        boot.skills_dir = None;
        let booted = bootstrap::open(&boot).unwrap();
        let mut client = g9_cli::client::Client::new(booted.ctx);
        let manifest =
            g9_cli::commands::import_seed::load_manifest(&bootstrap::default_seed_manifest())
                .unwrap();
        let mut noop = |_: &str, _: &Result<Value, g9_domain::G9Error>| {};
        let imported =
            g9_cli::commands::import_seed::import(&mut client, &manifest, &mut noop).unwrap();
        assert_eq!(imported["already_imported"], false);
    }
    let first = simulate::run(options(dir.path(), Some(FIXED_ORIGIN_MS))).unwrap();
    assert_eq!(first["result"], "PASS", "{first}");
    assert_eq!(first["steps"][1]["decision"], "PRESENT");
    let err = simulate::run(options(dir.path(), Some(FIXED_ORIGIN_MS))).unwrap_err();
    let g9_cli::CliError::Scenario(report) = err else {
        panic!("expected a scenario report");
    };
    assert_eq!(report["result"], "FAIL");
    let steps = steps_of(&report);
    let failing: Vec<&Value> = steps
        .iter()
        .filter(|s| {
            s["decision"]
                .as_str()
                .is_some_and(|d| d.starts_with("FAIL"))
        })
        .collect();
    // The replay halts the script; the only other failure is the final audit
    // check, which must fail because nothing was written in this process.
    let failing_ops: Vec<&str> = failing.iter().filter_map(|s| s["op"].as_str()).collect();
    assert_eq!(
        failing_ops,
        ["session.prepare", "check:audit chain verifies and grew"],
        "{report}"
    );
    assert_eq!(failing[0]["decision"], "FAIL: idempotency.replayed");
    assert_eq!(
        failing[0]["state_after"]["state"], "READY",
        "the replayed outcome"
    );
    // Nothing after the replay ran: no session was completed, no check passed on replayed data.
    assert_eq!(
        steps
            .iter()
            .filter(|s| s["op"] == "session.complete")
            .count(),
        0
    );
    assert!(
        !steps.iter().any(|s| s["op"]
            .as_str()
            .is_some_and(|o| o.contains("credited equals planned"))),
        "{report}"
    );
    let failures = report["failures"].as_array().unwrap();
    assert!(
        failures
            .iter()
            .any(|f| f.as_str().unwrap() == simulate::REPLAY_HINT),
        "{failures:?}"
    );
}

#[test]
fn crash_drill_then_init_reports_the_interrupted_session_through_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let data_str = data.to_str().unwrap();
    let (code, drill, drill_err) = g9(&[
        "crash-drill",
        "--scratch",
        "--data-dir",
        data_str,
        "--no-skills",
    ]);
    assert_eq!(code, 0, "{drill}");
    assert_eq!(drill["state"], "RUNNING");
    let session_id = drill["session_id"].as_str().unwrap().to_string();
    assert!(data.join("binding.key").is_file());
    // The drill really leaves an open interval behind: one interval, no end.
    assert!(drill["open_interval_started_at"].is_i64(), "{drill}");
    let intervals = drill["intervals"].as_array().unwrap();
    assert_eq!(intervals.len(), 1);
    assert!(intervals[0]["end_utc"].is_null(), "{drill}");
    let interval_id = intervals[0]["interval_id"].as_str().unwrap().to_string();

    let (code, init, init_err) = g9(&["init", "--data-dir", data_str, "--no-skills"]);
    assert_eq!(code, 0, "{init}");
    assert_eq!(
        init["recovery"]["interrupted_sessions"],
        serde_json::json!([session_id])
    );
    // Recovery closed exactly the interval the drill left open.
    assert_eq!(
        init["recovery"]["closed_intervals"],
        serde_json::json!([interval_id])
    );
    assert_eq!(init["recovery"]["audit_chain_ok"], true);
    let interrupted = &init["interrupted_sessions"][0];
    assert_eq!(interrupted["session_id"], session_id);
    assert_eq!(interrupted["state"], "INTERRUPTED");
    assert_eq!(interrupted["requires_correction"], true);
    assert_eq!(
        interrupted["credited_ms"], 0,
        "uncertain time is never credited"
    );
    assert_eq!(interrupted["intervals"][0]["interval_id"], interval_id);
    assert_eq!(interrupted["intervals"][0]["continuity"], "UNCERTAIN");
    assert_eq!(interrupted["intervals"][0]["closed"], true);
    assert_eq!(init["status"]["storage"]["state"], "READY");
    assert_eq!(init["status"]["model"]["available"], false);
    // No output carries the key bytes in any encoding (hex, base64, byte list).
    assert_no_key_material(
        &data.join("binding.key"),
        &[
            &serde_json::to_string(&drill).unwrap(),
            &drill_err,
            &serde_json::to_string(&init).unwrap(),
            &init_err,
        ],
    );

    // A second init finds nothing left to recover.
    let (code, again, _) = g9(&["init", "--data-dir", data_str, "--no-skills"]);
    assert_eq!(code, 0);
    assert_eq!(
        again["recovery"]["interrupted_sessions"],
        serde_json::json!([])
    );
}

#[test]
fn exit_codes_follow_the_contract_through_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let data_str = data.to_str().unwrap();
    // 2: a G9Error, printed serialized on stdout.
    let (code, err, stderr) = g9(&["call", "nope.op", "--data-dir", data_str, "--no-skills"]);
    assert_eq!(code, 2);
    assert_eq!(err["category"], "INVALID_INPUT");
    assert_eq!(err["code"], "ipc.unknown_operation");
    assert!(stderr.is_empty());
    // 2: a PROPOSED operation answers PRECONDITION_FAILED <area>.not_implemented.
    let (code, err, _) = g9(&[
        "call",
        "teaching.propose_hint",
        "--data-dir",
        data_str,
        "--no-skills",
    ]);
    assert_eq!(code, 2);
    assert_eq!(err["category"], "PRECONDITION_FAILED");
    assert_eq!(err["code"], "teaching.not_implemented");
    // 1: usage errors.
    let (code, _, _) = g9(&[
        "call",
        "system.status",
        "--payload",
        "{bad",
        "--data-dir",
        data_str,
    ]);
    assert_eq!(code, 1);
    let (code, _, stderr) = g9(&[
        "simulate-week",
        "--start-date",
        "2026-11-17",
        "--data-dir",
        data_str,
    ]);
    assert_eq!(code, 1, "without --scratch the command is refused");
    assert!(stderr.contains("--scratch"), "{stderr}");
    let (code, _, stderr) = g9(&[
        "simulate-week",
        "--scratch",
        "--start-date",
        "2026-11-17",
        "--data-dir",
        data_str,
    ]);
    assert_eq!(code, 1, "a Tuesday is refused");
    assert!(stderr.contains("Monday"), "{stderr}");
    let (code, _, _) = g9(&["definitely-not-a-command"]);
    assert_eq!(code, 1);
    // 0: success; ops needs no data directory.
    let (code, ops, _) = g9(&["ops", "--schemas"]);
    assert_eq!(code, 0);
    assert_eq!(ops["count"], 35);
    let (code, _, _) = g9(&["--help"]);
    assert_eq!(code, 0);
}
