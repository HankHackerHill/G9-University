# g9-data-integrity — methodology

Rules: STRICT tables; INTEGER epoch ms; per-FK deletion action with justification; immutability triggers on finalized rows; schema_migrations with sha256; dry run on VACUUM INTO copy; backup via Online Backup API; verify on copy; restore test before RETAINED; never binary downgrade.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
