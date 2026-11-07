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
| `g9 simulate-week --start-date YYYY-MM-DD [--zone Z] [--week N] [--fixed-clock EPOCH_MS]` | the Stage 03 exit criterion as a deterministic scripted week; per-step table; `PASS` (exit 0) only when every step ran and every check passed, `INCOMPLETE` when a step answered `<area>.not_implemented` (`NOT RUN`), `FAIL` otherwise (both exit 2) |
| `g9 crash-drill` | prepare + start a session and exit without completing it; a following `g9 init` reports it `INTERRUPTED` with an `UNCERTAIN` interval |

Global flags: `--data-dir`, `--skills-dir`/`--no-skills`, `--program-start YYYY-MM-DD`, `--zone`, `--operator-id`, `--human`.
Exit codes: 0 success · 2 `G9Error` or scenario `INCOMPLETE`/`FAIL` (printed serialized on stdout) · 1 usage / host I/O (stderr).

Build and test (standalone crate; the orchestrator adds it to the root workspace):

```sh
cd apps/cli && CARGO_TARGET_DIR=../../target/cli cargo build
CARGO_TARGET_DIR=../../target/cli cargo clippy --all-targets -- -D warnings && CARGO_TARGET_DIR=../../target/cli cargo test
bash ../../scripts/smoke_test.sh
```

See `docs/runbooks/testing-guide.md` for the full testing procedure (here/CI, on the G9 host, and what is not testable yet).
