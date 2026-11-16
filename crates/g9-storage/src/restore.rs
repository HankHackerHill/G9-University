//! Offline restore of a verified snapshot over the live database file
//! (ADR-0001; runbook `docs/runbooks/backup-restore.md` § Restore; gate A08).
//!
//! [`restore_over_live`] replaces the live database file with a snapshot. It is
//! the only place that swaps the live file; the live file is never copied by
//! hand (ADR-0001) and nothing is ever deleted: the replaced database and its
//! sidecars are moved into a quarantine directory.
//!
//! Steps, in order; every step before the swap fails closed and leaves the live
//! database untouched (the best-effort WAL checkpoint of step 5 only merges
//! committed WAL frames into the main file; it changes no content):
//!
//! 1. Both paths are resolved (`canonicalize`; the live file may be absent, its
//!    directory may not). The snapshot must be a regular, self-contained file
//!    (no write-ahead log or journal next to it) and must not be the live file.
//! 2. The snapshot is hashed; a given `expect_sha256` must match
//!    (`restore.checksum_mismatch`).
//! 3. The snapshot is copied to `<live>.restore-tmp-<millis>` in the live
//!    file's directory (`create_new`, mode `0600`, never overwriting; a numeric
//!    suffix resolves a name collision). The **copy** is re-hashed (it must equal
//!    the snapshot hash), refused when its schema is newer than the newest
//!    migration this build embeds (`restore.schema_too_new`) and verified with
//!    [`verify_snapshot`] (`integrity_check`, `foreign_key_check`, migration
//!    checksums, audit chain). On any failure only that temporary copy is removed.
//! 4. When the live file exists, exclusive access is taken with a second
//!    connection (`busy_timeout = 0`, `locking_mode = EXCLUSIVE`,
//!    `BEGIN EXCLUSIVE`). `SQLITE_BUSY`/`SQLITE_LOCKED` refuses with
//!    `restore.database_in_use`. In WAL mode an exclusive-locking connection
//!    needs the database file lock that every open WAL connection holds, so an
//!    idle application connection refuses as well as a writer. A live file that
//!    cannot be opened or read (corrupt: the case restore exists for) is not
//!    locked but is still quarantined.
//! 5. The live WAL is checkpointed (best effort). The live path is never empty and
//!    never holds unverified content, whatever interrupts the swap (a killed
//!    process included):
//!    * the old main file is kept under its quarantine name
//!      `<name>.<UTC yyyymmddThhmmssZ>.quarantine` by a **hard link** (directory
//!      created `0700`; a numeric suffix `.<n>` resolves a collision; existing files
//!      are never overwritten and nothing is deleted); the live path is untouched;
//!    * the `-wal`, `-shm` and `-journal` sidecars are renamed into the quarantine
//!      directory (a stale sidecar next to a different main file could be replayed
//!      over it);
//!    * the verified copy replaces the live path: `rename` over the old main file
//!      (atomic, same directory; the old inode lives on under its quarantine name),
//!      or, when there was no main file, `hard_link` and removal of the temporary
//!      name, which never overwrites: a database that appeared at the live path in
//!      the meantime is left untouched (`restore.swap_failed`);
//!    * the directories are synced.
//!
//!    A failure before the replacement puts the sidecars back where their names are
//!    free and removes only the extra quarantine link, and only while it still
//!    names the live file. A live file replaced by someone else during the swap is
//!    never overwritten (`restore.swap_failed`). The filesystem must support hard
//!    links; if it does not, `restore.swap_failed` is returned before anything
//!    changes.
//! 6. A [`RestoreReport`] describes what happened.
//!
//! Sidecars of the replaced database are always quarantined, even when the live
//! file itself is absent: a stale `-wal` next to a different main file could be
//! replayed over it.
//!
//! The temporary copy's own sidecars (`<temp>-wal`, `<temp>-shm`, which SQLite
//! creates when a snapshot in WAL mode is opened for verification) are removed
//! with the copy, on success and on failure.
//!
//! The quarantine directory must be on the same filesystem as the live file
//! (links and renames only; `restore.swap_failed` otherwise).
//!
//! PROPOSED (not implemented): restoring into a running application, and an
//! in-app restore operation with a `DataDelete`-class approval bound to the live
//! revision. This function is the storage primitive for both and is meant to run
//! offline, with the application stopped.

use crate::audit::AuditChain;
use crate::backup::{sha256_of_file, verify_snapshot, VerifyOptions};
use crate::connection::checkpoint_truncate;
use crate::error::{RestoreCode, StorageError, StorageResult};
use crate::migrations::{Migrator, CURRENT_SCHEMA_VERSION};
use g9_domain::UtcMillis;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Sidecars that belong to a live database file. Moved before the main file so
/// that a stale sidecar is never left next to a replaced main file.
const SIDECAR_SUFFIXES: [&str; 3] = ["-wal", "-shm", "-journal"];
/// Bounded search for a free temporary or quarantine name.
const MAX_NAME_ATTEMPTS: u32 = 1000;

/// Options for [`restore_over_live`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreOptions {
    /// Hex SHA-256 the snapshot must have (case-insensitive). `None` skips the
    /// comparison; the snapshot is still verified.
    pub expect_sha256: Option<String>,
    /// Instant used for the temporary file name and the quarantine stamp.
    pub now: UtcMillis,
}

/// What [`restore_over_live`] did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// Hex SHA-256 of the snapshot bytes that became the live database.
    pub snapshot_sha256: String,
    /// Highest schema version recorded in the restored database.
    pub schema_version: u32,
    /// Where the replaced files now are (empty when there was nothing to move).
    pub quarantined: Vec<PathBuf>,
    /// The live database path (resolved).
    pub live: PathBuf,
    /// Rows of the audit chain recomputed on the verified copy.
    pub audit_rows_verified: u64,
}

fn refuse(code: RestoreCode, message: impl Into<String>) -> StorageError {
    StorageError::restore(code, message)
}

/// Replace the live database file with `snapshot`, moving the replaced files into
/// `quarantine_dir`. See the module documentation for the exact sequence and
/// failure semantics.
pub fn restore_over_live(
    snapshot: &Path,
    live: &Path,
    quarantine_dir: &Path,
    options: &RestoreOptions,
) -> StorageResult<RestoreReport> {
    // (1) Paths and snapshot shape.
    let snapshot = resolve_snapshot(snapshot)?;
    let live = resolve_live(live)?;
    if same_file(&snapshot, &live) {
        return Err(refuse(
            RestoreCode::SnapshotIsLive,
            "the snapshot and the live database are the same file",
        ));
    }
    reject_sidecars(&snapshot)?;

    // (2) Hash and compare before anything is written.
    let expected = normalize_expected(options.expect_sha256.as_deref())?;
    let snapshot_sha256 = sha256_of_file(&snapshot)?;
    if let Some(expected) = &expected {
        if *expected != snapshot_sha256 {
            return Err(refuse(
                RestoreCode::ChecksumMismatch,
                "the snapshot's SHA-256 differs from the expected value",
            ));
        }
    }

    // (3) Copy next to the live file and verify THE COPY.
    let temp = TempCopy::create_from(&snapshot, &live, options.now)?;
    if sha256_of_file(&temp.path)? != snapshot_sha256 {
        return Err(refuse(
            RestoreCode::ChecksumMismatch,
            "the snapshot changed while it was being copied",
        ));
    }
    let (schema_version, audit_rows_verified) = verify_copy(&temp.path)?;

    // (4) Exclusive access to the live database, held until the swap is done.
    let lock = lock_live(&live)?;

    // (5) Checkpoint, quarantine, swap.
    let quarantined = swap(&live, temp, quarantine_dir, options.now, lock)?;

    Ok(RestoreReport {
        snapshot_sha256,
        schema_version,
        quarantined,
        live,
        audit_rows_verified,
    })
}

