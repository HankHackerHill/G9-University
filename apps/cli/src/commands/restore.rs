//! `g9 restore`: offline replacement of the live database with a verified backup
//! snapshot (ADR-0001; `docs/runbooks/backup-restore.md` § Restore).
//!
//! ```text
//! g9 restore --backup-id ID            [--data-dir DIR] [--confirm-replace-live-database]
//! g9 restore --snapshot PATH --expect-sha256 HEX [--data-dir DIR] [--confirm-replace-live-database]
//! ```
//!
//! The application must be stopped: a database that another connection holds is
//! refused (`restore.database_in_use`) and nothing is changed.
//!
//! * `--backup-id` reads the manifest from the live database through
//!   `g9_storage::backup::read_manifest_from_file` (read-only), requires the
//!   state `RETAINED` and a snapshot inside `<data dir>/backups`, and takes the
//!   checksum from the manifest. When the live database itself cannot be read,
//!   use `--snapshot` with the checksum recorded elsewhere.
//! * `--snapshot` restores an arbitrary file and therefore requires
//!   `--expect-sha256`.
//! * Without `--confirm-replace-live-database` the plan (snapshot, checksum,
//!   live path, quarantine directory) is printed and the command exits 1 having
//!   changed nothing, not even the data directory.
//! * With it, `g9_storage::restore::restore_over_live` swaps the file: the old
//!   database and its sidecars are moved to `<data dir>/quarantine` (never
//!   deleted), one metadata-only JSON line (timestamp, snapshot path, checksum,
//!   quarantined paths; no content, no key) is appended to
//!   `<data dir>/restore-log.jsonl`, then the normal boot runs (recovery, and
//!   migrations with their own pre-migration backup) and its report is printed.
//!
//! The CLI writes no SQL: every database access is a `g9-storage` function.

use super::init;
use crate::bootstrap::{
    self, BootOptions, BACKUPS_DIR, DATABASE_FILE, QUARANTINE_DIR, RESTORE_LOG_FILE,
};
use crate::cli::RestoreArgs;
use crate::output::CliError;
use g9_domain::{BackupId, BackupState, G9Error, UtcMillis};
use g9_storage::{backup, restore_over_live, RestoreCode, RestoreOptions, StorageError};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Everything the restore will do, resolved before anything is changed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
    root: PathBuf,
    backup_id: Option<String>,
    source: &'static str,
    snapshot: PathBuf,
    expected_sha256: String,
    live: PathBuf,
    quarantine_dir: PathBuf,
    restore_log: PathBuf,
}

/// Run `restore`.
pub fn run(boot: &BootOptions, args: &RestoreArgs) -> Result<Value, CliError> {
    let confirmed = args.confirm_replace_live_database;
    // Refused before anything (the data directory included) is created.
    require_utf8(&boot.data_dir, "the data directory")?;
    if let Some(snapshot) = &args.snapshot {
        require_utf8(snapshot, "the snapshot path")?;
    }
    // Only a confirmed restore of a snapshot may create the data directory (a
    // fresh machine); planning and manifest lookups never create anything.
    let root = if confirmed && args.snapshot.is_some() {
        bootstrap::prepare_root(&boot.data_dir)?
    } else {
        data_root(&boot.data_dir)?
    };
    let plan = plan(&root, args)?;

    if !confirmed {
        let actual = backup::sha256_of_file(&plan.snapshot).map_err(G9Error::from)?;
        return Err(CliError::Unconfirmed(plan_value(&plan, &actual)));
    }

    // Never swap the file under a running backend (app-4): the same lock the
    // desktop bridge and every CLI command take before recovery, held for the
    // swap only (the backend opened below takes it again).
    let now = UtcMillis::now();
    let report = {
        let _data_lock = g9_app::lock::DataLock::acquire(&plan.live)?;
        restore_over_live(
            &plan.snapshot,
            &plan.live,
            &plan.quarantine_dir,
            &RestoreOptions {
                expect_sha256: Some(plan.expected_sha256.clone()),
                now,
            },
        )
        .map_err(G9Error::from)?
    };

    // Metadata only: a failure to write the log must not hide a restore that happened.
    let quarantined: Vec<String> = report
        .quarantined
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let log_line = json!({
        "timestamp": now.as_millis(),
        "snapshot": text(&plan.snapshot),
        "sha256": report.snapshot_sha256,
        "quarantined": quarantined,
    });
    let log_error = bootstrap::append_private_line(&plan.restore_log, &log_line.to_string())
        .err()
        .map(|e| e.kind());

    let booted = init::run(boot).map_err(|e| after_restore_failure(e, &quarantined))?;

    let mut warnings = Vec::new();
    if let Some(kind) = log_error {
        warnings.push(format!(
            "WARNING restore.log_not_written: {RESTORE_LOG_FILE} could not be appended ({kind}); the restore itself completed"
        ));
    }
    Ok(json!({
        "restored": true,
        "backup_id": plan.backup_id,
        "snapshot": text(&plan.snapshot),
        "snapshot_sha256": report.snapshot_sha256,
        "schema_version": report.schema_version,
        "audit_rows_verified": report.audit_rows_verified,
        "live": text(&report.live),
        "quarantine_dir": text(&plan.quarantine_dir),
        "quarantined": quarantined,
        "restore_log": text(&plan.restore_log),
        "restore_log_written": log_error.is_none(),
        "warnings": warnings,
        "boot": booted,
    }))
}

