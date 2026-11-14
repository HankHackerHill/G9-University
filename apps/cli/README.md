# g9-cli — command-line driver (`g9`)

Thin client of `g9_app::dispatch` for end-to-end testing without the GTK/WebKit toolchain and for scripted operation on the G9 host. It composes the backend exactly like the Tauri bridge (`apps/desktop/src-tauri/src/lib.rs`): private data directory, binding key file (`g9_app::keyfile`), `Database::open`, `AppContext::builder(..).open()` (recovery), backend-assigned operator, `NullRuntime`. It never bypasses the pipeline, never writes SQL, never sets the actor from input.

| Command | What it does |
|---|---|
| `g9 init [--data-dir DIR]` | create/open `DIR/g9.sqlite`, run recovery, print the `RecoveryReport`, interrupted sessions and `system.status` |
| `g9 status` | `system.status` with bridge warnings appended |
| `g9 ops [--schemas]` | the 35 routed operations (action, resource kind, mutating, approval-expected, default scope) and the PROPOSED names |
| `g9 call <op> [--payload JSON \| --payload-file PATH] [--approval ID] [--expected-revision N] [--mode M] [--domain D] [--idempotency-key K] [--trace-id T]` | one dispatch; prints the `CommandOutcome` (exit 0) or the `G9Error` (exit 2) |
| `g9 approve <id>` / `g9 reject <id> [--reason]` / `g9 approvals [--state S]` | `approvals.decide` / `approvals.list` |
| `g9 import-seed [--manifest PATH]` | `curriculum.validate` → `curriculum.import` (`PROPOSED_SEED`) through the real approval round trip |
| `g9 reconcile --original PATH --candidate PATH [--write-candidate PATH --candidate-version LABEL --approved-by ID --approved-at now\|EPOCH_MS]` | `g9_curriculum::reconcile` of two manifest files: both are validated, the diff (days grouped by the fields that changed), the checksums and the block a reconciliation would carry are printed; reads two files, opens no data directory, approves nothing. With `--write-candidate` and the three review flags (none has a default: the block says this operator reviewed this diff) it writes the `RECONCILED_CANDIDATE` manifest plus `<stem>.import.json` and `<stem>.promote.json`, the payloads of `curriculum.import` and `curriculum.promote` (never over an existing file; read back and re-checked before success). The steps after that are `g9 call … --payload-file`, `g9 approve`: `docs/curriculum/promotion.md` |
| `g9 simulate-week --scratch --start-date YYYY-MM-DD [--zone Z] [--week N] [--fixed-clock EPOCH_MS]` | the Stage 03 exit criterion as a deterministic scripted week; per-step table; `PASS` (exit 0) only when every step ran and every check passed, `INCOMPLETE` when a step answered `<area>.not_implemented` (`NOT RUN`), `FAIL` otherwise (both exit 2) |
| `g9 crash-drill --scratch` | prepare + start a session and exit without completing it; a following `g9 init` reports it `INTERRUPTED` with an `UNCERTAIN` interval |
| `g9 restore (--backup-id ID \| --snapshot PATH --expect-sha256 HEX) [--confirm-replace-live-database]` | offline restore of a verified backup over the live database (stop the application first); without the confirmation flag it prints the plan and exits 1 changing nothing; with it the old database and its sidecars move to `<data dir>/quarantine` (never deleted), one metadata-only line is appended to `<data dir>/restore-log.jsonl`, then the normal boot runs; `g9_storage::restore` does the work, the CLI holds no SQL; procedure and refusal codes: `docs/runbooks/backup-restore.md` § Restore |

`simulate-week` and `crash-drill` write seed rows directly to storage (labelled `SEEDED`, bypassing the command pipeline) and refuse to run without `--scratch` (usage error, exit 1). The flag acknowledges that `--data-dir` is a throwaway directory; it does not inspect the directory.

Global flags: `--data-dir`, `--skills-dir`/`--no-skills`, `--program-start YYYY-MM-DD`, `--zone`, `--operator-id`, `--human`.
Exit codes: 0 success · 2 `G9Error` or scenario `INCOMPLETE`/`FAIL` (printed serialized on stdout) · 1 usage / host I/O (stderr), or `g9 restore` without `--confirm-replace-live-database` (the plan on stdout).

Build and test (standalone crate; the orchestrator adds it to the root workspace):

```sh
cd apps/cli && CARGO_TARGET_DIR=../../target/cli cargo build
CARGO_TARGET_DIR=../../target/cli cargo clippy --all-targets -- -D warnings && CARGO_TARGET_DIR=../../target/cli cargo test
bash ../../scripts/smoke_test.sh
```

See `docs/runbooks/testing-guide.md` for the full testing procedure (here/CI, on the G9 host, and what is not testable yet).
