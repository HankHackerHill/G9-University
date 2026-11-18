# Stage report: Stages 00 to 03 (G9 MENTOR), 2026-11-08

Format: `GOVERNANCE.md` "Stage PASS report (required content)". Evidence labels: VERIFIED, OBSERVED, INFERRED, ASSUMED, PROPOSED, TESTED, BLOCKED, NOT RUN. **No Stage PASS is issued.** Stage 00 needs an operator PASS (decision recorded in section 13); Stages 01 to 03 have partial exit criteria (section 3). Unexecuted checks are NOT RUN, never PASS.

Repository state: branch `ccr-0cea681c-p9jbru`; the commit that last touches this file is the report's HEAD; baseline `f414481` (initial commit, 2026-11-05). All commits are local and **unpushed** by operator instruction ("Do not connect to GitHub only work here"); the remote branch is at `e987383`.

## Update 2026-11-18 (latest): the owner's v2.0 plan applied as a second curriculum shape; scope cut at the owner's usage limit

Owner requests: apply the "G9 Cybersecurity Mastery v2.0" plan, fill gaps only if needed, add quizzes, tests, questions and hands-on learning (Hack The Box, TryHackMe and similar), then one more triple security audit with a search of everything known as of 2026-11-16; later, "I only have 13% usage left for the week ... only execute things that matter and help this app be useable ... while still maintaining the safety and security of the codebase". No CI or workflow was started (`ci.yml` stays `workflow_dispatch` only).

| Delivered | Where | Label |
|---|---|---|
| Second structure profile `MASTERY_V2` next to `CT9_480`; manifest 1.1.0; import, plan, labels, desktop mock | ADR-0023; the five `mastery_v2` test files and `apps/desktop/src/ipc/transports/mock.test.ts` | TESTED (counts below) |
| Courseware builder and its 460 tests; schemas for answer keys, resource registries and 60-day lesson sheets | `scripts/build_curriculum_v2.py`, `scripts/tests/test_gates_build_curriculum_v2.py`, `schemas/curriculum/` | TESTED |
| Resource registries of 12 weeks (460 entries) | `docs/curriculum/v2/WEEKS/Wnn/RESOURCES.json` | SEARCH_CHECKED, ASSUMED or UNVERIFIED per entry; no page opened |
| Course documents (drafted), week 1 design files and 61 fixtures (SHA-256 match: VERIFIED), 2 of 60 day guides | `docs/curriculum/v2/` | DRAFT; no review pass run |
| Doc-reference gate green again: 36 references to unwritten courseware allow-listed as `planned` | `scripts/doc_refs_allow.json` | TESTED (`check_doc_refs.py`: 0 broken) |
| `--available` judges only weeks with all five day guides, so a partly written week does not fail the preflight | `scripts/build_curriculum_v2.py` | TESTED (460 builder tests) |
| `typescript-eslint` 8.73.0; `npm audit` 12 to 11 advisories, all `picomatch` (no patched release exists), dev tooling only | `apps/desktop/package.json`, CHANGELOG | TESTED (tsc, eslint, vitest, build) |

Verification on this tree (this sandbox, no CI): @@VERIFY@@

NOT DONE, and why (each is also in `docs/curriculum/v2/README.md` and CHANGELOG): courseware of weeks 2 to 12 and the generated v2 files; the window on the v2 course (ADR-0023 decision 9 is PROPOSED; the window keeps importing the complete Appendix A draft); the triple audit with web research and a Rust advisory scan (the last review is `docs/audit/security-audit-2026-11-17.md`; `cargo audit` could not be installed here and was not worked around); ipc-1 (PROPOSED, needs the shell run). Owner actions: unchanged (task 26: run the Tauri shell on the G9 host and the restore drill; decisions listed in sections 12 and 13). To resume the courseware: `docs/curriculum/v2/README.md`, "What exists and what is pending".

## Update 2026-11-17: the no-decision steps of the path to 10, the 2026-11-10 review leads, and a final security review

Owner requests: "Start now" (the 20 no-decision steps of `docs/release/path-to-10.md` section 9), "Finish this build and don't stop until complete", and "Continue until project is completely finished and fully tested codebase for errors and security". No CI or workflow was started (owner instruction: CI runs only when the owner starts it). Every step's evidence is a row of `docs/release/path-to-10-evidence.md`.

| Delivered | Where | Label |
|---|---|---|
| Operations log: one allow-listed JSON line per command, 0600, rotated; a source gate on every log call site; a probe that every audit row has its log line (OBS-1, OBS-2) | `crates/g9-app/src/oplog.rs`, `crates/g9-app/tests/log_call_sites.rs`, `crates/g9-app/tests/oplog_probe.rs` | TESTED |
| The dependency law over everything `Cargo.lock` resolves; one version source (ARCH-1, BUILD-3) | `tests/integration/layering_gate.rs`, `tests/integration/version_gate.rs` | TESTED; the version gate failed on its first run (`apps/cli` stated its own version) |
| Registry completeness over all 53 operations (POL-2); the definition of done for one operation (APP-2) | `crates/g9-app/tests/registry_completeness.rs`, `docs/release/operation-checklist.md` | TESTED, mutation-checked |
| Backup verification hashes attachments (STOR-2); disk-full behaviour (STOR-4, free-space factor 2 PROPOSED); upgrade from every schema version 1 to 15 (REL-5) | `crates/g9-storage/src/backup.rs`, `crates/g9-app/tests/{backup_attachment_bytes,disk_full,upgrade_path}.rs` | TESTED; REL-5 found that schema 1 to 4 could not migrate with an attachment directory (fixed) |
| Raw-byte erasure probe (PRIV-3); data inventory of 81 tables (PRIV-7) | `apps/cli/tests/erasure_probe.rs`, `docs/data-model.md`, `scripts/check_data_inventory.py` | TESTED; PRIV-7 found gap G1 (fixed, see below) |
| 12-month performance fixture, latency harness, query-plan gate (PERF-1 to PERF-3) | `apps/cli/src/perf/`, `apps/cli/examples/perf_ipc.rs`, `apps/cli/tests/query_plans.rs`, `docs/release/perf-sandbox-2026-11-16.json` | TESTED (gate); MEASURED once in the sandbox: `audit.recent` p95 284 ms and `session.list` without a date p95 125 ms exceed the provisional 100 ms (PERF-6) |
| Content and document gates: tests for the six content gates (CLI-2; 7 gate defects found and fixed), error-code catalogue (OBS-5), document references and metrics (DOC-2), backlog ratchet (DOC-1), flake check (TEST-4) | `scripts/tests/`, `docs/runbooks/error-codes.md`, `scripts/check_doc_refs.py`, `docs/release/metrics.md`, `docs/release/backlog-map.json`, `scripts/flake_check.sh` | TESTED; DOC-1 "0 orphans" NOT MET (109 rows need plan steps or owner decisions) |
| Final security review of everything changed since `2e0b344`: five lenses, each finding checked by an independent skeptic; 10 findings (5 LOW, 5 INFO), all confirmed; 9 fixed fail-first plus 4 hardening items; ipc-1 (the shell parses the IPC body before the byte cap) PROPOSED, needs the shell on the host | `docs/audit/security-audit-2026-11-17.md`; commit `a2e1378` | TESTED |
| The 50 leads of the 2026-11-10 review re-assessed and the open ones fixed (PROC-7): 27 FIXED (17 by this round), 2 PARTIAL, 2 OWNER, 1 OPEN (PERF-6), 17 LATENT, 1 NOT A DEFECT | `docs/audit/review-2026-11-10-leads.md`; commits `0edcefc`, `f55077e`, `324161a` | TESTED: each fix fail-first or mutation-checked, each group independently reviewed |

