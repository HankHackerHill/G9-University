# Runbook: backup, verification, restore, migration (M17)

Preconditions: the app is the only writer; backups are created by the trusted backend (`g9-storage::backup`), never by shell `.dump` or file copy (C04/C05). Labels: IMPLEMENTED and TESTED (executed in this repository on 2026-11-08, not on the G9 host), or PROPOSED.

## Create and verify a backup

1. `backup.create` → `BackupState::Requested → SnapshotCreated` via the SQLite Online Backup API (`Backup::run_to_completion`) into `<data>/backups/<backup_id>.sqlite` (temporary name until verified).
2. `backup.verify` on the **copy**: `PRAGMA integrity_check`, `PRAGMA foreign_key_check`, migration checksums match the embedded set, table counts and relationship invariants, attachment manifest consistency → `Checked`.
3. `backup.restore_test`: open the copy in a disposable location, run the application invariant suite → `RestoreTested` → `Retained`. The manifest row records app version, schema version, page count, sha256, deletion-ledger checkpoint and (when enabled) encryption key id.
4. Failure at any step → `Failed` with the report attached; the previous retained set is untouched.

`backup.verify` and `backup.restore_test` read the manifest (state, path, checksum, audit anchor, verification report, revision) from `backup_manifests` (`g9_storage::backup::read_manifest`), so the three steps may run in different processes, for example `g9 call backup.create`, then `g9 call backup.verify`, then `g9 call backup.restore_test`, each a separate command (IMPLEMENTED, TESTED: `crates/g9-app/tests/backup_persistence.rs`, `scripts/smoke_test.sh`). An unknown id answers `NOT_FOUND backup.not_found`; an `--expected-revision` older than the stored revision answers `STALE_REVISION`. `system.status.last_backup` is the newest `RETAINED` manifest in the database. The operator label of `backup.create` is not persisted (`null` after a restart).

Recovery-point objective (proposed): daily automatic request after the first session of the day plus before every migration; retention 14 daily / 8 weekly / 12 monthly.

## Keep a copy on another drive

A backup in `<data>/backups` is on the same disk as the database. Copy each RETAINED backup to a USB drive or a NAS mount:

```
g9 copy-backup --backup-id <id> --to /media/usb/g9-backups
```

The destination folder must exist. The command reads the manifest from the live database (read-only; the application may be running), writes `<dest>.partial`, syncs it, renames it and syncs the folder, never overwrites a file, re-hashes the copy against the manifest's checksum, and writes `<id>.manifest.json` beside it with the checksum and the restore command. When this disk is gone, restore from the copy with `g9 restore --snapshot <copy> --expect-sha256 <sha256 from the manifest file> --confirm-replace-live-database` (TESTED: `apps/cli/tests/restore.rs::a_backup_copied_to_another_drive_restores_from_its_own_manifest`). Only RETAINED backups are copied (`restore.backup_not_retained` otherwise).

Durability of the backup itself: the snapshot file is synced before it is renamed to its final name and the backups folder is synced after (VERIFIED by syscall trace, 2026-11-13).

## Attachment files

Attached files live outside the database, in `<data>/attachments/ab/cd/<sha256>` (content-addressed; identical bytes stored once). They travel with the backups (IMPLEMENTED, TESTED: `apps/cli/tests/restore.rs::attachment_files_travel_with_a_copied_backup_and_come_back_on_restore`, `crates/g9-app/tests/feature_evidence.rs::a_backup_carries_the_attachment_bytes`):

| Step | What happens to the files |
|---|---|
| `backup.create` | every attachment the database references is copied to `<data>/backups/attachments/ab/cd/<sha256>`, hash-checked while copying, flushed, renamed into place; one store shared by all backups. A missing or damaged source is a warning on the backup (`backup.attachments_incomplete`); the snapshot is kept |
| `backup.verify`, `backup.restore_test` | check that every referenced file exists in the live store (`<data>/attachments`); hashing them during verification is PROPOSED |
| `g9 copy-backup` | copies the files the snapshot references to `<dest>/attachments/…`, from the backup store first, the live store for anything not there; reports `attachments.copied`, `missing`, `damaged`; existing files at the destination are kept |
| `g9 restore` | after the database swap and boot, copies the restored database's files into `<data>/attachments` from `--attachments <dir>`, by default the `attachments` folder beside the snapshot (so a copy made by `copy-backup` restores with no extra flag). A file not found intact is listed in `attachments.missing`/`damaged` with the warning `restore.attachments_missing`; the database restore is not undone. Run the restore again with `--attachments <folder>` once the files are found |

