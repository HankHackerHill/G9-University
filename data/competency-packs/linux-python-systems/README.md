# Knowledge pack: Linux, Python and systems engineering

**Status:** PROPOSED (v0.1.0) · **Owner module:** M12 · **Seed curriculum weeks:** Week 1 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `lab.authorization` | Lab authorization and boundaries | 1–2 | — | 180 |
| `linux.filesystem` | Filesystem hierarchy, permissions and ownership | 1–3 | lab.authorization | 180 |
| `linux.processes-services` | Processes, services and logging | 2–3 | linux.filesystem | 180 |
| `python.secure-parsing` | Secure parsing of structured security records | 2–4 | linux.processes-services | 240 |
| `python.testing-fixtures` | Testing with fixtures and reproducible runbooks | 2–4 | python.secure-parsing | 240 |
| `linux.evidence-completeness` | Evidence completeness assessment | 3–4 | linux.processes-services | 240 |
| `linux.hardening-baseline` | Hardened baseline and configuration verification | 3–5 | linux.evidence-completeness | 240 |
| `linux.blind-troubleshooting` | Blind Linux troubleshooting | 3–5 | linux.hardening-baseline, python.testing-fixtures | 180 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (VM), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Host telemetry: auth/syslog/journald, process and file auditing (auditd), package and service state; every diagnostic leaves an evidence trail.

## Safety boundaries

- Work only on the isolated lab VM; never run diagnostics on the G9 host without an approved operation
- Preserve logs and baselines before changing state
- No credentials in notes or fixtures

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| Linux Filesystem Hierarchy Standard | Linux Foundation | STANDARD | freely available specification | ASSUMED |
| Python 3 documentation | Python Software Foundation | DOC | PSF license, freely available | ASSUMED |
| OverTheWire Bandit | OverTheWire | LAB | free public wargame; terms must be checked before embedding | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
