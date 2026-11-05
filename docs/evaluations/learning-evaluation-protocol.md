# Learning effectiveness evaluation protocol (gates B01–B06)

Corrections preserved: no fixed sample size, p-value or "≥1 SD" promise (C08); single-learner designs report effect estimates with uncertainty and allow **INCONCLUSIVE** (C15); flashcard recall is not practical competence (C14, B02); confidence is a stated probability, not a 0–5 rating (§10).

## Measured outcomes (pre-registered per experiment window)

| Outcome | Definition | Source |
|---|---|---|
| Independent practical performance | rubric score on an **unseen** matched task in EXAM mode, `AssistanceLevel::Unaided` | `attempts`, `criterion_results` (non-advisory) |
| Delayed retention | recall-item success at +7 and +21 days; separately, practical transfer-check success at validity-window end | `review_attempts`, `practical_transfer_checks` |
| Transfer | success on a novel-context task of the same competency | `practical_transfer_checks` |
| Assistance use | hint tiers and `AssistanceLevel` per task | `attempts.assistance`, `exposure_log` |
| Calibration | Brier score and reliability bins over stated probabilities | `calibration_observations` |
| Workload / planning overhead (B06) | minutes in planning/admin vs credited study minutes | `time_intervals` by block |
| Decision quality (executive) | expected vs observed outcome of recorded decisions | `decisions`, `decision_outcomes` |

## Designs allowed for one learner

- **Baseline vs adaptive, alternating blocks (A-B-A-B)** with matched-difficulty tasks from the same equivalence group, counterbalanced order, washout between blocks for retention measures. Carryover and maturation are confounders and are declared.
- **Multiple-baseline across competencies**: adaptive intervention introduced at staggered times for different competencies; each competency serves as its own control.
- **Holdout tasks**: a sealed set of unseen tasks per competency, never exposed to teaching or retrieval (`holdout_leakage_check`).

Not allowed: withholding safety guidance or accommodations; randomizing within an official assessment; changing the pre-registered outcome after data collection.

## Analysis and reporting

- Report effect estimates (difference in rubric score, retention rate, transfer rate) with an uncertainty interval from a method appropriate to the design (e.g. randomization/permutation intervals for alternating designs, bootstrap for rates); state the number of observations.
- Outcomes: `EVIDENCE_OF_IMPROVEMENT`, `EVIDENCE_OF_NO_IMPROVEMENT`, `INCONCLUSIVE`. Inconclusive is expected often with few observations and is a valid, reportable result.
- Every report links each number to the records it was computed from (R-014) and is reproducible by re-running the stored query (`method_experiments.analysis_query`).
- Claims: the product may ship as a workflow tool without a demonstrated AI learning lift; labeling the adaptive engine as "superior" requires a pre-registered experiment with `EVIDENCE_OF_IMPROVEMENT` across at least two independent competency groups.

## Calibration

Discrete questions ask for a probability of correctness in [0, 1] (slider with numeric entry). Brier score, reliability diagram (bins) and over/under-confidence direction are computed in `g9-retention::calibration`. If a 0–5 scale is ever used for UX reasons, it must be mapped through an explicitly validated table stored with the experiment, never implicitly.

## Rubric validity (B01)

Before an assessment version is used for official grading: two independent reviewers (operator + mentor reviewer when available) score 5 unseen submissions; disagreements > 10 points on any criterion trigger rubric revision; results recorded in `assessment_versions.validity_review_ref`.
