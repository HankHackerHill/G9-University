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

## Addendum (2026-11-13): the schema itself is verified

Security audit 2026-11-12 (MEDIUM, CONFIRMED by two independent auditors and a CLI reproduction): verification compared the recorded migration checksums, which are rows inside the file, but never the schema. A copy of a genuine database with an immutability trigger dropped and a trigger planted (for example one that deletes every new audit row) passed `verify_snapshot`, `restore_over_live` and open-time recovery, and its triggers then ran inside every command.

Corrected:

| Check | Where |
|---|---|
| `migrations::verify_schema`: every table, index, trigger and view in `sqlite_master` must equal what the embedded migrations produce at the recorded version (built in memory with the same migrator; SQL text compared exactly, never with whitespace collapsed: a newline ends a `--` comment, so a collapsed comparison let a moved newline stretch a comment over a CHECK constraint, found while verifying this fix; only the `sqlite_stat1`–`sqlite_stat4` planner statistics are exempt, by exact name and type, so a `sqlite_`-named trigger written through `writable_schema` is still caught, and constraint autoindexes and `sqlite_sequence` are compared) | `crates/g9-storage/src/migrations.rs` |
| Backup verification and restore (`verify_connection`, so `verify_snapshot` and `restore_over_live`'s copy check) fail on any difference; `VerificationReport.schema_ok` | `crates/g9-storage/src/backup.rs`, `restore.rs` |
| Open-time recovery checks the schema before it writes anything; a difference blocks every mutation (`write_block_reason = schema_drift`, `RecoveryReport.schema_differences`), as a broken audit chain does | `crates/g9-app/src/recovery.rs` |
| The restore plan no longer prints the snapshot's actual SHA-256 next to the confirmation command when it does not match the expected one | `apps/cli/src/commands/restore.rs` |
| `table_counts` quotes table names read from a foreign file | `crates/g9-storage/src/migrations.rs` |
| A pinned SHA-256 of the expected schema at version 13: a change of stored text (an edit to `BOOTSTRAP_SQL`, a SQLite upgrade that stores CREATE or ADD COLUMN text differently) would put every existing database into `schema_drift`, so it fails CI instead (`the_schema_existing_databases_are_verified_against_is_pinned`; reproduced with SQLite 3.53.1 against the bundled 3.54.1) | `crates/g9-storage/src/migrations.rs` (tests) |

Residual: a snapshot with the correct schema and fabricated rows still verifies; backups carry no keyed MAC (IMP-M08 PROPOSED) and the audit chain is unkeyed (ADR-0004).

Verification: `crates/g9-storage/tests/schema_integrity.rs` (7 tests; the 5 tamper cases failed before the change), `crates/g9-app/tests/schema_drift.rs` (2 tests; the tamper case failed before), and the full workspace suite with no false positive on any existing database.
