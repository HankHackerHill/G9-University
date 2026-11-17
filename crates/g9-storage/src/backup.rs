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
//!   attachment check: the file of every referenced attachment is hashed at its
//!   content address under the root; a missing or damaged file fails
//!   verification and is reported by code and count (STOR-2).
//! * [`restore_to_copy`] always restores into a *new* file; it never overwrites
//!   the live database. Swapping the live file is an application decision that
//!   needs an operator approval (runbook § Restore).
//! * [`BackupManifest`] drives the `BackupState` machine
//!   Requested → SnapshotCreated → Checked → RestoreTested → Retained; failures
//!   are explicit (`Failed` with a reason).
//! * [`read_manifest`] and [`list_manifests`] read `backup_manifests` back into
//!   [`StoredManifest`] values (manifest plus stored revision), so a backup
//!   created by one process can be verified, restore-tested or restored by the
//!   next. Stored values that cannot be decoded (unknown state text, invalid
//!   verification JSON, inconsistent audit anchor) are errors, never defaults.
//! * [`vacuum_into`] is the alternative for compact exports.

use crate::audit::AuditChain;
use crate::error::{StorageError, StorageResult};
use crate::migrations::{foreign_key_check, integrity_check, table_counts, Migrator};
use crate::sql::enum_to_text;
use g9_domain::state::BackupEvent;
use g9_domain::{BackupId, BackupState, Revision, UtcMillis};
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
    /// Every non-erased attachment row's file was looked up at the content
    /// address of its `content_sha256` under the root and hashed (STOR-2).
    Checked {
        /// Number of attachment rows examined.
        expected: u64,
        /// Storage refs whose file was missing or unreadable.
        missing: Vec<String>,
        /// Number of distinct files present under the root that are not a copy
        /// a restore can use: bytes that do not hash to the content address, or
        /// not a regular file inside the store. A count only: the report is
        /// stored and shown, and names no file (0 in reports stored before
        /// this check existed).
        #[serde(default)]
        damaged: u64,
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
    /// The schema (tables, indexes, triggers, views) equals what the embedded
    /// migrations produce at the recorded version (`migrations::verify_schema`).
    /// `None` only in reports stored before this check existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ok: Option<bool>,
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
            && self.schema_ok != Some(false)
            && self.errors.is_empty()
            && !matches!(&self.attachment_manifest, AttachmentCheck::Checked { missing, damaged, .. } if !missing.is_empty() || *damaged > 0)
    }
}

/// Options for [`verify_snapshot`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyOptions {
    /// Root directory of the content-addressed attachment store a restore would
    /// copy the files from (the application passes `<backups>/attachments`);
    /// when set, the file of every non-erased attachment must be there, at the
    /// content address of its `content_sha256`, and hash to it.
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

/// Create `path` as a new, empty file that only its owner can read or write.
/// Fails with `AlreadyExists` when something is already there.
pub fn create_new_private(path: &Path) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map(|_| ())
}

/// Copy every page of `src` into a brand-new database file at `dest` with the
/// Online Backup API, then switch the copy to rollback-journal mode.
fn copy_with_backup_api(src: &Connection, dest: &Path) -> StorageResult<u64> {
    // The copy holds everything the database holds: it is created here, empty and
    // readable by its owner only, before SQLite opens it (SQLite would create it
    // with the process umask, 0644 on most hosts; security audit 2026-11-13).
    // SQLite gives a journal the mode of its database. `create_new` is also the
    // check that a backup never overwrites, without a gap between check and use.
    create_new_private(dest).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            StorageError::Conflict(
                "destination already exists; backups never overwrite".to_string(),
            )
        } else {
            StorageError::io("create the snapshot file", &e)
        }
    })?;
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

/// Free space a backup needs before it starts, as a multiple of the database
/// size ([`database_size_bytes`]) on the filesystem that holds the backup
/// directory (runbook "Disk-full and power-fail": "backups refuse to start
/// below a free-space threshold (proposed 2× database size)"; STOR-4).
///
/// PROPOSED default, the owner's to change. The attachment files copied with a
/// backup are not counted (PROPOSED extension: add the bytes not yet in
/// `<backups>/attachments`).
pub const BACKUP_FREE_SPACE_FACTOR: u64 = 2;

