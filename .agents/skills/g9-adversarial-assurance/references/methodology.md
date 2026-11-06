# g9-adversarial-assurance — methodology

Every test records: input, trust class, policy decision (rule id), backend outcome, state after. Categories: injection, memory poisoning, package tampering, TOCTOU, exam leakage, exfiltration, XSS, storage corruption, log leakage, exhaustion, lab escape, model integrity.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
