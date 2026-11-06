# ADR-0003: Approvals bind to the exact operation hash, resource, expected revision and expiry; single-use; HMAC-verified

**Status:** Accepted (2026-11-06)

## Context

v5.1 modelled approvals as PENDING/APPROVED/REJECTED with no binding (TEC-08, SEC-02) and spoke of 'multi-party' approval in a single-operator system (SEC-23). Without binding, a stale or re-targeted approval can be replayed (TOCTOU).

## Decision

`OperationDescriptor { action, resource, scope, parameters_sha256 }` where the hash is SHA-256 over canonical JSON (sorted keys, no whitespace, `g9_domain::hashing`). An approval records `binding_mac = HMAC-SHA256(install_secret, canonical(operation) || expected_revision || approval_id)`, an expiry (default 15 min) and the expected revision. `verify_and_consume` runs inside the applying transaction, compares in constant time, and moves the approval to `CONSUMED` (single use). Operator-only capabilities (shell, network, filesystem write, policy modify, grade finalize, curriculum promote, data delete, model load, lab orchestrate) are never granted to skill or model actors, approval or not.

## Alternatives considered

Session-wide 'approve all' tokens (rejected: no binding). PIN prompts in the webview (rejected: not a trust boundary, handoff §12). Native confirmation dialog raised from Rust for protected operations (adopted as a complementary control, see ADR-0014).

## Consequences

Approvals cannot be replayed or re-targeted; every evaluation is logged (gate A06, ST09).

## Verification

`g9-policy` approval tests: AlreadyConsumed, OperationMismatch, ResourceMismatch, RevisionMismatch, Expired, MacInvalid.
