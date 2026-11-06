# ADR-0002: Explicit deletion action per relationship; no blanket ON DELETE CASCADE

**Status:** Accepted (2026-11-06)

## Context

v5.1 §3.2 applied `ON DELETE CASCADE` broadly, including to evidentiary tables (C13, TEC-05, TEC-15, TEC-16). Cascading deletes would silently erase attempts, observations and audit rows when a parent is removed, violating R-003.

## Decision

Each foreign key declares its deletion action with a justification comment: `RESTRICT` on evidentiary and governance rows (attempts, criterion results, grade decisions, observations, audit metadata, approvals, policy decisions, events); `CASCADE` only for structural children inside one immutable curriculum version; `SET NULL` for informational references. Personal content is erased through `deletion_requests` (redaction + tombstone + derived-row invalidation), never by cascading.

## Alternatives considered

Uniform cascade (rejected: evidence loss). Soft-delete flags everywhere (rejected: leaks content, complicates privacy erasure). Chosen: explicit per-FK policy plus a deletion workflow.

## Consequences

Deleting a curriculum version with attempts is refused; privacy erasure is explicit and auditable (gate A10).

## Verification

`deletion_restrict_on_evidentiary_rows`, `a10_deletion_request_redacts_and_tombstones`, `foreign_keys_enforced`.
