//! Backup, verification and restore (M17; ARCHITECTURE.md §6 "Backup",
//! "Restore"; runbook `docs/runbooks/backup-restore.md`; handoff §16 Backup
//! machine; corrections C04/C05/C12).
//!
//! * Snapshots use the SQLite Online Backup API from the application's own
//!   connection into a temporary file (`<dest>.partial`), which is renamed to
//!   `dest` only after the copy completed and its table counts match the source.
//!   The copy is converted to rollback-journal mode so that the snapshot is one
//!   self-contained file. The live database file is never copied with the
//!   filesystem and `.dump` is never used.
//! * [`verify_snapshot`] opens the copy read-only and runs `PRAGMA integrity_check`,
//!   `PRAGMA foreign_key_check`, migration checksum verification, audit chain
//!   verification, table counts and (when an attachment root is given) the
//!   attachment manifest check.
//! * [`restore_to_copy`] always restores into a *new* file; it never overwrites
//!   the live database. Swapping the live file is an application decision that
//!   needs an operator approval (runbook § Restore).
//! * [`BackupManifest`] drives the `BackupState` machine
//!   Requested → SnapshotCreated → Checked → RestoreTested → Retained; failures
//!   are explicit (`Failed` with a reason).
//! * [`vacuum_into`] is the alternative for compact exports.

use crate::audit::AuditChain;
use crate::error::{StorageError, StorageResult};
use crate::migrations::{foreign_key_check, integrity_check, table_counts, Migrator};
use crate::sql::enum_to_text;
use g9_domain::state::BackupEvent;
use g9_domain::{BackupId, BackupState, UtcMillis};
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Bounded number of `Busy`/`Locked` retries while copying pages.
const MAX_BUSY_RETRIES: u32 = 100;
const BUSY_PAUSE: Duration = Duration::from_millis(25);

/// Request for a new snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRequest {
    /// Manifest id (also the default file stem).
    pub backup_id: BackupId,
    /// Instant recorded as `created_at`.
    pub requested_at: UtcMillis,
    /// Application version creating the snapshot.
    pub app_version: String,
}

/// Audit chain position captured with a snapshot (periodic anchor).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditAnchor {
    /// `audit_metadata.seq` of the chain head at snapshot time.
    pub seq: i64,
    /// `row_hash` of that row.
    pub row_hash: String,
}

/// Attachment manifest check outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttachmentCheck {
    /// No attachment root was supplied; the manifest was not checked.
    NotChecked,
    /// Every non-erased `attachments.storage_ref` was resolved under the root.
    Checked {
        /// Number of attachment rows examined.
        expected: u64,
        /// Storage refs that were missing or unreadable.
        missing: Vec<String>,
    },
}

/// Result of [`verify_snapshot`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    /// `PRAGMA integrity_check` returned `ok`.
    pub integrity_check_ok: bool,
    /// `PRAGMA foreign_key_check` found no violation.
    pub foreign_key_check_ok: bool,
    /// Every `schema_migrations` checksum matches this build and the schema is not newer.
    pub migration_checksums_ok: bool,
    /// The audit chain recomputed cleanly.
    pub audit_chain_ok: bool,
    /// Row count per application table.
    pub table_counts: BTreeMap<String, u64>,
    /// Attachment manifest result.
    pub attachment_manifest: AttachmentCheck,
    /// Human-readable problems (metadata only).
    pub errors: Vec<String>,
}

impl VerificationReport {
    /// `true` when every check passed (an unchecked attachment manifest does not fail verification).
    pub fn ok(&self) -> bool {
        self.integrity_check_ok
            && self.foreign_key_check_ok
            && self.migration_checksums_ok
            && self.audit_chain_ok
            && self.errors.is_empty()
            && !matches!(&self.attachment_manifest, AttachmentCheck::Checked { missing, .. } if !missing.is_empty())
    }
}

/// Options for [`verify_snapshot`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyOptions {
    /// Root directory of the content-addressed attachment store; when set, every
    /// non-erased attachment's `storage_ref` must exist under it.
    pub attachment_root: Option<PathBuf>,
}

