# Operation checklist: the definition of done for one IPC operation

Plan step APP-2 (`docs/release/path-to-10.md`, section 5.4). An operation is the unit the window and the CLI call: one name, one request schema, one handler, one policy decision. An operation is done when every row below is done, or explicitly not applicable with the reason written in the change.

A PROPOSED operation (listed in a `PROPOSED_OPERATIONS` table: `crates/g9-app/src/teaching.rs`, `analytics.rs`, `executive.rs`, `research.rs`) answers `PRECONDITION_FAILED <area>.not_implemented` and is not routed. Moving it out of that list makes every row below apply.

## Rows

| # | Item | Where | Done when | Checked by machine |
|---|---|---|---|---|
| 1 | Name | `<area>.<verb>`, lowercase, dotted | the name is unique and states what happens | registry and frontend list must agree (row 3) |
| 2 | Request and response schemas | `schemas/ipc/<op>.request.schema.json`, `<op>.response.schema.json`; embedded in `crates/g9-app/src/schemas.rs` | draft 2020-12, `$id`, `title`, `type: "object"`, `additionalProperties: false`, explicit `required`, no external `$ref` (CLAUDE.md); names that are shown to the operator use the keys `title`, `filename` or `label` so the boundary refuses hidden characters (`NAME_KEYS`, `crates/g9-app/src/dispatch.rs`) | `scripts/check_schemas.py`; `crates/g9-app/tests/schema_drift.rs`; every response is validated against its schema at run time (`schemas::validate_response`) |
| 3 | Registry row | `crates/g9-app/src/ops.rs` `OPERATIONS`; frontend `apps/desktop/src/ipc/operations.ts` `OPERATIONS` | `mutating`, policy `action` and `resource_kind` stated; `exam_exempt` only with the reason ADR-0019 requires | the registry completeness test (POL-2); `apps/desktop/src/ipc/schemas.test.ts` |
| 4 | Policy rule | `crates/g9-policy/src/rules.rs` `RuleSet::builtin_v1` | the operator is allowed in the intended domains and modes only; model and skill actors are refused; an operation that changes a protected record requires an approval (ADR-0003) | `crates/g9-policy/tests/a06_policy_matrix.rs`; POL-2 |
| 5 | Interaction mode | `commands::effective_meta` (`crates/g9-app/src/commands/mod.rs`) | the backend derives the mode; a request never declares EXAM away (ADR-0019) | `crates/g9-app/tests/exam_mode_derivation.rs` |
| 6 | Handler | `crates/g9-app/src/commands/<area>.rs`, routed in `commands::handler_for` | runs through `pipeline::execute`; no SQL in the handler (repositories only); domain rules in the pure crate; every refusal is a `G9Error` with a dotted code and no path, content or secret | `tests/integration/layering_gate.rs` (no SQLite outside storage and app) |
| 7 | Storage | `crates/g9-storage/src/repos/<area>.rs`; a new table only through a new migration `crates/g9-storage/src/migrations/00NN_<name>.sql` | `STRICT` tables; immutable records protected by triggers; a migration is never edited after it ships | migration checksums (`g9-storage` tests); the data inventory (PRIV-7) |
| 8 | Revision and idempotency | `CommandMeta.expected_revision`, `idempotency_key` | a mutation of an existing record requires the stored revision (`STALE_REVISION` otherwise); a repeated key with the same request replays, with another request conflicts | `crates/g9-app/tests/boundary_revision.rs`, `gates.rs`, `ledger_digests.rs` |
| 9 | Audit and log | `pipeline::execute` | a mutation appends one audit row (metadata only, no content, no digest of erasable content: ADR-0004, ADR-0017, ADR-0021); a refusal leaves a decision row; the operations log gets one line per call (`crates/g9-app/src/oplog.rs`); a new `tracing` call is registered in `crates/g9-app/tests/log_call_sites.rs` | `crates/g9-app/tests/oplog_probe.rs`; `log_call_sites.rs`; POL-2 |
| 10 | Approval text | `dispatch::approval_summary`, `g9_storage::describe` | an approval-gated operation shows what the record is from storage, never from the request (ADR-0014) | `crates/g9-app/tests/native_confirmation.rs` |
| 11 | Tests | `crates/g9-app/tests/feature_<area>.rs` or a dedicated file | success path; each refusal code; the approval round trip when gated; stale revision; replay; for free text an adversarial case (hidden characters, very long input, markup) | `cargo test --all-targets` |
| 12 | Error codes | the source | each new code has a meaning and an operator action | the error-code catalogue (OBS-5) when it is in place |
| 13 | Mock parity | `apps/desktop/src/ipc/transports/mock.ts` | the mock answers the same shapes and refusal codes as the backend for the cases the screens use | `apps/desktop/src/ipc/transports/mock.test.ts`; screen tests |
| 14 | Window | the screen under `apps/desktop/src/app/screens/` | the screen calls the operation through the typed client (`apps/desktop/src/ipc/client.ts`), shows refusals in words (`docs/ui/ux-copy.md`), and follows the design system (`docs/ui/design-system.md`) | vitest; `scripts/e2e/tour.cjs` and `personal.cjs` against the real backend |
| 15 | CLI | `g9 call <op>` | reachable without extra code (the CLI dispatches by name); a dedicated subcommand only for an offline or maintenance task | `apps/cli/tests/` |
| 16 | Requirements and docs | `docs/requirements/rtm.md`; `CHANGELOG.md`; the runbook the operator reads | the RTM row names the tests; labels are TESTED / NOT RUN / PROPOSED, never PASS without a run | the backlog map (DOC-1) and the document reference checker (DOC-2) when in place |

## Order of work

1. Schemas, then the registry row and the policy rule (rows 2 to 5): the operation is refused for the right reasons before it does anything.
2. Tests of the refusals, then the handler and storage (rows 6 to 11), fail-first.
3. Mock, window, CLI, docs (rows 13 to 16).

## Not applicable, with the reason

| Situation | Rows that do not apply |
|---|---|
| A read-only query | 8 (no revision, no idempotency), 10; row 9 applies to the log, not to the audit ledger |
| An operation the window never calls | 13, 14 (say so in the RTM row) |
| A PROPOSED operation | all rows except 1; it stays in its `PROPOSED_OPERATIONS` list |
