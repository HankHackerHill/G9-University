//! Crate error type and its mapping to the closed `G9Error` category set.
//!
//! Messages never carry row content, prompts or secrets, and I/O errors are
//! reduced to their `ErrorKind`. SQLite's own text for a refused statement names
//! tables, columns and constraints: a variant that holds it keeps it as its
//! payload (for tests) and displays a fixed sentence, so the text never reaches a
//! caller through `G9Error` (app-6). Internal causes are logged locally through
//! `tracing` at the point of conversion (codes only) and redacted from the
//! returned error.

use g9_domain::state::TransitionError;
use g9_domain::{ErrorCategory, G9Error, Revision};
use rusqlite::ffi;

/// Result alias used across the crate.
pub type StorageResult<T> = Result<T, StorageError>;

/// Prefix used by every immutability trigger in the migrations (`RAISE(ABORT, 'IMMUTABLE: ...')`).
pub const IMMUTABLE_PREFIX: &str = "IMMUTABLE:";

/// Stable refusal codes of [`crate::restore::restore_over_live`]. Each maps to
/// one closed error category; the `restore.*` code is what the operator and the
/// command-line driver see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreCode {
    /// The snapshot's SHA-256 differs from the expected value (or the file
    /// changed while it was being copied).
    ChecksumMismatch,
    /// The snapshot's schema version is newer than the newest migration this
    /// build knows.
    SchemaTooNew,
    /// Another connection holds or uses the live database; nothing was changed.
    DatabaseInUse,
    /// The snapshot path is unusable (missing, not a regular file, not a
    /// self-contained snapshot) or the expected checksum is malformed.
    SnapshotInvalid,
    /// The snapshot and the live database are the same file.
    SnapshotIsLive,
    /// The live database path is unusable (missing parent directory, a directory).
    LivePathInvalid,
    /// Moving the files failed; the live database was put back where possible.
    SwapFailed,
}

impl RestoreCode {
    /// Stable machine-readable code (`restore.<name>`).
    pub const fn code(self) -> &'static str {
        match self {
            RestoreCode::ChecksumMismatch => "restore.checksum_mismatch",
            RestoreCode::SchemaTooNew => "restore.schema_too_new",
            RestoreCode::DatabaseInUse => "restore.database_in_use",
            RestoreCode::SnapshotInvalid => "restore.snapshot_invalid",
            RestoreCode::SnapshotIsLive => "restore.snapshot_is_live",
            RestoreCode::LivePathInvalid => "restore.live_path_invalid",
            RestoreCode::SwapFailed => "restore.swap_failed",
        }
    }

    /// Closed error category.
    pub const fn category(self) -> ErrorCategory {
        match self {
            RestoreCode::ChecksumMismatch => ErrorCategory::DataCorruption,
            RestoreCode::SchemaTooNew | RestoreCode::DatabaseInUse => {
                ErrorCategory::PreconditionFailed
            }
            RestoreCode::SnapshotInvalid
            | RestoreCode::SnapshotIsLive
            | RestoreCode::LivePathInvalid => ErrorCategory::InvalidInput,
            RestoreCode::SwapFailed => ErrorCategory::ResourceUnavailable,
        }
    }
}

