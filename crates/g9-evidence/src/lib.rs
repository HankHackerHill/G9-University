#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-evidence
//!
//! Notes, note revisions, attachments (content-addressed), evidence items,
//! research source records and content classification (M05; research source
//! records for M15). Pure: no I/O, no clock reads, no randomness; every
//! instant and actor is supplied by the caller. Depends only on `g9-domain`.
//!
//! | Module | Contract |
//! |---|---|
//! | [`notes`] | `Note` with an append-only `NoteVersion` ledger; content handed back as `ContentRecord`; redaction keeps a hash tombstone |
//! | [`attachments`] | `ApprovedRoots` lexical path check; `MediaType` allow-list with quarantine; `content_address` (`ab/cd/<sha256>`); `AttachmentRef` |
//! | [`evidence`] | `EvidenceItem`; attempt/observation evidence requires operator or deterministic-system provenance |
//! | [`classification`] | `classify_text` — deterministic secret/instruction/script/URL labeling aid (not a boundary) |
//! | [`sources`] | `SourceRecord` over `SourceState`; trust derived from state; approval/supersede/revoke are operator-only |
//! | [`sanitize`] | `strip_control_chars`, `normalize_newlines`, `truncate_utf8_safe` |
//! | [`error`] | `EvidenceError` → `G9Error` (closed categories, no content in messages) |
//!
//! Integrity rules applied here (ARCHITECTURE.md §13, handoff §12):
//!
//! - imported, model and skill content is **data**: this crate labels it and
//!   records its provenance; it never grants anything. Authority is decided in
//!   `g9-policy` and applied by `g9-app`;
//! - note revisions are append-only; redaction (A10) leaves a tombstone with
//!   the content hash and is performed only by the operator or the system
//!   actor executing an approved deletion request;
//! - attachment paths are confined to approved roots lexically here and must
//!   be canonicalized by the storage layer before use; executables, archives
//!   and captures are refused (SEC-13);
//! - every record carries `DataDomain` and `Classification` (R-017);
//! - a research source is an `ApprovedReference` only in that state, reached
//!   only by an operator decision (R-016).
//!
//! Gate coverage in `tests/gates.rs`: `a07_*` (malicious note content stays
//! data), `a10_*` (redaction tombstones), `r016_*` (source approval),
//! `r017_*` (domain/classification on every record).

pub mod attachments;
pub mod classification;
pub mod error;
pub mod evidence;
pub mod notes;
pub mod sanitize;
pub mod sources;

pub use attachments::{
    content_address, ApprovedRoots, AttachmentRef, ContentAddress, MediaType, PathRejection,
    QuarantineReason, ResolvedPath, MAX_ATTACHMENT_BYTES, MAX_PATH_BYTES,
};
pub use classification::{classify_text, ContentClassification, ContentFlag, SecretPattern};
pub use error::EvidenceError;
pub use evidence::{EvidenceItem, EvidenceKind, MAX_SUMMARY_CHARS};
pub use notes::{ContentRecord, Note, NoteMeta, NoteVersion, RedactionTombstone, MAX_NOTE_BYTES};
pub use sanitize::{
    normalize_newlines, sanitize_for_display, strip_control_chars, truncate_utf8_safe, Sanitized,
};
pub use sources::{
    ResourceClass, SourceFields, SourceRecord, SourceTransitionRecord, MAX_FIELD_CHARS,
    MAX_RESOURCE_CHARS,
};
