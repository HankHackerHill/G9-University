//! `g9 reconcile`: compare a candidate curriculum manifest with the operator's
//! original plan (`g9_curriculum::reconcile`), print the diff with its checksums
//! and, only when the operator names who reviewed it and when, write the
//! `RECONCILED_CANDIDATE` manifest together with the two request payloads that
//! `curriculum.import` and `curriculum.promote` take.
//!
//! * It reads two manifest files and opens no data directory. Without
//!   `--write-candidate` it writes nothing.
//! * Both manifests must pass `g9_curriculum::validate`; one that does not is
//!   refused with its content-free validation errors.
//! * `--write-candidate` needs `--candidate-version`, `--approved-by` and
//!   `--approved-at`. None has a default: the block it writes says that this
//!   operator reviewed this diff, and the command never says that on its own. The
//!   block is not the approval of the promotion; `curriculum.promote` is a
//!   separate, approval-gated command.
//! * The block carries the original's checksum and the diff checksum; the diff
//!   checksum binds the candidate's content, so an edit after the review breaks it.
//! * Three new files are written, never over an existing one: the manifest,
//!   `<stem>.import.json` (the payload of `curriculum.import`) and
//!   `<stem>.promote.json` (the payload of `curriculum.promote`). The manifest is
//!   read back, validated and reconciled again; the command reports success only
//!   when the binding is `MATCHED` and promotion is allowed, otherwise it removes
//!   what it wrote.

use crate::cli::ReconcileArgs;
use crate::output::{next_command, CliError, NEXT_COMMAND_NOTE};
use g9_curriculum::{
    manifest_checksum, parse_manifest, reconcile, validate, DayDiffStatus, Manifest,
    Reconciliation, ReconciliationBinding, ReconciliationReport,
};
use g9_domain::{ActorId, CurriculumVersionId, G9Error, ManifestProvenance, UtcMillis};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Largest manifest file read; the shipped manifests are about 0.1 and 0.25 MiB.
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn read_manifest(path: &Path, which: &str) -> Result<Manifest, CliError> {
    let file = File::open(path).map_err(|e| CliError::io(which, &e))?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| CliError::io(which, &e))?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_MANIFEST_BYTES {
        return Err(CliError::Usage(format!(
            "the {which} manifest is larger than {} MiB",
            MAX_MANIFEST_BYTES / (1024 * 1024)
        )));
    }
    parse_manifest(&text)
        .map_err(|e| CliError::Usage(format!("the {which} manifest cannot be read: {e}")))
}

fn require_valid(manifest: &Manifest, which: &str) -> Result<(), CliError> {
    let report = validate(manifest);
    if report.is_valid() {
        return Ok(());
    }
    Err(CliError::Backend(
        G9Error::invalid_input(
            "curriculum.invariant_violated",
            "a manifest violates the curriculum invariants or the manifest contract; the diff is not computed",
        )
        .with_details(json!({ "manifest": which, "validation": report })),
    ))
}

fn facts(path: &Path, manifest: &Manifest, checksum: &str) -> Value {
    json!({
        "path": path_text(path),
        "curriculum_id": manifest.curriculum_id,
        "version": manifest.version,
        "provenance": manifest.provenance,
        "checksum": checksum,
    })
}

/// `now` or a positive UTC epoch time in milliseconds.
fn parse_approved_at(text: &str) -> Result<UtcMillis, CliError> {
    if text == "now" {
        return Ok(UtcMillis::now());
    }
    match text.parse::<i64>() {
        Ok(ms) if ms > 0 => Ok(UtcMillis::from_millis(ms)),
        _ => Err(CliError::Usage(
            "--approved-at is `now` or a positive UTC epoch time in milliseconds".to_string(),
        )),
    }
}