/// The data directory as an absolute path, canonical when it exists; no creation.
fn data_root(data_dir: &Path) -> Result<PathBuf, CliError> {
    let absolute = bootstrap::absolute_path(data_dir)?;
    Ok(fs::canonicalize(&absolute).unwrap_or(absolute))
}

fn plan(root: &Path, args: &RestoreArgs) -> Result<Plan, CliError> {
    let live = root.join(DATABASE_FILE);
    let given = args
        .expect_sha256
        .as_deref()
        .map(normalize_sha256)
        .transpose()?;
    let (snapshot, expected_sha256, backup_id, source) = match (&args.backup_id, &args.snapshot) {
        (Some(raw), None) => {
            let (snapshot, manifest_sha) = from_manifest(root, &live, raw)?;
            if given.as_ref().is_some_and(|g| *g != manifest_sha) {
                return Err(G9Error::from(StorageError::restore(
                    RestoreCode::ChecksumMismatch,
                    "--expect-sha256 differs from the checksum recorded in the manifest",
                ))
                .into());
            }
            (snapshot, manifest_sha, Some(raw.clone()), "manifest")
        }
        (None, Some(path)) => {
            let expected = given.ok_or_else(|| {
                CliError::Usage("--snapshot requires --expect-sha256".to_string())
            })?;
            (bootstrap::absolute_path(path)?, expected, None, "snapshot")
        }
        _ => {
            return Err(CliError::Usage(
                "exactly one of --backup-id and --snapshot is required".to_string(),
            ))
        }
    };
    // Every path is printed as JSON text and recorded in `restore-log.jsonl`.
    // `serde_json` refuses a non-UTF-8 path (the `json!` macro would panic, after
    // the swap on a confirmed run) and a lossy spelling would name another file,
    // so such a plan is refused before anything is changed.
    require_utf8(root, "the data directory")?;
    require_utf8(&snapshot, "the snapshot path")?;
    Ok(Plan {
        root: root.to_path_buf(),
        backup_id,
        source,
        snapshot,
        expected_sha256,
        quarantine_dir: root.join(QUARANTINE_DIR),
        restore_log: root.join(RESTORE_LOG_FILE),
        live,
    })
}

fn require_utf8(path: &Path, what: &str) -> Result<(), CliError> {
    if path.to_str().is_some() {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "{what} is not valid UTF-8; g9 restore reports and logs paths as JSON text (rename it)"
        )))
    }
}