/// Bytes the database occupies: `page_count × page_size`, as the connection
/// sees it (committed frames still in the WAL included). A snapshot copies
/// every page, so this is the size of the snapshot it would write.
pub fn database_size_bytes(conn: &Connection) -> StorageResult<u64> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    Ok(u64::try_from(pages)
        .unwrap_or(0)
        .saturating_mul(u64::try_from(page_size).unwrap_or(0)))
}

/// Where the free space of the backup directory's filesystem is read from.
/// Injectable so that a refusal is testable without filling a disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", content = "bytes", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FreeSpaceSource {
    /// `statvfs(2)` on the directory: blocks available to an unprivileged
    /// writer × fragment size (unix). On other platforms the free space is
    /// unknown and the check is skipped, which the caller reports.
    #[default]
    Filesystem,
    /// A fixed reading in bytes (tests and drills).
    Fixed(u64),
}

impl FreeSpaceSource {
    /// Bytes available on the filesystem that holds `dir`; `None` when this
    /// platform cannot tell. A directory that cannot be inspected is an I/O
    /// error (only its kind is kept).
    pub fn available_bytes(self, dir: &Path) -> StorageResult<Option<u64>> {
        match self {
            FreeSpaceSource::Fixed(bytes) => Ok(Some(bytes)),
            FreeSpaceSource::Filesystem => filesystem_available_bytes(dir),
        }
    }
}

#[cfg(unix)]
fn filesystem_available_bytes(dir: &Path) -> StorageResult<Option<u64>> {
    let stat = rustix::fs::statvfs(dir).map_err(|e| {
        StorageError::io(
            "read the free space of the backup directory",
            &std::io::Error::from(e),
        )
    })?;
    Ok(Some(stat.f_bavail.saturating_mul(stat.f_frsize)))
}

/// No safe free-space call is wired for this platform: the reading is unknown.
#[cfg(not(unix))]
fn filesystem_available_bytes(_dir: &Path) -> StorageResult<Option<u64>> {
    Ok(None)
}

/// The free-space reading a backup was allowed to start with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeSpaceCheck {
    /// [`BACKUP_FREE_SPACE_FACTOR`] × [`database_size_bytes`].
    pub required_bytes: u64,
    /// The reading; `None` when the platform cannot tell (not checked).
    pub available_bytes: Option<u64>,
}

/// Refuse to start a backup of `conn` into `dir` when fewer than
/// [`BACKUP_FREE_SPACE_FACTOR`] × the database size are available there
/// ([`StorageError::InsufficientSpace`], `RETRYABLE_STORAGE
/// backup.insufficient_space`: the category of `SQLITE_FULL`, retried once
/// space is freed). Nothing is written. An unknown reading is not a refusal:
/// the returned check says it was not made.
pub fn check_free_space(
    conn: &Connection,
    dir: &Path,
    source: FreeSpaceSource,
) -> StorageResult<FreeSpaceCheck> {
    let required_bytes = database_size_bytes(conn)?.saturating_mul(BACKUP_FREE_SPACE_FACTOR);
    let available_bytes = source.available_bytes(dir)?;
    if let Some(available) = available_bytes {
        if available < required_bytes {
            return Err(StorageError::InsufficientSpace {
                required_bytes,
                available_bytes: available,
            });
        }
    }
    Ok(FreeSpaceCheck {
        required_bytes,
        available_bytes,
    })
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
    // Durable before it is visible, and visible durably (TEC-11): the file's
    // data is synced before the rename (the copy connection syncs on commit;
    // this does not rely on its settings), and the directory after it, so a
    // power cut cannot leave the snapshot without its final name.
    sync_file(&partial)?;
    std::fs::rename(&partial, dest_path).map_err(|e| StorageError::io("rename snapshot", &e))?;
    if let Some(dir) = dest_path.parent() {
        sync_dir(dir)?;
    }

    manifest.schema_version = schema_version;
    manifest.page_count = page_count;
    manifest.sha256_of_file = sha;
    manifest.path = Some(dest_path.to_path_buf());
    manifest.audit_anchor = anchor;
    manifest.transition(BackupEvent::Snapshot)?;
    Ok(manifest)
}

/// Flush a file's data and metadata to stable storage.
pub fn sync_file(path: &Path) -> StorageResult<()> {
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| StorageError::io("sync a backup file", &e))
}

