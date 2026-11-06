# g9-retrieval-and-transfer — methodology

Baseline offsets 1/3/7/21/60; reset on fail; FSRS optional and declarative only; transfer checks by validity window with explicit novelty.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