/// Backup set manifest (`backup_manifests` row + state machine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    /// Manifest id.
    pub backup_id: BackupId,
    /// When the snapshot was requested/created.
    pub created_at: UtcMillis,
    /// Application version.
    pub app_version: String,
    /// Schema version of the source at snapshot time.
    pub schema_version: u32,
    /// Page count of the snapshot file.
    pub page_count: u64,
    /// Hex SHA-256 of the snapshot file bytes.
    pub sha256_of_file: String,
    /// Current state.
    pub state: BackupState,
    /// Snapshot path (when created).
    pub path: Option<PathBuf>,
    /// Audit chain anchor at snapshot time.
    pub audit_anchor: Option<AuditAnchor>,
    /// Verification report (when checked).
    pub verification: Option<VerificationReport>,
    /// When the restore test succeeded.
    pub restore_tested_at: Option<UtcMillis>,
    /// Failure reason (when `Failed`).
    pub failure_reason: Option<String>,
}

impl BackupManifest {
    /// A manifest in `Requested` state.
    pub fn requested(request: &SnapshotRequest) -> Self {
        Self {
            backup_id: request.backup_id.clone(),
            created_at: request.requested_at,
            app_version: request.app_version.clone(),
            schema_version: 0,
            page_count: 0,
            sha256_of_file: "0".repeat(64),
            state: BackupState::Requested,
            path: None,
            audit_anchor: None,
            verification: None,
            restore_tested_at: None,
            failure_reason: None,
        }
    }

    /// Apply a `BackupEvent` through the domain state machine.
    pub fn transition(&mut self, event: BackupEvent) -> StorageResult<BackupState> {
        self.state = self.state.transition(event)?;
        Ok(self.state)
    }

    /// Move to `Failed` with a reason (metadata only).
    pub fn fail(&mut self, reason: impl Into<String>) -> StorageResult<()> {
        self.failure_reason = Some(reason.into());
        self.transition(BackupEvent::Fail)?;
        Ok(())
    }

    /// Mark the set retained (allowed only after a restore test).
    pub fn retain(&mut self) -> StorageResult<()> {
        self.transition(BackupEvent::Retain)?;
        Ok(())
    }
}

