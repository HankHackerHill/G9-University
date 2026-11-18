# G9 Cybersecurity Mastery v2.0: grading rubrics

- Readers: the instructor and the learner. The rubrics are published so that scoring is no surprise. This file holds no answer, no fixture map and no fault list of any item; a reader learns how work is marked, never what an exam contains.
- Source: plan sections 9.1 (the four score parts and their sub-criteria), 9.2 (baseline, midpoint and final), 9.4 (automatic failures and acceptable partial performance), 1.3 and 10.4 (dual evidence, no AI in a benchmark). Process, key release and the mapping to the application's rubric are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`.
- Status: PROPOSED, written 2026-11-18. Nothing here records a graded attempt. Every number in section 11 is a synthetic example.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.

## 1. How to use this file

| Order | Step | Where |
|---|---|---|
| 1 | Check the two gates first: safety and evidence integrity. A failure is recorded before any point is given | section 8 |
| 2 | Score Part 1 from the answer key | section 4 |
| 3 | Score Parts 2, 3 and 4 sub-criterion by sub-criterion, from the evidence bundle only | sections 5 to 7 |
| 4 | Add the points. Compare each part with its diagnostic target and the total with the gate | section 2 |
| 5 | Derive the application's levels from the same sub-criterion points | `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, mapping |
| 6 | Write the weekly scorecard | section 12 |

## 2. The 100 points

| Part | Points | Sub-criteria and points | Diagnostic target (plan section 9.1) |
|---|---:|---|---:|
| 1 Knowledge | 20 | 20 questions of 1 point: at least 6 concept, 6 interpretation, 4 control, 4 synthesis | at least 16 |
| 2 Independent practical | 40 | P1 scope and design correctness 5; P2 enumeration and evidence fidelity 10; P3 independent hypothesis and reproduction 15; P4 impact and root-cause reasoning 10 | at least 30 |
| 3 Defensive validation | 25 | D1 correct data source and hypothesis 8; D2 positive validation 7; D3 negative testing 5; D4 hardening and mitigation assessment 5 | at least 18 |
| 4 Evidence and reporting | 15 | E1 reproducibility 5; E2 precision and provenance 5; E3 remediation and retest 5 | at least 12 |
| Total | 100 | | |

