# Knowledge pack: Detection engineering, SOC and threat hunting

**Status:** PROPOSED (v0.1.0) · **Owner module:** M12 · **Seed curriculum weeks:** Week 7 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `det.event-sources` | Event source inventory and coverage | 1–3 | — | 240 |
| `det.normalization` | Log normalization and enrichment | 2–3 | det.event-sources | 240 |
| `det.sigma-rules` | Sigma schema and detection hypotheses | 2–4 | det.normalization | 180 |
| `det.correlation-timeline` | Correlation and investigation timeline | 3–4 | det.sigma-rules | 180 |
| `det.criteria-fp` | Detection criteria and false-positive estimation | 3–4 | det.sigma-rules | 180 |
| `hunt.hypothesis` | Hunt hypothesis and approved telemetry | 3–5 | det.correlation-timeline | 180 |
| `det.blind-telemetry` | Blind telemetry investigation | 3–5 | det.criteria-fp, hunt.hypothesis | 180 |
| `det.sigma-backend-fidelity` | Backend conversion fidelity | 4–5 | det.sigma-rules | 240 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (CONTAINER), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Coverage maps per data source, Sigma rule tests with positive and negative fixtures, false-positive estimates.

## Safety boundaries

- Use licensed sample datasets only (note license per dataset)
- Rules are tested on fixtures, never deployed to production SIEMs from the lab
- No customer data

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| Sigma specification | SigmaHQ | STANDARD | DRL/Apache-2.0 (verify) | ASSUMED |
| MITRE ATT&CK | MITRE | STANDARD | free with attribution | ASSUMED |
| Splunk Boss of the SOC datasets | Splunk | DATASET | free download; redistribution terms must be verified | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
