//! Crate error type and its mapping to the closed `G9Error` category set.
//!
//! Messages never carry row content, prompts or secrets: SQLite constraint
//! messages name tables/columns only, and I/O errors are reduced to their
//! `ErrorKind`. Internal causes are logged locally through `tracing` at the
//! point of conversion and redacted from the returned error.

use g9_domain::state::TransitionError;
use g9_domain::{ErrorCategory, G9Error, Revision};
use rusqlite::ffi;

/// Result alias used across the crate.
pub type StorageResult<T> = Result<T, StorageError>;

/// Prefix used by every immutability trigger in the migrations (`RAISE(ABORT, 'IMMUTABLE: ...')`).
pub const IMMUTABLE_PREFIX: &str = "IMMUTABLE:";

/// Errors raised by the storage layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// An immutability trigger refused an UPDATE/DELETE (A04 at storage level).
    #[error("immutable record: {0}")]
    Immutable(String),
    /// A foreign-key constraint refused the statement (RESTRICT deletion semantics).
    #[error("foreign key constraint failed: {0}")]
    ForeignKey(String),
    /// A CHECK constraint refused the statement (enum/range/json mirror of g9-domain).
    #[error("check constraint failed: {0}")]
    CheckViolation(String),
    /// A UNIQUE/PRIMARY KEY constraint refused the statement.
    #[error("unique constraint failed: {0}")]
    UniqueViolation(String),
    /// The database is busy or locked (bounded writers; retry).
    #[error("database busy: {0}")]
    Busy(String),
    /// The disk is full (`SQLITE_FULL`); the command may be retried after space is freed.
    #[error("disk full")]
    DiskFull,
    /// The file is not a readable SQLite database or `integrity_check` failed.
    #[error("database corrupt: {0}")]
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
    /// Stored JSON/enum text could not be decoded into the expected type.
    #[error("stored row could not be decoded: {0}")]
    RowDecode(String),
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
            StorageError::Busy(_) => "storage.busy",
            StorageError::DiskFull => "storage.disk_full",
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
            | StorageError::NotImplemented(_) => ErrorCategory::PreconditionFailed,
            StorageError::CheckViolation(_) | StorageError::InvalidInput(_) => {
                ErrorCategory::InvalidInput
            }
            StorageError::UniqueViolation(_) | StorageError::Conflict(_) => ErrorCategory::Conflict,
            StorageError::Busy(_) | StorageError::DiskFull => ErrorCategory::RetryableStorage,
            StorageError::Corrupt(_)
            | StorageError::ChecksumMismatch { .. }
            | StorageError::Verification(_)
            | StorageError::RowDecode(_) => ErrorCategory::DataCorruption,
            StorageError::NotFound(_) => ErrorCategory::NotFound,
            StorageError::StaleRevision { .. } => ErrorCategory::StaleRevision,
            StorageError::Io { .. } => ErrorCategory::ResourceUnavailable,
            StorageError::Internal(_) => ErrorCategory::Internal,
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
                                    StorageError::Precondition(message)
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
                        tracing::warn!(?code, extended_code, "sqlite i/o failure");
                        StorageError::Io {
                            context: "sqlite",
                            kind: std::io::ErrorKind::Other,
                        }
                    }
                    _ => {
                        tracing::error!(?code, extended_code, %message, "unmapped sqlite failure");
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
            rusqlite::Error::SqlInputError { ref msg, .. } => {
                tracing::error!(%msg, "sql input error");
                StorageError::Internal("sql")
            }
            other => {
                tracing::error!(error = %other, "unmapped rusqlite error");
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