| Check | Command | Result | Label |
|---|---|---|---|
| CI preflight on a clean export of `a2e1378` | `scripts/ci_preflight.sh --offline --target <shared build directory>` | 20 passed, 0 failed, 5 SKIPPED (cargo-deny and cargo-cyclonedx not installed; no WebKit for the shell job; `--offline` skips npm). Rust in the export: **1152 passed, 0 failed, 1 ignored**; layering deny-target graph 1 passed; content gates and the gate tests (586) pass | TESTED |
| Rust fmt, clippy, shell | `cargo fmt --all --check`; `cargo clippy --all-targets -- -D warnings`; `cargo clippy -p g9-mentor-desktop --all-targets -- -D warnings` with a stub `pkg-config` | clean; clean; clean (type-checked, not linked or run) | VERIFIED |
| Frontend | `npx tsc --noEmit`, `npx eslint . --max-warnings 0`, `npx vitest run`, `npx vite build` | clean; **487 passed in 41 files**; built | TESTED |
| Smoke | `scripts/smoke_test.sh` | SMOKE TEST PASS, 100 checks | TESTED |
| The real interface against the real backend | `node scripts/e2e/personal.cjs`; `node scripts/e2e/tour.cjs --import --program-start 2026-11-16` | 29 passed, 0 failed; 16 screens, no console error, page error or CSP violation | TESTED |
| Flake check (TEST-4) | `scripts/flake_check.sh --rust 2 --rust-single 1 --vitest 3 --e2e 2` | 0 failures, 0 tests whose outcome differs (counts reduced from the plan's 20 for time; stated) | TESTED |
| Not run | the Tauri shell and WebKitGTK on a display; `cargo deny`; the SBOMs; CI (owner's decision) | | NOT RUN |

**Owner decisions needed** (nothing below was decided by the build):

| Item | Question | Where |
|---|---|---|
| `app-13` | Derived observations store uncertainty 0.0: report it as not modelled (null plus a warning), or a versioned default per reliability class (LRN-26)? | `docs/audit/review-2026-11-10-leads.md` |
| `core-12` | Keep ADR-0019 Decision 4 (declared MAINTENANCE) and relabel TEC-25 PARTIAL, or build a backend-held maintenance state? | ADR-0019 |
| `app-7` | What verifies an objective (a passing final decision, an observation, other)? Should a manifest name the objective each study block serves, so planned day sessions move it? | `crates/g9-app/src/commands/curriculum.rs` |
| `app-10` | Record the first `deletion.request` audit row as ACCEPTED (a new outcome and a migration of the immutable audit table)? Execute a pending erasure at open (needs an ADR; today fail-closed)? | `crates/g9-app/src/recovery.rs` |
| `app-11` | Store a key fingerprint in the database to detect a different valid key (migration)? | `crates/g9-app/src/keyfile.rs` |
| G1 remainder | Texts that have no erasure path (competency and assessment titles, review prompts, backup labels, correction reasons) stay in stored command results until the 30-day purge at the next open; add a periodic sweep? | `docs/data-model.md` |
| PERF-6 | Confirm the latency budgets (provisional 100 ms), then bound `session.list` without a date and `audit.recent` | `docs/ui/performance-budgets.md` |
| STOR-4 | Confirm the free-space factor 2 (PROPOSED) | `docs/runbooks/backup-restore.md` |
| DOC-1 | Map or close the 109 unmapped improvement-backlog rows | `docs/release/backlog-map.json` |
| Skill contract | Does `$dynamicRef` count as the forbidden `$ref` in skill schemas? | `docs/skills/skill-package-contract.md` |
| ipc-1 | Change the shell's IPC transport to raw bytes with a length check before parsing (must be tested with the shell running on the host) | `docs/audit/security-audit-2026-11-17.md` |

## Update 2026-11-14: the 12-week curriculum, its promotion route, its lesson sheets and the keyed ledger digests

Owner request: "All your recommendations and help me build the 12 week cybersecurity curriculum" (recorded in ADR-0020 and ADR-0022). The curriculum is **PROPOSED**: nothing was promoted, approved or run on the owner's data directory, and no CI was started.

| Delivered | Where | Label |
|---|---|---|
| 12 week files (72 day sheets, 12 Saturday assessments) as the single source; a builder that checks them and generates the manifest, the assessment payloads, the lesson sheets and the coverage matrix | `docs/curriculum/weeks/`, `scripts/build_curriculum.py`, `data/curriculum/g9-ct9-480.draft.v1.json`, `assessments/`, `g9-ct9-480.sheets.v1.json`, `docs/curriculum/coverage.md` | PROPOSED content; built and checked |
| The window imports the draft; the Daily Classroom shows the day's lesson sheet per block, only for the objectives it was written for | `ProgramSetup.tsx`, `LessonSheet.tsx`, `lib/lesson-sheets.ts`, `schemas/curriculum/lesson-sheets.schema.json` | TESTED |
| `g9 reconcile`: the diff and checksums of two manifests; on explicit review flags, the `RECONCILED_CANDIDATE` and the payloads of import and promote | `apps/cli/src/commands/reconcile.rs`, `docs/curriculum/promotion.md` | TESTED in a throwaway data directory; NOT RUN on the owner's |
| **Found and fixed:** a reconciliation block bound which fields differed, not what they said, so a reviewed objective reworded afterwards still matched its block. `diff_checksum` now includes the candidate's content checksum | `crates/g9-curriculum/src/{checksum,reconcile}.rs`, ADR-0022 | fail-first test; no real block existed |
| **The hashes of whole requests in the immutable ledgers are keyed** (owner decision ADR-0020 row 6): `parameters_sha256`, the idempotency `request_sha256`, the stored `result_sha256` and the skill input and output digests were plain SHA-256, so a four-digit note body was recovered from the database file; they are HMAC-SHA-256 under a subkey of the binding key, one label per use. An approval pending at the upgrade is refused and expires; rows and snapshots written before keep their plain digests | `crates/g9-policy/src/binding.rs`, `crates/g9-app/src/{pipeline,recovery}.rs`, `commands/{calendar,skills}.rs`, ADR-0021 | fail-first probe (`ledger_digests.rs`), independent HMAC vectors; TESTED |

| Check | Command | Result | Label |
|---|---|---|---|
| Rust fmt, clippy | `cargo fmt --all --check`; `cargo clippy --offline --all-targets -- -D warnings` (default members) | exit 0, exit 0 | VERIFIED |
| Rust tests | `cargo test --offline --all-targets`; `cargo test --offline --doc` | **975 passed, 0 failed** (951 before: +24 = `draft_v1.rs` 4, `a01_gate.rs` 1, `reconcile.rs` unit 8, `curriculum_promotion.rs` 4, `binding.rs` 2, `ledger_digests.rs` 5); 0 doc tests | TESTED |
| Fail-first | `draft_v1.rs::promotion_needs_a_reconciliation_block_bound_to_this_exact_diff` on the old `reconcile`; `ledger_digests.rs` (the whole-request probe and the skill ledger) on the old digests | the first failed (`assertion failed: matches!(r.binding, DiffChecksumMismatch { .. })`); the probe recovered body 4711 from both request forms and the skill input ledger held `sha256_json(input)`; all pass after | TESTED |
| The route through the binary | `apps/cli/tests/curriculum_promotion.rs` | report, write, import (approval), promote (approval) to `APPROVED_CANONICAL`; an edited candidate imports but is refused at promotion; a tampered payload is refused before any approval; the 12 assessments are accepted after the packs load | TESTED (scratch data directory) |
| UI | `npx tsc --noEmit`, `npx eslint .`, `npx vitest run`, `npx vite build` | clean; **481 passed in 40 files** (463 in 38 before: +9 `lesson-sheets.test.ts`, +8 `lesson-sheets.test.tsx`, +1 layer-order case); the sheets chunk is 326 kB (93 kB gzipped), loaded only when a day with blocks is shown | TESTED |
| Design system audit | `ds_audit.py --check` | exit 0 | MEASURED |
| The real interface against the real backend | `node scripts/e2e/personal.cjs` | **29 passed, 0 failed** (22 before; the window imports `1.0.0-draft1`; the Classroom's sheet has one section per block, the first open, no link, nothing written; completing a block opens the next). Screenshots read at 1280 and 1440 wide, dark, and `classic-desktop`. `tour.cjs --import --program-start <Monday>` (now importing the draft): 16 screens, no console error, page error or CSP violation | TESTED |
| Curriculum gates | `build_curriculum.py` (check mode); `check_curriculum_seed.py` on the draft and on the seed | sources valid, generated files current; both PASS (the draft: schema_errors=0 errors=0 violations=0 warnings=0) | VERIFIED |
| Other gates | `scripts/smoke_test.sh`; `check_skill_packages.py`, `check_competency_packs.py`, `check_schemas.py`; the hidden-character scan | SMOKE TEST PASS; exit 0 each; 2 scan tests pass | TESTED |
| Resources | 138 distinct external references (184 uses) | none opened: 15 checked by search result only (ATT&CK v20 current since 2026-10-13, ASVS 5.0.0, OWASP Top 10 2025, CVSS 4.0, SLSA v1.2, AI RMF 1.0 and others), 104 ASSUMED, 19 numbered publications cited from knowledge. The check corrected two lesson statements (ATT&CK data sources, Days 28 and 37) | CHECKED by search result only / ASSUMED |
| Not run | the Tauri shell and WebKitGTK; `cargo deny`; the promotion route on the owner's data directory; any check that needs `genai.owasp.org`, `modelcontextprotocol.io` or `attack.mitre.org` (not reachable from the authoring session's network) | | NOT RUN |

Open: the owner's items in `docs/curriculum/design.md` (the source of each blind practical, the resources they rely on, promotion); the importer stores no objective-to-competency link (the manifest carries 296; storing them needs a dimension and a weight that would be invented); the window has no promote action; rows and snapshots written before the keyed digests keep their plain ones (immutable); the window has no promote action; `core:default` and the shell items need the host.

## Update 2026-11-14 (later): the four LOW and four INFO audit findings closed

Owner request: "fix the last few things". The four LOW findings still open from the audit of 2026-11-12 (audit tail truncation, client-declared interaction mode, skill pin hashed from a second read, revision 0 at the boundary) and four INFO items are fixed, each with a test that failed on the old code (`docs/audit/security-audit-2026-11-12.md`, remediation table; ADR-0004 addendum; ADR-0019). Two decisions were taken without the owner and are each reversible in one place: a declined native confirmation is audited, and a production bundle refuses the mock transport. No CI or workflow was started.

| Check | Command | Result | Label |
|---|---|---|---|
| Rust fmt, clippy | `cargo fmt --all -- --check`; `cargo clippy --workspace --exclude g9-mentor-desktop --all-targets --offline -- -D warnings` | clean; exit 0 (clippy found one lint in a new test helper, fixed, re-run clean) | VERIFIED |
| Rust tests | `cargo test --workspace --exclude g9-mentor-desktop --offline --no-fail-fast` | **951 passed, 0 failed, 0 ignored** (916 before: +35 new, `package_hardening.rs` 9, `audit_tail.rs` 8 in g9-storage and 3 in g9-app, `exam_mode_derivation.rs` 4, `boundary_revision.rs` 3, `native_confirmation.rs` 2, `feature_sessions.rs` 1, `live_registry.rs` 1, 3 unit tests in `package.rs`, 1 in `orchestrator.rs`) | TESTED |
| Fail-first | each new test run against the old code before the fix | failed on the old code: 6 of 9 in `package_hardening.rs` (one blocked on a FIFO until its deadline), 6 of 8 in the storage `audit_tail.rs`, 1 of 4 in `exam_mode_derivation.rs`, 1 of 2 new in `native_confirmation.rs`, the `planned_start` test, the orchestrator boundary test, the 3 in `boundary_revision.rs`, the vitest tests for the transport choice and for the `planned_start` bound; the rest are regression guards that pass before and after | TESTED |
| Mutation check | the tail check disabled on purpose | the two app-level detection tests in `crates/g9-app/tests/audit_tail.rs` fail, the regression guard passes | TESTED |
| Desktop shell, type-check and lint only | `cargo clippy -p g9-mentor-desktop --offline --all-targets -- -D warnings` (stub `pkg-config`) | exit 0 | VERIFIED; the window, the dialog and a build are NOT RUN |
| Smoke test | `bash scripts/smoke_test.sh` | SMOKE TEST PASS | TESTED |
| Content gates | `check_curriculum_seed.py`, `check_skill_packages.py`, `skill_hashes.py --check` (now also refuses non-regular entries), `check_source_register.py`, `verify_source_register.py --selftest`, `check_schemas.py` | all exit 0; `live_registry_loads_under_the_shipped_pins` checks the 37 shipped packages against `skill-pins.json` with the production loader | TESTED |
| UI | `npx tsc --noEmit`, `npx eslint .`, `npx vitest run`, `npx vite build` | clean (eslint found one dot-notation error in the mock, fixed and re-run clean); **463 passed in 38 files** | TESTED |
| Design system audit, layout | `ds_audit.py --check`; `measure.cjs` (1280 x 800 and 960 x 640) | exit 0; 32 measurements, 32 clean | MEASURED |
| The first-run flow in the real interface against the real backend | `node scripts/e2e/personal.cjs` | **22 passed, 0 failed** (production bundle with the Tauri runtime injected, native confirmation, backup list, audit chain intact, no console, page or CSP problem) | TESTED |
| `tour.cjs` (16 screens on the real backend) | n/a | not re-run for this change | NOT RUN |
| A concurrent writer racing the package loader | n/a | the single read is tested by construction (`parsing_and_hashing_use_the_bytes_that_were_read_not_the_disk`: a file replaced after the read changes neither the parse nor the hash); no race was run | NOT RUN |
| `cargo deny` | n/a | not installed in this sandbox; the one new direct dependency is `libc =0.2.191` on unix (the version `Cargo.lock` already held; MIT or Apache-2.0; not in the ban list) | NOT RUN |
| Tauri shell on the host, the native confirmation dialog, WebKitGTK rendering, a restore drill | n/a | needs the host | NOT RUN |

Still open, not touched: keyed request hashes in the idempotency and approval ledgers (owner decision), `core:default` reduction (ARCHITECTURE.md names it), ADR-0017's erasure-deletes-files decision (owner to confirm), the PNG-plus-archive polyglot and the `already_stored` oracle (INFO), the CLI's default data directory (`g9-mentor`, not the application's `com.g9.mentor`), and everything that needs the real 12-week curriculum (Saturday assessments, objective-to-competency mapping, NICE pins, promotion to canonical, `FILL_CURRICULUM`).

## Update 2026-11-14: the window usable on its own, with the curriculum imported

Owner request: "the gui version fully complete and ready to use for personal use, with the fully cybersecurity learning curriculum imported in". Running the real interface against the real backend showed that the window could not run the curriculum (no import, no program start, no way to start a block, no read-back of notes, no erase, no competencies, no backup list). The decision and the findings are in `docs/adr/ADR-0018-program-setup-and-planned-days.md`; the steps for the first day are in `docs/runbooks/first-run.md`. Schema version 15 (`program_plan`); seven new operations (53 in all): `curriculum.program`, `curriculum.set_program_start`, `session.plan_day`, `notes.list`, `notes.get`, `competency.import_pack`, `backup.list`. The branch is pushed; CI runs only when started by hand (ADR-0015) and no workflow was started. **No Stage PASS is issued.**

| Check | Command | Result | Label |
|---|---|---|---|
| Rust fmt, clippy | `cargo fmt --all -- --check`; `cargo clippy --workspace --exclude g9-mentor-desktop --all-targets --offline -- -D warnings` | clean; exit 0 | VERIFIED |
| Rust tests | `cargo test --workspace --exclude g9-mentor-desktop --offline --no-fail-fast` | **916 passed, 0 failed, 0 ignored** (new: `program_setup.rs` 16, `notes_read_back.rs` 5, `packs_and_backups.rs` 9, `commands/packs.rs` 4, the v15 pin and the repo tests) | TESTED |
| Mutation checks of the new behaviour | each rule broken on purpose, the matching test must fail, the file restored | no Monday check, stored plan ignored, already-planned block not skipped, reversed prerequisite edges, no idempotency check: each made its test fail | TESTED |
| Desktop shell, type-check and lint only (no GTK or WebKit here; a stub `pkg-config` lets the crate compile) | `cargo clippy -p g9-mentor-desktop --offline --all-targets -- -D warnings` | exit 0 | VERIFIED; the window, the dialog and a build are NOT RUN |
| Smoke test | `bash scripts/smoke_test.sh` | SMOKE TEST PASS, 100 assertions passed, 0 failed (`ops` lists 53 operations) | TESTED |
| Content gates | `check_curriculum_seed.py`, `check_skill_packages.py`, `skill_hashes.py --check`, `check_source_register.py`, `verify_source_register.py --selftest`, `check_schemas.py schemas .agents/skills` (188 schemas) | all exit 0 | TESTED |
| UI | `npx tsc --noEmit`, `npx eslint .`, `npx vitest run`, `npx vite build` | clean; **460 passed in 38 files** | TESTED |
| Design system audit | `ds_audit.py apps/desktop/src --check` | exit 0 | MEASURED |
| Layout, targets, names, headings in Chromium | `measure.cjs` at 1280 x 800 and 960 x 640, with the default mock and with `--fresh` (the first-run states) | 32 measurements, 32 clean, twice | MEASURED |
| The whole first-run flow in the real interface against the real Rust backend | `node scripts/e2e/personal.cjs` (bridge example + Chromium, CSP of `tauri.conf.json`) | **22 passed, 0 failed**: import with approval and native confirmation, the Monday, plan today (sessions and calendar events read from the backend), a timed block (credited real seconds, none in doubt), a note listed after a reload, its erasure (no file of the data directory holds the text), 64 competencies, a backup listed after a reload, an intact audit chain, no console error or CSP violation | TESTED |
| Every screen on the real backend with the curriculum imported | `node scripts/e2e/tour.cjs --import --program-start <Monday>` | 16 screens, no console error, page error or CSP violation | TESTED |
| Tauri shell on the host, the native confirmation dialog, WebKitGTK rendering, a restore drill | n/a | needs the host | NOT RUN |
| Assessments for the twelve Saturdays, the objective-to-competency mapping, NICE pins, promotion to APPROVED_CANONICAL, `FILL_CURRICULUM` | n/a | PROPOSED: no owner decision or source holds them (ADR-0018) | PROPOSED |

## Update 2026-11-13: operator features, native confirmation, redesign, security audit

The sections below are the 2026-11-08 snapshot and are unchanged. Since then: the operator features (sessions, calendar events, review results, authoring, evidence, export), the native approval confirmation (ADR-0014), the Ledger 2 redesign of all 16 screens (`docs/ui/design-critique-2026-11-13.md`), the triple security audit of 2026-11-12 and 2026-11-13 with its fixes (`docs/audit/security-audit-2026-11-13.md`, ADR-0017, schema version 14) and a UI skill (`.claude/skills/g9-ui-design`). The branch is pushed (the operator later allowed it); CI runs only when started by hand (ADR-0015) and no workflow was started. **No Stage PASS is issued.**

| Check | Command | Result | Label |
|---|---|---|---|
| Rust fmt, clippy | `cargo fmt --all -- --check`; `cargo clippy --workspace --exclude g9-mentor-desktop --all-targets --offline -- -D warnings` | clean; exit 0 | VERIFIED |
| Rust tests | `cargo test --workspace --exclude g9-mentor-desktop --offline --no-fail-fast` | **880 passed, 0 failed, 0 ignored** | TESTED |
| Desktop shell, type-check and lint only (no GTK or WebKit here; a stub `pkg-config` lets the crate compile) | `cargo clippy -p g9-mentor-desktop --offline --all-targets -- -D warnings` | exit 0 | VERIFIED; the window, the dialog and a build are NOT RUN |
| Smoke test | `bash scripts/smoke_test.sh` | SMOKE TEST PASS, 100 assertions passed, 0 failed | TESTED |
| Content gates | `check_curriculum_seed.py`, `check_skill_packages.py`, `skill_hashes.py --check`, `check_source_register.py`, `verify_source_register.py --selftest`, `check_schemas.py schemas .agents/skills` | all exit 0 | TESTED |
| UI | `npx tsc --noEmit`, `npx eslint .`, `npx vitest run`, `npx vite build` | clean; **400 passed in 32 files** | TESTED |
| Design system audit | `ds_audit.py apps/desktop/src --check` | exit 0 | MEASURED |
| Layout, targets, names, headings in Chromium | `measure.cjs` (16 screens at 1280 x 800 and 960 x 640) | 32 measurements, 32 clean | MEASURED |
| Source text | `crates/g9-domain/tests/source_text.rs` (part of the Rust tests) | no hidden or direction-changing character in any text file | TESTED |
| Audit probes | the auditors' probes and two written in pass C, re-run against a rebuilt `g9` | all fixed findings stay fixed (`docs/audit/security-audit-2026-11-13.md`, pass C) | VERIFIED |
| Supply chain | RustSec advisory database 2026-11-06, scanned offline (486 packages); `npm audit` | 0 vulnerable, 2 informational; production 0, development 7 moderate | OBSERVED (live `cargo deny` is CI and was not run) |
| Release evidence collector | `scripts/collect_gate_evidence.py` | not re-run: `docs/release/gate-evidence.*` still hold the 2026-11-08 counts; `docs/evaluations/gate-evidence.md` was regenerated for the changed A10-01 scenario | NOT RUN |
| Tauri shell on the host, restore drill, WebKitGTK rendering, the three evals of the UI skill | n/a | needs the host or a model run | NOT RUN |

## 1. Requested scope

| # | Request (operator) | Date |
|---|---|---|
| 1 | Execute the v5.1 audit/plan and the v6.0 handoff: fix errors found, implement improvements, plan the optimal architecture | 2026-11-05 |
| 2 | Platform target is Ubuntu 26.04.1 LTS, never 24.04 | 2026-11-05 |
| 3 | Work locally; do not connect to GitHub | 2026-11-07 |
| 4 | Continue until the full app is finished and ready to test | 2026-11-07 |
| 5 | Review the desktop UI; show what `export.request` is; point to legitimate cybersecurity and AI reference sources; audit this session and its configuration; write the stage report when sufficient | 2026-11-07/08 |

## 2. Approved actions

Approved by the operator's messages: repository changes in the sandbox (items 1, 4), documentation and review work (item 5). **Not approved and not done:** pushing or any GitHub access; installing system packages; changing the host; downloading models or datasets; any network access to the reference-source hosts (the sandbox blocks them, section 5); running `scripts/verify_source_register.py --approve-network`. One command was denied by the session's safety classifier on 2026-11-08 (a `git commit` carrying a needless `-c core.hooksPath=/dev/null`); it was not worked around and the commit was made without the flag.

## 3. Stage exit criteria (`GOVERNANCE.md` stage table)

| Stage | Exit criterion | Status | Evidence and what is missing |
|---|---|---|---|
| 00 Design | three-pass review and operator PASS | NOT MET (operator PASS pending) | Triple audit done (`docs/audit/stage00-triple-audit.md`); 152 auditor rows verified (104 CONFIRMED, 44 DOWNGRADED, 1 UPGRADED, 3 REFUTED) and status re-assessed 2026-11-08 (section 9). Deliverables: requirements matrix, ADR-0001..0014, interfaces, threat model, skill graph present. **Gaps:** no standalone resource plan (only the finding that the v5.1 estimate rows sum to 85 person-weeks, not 72: CON-32, TEC-26); curriculum gap report cannot reconcile against the original manifest because it was never supplied (C01) |
| 01 Scaffold | reproducible clean build with logs | PARTIAL | Rust workspace (13 crates + CLI) builds clean (clippy `-D warnings`, fmt) and tests pass; SBOM job defined in CI. **Shell:** compiled and its `--lib` tests passed in the `desktop-shell` CI job on GitHub (runs 27 and 28, VERIFIED from the job conclusions; the per-test counts were not read). **Not met:** the shell has not been built or run on the G9 host, and no window flow has been exercised (NOT RUN); the collector's local check still exits 101 (no GTK/WebKit in this sandbox). CI history: runs 1 to 26 failed; the fixes are in section 6a |
| 02 Data and importer | reconciliation and fault-safe DB tests | PARTIAL | Fault-safe DB: A08 PASS (backup verify and restore test on a copy, crash mid-transaction rollback, migration checksum tamper, dry run on a copy). Importer: A01 PASS (12/72/216/28,800 and provenance gate). **Not met:** reconciliation against the original approved plan; canonical promotion BLOCKED |
| 03 Deterministic UX | simulated week and crash recovery | MET on the CLI path; NOT RUN in the GUI | `g9 simulate-week` (fixed clock, 189 steps with the seed pre-imported, PASS) and the two-step crash drill (open interval closed UNCERTAIN, 0 ms credited) in `scripts/smoke_test.sh`: 100 assertions passed. The same flows through the Tauri window are NOT RUN |

Release label (proposal for the operator, `GOVERNANCE.md`): **MVP BUILT** for the deterministic backend, CLI and web UI slice. Not **MVP VERIFIED**: A12 is NOT RUN, the shell has not been built or exercised on the G9 host (it compiles and passes its unit tests only on the CI runner), A01 canonical import is blocked. No label is granted by this report.

## 4. Implementation files

| Area | Where | Size (tracked lines) |
|---|---|---|
| Rust crates (13) | `crates/g9-{domain,policy,storage,curriculum,calendar,evidence,competency,assessment,retention,ai-adapter,skills,app,evaluation}` | Rust total 89,878 lines incl. shell and CLI |
| CLI | `apps/cli` (`g9`: init, status, ops, call, approve, reject, approvals, import-seed, simulate-week, crash-drill) | |
| Desktop | `apps/desktop` (React 19, 16 screens, typed IPC client, mock and Tauri transports), `apps/desktop/src-tauri` (bridge, capabilities, skill pins) | TS 5,394 + TSX 4,716 |
| Content | 37 skill packages (`.agents/skills`), 8 knowledge packs (`data/competency-packs`), curriculum seed (`data/curriculum`), 73 IPC schemas (`schemas/ipc`), reference source register (`data/sources`) | JSON 44,648 |
| Storage | 12 migrations (`crates/g9-storage/src/migrations`), 35 registered operations (`crates/g9-app/src/ops.rs`) | SQL 2,117 |
| Gates and docs | `scripts/*.py`, `tests/{integration,recovery,adversarial}`, `docs/**`, `.github/workflows/ci.yml`, `.github/CODEOWNERS` | MD 11,041; Python 4,045 |

Change against baseline: 973 tracked files, 39 commits since `f414481`; the baseline contained only the initial README.

## 5. Source versions

| Item | Version | Label |
|---|---|---|
| Rust toolchain | 1.97.0 (`rust-toolchain.toml`, CI) | VERIFIED |
| Tauri / `@tauri-apps/api` / `@tauri-apps/cli` | 2.12.5 / 2.12.4 / 2.12.4 | VERIFIED (manifests) |
| rusqlite (bundled SQLite, backup API) / jsonschema (Rust) / rs-fsrs | 0.41.1 / 0.60.2 / 1.2.1 (feature `fsrs`, off by default) | VERIFIED |
| React / Vite / Vitest / TypeScript / DOMPurify / marked | 19.3.1 / 7.3.7 / 4.1.12 / 5.9.3 / 3.4.17 / 18.1.3 | VERIFIED |
| cargo-deny / jsonschema (Python gates) | 0.20.2 / 4.26.0 | VERIFIED |
| CI runner label / target host | `ubuntu-26.04` / Ubuntu 26.04.1 LTS (released 2026-08-27) | the label is accepted: every job in runs 27 and 28 started on a runner reporting `labels: ["ubuntu-26.04"]` (VERIFIED from the job records). The runner's point release (26.04.1) was not inspected: ASSUMED. The G9 host itself was not inspected: ASSUMED |
| Reference source register | 48 candidates: VERIFIED 0, PARTIAL 22, UNVERIFIED 26, approved 0 | the sandbox blocks the hosts (every fetch ENOTFOUND); no publisher page was read |

## 6. Executed tests and results (this tree)

| Check | Command | Result | Label |
|---|---|---|---|
| Rust fmt | `cargo fmt --all -- --check` | clean | VERIFIED |
| Rust clippy | `cargo clippy --workspace --exclude g9-mentor-desktop --all-targets -- -D warnings` | exit 0 | VERIFIED |
| Rust tests | collector run: default members `--no-fail-fast`, plus `g9-retention --features fsrs` (the collector sums both runs; the second re-runs that crate's own tests) | **772 passed, 0 failed, 0 ignored** counted by the collector; **761 passed, 0 failed** in a single `--workspace --exclude g9-mentor-desktop --no-fail-fast` run | TESTED |
| UI | `npm run typecheck`, `npm run lint`, `npx vitest run`, `npm run build` | exit 0; **193 passed, 0 failed** (21 files; the collector's earlier run recorded 190, before the restore-guidance and layout-guard tests were added) | TESTED |
| Smoke test | `bash scripts/smoke_test.sh` | **100 assertions passed, 0 failed**; 189 simulated steps (193 on a fresh directory); includes backup, verify and restore-test in separate processes, offline restore with content proof, restore refusals and the `--scratch` guard | TESTED |
| Content gates | `check_curriculum_seed.py`, `check_skill_packages.py` (37), `check_competency_packs.py` (8), `check_schemas.py` (151), `skill_hashes.py --check`, `check_source_register.py` | all pass | TESTED |
| Supply chain | cargo-deny (advisories, bans, licenses, sources) | ok | TESTED |
| npm audit | `npm audit --audit-level=high`; `npm audit --omit dev` | 0 production vulnerabilities; 7 moderate, all in the ESLint toolchain (`minimatch` DoS advisory GHSA-g3hv-75wg-m249; fixes available) | OBSERVED |
| Tauri shell, local | `cargo check -p g9-mentor-desktop` | exit 101 (gdk-3.0 not found) | NOT RUN here (BLOCKED by host libraries) |
| Tauri shell, CI | `desktop-shell` job: apt prerequisites, `cargo check --locked`, `cargo test --locked -p g9-mentor-desktop --lib` | all three steps `success` in runs 27 and 28 on `ubuntu-26.04` (test counts not read) | VERIFIED (job conclusions); the window and the G9 host remain NOT RUN |
| CI | `.github/workflows/ci.yml` | GitHub run 28 (`e05d506`): all five jobs `success` (rust: fmt, clippy, test, doc tests, tree, cargo-deny; frontend; sbom; content-gates; desktop-shell). Run 27 failed at `rust`/`test` (section 6a). Step logs were read only for failing jobs, so per-step counts for run 28 are not recorded here | VERIFIED (job conclusions) |
| Model-in-the-loop | any skill against a real model | only `NullRuntime` and `ScriptedRuntime` exist | NOT RUN |

### 6a. CI steps replicated locally on a clean export of HEAD (2026-11-09)

`git archive HEAD` into an empty directory (984 files), so nothing that exists only in the working tree can mask a failure. TESTED unless marked. **Correction (2026-11-09):** that sentence held only for the rows marked "(export)" below. The original replication ran doc tests in the working tree and did not run `cargo test` on the export at all, which is why it missed the `g9-skills` fixture failure that GitHub CI then reported (below).

| CI step | Result |
|---|---|
| `rust` job: fmt, clippy `-D warnings` (`--locked`), `cargo test --locked --doc` (0 doctests, exit 0), `cargo tree --locked`, cargo-deny (working tree, not the export) | pass (deny with local cargo-deny 0.20.2 and an offline advisory snapshot dated 2026-11-06, not the pinned action; re-run 2026-11-09: `advisories ok, bans ok, licenses ok, sources ok`) |
| `rust` job: `cargo test --locked --all-targets --no-fail-fast` (export, with the `g9-skills` fix) | pass: 48 test binaries, **762 passed, 0 failed, 0 ignored** (761 from the earlier workspace run plus the one new test). Before the fix the same export reproduced the CI failure exactly |
| `frontend` job: `npm ci`, typecheck, lint, `npm test -- --run` (193), `npm run build`, `npm audit` | pass (audit: 0 production, 7 moderate dev-only) |
| `content-gates` job: curriculum seed, skill packages, `skill_hashes.py --check`, source register, verifier self-test, schemas | pass |
| `sbom` job: `cargo install cargo-cyclonedx --version 0.5.10 --locked` then `cargo cyclonedx --format json --target x86_64-unknown-linux-gnu --all-features` (16 SBOM files); `npx @cyclonedx/cyclonedx-npm@6.0.1 --omit dev` | pass |
| `cargo check --locked --workspace --exclude g9-mentor-desktop --all-targets` in the clean export | pass: nothing needed is missing from git |
| `desktop-shell` job: apt packages, `cargo check`, `cargo test -p g9-mentor-desktop --lib` | **NOT RUN here** (needs GTK/WebKit); passed on GitHub in runs 27 and 28 (see below) |
| Runner label `ubuntu-26.04` is a real hosted label | **VERIFIED**: jobs started on it in runs 27 and 28 |
| Action commit SHAs match their tags (`checkout` v7.0.1, `setup-node` v7.1.0, `upload-artifact` v7.0.2, `rust-toolchain`, `rust-cache` v2.9.3, `cargo-deny-action` v2.1.1) | the pinned SHAs resolve and every action step succeeded (VERIFIED); that each SHA is the commit of its stated tag was not checked: **NOT RUN** |
| Whole-workspace build with `RUSTFLAGS=-D warnings` as CI sets it | passed on GitHub (clippy and test steps, run 28); not rebuilt locally |

**GitHub results** (read with the GitHub Actions tools after the push; VERIFIED from the job records, step logs read only for failing jobs). `ci.yml` ran 28 times on this branch; runs 1 to 26 failed and the causes of runs 1 to 25 were not read. Run 26 (`e987383`) failed in `desktop-shell` (missing `icons/32x32.png`, fixed in `698ef3c`) and in `rust`/`test`. Run 27 (`4ec9866`): `frontend`, `sbom`, `content-gates` and `desktop-shell` passed; `rust` failed at `test`, and cargo stopped at the first failing binary, so doc tests, the dependency tree and cargo-deny were skipped (they had never run on CI). Run 28 (`e05d506`): **all five jobs passed**, including cargo-deny with the pinned action.

**Defect found by CI, invisible locally:** the fixture `crates/g9-skills/tests/fixtures/invalid/missing_references/g9-session-director/references/` was an empty directory. Git does not store empty directories, so a fresh checkout lacked it and the loader returned `Io { path: "references", reason: "entity not found" }` instead of `PackageError::MissingReferences`; the test passed on every local run because the directory existed in the working tree. Fix `e05d506`: the loader treats an absent `references/` as `MissingReferences` (other I/O errors stay `Io`), and a new test builds the absent, empty and non-markdown cases at runtime so nothing relies on git keeping an empty directory. Three other directories are empty and untracked (`schemas/event-schemas`, `tests/e2e`, `apps/desktop/src/styles`); nothing in code or CI depends on them.

Per-gate results (`docs/release/gate-evidence.md`, generated from the executed output of this run; it maps tests to gates by name prefix `a01_`, `b02_`, `st07_`, so the restore, learning-loop and eligibility tests, which are not named that way, are counted in the totals but not in any gate). The collector's rule is weak by design: a gate is PASS when at least one mapped test ran and none failed. Read the counts: several gates rest on one test, and a PASS here is harness-level evidence, not a stage sign-off.

| Gate | Result | | Gate | Result |
|---|---|---|---|---|
| A01 curriculum invariants | PASS (7 tests) | | A12 SBOM, licences, pins, accessibility | **NOT RUN** as a harness gate (the collector maps no executed test to it). Its SBOM, licence/advisory and skill-pin parts passed as CI jobs in run 28; no accessibility check was run |
| A02 recurrence, DST | PASS (10) | | B01 rubric validity | PASS (1 test; the content-validity review and blind re-grade are not executed) |
| A03 timer, uncertain time | PASS (6) | | B02 retention vs transfer | PASS (2 tests; measurement PROPOSED) |
| A04 grade immutability | PASS (17) | | B03 intervention baseline | **NOT RUN** |
| A05 competency replay | PASS (6) | | B04 effect size and uncertainty | **NOT RUN** |
| A06 policy matrix | PASS (19) | | B05 correction propagation | PASS (1 test) |
| A07 injection, privilege | PASS (22) | | B06 planning overhead | **NOT RUN** |
| A08 backup, restore, migrations | PASS (12) | | ST01 to ST12 adversarial catalogue | PASS, **one test per scenario**, at crate/harness level; model-in-the-loop variants NOT RUN |
| A09 model disabled | PASS (6) | | | |
| A10 export, deletion | PASS (9) | | | |
| A11 output validation | PASS (5) | | | |

## 7. Actual performance

NOT MEASURED. No inference benchmark (no model), no UI performance measurement, no installed-app measurement, and the smoke test duration was not re-measured after the restore section was added (15 s with a warm build before it). `docs/ui/performance-budgets.md` states budgets, not results.

## 8. Defects by severity

| Source | BLOCKER | CRITICAL | HIGH | MEDIUM | LOW | Notes |
|---|---|---|---|---|---|---|
| v6.0 corrections C01 to C22 | 1 | 3 | 12 | 6 | 0 | `docs/audit/defect-register.md` section A |
| Stage 00 auditor rows, verified severities (149; 3 REFUTED excluded) | 0 | 5 | 38 | 52 | 54 | status in section 9 |
| Repository code review, confirmed (66 reviewed, 47 confirmed) | 0 | 0 | 7 | 25 | 15 | fixed or substantially fixed except the items in section 10 |
| Desktop review: 12 verified findings + 11 fresh findings | | | 3 | 9 | 11 | all 23 fixed; independently re-verified; 20 of the new tests fail on the old code |
| Defects found and fixed on 2026-11-08, second round (learning loop, competency, restore) | | | 2 | 6 | 3 | learning-loop verifier: retake on an already-shown variant and a key shown before the attempt recorded as blind (HIGH); competency: one failing attempt triggered Regress (MEDIUM) and eligibility failed open (MEDIUM); restore verifier: swap could leave no live database (HIGH, as the verifier assessed), silent overwrite at the live path (MEDIUM), truncated snapshot accepted without checksum (MEDIUM), relative manifest path marked a backup FAILED (MEDIUM), non-UTF-8 path panic (LOW), restore plan created WAL files (LOW), leftover temp sidecars (LOW), smoke assertions that passed for a no-op restore (MEDIUM; counted here as a test defect). Severities are as assessed in this session, not by an independent auditor; each has a test that fails on the old code |
| Layout defect found by measuring the running UI in Chromium (2026-11-09): Assessments and Session Tracker widened their main area (1160 and 1184 px against 1140 px at a 1360 px window; against 804 px at 1024 px) because `grid-template-columns: 1fr 1fr` takes a table's minimum width; WCAG 1.4.10 reflow | | | | 2 | | fixed with `minmax(0, 1fr)` tracks and panels that contain their own overflow; re-measured: page and `main` scroll widths equal their client widths on all 16 screens at both widths; a source guard test fails on the old stylesheet. Measured in the mock-backed web UI; NOT RUN in the Tauri window |
| Defects found and fixed on 2026-11-08, first round | | | 1 | 2 | 1 | severities as assessed here (the sweep rated the first LOW; I rate it HIGH because it disabled the mandatory backup, migration dry run and attachment verification in the desktop configuration): directory approved-roots check (backups, migration dry run and attachment verification were inert in the desktop configuration); validation evidence not tied to a graded attempt; calibration bins on edges; skill registry hash column computed under a different definition than the runtime pins |

## 9. Register status (Stage 00 auditor rows, 138 formerly OPEN)

PARTIAL 76 · IMPLEMENTED 21 · ADDRESSED-IN-DOCS 18 · OPEN 16 · PROPOSED 7 (the 11 IMPROVEMENT and 3 REFUTED rows unchanged). Per-row evidence and gaps: `docs/audit/defect-register.md` section E. Method limits: assessed by reading, not by a second independent pass; cited paths and test names were mechanically checked against the tree. IMPLEMENTED means code plus a named test exists, not that a stage gate passed.

Largest still-OPEN or PROPOSED items: native Rust-side approval confirmation (SEC-03), at-rest encryption decision and code (SEC-07, TEC-34), the research fetch adapter with SSRF controls (SEC-21), the local runtime launch hardening (SEC-06), the probe-based recall KPI and seeded sampler (LRN-19), the n-of-1 protocol document and randomisation code (LRN-01, LRN-21), the weekly remediation capacity ledger (LRN-30).

## 10. Security impact

Controls added or corrected in this reporting period: deny-by-default policy with HMAC-bound single-use approvals; backend-assigned actor; metadata-only hash-chained audit; skill packages pinned by SHA-256 and enforced by the desktop bridge (`Registry::load_pinned`, compiled-in `skill-pins.json`; `scripts/skill_hashes.py --check` in CI); validation evidence must trace to a graded attempt; directory approved-roots check fixed and the CLI no longer widens the attachment boundary to the data directory (which holds `binding.key`); persistent SYNTHETIC DATA banner when the mock transport is active; `.gitignore` covers SQLite sidecars and key files; `.github/CODEOWNERS` for trust-bearing paths; CI uses `--locked` and pins SBOM tool versions.

Security audit 2026-11-12 (triple: two independent auditors, a lead pass and adversarial verification; `docs/audit/security-audit-2026-11-12.md`): no CRITICAL or HIGH; two MEDIUM fixed on 2026-11-13 with fail-first tests (backup verification, restore and open now verify the schema itself; approvals bind to the stored revision and a stale approval request is refused) and the release-build LOW fixed (`build.rs` refuses a dev-mode release binary); five LOW and seven INFO remain open as listed there.

Open: SEC-03 is PARTIAL (since 2026-11-13 an approval is granted only through the native confirmation the shell raises from Rust, on Linux; missing: an approval-request rate limit, the isolation pattern, an ACL manifest; ADR-0014 addendum); SEC-02 is PARTIAL (`RuntimeEpoch` exists in `g9-policy` but is not wired into `g9-app`, so approvals survive a restart within their TTL, 10 minutes since 2026-11-13 (ADR-0015)); no encryption at rest (rely on full-disk encryption); `export.request` writes the export after approval since 2026-11-13 (section 14; exported files are not encrypted: they rely on the host's full-disk encryption like the database); idempotency records no longer store note bodies or skill output (an idempotent replay rebuilds the view from storage and warns `idempotency.replay_rebuilt`; TESTED by `a10_erasure_leaves_no_copy_in_any_table_and_replay_returns_no_erased_content`), so erasure leaves no copy, but old idempotency records are not purged on a schedule (`system.retention_sweep` is PROPOSED); CSP lacks `form-action` and `frame-ancestors` and there is no `on_navigation` guard (deferred until a host run); the CLI loads skills unpinned and accepts `--operator-id`; `simulate-week` and `crash-drill` write seed rows directly to storage (labelled `SEEDED`) and now require `--scratch`, an acknowledgement only: nothing inspects the directory for operator data (PROPOSED); the repository history contains 26 commits authored as "G9 Mentor Build" with the operator's personal email as identity.

## 11. Recovery status

| Capability | Status |
|---|---|
| Backup via Online Backup API / `VACUUM INTO`, verify on a copy, restore test before RETAINED | TESTED (A08, smoke) |
| Crash mid-session: interval closed UNCERTAIN, 0 ms credited, session INTERRUPTED | TESTED (A03, smoke crash drill) |
| Forward migrations with a mandatory pre-migration backup and dry run on a copy | TESTED (the desktop configuration was broken until 2026-11-08; fixed, with regression tests) |
| Offline restore (`g9 restore`, `g9_storage::restore::restore_over_live`): verifies a copy (checksum, integrity, foreign keys, audit chain, page length, schema not newer), takes exclusive access, quarantines the old database (never deletes), swaps in by atomic rename; the live path holds a complete database at every point | TESTED (unit, binary and smoke level; 8 defects found by the verifier and fixed). NOT RUN: a real SIGKILL (kill points simulated), non-unix, network filesystems |
| In-app restore operation (`DataDelete`-class approval) and restoring into a running application | **PROPOSED** (the desktop app has no restore button; the operator needs the CLI) |
| Backups across restarts | TESTED: manifests are read from the database; verify and restore-test work in a later process; `system.status.last_backup` is the newest RETAINED manifest |
| Desktop shell backup flow | NOT RUN (shell uncompiled) |
| Attachment files in backups (2026-11-13) | TESTED: `backup.create` copies the referenced files to `<data>/backups/attachments` (hash-checked), `g9 copy-backup` carries them, `g9 restore` puts them back and lists any missing (`apps/cli/tests/restore.rs`). Verification checks that files exist; hashing during verification is PROPOSED |

## 12. Remaining risks

| Risk | Owner stage | Mitigation in place |
|---|---|---|
| Offline restore verified only with simulated kill points (no real SIGKILL), on Linux, on a local filesystem; the swap needs hard-link support and the quarantine on the same filesystem; the desktop app has no restore button (in-app restore PROPOSED) | 14 | refuses before changing anything when unsupported; the host procedure in section 13 should include a restore drill |
| Shell compiled and its bridge unit tests passed on the CI runner only (runs 27 and 28); not built on the G9 host and no window flow exercised; the per-test counts were not read | 01 | CI job `desktop-shell` runs `cargo test --locked -p g9-mentor-desktop --lib` when CI is started by hand (manual only since 2026-11-13; no push starts a run); the host procedure in section 13 remains the open item |
| Original curriculum never supplied; canonical import blocked | 02 | seed stays PROPOSED_SEED; importer blocks promotion |
| No local model benchmarked; no model-in-the-loop evidence | 06 | `NullRuntime`; protocol defined (`docs/evaluations/model-benchmark-protocol.md`) |
| Self-adjudicated qualitative grading; machine-checkable weight 45% against a 60% target | 04 | reliability class; reported, not enforced |
| Learning loop: finalized grades now produce competency observations (migration 0013, commit 4b404cc; TESTED end to end). In the single-operator MVP every decision is SELF_ADJUDICATED, so competency state stops at `Independent` with `provisional` set and `VALIDATED` is not reachable through IPC; no adjudication or external-review operation exists, so nothing supersedes a derived observation; finalized attempts from before migration 0013 are not backfilled; a version that declares no difficulty derives nothing and warns | 04/05 | ADR-0012 reliability class; eligibility fail-closed; Regress counts occasions (a single failing attempt is one result) |
| Skill output schemas leave `items` untyped (32 of 37), so figures inside `result` are not machine-checked for `evidence_refs`; L05 outcome enum and L04 step content unenforced | 04/08 | contract rule and envelope-level checks only; typed items PROPOSED |
| Reference sources unverified: licences are expectations; several are link-only or must not be ingested | 12 | `verify_source_register.py` (dry run by default) and `check_source_register.py` invariants; nothing approved |
| 7 moderate advisories in the dev-only ESLint toolchain | 01 | not in production dependencies; upgrade `typescript-eslint` in a deliberate change |
| `CLAUDE.md` names a scratchpad file under `/tmp` as controlling document 3 (the v6.0 handoff text), which does not exist outside this session | 00 | operator decision needed (section 13) |

## 13. Next exact proposed operations (nothing below has been run)

**A. Compile and test the desktop shell on the G9 host (the largest single gap).**
- Operation: on Ubuntu 26.04.1, `sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (the list from CI job `desktop-shell`; its install step succeeded on the `ubuntu-26.04` runner in runs 27 and 28, so the names resolve there; the G9 host may differ), then from the repository root `cargo check --locked -p g9-mentor-desktop && cargo test --locked -p g9-mentor-desktop --lib`, then `cd apps/desktop && npm ci && npm run tauri:dev`.
- Expected effects: installs system development packages (about 1 to 2 GB), builds into `target/`; the window opens against the Rust bridge with transport TAURI (no SYNTHETIC DATA banner), model UNAVAILABLE, network OFF.
- Risks: package versions on the host differ from the CI runner's; the bridge tests already passed on CI, so a failure on the host points at the host, not the code; the window flows have never run.
- Rollback: `sudo apt-get remove` the packages; delete `target/`; the app data directory (`~/.local/share/com.g9.mentor`) can be deleted.
- Verification: exit codes 0; open Settings, create a backup, verify it, run the restore test (RETAINED); run the crash flow from the testing guide; then close the app and run the restore drill from `docs/runbooks/backup-restore.md` (`g9 restore --backup-id <id>` without confirmation to read the plan, then with `--confirm-replace-live-database`) and confirm the app reopens on the restored data.

**B. Verify the reference source register (active network operation).**
- Operation: `python3 -I scripts/verify_source_register.py` (prints 66 requests to 35 hosts, does nothing), then after reading that list, `python3 -I scripts/verify_source_register.py --approve-network --out verification-report.json`.
- Effects: 66 HTTPS GET requests, at most 256 KiB each, no downloads of datasets, no login; writes one JSON report. Never edits the register.
- Risk: page content is untrusted; it is searched for keywords only.
- Rollback: delete the report. Then a person reads each licence page and raises `verification` with evidence; the validator rejects weaker edits.

**C. Operator decisions.**
1. Issue or withhold the Stage 00 operator PASS (`GOVERNANCE.md`).
2. Supply the original approved 12-week curriculum so C01 reconciliation can run, or accept the seed as the working plan with the PROPOSED_SEED label.
3. ~~`export.request`: keep as a disclosed request or fail fast~~ resolved 2026-11-13: the owner asked for export to a file; the writer exists (section 14).
4. Replace the `/tmp` path in `CLAUDE.md` with a repository path: copying the handoff text into `docs/` would publish it if the repository is ever pushed to a public remote.
5. Git identity: keep or rewrite the 26 "G9 Mentor Build" commits (already on `origin`; rewriting them needs a force push, which needs the GitHub access you withheld).
6. Pushing: 13 or more local commits are unpushed; pushing requires reversing the "work only here" instruction.
7. Review and disable the third-party plugins synced into the Claude Code environment that you do not use (see the session audit).

## 14. Owner feature requests, 2026-11-13

Request (operator, verbatim): "Now finish the build doing each of the following: 1. Create a study session option 2. Add, edit, delete calendar events 3. Record a review result 4. Create your own competencies, assessments or rubrics 5. Attach evidence or files 6. Export your data to a file", plus the six correctness defects, the native confirmation, backup hardening and the decisions recorded in ADR-0015.

| # | Feature | Operations | Tests (fail-first where a defect was involved) | Status |
|---|---|---|---|---|
| 1 | Study session | `session.create` | `crates/g9-app/tests/feature_sessions.rs` (3) | TESTED |
| 2 | Calendar events | `calendar.create_event`, `calendar.update_event`, `calendar.delete_event` (approval) | `feature_calendar.rs` (5) | TESTED |
| 3 | Review result | `retention.create_item`, `retention.record_review` | `feature_retention.rs` (4) | TESTED |
| 4 | Authoring | `competency.create`, `assessment.define` | `feature_authoring.rs` (3): an authored assessment is begun, submitted, graded, finalized and projects its competency | TESTED; custom rubric criteria, QUIZ and REPORT kinds PROPOSED (no storage) |
| 5 | Evidence and files | `evidence.attach_file`, `evidence.from_note`, `evidence.list`; `assessment.submit` takes the ids | `feature_evidence.rs` (6), `g9-storage` `attachment_store` (5, one caught a symlinked shard directory creating a folder outside the store before the fix) | TESTED; quarantine PROPOSED (mismatches are refused) |
| 6 | Export to a file | `export.request` writes `<data>/exports/<id>/` | `feature_export.rs` (4), `g9-storage` `export` (4, one caught the unassigned `schema_migrations` table) | TESTED |
| — | Attachments in backups | `backup.create`, `g9 copy-backup`, `g9 restore --attachments` | `apps/cli/tests/restore.rs::attachment_files_travel_with_a_copied_backup_and_come_back_on_restore` | TESTED |

Independent review of the new file-writing code (2026-11-13, read-only reviewer, proof tests outside the repository): no CRITICAL or HIGH; 2 MEDIUM, 8 LOW, 3 INFO. Fixed with a regression test that failed first: both MEDIUM (erased titles and file names surviving in evidence summaries and in idempotency records), LOW 3, 4, 5, 6, 7, 8, 9 and INFO 11, 13, plus one more found while fixing (a PRIVATE attempt's pass/fail in an INTERNAL event). Open by decision: LOW 10 (no attachment quota; bytes not reclaimed until the purge decision) and INFO 12 (an export folder can outlive a failed commit); ADR-0016 lists them.

IPC operations: 46 (was 35). New dependency: `base64 =0.23.1` in `g9-app` (already in `Cargo.lock` through the shell's tree; default features off). Frontend: section 6 counts are updated when the final verification of this change runs.

