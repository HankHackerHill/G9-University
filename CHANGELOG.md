# Changelog

All notable changes are recorded here. Labels follow `GOVERNANCE.md`.

## [0.1.0] — unreleased (2026-11-13 update)

### Security
- Backup verification, restore and open-time recovery now verify the schema itself: every table, index, trigger and view must equal what the embedded migrations produce at the recorded version. A tampered snapshot or live file with a dropped or planted trigger is refused at verify and restore, and blocks every mutation at open (`schema_drift`). Before, a copy of a genuine database with planted triggers passed every check (security audit 2026-11-12, MEDIUM). The SQL text is compared exactly: the first version collapsed whitespace, and since a newline ends a `--` comment, moving one newline could remove a CHECK constraint unnoticed (found while verifying the fix).
- Approvals bind to the stored revision of the resource, and a request whose `expected_revision` differs from it is refused `STALE_REVISION`. Before, an approval could be bound to a future revision and consumed after a regrade the operator never saw (MEDIUM).
- The restore plan no longer prints a mismatching snapshot hash next to the confirmation command; table names read from a foreign file are quoted.
- `src-tauri/build.rs` refuses a release build in Tauri development mode (which would load its UI from `http://localhost:5173`); the README builds with `npm run tauri:build` (LOW).
- Report: `docs/audit/security-audit-2026-11-12.md`.

### Changed
- Owner decisions (ADR-0015): the application database runs with `synchronous = FULL` (`g9_app::database_options`, used by the desktop bridge and the CLI), so a power cut loses no acknowledged command; approvals expire after 10 minutes (was 15); encryption at rest is left to the host's full-disk encryption; no AI model for now (a Claude API runtime is planned and needs an architecture amendment first); `VALIDATED` stays unreachable while every grade is self-adjudicated. ARCHITECTURE.md lines 103 and 149 still state the old values (reported deviation).
- CI runs only when started by hand (`workflow_dispatch`), by the owner's decision: the push and pull_request triggers are removed, so no push starts a run. All five jobs are unchanged. The same checks run locally with `scripts/ci_preflight.sh`. GitHub shows the "Run workflow" button only for workflows on the default branch, which does not contain `.github/workflows/ci.yml` yet.

## [0.1.0] — unreleased (2026-11-12 update)

### Added
- Desktop UI design system "The Ledger" (`docs/ui/design-system.md`): OKLCH tokens, a grammar of standing (posted, pencilled, in doubt, advice, owed, missing), the Recursive variable font (OFL-1.1, bundled, licence shipped) in three voices, cascade layers.
- New shell: grouped table of contents, day rail drawn to scale from backend values, status lamps with the backend clock, command palette (`Ctrl K`), ledger key (`?`).
- 21 themes (Ledger plus 20, including Hacker, Amber Terminal, Synthwave, Cyberpunk, Classic Desktop, Pocket Console, Clean Crisp, Modern Tech, Professional), each palette contrast-tested on every run; theme picker in the shell, preview gallery in Settings, palette commands.
- Every screen has a page head stating its purpose; Mission Control and Session Tracker reworked (per-block meters and totals, a timesheet summary); measured columns right-aligned in the machine voice.
- `scripts/ci_preflight.sh`: every CI job replicated on a clean `git archive` export of a revision; steps that cannot run locally are reported as SKIPPED, never as passed (testing guide §1.7).

### Changed
- CI cost guards: a newer push cancels the superseded run of the same ref (`concurrency`), every job has `timeout-minutes` (the default is 360), and the workflow can be started by hand (`workflow_dispatch`). No check was removed and the triggers are unchanged. Standard runners are free while the repository is public; a private repository draws 9 to 14 billed minutes per run (measured, runs 27 and 28) from the plan allowance.

## [0.1.0] — unreleased (2026-11-09 update)

