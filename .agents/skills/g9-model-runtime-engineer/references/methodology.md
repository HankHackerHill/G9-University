# g9-model-runtime-engineer — methodology

Mandatory measurements: TTFT, prefill tok/s, output tok/s, p50/p95 by output size, VRAM/RAM, cold start, crash/recovery, cancellation, structured validity, truncation behaviour, correctness fixtures, security regressions, energy. Quality and privacy outrank speed.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