fn resolve_snapshot(snapshot: &Path) -> StorageResult<PathBuf> {
    let resolved = fs::canonicalize(snapshot).map_err(|e| {
        refuse(
            RestoreCode::SnapshotInvalid,
            format!("the snapshot path cannot be resolved ({:?})", e.kind()),
        )
    })?;
    match fs::metadata(&resolved) {
        Ok(meta) if meta.is_file() => Ok(resolved),
        Ok(_) => Err(refuse(
            RestoreCode::SnapshotInvalid,
            "the snapshot is not a regular file",
        )),
        Err(e) => Err(refuse(
            RestoreCode::SnapshotInvalid,
            format!("the snapshot cannot be read ({:?})", e.kind()),
        )),
    }
}

/// The live path: the file itself when it exists, else its resolved directory
/// plus the file name.
fn resolve_live(live: &Path) -> StorageResult<PathBuf> {
    let name = live.file_name().ok_or_else(|| {
        refuse(
            RestoreCode::LivePathInvalid,
            "the live database path has no file name",
        )
    })?;
    match fs::symlink_metadata(live) {
        Ok(meta) if meta.is_dir() => Err(refuse(
            RestoreCode::LivePathInvalid,
            "the live database path is a directory",
        )),
        Ok(_) => fs::canonicalize(live).map_err(|e| {
            refuse(
                RestoreCode::LivePathInvalid,
                format!("the live database path cannot be resolved ({:?})", e.kind()),
            )
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = live
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = fs::canonicalize(parent).map_err(|e| {
                refuse(
                    RestoreCode::LivePathInvalid,
                    format!(
                        "the live database directory cannot be resolved ({:?})",
                        e.kind()
                    ),
                )
            })?;
            if !parent.is_dir() {
                return Err(refuse(
                    RestoreCode::LivePathInvalid,
                    "the live database directory is not a directory",
                ));
            }
            Ok(parent.join(name))
        }
        Err(e) => Err(refuse(
            RestoreCode::LivePathInvalid,
            format!("the live database path cannot be read ({:?})", e.kind()),
        )),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(x), Ok(y)) = (fs::metadata(a), fs::metadata(b)) {
            return x.dev() == y.dev() && x.ino() == y.ino();
        }
    }
    false
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// A snapshot is one self-contained file: a non-empty `-wal` or `-journal` next
/// to it would hold content that a plain copy of the main file loses.
fn reject_sidecars(snapshot: &Path) -> StorageResult<()> {
    for suffix in ["-wal", "-journal"] {
        let sidecar = with_suffix(snapshot, suffix);
        if let Ok(meta) = fs::symlink_metadata(&sidecar) {
            if meta.len() > 0 {
                return Err(refuse(
                    RestoreCode::SnapshotInvalid,
                    format!("the snapshot has a non-empty {suffix} file; it is not self-contained"),
                ));
            }
        }
    }
    Ok(())
}

fn normalize_expected(expected: Option<&str>) -> StorageResult<Option<String>> {
    match expected {
        None => Ok(None),
        Some(text) => {
            let text = text.trim();
            if text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(Some(text.to_ascii_lowercase()))
            } else {
                Err(refuse(
                    RestoreCode::SnapshotInvalid,
                    "the expected checksum is not a 64-character hex SHA-256",
                ))
            }
        }
    }
}

/// The temporary copy; removed on drop unless the swap consumed it.
#[derive(Debug)]
struct TempCopy {
    path: PathBuf,
    armed: Cell<bool>,
}

impl TempCopy {
    /// Copy `snapshot` to `<live>.restore-tmp-<millis>[-n]` (new file, mode `0600`).
    fn create_from(snapshot: &Path, live: &Path, now: UtcMillis) -> StorageResult<Self> {
        let mut source = File::open(snapshot).map_err(|e| StorageError::io("open snapshot", &e))?;
        for attempt in 0..MAX_NAME_ATTEMPTS {
            let suffix = if attempt == 0 {
                format!(".restore-tmp-{}", now.as_millis())
            } else {
                format!(".restore-tmp-{}-{attempt}", now.as_millis())
            };
            let path = with_suffix(live, &suffix);
            let mut file = match create_new_private(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(StorageError::io("create restore copy", &e)),
            };
            // From here on the guard owns the file: any failure removes it.
            let temp = TempCopy {
                path,
                armed: Cell::new(true),
            };
            io::copy(&mut source, &mut file).map_err(|e| StorageError::io("copy snapshot", &e))?;
            file.sync_all()
                .map_err(|e| StorageError::io("sync restore copy", &e))?;
            return Ok(temp);
        }
        Err(StorageError::Conflict(
            "no free temporary restore file name".to_string(),
        ))
    }
}

impl TempCopy {
    /// The swap is complete: remove what is left under the temporary name. The
    /// copy itself is gone when it was renamed into place, and still there as a
    /// second name for the live file when it was hard-linked.
    fn finish(self) {
        self.armed.set(false);
        remove_if_present(&self.path, "restore temporary copy was not removed");
        self.remove_sidecars();
    }

    /// SQLite creates `<temp>-wal` and `<temp>-shm` when it opens a snapshot in
    /// WAL mode for verification. They belong to this copy's name only.
    fn remove_sidecars(&self) {
        for suffix in SIDECAR_SUFFIXES {
            remove_if_present(
                &with_suffix(&self.path, suffix),
                "restore temporary sidecar was not removed",
            );
        }
    }
}

fn remove_if_present(path: &Path, message: &'static str) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != io::ErrorKind::NotFound {
            tracing::warn!(kind = ?e.kind(), what = message, "a file a restore no longer needs was not removed");
        }
    }
}

impl Drop for TempCopy {
    fn drop(&mut self) {
        if self.armed.get() {
            remove_if_present(&self.path, "restore temporary copy was not removed");
            self.remove_sidecars();
        }
    }
}

