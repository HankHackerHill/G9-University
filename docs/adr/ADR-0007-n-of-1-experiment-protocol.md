# ADR-0007: Pre-registered single-learner designs with uncertainty and an INCONCLUSIVE outcome; no fixed n/p thresholds

**Status:** Accepted (2026-11-06)

## Context

v5.1 prescribed alternating weeks, paired t-tests, p<0.05, n≈20 and a ≥1 SD gain (C08, LRN-01, LRN-02, LRN-03, LRN-21 BLOCKER). Weeks are confounded with curriculum content and maturation; there is one learner, not thirty.

## Decision

Allowed designs: objective-within-day or counterbalanced day-pair randomization blocked by week and difficulty; multiple-baseline across competency clusters; sealed holdout tasks. Pre-registration record stored immutably in `method_experiments` (design, unit, allocation seed and sequence, outcomes, analysis query, stop rule) before data collection. Reports give effect estimates with randomization/bootstrap intervals and one of EVIDENCE_OF_IMPROVEMENT / EVIDENCE_OF_NO_IMPROVEMENT / INCONCLUSIVE. Safety guidance is never withheld.

## Alternatives considered

Week alternation (rejected). Classical t-tests on tiny samples (rejected). Contextual bandits (deferred until a robust baseline and enough data exist).

## Consequences

Honest effectiveness claims (gates B03, B04).

## Verification

`docs/evaluations/learning-evaluation-protocol.md`; L05 skill schema requires an `inconclusive` outcome.
