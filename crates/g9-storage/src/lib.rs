#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-storage
//!
//! SQLite persistence foundation for G9 MENTOR (M17 Recovery/Audit plus the
//! storage rules of ARCHITECTURE.md §6). This crate owns:
//!
//! * [`connection`] — the fixed per-connection PRAGMA profile (`foreign_keys = ON`,
//!   `busy_timeout = 5000`, `journal_mode = WAL` for file databases,
//!   `synchronous = NORMAL`, `temp_store = MEMORY`, `trusted_schema = OFF`,
//!   `secure_delete = ON`) and the verified open sequence.
//! * [`migrations`] — numbered SQL migrations embedded at compile time, recorded in
//!   `schema_migrations(version, name, sha256, applied_at, app_version)`, applied
//!   one transaction each, checksum-verified on every open (`DATA_CORRUPTION` on
//!   mismatch) and rehearsable on a `VACUUM INTO` copy before touching the live file.
//! * [`backup`] — Online Backup API snapshots to a temporary file, verification on
//!   the copy (`integrity_check`, `foreign_key_check`, migration checksums, audit
//!   chain, table counts, attachment manifest), restore into a disposable copy and
//!   the `BackupState` machine Requested → SnapshotCreated → Checked → RestoreTested
//!   → Retained. Live-file copies and `.dump` recipes are rejected by design (C04/C05).
//! * [`restore`] — offline restore of a verified snapshot over the live database
//!   file ([`restore::restore_over_live`]): checksum, verification of a copy next
//!   to the live file, exclusive access, quarantine of the replaced database and
//!   its sidecars (never deleted), atomic rename. Restoring into a running
//!   application and an in-app restore operation are PROPOSED.
//! * [`audit`] — the hash-chained, metadata-only `audit_metadata` ledger
//!   (`row_hash = sha256(prev_hash || canonical_json(row))`) with chain verification.
//! * [`idempotency`] — the `(actor_id, idempotency_key)` replay store used by the
//!   command pipeline (ARCHITECTURE §5 steps 2 and 8).
//! * [`events`] — append-only `domain_events` rows for significant facts.
//! * [`deletion`] — the A10 erasure workflow: a `deletion_requests` row, in-place
//!   redaction of personal content, tombstones carrying the pre-erasure hash and
//!   invalidation markers on derived rows.
//! * [`approvals_store`] — persistence of approval requests/decisions and policy
//!   decision records as generic rows; the authority decisions themselves live in
//!   `g9-policy` and are applied by `g9-app`.
//! * [`diagnostics`] — read-only tooling for the performance work: query plans,
//!   table sizes, row dumps for a fixture checksum, and (feature
//!   `statement-trace`) a capture of the SQL a connection runs.
//!
//! ## Boundaries
//!
//! This crate depends on `g9-domain` and the pure domain crates (for row ↔ type
//! mapping in [`repos`]). It never evaluates policy, never interprets model
//! output and never touches the network. [`repos`] holds the typed repositories
//! (one module per table group) that `g9-app` composes so that no SQL is written
//! outside this crate; the other modules are the storage primitives.
//!
//! ## Determinism
//!
//! No function reads the wall clock implicitly except [`OpenOptions`] when
//! `applied_at` is left unset (migration bookkeeping). Every other timestamp is
//! supplied by the caller as [`g9_domain::UtcMillis`], so tests use fixtures.
//!
//! ## Error mapping
//!
//! Every public function returns [`StorageError`], which maps onto the closed
//! `G9Error` category set (`impl From<StorageError> for G9Error`). SQLite messages
//! are passed through only when they cannot carry content (constraint names,
//! pragma names); internal causes are logged with `tracing` and redacted.

pub mod approvals_store;
pub mod attachment_store;
pub mod audit;
pub mod backup;
pub mod connection;
pub mod deletion;
pub mod describe;
pub mod diagnostics;
pub mod error;
pub mod events;
pub mod export;
pub mod idempotency;
pub mod migrations;
pub mod repos;
pub mod restore;
pub(crate) mod sql;

pub use audit::{AuditChain, AuditDecision, AuditEntry, AuditOutcome, AuditRecord, ChainReport};
pub use backup::{
    AttachmentCheck, BackupManifest, SnapshotRequest, StoredManifest, VerificationReport,
    VerifyOptions,
};
pub use connection::{Database, MigrationPolicy, OpenOptions, PragmaProfile, Synchronous};
pub use error::{RestoreCode, StorageError, StorageResult};
pub use migrations::{
    AppliedMigration, ChecksumReport, DryRunReport, Migration, MigrationReport, Migrator,
    CURRENT_SCHEMA_VERSION, EMBEDDED_MIGRATIONS,
};
pub use repos::{ReadScope, RepoError, RepoResult};
pub use restore::{restore_over_live, RestoreOptions, RestoreReport};