fn create_new_private(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Schema gate, then the full snapshot verification, on the copy. Returns the
/// schema version and the number of audit rows recomputed.
fn verify_copy(copy: &Path) -> StorageResult<(u32, u64)> {
    let conn = Connection::open_with_flags(
        copy,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // The copy's schema is not trusted until verified: no function call from its
    // views or triggers (verify_snapshot below also compares the schema itself).
    conn.pragma_update(None, "trusted_schema", false)?;
    // A copy whose schema table cannot be read is reported by the verification below.
    if let Ok(version) = Migrator::schema_version(&conn) {
        if version > CURRENT_SCHEMA_VERSION {
            return Err(refuse(
                RestoreCode::SchemaTooNew,
                format!(
                    "the snapshot's schema version {version} is newer than the newest migration this build knows ({CURRENT_SCHEMA_VERSION})"
                ),
            ));
        }
    }
    let report = verify_snapshot(copy, &VerifyOptions::default())?;
    if !report.ok() {
        return Err(StorageError::Verification(report.errors));
    }
    check_not_truncated(&conn, copy)?;
    let schema_version = Migrator::schema_version(&conn)?;
    let chain = AuditChain::verify_chain(&conn)?;
    if !chain.ok {
        return Err(StorageError::Verification(vec![
            "audit chain did not verify on the restore copy".to_string(),
        ]));
    }
    Ok((schema_version, chain.rows))
}

/// A file shorter than `page_size * page_count` lost its tail. `integrity_check`
/// notices a missing page but not a partial last one (SQLite zero-fills a short
/// read of the last page), so a snapshot cut by a few bytes would otherwise pass
/// when no checksum was given.
fn check_not_truncated(conn: &Connection, copy: &Path) -> StorageResult<()> {
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let needed = u64::try_from(page_size)
        .unwrap_or(0)
        .saturating_mul(u64::try_from(page_count).unwrap_or(0));
    let len = fs::metadata(copy)
        .map_err(|e| StorageError::io("stat restore copy", &e))?
        .len();
    if len < needed {
        return Err(StorageError::Corrupt(
            "the snapshot file is shorter than its page count (truncated)".to_string(),
        ));
    }
    Ok(())
}

/// Exclusive access to the live file. `None` when there is no live file or it
/// cannot be read (then there is nothing to lock; it is still quarantined).
fn lock_live(live: &Path) -> StorageResult<Option<Connection>> {
    match fs::symlink_metadata(live) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(StorageError::io("stat live database", &e)),
    }
    let conn = match Connection::open_with_flags(
        live,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(e) => return unreadable_or_in_use(e),
    };
    // Refuse at once: a held lock is a running application, not a transient wait.
    if let Err(e) = conn.busy_timeout(Duration::ZERO) {
        return unreadable_or_in_use(e);
    }
    // Set before the first access so that, in WAL mode, the connection needs the
    // database file lock of every other open connection and keeps it afterwards.
    match conn.query_row("PRAGMA locking_mode = EXCLUSIVE", [], |r| {
        r.get::<_, String>(0)
    }) {
        Ok(mode) if mode.eq_ignore_ascii_case("exclusive") => {}
        Ok(_) => return Err(StorageError::Internal("restore locking mode")),
        Err(e) => return unreadable_or_in_use(e),
    }
    match conn.execute_batch("BEGIN EXCLUSIVE") {
        Ok(()) => {}
        Err(e) => return unreadable_or_in_use(e),
    }
    // Exclusive locking mode keeps the lock after the transaction ends; the
    // checkpoint below cannot run inside a transaction.
    conn.execute_batch("ROLLBACK")?;
    Ok(Some(conn))
}

fn unreadable_or_in_use(e: rusqlite::Error) -> StorageResult<Option<Connection>> {
    match StorageError::from(e) {
        StorageError::Busy(_) => Err(refuse(
            RestoreCode::DatabaseInUse,
            "the live database is in use by another connection; stop the application and retry",
        )),
        other => {
            tracing::warn!(
                code = other.code(),
                "live database could not be opened for locking; it is quarantined without a lock"
            );
            Ok(None)
        }
    }
}

/// Named moments of the swap at which the test hook may observe the directory,
/// change it, or stop the swap as a killed process would stop it.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapPoint {
    /// The old main file is linked into quarantine; nothing else has moved.
    MainLinked,
    /// The sidecars are in quarantine; the live path still holds the old main file.
    SidecarsMoved,
    /// The verified copy is at the live path; the swap only has bookkeeping left.
    Placed,
}

#[cfg(test)]
type SwapHook = Box<dyn FnMut(SwapPoint, &Path) -> bool>;

#[cfg(test)]
thread_local! {
    static SWAP_HOOK: std::cell::RefCell<Option<SwapHook>> = const { std::cell::RefCell::new(None) };
}

/// `true` when the test hook asks for the swap to stop here without any cleanup
/// (a killed process runs no destructors and undoes nothing).
#[cfg(test)]
fn killed_at(point: SwapPoint, live: &Path) -> bool {
    SWAP_HOOK.with(|hook| hook.borrow_mut().as_mut().is_some_and(|f| f(point, live)))
}

/// Why the verified copy could not be put in place.
enum PlaceError {
    /// The live path no longer names the file that was kept in quarantine.
    LiveChanged,
    /// A file appeared at the live path while there was none.
    Appeared,
    Io(io::Error),
}

/// Whether `a` and `b` are one file (device and inode). Where that cannot be
/// told (non-unix) the answer is `true`: the check it guards is then skipped.
fn names_one_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        true
    }
}

/// Put the verified copy at the live path without ever overwriting a file that is
/// not the old main file kept in quarantine.
fn place(live: &Path, temp: &Path, kept: Option<&Path>) -> Result<(), PlaceError> {
    match kept {
        // Atomic replacement of the old main file, whose inode lives on under its
        // quarantine name. Refused when the live path was swapped for another file.
        Some(kept) => {
            if !names_one_file(live, kept) {
                return Err(PlaceError::LiveChanged);
            }
            fs::rename(temp, live).map_err(PlaceError::Io)
        }
        // No old main file: create the live name without overwriting anything.
        None => fs::hard_link(temp, live).map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                PlaceError::Appeared
            } else {
                PlaceError::Io(e)
            }
        }),
    }
}

/// Checkpoint, keep the old live files in quarantine, put the verified copy at
/// the live path. See the module documentation for the sequence and what each
/// interruption leaves behind.
fn swap(
    live: &Path,
    temp: TempCopy,
    quarantine_dir: &Path,
    now: UtcMillis,
    lock: Option<Connection>,
) -> StorageResult<Vec<PathBuf>> {
    if let Some(conn) = &lock {
        if let Err(e) = checkpoint_truncate(conn) {
            tracing::warn!(
                code = e.code(),
                "live WAL checkpoint failed; the sidecars are quarantined with the database"
            );
        }
    }
    let mut plan = plan_quarantine(live, quarantine_dir, now)?;
    // The main file is planned last; it is linked, the sidecars are renamed.
    let main = match plan.last() {
        Some((from, _)) if from.as_path() == live => plan.pop(),
        _ => None,
    };

    // The old main file gets its quarantine name while keeping its live name.
    if let Some((from, to)) = &main {
        if let Err(e) = fs::hard_link(from, to) {
            return Err(swap_failed(
                "keep the live database in quarantine (hard link)",
                &e,
                true,
            ));
        }
    }
    #[cfg(test)]
    if killed_at(SwapPoint::MainLinked, live) {
        temp.armed.set(false);
        return Err(StorageError::Internal("restore interrupted (test)"));
    }

    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (from, to) in plan {
        if let Err(e) = fs::rename(&from, &to) {
            let restored = undo(&moved, main.as_ref(), live);
            return Err(swap_failed(
                "move a live sidecar into quarantine",
                &e,
                restored,
            ));
        }
        moved.push((from, to));
    }
    #[cfg(test)]
    if killed_at(SwapPoint::SidecarsMoved, live) {
        temp.armed.set(false);
        return Err(StorageError::Internal("restore interrupted (test)"));
    }

    if let Err(failure) = place(live, &temp.path, main.as_ref().map(|(_, to)| to.as_path())) {
        let restored = undo(&moved, main.as_ref(), live);
        return Err(match failure {
            PlaceError::LiveChanged => refuse(
                RestoreCode::SwapFailed,
                "the live database was replaced while the restore was running; it was left untouched",
            ),
            PlaceError::Appeared => refuse(
                RestoreCode::SwapFailed,
                "a database appeared at the live path while the restore was running; it was left untouched",
            ),
            PlaceError::Io(e) => swap_failed("move the restored copy into place", &e, restored),
        });
    }
    #[cfg(test)]
    if killed_at(SwapPoint::Placed, live) {
        temp.armed.set(false);
        return Err(StorageError::Internal("restore interrupted (test)"));
    }
    temp.finish();
    // Closing the connection last: it holds the exclusive lock through the swap.
    drop(lock);

    let quarantined: Vec<PathBuf> = moved
        .into_iter()
        .map(|(_, to)| to)
        .chain(main.map(|(_, to)| to))
        .collect();
    if let Some(dir) = live.parent() {
        sync_dir(dir);
    }
    if let Some(dir) = quarantined.first().and_then(|to| to.parent()) {
        sync_dir(dir);
    }
    Ok(quarantined)
}

