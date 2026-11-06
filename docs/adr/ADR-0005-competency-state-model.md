# ADR-0005: Dimension-specific, evidence-rule-derived competency state; no mastery float; model observations rejected

**Status:** Accepted (2026-11-06)

## Context

v5.1 M06 returned `{mastery: float}` (C10, LRN-06). The v6.0 chain UNASSESSED→…→RETEST_DUE lacked rules, demotion and per-dimension state (LRN-09, BLOCKER). The operator is also the learner, so self-adjudicated grades could reach VALIDATED (LRN-11).

## Decision

`g9-competency` derives state from retained (non-superseded) observations with explicit rules (`DerivationRules`): Introduced on any evidence; Guided on guided/hinted application ≥ 60; Independent on ≥ 2 unaided application observations ≥ 70 at difficulty ≥ 3 within 60 days; Validated only on EXAM-mode unaided Application/Transfer evidence ≥ 80; RetestDue when validation is older than the validity window; Regress on two recent sub-threshold unaided results, retaining earlier evidence. Per-dimension estimates (n, n_unaided, latest, independence-weighted mean, uncertainty) accompany the summary state. Observations with model provenance are rejected at the boundary. Addendum (LRN-11): grade decisions carry a `reliability_class` (DETERMINISTIC, EXTERNAL_REVIEW, SELF_ADJUDICATED); validation evidence whose decision is SELF_ADJUDICATED yields at most `Independent` with a `provisional` flag — implemented in wave 2 (`docs/audit/improvement-backlog.md`).

## Alternatives considered

Single float (rejected). Bayesian knowledge tracing (deferred: needs calibration data; may be added as an alternative projection, never as authority).

## Consequences

Replayable, explainable, correctable state (gates A05, B05).

## Verification

`a05_projection_is_replayable_and_order_independent`, `a05_model_sourced_observation_rejected`, `validated_requires_exam_mode_unaided`.
