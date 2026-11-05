# Changelog

All notable changes are recorded here. Labels follow `GOVERNANCE.md`.

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
