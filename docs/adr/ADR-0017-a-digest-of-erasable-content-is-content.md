# ADR-0017: A digest of erasable content is content

**Status:** Accepted (2026-11-13); the decision that an approved erasure also removes the attachment files was taken without the owner and is confirmed by the owner (2026-11-14, ADR-0020). Supersedes the "pre-erasure hash" part of ADR-0002 and resolves the PROPOSED purge item of ADR-0016.

## Context

ADR-0002 and `docs/privacy/data-lifecycle.md` had erasure keep the SHA-256 of what was erased: in each tombstone (`content_sha256_before`), in the redacted rows themselves (`notes.body_sha256`, `note_versions.body_sha256`, `attachments.content_sha256`, `evidence_items.content_sha256`) and in the events written when the content was created. The reasoning was evidence: the operator can later show that a given file or text was once held.

The security audit of 2026-11-13 (pass A1, reproduced by the lead auditor) showed what that costs. The SHA-256 of a short or guessable text is the text: a four-digit code is recovered from its digest in at most 10,000 guesses, a known file is recognised by its digest without any guessing. Exports carried these digests, and for a SECRET note that was never exported the digest still left through the tombstone and event tables. The probe printed `RECOVERED SECRET BODY: 4711` from an export that held no SECRET row.

| Where a digest of erasable content was stored | Could it be erased | Left in an export |
|---|---|---|
| `deletion_tombstones.content_sha256_before` | no (append-only) | yes |
| `deletion_requests.report_json` (listed the file paths, which are digests) | no (terminal state is immutable) | yes |
| `domain_events.content_sha256` of `note.created`, `note.revised`, `attachment.added` | no (append-only) | yes |
| `source_provenance.content_sha256` | no (append-only) | yes |
| `notes.body_sha256`, `note_versions.body_sha256`, `attachments.content_sha256`, `evidence_items.content_sha256`, `opt_in_reflections.body_sha256` | yes, and was kept on purpose | yes, for rows that are exported |
| the attachment file itself, named by its digest | only by hand | not applicable |

The erasure `reason` (operator text) and the approval `binding_mac` also left in exports; neither is content, but neither belongs in a file that leaves the application.

## Decision

1. **Erasure replaces every stored digest of the erased content** with the identity digest of the tombstone: `tombstone_digest(request_id, table, row_id, erased_at)` = SHA-256 of `g9-tombstone-v1\n<request id>\n<table>\n<row id>\n<erased_at in epoch ms>` (`g9_storage::deletion`). It is derived from nothing the content contains. It is written to the five tables above, `json_remove(provenance_json, '$.content_sha256')` is applied to evidence items, and the tombstone row holds the same value.
2. **No append-only table receives a digest of erasable content.** `note.created`, `note.revised` and `attachment.added` carry `content_sha256: null`; the provenance of an attached file carries none. What cannot be erased must not be written there.
3. **The approved erasure also removes the file.** `g9_storage::deletion::execute_erasure_in_store` removes, before it commits, exactly the stored files that this erasure left without a non-erased attachment (identical bytes share one file, which goes with its last attachment). The path comes from the row's own `storage_ref`; nothing walks the folder, so files this database does not reference (after a restore, the quarantined database still references some) are never touched. A `storage_ref` that is not the content address, a path that crosses a link and anything that is not a regular file are refused (`attachment_store::remove`). Removing the bytes first means a failure leaves rows that still name a missing file, which verification reports and a repeated erasure completes; the other order would leave an erased row and its bytes with no record of which file to remove. When no attachment folder inside an approved root is configured, the erasure still applies and answers the warning `deletion.attachment_bytes_not_purged`.
4. **The stored erasure report no longer lists file paths** (`attachment_storage_refs_to_purge` is not serialized): a path is the digest of the erased bytes.
5. **Exports withhold** `deletion_tombstones.content_sha256_before`, `deletion_requests.report_json`, `deletion_requests.reason`, `domain_events.content_sha256`, `source_provenance.content_sha256`, `approval_requests.binding_mac` and `approval_decisions.binding_mac`, in addition to what ADR-0016 withheld. SECRET rows are not exported, and nothing derived from them is.
5a. **Exports also withhold every hash of a whole request and every value that could confirm one** (found in pass C of the audit, after decisions 1 to 5 were written): `policy_decisions.parameters_sha256`, `approval_requests.parameters_sha256` and `.operation_json` (which repeats it), `approval_decisions.parameters_sha256`, and `audit_metadata.details_sha256`, `.prev_hash` and `.row_hash`. The hash of `{operation, payload}` is a guess-confirmation oracle: with the policy ledger of a SYSTEM export, a four-digit SECRET note body was recovered in 10,000 tries once its title was guessed (`pin`). The check values of the audit chain are withheld too, because they confirm a guess one hash further on. The ledgers themselves keep these values (they are immutable and bind approvals).
5b. **The attempt's submission digest** (`attempts.submission_sha256`, an evidentiary row that is never erased) commits to the evidence ids and to whether notes were handed in (`has_notes`), no longer to the notes text, which is stored as an erasable note.
6. **Migration 0014** recreates `trg_note_versions_update_only_erasure` so that the one update still allowed (an erasure of a version that was not erased, to an empty body, nothing else changed) must change `body_sha256`. An erasure that keeps the digest is refused by the schema.
7. **`deletion.request.reason`** is 1 to 500 characters (request schema).
8. **The confirmation says what goes.** The native approval confirmation names the stored note and how many files go with it (`g9_storage::describe`, ADR-0014 addendum 2).

