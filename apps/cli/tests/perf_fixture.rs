//! PERF-1: the performance fixture at a reduced scale. Every target count is
//! met, the records come through the operations the plan names, and two
//! builds into two fresh directories give the same checksum. The full sizes
//! (`--divisor 1`) are built by `g9 perf-fixture` outside `cargo test` (minutes).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// `CliError` carries a `G9Error` by value (the closed IPC error contract).
#![allow(clippy::result_large_err)]

mod common;

use common::{assert_no_key_material, g9};
use g9_cli::perf::fixture::{self, FixtureOptions, Targets, DERIVED_PER_ATTEMPT};
use g9_cli::CliError;

/// 1/50 of the budget sizes: 34 sessions, 100 intervals, 40 notes, 10
/// attempts, 400 observations, 1,000 audit rows.
const DIVISOR: u32 = 50;

#[test]
fn two_builds_into_fresh_directories_give_the_same_checksum() {
    let dir = tempfile::tempdir().unwrap();
    let a = FixtureOptions::new(dir.path().join("a"), DIVISOR).unwrap();
    let b = FixtureOptions::new(dir.path().join("b"), DIVISOR).unwrap();
    let (ra, ca) = fixture::build(&a, &mut |_| Ok(())).unwrap();
    drop(ca);
    let (rb, cb) = fixture::build(&b, &mut |_| Ok(())).unwrap();
    drop(cb);
    assert_eq!(ra.result, "PASS", "{:?}", ra.mismatches);
    assert_eq!(rb.result, "PASS", "{:?}", rb.mismatches);
    assert_eq!(ra.checksum.sha256, rb.checksum.sha256);
    assert_eq!(ra.checksum, rb.checksum, "per-table digests");
    assert_eq!(ra.counts, rb.counts);
    assert_eq!(ra.dispatched, rb.dispatched);
    // The projection had work to do: the backend minted ids and digests.
    assert!(
        ra.checksum.projected_uuids > 100,
        "{}",
        ra.checksum.projected_uuids
    );
    assert!(ra.checksum.projected_digests > 100);

    let t = Targets::divided(DIVISOR);
    let count = |table: &str| ra.counts[table];
    assert_eq!(count("training_days"), 72);
    assert_eq!(count("daily_objectives"), 216);
    assert_eq!(count("study_sessions"), u64::from(t.sessions));
    assert_eq!(count("time_intervals"), u64::from(t.intervals));
    assert_eq!(count("notes"), u64::from(t.notes));
    assert_eq!(count("note_versions"), 3 * u64::from(t.notes));
    assert_eq!(count("attempts"), u64::from(t.attempts));
    assert_eq!(count("competency_observations"), u64::from(t.observations));
    assert_eq!(count("audit_metadata"), u64::from(t.audit_rows));
    assert_eq!(ra.audit_top_up, 0, "the audit-row plan is exact");

    // Through the pipeline wherever an operation exists ...
    let d = |op: &str| ra.dispatched.get(op).copied().unwrap_or(0);
    assert_eq!(d("notes.create"), u64::from(t.notes));
    assert_eq!(d("notes.revise"), 2 * u64::from(t.notes));
    assert_eq!(d("assessment.begin"), u64::from(t.attempts));
    assert_eq!(d("assessment.approve_final"), u64::from(t.attempts));
    assert_eq!(
        d("assessment.approve_final (APPROVAL_REQUIRED)"),
        u64::from(t.attempts)
    );
    assert_eq!(
        d("session.prepare"),
        u64::from(t.sessions),
        "every session ran"
    );
    assert_eq!(d("session.complete"), u64::from(t.sessions));
    assert_eq!(
        d("session.start") + d("session.resume"),
        u64::from(t.intervals),
        "one interval per start or resume"
    );
    assert!(d("session.plan_day") > 0 && d("session.create") > 0);
    assert!(d("retention.record_review") > 0 && d("calendar.create_event") > 0);
    // ... and SEEDED, labelled, only where none exists.
    assert_eq!(
        ra.seeded.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["competency_observations", "learning_episodes"]
    );
    assert_eq!(
        ra.seeded["competency_observations"] + u64::from(DERIVED_PER_ATTEMPT * t.attempts),
        u64::from(t.observations)
    );
    assert_eq!(ra.seeded["learning_episodes"], u64::from(t.sessions));
}

#[test]
fn a_directory_that_holds_a_database_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("d");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("g9.sqlite"), b"").unwrap();
    let options = FixtureOptions::new(data.clone(), DIVISOR).unwrap();
    match fixture::build(&options, &mut |_| Ok(())) {
        Err(CliError::Usage(message)) => assert!(message.contains("fresh data directory")),
        Err(other) => panic!("unexpected {other:?}"),
        Ok(_) => panic!("a used directory was accepted"),
    }
    assert!(!data.join("binding.key").exists(), "nothing was created");
}

#[test]
fn the_binary_builds_a_fixture_and_prints_counts_and_checksum() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("f");
    let data_str = data.to_str().unwrap();
    let (code, refused, _) = g9(&["perf-fixture", "--data-dir", data_str]);
    assert_eq!(code, 1, "no --scratch");
    assert_eq!(refused, serde_json::Value::Null);
    assert!(!data.exists());
    let (code, out, stderr) = g9(&[
        "perf-fixture",
        "--scratch",
        "--data-dir",
        data_str,
        "--divisor",
        "100",
    ]);
    assert_eq!(code, 0, "{out} {stderr}");
    assert_eq!(out["scenario"], "perf-fixture");
    assert_eq!(out["result"], "PASS");
    assert_eq!(out["passed"], true);
    assert_eq!(out["divisor"], 100);
    assert_eq!(out["counts"]["audit_metadata"], 500);
    assert_eq!(out["checksum"]["sha256"].as_str().map(str::len), Some(64));
    assert_no_key_material(&data.join("binding.key"), &[&out.to_string(), &stderr]);
    // A second build into the same directory is refused, the first is kept.
    let (code, _, stderr) = g9(&[
        "perf-fixture",
        "--scratch",
        "--data-dir",
        data_str,
        "--divisor",
        "100",
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("fresh data directory"), "{stderr}");
}
