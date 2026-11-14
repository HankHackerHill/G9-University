# ADR-0021: The hashes of whole requests in the immutable ledgers are keyed

**Status:** Accepted (2026-11-14). Owner decision: ADR-0020 row 6. Closes the residual of the finding "Digests of erased and of SECRET content, erasure reasons and approval MACs leave in exports" (`docs/audit/security-audit-2026-11-13.md`) and the residual item of ADR-0017. Supersedes the sentence of ADR-0003 that says the parameters hash is a plain SHA-256.

## Context

`OperationDescriptor::parameters_sha256` and the idempotency `request_sha256` were SHA-256 of the canonical JSON of `{operation, payload}` (and, for the request hash, the meta fields). They are stored in ledgers that cannot be erased: `policy_decisions`, `approval_requests` (also inside `operation_json`), `approval_decisions`, `idempotency_records`, and in the details of `audit_metadata`. A plain hash of a request confirms a guess at it: with the title known, a four-digit note body is recovered in 10,000 tries by anyone who holds the database file or one of its snapshots. ADR-0017 stopped exports from carrying these values and erased the digests of erasable content, but the database file and every snapshot still held them. The stored result digest of a content-bearing command (`result_sha256` in a replay record) is the same kind of value, and so are the digests of a skill's input and output in `skill_invocations`, a sixth immutable ledger that the audit did not list (found while checking for the same class: no delete, no update once terminal, and a skill's input can be a note's text).

Reproduced on the unchanged code before this change (`crates/g9-app/tests/ledger_digests.rs::no_ledger_holds_a_plain_sha256_of_a_request_that_carried_a_four_digit_secret`): the probe took every 64-hex value stored anywhere in the database, hashed the 10,000 four-digit bodies in the two forms the pipeline uses, and found `parameters form: body 4711` and `request form: body 4711`.

## Decision

1. **One keyed construction.** `g9_policy::BindingKey::digest_json(domain, value)` is `HMAC-SHA256(subkey, len(label) ‖ label ‖ canonical_json(value))`, hex, with `subkey = HMAC-SHA256(binding_key, "g9-mentor/ledger-digest-key/v1")`. The subkey keeps the key that MACs approvals from MACing a digest too; the label keeps a digest made for one use from standing in for another. Checked against an independent implementation (Python `hmac`; three known-answer vectors in `crates/g9-policy/src/binding.rs`).
2. **Five domains.** `Parameters` (the operation and payload an approval is bound to), `Request` (the whole request as the idempotency ledger compares it), `Result` (the full result behind a content-free replay record), `SkillInput` and `SkillOutput` (what `skill_invocations` records).
3. **Where it applies.** `pipeline::parameters_digest` (the descriptor of every command, so `policy_decisions`, `approval_requests`, `approval_decisions` and the approval MAC that covers the descriptor), `pipeline::request_digest` (`idempotency_records.request_sha256` and the `request_sha256` in audit details), the stored `result_sha256`, the recovery sweep's descriptor (`recovery.interrupt_session`), the digest a calendar proposal is applied under (`calendar.propose`), and `skill_invocations.input_sha256` / `output_sha256` (`skills.invoke`). The plain `pipeline::parameters_sha256` no longer exists.
4. **Names and schema unchanged.** The columns and fields keep the name `*_sha256`; the value is a 64-character hex HMAC-SHA-256, which satisfies the same checks. No migration.
5. **What stays a plain hash, and why.** The audit chain (`prev_hash`, `row_hash`, `details_sha256`): its inputs now contain keyed values, so it confirms no guess about a request. Whether the chain itself is keyed (tamper evidence against a writer of the database file) is a separate, still PROPOSED decision (`docs/audit/security-audit-2026-11-12.md`, ADR-0004). Content hashes that are not requests (migrations, skill pins, manifests, attachment names) are unchanged.
6. **Key handling.** The key is `binding.key` (0600) in the data directory: outside the database, outside backups and outside exports (`g9 copy-backup` copies the snapshot, its manifest and the attachments only). Losing or replacing it makes every stored keyed digest unverifiable: an outstanding approval is refused (`approval.operation_mismatch`) and a replayed idempotency key reads as a conflict. Nothing else depends on it: no data is encrypted with it.