/// The diff as the operator reads it.
fn summarize(
    args: &ReconcileArgs,
    original: &Manifest,
    candidate: &Manifest,
    report: &ReconciliationReport,
) -> Value {
    // Days that changed in the same fields are one group, so a day that differs stands out.
    let mut groups: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut missing = Vec::new();
    let mut added = Vec::new();
    for row in &report.per_day {
        match &row.status {
            DayDiffStatus::Unchanged => {}
            DayDiffStatus::Changed { fields } => {
                match groups.iter_mut().find(|(known, _)| known == fields) {
                    Some((_, day_ids)) => day_ids.push(row.day_id.clone()),
                    None => groups.push((fields.clone(), vec![row.day_id.clone()])),
                }
            }
            DayDiffStatus::MissingInCandidate => missing.push(row.day_id.clone()),
            DayDiffStatus::AddedInCandidate => added.push(row.day_id.clone()),
        }
    }
    let changed_by_fields: Vec<Value> = groups
        .into_iter()
        .map(|(fields, day_ids)| json!({ "fields": fields, "day_ids": day_ids }))
        .collect();
    let s = &report.summary;
    let verdict = if report.promotion_allowed {
        "PRECONDITIONS MET: nothing differs, or the candidate carries a reconciliation block bound to this original and this diff. Promotion still needs an operator approval (curriculum.promote)."
    } else {
        "BLOCKED: the differences are not reconciled. Review them, then write the candidate with --write-candidate (promotion still needs an operator approval)."
    };
    json!({
        "original": facts(&args.original, original, report.original_checksum.as_str()),
        "candidate": {
            "path": path_text(&args.candidate),
            "curriculum_id": candidate.curriculum_id,
            "version": candidate.version,
            "provenance": candidate.provenance,
            "checksum": report.candidate_checksum.as_str(),
            "content_checksum": report.candidate_content_checksum.as_str(),
        },
        "curriculum_id_matches": report.curriculum_id_matches,
        "days": {
            "unchanged": s.unchanged,
            "changed": s.changed,
            "missing_in_candidate": s.missing_in_candidate,
            "added_in_candidate": s.added_in_candidate,
        },
        "metadata_changes": s.metadata_changes,
        "changed_by_fields": changed_by_fields,
        "missing_in_candidate": missing,
        "added_in_candidate": added,
        "binding": report.binding,
        "promotion_allowed": report.promotion_allowed,
        "verdict": verdict,
        "reconciliation_block_for_this_diff": {
            "original_checksum": report.original_checksum.as_str(),
            "diff_checksum": report.diff_checksum.as_str(),
        },
        "written": Value::Null,
    })
}

/// The three files `--write-candidate` creates.
struct Targets {
    manifest: PathBuf,
    import: PathBuf,
    promote: PathBuf,
}

impl Targets {
    fn of(manifest: &Path) -> Self {
        Self {
            manifest: manifest.to_path_buf(),
            import: manifest.with_extension("import.json"),
            promote: manifest.with_extension("promote.json"),
        }
    }

    fn all(&self) -> [&Path; 3] {
        [&self.manifest, &self.import, &self.promote]
    }
}

/// Create `path` (never over an existing file), write `text` and sync it.
fn create_new(path: &Path, text: &str) -> Result<(), CliError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                CliError::Backend(G9Error::conflict(
                    "reconcile.target_exists",
                    "a file to write already exists; it is never overwritten: choose a new name",
                ))
            } else {
                CliError::io("write", &e)
            }
        })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| CliError::io("write", &e))
}

fn pretty(value: &Value) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|_| CliError::Backend(G9Error::internal_redacted("cli.reconcile_serialize")))
}

fn command(argv: Vec<String>) -> Value {
    let (line, argv) = next_command(&argv);
    json!({ "command": line, "argv": argv })
}

