# KPI definitions (M13; register LRN-19, LRN-22)

Every KPI has a numerator, denominator, window, inclusion rule, minimum n and known biases, and is computed by a stored, reproducible query over authoritative tables. No KPI is shown without its n; below the minimum n the UI shows `INSUFFICIENT DATA`, never an extrapolated value.

| KPI id | Definition | Source tables | Window | Min n | Exclusions / known biases |
|---|---|---|---|---|---|
| `first_attempt_pass_rate` | APPROVED_FINAL first attempts with pass = true ÷ first attempts, EXAM mode | `attempts`, `grade_decisions` | rolling 4 weeks | 4 | retakes excluded (reported separately as `eventual_pass_rate`); SELF_ADJUDICATED decisions flagged |
| `eventual_pass_rate` | definitions with ≥ 1 passing attempt ÷ definitions attempted | same | program to date | 4 | inflated by retakes by construction; always paired with first-attempt rate |
| `criterion_level_mean` | mean anchored level (0–4) per rubric criterion over APPROVED_FINAL attempts | `criterion_results` | rolling 4 weeks | 3 per criterion | advisory (model) results excluded |
| `probe_recall_7d` / `probe_recall_21d` | correct ÷ probed on a seeded random 10 % sample of items with a successful review 7 ± 1 (21 ± 2) days earlier, probed regardless of due date | `review_attempts` (`probe = true`) | weekly | 10 probes | scheduler-independent by design; Wilson 95 % interval reported |
| `transfer_pass_rate` | passed transfer checks ÷ transfer checks due, unaided, novel context | `practical_transfer_checks` | rolling 8 weeks | 3 | novelty requirement recorded per check |
| `hint_dependence` | share of LAB/STUDY observations with assistance ≥ HINTED | `competency_observations` | rolling 2 weeks | 10 | EXAM excluded |
| `misconceptions_corrected` | hypotheses of kind MISCONCEPTION moved ACCEPTED → RETIRED with a passing retest | `learner_hypotheses`, `intervention_outcomes` | program to date | 1 | model-proposed hypotheses count only after operator acceptance |
| `calibration_brier` | mean Brier score over stated probabilities; reliability bins | `calibration_observations` | rolling 4 weeks | 20 | discrete questions only |
| `intervention_outcome_rate` | interventions EVALUATED with retest ≥ acceptance ÷ interventions EVALUATED | `interventions`, `intervention_outcomes` | program to date | 3 | pre-declared acceptance only |
| `minutes_per_validated_competency` | credited VERIFIED minutes ÷ competencies reaching VALIDATED | `time_intervals`, `competency_state_snapshots` | program to date | 3 | UNCERTAIN minutes excluded |
| `review_backlog` | due review items not completed by end of day | `review_items` | daily | — | overflow events listed separately |
| `planning_overhead_ratio` | minutes in planning/admin blocks ÷ credited study minutes (B06) | `time_intervals` by block | rolling 2 weeks | 5 days | — |
| `milestone_reliability` | milestones completed by due date ÷ milestones due | `milestones` | rolling 8 weeks | 3 | BUSINESS domain |
| `prediction_error` | median absolute error of time-to-completion estimates | `commitments`, `execution_observations` | rolling 8 weeks | 5 | — |
| `decision_review_rate` | decisions with a recorded outcome review ÷ decisions older than 30 days | `decisions`, `decision_outcomes` | program to date | 3 | — |

No composite "human worth" score and no forced comparative ranking are computed (handoff §17).