The column `deletion_tombstones.content_sha256_before` keeps its historical name: renaming a column of an append-only table needs a table rebuild and a new schema pin, for no gain in safety. It now holds the tombstone digest; the Rust field is `Tombstone.tombstone_sha256` (serde alias `content_sha256_before` for reports written earlier).

## Alternatives considered

| Option | Why not |
|---|---|
| Keep the digests, never export them | The database file and every backup still hold them; "erased" would mean "recoverable by guessing from the file". |
| Keyed digests (HMAC with the binding key) | Useless to anyone but the key holder, still derived from the content, and ties erasable rows to key management. An identity digest has no relation to the content at all. |
| Salt per row, kept beside the digest | The salt travels with the digest; the guess loop is unchanged. |
| Drop the tombstone table | ADR-0002 wants evidence that an erasure happened. The tombstone, the audit chain (actor, approval, time) and the request row still show that. |
| Leave the files and wait for an owner decision (ADR-0016, PROPOSED) | The file's name is the digest; leaving it makes decision 1 void. The approval is bound to the exact request, and the confirmation now says how many files go. |

## Consequences

- An erased record can no longer be matched to a known file or text by digest. The proof that the erasure happened (request id, actor, approval, time, the tombstone row) remains.
- Attaching identical bytes after an erasure writes the file again (`already_stored: false`): the digest no longer exists.
- Event consumers lose `content_sha256` of notes and attachments. Nothing in the UI reads it.
- **Rows erased before migration 0014** keep their old digests, and old backup snapshots keep everything they held: a snapshot is a copy of the database and is private, as before (`docs/privacy/data-lifecycle.md`). Exports withhold the old columns from every database. Scrubbing such rows needs a data migration that rewrites append-only rows; it is PROPOSED and not implemented, because no such database is known (the application has not yet run on the owner's host).
- **Residual:** the immutable ledgers (`policy_decisions`, `approval_requests`, `approval_decisions`, `audit_metadata`, `idempotency_records`) still hold hashes of whole requests, in the database file and in every backup snapshot. A snapshot is a copy of the database and is private; an export carries none of these values (5a). PROPOSED: hash requests with a key (HMAC with the binding key and a domain-separation prefix), so that the database file alone cannot confirm a guess. It changes the value every approval is bound to and every stored request hash, so it needs its own decision and tests; not done here.
- The erasure is the one command that deletes files; it does so only inside the attachment folder, only the files of the erased attachments, and only after an approval whose confirmation named the record.
- Reversal: pass no store to `execute_erasure_in_store` (`attachment_store_of` in `commands/privacy.rs`) to return to "rows only"; the digests are replaced either way.

## Verification

| Claim | Test |
|---|---|
| No table holds a digest of a note, its versions or its files after erasure (INTERNAL, PRIVATE, SECRET) | `crates/g9-app/tests/erasure_digests.rs::erasing_a_note_leaves_no_digest_of_its_versions_or_files_in_any_table` (scans every column of every table) |
| Append-only tables never receive such a digest | `before_an_erasure_no_immutable_table_holds_a_digest_of_a_note_or_its_files` |
| The result and the tombstones carry only identity digests | `the_deletion_result_and_its_tombstones_carry_no_digest_of_the_erased_content`; `crates/g9-storage/tests/gates.rs::a10_deletion_request_redacts_and_tombstones` |
| Exports of every area, in both formats, hold no digest, reason or MAC | `an_export_of_every_area_holds_no_digest_of_erased_content_no_erasure_reason_and_no_approval_mac`; `an_export_holds_no_digest_of_a_secret_note_that_was_never_erased` |
| The file leaves, shared and foreign files stay | `erasing_a_note_removes_its_file_from_the_folder_and_leaves_every_other_file`; `attachment_store::tests::remove_*` (exact path, links refused) |
| No folder configured: the warning is answered | `an_erasure_without_an_attachment_folder_says_the_bytes_were_not_removed` |
| The schema refuses an erasure that keeps the digest | `repos::evidence::tests::note_create_revise_and_load_ledger`; `migrations::tests::the_schema_at_version_14_is_pinned_too` |
| Reason length | `a_reason_longer_than_the_limit_is_refused_before_anything_is_stored` |
| No request or chain hash leaves in an export (JSON and CSV, every area) | `an_export_holds_no_hash_of_a_request_that_carried_a_secret_and_no_value_that_could_confirm_one` (failed first: the JSON export held them) |
| The attempt keeps no digest of erased submission notes | `an_erased_submission_note_leaves_no_digest_of_its_text_in_the_attempt` (failed first: `attempts.submission_sha256`) |
| The confirmation says how many files go | `crates/g9-app/tests/native_confirmation.rs::an_erasure_confirmation_says_how_many_files_go_with_the_note` |

NOT RUN: an erasure on a database created before 0014 through the desktop shell (the shell cannot be built in the authoring sandbox).