/// Write the candidate, the import payload and the promote payload, then verify.
fn write_candidate(
    args: &ReconcileArgs,
    data_dir: Option<&Path>,
    original: &Manifest,
    candidate: &Manifest,
    report: &ReconciliationReport,
) -> Result<Value, CliError> {
    let (Some(target), Some(version), Some(approved_by), Some(approved_at)) = (
        args.write_candidate.as_deref(),
        args.candidate_version.as_deref(),
        args.approved_by.as_deref(),
        args.approved_at.as_deref(),
    ) else {
        return Err(CliError::Usage(
            "--write-candidate needs --candidate-version, --approved-by and --approved-at"
                .to_string(),
        ));
    };
    ActorId::parse(approved_by)
        .map_err(|e| CliError::Usage(format!("--approved-by is not an actor id: {}", e.reason)))?;
    let approved_at = parse_approved_at(approved_at)?;
    let version_id = format!("{}:{version}", candidate.curriculum_id);
    CurriculumVersionId::parse(&version_id).map_err(|e| {
        CliError::Usage(format!(
            "--candidate-version does not make a curriculum version id: {}",
            e.reason
        ))
    })?;
    if version == candidate.version {
        return Err(CliError::Usage(
            "--candidate-version must differ from the candidate's own version, or the two would be one version"
                .to_string(),
        ));
    }
    if !report.curriculum_id_matches {
        return Err(CliError::Backend(G9Error::precondition_failed(
            "curriculum.promotion_blocked",
            "the candidate describes a different curriculum than the original; no reconciliation block can allow that",
        )));
    }

    let mut written = candidate.clone();
    written.provenance = ManifestProvenance::ReconciledCandidate;
    written.version = version.to_string();
    written.reconciliation = Some(Reconciliation {
        original_checksum: report.original_checksum.as_str().to_string(),
        diff_checksum: report.diff_checksum.as_str().to_string(),
        approved_by: approved_by.to_string(),
        approved_at,
    });
    require_valid(&written, "written candidate")?;

    let targets = Targets::of(target);
    for path in targets.all() {
        if fs::symlink_metadata(path).is_ok() {
            return Err(CliError::Backend(G9Error::conflict(
                "reconcile.target_exists",
                "a file to write already exists; it is never overwritten: choose a new name",
            )));
        }
    }

    let written_value = serde_json::to_value(&written)
        .map_err(|_| CliError::Backend(G9Error::internal_redacted("cli.reconcile_serialize")))?;
    let original_value = serde_json::to_value(original)
        .map_err(|_| CliError::Backend(G9Error::internal_redacted("cli.reconcile_serialize")))?;
    let import_payload = json!({
        "manifest": written_value,
        "provenance": "RECONCILED_CANDIDATE",
    });
    let promote_payload = json!({
        "curriculum_version_id": version_id,
        "candidate_manifest": written_value,
        "original_manifest": original_value,
    });

    let mut created: Vec<&Path> = Vec::new();
    let result = (|| -> Result<(), CliError> {
        for (path, value) in [
            (&targets.manifest, &written_value),
            (&targets.import, &import_payload),
            (&targets.promote, &promote_payload),
        ] {
            create_new(path, &pretty(value)?)?;
            created.push(path);
        }
        // Read it back as the importer will and run the gate again.
        let back = read_manifest(&targets.manifest, "written candidate")?;
        require_valid(&back, "written candidate")?;
        let again = reconcile(original, &back).map_err(|e| CliError::Backend(G9Error::from(e)))?;
        if again.binding != ReconciliationBinding::Matched
            || !again.promotion_allowed
            || again.diff_checksum != report.diff_checksum
            || again.candidate_content_checksum != report.candidate_content_checksum
        {
            return Err(CliError::Backend(G9Error::internal_redacted(
                "cli.reconcile_write_unverified",
            )));
        }
        Ok(())
    })();
    if let Err(error) = result {
        for path in created {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }

    let mut base = vec!["g9".to_string()];
    if let Some(dir) = data_dir {
        base.push("--data-dir".to_string());
        base.push(path_text(dir));
    }
    let with = |tail: &[&str], path: &Path| {
        let mut argv = base.clone();
        argv.extend(tail.iter().map(|t| (*t).to_string()));
        argv.push("--payload-file".to_string());
        argv.push(path_text(path));
        argv
    };
    let checksum = manifest_checksum(&written).map_err(|e| CliError::Backend(G9Error::from(e)))?;
    Ok(json!({
        "manifest": path_text(&targets.manifest),
        "import_payload": path_text(&targets.import),
        "promote_payload": path_text(&targets.promote),
        "curriculum_version_id": version_id,
        "version": version,
        "provenance": "RECONCILED_CANDIDATE",
        "manifest_checksum": checksum.as_str(),
        "candidate_content_checksum": report.candidate_content_checksum.as_str(),
        "original_checksum": report.original_checksum.as_str(),
        "diff_checksum": report.diff_checksum.as_str(),
        "reviewed_by": approved_by,
        "reviewed_at": approved_at.to_rfc3339(),
        "binding": "MATCHED",
        "promotion_allowed": true,
        "note": "the reconciliation block records who reviewed the diff and when; it does not approve the promotion",
        "next": {
            "import": command(with(&["call", "curriculum.import"], &targets.import)),
            "promote": command(with(&["call", "curriculum.promote"], &targets.promote)),
            "steps": [
                "run the import command: it answers APPROVAL_REQUIRED with an approval id",
                "review the request with `g9 approvals --state PENDING`, then `g9 approve <approval id>`",
                "run the same import command again with --approval <approval id>",
                "do the same three steps for the promote command",
            ],
            "note": NEXT_COMMAND_NOTE,
        },
    }))
}

/// Compare, print, and (on explicit flags) write the candidate.
pub fn run(args: &ReconcileArgs, data_dir: Option<&Path>) -> Result<Value, CliError> {
    let original = read_manifest(&args.original, "original")?;
    let candidate = read_manifest(&args.candidate, "candidate")?;
    require_valid(&original, "original")?;
    require_valid(&candidate, "candidate")?;
    let report =
        reconcile(&original, &candidate).map_err(|e| CliError::Backend(G9Error::from(e)))?;
    let mut out = summarize(args, &original, &candidate, &report);
    if args.write_candidate.is_some() {
        out["written"] = write_candidate(args, data_dir, &original, &candidate, &report)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use clap::Parser;

    const SEED: &str = include_str!("../../../../data/curriculum/g9-ct9-480.seed.v0.json");
    const DRAFT: &str = include_str!("../../../../data/curriculum/g9-ct9-480.draft.v1.json");

    struct Dir {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Dir {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_path_buf();
            fs::write(root.join("seed.json"), SEED).unwrap();
            fs::write(root.join("draft.json"), DRAFT).unwrap();
            Self { _dir: dir, root }
        }

        fn p(&self, name: &str) -> String {
            path_text(&self.root.join(name))
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.root)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    fn args(extra: &[&str]) -> ReconcileArgs {
        let mut argv = vec!["g9", "reconcile"];
        argv.extend_from_slice(extra);
        match Cli::try_parse_from(argv).unwrap().command {
            Command::Reconcile(a) => a,
            other => panic!("not a reconcile command: {other:?}"),
        }
    }

    fn write_flags(d: &Dir, out: &str) -> Vec<String> {
        [
            "--original",
            &d.p("seed.json"),
            "--candidate",
            &d.p("draft.json"),
            "--write-candidate",
            &d.p(out),
            "--candidate-version",
            "1.0.0-rc1",
            "--approved-by",
            "local-operator",
            "--approved-at",
            "1763000000000",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    fn run_with(flags: &[String]) -> Result<Value, CliError> {
        let refs: Vec<&str> = flags.iter().map(String::as_str).collect();
        run(&args(&refs), None)
    }

    #[test]
    fn the_report_alone_writes_nothing_and_says_blocked() {
        let d = Dir::new();
        let before = d.names();
        let out = run(
            &args(&[
                "--original",
                &d.p("seed.json"),
                "--candidate",
                &d.p("draft.json"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(d.names(), before, "a report writes nothing");
        assert_eq!(out["days"]["changed"], 72);
        assert_eq!(out["days"]["unchanged"], 0);
        assert_eq!(out["promotion_allowed"], false);
        assert_eq!(out["binding"]["kind"], "ABSENT");
        assert!(out["verdict"].as_str().unwrap().starts_with("BLOCKED"));
        assert_eq!(out["metadata_changes"], json!([]));
        assert_eq!(out["written"], Value::Null);
        let block = &out["reconciliation_block_for_this_diff"];
        assert_eq!(block["diff_checksum"].as_str().unwrap().len(), 64);
        assert_eq!(
            block["original_checksum"].as_str().unwrap(),
            out["original"]["checksum"].as_str().unwrap()
        );
        // Every day changes in the same eight fields: one group of 72 days.
        let groups = out["changed_by_fields"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["day_ids"].as_array().unwrap().len(), 72);
        let fields = groups[0]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 8);
        assert!(fields.contains(&json!("evidence_artifact")));
        assert!(fields.contains(&json!("resources")));
        assert_eq!(out["missing_in_candidate"], json!([]));
    }

    #[test]
    fn writing_the_candidate_creates_three_files_that_pass_the_gate() {
        let d = Dir::new();
        let out = run_with(&write_flags(&d, "rc1.json")).unwrap();
        assert_eq!(
            d.names(),
            [
                "draft.json",
                "rc1.import.json",
                "rc1.json",
                "rc1.promote.json",
                "seed.json"
            ]
        );
        let written = &out["written"];
        assert_eq!(written["binding"], "MATCHED");
        assert_eq!(written["promotion_allowed"], true);
        assert_eq!(written["curriculum_version_id"], "g9-ct9-480:1.0.0-rc1");
        assert_eq!(written["reviewed_by"], "local-operator");
        assert_eq!(written["reviewed_at"], "2025-11-13T02:13:20.000Z");

        let seed = parse_manifest(SEED).unwrap();
        let rc = parse_manifest(&fs::read_to_string(d.root.join("rc1.json")).unwrap()).unwrap();
        assert_eq!(rc.provenance, ManifestProvenance::ReconciledCandidate);
        assert_eq!(rc.version, "1.0.0-rc1");
        assert!(validate(&rc).is_valid());
        let block = rc.reconciliation.as_ref().unwrap();
        assert_eq!(block.approved_by, "local-operator");
        assert_eq!(block.approved_at.as_millis(), 1_763_000_000_000);
        let again = reconcile(&seed, &rc).unwrap();
        assert_eq!(again.binding, ReconciliationBinding::Matched);
        assert!(again.promotion_allowed);
        assert_eq!(
            again.diff_checksum.as_str(),
            out["reconciliation_block_for_this_diff"]["diff_checksum"]
        );
        assert_eq!(
            manifest_checksum(&rc).unwrap().as_str(),
            written["manifest_checksum"]
        );

        let import: Value =
            serde_json::from_str(&fs::read_to_string(d.root.join("rc1.import.json")).unwrap())
                .unwrap();
        assert_eq!(import["provenance"], "RECONCILED_CANDIDATE");
        assert_eq!(import["manifest"]["version"], "1.0.0-rc1");
        let promote: Value =
            serde_json::from_str(&fs::read_to_string(d.root.join("rc1.promote.json")).unwrap())
                .unwrap();
        assert_eq!(promote["curriculum_version_id"], "g9-ct9-480:1.0.0-rc1");
        assert_eq!(promote["candidate_manifest"], import["manifest"]);
        assert_eq!(promote["original_manifest"]["version"], "0.1.0-seed");

        // The next commands name the files and carry no approval.
        let next = &written["next"];
        let line = next["import"]["command"].as_str().unwrap();
        assert!(line.contains("curriculum.import") && line.contains("rc1.import.json"));
        assert!(!line.contains("--approval"));
        assert!(next["promote"]["argv"]
            .as_array()
            .unwrap()
            .contains(&json!("curriculum.promote")));
    }

    #[test]
    fn an_edit_after_the_review_breaks_the_binding() {
        let d = Dir::new();
        run_with(&write_flags(&d, "rc1.json")).unwrap();
        // The reviewed candidate is edited: one objective is reworded after the review.
        let path = d.root.join("rc1.json");
        let mut rc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let text = rc["weeks"][2]["days"][1]["objectives"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        rc["weeks"][2]["days"][1]["objectives"][0]["text"] = json!(format!("{text} (reworded)"));
        fs::write(d.root.join("edited.json"), pretty(&rc).unwrap()).unwrap();
        let out = run(
            &args(&[
                "--original",
                &d.p("seed.json"),
                "--candidate",
                &d.p("edited.json"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(out["binding"]["kind"], "DIFF_CHECKSUM_MISMATCH");
        assert_eq!(out["promotion_allowed"], false);
    }

    #[test]
    fn nothing_is_overwritten_and_nothing_is_left_behind() {
        let d = Dir::new();
        fs::write(d.root.join("rc1.promote.json"), "mine").unwrap();
        let error = run_with(&write_flags(&d, "rc1.json")).unwrap_err();
        match error {
            CliError::Backend(e) => assert_eq!(e.code, "reconcile.target_exists"),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            fs::read_to_string(d.root.join("rc1.promote.json")).unwrap(),
            "mine"
        );
        assert!(!d.root.join("rc1.json").exists());
        assert!(!d.root.join("rc1.import.json").exists());
        // Writing over one of the inputs is the same refusal.
        let flags = write_flags(&d, "draft.json");
        assert!(matches!(run_with(&flags), Err(CliError::Backend(_))));
        assert_eq!(
            fs::read_to_string(d.root.join("draft.json")).unwrap(),
            DRAFT
        );
    }

    #[test]
    fn the_flags_that_record_the_review_are_explicit() {
        // clap refuses a write without the three flags, and the three without a write.
        let d = Dir::new();
        let base = [
            "g9",
            "reconcile",
            "--original",
            &d.p("seed.json"),
            "--candidate",
            &d.p("draft.json"),
        ];
        let mut only_write = base.to_vec();
        let out = d.p("rc1.json");
        only_write.extend(["--write-candidate", &out]);
        assert!(Cli::try_parse_from(&only_write).is_err());
        for flag in [
            ["--approved-by", "local-operator"],
            ["--approved-at", "now"],
            ["--candidate-version", "1.0.0-rc1"],
        ] {
            let mut alone = base.to_vec();
            alone.extend(flag);
            assert!(
                Cli::try_parse_from(&alone).is_err(),
                "{flag:?} without --write-candidate"
            );
        }
        assert_eq!(d.names(), ["draft.json", "seed.json"]);
    }

    #[test]
    fn bad_review_values_are_refused_before_anything_is_written() {
        let d = Dir::new();
        for (flag, value, needle) in [
            ("--approved-by", "bad actor!", "actor id"),
            ("--approved-at", "yesterday", "epoch"),
            ("--approved-at", "-5", "epoch"),
            ("--candidate-version", "has space", "version id"),
            ("--candidate-version", "1.0.0-draft1", "must differ"),
        ] {
            // `--flag=value`: clap itself refuses a space-separated value that starts with `-`.
            let mut flags = write_flags(&d, "rc1.json");
            let at = flags.iter().position(|f| f == flag).unwrap();
            flags[at] = format!("{flag}={value}");
            flags.remove(at + 1);
            match run_with(&flags) {
                Err(CliError::Usage(message)) => {
                    assert!(message.contains(needle), "{flag} {value}: {message}")
                }
                other => panic!("{flag} {value}: {other:?}"),
            }
            assert_eq!(d.names(), ["draft.json", "seed.json"], "{flag} {value}");
        }
        assert!(parse_approved_at("now").unwrap().as_millis() > 1_700_000_000_000);
    }

    #[test]
    fn an_invalid_manifest_is_refused_with_its_errors() {
        let d = Dir::new();
        let mut broken: Value = serde_json::from_str(DRAFT).unwrap();
        broken["weeks"][0]["days"][0]["planned_minutes"] = json!(1);
        fs::write(d.root.join("broken.json"), broken.to_string()).unwrap();
        let error = run(
            &args(&[
                "--original",
                &d.p("seed.json"),
                "--candidate",
                &d.p("broken.json"),
            ]),
            None,
        )
        .unwrap_err();
        match error {
            CliError::Backend(e) => {
                assert_eq!(e.code, "curriculum.invariant_violated");
                assert_eq!(e.details.unwrap()["manifest"], "candidate");
            }
            other => panic!("{other:?}"),
        }
        // Unreadable input is a usage error that does not echo the content.
        fs::write(d.root.join("junk.json"), "{ not json").unwrap();
        let error = run(
            &args(&[
                "--original",
                &d.p("junk.json"),
                "--candidate",
                &d.p("draft.json"),
            ]),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(error, CliError::Usage(m) if m.contains("original manifest cannot be read"))
        );
    }

    #[test]
    fn a_different_curriculum_cannot_be_written_as_a_candidate() {
        let d = Dir::new();
        let mut other: Value = serde_json::from_str(DRAFT).unwrap();
        other["curriculum_id"] = json!("g9-other");
        fs::write(d.root.join("other.json"), other.to_string()).unwrap();
        let mut flags = write_flags(&d, "rc1.json");
        let at = flags.iter().position(|f| f == "--candidate").unwrap();
        flags[at + 1] = d.p("other.json");
        match run_with(&flags) {
            Err(CliError::Backend(e)) => assert_eq!(e.code, "curriculum.promotion_blocked"),
            other => panic!("{other:?}"),
        }
        assert!(!d.root.join("rc1.json").exists());
    }
}
