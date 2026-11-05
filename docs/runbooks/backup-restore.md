# Runbook: backup, verification, restore, migration (M17)

Preconditions: the app is the only writer; backups are created by the trusted backend (`g9-storage::backup`), never by shell `.dump` or file copy (C04/C05).

## Create and verify a backup

1. `backup.create` → `BackupState::Requested → SnapshotCreated` via the SQLite Online Backup API (`Backup::run_to_completion`) into `<data>/backups/<backup_id>.sqlite` (temporary name until verified).
2. `backup.verify` on the **copy**: `PRAGMA integrity_check`, `PRAGMA foreign_key_check`, migration checksums match the embedded set, table counts and relationship invariants, attachment manifest consistency → `Checked`.
3. `backup.restore_test`: open the copy in a disposable location, run the application invariant suite → `RestoreTested` → `Retained`. The manifest row records app version, schema version, page count, sha256, deletion-ledger checkpoint and (when enabled) encryption key id.
4. Failure at any step → `Failed` with the report attached; the previous retained set is untouched.

Recovery-point objective (proposed): daily automatic request after the first session of the day plus before every migration; retention 14 daily / 8 weekly / 12 monthly.

## Restore

1. Select a `Retained` backup in Settings/Recovery; the app verifies it again.
2. Restore into a disposable copy; replay the deletion ledger recorded after the backup's checkpoint; run invariants.
3. Operator approval (`DataDelete`-class capability because the live database is replaced) bound to the backup id and current live revision hash.
4. Swap: live database moved to `<data>/quarantine/<timestamp>.sqlite` (never deleted automatically); verified copy becomes live; WAL/SHM files of the old database are moved with it.
5. Post-restore: `audit_chain verify`, open in normal mode, record a `domain_events` row `recovery.restored`.

## Migration

1. Backup (above) is mandatory and verified.
2. `Migrator::dry_run_on_copy` runs the pending migrations against a `VACUUM INTO` copy; counts/constraints validated.
3. Live migration inside a transaction per migration file; `schema_migrations` records version, name, sha256, app version.
4. Failure → rollback-by-restore from the verified backup; the app never attempts a binary downgrade against a migrated schema (C12). Compatibility rules: an older app refuses to open a newer schema (`DATA_CORRUPTION`/`PRECONDITION_FAILED` with a clear message).

## Disk-full and power-fail

SQLite transactions guarantee atomicity; WAL checkpoints are monitored (`PRAGMA wal_checkpoint(TRUNCATE)` on idle). On `SQLITE_FULL` the command returns `RETRYABLE_STORAGE`; backups refuse to start below a free-space threshold (proposed 2× database size).

## Model-down and key-unavailable modes

- Model runtime unavailable: every deterministic feature stays available; teaching panels show "Recommendation unavailable (model offline)"; skill invocations return `RESOURCE_UNAVAILABLE`.
- Encryption key unavailable: RECOVERY-READONLY mode — refuse decryption, never overwrite, offer verified-backup restore.