### Fixed
- `g9-skills`: an absent `references/` directory in a skill package is now `PackageError::MissingReferences` (it was `Io`, `layout.io`); other I/O errors are unchanged. GitHub CI run 27 failed `package_rejects_each_contract_violation` because its fixture was an empty directory, which git does not store, so only a fresh checkout failed; local runs passed throughout. The test now builds the absent, empty and non-markdown cases at runtime. After the fix, GitHub CI run 28 passed all five jobs (rust incl. cargo-deny, frontend, sbom, content-gates, desktop-shell); the Tauri shell compiles and its unit tests pass on the CI runner for the first time.
- Docs: removed the claims that the shell was never compiled and that no CI result had been read; corrected the stage-report statement that the clean-export replication covered the test step (it did not).

## [0.1.0] — unreleased (2026-11-08 update)

### Fixed
- Desktop and CLI: directory approved-roots check (`ApprovedRoots::contains_directory`); backups, the pre-migration dry run and the attachment-manifest check were inert in the desktop configuration.
- `g9-competency`: validation evidence must trace to a graded attempt; an EXAM-labelled episode or an unrooted correction no longer reaches VALIDATED.
- `g9-retention`: reliability-bin edges placed probabilities one bin low.
- Desktop UI: 12 verified and 11 fresh review findings (warnings rendering, rubric codes, time-correction flow, error states, KPI definitions, calendar hours); SYNTHETIC DATA banner for the mock transport.
- Skill registry hash column recomputed under the runtime definition (`scripts/skill_hashes.py`); runtime pins unchanged.

### Added (learning loop, backups and restore, 2026-11-08)
- Finalized grades produce competency observations in the finalizing transaction with persisted validation eligibility (migration 0013); `competency.projection` honours it. Regress counts occasions; eligibility is fail-closed.
- Backups usable across restarts (database-backed manifests, `system.status.last_backup`), verified offline restore `g9 restore` (`g9_storage::restore`), `--scratch` required for `simulate-week` and `crash-drill`. Smoke test: 100 assertions.

### Added
- CLI `g9`, `scripts/smoke_test.sh` (67 assertions), testing guide; layering gate covers the CLI.
- Reference source register (48 unverified candidates, 0 approved) with schema, validator and an offline-safe verifier (`scripts/verify_source_register.py`, dry run by default).
- Defect register status re-assessment (section E), `docs/release/stage-report.md`, `docs/release/gate-evidence.*`.
- CI: `--locked` everywhere, shell tests, pinned SBOM tools, hash and register checks; `.github/CODEOWNERS`; `.gitignore` for SQLite sidecars and key files.

## [0.1.0] — unreleased (2026-11-05)

### Stage 00 — Design
- Triple audit of the v5.1 plan and the v6.0 handoff; defect register, improvement backlog and ADRs under `docs/audit/` and `docs/adr/`.
- `ARCHITECTURE.md` v6.1: 17 modules mapped to crates, dependency law, command pipeline, storage rules, time/DST policy, assessment/competency/memory model, retention scope, AI gateway and skill orchestration, authorization, UI, gates.
- Requirements traceability matrix, threat model, privacy model, evaluation protocols, UI screen flows.

### Stage 01 — Scaffold
- Cargo workspace (13 crates + Tauri shell excluded from default members), pinned dependencies, `deny.toml`, CI workflow, `g9-domain` vocabulary with state machines and invariants.
- Desktop UI scaffold with typed IPC client, mock transport, 16 screens, sanitized Markdown, accessible calendar grid.
- 37 skill packages and 8 knowledge packs with validators.

### Stage 02 — Data & importer
- SQLite schema and checksummed migrations, immutability triggers, hash-chained audit metadata, idempotency store.
- Curriculum manifest v1.0.0, seed v0 (Appendix A, `PROPOSED_SEED`), invariant validator, reconciliation diff, provenance gate.
- Backup via Online Backup API / `VACUUM INTO` with integrity and restore verification.
