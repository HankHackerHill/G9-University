# FSRS evaluation protocol (Stage 09 option gate)

Scope: FSRS schedules **declarative recall items** only. Practical cybersecurity competencies are validated through unseen transfer checks on their own validity windows (B02, handoff §10, §23 item 46). The default scheduler is the fixed baseline (`g9-retention::baseline`, offsets 1/3/7/21/60 days). The `fsrs` cargo feature wraps `rs-fsrs` 1.2.1 (MIT) and is **off by default**.

## Questions the evaluation must answer

1. Does FSRS reduce review workload at equal or better 7/21-day recall compared with the baseline for this learner's items?
2. Is the desired-retention parameter (0.90 candidate) appropriate given the 15-minute daily recall block? Evaluate 0.85, 0.90, 0.95 and report workload vs recall.
3. Are the library's parameters stable with the small item counts expected (hundreds, not tens of thousands)? Default parameters are used; optimizer training is out of scope (it would require `fsrs-rs` and an ML stack).

## Design

- Items split by competency into two matched sets; one scheduled by baseline, one by FSRS, for 6 weeks; then crossover for 6 weeks.
- Outcomes: daily review minutes (budget overflow events), recall success at due time, +7/+21-day delayed probes on a random 20 % sample, lapses.
- Report effect estimates with uncertainty; allow INCONCLUSIVE.

## Enabling

Operator decision recorded in an ADR with the evaluation report attached; `fsrs` feature enabled in the release build; `ItemScheduler` name recorded on every `review_attempts` row so results remain attributable.
