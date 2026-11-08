# Stage 00 triple audit — technical, adversarial, acceptance (v6.1)

Inputs: the v5.1 audit/plan text (operator-supplied) and the v6.0 FINAL MASTER BUILD & CODEX HANDOFF (5 Nov 2026). Method: four independent auditors (technical, adversarial-security, learning-science, consistency) over both documents, plus the maintainer's own review; findings in `defect-register.md` (152 auditor rows, 22 v6.0 corrections). Two planned passes did not complete in the first run because of an account usage limit (the platform/licensing auditor and the skeptical verification pass); both were run on 2026-11-06. The skeptical verification pass is complete: independent refute-first verifiers returned one verdict per auditor row (152 of 152): CONFIRMED 104, DOWNGRADED 44, REFUTED 3, UPGRADED 1. Verified severities, REFUTED excluded: BLOCKER 0, CRITICAL 5, HIGH 38, MEDIUM 52, LOW 54 (149 rows). Verdicts are in the Verdict column of `defect-register.md` section B; reasons, reported-to-verified severity changes and corrected corrections are in `defect-register.md` section D (Verification notes). The platform/licensing pass is recorded in `platform-licensing-audit.md` (PLT-01..PLT-18, separate from the 152 register rows). Platform facts were also verified separately by the maintainer where listed in `README.md`/`LICENSES.md` (Ubuntu 26.04.1, RTX 5080, NICE v2.2.0, FullCalendar v7 licensing, Agent Skills spec, Tauri 2.12.5, rusqlite 0.41.1).

## Pass A — technical and architectural correctness

**Result.** The modular monolith is right for a single-operator offline product, but neither source document was implementable as written: no module→crate map (TEC-01), no declared crate DAG (TEC-02), an incomplete IPC envelope without a success shape or backend-assigned actor (TEC-03, SEC-01), unspecified idempotency (TEC-04), a DDL with wrong foreign keys and zone-less timestamps (TEC-05, TEC-15, TEC-16, TEC-20), three inconsistent session vocabularies (CON-04, TEC-06), a backup recipe that is not a backup (TEC-09) and a false WAL prerequisite (TEC-10), no suspend/reboot detection design (TEC-07), and a v5.1 effort table whose rows sum to 85 person-weeks while the text says 72 (CON-xx, roadmap).

**Corrections applied in this repository.** `ARCHITECTURE.md` §3 maps all 17 modules to 13 crates with acyclic layering (ADR-0010); `g9-domain` fixes the vocabulary (ids, UTC ms, zones, civil time, 13 error categories, eight explicit state machines, invariants) (ADR-0011, ADR-0013); `g9-storage` carries the DDL with per-relationship deletion semantics, STRICT tables, triggers and checksummed migrations (ADR-0002); backup is Online Backup API/VACUUM INTO verified on a copy (ADR-0001); the timer is a wall+monotonic ledger with `UNCERTAIN` closure; the curriculum importer enforces 12/72/216/28,800 and the provenance gate (A01).

**Acceptance evidence.** Executed tests are listed in `docs/evaluations/gate-evidence.md` (generated gate harness) and `docs/release/wave2-integration-report.md` (workspace test counts); no Stage PASS report has been issued yet; anything not executed is NOT RUN. No irrecoverable migration defects in the fault suite (A08), no silent corruption (triggers + chain), complete curriculum arithmetic (A01), idempotent repeated mutation (pipeline tests, wave 2), bounded inference (A09/A11).

**Open technical items.** Generated TypeScript IPC types from Rust (backlog), `g9-ipc` crate split (backlog), analytics read models (Stage 05), layering test over `cargo metadata` (wave 2).

## Pass B — adversarial and privacy security

**Result.** The real attack surface is the bridge between untrusted text and privileged operations. The source documents relied on prompt safety and metadata (C11, C17); auditors added: self-asserted actor ids over IPC (SEC-01), unbound approvals and TOCTOU (SEC-02), a forgeable confirmation surface if the webview is compromised (SEC-03), no concrete Tauri capability allow-list (SEC-04), no sanitizer/CSP specification (SEC-05), no per-layer network-deny enforcement (SEC-06), wrong encryption primitive (SEC-07), audit immutability vs deletion (SEC-08), undefined exam sealing (SEC-10), and v5.1's "multi-party approval" and "model parses its own output" (SEC-23, SEC-24).

