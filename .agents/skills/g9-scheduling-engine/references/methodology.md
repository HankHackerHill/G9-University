# g9-scheduling-engine — methodology

Priority classes: Immutable → SafetyOrAssessmentDeadline → PrerequisiteReadiness → DueReview → OperatorPriority → PreferredWorkload. Civil time plus IANA zone; DST gaps shift forward, overlaps choose the earlier offset and flag it.

## Decision rules

- Every claim cites evidence refs; inferences carry confidence limits.
- The skill proposes; `g9-policy` decides; the operator approves.
- Abstain rather than guess when inputs are missing or untrusted.
