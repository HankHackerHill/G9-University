# ADR-0012: Non-compensatory pass rule with enumerated gates and a reliability class on every grade decision

**Status:** Accepted (2026-11-06)

## Context

v6.0's pass rule (≥ 80 total) is compensatory: a 0 on root-cause reasoning can still pass; gates were named but not enumerated; the operator grading their own exam is self-adjudication (LRN-10, LRN-11, LRN-14).

## Decision

PASS iff total ≥ 80 AND every criterion ≥ 50 % of its weight AND no gate failure. Gates: out-of-scope target/action; unsafe or destructive unauthorized action; fabricated or altered evidence; time limit exceeded without submission. Criteria are scored on anchored 0–4 levels stored with the assessment version. `grade_decisions.reliability_class ∈ {DETERMINISTIC, EXTERNAL_REVIEW, SELF_ADJUDICATED}`; official blind practicals target ≥ 60 % machine-checkable weight; SELF_ADJUDICATED decisions never satisfy validation evidence (ADR-0005). Status: rule adopted; implementation of the per-criterion minimum, anchors and reliability class is scheduled in the wave-2 fix pass (register LRN-10/LRN-11).

## Alternatives considered

Keep compensatory rule (rejected). Require external reviewer for every grade (rejected for MVP: unavailable; mitigated by reliability class and machine-checkable weight).

## Consequences

Grades reflect the weakest criterion; validation cannot be self-granted.

## Verification

`pass_threshold_80_boundary`, `critical_gate_failure_yields_final_fail`, new tests for per-criterion minimum and reliability class (wave 2).