/// A path as JSON text. Plan paths are valid UTF-8 (see [`require_utf8`]); paths
/// the restore reports back (resolved symlinks) are converted lossily so that no
/// output can ever panic.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Snapshot path and checksum of a `RETAINED` manifest in the live database.
fn from_manifest(root: &Path, live: &Path, raw_id: &str) -> Result<(PathBuf, String), CliError> {
    let id = BackupId::parse(raw_id)
        .map_err(|_| CliError::Usage(format!("--backup-id {raw_id:?} is not a valid backup id")))?;
    if !live.is_file() {
        return Err(G9Error::precondition_failed(
            "restore.live_database_missing",
            "there is no database in the data directory to read the manifest from; restore with --snapshot and --expect-sha256",
        )
        .into());
    }
    let stored = backup::read_manifest_from_file(live, &id)
        .map_err(G9Error::from)?
        .ok_or_else(|| {
            G9Error::not_found(
                "restore.backup_not_found",
                "the live database has no backup manifest with this id",
            )
        })?;
    let manifest = stored.manifest;
    if manifest.state != BackupState::Retained {
        return Err(G9Error::precondition_failed(
            "restore.backup_not_retained",
            format!(
                "the backup is {}; only RETAINED backups can be restored (run backup.verify and backup.restore_test first)",
                manifest.state
            ),
        )
        .into());
    }
    let recorded = manifest.path.ok_or_else(|| {
        G9Error::precondition_failed(
            "restore.backup_has_no_snapshot",
            "the manifest records no snapshot file",
        )
    })?;
    let snapshot = fs::canonicalize(&recorded).map_err(|e| {
        G9Error::not_found(
            "restore.snapshot_missing",
            format!(
                "the recorded snapshot file cannot be resolved ({:?})",
                e.kind()
            ),
        )
    })?;
    let inside = fs::canonicalize(root.join(BACKUPS_DIR))
        .map(|dir| snapshot.starts_with(dir))
        .unwrap_or(false);
    if !inside {
        return Err(G9Error::unauthorized(
            "restore.snapshot_outside_backups",
            "the recorded snapshot is not inside <data dir>/backups; restore it with --snapshot and --expect-sha256 if it is the file you mean",
        )
        .into());
    }
    Ok((snapshot, manifest.sha256_of_file.to_ascii_lowercase()))
}

fn normalize_sha256(text: &str) -> Result<String, CliError> {
    let text = text.trim();
    if text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(text.to_ascii_lowercase())
    } else {
        Err(CliError::Usage(
            "--expect-sha256 must be a 64-character hex SHA-256".to_string(),
        ))
    }
}

fn plan_value(plan: &Plan, actual_sha256: &str) -> Value {
    let matches = actual_sha256 == plan.expected_sha256;
    let mut command = String::from("g9 restore");
    match &plan.backup_id {
        Some(id) => command.push_str(&format!(" --backup-id {id}")),
        None => command.push_str(&format!(
            " --snapshot {} --expect-sha256 {}",
            plan.snapshot.display(),
            plan.expected_sha256
        )),
    }
    command.push_str(&format!(
        " --data-dir {} --confirm-replace-live-database",
        plan.root.display()
    ));
    json!({
        "restore": "PLAN",
        "confirmed": false,
        "changed": false,
        "source": plan.source,
        "backup_id": plan.backup_id,
        "snapshot": text(&plan.snapshot),
        "expected_sha256": plan.expected_sha256,
        // The actual hash is printed only when it already matches: printed next to
        // the confirmation command on a mismatch, it would invite copying it into
        // --expect-sha256, which then authenticates nothing (security audit
        // 2026-11-12). Compare against this installation's manifest or your own record.
        "snapshot_sha256": if matches { Value::String(actual_sha256.to_string()) } else { Value::Null },
        "snapshot_matches_expected": matches,
        "checksum": if matches {
            "the snapshot's SHA-256 matches the expected value"
        } else {
            "the snapshot's SHA-256 does not match the expected value; it is not shown here. Take the expected value from this installation's backup manifest (--backup-id) or from your own record of the file, never from this output"
        },
        "live": text(&plan.live),
        "live_exists": plan.live.exists(),
        "quarantine_dir": text(&plan.quarantine_dir),
        "restore_log": text(&plan.restore_log),
        "note": "stop the application first; the live database and its sidecars are moved to the quarantine directory, never deleted",
        "to_confirm": command,
    })
}

