# Architecture decision records

Each ADR resolves one or more rows of `docs/audit/defect-register.md`. Status: Accepted = binding for this repository; Proposed = awaiting operator decision.

| ADR | Title | Resolves |
|---|---|---|
| [ADR-0001](ADR-0001-sqlite-backup-strategy.md) | SQLite backup: Online Backup API or VACUUM INTO, verified on a copy, independent of WAL | C04, C05, TEC-09, TEC-10, SEC-29 |
| [ADR-0002](ADR-0002-deletion-semantics-per-relationship.md) | Explicit deletion action per relationship; no blanket ON DELETE CASCADE | C13, TEC-05, TEC-15, TEC-16 |
| [ADR-0003](ADR-0003-approval-token-binding.md) | Approvals bind to the exact operation hash, resource, expected revision and expiry; single-use; HMAC-verified | TEC-08, SEC-02, SEC-23 |
| [ADR-0004](ADR-0004-audit-chain-and-privacy.md) | Hash-chained, metadata-only audit records; private content never logged | C19, SEC-08 |
| [ADR-0005](ADR-0005-competency-state-model.md) | Dimension-specific, evidence-rule-derived competency state; no mastery float; model observations rejected | C10, LRN-06, LRN-09, LRN-11 |
| [ADR-0006](ADR-0006-fsrs-scope-and-baseline-schedule.md) | Fixed baseline review schedule first; FSRS optional and only for declarative items | C14, LRN-04, LRN-18, LRN-20 |
| [ADR-0007](ADR-0007-n-of-1-experiment-protocol.md) | Pre-registered single-learner designs with uncertainty and an INCONCLUSIVE outcome; no fixed n/p thresholds | C08, LRN-01, LRN-02, LRN-03, LRN-21 |
| [ADR-0008](ADR-0008-no-fullcalendar-in-mvp.md) | In-house accessible calendar grid; no FullCalendar dependency | v5.1 F8 / PLT |
| [ADR-0009](ADR-0009-model-selection-deferred-to-benchmark.md) | No model pre-selected; candidates discovered and benchmarked on the actual host | C06, C07, TEC-28, CON-28 |
| [ADR-0010](ADR-0010-crate-layout-for-17-modules.md) | Thirteen crates plus the Tauri shell, with an explicit module→crate map and acyclic layering | TEC-01, TEC-02 |
| [ADR-0011](ADR-0011-time-representation.md) | UTC epoch milliseconds for instants, IANA zone + civil time for recurrences, monotonic ledger for the timer | TEC-07, TEC-20 |
| [ADR-0012](ADR-0012-assessment-pass-rule-and-reliability.md) | Non-compensatory pass rule with enumerated gates and a reliability class on every grade decision | LRN-10, LRN-11, LRN-14 |
| [ADR-0013](ADR-0013-typed-ipc-envelope-and-error-categories.md) | Closed 13-category error set, backend-assigned actor, success envelope with revision and replay flag | TEC-03, TEC-04, SEC-01 |
| [ADR-0014](ADR-0014-skill-md-is-not-a-security-boundary.md) | SKILL.md is instructional metadata; permissions are decided in Rust; protected operations get a native confirmation | C11, SEC-03 |
| [ADR-0015](ADR-0015-owner-decisions-2026-11-13.md) | Owner decisions: synchronous FULL, 10-minute approvals, host disk encryption, no AI yet, VALIDATED unreachable accepted, manual CI, private repository | TEC-12, SEC-02, IMP-M06 |
| [ADR-0016](ADR-0016-operator-content-attachments-and-export.md) | Operator-authored content, attachment bytes from the webview into a content-addressed store, export to a file (SECRET never), attachments in backups | SEC-13, SEC-09, A10 |