/// `(from, to)` renames that move every existing live file into the quarantine
/// directory, sidecars first. Empty when nothing exists (the directory is then
/// not created).
fn plan_quarantine(
    live: &Path,
    quarantine_dir: &Path,
    now: UtcMillis,
) -> StorageResult<Vec<(PathBuf, PathBuf)>> {
    let mut sources: Vec<PathBuf> = SIDECAR_SUFFIXES
        .iter()
        .map(|suffix| with_suffix(live, suffix))
        .collect();
    sources.push(live.to_path_buf());
    sources.retain(|path| fs::symlink_metadata(path).is_ok());
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    create_private_dir(quarantine_dir)?;
    let dir = fs::canonicalize(quarantine_dir)
        .map_err(|e| StorageError::io("resolve quarantine directory", &e))?;
    let stamp = now
        .to_datetime()
        .ok_or_else(|| StorageError::InvalidInput("restore instant is out of range".to_string()))?
        .format("%Y%m%dT%H%M%SZ")
        .to_string();
    // One suffix for the whole set so that a database and its sidecars stay a pair.
    for attempt in 0..MAX_NAME_ATTEMPTS {
        let moves: Vec<(PathBuf, PathBuf)> = sources
            .iter()
            .map(|from| {
                (
                    from.clone(),
                    dir.join(quarantine_name(from, &stamp, attempt)),
                )
            })
            .collect();
        if moves.iter().all(|(_, to)| is_free(to)) {
            return Ok(moves);
        }
    }
    Err(StorageError::Conflict(
        "no free quarantine file name".to_string(),
    ))
}

fn quarantine_name(from: &Path, stamp: &str, attempt: u32) -> OsString {
    let mut name: OsString = from.file_name().map(OsString::from).unwrap_or_default();
    name.push(format!(".{stamp}"));
    if attempt > 0 {
        name.push(format!(".{attempt}"));
    }
    name.push(".quarantine");
    name
}

fn is_free(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
}

fn create_private_dir(dir: &Path) -> StorageResult<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(dir)
        .map_err(|e| StorageError::io("create quarantine directory", &e))?;
    if fs::metadata(dir).is_ok_and(|m| m.is_dir()) {
        Ok(())
    } else {
        Err(StorageError::InvalidInput(
            "the quarantine path is not a directory".to_string(),
        ))
    }
}

/// Undo a swap that failed before the verified copy was in place: put the
/// sidecars back (reverse order; only where the original name is still free) and
/// remove the extra quarantine link to the main file, but only while that link and
/// the live path still name one file (if the live path was replaced, the link is
/// the old database's only name and stays). `true` when everything is back.
fn undo(moved: &[(PathBuf, PathBuf)], main: Option<&(PathBuf, PathBuf)>, live: &Path) -> bool {
    let mut all_back = true;
    for (from, to) in moved.iter().rev() {
        let back = fs::hard_link(to, from).and_then(|()| fs::remove_file(to));
        if let Err(e) = back {
            all_back = false;
            tracing::error!(kind = ?e.kind(), "restore could not put a quarantined file back");
        }
    }
    if let Some((_, kept)) = main {
        if names_one_file(live, kept) {
            if let Err(e) = fs::remove_file(kept) {
                all_back = false;
                tracing::error!(kind = ?e.kind(), "restore could not remove its quarantine link");
            }
        } else {
            all_back = false;
        }
    }
    all_back
}

fn swap_failed(what: &str, e: &io::Error, restored: bool) -> StorageError {
    tracing::warn!(kind = ?e.kind(), what, "restore swap failed");
    let hint = if e.kind() == io::ErrorKind::CrossesDevices {
        "; the quarantine directory must be on the same filesystem as the live database"
    } else {
        ""
    };
    let state = if restored {
        "the live database was left in place"
    } else {
        "some replaced files remain in the quarantine directory"
    };
    refuse(
        RestoreCode::SwapFailed,
        format!("could not {what} ({:?}){hint}; {state}", e.kind()),
    )
}

