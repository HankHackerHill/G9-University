# Changelog

All notable changes are recorded here. Labels follow `GOVERNANCE.md`.

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