/// Flush a directory's entries (a rename into it) to stable storage. On
/// non-unix systems there is no portable equivalent and this does nothing.
pub fn sync_dir(dir: &Path) -> StorageResult<()> {
    #[cfg(unix)]
    std::fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| StorageError::io("sync a backup directory", &e))?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
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
    let schema_ok = match crate::migrations::verify_schema(conn) {
        Ok(check) if check.ok() => true,
        Ok(check) => {
            errors.push(format!(
                "schema differs from the migrations at version {}: {}",
                check.version,
                check.differences.join("; ")
            ));
            false
        }
        Err(e) => {
            errors.push(format!("schema unreadable: {}", e.code()));
            false
        }
    };
    let counts = table_counts(conn)?;
    let attachment_manifest = match &options.attachment_root {
        None => AttachmentCheck::NotChecked,
        Some(root) => check_attachments(conn, root)?,
    };
    if let AttachmentCheck::Checked {
        missing, damaged, ..
    } = &attachment_manifest
    {
        // Code and count only: a storage ref is derived from the content hash,
        // and neither belongs in an error.
        let missing_files = missing
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if missing_files > 0 {
            errors.push(format!(
                "backup.attachments_missing: {missing_files} attachment file(s) the snapshot references are not in the attachment store or cannot be read"
            ));
        }
        if *damaged > 0 {
            errors.push(format!(
                "backup.attachments_damaged: {damaged} attachment file(s) in the attachment store do not match their content hash"
            ));
        }
    }
    Ok(VerificationReport {
        integrity_check_ok,
        foreign_key_check_ok,
        migration_checksums_ok,
        audit_chain_ok,
        schema_ok: Some(schema_ok),
        table_counts: counts,
        attachment_manifest,
        errors,
    })
}

