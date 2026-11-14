//! The route from the draft curriculum to `APPROVED_CANONICAL`, through the `g9`
//! binary, each command in its own process and a throwaway data directory:
//!
//! `g9 reconcile` (review, write the candidate) -> `curriculum.import` (approval) ->
//! `curriculum.promote` (approval) -> a plan without the non-canonical warning.
//!
//! Nothing here approves anything for the operator: the test is the operator of a
//! scratch installation. The same steps on the real data directory are the owner's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::g9;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const SEED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/curriculum/g9-ct9-480.seed.v0.json"
);
const DRAFT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/curriculum/g9-ct9-480.draft.v1.json"
);

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

    /// One `g9` process on the scratch data directory.
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

    /// Write the reviewed candidate; returns the report.
    fn write_candidate(&self, out: &str) -> Value {
        let target = self.p(out);
        let (code, report, stderr) = g9(&[
            "reconcile",
            "--original",
            SEED,
            "--candidate",
            DRAFT,
            "--write-candidate",
            &target,
            "--candidate-version",
            "1.0.0-rc1",
            "--approved-by",
            "local-operator",
            "--approved-at",
            "1763000000000",
        ]);
        assert_eq!(code, 0, "{report} {stderr}");
        report
    }

    /// The approval round trip of one operation: refused with an approval id,
    /// approved by the operator, retried with it.
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

    fn plan_warnings(&self) -> Vec<String> {
        let (code, plan, stderr) = self.run(&[
            "call",
            "curriculum.daily_plan",
            "--payload",
            r#"{"date":"2026-11-16"}"#,
        ]);
        assert_eq!(code, 0, "{plan} {stderr}");
        assert_eq!(plan["result"]["day_id"], "D01");
        plan["warnings"]
            .as_array()
            .map(|w| w.iter().map(|v| v.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    }
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_draft_reaches_canonical_only_through_review_import_and_two_approvals() {
    let env = Env::new();

    // 1. The report alone: nothing differs structurally, everything is enriched, promotion is blocked.
    let (code, report, stderr) = g9(&["reconcile", "--original", SEED, "--candidate", DRAFT]);
    assert_eq!(code, 0, "{report} {stderr}");
    assert_eq!(report["days"]["changed"], 72);
    assert_eq!(report["metadata_changes"], json!([]));
    assert_eq!(report["binding"]["kind"], "ABSENT");
    assert_eq!(report["promotion_allowed"], false);
    assert!(!env.data.exists(), "a report opens no data directory");

    // 2. The operator names the review: the candidate and both payloads are written.
    let written = env.write_candidate("rc1.json");
    assert_eq!(written["written"]["binding"], "MATCHED");
    assert_eq!(
        written["written"]["diff_checksum"],
        report["reconciliation_block_for_this_diff"]["diff_checksum"]
    );
    assert!(
        !env.data.exists(),
        "writing the candidate opens no data directory"
    );
    let import_file = env.p("rc1.import.json");
    let promote_file = env.p("rc1.promote.json");

    // 3. The candidate is imported (approval) and is still not canonical.
    let (code, init, stderr) = env.run(&["init"]);
    assert_eq!(code, 0, "{init} {stderr}");
    let (code, imported, stderr) = env.with_approval("curriculum.import", &import_file);
    assert_eq!(code, 0, "{imported} {stderr}");
    assert_eq!(imported["result"]["provenance"], "RECONCILED_CANDIDATE");
    assert_eq!(
        imported["result"]["curriculum_version_id"],
        "g9-ct9-480:1.0.0-rc1"
    );
    assert_eq!(
        env.plan_warnings(),
        vec![
            "curriculum.non_canonical: plan derived from a version that is not APPROVED_CANONICAL"
        ]
    );

    // 4. A promote payload whose candidate is not the stored one is refused outright.
    let mut tampered = read_json(&promote_file);
    tampered["candidate_manifest"]["weeks"][2]["days"][1]["objectives"][0]["text"] =
        json!("Reworded after the import");
    let tampered_file = env.p("tampered.promote.json");
    fs::write(&tampered_file, tampered.to_string()).unwrap();
    let (code, refused, _) = env.run(&[
        "call",
        "curriculum.promote",
        "--payload-file",
        &tampered_file,
    ]);
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["code"], "curriculum.candidate_checksum_mismatch");
    assert_eq!(env.plan_warnings().len(), 1, "still not canonical");

    // 5. The real payload is promoted (approval) and the plan loses the warning.
    let (code, promoted, stderr) = env.with_approval("curriculum.promote", &promote_file);
    assert_eq!(code, 0, "{promoted} {stderr}");
    assert_eq!(promoted["result"]["provenance"], "APPROVED_CANONICAL");
    assert_eq!(
        promoted["result"]["diff_checksum"],
        report["reconciliation_block_for_this_diff"]["diff_checksum"]
    );
    assert_eq!(env.plan_warnings(), Vec::<String>::new());
}

#[test]
fn a_candidate_edited_after_the_review_is_imported_but_cannot_be_promoted() {
    let env = Env::new();
    env.write_candidate("rc1.json");

    // After the review (the block was written for these words) one objective is reworded.
    // The importer cannot see that: it has no original to compare with.
    let mut edited = read_json(&env.p("rc1.json"));
    let text = edited["weeks"][2]["days"][1]["objectives"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    edited["weeks"][2]["days"][1]["objectives"][0]["text"] = json!(format!("{text} (reworded)"));
    let import = json!({ "manifest": edited, "provenance": "RECONCILED_CANDIDATE" });
    let promote = json!({
        "curriculum_version_id": "g9-ct9-480:1.0.0-rc1",
        "candidate_manifest": edited,
        "original_manifest": read_json(SEED),
    });
    let import_file = env.p("edited.import.json");
    let promote_file = env.p("edited.promote.json");
    fs::write(&import_file, import.to_string()).unwrap();
    fs::write(&promote_file, promote.to_string()).unwrap();

    // The report names the problem.
    let edited_file = env.p("edited.json");
    fs::write(&edited_file, edited.to_string()).unwrap();
    let (code, report, _) = g9(&["reconcile", "--original", SEED, "--candidate", &edited_file]);
    assert_eq!(code, 0);
    assert_eq!(report["binding"]["kind"], "DIFF_CHECKSUM_MISMATCH");
    assert_eq!(report["promotion_allowed"], false);

    let (code, init, stderr) = env.run(&["init"]);
    assert_eq!(code, 0, "{init} {stderr}");
    let (code, imported, stderr) = env.with_approval("curriculum.import", &import_file);
    assert_eq!(code, 0, "{imported} {stderr}");

    // The promotion is refused before any approval is asked for.
    let (code, refused, _) = env.run(&[
        "call",
        "curriculum.promote",
        "--payload-file",
        &promote_file,
    ]);
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["code"], "curriculum.promotion_blocked");
    assert_eq!(refused["category"], "PRECONDITION_FAILED");
    assert!(
        refused["message"]
            .as_str()
            .unwrap()
            .starts_with("reconciliation block references diff checksum"),
        "{refused}"
    );
    assert_eq!(refused["details"]["promotion_allowed"], false);
    assert_eq!(
        env.plan_warnings().len(),
        1,
        "the version is still not canonical"
    );
}

#[test]
fn the_review_flags_are_required_and_nothing_is_written_without_them() {
    let env = Env::new();
    let target = env.p("rc1.json");
    for extra in [
        vec!["--write-candidate", &target],
        vec![
            "--write-candidate",
            &target,
            "--candidate-version",
            "1.0.0-rc1",
        ],
        vec![
            "--write-candidate",
            &target,
            "--candidate-version",
            "1.0.0-rc1",
            "--approved-by",
            "local-operator",
        ],
    ] {
        let mut args = vec!["reconcile", "--original", SEED, "--candidate", DRAFT];
        args.extend(extra);
        let (code, _, stderr) = g9(&args);
        assert_eq!(code, 1, "usage error expected: {stderr}");
        assert!(stderr.contains("required"), "{stderr}");
    }
    assert!(!Path::new(&target).exists());
    assert_eq!(fs::read_dir(&env.root).unwrap().count(), 0);
}