fn path_str(path: &Path) -> StorageResult<&str> {
    path.to_str()
        .ok_or_else(|| StorageError::InvalidInput("path is not valid UTF-8".to_string()))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Copy every page of `src` into a brand-new database file at `dest` with the
/// Online Backup API, then switch the copy to rollback-journal mode.
fn copy_with_backup_api(src: &Connection, dest: &Path) -> StorageResult<u64> {
    if dest.exists() {
        return Err(StorageError::Conflict(
            "destination already exists; backups never overwrite".to_string(),
        ));
    }
    let mut dst = Connection::open_with_flags(
        dest,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    {
        let backup = Backup::new(src, &mut dst)?;
        let mut retries = 0_u32;
        loop {
            match backup.step(-1)? {
                StepResult::Done => break,
                StepResult::More => continue,
                StepResult::Busy | StepResult::Locked => {
                    retries += 1;
                    if retries > MAX_BUSY_RETRIES {
                        return Err(StorageError::Busy("backup source stayed busy".to_string()));
                    }
                    std::thread::sleep(BUSY_PAUSE);
                }
                _ => return Err(StorageError::Internal("backup step")),
            }
        }
    }
    let _mode: String = dst.query_row("PRAGMA journal_mode = DELETE", [], |r| r.get(0))?;
    let page_count: i64 = dst.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    dst.close().map_err(|(_, e)| StorageError::from(e))?;
    Ok(page_count.max(0) as u64)
}

/// Hex SHA-256 of a file's bytes.
pub fn sha256_of_file(path: &Path) -> StorageResult<String> {
    let mut file = std::fs::File::open(path).map_err(|e| StorageError::io("open snapshot", &e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| StorageError::io("read snapshot", &e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Create a snapshot of `src` at `dest_path` (Requested → SnapshotCreated).
///
/// Refuses to write over an existing file or onto the source path itself.
/// Table counts of the copy are compared with the source after the copy.
pub fn create_snapshot(
    src: &Connection,
    dest_path: &Path,
    request: &SnapshotRequest,
) -> StorageResult<BackupManifest> {
    let mut manifest = BackupManifest::requested(request);
    if let Some(live) = src.path().filter(|p| !p.is_empty()) {
        if same_file(Path::new(live), dest_path) {
            manifest.fail("destination equals the live database path")?;
            return Err(StorageError::InvalidInput(
                "snapshot destination must not be the live database".to_string(),
            ));
        }
    }
    if dest_path.exists() {
        return Err(StorageError::Conflict(
            "snapshot destination already exists; backups never overwrite".to_string(),
        ));
    }
    let partial = dest_path.with_extension("partial");
    if partial.exists() {
        std::fs::remove_file(&partial).map_err(|e| StorageError::io("remove stale partial", &e))?;
    }
    let source_counts = table_counts(src)?;
    let schema_version = Migrator::schema_version(src)?;
    let anchor = AuditChain::head(src)?.map(|(seq, row_hash)| AuditAnchor { seq, row_hash });

    let page_count = match copy_with_backup_api(src, &partial) {
        Ok(n) => n,
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            manifest.fail(e.code())?;
            return Err(e);
        }
    };
    // Relationship invariant on the copy: every table has the same row count as the source.
    let copy = Connection::open_with_flags(
        &partial,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let copy_counts = table_counts(&copy)?;
    drop(copy);
    if copy_counts != source_counts {
        let _ = std::fs::remove_file(&partial);
        manifest.fail("row counts differ between source and snapshot")?;
        return Err(StorageError::Verification(vec![
            "row counts differ between source and snapshot".to_string(),
        ]));
    }
    let sha = sha256_of_file(&partial)?;
    std::fs::rename(&partial, dest_path).map_err(|e| StorageError::io("rename snapshot", &e))?;

    manifest.schema_version = schema_version;
    manifest.page_count = page_count;
    manifest.sha256_of_file = sha;
    manifest.path = Some(dest_path.to_path_buf());
    manifest.audit_anchor = anchor;
    manifest.transition(BackupEvent::Snapshot)?;
    Ok(manifest)
}

/// Verify a snapshot file (opened read-only; the file is not modified).
pub fn verify_snapshot(path: &Path, options: &VerifyOptions) -> StorageResult<VerificationReport> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "trusted_schema", false)?;
    verify_connection(&conn, options)
}

/// Verification checks on an already-open connection (used for snapshots and restored copies).
pub fn verify_connection(
    conn: &Connection,
    options: &VerifyOptions,
) -> StorageResult<VerificationReport> {
    let mut errors = Vec::new();
    let integrity = integrity_check(conn)?;
    let integrity_check_ok = integrity.len() == 1 && integrity[0] == "ok";
    if !integrity_check_ok {
        errors.push(format!(
            "integrity_check returned {} message(s)",
            integrity.len()
        ));
    }
    let fk = foreign_key_check(conn)?;
    let foreign_key_check_ok = fk == 0;
    if !foreign_key_check_ok {
        errors.push(format!("foreign_key_check found {fk} violation(s)"));
    }
    let migration_checksums_ok = match Migrator::verify_checksums(conn) {
        Ok(report)
            if report.ok
                && report.db_schema_version <= crate::migrations::CURRENT_SCHEMA_VERSION =>
        {
            true
        }
        Ok(report) => {
            errors.push(format!(
                "migration checksums: {} mismatch(es), db version {}",
                report.mismatches.len(),
                report.db_schema_version
            ));
            false
        }
        Err(e) => {
            errors.push(format!("migration checksums unreadable: {}", e.code()));
            false
        }
    };
    let audit_chain_ok = match AuditChain::verify_chain(conn) {
        Ok(report) if report.ok => true,
        Ok(report) => {
            errors.push(format!(
                "audit chain broken at seq {:?}: {}",
                report.first_bad_seq,
                report.first_bad_reason.unwrap_or_default()
            ));
            false
        }
        Err(e) => {
            errors.push(format!("audit chain unreadable: {}", e.code()));
            false
        }
    };
    let counts = table_counts(conn)?;
    let attachment_manifest = match &options.attachment_root {
        None => AttachmentCheck::NotChecked,
        Some(root) => check_attachments(conn, root)?,
    };
    Ok(VerificationReport {
        integrity_check_ok,
        foreign_key_check_ok,
        migration_checksums_ok,
        audit_chain_ok,
        table_counts: counts,
        attachment_manifest,
        errors,
    })
}

fn check_attachments(conn: &Connection, root: &Path) -> StorageResult<AttachmentCheck> {
    let mut stmt =
        conn.prepare("SELECT storage_ref FROM attachments WHERE erased = 0 ORDER BY id")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut expected = 0_u64;
    let mut missing = Vec::new();
    for row in rows {
        let storage_ref = row?;
        expected += 1;
        let rel = Path::new(&storage_ref);
        let safe = !rel.is_absolute()
            && rel
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !safe || !root.join(rel).is_file() {
            missing.push(storage_ref);
        }
    }
    Ok(AttachmentCheck::Checked { expected, missing })
}

/// Verify the snapshot named in the manifest and advance SnapshotCreated → Checked
/// (or → Failed). Returns the report either way.
pub fn verify_and_advance(
    manifest: &mut BackupManifest,
    options: &VerifyOptions,
) -> StorageResult<VerificationReport> {
    let path = manifest
        .path
        .clone()
        .ok_or_else(|| StorageError::Precondition("manifest has no snapshot path".to_string()))?;
    let current_sha = sha256_of_file(&path)?;
    // A snapshot that cannot even be opened or checked is a failed verification:
    // the manifest is driven to `Failed` before the error is returned.
    let mut report = match verify_snapshot(&path, options) {
        Ok(report) => report,
        Err(e) => {
            manifest.fail("verification failed")?;
            return Err(e);
        }
    };
    if current_sha != manifest.sha256_of_file {
        report
            .errors
            .push("snapshot file hash differs from the manifest".to_string());
    }
    if report.ok() {
        manifest.verification = Some(report.clone());
        manifest.transition(BackupEvent::Check)?;
        Ok(report)
    } else {
        manifest.verification = Some(report.clone());
        manifest.fail("verification failed")?;
        Err(StorageError::Verification(report.errors))
    }
}

/// Restore `backup_path` into a *new* file at `dest_path` using the Backup API
/// and verify the result. Never overwrites an existing file.
pub fn restore_to_copy(backup_path: &Path, dest_path: &Path) -> StorageResult<PathBuf> {
    if dest_path.exists() {
        return Err(StorageError::Conflict(
            "restore destination already exists; restores never overwrite".to_string(),
        ));
    }
    if same_file(backup_path, dest_path) {
        return Err(StorageError::InvalidInput(
            "restore destination must differ from the backup file".to_string(),
        ));
    }
    let src = Connection::open_with_flags(
        backup_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    copy_with_backup_api(&src, dest_path)?;
    drop(src);
    let copy = Connection::open_with_flags(
        dest_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let integrity = integrity_check(&copy)?;
    if !(integrity.len() == 1 && integrity[0] == "ok") {
        return Err(StorageError::Corrupt(
            "restored copy failed integrity_check".to_string(),
        ));
    }
    Ok(dest_path.to_path_buf())
}

/// Restore the manifest's snapshot into `scratch_dir/<backup_id>.restore-test.sqlite`,
/// verify it and advance Checked → RestoreTested (or → Failed). The restored copy
/// is left for inspection; the caller removes it.
pub fn restore_test(
    manifest: &mut BackupManifest,
    scratch_dir: &Path,
    options: &VerifyOptions,
    now: UtcMillis,
) -> StorageResult<PathBuf> {
    let path = manifest
        .path
        .clone()
        .ok_or_else(|| StorageError::Precondition("manifest has no snapshot path".to_string()))?;
    let dest = scratch_dir.join(format!(
        "{}.restore-test.sqlite",
        manifest.backup_id.as_str()
    ));
    let restored = match restore_to_copy(&path, &dest) {
        Ok(p) => p,
        Err(e) => {
            manifest.fail(e.code())?;
            return Err(e);
        }
    };
    let report = verify_snapshot(&restored, options)?;
    if report.ok() {
        manifest.restore_tested_at = Some(now);
        manifest.transition(BackupEvent::TestRestore)?;
        Ok(restored)
    } else {
        manifest.fail("restore test verification failed")?;
        Err(StorageError::Verification(report.errors))
    }
}

/// `VACUUM INTO dest` — a compact, consistent export of the database.
pub fn vacuum_into(conn: &Connection, dest: &Path) -> StorageResult<PathBuf> {
    if dest.exists() {
        return Err(StorageError::Conflict(
            "vacuum destination already exists".to_string(),
        ));
    }
    conn.execute("VACUUM INTO ?1", params![path_str(dest)?])?;
    Ok(dest.to_path_buf())
}

/// Insert or update the `backup_manifests` row for a manifest.
pub fn record_manifest(
    conn: &Connection,
    manifest: &BackupManifest,
    now: UtcMillis,
) -> StorageResult<()> {
    let state = enum_to_text(&manifest.state)?;
    let verification_json = match &manifest.verification {
        Some(v) => Some(
            serde_json::to_string(v).map_err(|_| StorageError::Internal("verification json"))?,
        ),
        None => None,
    };
    let path = match &manifest.path {
        Some(p) => Some(path_str(p)?.to_string()),
        None => None,
    };
    let existing: Option<i64> = conn
        .query_row(
            "SELECT revision FROM backup_manifests WHERE id = ?1",
            params![manifest.backup_id.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        None => {
            conn.execute(
                "INSERT INTO backup_manifests (id, created_at, app_version, schema_version, page_count, sha256_of_file, state,
                    path, audit_anchor_seq, audit_anchor_hash, verification_json, restore_tested_at, failure_reason, revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1, ?14)",
                params![
                    manifest.backup_id.as_str(),
                    manifest.created_at.as_millis(),
                    manifest.app_version,
                    i64::from(manifest.schema_version),
                    manifest.page_count as i64,
                    manifest.sha256_of_file,
                    state,
                    path,
                    manifest.audit_anchor.as_ref().map(|a| a.seq),
                    manifest.audit_anchor.as_ref().map(|a| a.row_hash.clone()),
                    verification_json,
                    manifest.restore_tested_at.map(UtcMillis::as_millis),
                    manifest.failure_reason,
                    now.as_millis(),
                ],
            )?;
        }
        Some(rev) => {
            conn.execute(
                "UPDATE backup_manifests SET schema_version = ?2, page_count = ?3, sha256_of_file = ?4, state = ?5, path = ?6,
                    audit_anchor_seq = ?7, audit_anchor_hash = ?8, verification_json = ?9, restore_tested_at = ?10,
                    failure_reason = ?11, revision = ?12, updated_at = ?13
                 WHERE id = ?1 AND revision = ?14",
                params![
                    manifest.backup_id.as_str(),
                    i64::from(manifest.schema_version),
                    manifest.page_count as i64,
                    manifest.sha256_of_file,
                    state,
                    path,
                    manifest.audit_anchor.as_ref().map(|a| a.seq),
                    manifest.audit_anchor.as_ref().map(|a| a.row_hash.clone()),
                    verification_json,
                    manifest.restore_tested_at.map(UtcMillis::as_millis),
                    manifest.failure_reason,
                    rev + 1,
                    now.as_millis(),
                    rev,
                ],
            )?;
        }
    }
    Ok(())
}

/// Read a manifest's stored state (for the Settings/Recovery screen).
pub fn manifest_state(
    conn: &Connection,
    backup_id: &BackupId,
) -> StorageResult<Option<BackupState>> {
    let state: Option<String> = conn
        .query_row(
            "SELECT state FROM backup_manifests WHERE id = ?1",
            params![backup_id.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    match state {
        None => Ok(None),
        Some(s) => Ok(Some(crate::sql::enum_from_text(&s)?)),
    }
}