/// Errors raised by the storage layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// An immutability trigger refused an UPDATE/DELETE (A04 at storage level).
    /// The payload (the trigger's text after `IMMUTABLE:`, or the repository's
    /// reason) names the table; it is not displayed (app-6).
    #[error("the record is final and cannot be changed; a correction is a new record")]
    Immutable(String),
    /// A foreign-key constraint refused the statement (RESTRICT deletion semantics).
    /// The payload is SQLite's text or the repository's reason (tables and
    /// columns); it is not displayed (app-6).
    #[error("a record this one refers to is missing, or a record still refers to it")]
    ForeignKey(String),
    /// A CHECK constraint refused the statement (enum/range/json mirror of g9-domain). The payload
    /// is SQLite's own text (table, column, expression): it is for logs and tests, and is not part
    /// of the displayed message that reaches a caller (security audit 2026-11-12).
    #[error("a value is outside what storage accepts for this record")]
    CheckViolation(String),
    /// A UNIQUE/PRIMARY KEY constraint refused the statement. The payload is
    /// SQLite's text (table and columns); it is not displayed (app-6).
    #[error("a record with the same identity already exists")]
    UniqueViolation(String),
    /// A trigger refused the statement with its own reason (`RAISE(ABORT, ...)`
    /// without the `IMMUTABLE:` prefix). The payload is the trigger's text, which
    /// names tables and columns; it is not displayed (app-6). Same code and
    /// category as [`StorageError::Precondition`].
    #[error("a storage rule refused the change")]
    TriggerRefused(String),
    /// The database is busy or locked (bounded writers; retry). The payload is
    /// SQLite's text or the caller's reason; it is not displayed (app-6).
    #[error("the database is busy; retry the command")]
    Busy(String),
    /// The disk is full (`SQLITE_FULL`); the command may be retried after space is freed.
    #[error("disk full")]
    DiskFull,
    /// A backup refused to start: the backup directory's filesystem has less
    /// free space than the threshold (`backup::check_free_space`); nothing was
    /// written, and the command may be retried after space is freed.
    #[error("not enough free space to start a backup: {available_bytes} bytes available, {required_bytes} required")]
    InsufficientSpace {
        /// Threshold: `BACKUP_FREE_SPACE_FACTOR` × the database size.
        required_bytes: u64,
        /// Free space reported for the backup directory's filesystem.
        available_bytes: u64,
    },
    /// The file is not a readable SQLite database, `integrity_check` failed, or
    /// stored bytes do not match their digest. The payload (SQLite's text, which
    /// can quote schema names, or the repository's reason) is not displayed (app-6).
    #[error("stored data failed an integrity check")]
    Corrupt(String),
    /// An applied migration's recorded checksum does not match the embedded SQL.
    #[error("migration {version} checksum mismatch (recorded {recorded}, embedded {embedded})")]
    ChecksumMismatch {
        /// Migration version number.
        version: u32,
        /// Checksum stored in `schema_migrations`.
        recorded: String,
        /// Checksum of the SQL embedded in this build.
        embedded: String,
    },
    /// The database was migrated by a newer application; forward-only migration rule (C12).
    #[error("database schema version {db_version} is newer than this build supports ({app_max})")]
    SchemaNewerThanApp {
        /// Highest version recorded in `schema_migrations`.
        db_version: u32,
        /// Highest version embedded in this build.
        app_max: u32,
    },
    /// A PRAGMA did not take the required value after `open`.
    #[error("pragma {pragma} = {actual}, expected {expected}")]
    PragmaNotApplied {
        /// Pragma name.
        pragma: &'static str,
        /// Expected value.
        expected: String,
        /// Observed value.
        actual: String,
    },
    /// The requested row does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// Optimistic concurrency failure.
    #[error("stale revision: expected {expected:?}, actual {actual:?}")]
    StaleRevision {
        /// Revision the caller expected.
        expected: Option<Revision>,
        /// Revision currently stored.
        actual: Revision,
    },
    /// A state-machine edge is not allowed.
    #[error("invalid transition: {0}")]
    InvalidTransition(TransitionError),
    /// The caller supplied an invalid value.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The operation conflicts with existing state.
    #[error("conflict: {0}")]
    Conflict(String),
    /// A precondition of the operation does not hold.
    #[error("precondition failed: {0}")]
    Precondition(String),
    /// Filesystem failure; only the error kind is reported.
    #[error("i/o failure during {context}: {kind:?}")]
    Io {
        /// What the crate was doing.
        context: &'static str,
        /// `std::io::ErrorKind` of the failure.
        kind: std::io::ErrorKind,
    },
    /// Behaviour that is specified but intentionally not implemented in this stage.
    #[error("{0} is not implemented in this stage")]
    NotImplemented(&'static str),
    /// Backup verification found problems; the list is metadata only.
    #[error("backup verification failed: {}", .0.join("; "))]
    Verification(Vec<String>),
    /// Stored JSON/enum text could not be decoded into the expected type. The
    /// payload (the column, and sometimes the stored value) is kept for tests
    /// and never displayed: messages never carry row content.
    #[error("a stored record could not be read; its data does not have the expected form")]
    RowDecode(String),
    /// An offline restore was refused (`restore.*` code); the message is metadata only.
    #[error("restore refused: {message}")]
    Restore {
        /// Stable refusal code.
        code: RestoreCode,
        /// Human-readable reason without paths or content.
        message: String,
    },
    /// Any other failure; the cause was logged locally and is redacted here.
    #[error("internal storage error ({0})")]
    Internal(&'static str),
}

impl StorageError {
    /// Stable machine-readable code (`storage.<name>`).
    pub fn code(&self) -> &'static str {
        match self {
            StorageError::Immutable(_) => "storage.immutable",
            StorageError::ForeignKey(_) => "storage.foreign_key",
            StorageError::CheckViolation(_) => "storage.check_violation",
            StorageError::UniqueViolation(_) => "storage.unique_violation",
            StorageError::TriggerRefused(_) => "storage.precondition_failed",
            StorageError::Busy(_) => "storage.busy",
            StorageError::DiskFull => "storage.disk_full",
            StorageError::InsufficientSpace { .. } => "backup.insufficient_space",
            StorageError::Corrupt(_) => "storage.corrupt",
            StorageError::ChecksumMismatch { .. } => "storage.migration_checksum_mismatch",
            StorageError::SchemaNewerThanApp { .. } => "storage.schema_newer_than_app",
            StorageError::PragmaNotApplied { .. } => "storage.pragma_not_applied",
            StorageError::NotFound(_) => "storage.not_found",
            StorageError::StaleRevision { .. } => "revision.stale",
            StorageError::InvalidTransition(_) => "storage.invalid_transition",
            StorageError::InvalidInput(_) => "storage.invalid_input",
            StorageError::Conflict(_) => "storage.conflict",
            StorageError::Precondition(_) => "storage.precondition_failed",
            StorageError::Io { .. } => "storage.io",
            StorageError::NotImplemented(_) => "storage.not_implemented",
            StorageError::Verification(_) => "storage.backup_verification_failed",
            StorageError::RowDecode(_) => "storage.row_decode",
            StorageError::Restore { code, .. } => code.code(),
            StorageError::Internal(_) => "storage.internal",
        }
    }

    /// Closed error category this error maps to.
    pub fn category(&self) -> ErrorCategory {
        match self {
            StorageError::Immutable(_)
            | StorageError::ForeignKey(_)
            | StorageError::SchemaNewerThanApp { .. }
            | StorageError::PragmaNotApplied { .. }
            | StorageError::InvalidTransition(_)
            | StorageError::Precondition(_)
            | StorageError::TriggerRefused(_)
            | StorageError::NotImplemented(_) => ErrorCategory::PreconditionFailed,
            StorageError::CheckViolation(_) | StorageError::InvalidInput(_) => {
                ErrorCategory::InvalidInput
            }
            StorageError::UniqueViolation(_) | StorageError::Conflict(_) => ErrorCategory::Conflict,
            StorageError::Busy(_)
            | StorageError::DiskFull
            | StorageError::InsufficientSpace { .. } => ErrorCategory::RetryableStorage,
            StorageError::Corrupt(_)
            | StorageError::ChecksumMismatch { .. }
            | StorageError::Verification(_)
            | StorageError::RowDecode(_) => ErrorCategory::DataCorruption,
            StorageError::NotFound(_) => ErrorCategory::NotFound,
            StorageError::StaleRevision { .. } => ErrorCategory::StaleRevision,
            StorageError::Io { .. } => ErrorCategory::ResourceUnavailable,
            StorageError::Restore { code, .. } => code.category(),
            StorageError::Internal(_) => ErrorCategory::Internal,
        }
    }

    /// A restore refusal with a stable code.
    pub fn restore(code: RestoreCode, message: impl Into<String>) -> Self {
        StorageError::Restore {
            code,
            message: message.into(),
        }
    }

    /// Build an I/O error from a `std::io::Error`, keeping only its kind.
    pub fn io(context: &'static str, err: &std::io::Error) -> Self {
        tracing::warn!(context, kind = ?err.kind(), "storage i/o failure");
        StorageError::Io {
            context,
            kind: err.kind(),
        }
    }
}

