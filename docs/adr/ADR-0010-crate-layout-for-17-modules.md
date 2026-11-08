# ADR-0010: Thirteen crates plus the Tauri shell, with an explicit module→crate map and acyclic layering

**Status:** Accepted (2026-11-06)

## Context

The handoff gave 17 modules, 11 bounded contexts and a 10-crate tree without a mapping (TEC-01, TEC-02).

## Decision

`ARCHITECTURE.md` §3 maps every module to a crate: domain (L0) → policy, storage (L1) → pure domain services curriculum/calendar/evidence/competency/assessment/retention (L2) → ai-adapter (L3) → skills (L4) → app (L5) → Tauri shell (L6). Analytics, executive, research and teaching are modules inside `g9-app` until they justify crates. A workspace test over `cargo metadata` asserts the layering (wave 2). IPC DTO schemas live in `schemas/ipc` and are validated against the Rust handlers; a generated `g9-ipc` crate with TypeScript emission is a backlog item.

## Alternatives considered

One crate per module (rejected: 17 crates of boilerplate). Single crate (rejected: no drift control).

## Consequences

Drift is caught by the build; module ownership is unambiguous.

## Verification

`cargo tree` in CI; layering test in wave 2.

## Addendum (2026-11-07): command-line driver as a second L6 shell

`apps/cli` (package `g9-cli`, binary `g9`) is a second L6 front end beside the Tauri shell. It is a workspace member and a default member (it builds without GTK/WebKit). Rules enforced by `tests/integration/layering_gate.rs`:

| Rule | Statement |
|---|---|
| Edges | `g9-cli` may name `g9-domain`, `g9-policy`, `g9-storage`, `g9-skills`, `g9-app`, `g9-curriculum`, `g9-calendar`, `g9-assessment`, `g9-competency`; never `g9-ai-adapter`, `g9-evaluation` or the Tauri shell |
| Inbound | nothing depends on `g9-cli` |
| Bootstrap | same composition as the Tauri bridge (database, binding key via `g9_app::keyfile`, skill registry, `NullRuntime`); the backend assigns the operator actor |
| Pipeline | every command enters `g9_app::dispatch`; the only writes outside it are `SEEDED` repository rows of `simulate-week` and `crash-drill` for records with no IPC creation path yet (`apps/cli/src/seed.rs`); `g9 call` cannot reach them. `g9 restore` is an offline maintenance command: it replaces the database file through `g9_storage::restore::restore_over_live` before any context is opened and writes no rows (the CLI holds no SQL) |
| Status | `simulate-week` and `crash-drill` refuse to run unless `--scratch` is given (usage error, exit 1, no data directory created): they write seed rows directly to storage and must only be pointed at a throwaway data directory. IMPLEMENTED as an explicit acknowledgement: the flag does not inspect the directory. PROPOSED: refuse a data directory that already holds operator data |
