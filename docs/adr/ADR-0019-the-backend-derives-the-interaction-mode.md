# ADR-0019: The effective interaction mode is derived by the backend, once, in dispatch

**Status:** Accepted (2026-11-14). Closes the LOW finding "Interaction mode is client-declared for 33 of 35 operations; declaring STUDY during an open exam is allowed" of `docs/audit/security-audit-2026-11-12.md` and completes TEC-25 / SEC-10 (`docs/audit/defect-register.md`).

## Context

The policy engine evaluates a command in the mode of its scope (`STUDY`, `LAB`, `EXAM`, `REVIEW`, `EXECUTIVE`, `RESEARCH`, `MAINTENANCE`). Two operations, `skills.invoke` and `session.correct_time`, replaced the declared mode with `EXAM` while an `EXAM` attempt was `STARTED` (`with_backend_mode`). Every other operation evaluated the mode the request declared, so a client that declared `STUDY` during an open exam was evaluated in `STUDY`. The audit probe: declared `EXAM` was refused (`policy.denied`), declared `STUDY` during the same open exam was allowed.

## Decision

1. **One derivation point.** `dispatch` calls `commands::effective_meta` after the request is validated and before the handler runs. A mutating operation is evaluated in `EXAM` while an `EXAM` attempt is `STARTED`, whatever mode the request declared. The handler receives the effective metadata, so the stored policy decision, the audit row, the approval request and its binding all carry the mode the backend evaluated. `skills.invoke` and `session.correct_time` no longer derive on their own.
2. **Backend-owned exemption.** `backup.create`, `backup.verify` and `backup.restore_test` carry `OpSpec::exam_exempt` and are evaluated as declared (`MAINTENANCE`). They copy and verify the database; they cannot reveal or change an exam. The policy allows system writes only in `MAINTENANCE`, so forcing `EXAM` on them would let an exam attempt that was left open block every backup. The flag is data in `crates/g9-app/src/ops.rs`; a request cannot set it.
3. **Reads pass as declared.** A read-only operation is evaluated under no mode-gated rule, and `system.status` and `audit.recent` must keep answering when storage is what is wrong. A mutating command whose open-attempt lookup fails with a storage error fails; a mode that cannot be derived is not assumed.
4. **Every other mode stays declared.** No backend state holds `MAINTENANCE`, so it is declared intent. What it gates is gated by something else: policy and settings changes need an approved change set, curriculum import and promotion need approval and the native confirmation, system writes are limited to the operations that exist. A backend-held maintenance state is PROPOSED, not built.

Measured before choosing (a throwaway test over `ops::OPERATIONS` with the operator actor and each operation's default scope, evaluated in its declared mode and in `EXAM`): four operations change outcome in `EXAM`, `session.correct_time` (already derived) and the three backup operations. Nothing else in the 53 operations is affected by deriving the mode for all of them.

## Alternatives considered

| Alternative | Why not |
|---|---|
| Keep two derived operations | Any mode-gated rule added later is bypassable by declaring another mode; the audit asked for one derivation. |
| `EXAM` for every mutating operation without exemption | Blocks backups for as long as an attempt stays open (measured above). |
| Derive only when the declared domain is `LEARNING` | The declared domain is client-supplied too; a rule that depends on it moves the bypass. |
| Derive `MAINTENANCE` from backend state | No such state or screen exists; PROPOSED. |

## Consequences

- During an open exam attempt, a write whose rule does not allow `EXAM` is refused whatever the window declared. Today that includes writes to the `PERSONAL` area (`operator.write.personal.allow` lists STUDY, REVIEW and EXECUTIVE). `assessment.submit` closes the attempt and lifts it.
- An approval requested before the exam started does not match a command evaluated in `EXAM` (the approval binds the whole operation descriptor, scope included: `operations_equal` in `g9-policy`, VERIFIED by reading the code). It stays pending until it expires; retry after the attempt is submitted.
- An idempotent retry of an earlier request while an attempt is open carries a different scope in its request hash and is refused as a key reuse. Retries do not cross an exam boundary in the window's own use.
- `skills.invoke` compares the optional payload `mode` with the effective mode, so during an open exam a payload that names another mode is refused as `skills.mode_mismatch`.
- The mock transport (`apps/desktop/src/ipc/transports/mock.ts`) does not model mode-gated policy beyond the teaching-skill denial; it is a stand-in, not a second implementation of policy.

## Verification

`crates/g9-app/tests/exam_mode_derivation.rs` (4; `a_write_is_evaluated_in_exam_while_an_exam_is_open_whatever_mode_the_client_declares` failed on the old code: the personal write was allowed during the open attempt), `assessment_reliability::tec25_declared_mode_cannot_widen_the_backend_exam_mode` (skills and time corrections, unchanged), the whole `g9-app` suite after the change (0 failed; counts in `CHANGELOG.md`).