impl From<StorageError> for G9Error {
    fn from(e: StorageError) -> Self {
        let category = e.category();
        let code = e.code();
        match e {
            StorageError::StaleRevision { expected, actual } => {
                G9Error::stale_revision(expected, actual)
            }
            StorageError::Internal(_) => G9Error::internal_redacted(code),
            StorageError::NotImplemented(area) => {
                G9Error::precondition_failed(format!("{area}.not_implemented"), e.to_string())
            }
            StorageError::ChecksumMismatch { version, .. } => {
                G9Error::new(category, code, e.to_string())
                    .with_details(serde_json::json!({ "version": version }))
            }
            StorageError::InsufficientSpace {
                required_bytes,
                available_bytes,
            } => G9Error::new(category, code, e.to_string()).with_details(serde_json::json!({
                "required_bytes": required_bytes,
                "available_bytes": available_bytes,
            })),
            StorageError::Verification(ref errors) => {
                G9Error::new(category, code, "backup verification failed")
                    .with_details(serde_json::json!({ "errors": errors }))
            }
            other => G9Error::new(category, code, other.to_string()),
        }
    }
}

impl From<TransitionError> for StorageError {
    fn from(e: TransitionError) -> Self {
        StorageError::InvalidTransition(e)
    }
}

/// The variant of a `rusqlite::Error`, as a word. Its `Display` can quote a
/// column name, a path or part of a statement, so only the name is logged.
fn rusqlite_variant(e: &rusqlite::Error) -> &'static str {
    match e {
        rusqlite::Error::SqliteFailure { .. } => "SqliteFailure",
        rusqlite::Error::SqliteSingleThreadedMode => "SqliteSingleThreadedMode",
        rusqlite::Error::Utf8Error { .. } => "Utf8Error",
        rusqlite::Error::NulError { .. } => "NulError",
        rusqlite::Error::InvalidParameterName { .. } => "InvalidParameterName",
        rusqlite::Error::InvalidPath { .. } => "InvalidPath",
        rusqlite::Error::ExecuteReturnedResults => "ExecuteReturnedResults",
        rusqlite::Error::InvalidColumnIndex { .. } => "InvalidColumnIndex",
        rusqlite::Error::InvalidColumnName { .. } => "InvalidColumnName",
        rusqlite::Error::StatementChangedRows { .. } => "StatementChangedRows",
        rusqlite::Error::ToSqlConversionFailure { .. } => "ToSqlConversionFailure",
        rusqlite::Error::InvalidQuery => "InvalidQuery",
        rusqlite::Error::MultipleStatement => "MultipleStatement",
        rusqlite::Error::InvalidParameterCount { .. } => "InvalidParameterCount",
        _ => "other",
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::SqliteFailure(
                ffi::Error {
                    code,
                    extended_code,
                },
                msg,
            ) => {
                let message = msg.unwrap_or_default();
                match code {
                    ffi::ErrorCode::ConstraintViolation => {
                        if message.starts_with(IMMUTABLE_PREFIX) {
                            StorageError::Immutable(
                                message
                                    .trim_start_matches(IMMUTABLE_PREFIX)
                                    .trim()
                                    .to_string(),
                            )
                        } else if message.contains("FOREIGN KEY constraint failed") {
                            // `ON DELETE RESTRICT` is enforced through an internal RAISE(ABORT) and therefore
                            // surfaces as SQLITE_CONSTRAINT_TRIGGER, not SQLITE_CONSTRAINT_FOREIGNKEY.
                            StorageError::ForeignKey(message)
                        } else {
                            match extended_code {
                                ffi::SQLITE_CONSTRAINT_FOREIGNKEY => {
                                    StorageError::ForeignKey(message)
                                }
                                ffi::SQLITE_CONSTRAINT_CHECK
                                | ffi::SQLITE_CONSTRAINT_NOTNULL
                                | ffi::SQLITE_CONSTRAINT_DATATYPE => {
                                    StorageError::CheckViolation(message)
                                }
                                ffi::SQLITE_CONSTRAINT_UNIQUE
                                | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                                    StorageError::UniqueViolation(message)
                                }
                                ffi::SQLITE_CONSTRAINT_TRIGGER => {
                                    StorageError::TriggerRefused(message)
                                }
                                _ => StorageError::CheckViolation(message),
                            }
                        }
                    }
                    ffi::ErrorCode::DatabaseBusy | ffi::ErrorCode::DatabaseLocked => {
                        StorageError::Busy(message)
                    }
                    ffi::ErrorCode::DiskFull => StorageError::DiskFull,
                    ffi::ErrorCode::DatabaseCorrupt | ffi::ErrorCode::NotADatabase => {
                        StorageError::Corrupt(message)
                    }
                    ffi::ErrorCode::CannotOpen
                    | ffi::ErrorCode::SystemIoFailure
                    | ffi::ErrorCode::ReadOnly => {
                        tracing::warn!(sqlite = ?code, extended_code, "sqlite i/o failure");
                        StorageError::Io {
                            context: "sqlite",
                            kind: std::io::ErrorKind::Other,
                        }
                    }
                    _ => {
                        // The text SQLite gives for a failure can quote what a statement held:
                        // the codes are logged, the text is not (`g9_app::oplog`).
                        tracing::error!(sqlite = ?code, extended_code, "unmapped sqlite failure");
                        StorageError::Internal("sqlite")
                    }
                }
            }
            rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound("row"),
            rusqlite::Error::InvalidColumnType(_, ref name, _) => {
                StorageError::RowDecode(format!("column {name}"))
            }
            rusqlite::Error::FromSqlConversionFailure(_, _, _)
            | rusqlite::Error::IntegralValueOutOfRange(_, _) => {
                StorageError::RowDecode("value out of range".to_string())
            }
            rusqlite::Error::SqlInputError { ref error, .. } => {
                tracing::error!(
                    sqlite = ?error.code,
                    extended_code = error.extended_code,
                    "sql input error"
                );
                StorageError::Internal("sql")
            }
            other => {
                tracing::error!(kind = rusqlite_variant(&other), "unmapped rusqlite error");
                StorageError::Internal("rusqlite")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_maps_to_a_closed_category_and_stable_code() {
        let samples = vec![
            StorageError::Immutable("x".into()),
            StorageError::ForeignKey("x".into()),
            StorageError::CheckViolation("x".into()),
            StorageError::UniqueViolation("x".into()),
            StorageError::TriggerRefused("x".into()),
            StorageError::Busy("x".into()),
            StorageError::DiskFull,
            StorageError::Corrupt("x".into()),
            StorageError::ChecksumMismatch {
                version: 1,
                recorded: "a".into(),
                embedded: "b".into(),
            },
            StorageError::SchemaNewerThanApp {
                db_version: 10,
                app_max: 9,
            },
            StorageError::NotFound("row"),
            StorageError::StaleRevision {
                expected: Some(Revision::new(1)),
                actual: Revision::new(2),
            },
            StorageError::InvalidInput("x".into()),
            StorageError::Conflict("x".into()),
            StorageError::Precondition("x".into()),
            StorageError::Io {
                context: "t",
                kind: std::io::ErrorKind::Other,
            },
            StorageError::NotImplemented("deletion.study_session"),
            StorageError::Verification(vec!["integrity".into()]),
            StorageError::RowDecode("c".into()),
            StorageError::Restore {
                code: RestoreCode::DatabaseInUse,
                message: "x".into(),
            },
            StorageError::Internal("x"),
        ];
        for e in samples {
            let cat = e.category();
            assert!(ErrorCategory::ALL.contains(&cat), "{e:?}");
            let g: G9Error = e.clone().into();
            assert_eq!(g.category, cat, "{e:?}");
            assert!(g.code.contains('.'), "{e:?}");
            assert!(!g.message.contains("backtrace"));
        }
    }

    #[test]
    fn every_restore_code_has_a_stable_restore_prefixed_code_and_closed_category() {
        let all = [
            (
                RestoreCode::ChecksumMismatch,
                "restore.checksum_mismatch",
                ErrorCategory::DataCorruption,
            ),
            (
                RestoreCode::SchemaTooNew,
                "restore.schema_too_new",
                ErrorCategory::PreconditionFailed,
            ),
            (
                RestoreCode::DatabaseInUse,
                "restore.database_in_use",
                ErrorCategory::PreconditionFailed,
            ),
            (
                RestoreCode::SnapshotInvalid,
                "restore.snapshot_invalid",
                ErrorCategory::InvalidInput,
            ),
            (
                RestoreCode::SnapshotIsLive,
                "restore.snapshot_is_live",
                ErrorCategory::InvalidInput,
            ),
            (
                RestoreCode::LivePathInvalid,
                "restore.live_path_invalid",
                ErrorCategory::InvalidInput,
            ),
            (
                RestoreCode::SwapFailed,
                "restore.swap_failed",
                ErrorCategory::ResourceUnavailable,
            ),
        ];
        for (code, text, category) in all {
            let e = StorageError::restore(code, "reason");
            assert_eq!(e.code(), text);
            assert_eq!(e.category(), category);
            let g: G9Error = e.into();
            assert_eq!(g.code, text);
            assert_eq!(g.category, category);
        }
    }

    /// app-6: what SQLite says about a refused statement names tables, columns
    /// and constraints. Each refusal keeps that text as its payload, and the
    /// error a caller receives carries a fixed sentence and the stable code.
    #[test]
    fn a_row_that_cannot_be_decoded_is_reported_without_its_column_or_value() {
        // Final security review (privacy hardening): the message quoted the
        // column name and, through serde and enum errors, the stored value.
        let e = StorageError::RowDecode("column title: unexpected enum text \"SECRET-ROW\"".into());
        let g: G9Error = e.into();
        assert_eq!(g.code, "storage.row_decode");
        assert_eq!(g.category, ErrorCategory::DataCorruption);
        assert!(!g.message.contains("SECRET-ROW"), "{}", g.message);
        assert!(!g.message.contains("title"), "{}", g.message);
    }

    #[test]
    fn a_refused_statement_reaches_the_caller_without_schema_names() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE parent_rows (id TEXT PRIMARY KEY, ref_label TEXT NOT NULL UNIQUE);
             CREATE TABLE child_rows (id TEXT PRIMARY KEY,
                 parent_ref TEXT NOT NULL REFERENCES parent_rows (id),
                 grade_pct INTEGER NOT NULL CHECK (grade_pct BETWEEN 0 AND 100));
             CREATE TABLE ledger_rows (id TEXT PRIMARY KEY, ledger_amount INTEGER NOT NULL);
             CREATE TRIGGER trg_ledger_rows_no_update BEFORE UPDATE ON ledger_rows
             BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: ledger_rows rows cannot be updated'); END;
             CREATE TRIGGER trg_ledger_rows_amount BEFORE INSERT ON ledger_rows WHEN NEW.ledger_amount < 0
             BEGIN SELECT RAISE(ABORT, 'ledger_rows.ledger_amount and parent_rows.ref_label are written together'); END;
             INSERT INTO parent_rows VALUES ('p1', 'C1');
             INSERT INTO ledger_rows VALUES ('l1', 5);",
        )
        .unwrap();
        let refusals = [
            (
                "INSERT INTO parent_rows VALUES ('p2', 'C1')",
                "storage.unique_violation",
            ),
            (
                "INSERT INTO child_rows VALUES ('c1', 'missing', 1)",
                "storage.foreign_key",
            ),
            (
                "INSERT INTO child_rows VALUES ('c2', 'p1', 500)",
                "storage.check_violation",
            ),
            (
                "UPDATE ledger_rows SET ledger_amount = 6",
                "storage.immutable",
            ),
            (
                "INSERT INTO ledger_rows VALUES ('l2', -1)",
                "storage.precondition_failed",
            ),
        ];
        let schema = [
            "parent_rows",
            "child_rows",
            "ledger_rows",
            "ref_label",
            "parent_ref",
            "grade_pct",
            "ledger_amount",
        ];
        for (sql, code) in refusals {
            let raw = conn.execute(sql, []).unwrap_err();
            let raw_text = raw.to_string();
            let e = StorageError::from(raw);
            assert_eq!(e.code(), code, "{sql}: {e:?}");
            // The payload keeps what SQLite said (tests and local diagnosis).
            assert!(
                format!("{e:?}").contains(raw_text.trim_start_matches("IMMUTABLE:").trim()),
                "{sql}: {e:?}"
            );
            let g: G9Error = e.into();
            assert_eq!(g.code, code);
            let wire = serde_json::to_string(&g).unwrap();
            for name in schema {
                assert!(!wire.contains(name), "{sql}: {name} in {wire}");
            }
            assert!(!wire.to_lowercase().contains("constraint"), "{sql}: {wire}");
            assert!(!wire.contains("UNIQUE"), "{sql}: {wire}");
        }
        // Busy and corrupt: SQLite's text is not repeated either.
        for (code, text, expected) in [
            (
                ffi::ErrorCode::DatabaseBusy,
                "database is locked: parent_rows",
                "storage.busy",
            ),
            (
                ffi::ErrorCode::DatabaseCorrupt,
                "malformed database schema (parent_rows) - near \"ref_label\"",
                "storage.corrupt",
            ),
        ] {
            let raw = rusqlite::Error::SqliteFailure(
                ffi::Error {
                    code,
                    extended_code: 0,
                },
                Some(text.to_string()),
            );
            let g: G9Error = StorageError::from(raw).into();
            assert_eq!(g.code, expected);
            let wire = serde_json::to_string(&g).unwrap();
            assert!(!wire.contains("parent_rows"), "{wire}");
            assert!(
                !wire.contains("ref_label") && !wire.contains("near"),
                "{wire}"
            );
        }
    }

    #[test]
    fn restrict_action_is_mapped_to_foreign_key() {
        let err = rusqlite::Error::SqliteFailure(
            ffi::Error {
                code: ffi::ErrorCode::ConstraintViolation,
                extended_code: ffi::SQLITE_CONSTRAINT_TRIGGER,
            },
            Some("FOREIGN KEY constraint failed".to_string()),
        );
        let e: StorageError = err.into();
        assert!(matches!(e, StorageError::ForeignKey(_)), "{e:?}");
        assert_eq!(e.category(), ErrorCategory::PreconditionFailed);
    }

    #[test]
    fn immutable_trigger_message_is_recognised() {
        let err = rusqlite::Error::SqliteFailure(
            ffi::Error {
                code: ffi::ErrorCode::ConstraintViolation,
                extended_code: ffi::SQLITE_CONSTRAINT_TRIGGER,
            },
            Some("IMMUTABLE: audit_metadata rows cannot be updated".to_string()),
        );
        let e: StorageError = err.into();
        assert_eq!(
            e,
            StorageError::Immutable("audit_metadata rows cannot be updated".to_string())
        );
        assert_eq!(e.category(), ErrorCategory::PreconditionFailed);
    }
}