**Corrections applied.** Deny-by-default `g9-policy` with operator-only capabilities and HMAC-bound single-use approvals (ADR-0003); actor set by the backend (ADR-0013); Tauri capabilities limited to `core:default` with a strict CSP and `connect-src 'none'`; DOMPurify allow-list rendering; `cargo-deny` bans HTTP stacks in runtime crates; context assembler refuses untrusted segments in instruction roles and all `Secret` content; sealed keys as `Secret` attachments with an exposure log; metadata-only hash-chained audit (ADR-0004); layered encryption plan (FDE + optional SQLCipher + encrypted backups) in `docs/privacy/data-lifecycle.md`; native Rust-side confirmation for protected operations adopted as PROPOSED (ADR-0014).

**Adversarial validation.** ST01–ST12 in `docs/threat-model/threat-model.md` §6; each records the policy decision and the final backend state. Model-in-the-loop variants are NOT RUN until Stage 06/07.

**Residual risks.** Host compromise, malicious native dependencies passing advisories, device theft without FDE, GPU/runtime library compromise, hallucination inside approved content, undetected poisoning, overfitting of personal recommendations.

## Pass C — acceptance: measurable learner, business and product value

**Result.** The learning design was promising but unmeasurable as written: no competency evidence rules (LRN-09 BLOCKER), no experiment protocol (LRN-21 BLOCKER), confounded week-alternation and invalid statistics in v5.1 (LRN-01..03), a compensatory pass rule and unenumerated gates (LRN-10), self-adjudicated grades flowing into validated states (LRN-11), no exposure ledger or retake policy (LRN-13), scheduler-dependent recall KPIs (LRN-19), undefined KPIs and non-executable B-gates (LRN-22, LRN-23), FSRS misapplied to competencies (LRN-04, LRN-18).

**Corrections applied.** Dimension-specific, rule-derived competency state with model observations rejected (ADR-0005); fixed baseline schedule with FSRS optional for declarative items (ADR-0006); pre-registered N-of-1 designs with INCONCLUSIVE as a valid outcome (ADR-0007); non-compensatory pass rule, enumerated gates and reliability class (ADR-0012); exposure ledger, variant-based retakes and first-attempt vs eventual pass rates (ARCHITECTURE §8 addendum, wave 2); scheduler-independent probe recall and KPI definitions (`docs/evaluations/kpi-definitions.md`); executable B-gates (`docs/evaluations/effectiveness-gates.md`).

**Go / no-go.** GO to continue building the deterministic MVP (Stages 01–03). NO-GO for any "proven adaptive improvement" claim until a pre-registered experiment reports EVIDENCE_OF_IMPROVEMENT. NO-GO for canonical curriculum import until the original approved plan is reconciled and approved (C01).

## Unresolved risks carried forward

| Risk | Owner stage | Mitigation in place |
|---|---|---|
| Original curriculum not yet supplied | 02 | seed stays PROPOSED_SEED; importer blocks promotion |
| No local model benchmarked | 06 | NullRuntime; protocol defined |
| Self-adjudication of qualitative criteria | 04 | reliability class; machine-checkable weight target |
| Tauri shell not compiled in the build sandbox | 01/14 | CI job with system libraries; NOT RUN here |
| Status column of `defect-register.md` section B | 00 | RE-ASSESSED 2026-11-08 for the 138 rows that were OPEN: PARTIAL 76, IMPLEMENTED 21, ADDRESSED-IN-DOCS 18, OPEN 16, PROPOSED 7, with per-row evidence and remaining gaps in section E. Assessed by reading (no second independent pass); cited paths and tests mechanically checked. The 16 still-OPEN and 76 PARTIAL rows are the real backlog; six section B rows have unescaped `\|` characters in the auditors' original text (pre-existing) |
