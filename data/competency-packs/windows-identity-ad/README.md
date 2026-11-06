# Knowledge pack: Windows, identity, Active Directory and access controls

**Status:** PROPOSED (v0.1.0) · **Owner module:** M12 · **Seed curriculum weeks:** Weeks 5-6 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `win.principals-services` | Windows principals, services and event records | 1–3 | — | 240 |
| `linux.privilege-boundaries` | Linux privilege boundaries: sudo, capabilities, services | 2–4 | linux-python-systems:linux.hardening-baseline | 240 |
| `win.token-service-controls` | Windows tokens and service controls | 2–4 | win.principals-services | 240 |
| `host.persistence-telemetry` | Persistence mechanisms and telemetry | 2–4 | win.token-service-controls | 240 |
| `ad.forest-trust-model` | Forest, domain and trust model | 2–3 | win.principals-services | 240 |
| `ad.kerberos` | Kerberos issuance and validation | 3–4 | ad.forest-trust-model | 180 |
| `ad.delegation-permissions` | Delegation types and excessive permissions | 3–5 | ad.kerberos | 180 |
| `ad.identity-detection` | Identity telemetry and detection | 3–5 | ad.delegation-permissions, host.persistence-telemetry | 180 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (VM), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Windows event logs (4624/4625/4672/4768/4769/4776), Sysmon, service and task changes; identity misuse leaves ticket and logon artifacts.

## Safety boundaries

- Isolated AD lab forest only; never domain-joined production machines
- No real credentials or hashes in fixtures
- Privilege-path analysis stays within the lab trust boundary

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| Microsoft Learn: Windows security auditing | Microsoft | DOC | public documentation | ASSUMED |
| MITRE ATT&CK Enterprise | MITRE | STANDARD | ATT&CK license (free use with attribution) | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
