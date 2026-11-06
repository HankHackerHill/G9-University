# ADR-0011: UTC epoch milliseconds for instants, IANA zone + civil time for recurrences, monotonic ledger for the timer

**Status:** Accepted (2026-11-06)

## Context

v5.1 DDL used zone-less `TIMESTAMP` columns (TEC-20); suspend/reboot detection was unspecified (TEC-07).

## Decision

`UtcMillis` (i64) for instants; `IanaZone` + `CivilDateTime` for operator intent and recurrences; occurrences resolved with explicit DST gap/overlap policy (`Adjustment::Gap` → first valid instant after the gap; `Adjustment::Overlap` → earlier offset, flagged). Timer intervals store wall and monotonic timestamps; an open interval at process start is closed `UNCERTAIN` and the session becomes `Interrupted`; uncertain time is never credited without an operator correction.

## Alternatives considered

Local timestamps (rejected). `jiff` instead of `chrono-tz` (deferred; chrono-tz exposes `LocalResult` which is sufficient and widely known).

## Consequences

Wall-time intent survives DST; double counting is impossible (gates A02, A03).

## Verification

`a02_dst_*`, `a03_*`.
