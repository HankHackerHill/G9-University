# ADR-0004: Hash-chained, metadata-only audit records; private content never logged

**Status:** Accepted (2026-11-06)

## Context

v5.1 S6 proposed immutable append-only logs with payloads; v6.0 C19 noted the conflict with privacy and deletion (SEC-08).

## Decision

`audit_metadata` rows carry actor, operation, resource ids, decision, outcome, request/trace ids, timestamps and a `details_sha256`; `row_hash = sha256(prev_hash || canonical_json(row))`; UPDATE/DELETE are refused by triggers; periodic anchors are copied into backup manifests. Raw prompts, note bodies, answers and attachments are never written to audit or diagnostic logs; invocation records hold token counts and hashes only.

## Alternatives considered

Full-payload immutable logs (rejected: irreconcilable with erasure). No tamper evidence (rejected: handoff §12 requires it).

## Consequences

Tamper evidence without content retention; deletion requests remain auditable through tombstone hashes.

## Verification

`audit_chain_detects_tamper`, `a04_trigger_blocks_audit_row_mutation`, `invocation_record_has_no_prompt_text`.

## Addendum (2026-11-14): the tail, and what tamper evidence does not cover

**Status:** Accepted. Closes the LOW finding "Deleting the newest audit rows is not detected" of `docs/audit/security-audit-2026-11-12.md`.

The links of the chain cannot show that the newest rows are missing: every row that is left still verifies. `AuditChain::verify_chain` (`crates/g9-storage/src/audit.rs`) therefore also checks, after the links:

| Check | What it compares | Detects |
|---|---|---|
| Sequence counter | `sqlite_sequence.seq` for `audit_metadata` (the largest `seq` `AUTOINCREMENT` ever allocated) with the `seq` of the newest row | newest rows deleted; counter lowered |
| Backup anchors | for every `backup_manifests` row with `audit_anchor_seq`/`audit_anchor_hash`: the row at that `seq` exists and carries that hash; half an anchor is refused | newest rows deleted together with a reset counter |

A failure is reported like a broken link (`ok: false`, `first_bad_seq` = the first missing position, `first_bad_reason`). `recovery::run` runs it at every open, so a truncated tail makes the database read-only (`audit_chain_broken`, `DATA_CORRUPTION recovery.integrity_failed` for every mutation) and `backup.verify` refuses a snapshot whose tail is truncated. Tests: `crates/g9-storage/tests/audit_tail.rs`, `crates/g9-app/tests/audit_tail.rs`.

**Limit, stated.** The counter and the anchors live in the same file as the rows. A writer with write access to the database file who deletes the newest rows, sets the counter back **and** deletes or rewrites the manifests that anchor them leaves a chain that verifies (`a_writer_that_also_rewrites_the_anchors_is_not_detected_which_is_the_documented_limit` states it as a test). A MAC over the anchors does not close this: the key (`binding.key`) is readable by the same user. Tamper evidence against a same-user writer needs an anchor kept outside that user's reach.

PROPOSED, not built: copy the chain head (`seq`, `row_hash`) with every backup that is copied off the machine (`g9 copy-backup`) and add a check that a live database contains the head of an off-site snapshot at the same `seq` with the same hash. Until then the audit chain is tamper-evident against accidental damage, a crashed writer and a naive edit, and not against a deliberate same-user editor.
