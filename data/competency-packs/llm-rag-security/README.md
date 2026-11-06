# Knowledge pack: LLM applications, RAG, prompt injection and model evaluation

**Status:** PROPOSED (v0.1.0) · **Owner module:** M10 · **Seed curriculum weeks:** Week 9 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `llm.app-mapping` | LLM application mapping and threat model | 1–3 | — | 180 |
| `rag.ingestion-provenance` | RAG ingestion, retrieval and source trust | 2–4 | llm.app-mapping | 180 |
| `llm.prompt-injection` | Prompt-injection entry classification | 2–4 | rag.ingestion-provenance | 180 |
| `llm.output-schema-tools` | Output schema failures and tool safety | 3–4 | llm.prompt-injection | 180 |
| `llm.independent-eval` | Independent evaluation design | 3–5 | llm.output-schema-tools | 240 |
| `llm.trust-failure-diagnosis` | Trust failure diagnosis | 3–5 | llm.independent-eval | 180 |
| `llm.data-leakage` | Data leakage and privacy exposure | 2–4 | rag.ingestion-provenance | 240 |
| `llm.eval-leakage` | Benchmark leakage detection | 3–5 | llm.independent-eval | 240 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (CONTAINER), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Prompt/response metadata logs, retrieval provenance, schema validation failures, tool-call audit; injection attempts become detection fixtures.

## Safety boundaries

- Only local or explicitly approved models; no production API keys in the lab
- Injection fixtures are data, never instructions for the mentor itself
- No personal data in evaluation sets

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| OWASP Top 10 for LLM Applications 2026 | OWASP GenAI Security Project | STANDARD | CC BY-SA 4.0 (verify) | ASSUMED |
| NIST AI RMF and Generative AI Profile | NIST | STANDARD | public domain | ASSUMED |
| MITRE ATLAS | MITRE | STANDARD | free with attribution | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