## Alternatives considered

| Alternative | Why not |
|---|---|
| A random salt stored beside each hash | The salt sits in the same file: whoever has the file has the salt |
| Encrypt the ledgers | A format change and a key anyway; approval verification would need decryption. The keyed digest gives exactly the property needed |
| Stop storing whole-request hashes | The approval binding and idempotent replay need them |
| Use the binding key itself, without a subkey | Reuses the approval MAC key for a second purpose; the subkey costs one HMAC |
| Key the audit chain in the same change | A different property (tamper evidence) with its own decision and its own failure modes |

## Consequences

- **At the upgrade, an approval that is pending or approved and not yet consumed is refused** (`approval.operation_mismatch`, the approval is left unconsumed) and expires unused after at most 10 minutes: the request is made again. Tested by building such an approval exactly as the older build stored it (`an_approval_made_before_the_digests_were_keyed_does_not_authorize_the_request_now`). An idempotency key reused across the upgrade reads as a conflict.
- **Rows written before the upgrade keep their plain digests** (the ledgers are immutable), and so do snapshots taken before it. Only rows from the upgrade on are protected. A database that held a secret request before the upgrade still holds a value that confirms it (PROPOSED: a re-keying migration is not possible for immutable rows; the protection is for new requests).
- **The fingerprint** (the first 16 characters of `parameters_sha256` on the approval card, in the native confirmation and on the Approvals screen) is now a keyed value: it is still the same on all three, but it cannot be recomputed from a payload outside this installation. It could not be, in practice, before: the payload is not stored.
- **A key change refuses an approval earlier.** The scenario ST09 of the evaluation harness split into a directly forged MAC (`approval.mac_invalid`) and an approval presented after the install secret changed (`approval.operation_mismatch`: the keyed digest inside it no longer matches, before the MAC is checked).
- **Residual risk.** Whoever holds both the database and `binding.key` (a compromise of the operator's own account) can recompute the digests; that is outside the threat model of a single-user local application (ADR-0004: tamper evidence does not hold against a same-user writer). Someone who holds only the database, a snapshot or an export cannot confirm a guess at a request from them (equal requests still have equal digests, so a request made twice is visible as such).
- `ARCHITECTURE.md` names `parameters_sha256` and does not say it is unkeyed; it needs no edit.

## Verification

| Check | State |
|---|---|
| `ledger_digests.rs::no_ledger_holds_a_plain_sha256_of_a_request_that_carried_a_four_digit_secret` failed on the old code (recovered body 4711 from both forms) and passes now | TESTED 2026-11-14 |
| `ledger_digests.rs`: the ledgers hold the keyed digests of exactly those requests (the positive control: the pre-images are the real ones); a stored result digest is keyed and a replay still works; an approval stored by an older build is refused and left unconsumed, and a new request goes through | TESTED |
| `ledger_digests.rs::the_digests_of_a_skills_input_and_output_are_keyed`: failed on the old code (the ledger held `sha256_json(input)`); now the input digest is the keyed one and both digests differ across binding keys and repeat under one (a plain hash is the same in all three runs) | TESTED |
| `crates/g9-policy/src/binding.rs`: the digest matches an independent implementation for all five domains, is keyed, domain-separated, canonical, and is neither the plain hash nor an approval MAC | TESTED |
| The evaluation gates (`gate_evidence_document`, `st_adversarial_catalogue`) and `docs/evaluations/gate-evidence.md` regenerated for the five "bound to the exact parameters" rows and the split ST09; `scripts/smoke_test.sh` (the `simulate-week` step table is still reproducible across fresh data directories) | TESTED |
| The whole default workspace: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test --all-targets`: 975 passed, 0 failed | TESTED 2026-11-14 |
| A snapshot taken before the upgrade | still holds plain digests: NOT changed |
