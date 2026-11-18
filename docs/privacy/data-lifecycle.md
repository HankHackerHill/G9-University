# Privacy, retention, deletion and key management

## Data domains and classifications

Every content-bearing record carries `data_domain` (LEARNING, BUSINESS, PERSONAL, SECURITY_SENSITIVE, SYSTEM) and `classification` (PUBLIC, INTERNAL, PRIVATE, SECRET). Rules:

- Cross-domain reads require `Capability::CrossDomainRead`, granted only by an operator approval bound to the operation (R-017).
- `PRIVATE`/`SECRET` content never enters model context or an export without an approval; `SECRET` never enters model context at all (`g9-ai-adapter::context`).
- Audit metadata is **metadata only**: actor, operation, resource ids, decision, hashes, timestamps. Raw prompts, note bodies, answers and attachments are never written to audit or diagnostic logs (handoff C19).

## Retention

| Record class | Default retention | Notes |
|---|---|---|
| Authoritative evidence (attempts, criterion results, grade decisions, observations) | indefinite | immutable; corrections supersede |
| Audit metadata, policy decisions, approval decisions | indefinite | hash-chained; tamper-evident |
| Notes, note versions, attachments | until operator deletion | erasure via `deletion_requests` (the tombstone identifies the erased row and keeps no digest of its content; the file is removed) |
| Opt-in reflections, capacity self-reports | 365 days default, configurable | PERSONAL domain; excluded from exports by default |
| Skill invocation records | 180 days | metadata only |
| Idempotency records | 30 days | purged at open, counted back from the earlier of the clock and the newest stored record (a clock set ahead purges nothing recent); a deleted calendar event's title is removed from them at once |
| Backups | per `data_retention_rules` (daily 14, weekly 8, monthly 12 proposed) | NOT encrypted by the application: a snapshot and every copy (`g9 copy-backup`, attachment files) is a plain SQLite file or plain file, mode 0600, protected only by the host's full-disk encryption (ADR-0015). A copy on a USB drive or a synced folder is readable by whoever holds it. Application-level encryption is PROPOSED (row below) |

Retention is enforced by a deterministic maintenance command (`system.retention_sweep`, Stage 03) that requires an approval per run and writes a `domain_events` row summarizing counts.

## Deletion model (A10)

1. Operator creates a `deletion_request` (scope: record ids or a domain + time range).
2. Policy requires `Capability::DataDelete` approval bound to the request hash.
3. Execution (single transaction): content columns are redacted to NULL/empty and every stored digest of the content (`notes.body_sha256`, `note_versions.body_sha256`, `attachments.content_sha256`, `evidence_items.content_sha256`, `opt_in_reflections.body_sha256`) is replaced by the tombstone's identity digest, because a digest of erasable content is content (ADR-0017: the SHA-256 of a four-digit code is the code). A tombstone keeps the table, row id, request and timestamps, never a digest of what was erased; the files of the erased attachments are removed from the attachment folder before the transaction commits; derived rows that referenced the content get `invalidated_by = deletion_request_id`; evidentiary rows that *must* survive (grades, audit) are retained but their content references resolve to tombstones. Append-only tables (events, provenance) never receive a digest of erasable content.
4. Backups: deletion is recorded in `deletion_requests`; backups created before the deletion remain until their retention expires. **PROPOSED, not built (PRIV-5, gap G7):** on restore, replay the deletion ledger before the database is accepted. Today a restore of a snapshot taken before an erasure brings the erased content back; the operator re-requests the erasure after such a restore. This is the reconciliation of "user deletion vs backup retention" (handoff §7) that remains open.
5. Exports produced after deletion exclude tombstoned content (erased rows, and the summaries of evidence that pointed at them). Exports never carry digests of governance records (tombstones, deletion reports and reasons, event and provenance digests), hashes of whole requests or the audit chain's check values (a hash of `{operation, payload}` confirms a guessed secret), or approval MACs; they never carry SECRET rows. Exports produced before are the operator's responsibility and are named by the `export.written` events.
6. Rows erased before migration 0014 keep their old digests (PROPOSED: a data migration to scrub them; no such database is known), and backup snapshots keep everything they held.
7. The immutable ledgers (`policy_decisions`, `approval_requests`, `approval_decisions`, `idempotency_records`, `skill_invocations`, the audit details) hold the hash of a whole request only as an HMAC-SHA-256 under a subkey of the binding key, which is not in the database, a snapshot or an export (ADR-0021): the database alone cannot confirm a guess at a request. Requests made before that change keep their plain digests in the rows and snapshots that already hold them.

