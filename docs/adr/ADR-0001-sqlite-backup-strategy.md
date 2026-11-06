# ADR-0001: SQLite backup: Online Backup API or VACUUM INTO, verified on a copy, independent of WAL

**Status:** Accepted (2026-11-06)

## Context

v5.1 F2 claimed the Online Backup API requires WAL and proposed a shell `.dump` recipe (register C04, C05, TEC-09, TEC-10, SEC-29). The SQLite documentation (Appendix C #5) states the backup API works on rollback-journal and WAL databases alike; `.dump` emits SQL text, needs a shell capability the runtime must not have, and blocks writers.

## Decision

Backups are produced only by the trusted backend (`crates/g9-storage::backup`) with `rusqlite::backup::Backup::run_to_completion` or `VACUUM INTO`, written to a temporary file, then verified on the copy (`PRAGMA integrity_check`, `PRAGMA foreign_key_check`, migration checksums, invariant counts, attachment manifest) and restore-tested before the manifest reaches `RETAINED`. WAL is a chosen concurrency profile for file databases and is not a precondition. Live-file copies and `.dump` are rejected by design and by review.

## Alternatives considered

(a) Shell `.dump` — rejected: text output, lock contention, needs shell capability. (b) Filesystem snapshot (btrfs/LVM) — rejected as the primary mechanism: host-specific and unverifiable from the app; acceptable as an additional host-level control. (c) SQLCipher-encrypted copies — complementary (ADR-0004).

## Consequences

Backups are consistent and verifiable; restore drills are automatable (gate A08). Cost: temporary disk equal to the database size during snapshot.

## Verification

`a08_snapshot_verify_restore_roundtrip`, `a08_crash_mid_transaction_rolls_back`, `a08_migration_checksum_tamper_detected`, runbook `docs/runbooks/backup-restore.md`.
