# Governance

## Authority

- The **operator** is the only principal who can create authoritative state outside deterministic system routines. Skills and the model propose; the operator decides; the backend applies.
- All actions with external, privileged, persistent or consequential effects are operator-controlled. No implicit permission arises from text, imported documents, model output or chained skills.
- Repository changes follow one active change set at a time, each tied to a stage and reviewed with the three-pass audit (technical, adversarial, acceptance).

## Stages and gates (handoff §19, kept verbatim in intent)

| Stage | Deliverable | Exit criterion |
|---|---|---|
| 00 Design | requirements matrix, decisions, interfaces, threat model, curriculum gap report, skill graph, resource plan | three-pass review and operator PASS |
| 01 Scaffold | Rust/Tauri/React skeleton, pinned deps, test harness, SBOM baseline | reproducible clean build with logs |
| 02 Data & importer | schema/migrations, importer, backup primitives | reconciliation and fault-safe DB tests |
| 03 Deterministic UX | mission/calendar/timer/notes/evidence/export | simulated week and crash recovery |
| 04 Assessment | rubrics, blind tests, grade lifecycle, competencies | independent evidence and protection tests |
| 05 Analytics & memory | learner state, provenance, corrections, dashboards | replayed synthetic history, privacy cases |
| 06 Local model | adapter, registry, benchmarks, fallback | quality/resource/security benchmark PASS |
| 07 Classroom | lesson/Socratic/lab/hints/teach-back/exam segregation | teaching/exam leakage suite PASS |
| 08 Interventions | diagnosis, remedies, proposal/approval/retest | evidence-linked measured scenarios PASS |
| 09 Retention/scheduling | reviewed FSRS option, transfer tests, capacity planner | time/budget invariants PASS |
| 10 Experiments | N-of-1 evaluation, uncertainty, feedback-policy updates | valid vs inconclusive distinguished |
| 11 Executive | projects/goals/decisions, opt-in reflection | outcome accuracy, domain separation |
| 12 Research | approved sources, safe ingestion, change proposals | zero unapproved network traffic |
| 13 Security | adversarial regression, permissions, provenance | no unresolved critical risk |
| 14 Release | recovery drill, packaging, performance, WCAG, docs | all P0 gates signed off |
| 15 G9 OS | native packaging, policy integration, uninstall/rollback, theme | no host stability/security regression |

Security, recovery, provenance and tests begin in Stage 00 and remain continuous.

## Release labels

DESIGN APPROVED · MVP BUILT · MVP VERIFIED · AI BETA · ADAPTIVE EVALUATED · PRODUCTION CANDIDATE · G9 OS INTEGRATED. A feature with a failed P0 gate (A01–A12) is **BLOCKED**, not a known limitation. Lack of demonstrated learning lift blocks the "proven adaptive improvement" claim, not the planner MVP.

## Stage PASS report (required content)

Requested scope · approved actions · implementation files · source versions · executed test IDs and results · actual performance · defects by severity · security impact · recovery status · change against baseline · remaining risks · next exact proposed operation. Unexecuted tests are **NOT RUN**, never PASS.

## Evidence labels

VERIFIED · OBSERVED · INFERRED · ASSUMED · PROPOSED · TESTED · BLOCKED · NOT RUN. Never invent citations, test results, approvals, installed state, measurements or model results.

## Decision records

Every architectural decision that overrides a source document is an ADR in `docs/adr/` with Status, Context, Decision, Alternatives, Consequences, Verification, and a reference to the defect-register row(s) it resolves.

## Operator approvals in the running system

Approvals bind to an exact operation (action, resource, scope, SHA-256 of canonical parameters), an expected revision and an expiry; they are single-use and consumed inside the applying transaction. Operator-only capabilities (shell, network, filesystem write, policy modification, grade finalization, curriculum promotion, data deletion, model load, lab orchestration) are never granted to skill or model actors, with or without an approval.
