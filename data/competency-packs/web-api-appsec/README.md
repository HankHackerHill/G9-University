# Knowledge pack: Web, API and application security

**Status:** PROPOSED (v0.1.0) · **Owner module:** M12 · **Seed curriculum weeks:** Weeks 3-4 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `web.http-semantics` | HTTP semantics and browser trust boundaries | 1–2 | — | 240 |
| `web.session-auth` | Authentication and session state | 2–3 | web.http-semantics | 180 |
| `web.authz-idor` | Authorization and object-level access control | 2–4 | web.session-auth | 180 |
| `web.injection` | Injection root causes and prepared statements | 2–4 | web.http-semantics | 180 |
| `web.xss-contexts` | XSS execution contexts and output encoding | 2–4 | web.http-semantics | 180 |
| `api.token-authz` | API identity, tokens and object-level authorization | 3–4 | web.authz-idor | 180 |
| `web.ssrf-upload-deserialization` | SSRF, uploads and unsafe deserialization | 3–5 | api.token-authz, web.injection | 180 |
| `web.threat-model-logic` | Application threat modeling and business-logic invariants | 3–5 | web.ssrf-upload-deserialization | 240 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (CONTAINER), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Application logs, WAF/proxy logs, authentication events; each vulnerability class maps to a detection signal and a verified fix.

## Safety boundaries

- Only deliberately vulnerable lab applications (OWASP Juice Shop, PortSwigger labs); never production targets
- No credential reuse between lab and real accounts
- Keep exploit artifacts inside the lab evidence store, classified SECURITY_SENSITIVE

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| OWASP Web Security Testing Guide | OWASP | STANDARD | CC BY-SA 4.0 | ASSUMED |
| OWASP ASVS | OWASP | STANDARD | CC BY-SA 4.0 | ASSUMED |
| PortSwigger Web Security Academy | PortSwigger | LAB | free account; terms to verify before embedding | ASSUMED |
| OWASP Juice Shop | OWASP | LAB | MIT | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