Exports (`export.request`, approval-gated) write `<data>/exports/<export_id>/`; they are your copy of the data, not a backup: nothing restores from them.

## Restore

Offline restore is IMPLEMENTED (`g9 restore` in `apps/cli`, on `g9_storage::restore::restore_over_live`; TESTED: `crates/g9-storage` `restore::tests::*`, `apps/cli/tests/restore.rs`, the restore section of `scripts/smoke_test.sh`). Restoring from inside the running application is PROPOSED (see the end of this section). ADR-0001 still holds: the live file is never copied by hand; it is replaced only by `restore_over_live`, which keeps the old database.

### Procedure (offline, application stopped)

1. Stop every process that has the data directory open (the desktop shell, any `g9` command). A database that another connection holds is refused with `restore.database_in_use` and nothing is changed; this includes an idle open application connection.
2. Choose a `RETAINED` backup: `g9 status --data-dir DIR` names the newest (`last_backup.backup_id`); the id is also the `backup_id` returned by `backup.create`.
3. Plan: `g9 restore --backup-id ID --data-dir DIR`. It prints the snapshot, its expected and actual SHA-256, the live path, the quarantine directory and the exact confirming command, exits 1 and changes nothing (it creates no directory and no file: when no `-wal`, `-shm` or `-journal` sits next to the live database it is read with `immutable=1`, otherwise through an ordinary read-only connection, which creates nothing either). Paths must be UTF-8 (they are printed and logged as JSON text); a path that is not is a usage error (exit 1) before anything is created.
4. Confirm: `g9 restore --backup-id ID --data-dir DIR --confirm-replace-live-database`. In order, each step failing closed and leaving the live database untouched until the rename:
   1. the manifest in the live database must be `RETAINED` and its snapshot must lie in `DIR/backups` (`restore.backup_not_retained`, `restore.snapshot_outside_backups`); with `--snapshot PATH --expect-sha256 HEX` instead of `--backup-id`, the checksum is mandatory;
   2. the snapshot is hashed and compared with the manifest checksum (and with `--expect-sha256` when also given): `restore.checksum_mismatch`;
   3. the snapshot is copied to `DIR/g9.sqlite.restore-tmp-<millis>` (new file, `0600`, never overwriting); **the copy** is hashed again, refused when its schema is newer than this build's newest migration (`restore.schema_too_new`) and verified with `verify_snapshot` (`integrity_check`, `foreign_key_check`, migration checksums, audit chain) and checked for truncation (a file shorter than `page_size * page_count`, e.g. cut inside its last page, is `storage.corrupt`); any failure removes only that temporary copy and the `-wal`/`-shm` SQLite created for it (also after a successful restore of a snapshot in WAL mode);
   4. exclusive access to the live file (`busy_timeout = 0`, `BEGIN EXCLUSIVE`): `restore.database_in_use` when busy; a live file that cannot be read (corrupt: the case restore exists for) is not locked but is still quarantined;
   5. the live WAL is checkpointed (best effort); then, so that the live path is never empty and never holds unverified content, whatever interrupts the swap: (a) the old `g9.sqlite` gets its quarantine name `DIR/quarantine/<name>.<UTC yyyymmddThhmmssZ>.quarantine` as a **hard link** (directory `0700`; a numeric suffix on collision; nothing is overwritten or deleted), the live path untouched; (b) its `-wal`, `-shm`, `-journal` sidecars are renamed into the quarantine; (c) the verified copy replaces the live path (an atomic `rename` over the old file, which lives on under its quarantine name; with no old file, a no-clobber `hard_link`); (d) the directories are synced. A failure before (c) puts the sidecars back where their names are free and removes only the extra link, and only while it still names the live file. A live file replaced by someone else during the swap, or a database that appeared at the live path while there was none, is never overwritten (`restore.swap_failed`). The filesystem must support hard links and the quarantine must be on the same filesystem as the live file, else `restore.swap_failed` before anything changes.
5. One metadata-only line (timestamp, snapshot path, SHA-256, quarantined paths; never content or key material) is appended to `DIR/restore-log.jsonl`.
6. The normal boot runs and its report is printed: recovery (open intervals become `UNCERTAIN`, the audit chain is verified) and, when the snapshot's schema is older than this build, migrations with their own verified pre-migration backup (the next section). Exit 0 on success, 2 with the serialized error, 1 on usage.