/// Look up and hash the file of every non-erased attachment row under `root`.
///
/// The file is found by its digest, never by the stored `storage_ref`: a restore
/// (`g9 restore`, `g9 copy-backup`) copies by digest out of a content-addressed
/// store, so the content address is the file a restore would use. Each distinct
/// digest is hashed once. A digest that is not lowercase hex cannot be resolved
/// and counts as missing. A schema older than migration `0005` has no
/// `attachments` table and references no file.
fn check_attachments(conn: &Connection, root: &Path) -> StorageResult<AttachmentCheck> {
    use crate::attachment_store::{has_attachment_table, inspect, FileState};
    if !has_attachment_table(conn)? {
        return Ok(AttachmentCheck::Checked {
            expected: 0,
            missing: Vec::new(),
            damaged: 0,
        });
    }
    let mut stmt = conn.prepare(
        "SELECT storage_ref, content_sha256 FROM attachments WHERE erased = 0 ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut expected = 0_u64;
    let mut missing = Vec::new();
    let mut states: BTreeMap<String, FileState> = BTreeMap::new();
    for row in rows {
        let (storage_ref, sha256) = row?;
        expected += 1;
        let state = *states
            .entry(sha256)
            .or_insert_with_key(|sha256| inspect(root, sha256));
        if state == FileState::Missing {
            missing.push(storage_ref);
        }
    }
    let damaged = states
        .values()
        .filter(|state| **state == FileState::Damaged)
        .count() as u64;
    Ok(AttachmentCheck::Checked {
        expected,
        missing,
        damaged,
    })
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

/// The snapshot file must still be the bytes this manifest recorded.
fn manifest_hash_errors(manifest: &BackupManifest, path: &Path, when: &str) -> Vec<String> {
    match sha256_of_file(path) {
        Ok(sha) if sha == manifest.sha256_of_file => Vec::new(),
        Ok(_) => vec![format!(
            "snapshot file hash differs from the manifest {when}"
        )],
        Err(e) => vec![format!("snapshot file unreadable {when}: {}", e.code())],
    }
}

/// Restore the manifest's snapshot into `scratch_dir/<backup_id>.restore-test.sqlite`,
/// verify it and advance Checked → RestoreTested (or → Failed). On success the
/// restored copy is returned and the caller removes it; on failure it is
/// removed here. A copy left by an interrupted restore test is our own scratch
/// file and is replaced.
///
/// Only a failure that says something about the backup's content fails it: a
/// corrupt copy, or a restored copy that does not verify. An environmental
/// failure (scratch directory missing or unwritable, disk full, I/O, a busy
/// file) is returned with the manifest left `Checked`, so a good backup stays
/// usable (storage-core-2).
///
/// The bytes restored must be the bytes verified under this manifest (ADR-0001,
/// threat ST10): the snapshot file is re-hashed against
/// `manifest.sha256_of_file` before the copy (a replaced file is refused without
/// restoring it) and again after it (a swap during the copy), and the restored
/// copy's audit chain head must equal `manifest.audit_anchor`.
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
    let before = manifest_hash_errors(manifest, &path, "before the restore");
    if !before.is_empty() {
        manifest.fail("restore test verification failed")?;
        return Err(StorageError::Verification(before));
    }
    let dest = scratch_dir.join(format!(
        "{}.restore-test.sqlite",
        manifest.backup_id.as_str()
    ));
    crate::migrations::remove_scratch_copy(&dest)?;
    let restored = match restore_to_copy(&path, &dest) {
        Ok(p) => p,
        Err(e) => {
            if matches!(e, StorageError::Corrupt(_)) {
                manifest.fail(e.code())?;
            }
            discard_scratch_copy(&dest);
            return Err(e);
        }
    };
    let checked = (|| {
        let mut errors = manifest_hash_errors(manifest, &path, "after the restore");
        let report = verify_snapshot(&restored, options)?;
        errors.extend(report.errors.iter().cloned());
        let restored_head = {
            let copy = Connection::open_with_flags(
                &restored,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            AuditChain::head(&copy)?.map(|(seq, row_hash)| AuditAnchor { seq, row_hash })
        };
        if restored_head != manifest.audit_anchor {
            errors.push("restored audit chain head differs from the manifest anchor".to_string());
        }
        Ok::<_, StorageError>((report.ok(), errors))
    })();
    match checked {
        Ok((true, errors)) if errors.is_empty() => {
            manifest.restore_tested_at = Some(now);
            manifest.transition(BackupEvent::TestRestore)?;
            Ok(restored)
        }
        Ok((_, errors)) => {
            manifest.fail("restore test verification failed")?;
            discard_scratch_copy(&restored);
            Err(StorageError::Verification(errors))
        }
        Err(e) => {
            discard_scratch_copy(&restored);
            Err(e)
        }
    }
}

/// Remove a scratch copy; a failure is logged, never fatal.
fn discard_scratch_copy(copy: &Path) {
    if let Err(e) = crate::migrations::remove_scratch_copy(copy) {
        tracing::warn!(code = e.code(), "scratch copy was not removed");
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

/// A `backup_manifests` row read back from the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredManifest {
    /// The reconstructed manifest.
    pub manifest: BackupManifest,
    /// `backup_manifests.revision` as maintained by [`record_manifest`].
    pub revision: Revision,
}

const MANIFEST_COLUMNS: &str = "id, created_at, app_version, schema_version, page_count, sha256_of_file, state, \
     path, audit_anchor_seq, audit_anchor_hash, verification_json, restore_tested_at, failure_reason, revision";

/// Undecoded `backup_manifests` columns (decoding can fail with a storage error,
/// which a `rusqlite` row mapper cannot return).
struct RawManifestRow {
    id: String,
    created_at: i64,
    app_version: String,
    schema_version: i64,
    page_count: i64,
    sha256_of_file: String,
    state: String,
    path: Option<String>,
    audit_anchor_seq: Option<i64>,
    audit_anchor_hash: Option<String>,
    verification_json: Option<String>,
    restore_tested_at: Option<i64>,
    failure_reason: Option<String>,
    revision: i64,
}

fn raw_manifest_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawManifestRow> {
    Ok(RawManifestRow {
        id: row.get(0)?,
        created_at: row.get(1)?,
        app_version: row.get(2)?,
        schema_version: row.get(3)?,
        page_count: row.get(4)?,
        sha256_of_file: row.get(5)?,
        state: row.get(6)?,
        path: row.get(7)?,
        audit_anchor_seq: row.get(8)?,
        audit_anchor_hash: row.get(9)?,
        verification_json: row.get(10)?,
        restore_tested_at: row.get(11)?,
        failure_reason: row.get(12)?,
        revision: row.get(13)?,
    })
}

fn decode_manifest(raw: RawManifestRow) -> StorageResult<StoredManifest> {
    let backup_id = BackupId::parse(&raw.id)
        .map_err(|e| StorageError::RowDecode(format!("invalid backup id in row: {e}")))?;
    let state: BackupState = crate::sql::enum_from_text(&raw.state)?;
    let schema_version = u32::try_from(raw.schema_version)
        .map_err(|_| StorageError::RowDecode("schema_version out of range".to_string()))?;
    let page_count = u64::try_from(raw.page_count)
        .map_err(|_| StorageError::RowDecode("page_count out of range".to_string()))?;
    let revision = u64::try_from(raw.revision)
        .ok()
        .filter(|r| *r >= 1)
        .map(Revision::new)
        .ok_or_else(|| StorageError::RowDecode("revision out of range".to_string()))?;
    if raw.sha256_of_file.len() != 64 || !raw.sha256_of_file.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(StorageError::RowDecode(
            "sha256_of_file is not a 64-char hex digest".to_string(),
        ));
    }
    let audit_anchor = match (raw.audit_anchor_seq, raw.audit_anchor_hash) {
        (Some(seq), Some(row_hash)) => Some(AuditAnchor { seq, row_hash }),
        (None, None) => None,
        _ => {
            return Err(StorageError::RowDecode(
                "audit anchor columns are inconsistent".to_string(),
            ))
        }
    };
    let verification = match raw.verification_json {
        Some(text) => Some(
            serde_json::from_str::<VerificationReport>(&text).map_err(|_| {
                StorageError::RowDecode(
                    "verification_json is not a verification report".to_string(),
                )
            })?,
        ),
        None => None,
    };
    Ok(StoredManifest {
        manifest: BackupManifest {
            backup_id,
            created_at: UtcMillis::from_millis(raw.created_at),
            app_version: raw.app_version,
            schema_version,
            page_count,
            sha256_of_file: raw.sha256_of_file,
            state,
            path: raw.path.map(PathBuf::from),
            audit_anchor,
            verification,
            restore_tested_at: crate::sql::opt_millis(raw.restore_tested_at),
            failure_reason: raw.failure_reason,
        },
        revision,
    })
}

/// Read one manifest with its stored revision; `None` when the id is unknown.
///
/// A row whose values cannot be decoded (unknown `state` text, a
/// `verification_json` that is not a [`VerificationReport`], an inconsistent
/// audit anchor, out-of-range numbers) is an error (`storage.row_decode`),
/// never silently defaulted.
pub fn read_manifest(
    conn: &Connection,
    backup_id: &BackupId,
) -> StorageResult<Option<StoredManifest>> {
    let raw = conn
        .query_row(
            &format!("SELECT {MANIFEST_COLUMNS} FROM backup_manifests WHERE id = ?1"),
            params![backup_id.as_str()],
            raw_manifest_row,
        )
        .optional()?;
    raw.map(decode_manifest).transpose()
}

/// Every manifest, newest first (`created_at` descending, then `id` descending).
///
/// One undecodable row fails the whole listing (see [`read_manifest`]).
pub fn list_manifests(conn: &Connection) -> StorageResult<Vec<StoredManifest>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {MANIFEST_COLUMNS} FROM backup_manifests ORDER BY created_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], raw_manifest_row)?;
    let mut manifests = Vec::new();
    for row in rows {
        manifests.push(decode_manifest(row?)?);
    }
    Ok(manifests)
}

