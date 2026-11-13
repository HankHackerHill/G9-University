//! `g9 copy-backup`: copy a RETAINED backup to another directory (a USB drive,
//! a NAS mount) so a copy survives the loss of this disk.
//!
//! * Only a RETAINED backup of this installation is copied (restore-tested;
//!   looked up read-only in the live database, as `g9 restore --backup-id`
//!   does). The application may be running: nothing in the data directory is
//!   written.
//! * The snapshot is written as `<dest>.partial`, synced, renamed and the
//!   directory synced (the same durable sequence as the backup itself); an
//!   existing file is never overwritten.
//! * The copy is re-hashed against the manifest's checksum, and
//!   `<backup_id>.manifest.json` is written beside it, so the copy can be
//!   restored with `g9 restore --snapshot <copy> --expect-sha256 <sha256>` even
//!   when this disk is gone.

use crate::bootstrap::{BootOptions, DATABASE_FILE};
use crate::cli::CopyBackupArgs;
use crate::commands::restore::{data_root, from_manifest};
use crate::CliError;
use g9_domain::G9Error;
use g9_storage::backup::{sha256_of_file, sync_dir, sync_file};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Copy the backup and report where it went.
pub fn run(boot: &BootOptions, args: &CopyBackupArgs) -> Result<Value, CliError> {
    let root = data_root(&boot.data_dir)?;
    let live = root.join(DATABASE_FILE);
    let (snapshot, sha256) = from_manifest(&root, &live, &args.backup_id)?;
    let dest_dir = fs::canonicalize(&args.to)
        .ok()
        .filter(|d| d.is_dir())
        .ok_or_else(|| {
            G9Error::precondition_failed(
                "copy_backup.destination_missing",
                "the destination directory does not exist; mount the drive and create the folder first",
            )
        })?;
    let file_name = snapshot.file_name().ok_or_else(|| {
        G9Error::precondition_failed(
            "copy_backup.snapshot_unnamed",
            "the snapshot path has no file name",
        )
    })?;
    let dest = dest_dir.join(file_name);
    let manifest_path = dest_dir.join(format!("{}.manifest.json", args.backup_id));
    for path in [&dest, &manifest_path] {
        if path.exists() {
            return Err(G9Error::conflict(
                "copy_backup.destination_exists",
                format!(
                    "{} already exists; copies never overwrite",
                    path.to_string_lossy()
                ),
            )
            .into());
        }
    }

    durable_copy(&snapshot, &dest)?;
    let copied = sha256_of_file(&dest).map_err(G9Error::from)?;
    if copied != sha256 {
        let _ = fs::remove_file(&dest);
        return Err(G9Error::precondition_failed(
            "copy_backup.checksum_mismatch",
            "the copy does not match the manifest's checksum and was removed",
        )
        .into());
    }
    let record = json!({
        "backup_id": args.backup_id,
        "snapshot_file": file_name.to_string_lossy(),
        "sha256": sha256,
        "restore": format!(
            "g9 restore --snapshot <path to {}> --expect-sha256 {sha256} --confirm-replace-live-database",
            file_name.to_string_lossy()
        ),
    });
    let text = serde_json::to_string_pretty(&record)
        .map_err(|_| G9Error::internal_redacted("copy_backup.manifest_serialize"))?;
    write_durably(&manifest_path, text.as_bytes())?;
    Ok(json!({
        "backup_id": args.backup_id,
        "copied_to": dest.to_string_lossy(),
        "manifest": manifest_path.to_string_lossy(),
        "sha256": sha256,
        "verified": true,
    }))
}

/// `<dest>.partial` → sync → rename → sync the directory.
fn durable_copy(from: &Path, dest: &Path) -> Result<(), CliError> {
    let partial = partial_of(dest);
    let _ = fs::remove_file(&partial);
    fs::copy(from, &partial).map_err(|e| io_error("copy the snapshot", &e))?;
    finish(&partial, dest)
}

fn write_durably(dest: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let partial = partial_of(dest);
    fs::write(&partial, bytes).map_err(|e| io_error("write the manifest", &e))?;
    finish(&partial, dest)
}

fn finish(partial: &Path, dest: &Path) -> Result<(), CliError> {
    sync_file(partial).map_err(G9Error::from)?;
    fs::rename(partial, dest).map_err(|e| io_error("rename the copy", &e))?;
    if let Some(dir) = dest.parent() {
        sync_dir(dir).map_err(G9Error::from)?;
    }
    Ok(())
}

fn partial_of(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

fn io_error(what: &str, e: &std::io::Error) -> CliError {
    G9Error::resource_unavailable(
        "copy_backup.io",
        format!("could not {what} ({:?})", e.kind()),
    )
    .into()
}
