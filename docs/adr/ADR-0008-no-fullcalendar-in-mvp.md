# ADR-0008: In-house accessible calendar grid; no FullCalendar dependency

**Status:** Accepted (2026-11-06)

## Context

v5.1 chose FullCalendar Standard (MIT) but the premium timeline/resource views are commercial and v7 Premium moved to AGPLv3 (VERIFIED 2026-11-05). Mixing tiers creates a licensing boundary inside one component.

## Decision

Build a keyboard-accessible week/day grid (`role=grid`, roving tabindex, ARIA labels) in `apps/desktop`. Revisit only if a measured UX need appears that the in-house grid cannot meet.

## Alternatives considered

FullCalendar Standard only (rejected: feature creep toward premium; v7 license shift). Other MIT calendars (deferred; a11y not verified).

## Consequences

Zero third-party calendar license exposure; full control over accessibility.

## Verification

UI vitest keyboard-navigation tests; `LICENSES.md`.