/// Open `db_path` read-only (no creation, no migration, no PRAGMA changes) and
/// read one manifest. For tooling that must inspect a database without
/// becoming a writer, such as the offline `g9 restore` planner. When no
/// application holds the file (no `-wal`, `-shm` or `-journal` next to it) the
/// directory is left exactly as it was found: see [`open_for_inspection`].
pub fn read_manifest_from_file(
    db_path: &Path,
    backup_id: &BackupId,
) -> StorageResult<Option<StoredManifest>> {
    let conn = open_for_inspection(db_path)?;
    conn.busy_timeout(Duration::from_millis(1000))?;
    conn.pragma_update(None, "trusted_schema", false)?;
    read_manifest(&conn, backup_id)
}

/// A read-only connection that leaves no trace in the directory.
///
/// A database in WAL mode that no connection holds has no sidecars, and a plain
/// read-only open creates an empty `-wal` and a `-shm` next to it. When none of
/// `-wal`, `-shm` and `-journal` exists, nothing holds the file and nothing is
/// pending in a log, so it is opened `immutable=1` (no locks, no sidecars). When
/// any of them exists, committed frames may be waiting in the log, which
/// `immutable` would not see, so the ordinary read-only open is used.
fn open_for_inspection(db_path: &Path) -> StorageResult<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    #[cfg(unix)]
    {
        let sidecar_present = ["-wal", "-shm", "-journal"].iter().any(|suffix| {
            let mut name = db_path.as_os_str().to_owned();
            name.push(suffix);
            std::fs::symlink_metadata(PathBuf::from(name)).is_ok()
        });
        if !sidecar_present {
            if let Some(text) = db_path.to_str() {
                let mut uri = String::from("file:");
                for c in text.chars() {
                    match c {
                        '%' => uri.push_str("%25"),
                        '?' => uri.push_str("%3f"),
                        '#' => uri.push_str("%23"),
                        other => uri.push(other),
                    }
                }
                uri.push_str("?immutable=1");
                return Ok(Connection::open_with_flags(
                    uri,
                    flags | OpenFlags::SQLITE_OPEN_URI,
                )?);
            }
        }
    }
    Ok(Connection::open_with_flags(db_path, flags)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{Database, OpenOptions};

    fn db() -> Database {
        Database::open_in_memory(OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)))
            .unwrap()
    }

    fn report(missing: Vec<String>) -> VerificationReport {
        VerificationReport {
            integrity_check_ok: true,
            foreign_key_check_ok: true,
            migration_checksums_ok: true,
            audit_chain_ok: true,
            schema_ok: Some(true),
            table_counts: BTreeMap::from([
                ("notes".to_string(), 3),
                ("backup_manifests".into(), 1),
            ]),
            attachment_manifest: AttachmentCheck::Checked {
                expected: 2,
                missing,
                damaged: 0,
            },
            errors: Vec::new(),
        }
    }

    fn manifest(id: &str, created_at: i64, state: BackupState) -> BackupManifest {
        BackupManifest {
            backup_id: BackupId::parse(id).unwrap(),
            created_at: UtcMillis::from_millis(created_at),
            app_version: "0.1.0-test".to_string(),
            schema_version: 13,
            page_count: 42,
            sha256_of_file: "ab".repeat(32),
            state,
            path: Some(PathBuf::from(format!("/data/backups/{id}.sqlite"))),
            audit_anchor: Some(AuditAnchor {
                seq: 7,
                row_hash: "cd".repeat(32),
            }),
            verification: Some(report(vec!["missing/ref".to_string()])),
            restore_tested_at: Some(UtcMillis::from_millis(created_at + 5)),
            failure_reason: Some("reason.code".to_string()),
        }
    }

    #[test]
    fn every_state_round_trips_through_record_and_read() {
        let db = db();
        for (i, state) in BackupState::ALL.iter().enumerate() {
            let id = format!("bk-{i}");
            let m = manifest(&id, 1_000 + i as i64, *state);
            record_manifest(db.conn(), &m, UtcMillis::from_millis(2_000)).unwrap();
            let stored = read_manifest(db.conn(), &m.backup_id).unwrap().unwrap();
            assert_eq!(stored.manifest, m, "{state:?}");
            assert_eq!(stored.revision, Revision::INITIAL, "{state:?}");
            // `manifest_state` agrees with the reader.
            assert_eq!(
                manifest_state(db.conn(), &m.backup_id).unwrap(),
                Some(*state)
            );
        }
        assert_eq!(
            list_manifests(db.conn()).unwrap().len(),
            BackupState::ALL.len()
        );
    }

    #[test]
    fn optional_fields_round_trip_as_none_and_not_checked() {
        let db = db();
        let request = SnapshotRequest {
            backup_id: BackupId::parse("bk-min").unwrap(),
            requested_at: UtcMillis::from_millis(10),
            app_version: "0.1.0-test".to_string(),
        };
        let mut m = BackupManifest::requested(&request);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(11)).unwrap();
        let stored = read_manifest(db.conn(), &m.backup_id).unwrap().unwrap();
        assert_eq!(stored.manifest, m);
        assert_eq!(stored.manifest.path, None);
        assert_eq!(stored.manifest.audit_anchor, None);
        assert_eq!(stored.manifest.verification, None);

        let mut r = report(Vec::new());
        r.attachment_manifest = AttachmentCheck::NotChecked;
        m.verification = Some(r);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(12)).unwrap();
        let stored = read_manifest(db.conn(), &m.backup_id).unwrap().unwrap();
        assert_eq!(stored.manifest, m);
    }

    #[test]
    fn stored_revision_follows_record_manifest_updates() {
        let db = db();
        let mut m = manifest("bk-rev", 100, BackupState::SnapshotCreated);
        m.failure_reason = None;
        record_manifest(db.conn(), &m, UtcMillis::from_millis(101)).unwrap();
        m.state = BackupState::Checked;
        record_manifest(db.conn(), &m, UtcMillis::from_millis(102)).unwrap();
        m.state = BackupState::Retained;
        record_manifest(db.conn(), &m, UtcMillis::from_millis(103)).unwrap();
        let stored = read_manifest(db.conn(), &m.backup_id).unwrap().unwrap();
        assert_eq!(stored.revision, Revision::new(3));
        assert_eq!(stored.manifest.state, BackupState::Retained);
    }

    #[test]
    fn unknown_id_reads_as_none() {
        let db = db();
        let id = BackupId::parse("bk-unknown").unwrap();
        assert_eq!(read_manifest(db.conn(), &id).unwrap(), None);
        assert_eq!(list_manifests(db.conn()).unwrap(), Vec::new());
    }

    #[test]
    fn list_is_newest_first_with_id_as_tie_break() {
        let db = db();
        for (id, created) in [("bk-a", 10), ("bk-c", 30), ("bk-b", 20), ("bk-d", 30)] {
            let m = manifest(id, created, BackupState::Retained);
            record_manifest(db.conn(), &m, UtcMillis::from_millis(99)).unwrap();
        }
        let ids: Vec<String> = list_manifests(db.conn())
            .unwrap()
            .iter()
            .map(|s| s.manifest.backup_id.as_str().to_string())
            .collect();
        assert_eq!(ids, ["bk-d", "bk-c", "bk-b", "bk-a"]);
    }

    /// Run `sql` with CHECK constraints off (the only way to plant a corrupt value).
    fn corrupt(db: &Database, sql: &str) {
        db.conn()
            .execute_batch(&format!(
                "PRAGMA ignore_check_constraints = ON; {sql}; PRAGMA ignore_check_constraints = OFF;"
            ))
            .unwrap();
    }

    #[test]
    fn corrupt_state_text_is_an_error_not_a_default() {
        let db = db();
        let m = manifest("bk-bad-state", 5, BackupState::Retained);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        corrupt(
            &db,
            "UPDATE backup_manifests SET state = 'RETAINED_MAYBE' WHERE id = 'bk-bad-state'",
        );
        let err = read_manifest(db.conn(), &m.backup_id).unwrap_err();
        assert!(matches!(err, StorageError::RowDecode(_)), "{err:?}");
        assert_eq!(err.code(), "storage.row_decode");
        assert!(matches!(
            list_manifests(db.conn()).unwrap_err(),
            StorageError::RowDecode(_)
        ));
    }

    #[test]
    fn invalid_verification_json_is_an_error() {
        let db = db();
        let m = manifest("bk-bad-json", 5, BackupState::Checked);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        // Valid JSON that is not a verification report passes the CHECK.
        db.conn()
            .execute(
                "UPDATE backup_manifests SET verification_json = '{\"integrity_check_ok\": 1}' WHERE id = 'bk-bad-json'",
                [],
            )
            .unwrap();
        assert!(matches!(
            read_manifest(db.conn(), &m.backup_id).unwrap_err(),
            StorageError::RowDecode(_)
        ));
        // Malformed JSON (only possible with the CHECK bypassed).
        corrupt(
            &db,
            "UPDATE backup_manifests SET verification_json = '{not json' WHERE id = 'bk-bad-json'",
        );
        assert!(matches!(
            read_manifest(db.conn(), &m.backup_id).unwrap_err(),
            StorageError::RowDecode(_)
        ));
    }

    #[test]
    fn inconsistent_audit_anchor_and_bad_numbers_are_errors() {
        let db = db();
        let m = manifest("bk-bad-anchor", 5, BackupState::Retained);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        corrupt(
            &db,
            "UPDATE backup_manifests SET audit_anchor_hash = NULL WHERE id = 'bk-bad-anchor'",
        );
        assert!(matches!(
            read_manifest(db.conn(), &m.backup_id).unwrap_err(),
            StorageError::RowDecode(_)
        ));
        let m2 = manifest("bk-bad-sha", 6, BackupState::Retained);
        record_manifest(db.conn(), &m2, UtcMillis::from_millis(7)).unwrap();
        corrupt(
            &db,
            "UPDATE backup_manifests SET sha256_of_file = 'zz' || substr(sha256_of_file, 3) WHERE id = 'bk-bad-sha'",
        );
        assert!(matches!(
            read_manifest(db.conn(), &m2.backup_id).unwrap_err(),
            StorageError::RowDecode(_)
        ));
    }

    #[test]
    fn read_manifest_from_file_opens_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.sqlite");
        let db = Database::open(
            &path,
            OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)),
        )
        .unwrap();
        let m = manifest("bk-file", 5, BackupState::Retained);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        let stored = read_manifest_from_file(&path, &m.backup_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.manifest, m);
        assert_eq!(
            read_manifest_from_file(&path, &BackupId::parse("bk-none").unwrap()).unwrap(),
            None
        );
        // A path that does not exist is an error, never created.
        let missing = dir.path().join("absent.sqlite");
        assert!(read_manifest_from_file(&missing, &m.backup_id).is_err());
        assert!(!missing.exists());
    }

    fn directory_listing(dir: &Path) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().unwrap().len(),
                )
            })
            .collect();
        entries.sort();
        entries
    }

    #[test]
    fn reading_a_closed_wal_database_leaves_the_directory_as_it_was() {
        // The application's database is in WAL mode; once its last connection is
        // closed there is no `-wal` or `-shm`. A plain read-only open would create
        // both, which made `g9 restore` without confirmation change the directory.
        // The path has characters that a URI would otherwise take as syntax.
        let dir = tempfile::tempdir().unwrap();
        let odd = dir.path().join("a%20b?c#d");
        std::fs::create_dir(&odd).unwrap();
        let path = odd.join("live.sqlite");
        let m = manifest("bk-quiet", 5, BackupState::Retained);
        {
            let db = Database::open(
                &path,
                OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)),
            )
            .unwrap();
            assert_eq!(db.profile().journal_mode, "wal");
            record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        }
        let before = directory_listing(&odd);
        assert_eq!(before.len(), 1, "{before:?}");
        let stored = read_manifest_from_file(&path, &m.backup_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.manifest, m);
        assert_eq!(directory_listing(&odd), before);
    }

    #[test]
    fn reading_a_database_an_application_holds_still_sees_committed_frames() {
        // An open WAL connection (sidecars present) with a committed manifest that
        // sits in the log: the ordinary read-only open reads through the log.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.sqlite");
        let db = Database::open(
            &path,
            OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)),
        )
        .unwrap();
        db.conn()
            .execute_batch("PRAGMA wal_autocheckpoint = 0")
            .unwrap();
        let m = manifest("bk-in-wal", 5, BackupState::Retained);
        record_manifest(db.conn(), &m, UtcMillis::from_millis(6)).unwrap();
        assert!(std::fs::metadata(dir.path().join("live.sqlite-wal"))
            .map(|meta| meta.len() > 0)
            .unwrap_or(false));
        let stored = read_manifest_from_file(&path, &m.backup_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.manifest, m);
    }

    #[test]
    fn a_backup_needs_twice_the_database_size_free() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let size = database_size_bytes(db.conn()).unwrap();
        assert!(size > 0);
        let required = size * BACKUP_FREE_SPACE_FACTOR;
        let err = check_free_space(db.conn(), dir.path(), FreeSpaceSource::Fixed(required - 1))
            .unwrap_err();
        assert_eq!(
            err,
            StorageError::InsufficientSpace {
                required_bytes: required,
                available_bytes: required - 1
            }
        );
        assert_eq!(err.code(), "backup.insufficient_space");
        assert_eq!(
            err.category(),
            g9_domain::ErrorCategory::RetryableStorage,
            "the category of SQLITE_FULL"
        );
        let g: g9_domain::G9Error = err.into();
        assert_eq!(g.details.unwrap()["required_bytes"], required);
        let ok = check_free_space(db.conn(), dir.path(), FreeSpaceSource::Fixed(required)).unwrap();
        assert_eq!(
            ok,
            FreeSpaceCheck {
                required_bytes: required,
                available_bytes: Some(required)
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_filesystem_reading_comes_from_statvfs() {
        let dir = tempfile::tempdir().unwrap();
        let available = FreeSpaceSource::Filesystem
            .available_bytes(dir.path())
            .unwrap();
        assert!(available.is_some_and(|b| b > 0), "{available:?}");
        // A directory that does not exist cannot be read: an I/O error, kind only.
        let err = FreeSpaceSource::Filesystem
            .available_bytes(&dir.path().join("absent"))
            .unwrap_err();
        assert_eq!(err.code(), "storage.io");
        assert!(!err.to_string().contains("absent"), "{err}");
    }
}
