# g9-spec-assurance — methodology

Matrix rules: one row per requirement; columns gate, enforcing crate, test id pattern, status. A row is COVERED only when the test exists and is not ignored. Conflicts cite both locations and the ADR that resolves them.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
