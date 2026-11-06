# Executable effectiveness gates B01–B06 (register LRN-23)

Each gate has a procedure, a pass criterion and a report location. A gate that was not executed is reported NOT RUN.

| Gate | Procedure | Pass criterion | Report |
|---|---|---|---|
| B01 Rubric validity | (i) Content-validity matrix: every criterion maps to ≥ 1 objective and ≥ 1 NICE TKS reference; (ii) deterministic checkers scored on known-answer fixtures; (iii) qualitative criteria re-graded blind ≥ 7 days later or by a second reviewer on ≥ 20 artifacts | (i) 0 orphans; (ii) 100 % on fixtures; (iii) exact-level agreement ≥ 80 % or weighted kappa ≥ 0.6 | `docs/evaluations/reports/b01-<version>.md` |
| B02 Retention vs transfer | Probe recall (`probe_recall_7d/21d`) and `transfer_pass_rate` computed from separate tables; a static check that no transfer check is scheduled by a memory scheduler | both KPIs present with n ≥ minimum; separation test passes | `b02-<date>.md` |
| B03 Intervention vs baseline | Pre-registered multiple-baseline design across ≥ 3 competency clusters; retests on sealed variants | report produced with effect estimate and interval; comparability documented | `b03-<experiment>.md` |
| B04 Effect size and uncertainty | Every experiment report states estimate, interval, n, design, and one of EVIDENCE_OF_IMPROVEMENT / EVIDENCE_OF_NO_IMPROVEMENT / INCONCLUSIVE | no report without an uncertainty statement; INCONCLUSIVE allowed | same |
| B05 Correction propagation | Reject an accepted hypothesis in the Memory Explorer; confirm every dependent plan/intervention/recommendation is listed as invalidated and the next agenda proposal changes | all dependents invalidated; agenda diff shows the change | `b05-<date>.md` + `b05_rejected_hypothesis_invalidates_dependents` |
| B06 Planning overhead | `planning_overhead_ratio` over 2 weeks | ≤ 0.15 (provisional target; operator may set) | KPI dashboard |

Release label ADAPTIVE EVALUATED requires B01–B05 PASS and B06 within target.
