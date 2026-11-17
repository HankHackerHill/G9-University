//! Connection profile and the verified open sequence (ARCHITECTURE.md §6).
//!
//! Every connection opened here gets the fixed PRAGMA profile and is checked
//! afterwards: `PRAGMA foreign_keys` must read back `1`, `trusted_schema` must
//! read back `0`. A connection whose pragmas did not apply is refused rather
//! than used, because the deletion semantics and immutability rules in the DDL
//! depend on them.
//!
//! Migration on open follows [`MigrationPolicy`]: a fresh database is migrated
//! to the current schema; an existing database with pending migrations is opened
//! *without* migrating and reports [`Database::pending_migrations`], because the
//! runbook (`docs/runbooks/backup-restore.md` § Migration) requires a verified
//! backup and a dry run before the live file is migrated — that sequence is
//! driven by `g9-app`, which then calls [`Database::migrate_pending`].
//!
//! `synchronous` defaults to `NORMAL` under WAL as fixed by ARCHITECTURE.md §6.
//! Defect TEC-12 (register, status OPEN) argues for `FULL`; the option is exposed
//! as [`Synchronous::Full`] so that `g9-app` can choose it without a storage change.

use crate::error::{StorageError, StorageResult};
use crate::migrations::{MigrationReport, Migrator};
use g9_domain::UtcMillis;
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Durability profile for `PRAGMA synchronous`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Synchronous {
    /// `synchronous = NORMAL` (ARCHITECTURE §6 default under WAL): durable across
    /// application crashes; the last commits may be lost on power failure.
    Normal,
    /// `synchronous = FULL`: fsync on every commit (TEC-12 recommendation).
    Full,
}

impl Synchronous {
    const fn sqlite_value(self) -> i64 {
        match self {
            Synchronous::Normal => 1,
            Synchronous::Full => 2,
        }
    }
}

/// Options for [`Database::open`] / [`Database::open_in_memory`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenOptions {
    /// Application version recorded in `schema_migrations.app_version`.
    pub app_version: String,
    /// Instant recorded as `applied_at` for migrations applied during this open.
    /// `None` uses the wall clock (the only implicit clock read in this crate).
    pub applied_at: Option<UtcMillis>,
    /// `PRAGMA synchronous` profile.
    pub synchronous: Synchronous,
    /// `PRAGMA secure_delete = ON` so that erased content does not linger in free pages (SEC-09).
    pub secure_delete: bool,
    /// When to apply pending migrations during open. Checksums are verified and a
    /// newer schema is refused under every policy.
    pub migration: MigrationPolicy,
    /// `PRAGMA busy_timeout` in milliseconds.
    pub busy_timeout_ms: u32,
}

impl OpenOptions {
    /// Default profile for an application version.
    pub fn new(app_version: impl Into<String>) -> Self {
        Self {
            app_version: app_version.into(),
            applied_at: None,
            synchronous: Synchronous::Normal,
            secure_delete: true,
            migration: MigrationPolicy::FreshOnly,
            busy_timeout_ms: 5000,
        }
    }

    /// Fix the migration `applied_at` instant (tests and reproducible installs).
    pub fn applied_at(mut self, at: UtcMillis) -> Self {
        self.applied_at = Some(at);
        self
    }

    /// Choose the `synchronous` profile.
    pub fn synchronous(mut self, s: Synchronous) -> Self {
        self.synchronous = s;
        self
    }

    /// Never migrate on open (inspection of copies, read-only tooling).
    pub fn no_auto_migrate(mut self) -> Self {
        self.migration = MigrationPolicy::Never;
        self
    }

    /// Always apply pending migrations on open (tests; `g9-app` after a verified backup).
    pub fn auto_migrate(mut self) -> Self {
        self.migration = MigrationPolicy::Always;
        self
    }

    /// Choose the migration policy explicitly.
    pub fn migration(mut self, policy: MigrationPolicy) -> Self {
        self.migration = policy;
        self
    }
}

/// When [`Database::open`] applies pending migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MigrationPolicy {
    /// Migrate only a database with no applied migrations (fresh install). An
    /// existing database with pending migrations opens unmigrated and reports them.
    FreshOnly,
    /// Apply every pending migration on open.
    Always,
    /// Never migrate on open.
    Never,
}

