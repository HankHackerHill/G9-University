# ADR-0016: Operator-authored content, attachment bytes, and export to a file

**Status:** Accepted (2026-11-13), implementing the owner's request "Now finish the build: create a study session option; add, edit, delete calendar events; record a review result; create your own competencies, assessments or rubrics; attach evidence or files; export your data to a file".

## Context

Before this change the operator could not create sessions, calendar events, review items, competencies or assessments through the IPC (they came from seed data or the scheduler), attachments had a table but no way in, and `export.request` recorded an approved request without writing anything. ARCHITECTURE.md requires deny by default, no arbitrary filesystem capability in the mentor runtime, imported content treated as data, and every exported record to carry a domain and a classification (§3, §6).

## Decision

| Topic | Decision | Why |
|---|---|---|
| New operations | `session.create`, `calendar.create_event`/`update_event`/`delete_event`, `retention.create_item`/`record_review`, `competency.create`, `assessment.define`, `evidence.attach_file`/`from_note`/`list` (46 operations, was 35) | One operation per operator act, each through the command pipeline (schema, policy, revision, audit) like the existing ones |
| Which events can change | Only events with provenance OPERATOR, not protected, not immutable (`calendar.event_not_editable`); deleting needs `DataDelete` approval (`operator.delete.approval`) | Scheduler and curriculum events have their own lifecycle |
| How file bytes arrive | The webview reads the file the operator picked in the system file chooser and sends the bytes base64 (at most 16 MiB); the backend never opens a host path | Keeps the backend free of a filesystem read capability; the operator's pick is the authorization of the read |
| Where bytes live | Content-addressed store `<data>/attachments/ab/cd/<sha256>` (`g9_storage::attachment_store`): the path comes from the hash, never from a name; written `create_new` 0600, flushed, renamed, directory flushed; each shard level inspected without following links before it is created; identical bytes stored once | No caller-controlled path; no partial file; a link planted in the store cannot redirect a write |
| What is accepted | `g9_evidence::AttachmentRef::new`: the 7-type allow-list; executables, archives and packet captures refused by signature (SEC-13); a file whose bytes do not match its declared type is refused, because the schema has no quarantine columns | Quarantine needs storage first (PROPOSED) |
| Authority of attachments | Trust class `IMPORTED_DOCUMENT`; never parsed (PDF included); domain and classification inherited from the note, so a file on a security-sensitive note needs an approval | Imported content is data, never instructions |
| Export writer | An approved `export.request` writes `<data>/exports/<export_id>/` inside the command transaction: `data.json` or one CSV per table, the attachment bytes (hash-checked), `manifest.json` last; assembled in a hidden partial directory and renamed; never overwrites | One consistent read; a failure rolls the command back and the approval stays unspent |
| Export selection | Every schema table assigned exactly once (`g9_storage::export::TABLES`; a test fails on an unassigned table): own domain and classification, child of an exported row, or one fixed domain (governance tables → SYSTEM); `idempotency_records` never; erased rows never; SECRET rows never (docs/privacy/data-lifecycle.md); PRIVATE only with `include_private` | A new table cannot leak through an export unnoticed |
| CSV | RFC 4180 quoting; a text cell starting with `=`, `+`, `-`, `@`, tab or CR is prefixed with `'` | Imported text must not run as a spreadsheet formula |
| Backups | `backup.create` copies referenced attachment bytes to `<data>/backups/attachments`; `g9 copy-backup` carries them; `g9 restore` puts them back (`--attachments`) and lists any missing | A database restore without its files is a silent loss |

## Not decided here (PROPOSED or owner decision)

- Quarantine of attachments whose bytes do not match their type (needs columns).
- Hashing attachment files during backup verification (copies are hash-checked; verification checks existence).
- Signing the export manifest with the local key; an `export_records` table (the domain events `export.requested` and `export.written` record each export today).
- Purging an erased attachment's bytes from the live store: the erasure redacts the row and reports `deletion.attachment_bytes_not_purged`; the files stay until the owner decides whether the approved erasure may also remove them (and whether backup copies follow).
- Custom rubric criteria and the QUIZ and REPORT assessment kinds (no storage).

## Consequences

- New dependency `base64 =0.23.1` in `g9-app` (already in `Cargo.lock` through the shell's tree; default features off, so the `simd-unsafe` path is not compiled).
- The desktop bridge and the CLI configure `attachment_dir` and `export_dir` (`<data>/exports`, an approved root of its own).
- Exported files are not encrypted; like the database they rely on the host's full-disk encryption (ADR-0015).
