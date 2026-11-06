#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! SQLite persistence (M17 + repositories): schema migrations with checksums, foreign keys, WAL profile, Online Backup API / VACUUM INTO snapshots, integrity verification, hash-chained audit metadata.
//!
//! STATUS: SCAFFOLD — module contracts are defined in ARCHITECTURE.md; implementation lands in the stage that owns this crate.
