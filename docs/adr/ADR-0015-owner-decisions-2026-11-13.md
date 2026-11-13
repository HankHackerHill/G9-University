# ADR-0015: Owner decisions of 2026-11-13 (durability, approval window, encryption, AI, validation, CI, repository)

**Status:** Accepted (2026-11-13), decided by the owner

## Context

The production-readiness review for single-operator use listed decisions only the owner can make (stage report, operator decisions; production gaps classed OPERATOR_DECISION). The owner decided them on 2026-11-13.

## Decision

| Topic | Decision | Implementation |
|---|---|---|
| Durability (TEC-12) | `synchronous = FULL` for the application database: in WAL mode the WAL is synced at every commit, so a power cut loses no acknowledged command | `g9_app::database_options` (used by the desktop bridge and the CLI); storage default stays NORMAL for other callers |
| Approval window (SEC-02) | 10 minutes | `g9_policy::DEFAULT_APPROVAL_TTL`; the mock transport and the Approvals screen follow |
| Encryption at rest (IMP-M06, IMP-M08) | No application-level encryption; the G9 host must use full-disk encryption | Host requirement, no code |
| AI model | None for now: `NullRuntime` stays, AI skills answer `RESOURCE_UNAVAILABLE`, and the application remains fully usable (R-002, A09). A remote runtime (Claude API, Claude Opus 5.5) is planned later; it needs an amendment of ARCHITECTURE.md's "no network capability in the mentor runtime" non-negotiable and its own ADR before any code | No code |
| Competency validation | Accepted that `VALIDATED` is unreachable while every grade is `SELF_ADJUDICATED` (ADR-0012) | No code |
| CI | Runs only when started by hand (`workflow_dispatch`) | `.github/workflows/ci.yml` |
| Repository visibility | Private (27 commits carry the owner's personal e-mail address in author metadata) | Owner action in the GitHub repository settings; no tool in the agent sessions changes visibility |

## Deviations from ARCHITECTURE.md

ARCHITECTURE.md is the controlling document and agents do not edit it (CLAUDE.md). Two lines now differ from the owner's decisions and are to be updated by the owner:

| ARCHITECTURE.md | Says | Decided |
|---|---|---|
| Line 103 (§6) | `synchronous = NORMAL` in WAL | FULL for the application database |
| Line 149 (§11) | approval expiry default 15 minutes | 10 minutes |

## Consequences

- One sync of the WAL per committed command: slightly higher commit latency, negligible for a single operator.
- An approval must be used within 10 minutes of the request, not 15.
- Data on disk is only as protected as the host's disk encryption.

## Verification

`apps/cli/tests/durability.rs::the_backend_opens_its_database_with_synchronous_full` (reads back `PRAGMA synchronous = 2` on the connection the backend opens); `g9_policy` tests `default_ttl_is_ten_minutes`, `expire_due_sweeps_pending_and_approved`, `toctou_expire_due_sweeps_only_due_records`; mock transport tests "expires approvals after 10 minutes". Each failed before the change.