8. The request's `reason` is kept verbatim in `deletion_requests` (SYSTEM domain), in the database and in every snapshot taken after the request; it is never exported or logged. Do not repeat the erased content in it.
9. Attachment copies under `<data>/backups/attachments/` are shared by all backups and are not removed by an erasure: they belong to the snapshots taken before it and leave only with backup retention (PROPOSED: retention must also remove copies no retained snapshot references).
10. Verified on raw bytes (TESTED 2026-11-16, `apps/cli/tests/erasure_probe.rs`, plan step PRIV-3): after an approved erasure, no file of the data directory holds the erased title, body, file name, file bytes, their base64 or their SHA-256, except the snapshot taken before the erasure and the attachment copy only it references. Both `secure_delete` and the TRUNCATE checkpoint after the erasure are needed: with either removed, the probe finds the content.

## Operations log

`<data>/logs/g9.log` (`crates/g9-app/src/oplog.rs`, plan step OBS-1): one JSON line per command (registry operation name, request id, outcome category, error code, duration) and the backend's diagnostic events with allow-listed fields only. It holds no payload, title, file name, path or free text (TESTED: `crates/g9-app/tests/oplog_probe.rs`; every logging call site is literal, allow-listed and registered: `crates/g9-app/tests/log_call_sites.rs`). Directory `0700`, files `0600`; at most four files of 1 MiB (rotated). It is not part of a backup or an export, and an erasure has nothing to remove from it. It is metadata about activity (which operation, when, how long): it stays on the machine like the database.

## Encryption and key management (decision: layered, operator-chosen)

| Layer | Option | Status |
|---|---|---|
| Host | LUKS full-disk encryption on the G9 OS workstation | ASSUMED available; verified in Stage 15 |
| Database at rest | (a) rely on FDE (default); (b) SQLCipher via `rusqlite` `bundled-sqlcipher` feature | (b) PROPOSED behind a feature flag; key from OS keyring (Secret Service API on Linux) — never from a file inside the repo |
| Backups | age/XChaCha20-Poly1305 encryption of the snapshot file with a key in the OS keyring; manifest stores key id, not key | PROPOSED (Stage 03); unencrypted snapshots allowed only inside an FDE volume and flagged in the manifest |
| Approval binding key | 32 random bytes generated on first run, stored in the OS keyring (fallback: `0600` file under the app data dir with a WARNING state in Settings/Recovery) | IMPLEMENTED in `g9-policy::binding` (key supplied by `g9-app`) |
| Key unavailable | app opens in RECOVERY-READONLY mode: refuses decryption, never overwrites, offers restore from a verified backup | contract in `g9-storage`; UI state in Settings/Recovery |

No key material is ever logged, serialized into IPC, or included in exports.

## Exports (A10)

`export.request` (approval-gated, IMPLEMENTED 2026-11-13, ADR-0016) writes `<data>/exports/<export_id>/`: `data.json` or one CSV per table, the attachment bytes, and `manifest.json` (SHA-256 and size of every file, app version, domain filter, `include_private`, withheld columns). `PRIVATE` rows leave only with `include_private`; `SECRET` is never exported; erased rows never leave; assessment task statements are withheld. The manifest is not signed (signing with the local key is PROPOSED), and there is no `export_records` table: each export is recorded by the domain events `export.requested` and `export.written`. The export is a derived artifact; the database remains authoritative.