When the live database cannot be read (so `--backup-id` cannot be resolved), use `g9 restore --snapshot DIR/backups/ID.sqlite --expect-sha256 HEX ...` with the checksum from the manifest as printed by an earlier `backup.*` call (`manifest_sha256`) or from your own record; a snapshot is verified in any case. `--snapshot` into a data directory that does not exist yet creates it (`0700`) and restores without quarantine.

### Refusal codes

| Code | Meaning | Changes |
|---|---|---|
| `restore.checksum_mismatch` | snapshot SHA-256 differs from the expected value, or the file changed while it was copied | nothing |
| `restore.schema_too_new` | snapshot schema newer than this build (C12: no downgrade) | nothing |
| `restore.database_in_use` | another connection holds the live database | nothing |
| `restore.snapshot_invalid`, `restore.snapshot_is_live`, `restore.live_path_invalid` | unusable path, not a self-contained file, snapshot equals the live file | nothing |
| `storage.backup_verification_failed`, `storage.corrupt` | the copy failed verification or is not a database | nothing (temporary copy removed) |
| `restore.swap_failed` | a link or rename failed (hard links and a quarantine directory on the same filesystem as the live file are required), or the live path was replaced / a database appeared at it during the swap | the live database as it was (or the other file, untouched); sidecars put back where their names are free; the message says which |
| `restore.backup_not_found`, `restore.backup_not_retained`, `restore.snapshot_missing`, `restore.snapshot_outside_backups`, `restore.live_database_missing` (CLI) | the manifest cannot be used | nothing |

### An interrupted restore

If the process is killed (power loss, `SIGKILL`) during step 4.5, the live path still holds a complete database: the old one until the verified copy is renamed over it, the new one after. What can remain is a verified `DIR/g9.sqlite.restore-tmp-<millis>` and some quarantine files (links to the old database and its sidecars; the quarantine never loses the old content). Nothing needs repair: run the same command again (names never collide: temporary and quarantine names get a numeric suffix); delete the stale `restore-tmp` file by hand once no restore is running. Between the sidecar moves and the rename, the live path holds the old database without its `-wal`; with the application stopped and the WAL checkpointed (the first part of step 4.5) that WAL is empty. TESTED: `restore::tests::a_process_killed_during_the_swap_leaves_a_live_database_and_loses_nothing` (a simulated kill at each of three points). NOT RUN: a real `SIGKILL` of the CLI.

### After a restore

- The quarantine holds the replaced database and its sidecars. It is never deleted automatically; delete it by hand only when you no longer need it. To inspect it read-only without creating files next to it (the checkpointed main file holds the data): `python3 -I -c "import sqlite3; c = sqlite3.connect('file:/DIR/quarantine/g9.sqlite.<stamp>.quarantine?immutable=1', uri=True); print(c.execute('SELECT COUNT(*) FROM notes').fetchone())"` (the smoke test does exactly this); or copy the files to a scratch directory under the names `x.sqlite`, `x.sqlite-wal`, `x.sqlite-shm` and open that.
- The restored database is exactly the state at the snapshot: later notes, sessions, approvals, audit rows and backup manifests are not in it (they remain in the quarantine). A snapshot is taken before its own manifest row is written, so right after a restore `system.status.last_backup` is `null` until a new backup is retained; the snapshot file stays in `DIR/backups` and can be restored again with `--snapshot` and its checksum.
- Content erased after the snapshot (A10, `deletion.request`) is present again in the restored database. The quarantined database's `deletion_requests` and `deletion_tombstones` list those erasures; re-run them. Replaying the deletion ledger recorded after the backup's checkpoint automatically is PROPOSED (not implemented).
- `binding.key` is not touched; approvals stored in the restored database stay bound to it.
- A `domain_events` row `recovery.restored` is PROPOSED (not implemented): the restore is recorded in `restore-log.jsonl` and by the quarantine, not in the database.

### PROPOSED (not implemented)

- An in-app restore operation: select a `Retained` backup in Settings/Recovery, restore into a disposable copy, replay the deletion ledger after the backup's checkpoint, require an operator approval of `DataDelete` class bound to the backup id and the current live revision hash (the live database is replaced), then swap with the same quarantine rule. `g9_storage::restore::restore_over_live` is the storage primitive for it.
- Restoring into a running application (close the application's connection, swap, reopen). Today the application must be stopped; the `READ_ONLY` refusal of a mutating command says that in-app restore is not implemented.

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
