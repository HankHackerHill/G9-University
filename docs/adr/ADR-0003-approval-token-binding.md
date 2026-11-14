# ADR-0003: Approvals bind to the exact operation hash, resource, expected revision and expiry; single-use; HMAC-verified

**Status:** Accepted (2026-11-06)

## Context

v5.1 modelled approvals as PENDING/APPROVED/REJECTED with no binding (TEC-08, SEC-02) and spoke of 'multi-party' approval in a single-operator system (SEC-23). Without binding, a stale or re-targeted approval can be replayed (TOCTOU).

## Decision

`OperationDescriptor { action, resource, scope, parameters_sha256 }` where the hash is SHA-256 over canonical JSON (sorted keys, no whitespace, `g9_domain::hashing`). An approval records `binding_mac = HMAC-SHA256(install_secret, canonical(operation) || expected_revision || approval_id)`, an expiry (default 10 min since 2026-11-13, ADR-0015; was 15) and the expected revision. `verify_and_consume` runs inside the applying transaction, compares in constant time, and moves the approval to `CONSUMED` (single use). Operator-only capabilities (shell, network, filesystem write, policy modify, grade finalize, curriculum promote, data delete, model load, lab orchestrate) are never granted to skill or model actors, approval or not.

## Alternatives considered

Session-wide 'approve all' tokens (rejected: no binding). PIN prompts in the webview (rejected: not a trust boundary, handoff §12). Native confirmation dialog raised from Rust for protected operations (adopted as a complementary control, see ADR-0014).

## Consequences

Approvals cannot be replayed or re-targeted; every evaluation is logged (gate A06, ST09).

## Verification

`g9-policy` approval tests: AlreadyConsumed, OperationMismatch, ResourceMismatch, RevisionMismatch, Expired, MacInvalid.

## Addendum (2026-11-13): the bound revision is the stored one

Security audit 2026-11-12 (MEDIUM, CONFIRMED by an executable probe): the pipeline bound an approval to the client's `expected_revision` and compared it with the stored revision only after the approval step, so a client could request an approval of a failing attempt "for revision r+2", regrade it after the grant and consume the approval: a final, immutable grade the operator never saw.

Corrected in `crates/g9-app/src/pipeline.rs`:

| Rule | Behaviour |
|---|---|
| Binding | An approval binds to the stored revision of the resource whenever the resource exists; the client's `expected_revision` is bound only for a resource that does not exist yet. |
| Request | A request whose `expected_revision` differs from the stored revision is refused `STALE_REVISION` before any approval is created (the decision row records the approval requirement; no `approval_requests` row). |
| Consumption | The same rule computes the revision compared with the approval's binding, so any change between grant and use refuses the approval (`approval.revision_mismatch`) whatever revision the retry claims. |

Verification: `crates/g9-app/tests/approval_binding.rs` (4 tests; the two exploit cases, `assessment.approve_final` and `deletion.request`, failed before the change and pass after; two guards keep a current-revision request and a revision-less request working).

## Addendum 2026-11-14: the parameters hash is keyed

Since ADR-0021 `parameters_sha256` is not a plain SHA-256 of the canonical `{operation, payload}` but an HMAC-SHA-256 of it under a subkey of the binding key (`BindingKey::digest_json`, domain `parameters`): a plain hash in an immutable ledger confirmed a guess at the payload for anyone holding the database file. The binding is otherwise as above; the approval MAC covers the keyed value. An approval stored before the upgrade does not match and expires unused.