/// Observed PRAGMA values after open; returned for diagnostics and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PragmaProfile {
    /// `PRAGMA foreign_keys` (must be `true`).
    pub foreign_keys: bool,
    /// `PRAGMA journal_mode` as reported (`wal` for file databases, `memory` in memory).
    pub journal_mode: String,
    /// `PRAGMA synchronous` numeric value (1 = NORMAL, 2 = FULL).
    pub synchronous: i64,
    /// `PRAGMA busy_timeout` in ms.
    pub busy_timeout_ms: i64,
    /// `PRAGMA temp_store` (2 = MEMORY).
    pub temp_store: i64,
    /// `PRAGMA trusted_schema` (must be `false`).
    pub trusted_schema: bool,
    /// `PRAGMA secure_delete` (1 = ON).
    pub secure_delete: i64,
}

/// WAL checkpoint result (`PRAGMA wal_checkpoint(TRUNCATE)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointReport {
    /// `0` when the checkpoint completed, `1` when it was blocked (busy).
    pub busy: i64,
    /// Number of frames in the WAL before the checkpoint (`-1` when not in WAL mode).
    pub log_frames: i64,
    /// Number of frames checkpointed (`-1` when not in WAL mode).
    pub checkpointed_frames: i64,
}

/// An open, profile-verified, migrated SQLite database.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
    path: Option<PathBuf>,
    profile: PragmaProfile,
    migrator: Migrator,
    migration_report: Option<MigrationReport>,
    pending: Vec<u32>,
}

impl Database {
    /// Open (or create) a file database, apply and verify the PRAGMA profile,
    /// verify applied migration checksums, refuse a schema newer than this build,
    /// and apply pending migrations according to `options.migration`.
    pub fn open(path: impl AsRef<Path>, options: OpenOptions) -> StorageResult<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let profile = apply_pragmas(&conn, true, &options)?;
        Self::finish_open(conn, Some(path), profile, &options)
    }

    /// Open a private in-memory database with the same profile (journal mode is
    /// `memory`; everything else applies).
    pub fn open_in_memory(options: OpenOptions) -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        let profile = apply_pragmas(&conn, false, &options)?;
        Self::finish_open(conn, None, profile, &options)
    }

    fn finish_open(
        mut conn: Connection,
        path: Option<PathBuf>,
        profile: PragmaProfile,
        options: &OpenOptions,
    ) -> StorageResult<Self> {
        let migrator = Migrator::new(
            options.app_version.clone(),
            options.applied_at.unwrap_or_else(UtcMillis::now),
        );
        Migrator::bootstrap(&conn)?;
        let checksums = migrator.ensure_checksums(&conn)?;
        let fresh = checksums.db_schema_version == 0;
        let apply = match options.migration {
            MigrationPolicy::Always => true,
            MigrationPolicy::FreshOnly => fresh,
            MigrationPolicy::Never => false,
        };
        let report = if apply {
            Some(migrator.migrate(&mut conn)?)
        } else {
            None
        };
        let pending = Migrator::pending(&conn)?;
        if !pending.is_empty() {
            tracing::warn!(
                ?pending,
                "database opened with pending migrations; back up, dry-run, then migrate"
            );
        }
        Ok(Self {
            conn,
            path,
            profile,
            migrator,
            migration_report: report,
            pending,
        })
    }

    /// Versions embedded in this build that are not yet applied to this database.
    pub fn pending_migrations(&self) -> &[u32] {
        &self.pending
    }

    /// Apply pending migrations now (after `g9-app` has taken and verified a backup
    /// and rehearsed them with [`Migrator::dry_run_on_copy`]).
    pub fn migrate_pending(&mut self) -> StorageResult<MigrationReport> {
        let report = self.migrator.migrate(&mut self.conn)?;
        self.pending = Migrator::pending(&self.conn)?;
        self.migration_report = Some(report.clone());
        Ok(report)
    }

    /// Borrow the connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Mutably borrow the connection (needed for `transaction_with_behavior`).
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Path of the database file; `None` for in-memory databases.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// PRAGMA values observed right after open.
    pub fn profile(&self) -> &PragmaProfile {
        &self.profile
    }

    /// Report of the last migration run on this handle (`None` when nothing was applied).
    pub fn migration_report(&self) -> Option<&MigrationReport> {
        self.migration_report.as_ref()
    }

    /// `BEGIN IMMEDIATE` transaction (command pipeline step 5).
    pub fn begin_immediate(&mut self) -> StorageResult<Transaction<'_>> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }

    /// Run `write` in one `BEGIN IMMEDIATE` transaction on this handle: committed
    /// when it returns `Ok`, rolled back otherwise. For writers outside the
    /// command pipeline whose rows span several repository calls (the CLI seed,
    /// storage-repos-10), so that a failure part-way leaves none of them.
    /// Fails if a transaction is already open on the handle.
    pub fn write_transaction<T, E: From<StorageError>>(
        &self,
        write: impl FnOnce(&Connection) -> Result<T, E>,
    ) -> Result<T, E> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(StorageError::from)?;
        let value = write(&tx)?;
        tx.commit().map_err(StorageError::from)?;
        Ok(value)
    }

    /// `PRAGMA wal_checkpoint(TRUNCATE)`; a no-op report for non-WAL databases.
    pub fn checkpoint_truncate(&self) -> StorageResult<CheckpointReport> {
        checkpoint_truncate(&self.conn)
    }

    /// Give up the wrapper and keep the raw connection.
    pub fn into_connection(self) -> Connection {
        self.conn
    }
}

