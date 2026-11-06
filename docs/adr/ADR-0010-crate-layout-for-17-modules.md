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