/// The swap succeeded but the boot after it did not: say so, with what moved.
fn after_restore_failure(error: CliError, quarantined: &[String]) -> CliError {
    match error {
        CliError::Backend(e) => CliError::Backend(e.with_details(json!({
            "restore_completed": true,
            "quarantined": quarantined,
        }))),
        other => CliError::Io(format!(
            "the restore completed ({} file(s) quarantined) but the boot after it failed: {other}",
            quarantined.len()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(backup_id: Option<&str>, snapshot: Option<&str>, sha: Option<&str>) -> RestoreArgs {
        RestoreArgs {
            backup_id: backup_id.map(str::to_string),
            snapshot: snapshot.map(PathBuf::from),
            expect_sha256: sha.map(str::to_string),
            confirm_replace_live_database: false,
        }
    }

    #[test]
    fn a_snapshot_needs_a_well_formed_checksum() {
        let root = Path::new("/data");
        let hex = "AB".repeat(32);
        let p = plan(root, &args(None, Some("/s/snap.sqlite"), Some(&hex))).unwrap();
        assert_eq!(p.expected_sha256, "ab".repeat(32));
        assert_eq!(p.source, "snapshot");
        assert_eq!(p.live, Path::new("/data/g9.sqlite"));
        assert_eq!(p.quarantine_dir, Path::new("/data/quarantine"));
        assert_eq!(p.restore_log, Path::new("/data/restore-log.jsonl"));
        assert!(matches!(
            plan(root, &args(None, Some("/s/snap.sqlite"), Some("xyz"))),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            plan(root, &args(None, Some("/s/snap.sqlite"), None)),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            plan(root, &args(None, None, None)),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn a_malformed_backup_id_is_a_usage_error_and_a_missing_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            plan(dir.path(), &args(Some("not a valid id!"), None, None)),
            Err(CliError::Usage(_))
        ));
        match plan(dir.path(), &args(Some("bk-1"), None, None)) {
            Err(CliError::Backend(e)) => assert_eq!(e.code, "restore.live_database_missing"),
            other => panic!("unexpected {other:?}"),
        }
        // Planning created nothing.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_path_that_is_not_utf8_is_a_usage_error_not_a_panic() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let bad = PathBuf::from(OsString::from_vec(b"/data/sn\xffap.sqlite".to_vec()));
        let sha = "ab".repeat(32);
        let mut with_bad_snapshot = args(None, None, Some(&sha));
        with_bad_snapshot.snapshot = Some(bad.clone());
        assert!(matches!(
            plan(Path::new("/data"), &with_bad_snapshot),
            Err(CliError::Usage(m)) if m.contains("UTF-8")
        ));
        assert!(matches!(
            plan(&bad, &args(None, Some("/s/snap.sqlite"), Some(&sha))),
            Err(CliError::Usage(m)) if m.contains("UTF-8")
        ));
        // Output conversion never fails.
        assert!(text(&bad).contains('\u{fffd}'));
    }

    #[test]
    fn the_plan_lists_the_paths_and_the_exact_confirming_command() {
        let p = plan(
            Path::new("/data"),
            &args(None, Some("/s/snap.sqlite"), Some(&"cd".repeat(32))),
        )
        .unwrap();
        let v = plan_value(&p, &"cd".repeat(32));
        assert_eq!(v["confirmed"], false);
        assert_eq!(v["changed"], false);
        assert_eq!(v["snapshot_matches_expected"], true);
        assert_eq!(v["live"], "/data/g9.sqlite");
        assert_eq!(v["quarantine_dir"], "/data/quarantine");
        let command = v["to_confirm"].as_str().unwrap();
        assert!(command.ends_with("--data-dir /data --confirm-replace-live-database"));
        assert_eq!(
            plan_value(&p, &"00".repeat(32))["snapshot_matches_expected"],
            false
        );
        assert_eq!(
            plan_value(&p, &"00".repeat(32))["snapshot_sha256"],
            Value::Null,
            "a mismatching hash is never printed next to the confirmation command"
        );
    }
}
