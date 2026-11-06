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
