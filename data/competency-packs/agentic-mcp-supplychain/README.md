# Knowledge pack: Agentic AI, MCP authorization, tool misuse and AI supply chain

**Status:** PROPOSED (v0.1.0) · **Owner module:** M16 · **Seed curriculum weeks:** Weeks 10-11 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `agent.workflow-authority` | Agent workflow and authority crossings | 2–3 | llm-rag-security:llm.app-mapping | 180 |
| `agent.identity-scopes` | Identities, scopes and excessive agency | 2–4 | agent.workflow-authority | 180 |
| `mcp.transport-auth` | MCP transport, actors and token audience | 3–4 | agent.identity-scopes | 180 |
| `agent.memory-integrity` | Persistent memory trust and poisoning | 3–4 | mcp.transport-auth | 180 |
| `agent.redblue-experiment` | Bounded red/blue experiment | 3–5 | agent.memory-integrity | 180 |
| `ai.threat-model-redteam` | AI asset threat modeling and adversarial fixtures | 3–5 | agent.redblue-experiment | 180 |
| `ai.supply-chain` | Model and dependency provenance | 3–5 | agent.identity-scopes | 240 |
| `ai.data-governance-eval` | Data poisoning, privacy and evaluation protocol | 3–5 | ai.supply-chain, llm-rag-security:llm.eval-leakage | 240 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (CONTAINER), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Tool-call audit trails, approval decision logs, token audience checks, package/model provenance records; excessive agency shows as unapproved actions.

## Safety boundaries

- Agents run only in the isolated lab with mock tools
- No real tokens; audience/resource binding tested with fixtures
- Supply-chain experiments use quarantined packages, never installed on the host

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| OWASP Top 10 for Agentic Applications 2026 | OWASP GenAI Security Project | STANDARD | CC BY-SA 4.0 (verify) | ASSUMED |
| Model Context Protocol authorization specification 2025-11-25 | MCP project | STANDARD | MIT (verify) | ASSUMED |
| Open Agent Skills specification | agentskills.io | STANDARD | open specification | VERIFIED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
