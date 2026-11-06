# ADR-0006: Fixed baseline review schedule first; FSRS optional and only for declarative items

**Status:** Accepted (2026-11-06)

## Context

v5.1 applied FSRS per competency with a mandated 90 % target (C14, LRN-04, LRN-18, LRN-20).

## Decision

Default scheduler: offsets 1/3/7/21/60 days, reset on failure, bounded daily budget with reported overflow. `rs-fsrs` behind the `fsrs` cargo feature, off by default, applicable to `DeclarativeFact` items only; desired retention is a parameter evaluated per `docs/evaluations/fsrs-evaluation-protocol.md`. Practical competencies are re-verified by transfer checks on validity windows, never by memory curves.

## Alternatives considered

FSRS everywhere (rejected: conflates recall with practical competence). `fsrs-rs` with optimizer (rejected: ML training stack footprint; optimizer needs far more data than one learner produces).

## Consequences

Separation of recall and transfer (gate B02); workload stays bounded.

## Verification

`b02_transfer_checks_separate_from_item_reviews`, `baseline_intervals_monotone_and_reset_on_fail`.
