# g9-domain-architect — methodology

Layering: L0 g9-domain; L1 g9-policy, g9-storage; L2 pure domain services; L3 g9-ai-adapter; L4 g9-skills; L5 g9-app; L6 Tauri shell. UI never queries SQLite; the model never calls native APIs.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
