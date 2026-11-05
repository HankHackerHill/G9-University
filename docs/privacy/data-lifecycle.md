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
| Notes, note versions, attachments | until operator deletion | erasure via `deletion_requests` (tombstone keeps sha256 + size + timestamps) |
| Opt-in reflections, capacity self-reports | 365 days default, configurable | PERSONAL domain; excluded from exports by default |
| Skill invocation records | 180 days | metadata only |
| Idempotency records | 30 days | |
| Backups | per `data_retention_rules` (daily 14, weekly 8, monthly 12 proposed) | encrypted at rest (see below) |

Retention is enforced by a deterministic maintenance command (`system.retention_sweep`, Stage 03) that requires an approval per run and writes a `domain_events` row summarizing counts.

## Deletion model (A10)

1. Operator creates a `deletion_request` (scope: record ids or a domain + time range).
2. Policy requires `Capability::DataDelete` approval bound to the request hash.
3. Execution (single transaction): content columns are redacted to NULL/empty, a tombstone keeps `content_sha256`, `byte_len`, timestamps and provenance; derived rows that referenced the content get `invalidated_by = deletion_request_id`; evidentiary rows that *must* survive (grades, audit) are retained but their content references resolve to tombstones.
4. Backups: deletion is recorded in `deletion_requests`; backups created before the deletion remain until their retention expires; on restore, the deletion ledger is replayed before the database is accepted (`backup_manifests.deletion_ledger_checkpoint`). This is the explicit reconciliation of "user deletion vs backup retention" (handoff §7).
5. Exports produced after deletion exclude tombstoned content; exports produced before are the operator's responsibility and listed in `export_records`.

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

`export.request` produces a signed manifest (sha256 of every file, app/schema version, domain filter, classification ceiling) and a directory of JSON/CSV files; `PRIVATE` requires approval, `SECRET` is never exported. The export is a derived artifact; the database remains authoritative.