/// Best-effort durability of a rename; the swap itself is already done.
fn sync_dir(dir: &Path) {
    if let Err(e) = File::open(dir).and_then(|d| d.sync_all()) {
        tracing::warn!(kind = ?e.kind(), "directory sync after restore failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditDecision, AuditEntry, AuditOutcome};
    use crate::backup::{create_snapshot, BackupManifest, SnapshotRequest};
    use crate::connection::{Database, OpenOptions};
    use g9_domain::{Actor, ActorId, BackupId, RequestId, ResourceKind, ResourceRef, TraceId};
    use pretty_assertions::assert_eq;

    /// 2026-11-08T10:15:30Z.
    const NOW: i64 = 1_794_132_930_000;
    const STAMP: &str = "20261108T101530Z";

    fn now() -> UtcMillis {
        UtcMillis::from_millis(NOW)
    }

    fn opts() -> RestoreOptions {
        RestoreOptions {
            expect_sha256: None,
            now: now(),
        }
    }

    fn open(path: &Path) -> Database {
        Database::open(
            path,
            OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)),
        )
        .unwrap()
    }

    /// Marker row (the `idempotency_records` table has few NOT NULL columns).
    fn put(conn: &Connection, key: &str) {
        conn.execute(
            "INSERT INTO idempotency_records (actor_id, idempotency_key, request_sha256, outcome_json, created_at)
             VALUES ('a', ?1, ?2, '{}', 1)",
            rusqlite::params![key, "0".repeat(64)],
        )
        .unwrap();
    }

    fn audit(conn: &Connection, n: usize) {
        AuditChain::append(
            conn,
            &AuditEntry {
                occurred_at: UtcMillis::from_millis(10 + n as i64),
                actor: Actor::operator(ActorId::parse("operator-1").unwrap()),
                operation: "note.create".to_string(),
                resource: Some(ResourceRef::new(ResourceKind::Note, "n-1")),
                decision: AuditDecision::Allow,
                outcome: AuditOutcome::Success,
                request_id: RequestId::parse(&format!("req-{n}")).unwrap(),
                trace_id: TraceId::parse("trace-1").unwrap(),
                details_sha256: None,
            },
        )
        .unwrap();
    }

    /// Marker keys of a closed database file (immutable: no WAL, no lock, no sidecars).
    fn keys(path: &Path) -> Vec<String> {
        let conn = Connection::open_with_flags(
            format!("file:{}?immutable=1", path.display()),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .unwrap();
        let mut stmt = conn
            .prepare("SELECT idempotency_key FROM idempotency_records ORDER BY idempotency_key")
            .unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(Result::unwrap).collect()
    }

    struct Fixture {
        dir: tempfile::TempDir,
        live: PathBuf,
        snapshot: PathBuf,
        manifest: BackupManifest,
    }

    impl Fixture {
        /// Live database with `before-1`, `before-2` (+2 audit rows), a snapshot of
        /// it, then `after-snapshot` (+1 audit row); the database is closed.
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let live = dir.path().join("g9.sqlite");
            let snapshot = dir.path().join("backups").join("s1.sqlite");
            fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
            let db = open(&live);
            put(db.conn(), "before-1");
            put(db.conn(), "before-2");
            audit(db.conn(), 1);
            audit(db.conn(), 2);
            let manifest = create_snapshot(
                db.conn(),
                &snapshot,
                &SnapshotRequest {
                    backup_id: BackupId::parse("s1").unwrap(),
                    requested_at: UtcMillis::from_millis(5),
                    app_version: "test".to_string(),
                },
            )
            .unwrap();
            put(db.conn(), "after-snapshot");
            audit(db.conn(), 3);
            drop(db);
            Self {
                dir,
                live,
                snapshot,
                manifest,
            }
        }

        fn quarantine(&self) -> PathBuf {
            self.dir.path().join("quarantine")
        }

        fn restore(&self, options: &RestoreOptions) -> StorageResult<RestoreReport> {
            restore_over_live(&self.snapshot, &self.live, &self.quarantine(), options)
        }

        fn live_bytes(&self) -> Vec<u8> {
            fs::read(&self.live).unwrap()
        }

        /// No temporary restore file anywhere in the live directory.
        fn assert_no_temp(&self) {
            let leftovers: Vec<String> = fs::read_dir(self.dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n.contains(".restore-tmp-"))
                .collect();
            assert_eq!(leftovers, Vec::<String>::new());
        }
    }

    fn code_of(result: StorageResult<RestoreReport>) -> &'static str {
        result.unwrap_err().code()
    }

    /// The quarantined main database file of a report (the sidecars carry
    /// `-wal`/`-shm`/`-journal` in their names).
    fn quarantined_main(report: &RestoreReport) -> &PathBuf {
        report
            .quarantined
            .iter()
            .find(|p| {
                let name = p.file_name().unwrap().to_string_lossy();
                name.starts_with("g9.sqlite.")
            })
            .unwrap()
    }

    #[test]
    fn happy_path_replaces_the_live_database_and_quarantines_the_old_one() {
        let f = Fixture::new();
        assert_eq!(keys(&f.live), ["after-snapshot", "before-1", "before-2"]);
        let report = f
            .restore(&RestoreOptions {
                expect_sha256: Some(f.manifest.sha256_of_file.to_ascii_uppercase()),
                now: now(),
            })
            .unwrap();

        // Rows of the snapshot are live, rows written after it are gone.
        let reopened = open(&f.live);
        assert_eq!(
            crate::migrations::integrity_check(reopened.conn()).unwrap(),
            ["ok"]
        );
        drop(reopened);
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);

        // The report describes the verified copy.
        assert_eq!(report.snapshot_sha256, f.manifest.sha256_of_file);
        assert_eq!(report.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(report.audit_rows_verified, 2);
        assert_eq!(report.live, fs::canonicalize(&f.live).unwrap());

        // The quarantine holds the old content under the documented name; the
        // only other file is the (empty, checkpointed) WAL of the live database.
        let quarantined = quarantined_main(&report);
        assert_eq!(
            quarantined.file_name().unwrap().to_string_lossy(),
            format!("g9.sqlite.{STAMP}.quarantine")
        );
        for other in report.quarantined.iter().filter(|p| *p != quarantined) {
            assert_eq!(
                other.file_name().unwrap().to_string_lossy(),
                format!("g9.sqlite-wal.{STAMP}.quarantine")
            );
            assert_eq!(fs::metadata(other).unwrap().len(), 0);
        }
        assert_eq!(
            keys(quarantined),
            ["after-snapshot", "before-1", "before-2"]
        );
        assert!(!with_suffix(&f.live, "-wal").exists());
        f.assert_no_temp();
        // The snapshot itself is untouched.
        assert_eq!(
            crate::backup::sha256_of_file(&f.snapshot).unwrap(),
            f.manifest.sha256_of_file
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(f.quarantine()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn checksum_mismatch_changes_nothing() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let wrong = RestoreOptions {
            expect_sha256: Some("0".repeat(64)),
            now: now(),
        };
        assert_eq!(code_of(f.restore(&wrong)), "restore.checksum_mismatch");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
        assert!(!f.quarantine().exists());
        // A malformed expectation is refused before anything is read.
        let malformed = RestoreOptions {
            expect_sha256: Some("not-hex".to_string()),
            now: now(),
        };
        assert_eq!(code_of(f.restore(&malformed)), "restore.snapshot_invalid");
        assert_eq!(f.live_bytes(), before);
    }

    #[test]
    fn corrupt_snapshots_are_refused_and_leave_no_temp_file() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let good = fs::read(&f.snapshot).unwrap();
        for (name, bytes) in [
            ("garbage", vec![0x5a_u8; 8192]),
            ("empty", Vec::new()),
            ("truncated", good[..good.len() / 2 + 100].to_vec()),
            ("bad header", {
                let mut b = good.clone();
                b[..16].copy_from_slice(b"NOT A DATABASE!\0");
                b
            }),
        ] {
            let bad = f.dir.path().join("backups").join(format!("{name}.sqlite"));
            fs::write(&bad, &bytes).unwrap();
            let err = restore_over_live(&bad, &f.live, &f.quarantine(), &opts()).unwrap_err();
            assert!(
                matches!(
                    err.code(),
                    "storage.corrupt" | "storage.backup_verification_failed"
                ),
                "{name}: {err:?}"
            );
            assert_eq!(f.live_bytes(), before, "{name}");
            f.assert_no_temp();
            assert!(!f.quarantine().exists(), "{name}");
        }
    }

    #[test]
    fn a_locked_live_database_is_refused_and_untouched() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let holder = Connection::open(&f.live).unwrap();
        holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
        assert_eq!(code_of(f.restore(&opts())), "restore.database_in_use");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
        assert!(!f.quarantine().exists());
        // Once the lock is released the same restore succeeds.
        holder.execute_batch("ROLLBACK").unwrap();
        drop(holder);
        f.restore(&opts()).unwrap();
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
    }

    #[test]
    fn an_idle_open_application_connection_is_refused() {
        let f = Fixture::new();
        let app = open(&f.live);
        assert_eq!(app.profile().journal_mode, "wal");
        let before = f.live_bytes();
        assert_eq!(code_of(f.restore(&opts())), "restore.database_in_use");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
        assert!(!f.quarantine().exists());
        // The refused restore did not disturb the application's connection.
        put(app.conn(), "still-writable");
        drop(app);
        f.restore(&opts()).unwrap();
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
    }

    #[test]
    fn a_snapshot_with_a_newer_schema_is_refused() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let future = f.dir.path().join("backups").join("future.sqlite");
        fs::copy(&f.snapshot, &future).unwrap();
        let conn = Connection::open(&future).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, sha256, applied_at, app_version)
             VALUES (?1, 'from_the_future', ?2, 1, 'newer')",
            rusqlite::params![CURRENT_SCHEMA_VERSION + 1, "0".repeat(64)],
        )
        .unwrap();
        drop(conn);
        let result = restore_over_live(&future, &f.live, &f.quarantine(), &opts());
        assert_eq!(code_of(result), "restore.schema_too_new");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
        assert!(!f.quarantine().exists());
    }

    #[test]
    fn a_snapshot_equal_to_the_live_file_is_refused() {
        let f = Fixture::new();
        let before = f.live_bytes();
        for snapshot in [
            f.live.clone(),
            f.dir.path().join(".").join("g9.sqlite"),
            f.dir.path().join("backups").join("..").join("g9.sqlite"),
        ] {
            let result = restore_over_live(&snapshot, &f.live, &f.quarantine(), &opts());
            assert_eq!(code_of(result), "restore.snapshot_is_live");
        }
        // A hard link is the same file under another name.
        let link = f.dir.path().join("link.sqlite");
        fs::hard_link(&f.live, &link).unwrap();
        let result = restore_over_live(&link, &f.live, &f.quarantine(), &opts());
        assert_eq!(code_of(result), "restore.snapshot_is_live");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
    }

    #[test]
    fn unusable_snapshot_and_live_paths_are_refused() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let missing = f.dir.path().join("backups").join("missing.sqlite");
        let dir = f.dir.path().join("backups");
        for snapshot in [&missing, &dir] {
            let result = restore_over_live(snapshot, &f.live, &f.quarantine(), &opts());
            assert_eq!(code_of(result), "restore.snapshot_invalid");
        }
        // A snapshot with a non-empty write-ahead log is not self-contained.
        fs::write(with_suffix(&f.snapshot, "-wal"), b"frames").unwrap();
        assert_eq!(code_of(f.restore(&opts())), "restore.snapshot_invalid");
        fs::remove_file(with_suffix(&f.snapshot, "-wal")).unwrap();
        // The live path must have a file name and an existing directory.
        let no_dir = f.dir.path().join("nowhere").join("g9.sqlite");
        let result = restore_over_live(&f.snapshot, &no_dir, &f.quarantine(), &opts());
        assert_eq!(code_of(result), "restore.live_path_invalid");
        let result = restore_over_live(&f.snapshot, f.dir.path(), &f.quarantine(), &opts());
        assert_eq!(code_of(result), "restore.live_path_invalid");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
    }

    #[test]
    fn sidecars_are_checkpointed_and_moved_with_the_database() {
        let f = Fixture::new();
        // A database whose last writes sit in its WAL: image the files while the
        // connection is open (what a killed process leaves behind).
        let src = tempfile::tempdir().unwrap();
        let src_live = src.path().join("g9.sqlite");
        let db = open(&src_live);
        db.conn()
            .execute_batch("PRAGMA wal_autocheckpoint = 0")
            .unwrap();
        put(db.conn(), "in-main");
        db.conn()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        put(db.conn(), "only-in-wal");
        let wal = with_suffix(&src_live, "-wal");
        assert!(fs::metadata(&wal).unwrap().len() > 0);
        fs::remove_file(&f.live).unwrap();
        for suffix in ["", "-wal", "-shm"] {
            fs::copy(with_suffix(&src_live, suffix), with_suffix(&f.live, suffix)).unwrap();
        }
        drop(db);
        assert!(with_suffix(&f.live, "-wal").exists());
        assert!(with_suffix(&f.live, "-shm").exists());

        let report = f.restore(&opts()).unwrap();
        let mut names: Vec<String> = report
            .quarantined
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                format!("g9.sqlite-shm.{STAMP}.quarantine"),
                format!("g9.sqlite-wal.{STAMP}.quarantine"),
                format!("g9.sqlite.{STAMP}.quarantine"),
            ]
        );
        // Nothing of the old database is left beside the new live file.
        assert!(!with_suffix(&f.live, "-wal").exists());
        assert!(!with_suffix(&f.live, "-shm").exists());
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
        // The checkpoint merged the WAL into the quarantined main file.
        let main = report
            .quarantined
            .iter()
            .find(|p| p.to_string_lossy().contains("g9.sqlite."))
            .unwrap();
        assert_eq!(keys(main), ["in-main", "only-in-wal"]);
        f.assert_no_temp();
    }

    #[test]
    fn quarantine_names_never_overwrite_and_get_a_numeric_suffix() {
        let f = Fixture::new();
        fs::create_dir_all(f.quarantine()).unwrap();
        let taken = f.quarantine().join(format!("g9.sqlite.{STAMP}.quarantine"));
        fs::write(&taken, b"sentinel").unwrap();

        let first = f.restore(&opts()).unwrap();
        assert_eq!(fs::read(&taken).unwrap(), b"sentinel");
        assert_eq!(
            quarantined_main(&first)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            format!("g9.sqlite.{STAMP}.1.quarantine")
        );

        // Replace the live file again at the same instant: the next free suffix.
        fs::remove_file(&f.live).unwrap();
        let db = open(&f.live);
        put(db.conn(), "second-generation");
        drop(db);
        let second = f.restore(&opts()).unwrap();
        assert_eq!(
            quarantined_main(&second)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            format!("g9.sqlite.{STAMP}.2.quarantine")
        );
        assert_eq!(fs::read(&taken).unwrap(), b"sentinel");
        assert_eq!(keys(quarantined_main(&first)).len(), 3);
        assert_eq!(keys(quarantined_main(&second)), ["second-generation"]);
    }

    #[test]
    fn a_missing_live_file_is_restored_without_quarantine() {
        let f = Fixture::new();
        fs::remove_file(&f.live).unwrap();
        let report = f.restore(&opts()).unwrap();
        assert_eq!(report.quarantined, Vec::<PathBuf>::new());
        assert!(!f.quarantine().exists());
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
        f.assert_no_temp();
    }

    #[test]
    fn an_unreadable_live_file_is_quarantined_without_a_lock() {
        let f = Fixture::new();
        let garbage = vec![0xab_u8; 8192];
        fs::write(&f.live, &garbage).unwrap();
        let report = f.restore(&opts()).unwrap();
        assert_eq!(fs::read(quarantined_main(&report)).unwrap(), garbage);
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
        f.assert_no_temp();
    }

    #[test]
    fn a_rollback_journal_live_file_is_restored_too() {
        // The snapshot itself is a rollback-journal database: use it as the live file.
        let f = Fixture::new();
        fs::remove_file(&f.live).unwrap();
        fs::copy(&f.snapshot, &f.live).unwrap();
        let db = Connection::open(&f.live).unwrap();
        put(&db, "written-to-the-old-live-file");
        drop(db);
        let report = f.restore(&opts()).unwrap();
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
        assert_eq!(
            keys(quarantined_main(&report)),
            ["before-1", "before-2", "written-to-the-old-live-file"]
        );
        f.assert_no_temp();
    }

    #[test]
    fn an_unusable_quarantine_path_is_refused_before_the_swap() {
        let f = Fixture::new();
        let blocker = f.dir.path().join("quarantine-is-a-file");
        fs::write(&blocker, b"not a directory").unwrap();
        let err = restore_over_live(&f.snapshot, &f.live, &blocker, &opts()).unwrap_err();
        assert!(
            matches!(err.code(), "storage.io" | "storage.invalid_input"),
            "{err:?}"
        );
        // The live database still holds its rows and the blocker is untouched.
        assert_eq!(keys(&f.live), ["after-snapshot", "before-1", "before-2"]);
        assert_eq!(fs::read(&blocker).unwrap(), b"not a directory");
        f.assert_no_temp();
    }

    #[test]
    fn a_taken_temporary_name_is_not_overwritten() {
        let f = Fixture::new();
        let taken = with_suffix(&f.live, &format!(".restore-tmp-{NOW}"));
        fs::write(&taken, b"someone else's file").unwrap();
        f.restore(&opts()).unwrap();
        assert_eq!(fs::read(&taken).unwrap(), b"someone else's file");
        assert_eq!(keys(&f.live), ["before-1", "before-2"]);
        // Only the sentinel is left; the copy this call created is gone.
        let temps: Vec<String> = fs::read_dir(f.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".restore-tmp-"))
            .collect();
        assert_eq!(temps, [format!("g9.sqlite.restore-tmp-{NOW}")]);
    }

    /// Installs a swap hook for the current thread; removed on drop.
    struct HookGuard;

    impl HookGuard {
        fn set(hook: impl FnMut(SwapPoint, &Path) -> bool + 'static) -> Self {
            SWAP_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
            HookGuard
        }
    }

    impl Drop for HookGuard {
        fn drop(&mut self) {
            SWAP_HOOK.with(|h| *h.borrow_mut() = None);
        }
    }

    const OLD_ROWS: [&str; 3] = ["after-snapshot", "before-1", "before-2"];
    const NEW_ROWS: [&str; 2] = ["before-1", "before-2"];

    fn restore_temp_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".restore-tmp-"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_live_path_holds_a_complete_database_at_every_point_of_the_swap() {
        let f = Fixture::new();
        type Seen = Vec<(SwapPoint, bool, Vec<String>)>;
        let seen: std::rc::Rc<std::cell::RefCell<Seen>> = std::rc::Rc::default();
        let log = std::rc::Rc::clone(&seen);
        let _hook = HookGuard::set(move |point, live| {
            let present = live.exists();
            let rows = if present { keys(live) } else { Vec::new() };
            log.borrow_mut().push((point, present, rows));
            false
        });
        f.restore(&opts()).unwrap();
        let old: Vec<String> = OLD_ROWS.iter().map(|s| s.to_string()).collect();
        let new: Vec<String> = NEW_ROWS.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            *seen.borrow(),
            vec![
                (SwapPoint::MainLinked, true, old.clone()),
                (SwapPoint::SidecarsMoved, true, old),
                (SwapPoint::Placed, true, new),
            ]
        );
    }

    #[test]
    fn a_process_killed_during_the_swap_leaves_a_live_database_and_loses_nothing() {
        for point in [
            SwapPoint::MainLinked,
            SwapPoint::SidecarsMoved,
            SwapPoint::Placed,
        ] {
            let f = Fixture::new();
            let hook = HookGuard::set(move |at, _| at == point);
            let err = f.restore(&opts()).unwrap_err();
            assert_eq!(err.code(), "storage.internal", "{point:?}");
            drop(hook);

            // Whatever moment the process died at, the live path holds a complete
            // database: the old one before the replacement, the new one after.
            let expected: &[&str] = if point == SwapPoint::Placed {
                &NEW_ROWS
            } else {
                &OLD_ROWS
            };
            assert_eq!(keys(&f.live), expected, "{point:?}");
            let quarantined: Vec<PathBuf> = fs::read_dir(f.quarantine())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            let main = quarantined
                .iter()
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("g9.sqlite.")
                })
                .unwrap();
            // The old database is still reachable under its quarantine name.
            assert_eq!(keys(main), OLD_ROWS, "{point:?}");

            if point == SwapPoint::Placed {
                continue;
            }
            // The verified copy the killed run left behind is complete, and a rerun
            // finishes the job without touching it.
            let left = restore_temp_names(f.dir.path());
            assert_eq!(left.len(), 1, "{point:?}: {left:?}");
            assert_eq!(
                fs::read(f.dir.path().join(&left[0])).unwrap(),
                fs::read(&f.snapshot).unwrap()
            );
            f.restore(&opts()).unwrap();
            assert_eq!(keys(&f.live), NEW_ROWS, "{point:?}");
            assert_eq!(restore_temp_names(f.dir.path()), left, "{point:?}");
        }
    }

    #[test]
    fn a_database_that_appears_at_the_live_path_is_never_overwritten() {
        let f = Fixture::new();
        fs::remove_file(&f.live).unwrap();
        // A stale sidecar of the vanished database is quarantined, and put back.
        let stale = with_suffix(&f.live, "-wal");
        fs::write(&stale, b"stale frames").unwrap();
        let live = f.live.clone();
        let _hook = HookGuard::set(move |point, _| {
            if point == SwapPoint::SidecarsMoved {
                // Another process created the database in the meantime.
                fs::write(&live, b"someone else's new database").unwrap();
            }
            false
        });
        let err = f.restore(&opts()).unwrap_err();
        assert_eq!(err.code(), "restore.swap_failed", "{err:?}");
        assert_eq!(fs::read(&f.live).unwrap(), b"someone else's new database");
        assert_eq!(fs::read(&stale).unwrap(), b"stale frames");
        f.assert_no_temp();
    }

    #[test]
    fn a_live_file_replaced_during_the_swap_is_never_overwritten() {
        let f = Fixture::new();
        let live = f.live.clone();
        let _hook = HookGuard::set(move |point, _| {
            if point == SwapPoint::SidecarsMoved {
                // The live name is taken over by a different file.
                fs::remove_file(&live).unwrap();
                fs::write(&live, b"someone else's database").unwrap();
            }
            false
        });
        let err = f.restore(&opts()).unwrap_err();
        assert_eq!(err.code(), "restore.swap_failed", "{err:?}");
        assert_eq!(fs::read(&f.live).unwrap(), b"someone else's database");
        // The old database's only remaining name is the quarantine link: kept.
        let main = fs::read_dir(f.quarantine())
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("g9.sqlite.")
            })
            .unwrap();
        assert_eq!(keys(&main), OLD_ROWS);
        f.assert_no_temp();
    }

    #[test]
    fn a_failed_swap_removes_only_its_own_quarantine_link() {
        let f = Fixture::new();
        let live = f.live.clone();
        // Fail between the link and the placement by taking the sidecar's
        // original name for the quarantine plan: a directory cannot be replaced
        // by `place` when it is not the kept file, so use the hook instead.
        let _hook = HookGuard::set(move |point, _| {
            if point == SwapPoint::SidecarsMoved {
                // An unrelated file takes the live name's place and is then removed
                // again: the live path is gone, the link is the only name.
                fs::remove_file(&live).unwrap();
            }
            false
        });
        let err = f.restore(&opts()).unwrap_err();
        assert_eq!(err.code(), "restore.swap_failed", "{err:?}");
        assert!(!f.live.exists());
        let kept: Vec<PathBuf> = fs::read_dir(f.quarantine())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("g9.sqlite.")
            })
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(keys(&kept[0]), OLD_ROWS);
        f.assert_no_temp();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_paths_are_resolved_and_never_followed_into_the_live_file() {
        let f = Fixture::new();
        let before = f.live_bytes();
        // A symlink to a good snapshot is resolved and restored like the snapshot.
        let link = f.dir.path().join("link.sqlite");
        std::os::unix::fs::symlink(&f.snapshot, &link).unwrap();
        // A symlink to the live file is the live file.
        let to_live = f.dir.path().join("to-live.sqlite");
        std::os::unix::fs::symlink(&f.live, &to_live).unwrap();
        let err = restore_over_live(&to_live, &f.live, &f.quarantine(), &opts()).unwrap_err();
        assert_eq!(err.code(), "restore.snapshot_is_live");
        // A dangling symlink at the live path cannot be resolved: refused, untouched.
        let dangling = f.dir.path().join("dangling.sqlite");
        std::os::unix::fs::symlink(f.dir.path().join("nowhere.sqlite"), &dangling).unwrap();
        let err = restore_over_live(&f.snapshot, &dangling, &f.quarantine(), &opts()).unwrap_err();
        assert_eq!(err.code(), "restore.live_path_invalid");
        assert_eq!(f.live_bytes(), before);
        f.assert_no_temp();
        assert!(!f.quarantine().exists());

        let report = restore_over_live(&link, &f.live, &f.quarantine(), &opts()).unwrap();
        assert_eq!(report.snapshot_sha256, f.manifest.sha256_of_file);
        assert_eq!(keys(&f.live), NEW_ROWS);
        // The symlink itself was not replaced or removed.
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(f.snapshot.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_quarantine_directory_on_another_filesystem_is_refused_before_anything_changes() {
        use std::os::unix::fs::MetadataExt;
        let shm = Path::new("/dev/shm");
        let f = Fixture::new();
        let same_device = fs::metadata(shm)
            .map(|m| m.dev() == fs::metadata(f.dir.path()).unwrap().dev())
            .unwrap_or(true);
        if same_device {
            // No second filesystem here; the cross-device path is NOT RUN.
            return;
        }
        let other = tempfile::tempdir_in(shm).unwrap();
        let before = f.live_bytes();
        let err =
            restore_over_live(&f.snapshot, &f.live, &other.path().join("q"), &opts()).unwrap_err();
        assert_eq!(err.code(), "restore.swap_failed", "{err:?}");
        assert!(format!("{err:?}").contains("same filesystem"), "{err:?}");
        assert_eq!(f.live_bytes(), before);
        assert_eq!(keys(&f.live), OLD_ROWS);
        f.assert_no_temp();
        // Nothing was linked or moved into the other filesystem.
        let quarantined: Vec<_> = fs::read_dir(other.path().join("q")).unwrap().collect();
        assert!(quarantined.is_empty(), "{quarantined:?}");
    }

    #[test]
    fn undo_never_overwrites_a_name_that_was_taken_in_the_meantime() {
        let dir = tempfile::tempdir().unwrap();
        let q = dir.path().join("q");
        fs::create_dir_all(&q).unwrap();
        let live = dir.path().join("g9.sqlite");
        let wal = with_suffix(&live, "-wal");
        let shm = with_suffix(&live, "-shm");
        let q_wal = q.join("wal.quarantine");
        let q_shm = q.join("shm.quarantine");
        let q_main = q.join("main.quarantine");
        fs::write(&q_wal, b"wal").unwrap();
        fs::write(&q_shm, b"shm").unwrap();
        // The live name is taken again by a new file; the shm name is free.
        fs::write(&wal, b"a newer wal").unwrap();
        fs::write(&live, b"the old main").unwrap();
        fs::hard_link(&live, &q_main).unwrap();
        let moved = vec![(wal.clone(), q_wal.clone()), (shm.clone(), q_shm.clone())];
        let main = (live.clone(), q_main.clone());

        let all_back = undo(&moved, Some(&main), &live);
        assert!(!all_back, "one sidecar could not go back");
        // The free name got its file back; the taken name was not overwritten and
        // its quarantined file stays.
        assert_eq!(fs::read(&shm).unwrap(), b"shm");
        assert!(!q_shm.exists());
        assert_eq!(fs::read(&wal).unwrap(), b"a newer wal");
        assert_eq!(fs::read(&q_wal).unwrap(), b"wal");
        // The extra link was removed because it still named the live file.
        assert_eq!(fs::read(&live).unwrap(), b"the old main");
        assert!(!q_main.exists());

        // When the live name was taken over by another file, the link is the old
        // file's only name and stays.
        fs::hard_link(&live, &q_main).unwrap();
        fs::remove_file(&live).unwrap();
        fs::write(&live, b"someone else's").unwrap();
        assert!(!undo(&[], Some(&main), &live));
        assert_eq!(fs::read(&q_main).unwrap(), b"the old main");
        assert_eq!(fs::read(&live).unwrap(), b"someone else's");
    }

    #[test]
    fn a_snapshot_cut_short_inside_its_last_page_is_refused_without_a_checksum() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let good = fs::read(&f.snapshot).unwrap();
        for lost in [1_usize, 100, 4095] {
            let cut = f
                .dir
                .path()
                .join("backups")
                .join(format!("cut{lost}.sqlite"));
            fs::write(&cut, &good[..good.len() - lost]).unwrap();
            let err = restore_over_live(&cut, &f.live, &f.quarantine(), &opts()).unwrap_err();
            // A few bytes: only the length check notices; a whole missing page
            // below the last one is also seen by `integrity_check`.
            assert!(
                matches!(
                    err.code(),
                    "storage.corrupt" | "storage.backup_verification_failed"
                ),
                "{lost}: {err:?}"
            );
            assert_eq!(f.live_bytes(), before, "{lost}");
            f.assert_no_temp();
            assert!(!f.quarantine().exists(), "{lost}");
        }
        // The intact file is still accepted.
        f.restore(&opts()).unwrap();
        assert_eq!(keys(&f.live), NEW_ROWS);
    }

    /// A copy of the fixture snapshot whose header says WAL mode.
    fn wal_mode_snapshot(f: &Fixture, name: &str) -> PathBuf {
        let path = f.dir.path().join("backups").join(name);
        fs::copy(&f.snapshot, &path).unwrap();
        let conn = Connection::open(&path).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        drop(conn);
        path
    }

    #[test]
    fn a_wal_mode_snapshot_leaves_nothing_behind_under_the_temporary_name() {
        // Verifying a WAL-mode copy makes SQLite create `<copy>-wal` and `<copy>-shm`.
        let f = Fixture::new();
        let snapshot = wal_mode_snapshot(&f, "wal.sqlite");
        restore_over_live(&snapshot, &f.live, &f.quarantine(), &opts()).unwrap();
        assert_eq!(keys(&f.live), NEW_ROWS);
        assert_eq!(restore_temp_names(f.dir.path()), Vec::<String>::new());

        // The same when the verification fails: a tampered audit chain.
        let f = Fixture::new();
        let snapshot = wal_mode_snapshot(&f, "wal-bad.sqlite");
        let conn = Connection::open(&snapshot).unwrap();
        conn.execute(
            "INSERT INTO audit_metadata (id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, operation, decision, outcome, request_id, trace_id)
             VALUES ('forged-1', ?1, ?1, 1, 'attacker', 'MODEL', 'grade.finalize', 'ALLOW', 'SUCCESS', 'r', 't')",
            rusqlite::params!["0".repeat(64)],
        )
        .unwrap();
        drop(conn);
        let before = f.live_bytes();
        let err = restore_over_live(&snapshot, &f.live, &f.quarantine(), &opts()).unwrap_err();
        assert_eq!(err.code(), "storage.backup_verification_failed", "{err:?}");
        assert_eq!(f.live_bytes(), before);
        assert_eq!(restore_temp_names(f.dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_snapshot_that_violates_foreign_keys_or_breaks_the_audit_chain_is_refused() {
        let f = Fixture::new();
        let before = f.live_bytes();
        let tampered = f.dir.path().join("backups").join("tampered.sqlite");
        fs::copy(&f.snapshot, &tampered).unwrap();
        let conn = Connection::open(&tampered).unwrap();
        conn.execute(
            "INSERT INTO audit_metadata (id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, operation, decision, outcome, request_id, trace_id)
             VALUES ('forged-1', ?1, ?1, 1, 'attacker', 'MODEL', 'grade.finalize', 'ALLOW', 'SUCCESS', 'r', 't')",
            rusqlite::params!["0".repeat(64)],
        )
        .unwrap();
        drop(conn);
        let orphaned = f.dir.path().join("backups").join("orphaned.sqlite");
        fs::copy(&f.snapshot, &orphaned).unwrap();
        let conn = Connection::open(&orphaned).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF; DELETE FROM notes;")
            .ok();
        let broken: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        drop(conn);
        for (name, path) in [("audit chain", &tampered), ("foreign keys", &orphaned)] {
            if name == "foreign keys" && broken == 0 {
                continue;
            }
            let err = restore_over_live(path, &f.live, &f.quarantine(), &opts()).unwrap_err();
            assert_eq!(
                err.code(),
                "storage.backup_verification_failed",
                "{name}: {err:?}"
            );
            assert_eq!(f.live_bytes(), before, "{name}");
            f.assert_no_temp();
            assert!(!f.quarantine().exists(), "{name}");
        }
    }

    #[test]
    fn quarantine_plan_covers_every_sidecar_and_never_collides() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("g9.sqlite");
        for suffix in ["", "-wal", "-shm", "-journal"] {
            fs::write(with_suffix(&live, suffix), suffix.as_bytes()).unwrap();
        }
        let q = dir.path().join("q");
        let plan = plan_quarantine(&live, &q, now()).unwrap();
        let tos: Vec<String> = plan
            .iter()
            .map(|(_, to)| to.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // Sidecars first, the main file last.
        assert_eq!(
            tos,
            [
                format!("g9.sqlite-wal.{STAMP}.quarantine"),
                format!("g9.sqlite-shm.{STAMP}.quarantine"),
                format!("g9.sqlite-journal.{STAMP}.quarantine"),
                format!("g9.sqlite.{STAMP}.quarantine"),
            ]
        );
        // One occupied name moves the whole set to the next suffix.
        fs::write(q.join(format!("g9.sqlite-shm.{STAMP}.quarantine")), b"x").unwrap();
        let plan = plan_quarantine(&live, &q, now()).unwrap();
        assert!(plan.iter().all(|(_, to)| to
            .to_string_lossy()
            .ends_with(&format!("{STAMP}.1.quarantine"))));
        // Nothing to move: the directory is not created.
        let none = dir.path().join("none");
        let empty_live = dir.path().join("absent.sqlite");
        assert_eq!(
            plan_quarantine(&empty_live, &none, now()).unwrap(),
            Vec::new()
        );
        assert!(!none.exists());
    }
}