/// Apply the fixed PRAGMA profile to `conn` and read it back.
///
/// `file_backed = false` skips `journal_mode = WAL` (in-memory databases report
/// `memory`). Used by [`Database`] and by the backup module for restored copies.
pub fn apply_pragmas(
    conn: &Connection,
    file_backed: bool,
    options: &OpenOptions,
) -> StorageResult<PragmaProfile> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "busy_timeout", i64::from(options.busy_timeout_ms))?;
    conn.pragma_update(None, "temp_store", 2_i64)?;
    conn.pragma_update(None, "trusted_schema", false)?;
    conn.pragma_update(None, "secure_delete", options.secure_delete)?;
    if file_backed {
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StorageError::PragmaNotApplied {
                pragma: "journal_mode",
                expected: "wal".to_string(),
                actual: mode,
            });
        }
    }
    conn.pragma_update(None, "synchronous", options.synchronous.sqlite_value())?;
    read_profile(conn, options)
}

fn read_profile(conn: &Connection, options: &OpenOptions) -> StorageResult<PragmaProfile> {
    let foreign_keys: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
    if foreign_keys != 1 {
        return Err(StorageError::PragmaNotApplied {
            pragma: "foreign_keys",
            expected: "1".to_string(),
            actual: foreign_keys.to_string(),
        });
    }
    let trusted_schema: i64 = conn.query_row("PRAGMA trusted_schema", [], |r| r.get(0))?;
    if trusted_schema != 0 {
        return Err(StorageError::PragmaNotApplied {
            pragma: "trusted_schema",
            expected: "0".to_string(),
            actual: trusted_schema.to_string(),
        });
    }
    let synchronous: i64 = conn.query_row("PRAGMA synchronous", [], |r| r.get(0))?;
    if synchronous != options.synchronous.sqlite_value() {
        return Err(StorageError::PragmaNotApplied {
            pragma: "synchronous",
            expected: options.synchronous.sqlite_value().to_string(),
            actual: synchronous.to_string(),
        });
    }
    let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
    let busy_timeout_ms: i64 = conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0))?;
    let temp_store: i64 = conn.query_row("PRAGMA temp_store", [], |r| r.get(0))?;
    let secure_delete: i64 = conn.query_row("PRAGMA secure_delete", [], |r| r.get(0))?;
    Ok(PragmaProfile {
        foreign_keys: true,
        journal_mode,
        synchronous,
        busy_timeout_ms,
        temp_store,
        trusted_schema: false,
        secure_delete,
    })
}

/// `PRAGMA wal_checkpoint(TRUNCATE)` on any connection.
pub fn checkpoint_truncate(conn: &Connection) -> StorageResult<CheckpointReport> {
    let (busy, log_frames, checkpointed_frames): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    Ok(CheckpointReport {
        busy,
        log_frames,
        checkpointed_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_profile_is_verified() {
        let db = Database::open_in_memory(
            OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)),
        )
        .unwrap();
        let p = db.profile();
        assert!(p.foreign_keys);
        assert!(!p.trusted_schema);
        assert_eq!(p.synchronous, 1);
        assert_eq!(p.busy_timeout_ms, 5000);
        assert_eq!(p.temp_store, 2);
        assert_eq!(p.secure_delete, 1);
        assert_eq!(p.journal_mode, "memory");
        assert!(db.migration_report().is_some());
    }

    #[test]
    fn full_synchronous_option_is_honoured() {
        let db = Database::open_in_memory(
            OpenOptions::new("test")
                .applied_at(UtcMillis::from_millis(1))
                .synchronous(Synchronous::Full),
        )
        .unwrap();
        assert_eq!(db.profile().synchronous, 2);
    }
}