- Weekly gate: at least 80 of 100, and the safety gate and the evidence-integrity gate both pass. The diagnostic targets are not a substitute for that gate and never replace it; a part below its target tells you where to remediate.
- Midpoint aim: at least 80 of 100 on the same rubric. Final target: at least 85 of 100 with no safety or evidence-integrity violation (section 9).
- The application grades the same attempt under its own, stricter rule on six criteria. Its levels are derived from the sub-criterion points of this file (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`).

## 3. Band rules

Every sub-criterion has three bands. The thresholds are shares of the sub-criterion's maximum and are chosen to coincide with the application's anchored levels (level 3 is 80 percent of a criterion's weight, level 2 is 50 percent). PROPOSED; the plan fixes the sub-criteria and their points, not the bands.

| Band | Share of the maximum | Meaning |
|---|---|---|
| Full | 80 percent or more | the required observable items are all present |
| Partial | 50 percent to below 80 | the sub-criterion is met with a material gap, such as one required item missing or weak |
| None | below 50 percent | absent, or attempted and materially wrong; includes work that scores a point or two for effort |

Points by band:

| Sub-criterion | Maximum | Full | Partial | None |
|---|---:|---|---|---|
| P1, D3, D4, E1, E2, E3 | 5 | 4 to 5 | 3 | 0 to 2 |
| P2, P4 | 10 | 8 to 10 | 5 to 7 | 0 to 4 |
| P3 | 15 | 12 to 15 | 8 to 11 | 0 to 7 |
| D1 | 8 | 7 to 8 | 4 to 6 | 0 to 3 |
| D2 | 7 | 6 to 7 | 4 to 5 | 0 to 3 |

Rules of scoring:

1. Score only what the evidence bundle shows. The bundle is the one hashed at submission. A claim with no artifact scores in the none band of the sub-criterion it supports.
2. Required items are labelled R1, R2 and so on. Full needs every R item. One missing or weak R item is partial. Two or more missing is none.
3. The verification item, labelled V, is a second independent observation of the same fact (plan section 10.4: tool output plus a second observation whenever feasible). The maximum of the full band needs V. Without V the best score is one point below the maximum. Where V is infeasible and the evidence says why, the best score is still one below the maximum.
4. Within a band, start at its lowest point when the R items are just met. Add a point for each place where the evidence clearly exceeds the descriptor, up to the band's top.
5. When the evidence sits between two bands, take the lower band.
6. Half points exist only in Part 1.
7. No rounding. A total may end in .5.
8. Hints and published solutions do not exist in a scored attempt. Any use of one, disclosed or not, puts P3 in the none band. An undisclosed use is also an evidence-integrity failure (section 8).
9. An out-of-scope action puts P1 in the none band, at most 2, and is a safety-gate failure (section 8).
10. A blocked lab or a missing dependency is scored as demonstrated: the work that was done is scored; the work that was not done earns nothing; the block itself is not punished beyond that. Documented diagnostics are evidence of P1, P2 and D1.
11. Work added after the time limit is an addendum and is not scored.

## 4. Part 1: Knowledge (20 points)

A question is worth 1 point. Multiple-choice questions are 1 or 0. A constructed-response question is 1, 0.5 or 0. The half point is PROPOSED (see departures). The marking points and the rejected overclaims are in the question's key, which is instructor material; this table says how the key's content is applied.

| Type | Band | Points | Descriptor | Observable evidence |
|---|---|---:|---|---|
| Concept (at least 6) | Full | 1 | names the mechanism and the qualifier or limit the key requires; or the correct option | the locked answer file against the key's Answer and Why lines |
| Concept | Partial | 0.5 | right mechanism, missing the key's limit or qualifier; constructed response only | the same, with the missing element identified |
| Concept | None | 0 | wrong; states an overclaim the key lists as rejected; wrong option | the same |
| Interpretation (at least 6) | Full | 1 | reads the supplied `Sample (synthetic):` log, packet, code or output correctly, says what it shows and what it does not, cites the field, line or record | the answer cites the record; the key's reading |
| Interpretation | Partial | 0.5 | correct reading, but no limit stated or no citation | the answer against the key |
| Interpretation | None | 0 | misreads the sample, or concludes beyond what it shows | the answer against the key |
| Control (at least 4) | Full | 1 | picks the control that acts on the cause and says how to verify it with a positive and a negative test | the answer names the control, the cause and both tests |
| Control | Partial | 0.5 | right control, not tied to the cause, or no verification | the answer |
| Control | None | 0 | a control for the symptom, or one that does not act at the boundary in question | the answer |
| Synthesis (at least 4) | Full | 1 | combines at least two sources or ideas into one conclusion, states its confidence and the next discriminating test | the answer names both sources, the confidence and the test |
| Synthesis | Partial | 0.5 | combines the sources, with no confidence or no next test | the answer |
| Synthesis | None | 0 | restates one source, or concludes beyond the combined evidence | the answer |

Notes:

- A multiple-choice option that the key marks as an overclaim scores 0 like any wrong option; the key's distractor notes say why each option is wrong.
- The answer file is locked at the end of block A: its SHA-256 and the time are written down. A later edit is an addendum and is not scored.
- Closed book. No notes, no reference, no AI.
- Do not reward a memorized tool name or flag. A correct command with a wrong mechanism scores 0 or 0.5 by the descriptor.

## 5. Part 2: Independent practical (40 points)

The practical is an unseen or modified scenario (plan section 9.3). What the learner cannot assume is part of the task. Hints and AI do not exist in the scored attempt. The sub-criteria are scored from the evidence bundle.

### P1 Scope and design correctness (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 a scope block exists before the first action: target ids and addresses, allowed actions, exclusions, time limit, stop rule. R2 a test plan orders the hypotheses and says what each test would show. R3 every logged action names a target that is in the scope. V (5): the identity of each target was confirmed from an independent source (platform scope page, lab inventory) before the first action | the first record of the attempt, timestamped before the first command log line; a target column in the command log; the inventory excerpt |
| Partial | 3 | the scope block lacks one element (exclusions, time limit or stop rule), or the plan gives no reason for its order; all actions are in scope | the scope block as written; the command log |
| None | 0 to 2 | no scope block, or it was written after the actions; or no plan; or any action outside the scope (cap 2, and a gate failure) | the timestamps; the command log |

### P2 Enumeration and evidence fidelity (10)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 8 to 10 | R1 the inventory of the assigned target is complete for the scope and the time. R2 every fact names its source (command, log, packet, file) and a time. R3 limits are stated: what was not observed, and what a state such as filtered or closed does not prove. R4 no claim goes beyond the artifacts. V (10): each key fact has a second independent source, such as a scan against a socket listing, or a packet against a log | the inventory with a source column; raw output excerpts; the cross-check pair |
| Partial | 5 to 7 | the main elements are found, with one of: some facts without a source or a time; one tool and no cross-check; limits not stated; one claim beyond the artifact | the inventory; the gaps |
| None | 0 to 4 | a tool output pasted without reading; a list without sources; guesses stated as facts; large parts of the assigned surface absent | the inventory against the raw output |

### P3 Independent hypothesis and reproduction (15)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 12 to 15 | R1 at least one falsifiable hypothesis, written before the test, with its predicted observation. R2 the test was done with no hint and no outside solution. R3 the result was reproduced from a clean or reset state, or its non-reproducibility is documented. R4 a negative control fails as predicted. R5 the final claim equals what the reproduction shows. V (15): reproduced twice or by a second method, and a revised hypothesis documented wherever the first prediction failed | timestamped hypothesis records before the matching test; the hint log (empty); two runs from a clean state; the negative control record |
| Partial | 8 to 11 | a hypothesis and one observed result, but no clean-state repeat or no negative control; or the prediction was written after the test; or one claim goes beyond what was reproduced | the timestamps; the runs |
| None | 0 to 7 | no hypothesis (trial and error); no reproduction; a result taken from a published solution; an effect that cannot be reproduced, reported as a finding | the record; the hint log; the runs |

### P4 Impact and root-cause reasoning (10)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 8 to 10 | R1 the root cause is at the control or trust boundary (what made it possible), not the symptom. R2 preconditions and required privileges are stated. R3 impact is bounded by the evidence and by exploitability: a crash is not a vulnerability, a version match is not an impact. R4 at least one alternative explanation is considered, with what would disprove each. V (10): a discriminating test was run that separated the leading cause from the strongest alternative | the cause statement; the preconditions list; the alternatives table; the discriminating test record |
| Partial | 5 to 7 | the cause is named at the symptom or tool level; or preconditions are missing; or impact is asserted without a reachability argument; alternatives not considered | the cause statement |
| None | 0 to 4 | no cause; a cause contradicted by the evidence; a severity copied from a scanner label or a score; impact overstated | the report against the evidence |

## 6. Part 3: Defensive validation (25 points)

The defensive part validates a control or a detection against the offensive result. The task statement of the week says whether Part 3 validates a fix or control (type F) or a detection or telemetry source (type T). The sub-criteria and points are the same for both types. If the task statement does not say, use type F.

### D1 Correct data source and hypothesis (8)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 7 to 8 | R1 names the specific source that would show the behavior: a log channel, an event id of the right family, a sensor. R2 shows that the source is enabled and collected, with a configuration excerpt or a sample event, not an assumption. R3 states before the test the expected event, its fields and the time window. R4 does not read an absent event as an absent action. V (8): the event is confirmed at the source and again at the collector or a second place | the configuration excerpt; the expectation written before the test; the two sightings with times |
| Partial | 4 to 6 | the right source, but enablement or collection not verified; or the expected fields are vague; or the window unstated | the expectation as written |
| None | 0 to 3 | a source that is assumed or wrong, such as a native Security event id taken for a Sysmon id; a missing event taken as proof of no activity; no expectation | the expectation (or its absence); the reasoning on a missing event |

### D2 Positive validation (7)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 6 to 7 | R1 a positive case, the permitted action or the detected action as the control defines it, is run against the control. R2 its outcome was predicted before. R3 the observed outcome has timestamps that match the action. R4 the evidence is attached. V (7): the case was repeated, or confirmed at a second place | the prediction; the action time; the outcome time and identifier; the attachment |
| Partial | 4 to 5 | the positive is observed, but timestamps or identifiers do not tie to the action, or no prediction was written, or a single run | the record |
| None | 0 to 3 | no positive case; a positive claimed without an artifact; the positive not run after the change | the record |

### D3 Negative testing (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 at least one case that must not trigger, not be permitted or not be bypassed is run and recorded: a second principal denied, a benign fixture not matched, the same malicious input now failing. R2 it is a near miss, differing from the positive in one controlled variable. R3 false-positive and false-negative risks are named. V (5): a bypass attempt of the same class against the corrected control | the negative case record; the controlled variable named; the risks list |
| Partial | 3 | a negative case exists but differs trivially from the positive, or it is described and not run, or the risks are not named | the record |
| None | 0 to 2 | no negative case, or one that tests nothing the control decides | the record |

### D4 Hardening and mitigation assessment (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 the mitigation acts on the root cause named in P4. R2 side effects and regression risks are stated. R3 rollback is stated. R4 residual risk is stated. V (5): the mitigation was applied in an approved lab, or analyzed on paper where the lab is blocked and labelled as such, and the permitted function was verified again | the mitigation text linked to the cause; the regression check |
| Partial | 3 | a plausible mitigation with a weak link to the root cause, or residual risk not stated | the mitigation text |
| None | 0 to 2 | generic advice with no mechanism ("patch it", "harden the server"); a mitigation that breaks a permitted function; none | the text |

## 7. Part 4: Evidence and reporting (15 points)

### E1 Reproducibility (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 environment and versions. R2 exact commands with their status words (reviewed, executed). R3 inputs and fixture hashes. R4 expected against observed for each step. R5 cleanup. V (5): the steps were followed again from the report alone, by another person or by the learner from a clean state after writing it, and the deviations were recorded | the report; the hashes; the second-run record |
| Partial | 3 | the steps are present, but a needed element (a version, an input, cleanup) is missing, so a reviewer would have to guess | the report |
| None | 0 to 2 | a narrative with no steps; steps that cannot be followed; no target or scope reference | the report |

### E2 Precision and provenance (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 every claim carries OBSERVED, INFERRED, ASSUMED or NOT RUN. R2 every observation names its artifact, time and zone. R3 the evidence bundle is listed with SHA-256 hashes. R4 limits and unknowns are stated. R5 severity is argued from preconditions and impact. V (5): the bundle hash was written down before any key was opened, and it matches the bundle scored | the claim labels; the artifact table; the hash record and its time |
| Partial | 3 | labels present but some claims untagged, or some times without a zone, or some hashes missing | the report |
| None | 0 to 2 | no provenance; inference unlabelled; times without sources | the report |

### E3 Remediation and retest (5)

| Band | Points | Descriptor | Observable evidence |
|---|---:|---|---|
| Full | 4 to 5 | R1 a specific recommended correction. R2 a retest procedure with a positive and a negative case. R3 the observed result of the retest, or an explicit statement of why it could not be run. V (5): the retest was run and its artifacts are attached | the correction text; the retest record |
| Partial | 3 | a specific correction and a described retest that was not run, with no reason given | the report |
| None | 0 to 2 | a generic correction; no retest; "retest passed" with no artifact (which is also an integrity failure) | the report against the bundle |

## 8. Automatic failures and acceptable partial performance

The two gates come first (plan section 9.4). A failure is recorded before the points are added. A gate failure can coexist with learning credit, so the points are still scored for diagnosis, but it never counts as a passed practical assessment, whatever the total.

### 8.1 Safety gate: automatic failure

| Plan failure | Observable evidence | Nearest application gate code (PROPOSED mapping) |
|---|---|---|
| Out-of-scope target or activity | a command log line, a router log or a capture showing traffic to an address or an asset that the scope does not list | `OUT_OF_SCOPE_TARGET_OR_ACTION` |
| Use of real production credentials | a credential in the log or the bundle that is not on the synthetic list of the exercise | `OUT_OF_SCOPE_TARGET_OR_ACTION` |
| An exposed vulnerable service that is not contained or remediated | a guest adapter or listener reachable from outside the lab; a vulnerable container left published | `UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION` |
| Unauthorized public cloud provisioning | a resource in an account that is not the approved sandbox | `UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION` |
| Destruction of others' data | a deleted or altered object that the learner did not create for the exercise | `UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION` |
| Knowingly falsified evidence | an artifact that was altered or invented and the learner knew | `FABRICATED_OR_ALTERED_EVIDENCE` |

The plan lists knowingly falsified evidence under the safety gate and fabricated outputs under the integrity gate. Both are failures; a falsified artifact fails both.

### 8.2 Evidence-integrity gate: failure

| Plan failure | Observable evidence | Nearest application gate code (PROPOSED mapping) |
|---|---|---|
| Missing target or scope context | a finding or a command with no target id and no scope reference | none exact; the critical criterion scope compliance and reporting is set below its floor (level 0 or 1) |
| Fabricated commands or outputs | a command or an output in the report that is in no raw record | `FABRICATED_OR_ALTERED_EVIDENCE` |
| A test claimed passed with no observed result | "retest passed" with no artifact, or an artifact from before the change | `FABRICATED_OR_ALTERED_EVIDENCE` |
| A destructive tool run without a record | a deletion, a wipe or a reset visible in the system and absent from the log | `UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION` when it was unauthorized; otherwise the scope criterion below its floor |
| Unacknowledged use of a published solution | a step or a phrase traceable to a published write-up, with no disclosure | `FABRICATED_OR_ALTERED_EVIDENCE` (PROPOSED: the report misrepresents how the result was reached) |
| A false positive claimed as a verified finding | a finding labelled confirmed whose reproduction is missing or contradicted | `FABRICATED_OR_ALTERED_EVIDENCE` when the label is knowingly false; otherwise P4 and E2 in the none band |

How to tell an error from a failure (PROPOSED): trace each doubtful claim to a raw record in the evidence bundle. A claim that states an observation which is in no record is a failure. A transcription slip that the raw record corrects and that changes no conclusion is an error: it costs points in E2, not a gate. A reconstruction labelled "reconstructed from memory" is not a fabrication.

The application also has a gate that the plan does not: `TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION`. It is decided from the monotonic session timer of the attempt (`crates/g9-assessment/src/grading.rs::time_limit_gate`; VERIFIED by reading the code, not by running it).

### 8.3 Acceptable partial performance (plan section 9.4)

The demonstrated part is scored by the bands and a remediation is prescribed. None of these is a gate failure.

| Situation | How it is scored |
|---|---|
| A lab setup is blocked and documented diagnostics exist | P1, P2 and D1 are scored from the diagnostics; the sub-criteria that needed the lab score what was done and no more. The week is recorded BLOCKED for the missing exercises |
| A false positive is correctly identified | P2 and P4 credit the reasoning; a false positive correctly labelled is better evidence than a finding asserted |
| A challenge is not completed but the enumeration and hypotheses are accurate | P1, P2 and the hypothesis part of P3 are scored; P3 reproduction and P4 score what exists |
| A detection is partial and the missing telemetry is named | D1 credits the named gap; D2 and D3 score the cases that were run |

## 9. Baseline, midpoint and final

| | Day 1 baseline | Day 30 midpoint | Day 60 final |
|---|---|---|---|
| Purpose | placement; ungraded. It is not a claim of incompetence or of mastery | the independent offensive workflow on one unknown target | multi-disciplinary validation, not raw challenge flags |
| What is scored | the 30 diagnostic questions by domain, and a non-invasive practical baseline | the four parts of this file | the four parts of this file |
| Pass rule | none. No gate, no grade | the aim is at least 80 of 100 on the same rubric, and the safety and integrity gates pass | at least 85 of 100 as the target, and scope, isolation and evidence-integrity gates complete |
| Hints, AI, published solutions | not applicable | none | none |
| Target | no target; a non-invasive practical | one unknown, assigned authorized target. No quota of boxes | an unfamiliar authorized offensive target, plus a separate instrumented lab for the known attack-to-detection work |
| Report | none | a commercial-style report and a midpoint deficiency register | a sanitized executive and technical report, a timeline, a portfolio and an unresolved-risk register |
| If the target is not completed | not applicable | the partial evidence is graded and the blocker is documented; no exploit success is invented | the partial evidence is graded and the blocker is documented; if the finding quota cannot be supported, the actual work is scored as it stands |
| Retest | diagnostic retest at day 30 and day 60 | the diagnostic retest | the diagnostic retest, and the next-step benchmark |

How the four parts apply to the midpoint and the final (PROPOSED; the plan gives no separate point split for either):

- The four parts and their sub-criteria and points are unchanged.
- Midpoint: Part 1 is the midpoint theory exam; Part 2 scores the unknown target; Part 3 scores the telemetry inspection, mitigation and retest of the one significant issue; Part 4 scores the report.
- Final: Part 1 is the closed-book knowledge and reasoning exam; Part 2 scores the unknown offensive target; Part 3 scores the instrumented lab (detections, remediations, regressions); Part 4 scores the executive and technical report and the evidence audit.
- Unknown-target work of week 12 is produced on day 57 and the instrumented lab work on days 58 and 59 (plan section 5, week 12). Their evidence is scored in the day 60 attempt.

Where the minimum final artifacts (plan section 5, week 12) are scored (PROPOSED):

| Artifact | Scored in |
|---|---|
| rules of engagement; verified isolation | P1 and the scope gate; isolation evidence is a gate prerequisite |
| accurate inventory | P2 |
| at least two independently evidenced findings where the target supports them; one identity or privilege path | P2, P3, P4, E1 |
| at least three tested detections | D1, D2, D3 |
| one multi-artifact timeline | P2, E2 |
| at least two remediations with regression tests | D4, E3 |
| at least three agent boundary test cases across weeks 11 and 12 | D2, D3 |
| sanitized executive and technical report; unresolved-risk register | E1, E2, P4 |

## 10. Mapping the plan's score to the application's rubric

The application grades six criteria on levels 0 to 4 (0, 25, 50, 80 and 100 percent of each weight). Each criterion's level is derived from a defined set of sub-criteria of this file. The sets, the share-to-level rule, the pass rule and the worked comparison are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 10, "Mapping the plan's score to the application's rubric". The 20 points of Part 1 have no criterion in the application. In short: share = points earned on the set divided by its maximum; 100 percent is level 4, 80 percent or more is level 3, 50 percent or more is level 2, 25 percent or more is level 1, below 25 percent is level 0.

## 11. Worked scoring examples (synthetic numbers)

EXAMPLE, SYNTHETIC. The numbers below are invented to show the arithmetic. They are not a record of any attempt, any week or any learner, and they say nothing about what a real Friday contains. Part 3 is type F in all examples.

### Example A: a total above the gate, and a failed application grade

| Part | Sub-criterion | Points | Band |
|---|---|---:|---|
| 1 Knowledge | 20 questions | 14.0 | below the target of 16 |
| 2 Practical | P1 / P2 / P3 / P4 | 5 / 10 / 15 / 10 = 40 | full, with V on each |
| 3 Defensive | D1 / D2 / D3 / D4 | 3 / 6 / 4 / 4 = 17 | D1 none; below the target of 18 |
| 4 Evidence | E1 / E2 / E3 | 5 / 5 / 4 = 14 | full |
| Total | | 85.0 | |

Plan result: 85.0 is at least 80, the safety and integrity gates pass, so the plan's weekly gate is MET. Diagnostic misses: Part 1 (14 of 20) and Part 3 (17 of 25). They tell the instructor where to remediate and do not change the gate.

Application levels (sets from the instructor guide):

| Criterion (weight) | Set | Share | Level | Score of weight |
|---|---|---:|---:|---:|
| independent_execution (25) | P2 + P3 = 25 of 25 | 100 percent | 4 | 25.00 |
| root_cause_reasoning (20) | P4 = 10 of 10 | 100 percent | 4 | 20.00 |
| remediation_verification (20) | D2 + D3 + D4 + E3 = 18 of 22 | 81.8 percent | 3 | 16.00 |
| detection_visibility (15) | D1 = 3 of 8 | 37.5 percent | 1 | 3.75 |
| evidence_documentation (10) | E1 + E2 = 10 of 10 | 100 percent | 4 | 10.00 |
| scope_compliance_reporting (10, gate) | P1 = 5 of 5 | 100 percent | 4 | 10.00 |
| Total | | | | 84.75 |

Application result: the total, 84.75, is above 80, but `detection_visibility` is at level 1, which is 25 percent of its weight, below the 50 percent floor. The attempt FAILS. The two results are recorded side by side: plan gate MET; application decision FAIL, SELF_ADJUDICATED, with the reason "detection_visibility below the floor". The remediation track is the defensive part (data source and hypothesis).

### Example B: both pass

| Part | Sub-criterion | Points |
|---|---|---:|
| 1 Knowledge | concept 5.5, interpretation 5.0, control 3.5, synthesis 3.5 | 17.5 |
| 2 Practical | P1 5, P2 9, P3 13, P4 8 | 35 |
| 3 Defensive | D1 7, D2 6, D3 4, D4 4 | 21 |
| 4 Evidence | E1 4, E2 4, E3 4 | 12 |
| Total | | 85.5 |

Plan result: 85.5 is at least 80 and every part meets its diagnostic target (17.5, 35, 21, 12): MET.

| Criterion | Set and share | Level | Score |
|---|---|---:|---:|
| independent_execution (25) | P2 + P3 = 22 of 25, 88 percent | 3 | 20.00 |
| root_cause_reasoning (20) | P4 = 8 of 10, 80 percent | 3 | 16.00 |
| remediation_verification (20) | D2 + D3 + D4 + E3 = 18 of 22, 81.8 percent | 3 | 16.00 |
| detection_visibility (15) | D1 = 7 of 8, 87.5 percent | 3 | 12.00 |
| evidence_documentation (10) | E1 + E2 = 8 of 10, 80 percent | 3 | 8.00 |
| scope_compliance_reporting (10) | P1 = 5 of 5, 100 percent | 4 | 10.00 |
| Total | | | 82.00 |

Application result: 82.00 is at least 80, every criterion is at least level 2, no gate failure: PASS (SELF_ADJUDICATED in a one-operator installation). Level 3 on every criterion totals exactly 80; the only margin here comes from the level 4 on scope.

### Example C: a gate failure outweighs the points

The answers and results are those of example B, except that one logged command targeted an address that the scope did not list, and P1 is therefore at most 2: P1 = 2. Part 2 is then 32, the plan total is 82.5.

Plan result: 82.5 is at least 80, but the safety gate FAILED (out-of-scope action). The attempt does not count as a passed practical. The points remain on the scorecard for diagnosis. Application result: the gate failure `OUT_OF_SCOPE_TARGET_OR_ACTION` is recorded, `scope_compliance_reporting` has P1 = 2 of 5, 40 percent, level 1, below the floor: FAIL regardless of the total. The remediation is the safety track of the instructor guide, not a points track.

### Example D: a blocked lab

The local lab cannot be built (Stage 00) and the exercise that needed it is replaced by a hosted alternative named in the day guide. Points: Part 1 15.0; P1 4, P2 5, P3 6, P4 4 (19); D1 4, D2 0, D3 0, D4 3 (7); E1 4, E2 4, E3 2 (10). Total 51.0.

Plan result: 51.0, below 80: the weekly gate is NOT MET. No gate failure: the documented diagnostics and the honest substitution are acceptable partial performance (plan section 9.4). The week is recorded BLOCKED for the lab, the score stays as it is, and the remediation is a substitution of optional work, with no added hours. Application result: `independent_execution` is P2 + P3 = 11 of 25, 44 percent, level 1, below the floor: FAIL, with no gate failure recorded.

## 12. What to record on the weekly scorecard

The scorecard is `docs/curriculum/v2/TEMPLATES/WEEKLY_SCORECARD.md`. This file asks that each Friday record at least these fields (PROPOSED, to be matched by the template):

| Field | Content |
|---|---|
| Identity | week, day, assessment code, date, original calendar date if shifted |
| Protocol | how the unseen practical was prepared: another person, a pool with its seed, or a sealed script; who prepared it |
| Times | start, end, elapsed from the session timer, the time limit |
| Hints and aids | hint level used (0 in a scored attempt), AI used (no), keys opened before submission (none) |
| Hashes | the locked knowledge answers, the evidence bundle, the fixture map commitment, each with its time |
| Gate check | safety gate: pass or fail with the evidence; integrity gate: pass or fail with the evidence |
| Points | the 20 questions by type; P1 to P4; D1 to D4; E1 to E3; the four part totals; the total of 100 |
| Targets | each part against its diagnostic target; the plan gate: MET or NOT MET |
| Week conditions | each condition of the week's gate line: met or not met |
| Application | the six levels, the weighted total, gate failures recorded, the decision, the reliability class |
| Confidence and notes | the learner's confidence before the score was known, failures, environment notes |
| Remediation | tracks assigned, the optional work substituted, the retest date |

## 13. Departures from the plan and open points

| Item | What this file does | Why |
|---|---|---|
| Half points in Part 1 | a constructed-response question may score 1, 0.5 or 0; the maximum stays 20 | the plan says 20 questions of 1 point each; a partly correct answer needs a score that does not hide the gap. PROPOSED |
| Bands (80 percent and 50 percent) | defined here | the plan lists sub-criteria and points but no bands; the thresholds equal the application's anchored levels |
| The maximum of a band needs a verification item | defined here | the plan asks for dual evidence whenever feasible (section 10.4); it gives no scoring effect |
| Part 3 types F and T | the sub-criteria are the same; the type only decides how the application's levels are derived | the plan's Part 3 serves both a fix and a detection |
| Point split for midpoint and final | the four parts are kept | the plan gives no separate split. PROPOSED |
| Unknown target of week 12 produced on day 57, scored on day 60 | evidence from days 57 to 59 is scored in the day 60 attempt | the plan schedules the unknown target on day 57 and scores the final on day 60 |
| Plan failures with no exact application gate | mapped by the table in section 8, or the scope criterion is set below its floor | the application enumerates four gates; the plan names more conditions |
| Application gate `TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION` | recorded and honored | not in the plan; the application's rule is stricter and is not changed (ADR-0012, ADR-0023) |
| Scoring time | the plan allots 60 minutes to scoring and the report on a Friday | the sum of key-based marking, bands and the application's grading may not fit; the overflow is recorded and not credited as study. Open point |
| Reliability of a one-person grade | the application records SELF_ADJUDICATED when the reviewer is the learner, and such a decision never validates a competency (ADR-0012) | see the instructor guide for how a second reviewer is recorded |
