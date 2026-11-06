# ADR-0013: Closed 13-category error set, backend-assigned actor, success envelope with revision and replay flag

**Status:** Accepted (2026-11-06)

## Context

The handoff's envelope lacked a success shape, correlation id and typed error details, and let the client supply `actor_id` (TEC-03, TEC-04, SEC-01).

## Decision

`CommandMeta` (request_id, operation, resource, expected_revision, idempotency_key, scope, payload_schema_version, approval_id, trace_id, issued_at) — the backend sets the actor from the session, never from the payload. `CommandOutcome<T>` returns `revision`, `audit_ref`, `replayed`. `G9Error` has exactly 13 categories with machine codes and secret-free details (`STALE_REVISION` carries expected/actual; `APPROVAL_REQUIRED` carries `approval_id`). Idempotency outcomes are persisted so a crash between commit and reply is replayable.

## Alternatives considered

Free-form error strings (rejected). Client-asserted actor (rejected).

## Consequences

Stable UI contracts; schema-checked at both ends (gate A11, R-005).

## Verification

`error_serializes_without_backtrace_or_secret_fields`, `categories_are_exactly_thirteen_and_closed`, pipeline tests in `g9-app`.
