# Weekly scorecard template

- Source: plan sections 9.1 (the 100-point Friday score, its four parts and sub-criteria, the weekly gate, and "record time, failures, confidence and environment notes"), 9.2 (baseline, midpoint and final), 9.4 (the safety gate and the evidence-integrity gate), 1.3 (weekly pass 80 of 100, final 85 of 100) and 14.3 (scorecards present).
- Status: PROPOSED. This is a blank form. It holds no score and no attempt, and no scorecard made with it exists in the repository. Every number in a scorecard is entered from an attempt that really took place.
- Reader: the owner, as instructor and as learner (one person), who fills it, and the examiner, who ticks the quality checks. A second reader, where one exists, is named on it.
- Marking bands, descriptors and the evidence each sub-criterion needs: `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`. The Friday procedure, key release, unseen-practical protocols, remediation tracks and the mapping to the application's rubric: `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. One scorecard for every scored attempt: the weekly Friday attempts of weeks 1 to 11 (days 5, 10 and so on to 55), the midpoint (day 30) and the final (day 60, attempt `V2-W12-CAPSTONE`). The day 1 diagnostic is an ungraded placement and has no scorecard.

Order of work (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md` sections 2 and 3).

1. Before the week, outside counted hours: open the scorecard with the identity, the protocol of the unseen practical and its commitment hash.
2. During the Friday: record the times, the hints and aids, and the hashes as they happen. Lock the knowledge answers and the evidence bundle by hash and time before any key is opened.
3. After submission: open the key and the fixture map, and log each opening with its time.
4. Record the two gates first, and add points afterwards. A gate failure is recorded before any point is given and is never offset by points.
5. Score Part 1 from the key, then Parts 2 to 4 sub-criterion by sub-criterion from the evidence bundle only, each with one line naming the artifact.
6. Add the points, compare each part with its diagnostic target and the total with the gate.
7. Derive the six levels of the application's rubric, enter them in the window, and copy the application's attempt id and decision here.
8. Write your confidence, the failures and the environment notes, assign the remediation, and set the retest date.

Rules.

- A claim with no artifact in the bundle scores in the none band of the sub-criterion it supports. Where two bands are plausible, take the lower. Half points exist only in Part 1.
- The scorecard is append-only once finished. A correction or a later remediation line is a new dated entry in the supersession log, and the original stays (finalized records are superseded, not edited).
- The plan record and the application record are reported side by side and are never averaged or converted into each other. A week is never called passed without saying which record says so.
- The scorecard holds no answer, no fixture map and no fault list. It holds hashes, times, points and one-line references to artifacts.
- A blocked lab is recorded BLOCKED. It is scored on what was demonstrated and is never described as passed.

Done when every section is filled or says NOT RUN with the reason, the arithmetic has been checked twice, and the quality checks below are ticked.

## The 100 points

The parts, the sub-criteria and the targets are the plan's (section 9.1). The bands are defined in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` section 3.

| Part | Points | Sub-criteria and points | Diagnostic target |
|---|---:|---|---:|
| 1 Knowledge | 20 | 20 questions of 1 point: at least 6 concept, 6 interpretation, 4 control, 4 synthesis | at least 16 |
| 2 Independent practical | 40 | P1 scope and design correctness 5; P2 enumeration and evidence fidelity 10; P3 independent hypothesis and reproduction 15; P4 impact and root-cause reasoning 10 | at least 30 |
| 3 Defensive validation | 25 | D1 correct data source and hypothesis 8; D2 positive validation 7; D3 negative testing 5; D4 hardening and mitigation assessment 5 | at least 18 |
| 4 Evidence and reporting | 15 | E1 reproducibility 5; E2 precision and provenance 5; E3 remediation and retest 5 | at least 12 |
| Total | 100 | | |

The plan gate, in three conditions that must all hold:

1. the total is at least 80 of 100 (the midpoint aim is 80 on the same rubric; the final target is 85, with the scope, isolation and evidence-integrity gates complete);
2. the safety gate and the evidence-integrity gate both pass;
3. the week's own conditions, listed in the `## Pass conditions` of its week guide, are met (INFERRED reading, `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 3).

The category targets are diagnostic. A part below its target steers remediation. It never replaces the gate and it never fails a week by itself.

The application's own rule is stricter and is recorded separately: the weighted total is at least 80, every one of the six criteria reaches level 2 (50 percent of its weight), and no gate failure is recorded (ADR-0012; `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 10).

## Status vocabulary

| Where | Words |
|---|---|
| Each gate | PASS, FAIL |
| Each part against its target | MET, NOT MET |
| The plan gate | MET, NOT MET, BLOCKED (a lab was blocked and the gate could not be judged on the demonstrated work) |
| A week condition | MET, NOT MET, NOT DEMONSTRATED (the work was substituted or blocked) |
| The application decision | PASS, FAIL, with the reliability class DETERMINISTIC, EXTERNAL_REVIEW or SELF_ADJUDICATED, as the window records it |

## Fields

| Section | What it holds |
|---|---|
| 1 Identity | week, course day id, assessment code (`V2-Wnn-EXAM`, or `V2-W12-CAPSTONE`), kind, the competency id, the original calendar date and the actual date with zone, and the shift if the day moved |
| 2 Protocol | which unseen-practical protocol was used (1 another person, 2 a public challenge from a pool fixed in advance, 3 a sealed fault-injection script), who prepared it, the pool and the draw if any; the midpoint and the final admit protocols 1 and 2 only |
| 3 Times | the window, the start and end of the attempt, the elapsed time from the session timer (not from a wall-clock subtraction), the time limit, and the scoring steps left unfinished at the end of the day |
| 4 Hints and aids | the hint level used (0 in a scored attempt), AI used (no), a published solution read before submission (none), a key opened before submission (none) |
| 5 Hashes and key log | the SHA-256 and time of the locked knowledge answers, of the evidence bundle and of the fixture-map commitment (the hash only, never the map); every key opening with file, time and reason |
| 6 Gate check | the safety gate and the evidence-integrity gate, each PASS or FAIL with the artifact that decides it |
| 7 Points | Part 1 by question type, P1 to P4, D1 to D4, E1 to E3 each with band, points and one artifact line, the four part totals, the total |
| 8 Targets and plan gate | each part against its diagnostic target, and the three conditions of the plan gate |
| 9 Week conditions | each condition of the week's gate line, with its status and artifact; the substitutions |
| 10 Application record | the attempt id, the six levels, the weighted total, the gate failures, the decision, the reliability class, the reviewer, and the reason for any disagreement with the plan record |
| 11 Confidence and notes | your confidence before you knew the score, the failures, the environment notes |
| 12 Remediation | the tracks assigned, the optional work substituted, when it counts as done, the retest date |
| 13 Supersession log | dated corrections and later remediation lines |

Where a week's gate line in the plan names a percentage ("at least 80% quiz", "at least 80%", "at least 80% knowledge"), write in section 9 which measure you used. Read it as the knowledge exam at 16 of 20 unless the week guide says another measure (INFERRED, `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 12).

## Blank scorecard

Copy the block. Keep the section numbers.

```markdown
# Weekly scorecard: <assessment code>

## 1. Identity
Week / course day id:            Assessment code / kind:
Competency id:
Original calendar date:          Actual date (zone):          Shift and reason:

## 2. Protocol of the unseen practical
Protocol (1, 2 or 3):            Prepared by:
Pool and draw (method, seed, result):

## 3. Times
Window (start / end, zone):
Attempt start / end (zone):      Elapsed from the session timer:       Time limit:
Scoring steps unfinished at the end of the day:

## 4. Hints and aids
Hint level used (0 in a scored attempt):
AI used (no):
Published solution read before submission (none):
Key opened before submission (none):

## 5. Hashes and key log
| Item | SHA-256 | Time (zone) |
|---|---|---|
| Locked knowledge answers | | |
| Evidence bundle | | |
| Fixture-map commitment (hash only) | | |
| Key opening | File | Time | Reason |
|---|---|---|---|

## 6. Gate check (recorded before any point)
Safety gate: PASS / FAIL. Artifact:
  Checked: out-of-scope target or activity; real production credentials; an exposed vulnerable service not contained or remediated; unauthorized public cloud provisioning; destruction of others' data; knowingly falsified evidence.
Evidence-integrity gate: PASS / FAIL. Artifact:
  Checked: missing target or scope context; fabricated commands or outputs; a test claimed passed with no observed result; a destructive tool run without a record; an unacknowledged published solution; a false positive claimed as a verified finding.
Acceptable partial performance noted (blocked setup with diagnostics, correct false positive, unfinished challenge with accurate enumeration, partial detection with named gaps):

## 7. Points
Part 1 Knowledge
| Type | Items | Points |
|---|---:|---:|
| Concept (at least 6) | | |
| Interpretation (at least 6) | | |
| Control (at least 4) | | |
| Synthesis (at least 4) | | |
| Part 1 total (of 20) | 20 | |
Parts 2 to 4
| Sub-criterion | Max | Band | Points | V item (yes / no / infeasible) | One artifact line |
|---|---:|---|---:|---|---|
| P1 scope and design correctness | 5 | | | | |
| P2 enumeration and evidence fidelity | 10 | | | | |
| P3 independent hypothesis and reproduction | 15 | | | | |
| P4 impact and root-cause reasoning | 10 | | | | |
| D1 correct data source and hypothesis | 8 | | | | |
| D2 positive validation | 7 | | | | |
| D3 negative testing | 5 | | | | |
| D4 hardening and mitigation assessment | 5 | | | | |
| E1 reproducibility | 5 | | | | |
| E2 precision and provenance | 5 | | | | |
| E3 remediation and retest | 5 | | | | |
Part 3 type (F fix or control, T detection or telemetry):

## 8. Targets and plan gate
| Part | Points | Target | MET / NOT MET |
|---|---:|---:|---|
| 1 Knowledge | | 16 | |
| 2 Practical | | 30 | |
| 3 Defensive validation | | 18 | |
| 4 Evidence and reporting | | 12 | |
| Total | | 80 (final 85) | |
Plan gate: total at least the threshold: yes / no. Both gates PASS: yes / no. Week conditions MET: yes / no.
Plan gate result: MET / NOT MET / BLOCKED

## 9. Week conditions
| Condition (from the week guide) | Status | Artifact |
|---|---|---|
Measure used where the plan names a percentage:
Substitutions (what replaced what, and why):
Isolation state (tests current and passed, NOT RUN, or lab BLOCKED):

## 10. Application record (copied from the window; never computed here)
Attempt id:                      Assessment code:
Decision (PASS / FAIL):          Reliability class:             Reviewer:
Weighted total:                  Gate failures recorded:
| Criterion (weight) | Set | Points of set / max | Share | Level (0 to 4) |
|---|---|---|---|---:|
| independent_execution (25) | P2 + P3 | | | |
| root_cause_reasoning (20) | P4 | | | |
| remediation_verification (20) | F: D2 + D3 + D4 + E3. T: D4 + E3 | | | |
| detection_visibility (15) | F: D1. T: D1 + D2 + D3 | | | |
| evidence_documentation (10) | E1 + E2 | | | |
| scope_compliance_reporting (10, gate) | P1 | | | |
Criteria below level 2:
Reason where the plan gate and the application decision differ:

## 11. Confidence and notes
Confidence before the score was known (time, and per part):
Failures (what did not go as expected, and what was learned):
Environment notes (Stage 00 state, isolation tests and their ids and dates, baseline id, tool versions, substitutions):

## 12. Remediation assigned
| Trigger | Track (K, P, D, E, W, S, I, B or R) | Optional work substituted (no added hours) | Counts as done when | Retest date |
|---|---|---|---|---|

## 13. Supersession log
| Date (zone) | What changed | Why |
|---|---|---|
```

## Quality checks before you submit

An examiner ticks each line from the scorecard and the evidence bundle only.

Gates and integrity of the record

- [ ] The safety gate and the evidence-integrity gate are recorded before any point, each with the artifact that decides it, and a failure is not offset by points.
- [ ] The hash and time of the knowledge answers and of the evidence bundle were recorded before any key was opened; if one is missing, the scorecard says so and E2 earns no verification point.
- [ ] The fixture-map commitment hash was recorded before the attempt (where the protocol uses one), and no map, answer or fault list appears anywhere in the scorecard.
- [ ] The key log lists every opening with file, time and reason, and no key was opened before submission, or the early opening is declared and its effect on the score is written.
- [ ] Hints used are 0 and AI used is no; any hint or published solution makes P3 fall in the none band and is declared.

Points

- [ ] Every sub-criterion has a band, points and a one-line reference to an artifact; a claim with no artifact is in the none band.
- [ ] Where two bands were plausible, the lower was taken; half points appear only in Part 1.
- [ ] The part totals and the total add up, were checked twice, and no rounding was applied.
- [ ] Each part is compared with its diagnostic target, and the plan gate states all three conditions.
- [ ] The threshold is right for the attempt: 80 for a week and for the midpoint aim, 85 and complete scope, isolation and evidence-integrity gates for the final.

Records side by side

- [ ] The application record has the attempt id, the six levels, the weighted total, the gate failures, the decision and the reliability class, copied from the window and not computed here.
- [ ] The plan result and the application result are shown side by side, not averaged, and a disagreement has its reason written.
- [ ] A SELF_ADJUDICATED decision is named as such; a second reviewer, if any, is named.

Honesty and follow-up

- [ ] A blocked lab is recorded BLOCKED, the substitution is stated, and nothing is described as passed that was not run.
- [ ] Confidence was written before the score was known, and the failures and the environment notes are specific (Stage 00 state, isolation tests with ids and dates).
- [ ] Every part below its target has an assigned remediation track, optional work only, no added hours, and a retest date.
- [ ] The scorecard was not overwritten: later changes are dated entries in the supersession log.

## Sample (synthetic)

Nothing below is a result. The numbers are the invented numbers of example A in section 11 of `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`, repeated to show how the two records are written side by side.

```text
Sample (synthetic): not a record of any attempt.
Plan record:        total 85.0 of 100; safety gate PASS; evidence-integrity gate PASS;
                    week conditions MET. Plan gate MET. Parts below target: Part 1 (14 of
                    20) and Part 3 (17 of 25); they steer remediation and do not change
                    the gate.
Application record: attempt id SYN-ATT-001; weighted total 84.75; decision FAIL,
                    SELF_ADJUDICATED. Reason: detection_visibility at level 1 is below its
                    floor of level 2, although the total is above 80.
Statement:          the plan gate is MET and the application decision is FAIL. The week
                    is not called passed without naming the record.
```

## Departures from the plan

| What | Why |
|---|---|
| The four parts and their sub-criteria are kept for the midpoint and the final | the plan gives no separate point split for either. PROPOSED (`docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` section 9) |
| A third plan-gate condition, the week's own pass conditions, is recorded | plan section 9.1 sets the overall Friday gate and plan section 5 sets a gate line for each week, and neither says that one replaces the other. INFERRED |
| BLOCKED and NOT DEMONSTRATED are added to the status words | the plan accepts blocked work as partial performance (section 9.4) and uses BLOCKED for days (section 10.1); a scorecard needs a word for a gate that cannot be judged. PROPOSED |
| Bands, the verification item and the half points of Part 1 come from the rubrics file | the plan fixes the sub-criteria and points but not the bands. PROPOSED there |
| The application's attempt id, levels and decision are recorded next to the plan score | the application's rule is stricter and is not changed (ADR-0012, ADR-0023 decision 8); the plan's 100-point score is recorded here and the two are never averaged |
| The key log, the commitment hash and the supersession log are added | the plan asks to store answers outside the student's distribution and to record time, failures, confidence and environment notes (section 9.1); the logs make the order of events checkable. PROPOSED |
