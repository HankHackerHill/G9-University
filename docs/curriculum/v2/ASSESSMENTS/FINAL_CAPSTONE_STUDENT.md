# G9 Cybersecurity Mastery v2.0: final capstone, student file (week 12, days 56 to 60)

- Reader: the owner, as the learner. Scoring, key release and the preparation of an unseen practical are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`. The marking bands are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`. The portfolio specification is `docs/curriculum/v2/PORTFOLIO/README.md`. The instructor side of this capstone exists as `docs/curriculum/v2/ASSESSMENTS/FINAL_CAPSTONE_INSTRUCTOR.md`; this file does not describe its contents.
- Source of requirements: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 0.3 (final day 60), 1.3 (85 of 100, retest), 5 week 12 (the five days, the two-environment fairness rule, the minimum final artifacts, the gate, the expansion points), 9.1 to 9.4, 11 and 13; `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md`.
- Status: PROPOSED, written 2026-11-18. No capstone environment exists, nothing was run, scanned, installed or tested, no pool is fixed and no attempt is recorded. Every command in this file is proposed text for the owner's review. Text marked `Sample (synthetic):` is invented to show a format and records no observation.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN, ESTIMATE (a planning figure that was not measured).
- This file holds no scoring key, no fixture map, no list of what is seeded in the known lab, and no description of any target's weakness.
- Precedence: if a week 12 file (the `ASSESSMENT.md`, the labs, the day guides of days 56 to 60) is looser than a rule of this file, the stricter text applies and the difference is recorded as a defect.

## 1. What the capstone is

| Item | Value |
|---|---|
| Days | 56 to 60, Monday 2027-02-01 to Friday 2027-02-05 on the original calendar |
| Hours | 40: 8 AI (day 56) and 32 combined capstone (days 57 to 60) |
| Assessment | one definition, `V2-W12-CAPSTONE`, kind CAPSTONE. One attempt, begun in the window before the knowledge exam on day 60 and submitted with the report (ADR-0023, decision 2). The evidence of days 56 to 59 is scored in that attempt (`docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`, section 9) |
| Output of the week | `W12-Final-Executive-and-Technical-Capstone.md` (the plan's name) |
| What is scored | the four parts of the Friday rubric, 100 points: closed-book knowledge and reasoning 20, independent practical 40, defensive validation 25, evidence and reporting 15 |
| Gate | at least 85 of 100, with the safety gate and the evidence-integrity gate passed and the scope and isolation gates complete (section 9). No invented exploitation outcome and no invented telemetry (plan, week 12 gate) |
| Emphasis | multi-disciplinary validation over raw challenge flags (plan 9.2) |
| Aids | no hints, no AI, no published solution, from the first scored work on day 56 until the attempt is submitted (section 5) |

The capstone is not a certification, a proof of professional status or a prediction of an exam result (section 11).

## 2. State on 2026-11-18

| Needed | State | Consequence |
|---|---|---|
| Stage 00 resolved and the lab built | unresolved. No guest, network, router, domain or SIEM exists (`docs/curriculum/v2/README.md`) | the known lab (section 3) does not exist. While that holds, days 58 and 59 cannot be done as written and are recorded BLOCKED (section 13) |
| The full stack for days 58 and 59: `G9-RTR-01`, `G9-ATK-01`, `G9-TGT-01`, `G9-DC-01`, `G9-WKS-01`, `G9-SIEM-01` (16 vCPU, 31 GiB, 390 GiB configured) | not built, not measured | the stack uses 100 percent of the 31 GiB budget (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 13). PROPOSED heuristic: at least 31 plus 8 GiB of host memory available at the start of a session, and no other heavy workload |
| The isolation tests T1 to T9 current for that stack and policy version | NOT RUN | no intrusive local work without them (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11) |
| A pool of unknown targets for day 57, fixed before the week | not fixed. `docs/curriculum/v2/WEEKS/W12/RESOURCES.json` names index pages only (labs index, beginner path, web labs, free rooms, blue-team challenges) and records no individual candidate as verified | section 3.2 says how to build it. Retired Hack The Box content is probably a paid tier (INFERRED from the week 6 and week 8 registries; ASSUMED here, see `docs/curriculum/v2/RESOURCE_REGISTRY.md`, generated later) |
| The toy agent, its broker and mock tools from week 11 | not built | day 56 needs them. The path by which a model's text reaches the broker is open decision O-05 of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`; the default is a recorded transcript |
| Outputs and records of weeks 1 to 11 | none exist | the portfolio is built from them; absent items are stated as absent |
| Written scope, rules of engagement and approvals for every exercise | none exist | an exercise without them is not available (class E to I of SAFETY_AUTHORIZATION) |

A caution about the plan file. `docs/curriculum/v2/MASTER_PLAN_V2.0.md` is the owner's source document, not a learner file. Its section 8 holds the reference answers of the diagnostic and of the weekly seed questions, including week 12. Do not read section 8 before the day 60 exam is locked. The separation of instructor files in this repository prevents accidental exposure only; the plan file is outside it (open point OP-FC-02).

## 3. The two-environment fairness rule

The plan's rule (section 5, week 12): use an unknown authorized challenge for independent offensive work and a separate instrumented local lab for known attack-to-detection correlation, and do not pretend that an environment you yourself seeded is truly unseen.

### 3.1 The two environments

| | U: the unknown authorized target | K: the instrumented local lab |
|---|---|---|
| What it is | one platform-designated retired or beginner exercise, drawn from a pool fixed before the week, or chosen by another person | the G9 lab: `G9-RTR-01`, `G9-ATK-01`, `G9-TGT-01`, `G9-DC-01`, `G9-WKS-01`, `G9-SIEM-01` on `10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test` |
| Known to you | no. Its faults are unknown to you | yes, by design. You know the inventory, you run the replays, and what is seeded is held on the instructor side by a map whose hash was committed before the week |
| Days | day 57 (blocks B and C, 270 minutes of window), and the reproduction of day 60 | days 58 and 59, and the reproduction of day 60. Day 56 uses a third environment, the toy agent and its broker (target ids `AGENT:<toy-id>`) |
| Used for | independent offensive work: enumeration, falsifiable hypotheses, validated findings, impact | known attack-to-detection work: replays of benign attack-like scenarios, telemetry, three tested detections, a multi-artifact timeline, remediations and regressions |
| Counts as unseen evidence | yes | never |
| Scope class | F (hosted), under the platform's terms read and dated | E (local), with isolation tests current |
| Instructor-side map | none that you may have: it is a third party's target | the fixture map, hash committed before week 12 |

### 3.2 What follows from the rule

1. A finding made in K is labelled `known-lab` in every record and report. It never counts toward the quota of independently evidenced findings (the plan's "at least two findings where the target supports them"). Those findings come from U.
2. The one identity or privilege path of the minimum artifacts comes from U where U supports one. Otherwise it comes from K, labelled `known-lab`, and the record says which. It then shows path analysis and testing skill, not discovery on an unseen target.
3. Do not shape your work on U with what you know about K, or the reverse. Do not recreate any exercise of the platform in K, and do not read the description of a candidate for U beyond the eligibility checks.
4. The pool for U: fixed and hashed by Friday 2027-01-29 (day 55); at least 5 eligible entries; the eligibility checks E-01 to E-10 of `docs/curriculum/v2/ASSESSMENTS/MIDPOINT_OFFENSIVE_BENCHMARK.md`, section 5.3, apply unchanged; no entry may be an exercise you have started in any earlier day, week or the midpoint; the entry you drew at the midpoint is consumed. Protocols 1 and 2 only for U. A guided room whose task questions act as hints is ineligible.
5. K may use protocol 1 (another person prepares the scenario and holds the map) or protocol 3 (a sealed script). K is known by design and is never presented as an unseen exam (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 8.4).
6. What you are told about K: the inventory, the isolation test records, and scenario cards. A card gives the action, the guest, the account and the cleanup of one benign attack-like scenario. It gives no expected telemetry, no event id and no expected alert: predicting where an action shows up, and checking it, is the work.
7. Catalogue of scenario classes that a replay set may draw from. A catalogue names classes, not instances. This instance's selection and parameters are instructor-held.

| Class | Plain description |
|---|---|
| authentication | logon successes and failures for synthetic accounts, interactive and over the network |
| process creation | a harmless process started by a harmless parent, with a visible command line |
| account and group change | creation or change of a synthetic account or group membership |
| scheduled work | creation or change of a scheduled task that runs a no-op |
| service change | creation or change of a service that runs a no-op |
| file activity | creation of a marker file in a scratch directory |
| name resolution | a lookup of a name under `corp.g9.test` |
| network connection | a connection from one lab guest to a permitted port of another |
| collection health | whether a source that you expect is configured, running and collected |

## 4. The five days

### 4.1 The plan's rows

| Day and date | A, 08:30 to 10:30 | B, 10:45 to 12:45 | C, 13:45 to 16:15 | D, 16:15 to 17:15 |
|---|---|---|---|---|
| 56 Mon 2027-02-01 | Finalize RoE, synthetic agent threat model and success checks | Verify approved agent tool/test harness, no privileged access | Execute fresh AI policy tests, including malicious low-trust document and allowed baseline | Record actual tool authorization and readiness gate; 8 AI hours |
| 57 Tue 2027-02-02 | Define unknown-target assessment scope and reporting standard | Independently enumerate designated unseen target | Validate hypotheses, explain attack preconditions and impact | Preserve observed request/host evidence and avoid spoilers |
| 58 Wed 2027-02-03 | Detection strategy, incident reconstruction objectives | Replay known attack-like benign scenarios in instrumented G9 lab | Investigate host/network events and create three detections | Positive/negative detection tests and timeline |
| 59 Thu 2027-02-04 | Prioritize confirmed risks and business impact | Implement approved lab remediations | Repeat permitted offensive and positive/negative regression cases | Assess residual risks and unintended functional regressions |
| 60 Fri 2027-02-05 | Final closed-book knowledge and reasoning examination | Executive/technical report assembly | Independently reproduce recorded high-value results | Final evidence audit, score 85/100 target, portfolio and next-step benchmark |

Day 56 is the AI day, and the plan counts it as 8 AI hours. Together with the 16 AI hours of week 11 the course holds 24 AI hours. Times are America/Chicago (UTC-6 in February; ASSUMED standard-time offset, read it from the clock you record on). The clock of every day is the plan's: R 08:00 to 08:30; A, B, C, D as in the table; breaks 10:30 to 10:45 and 12:45 to 13:45 are not study.

### 4.2 What each day means here

| Day | Environment, scope class | Work in the blocks | Hand-in at the end of block D | Notes |
|---|---|---|---|---|
| 56 | AGENT, class H (agent RoE) | A: the agent RoE, a synthetic threat model (assets, trust boundaries, the lower-trust channels) and the success checks written before any test. B: confirm that the harness runs with no production privilege, no network, mock tools over a scratch store and a broker that is independent of the model (AI-01 to AI-06 of SAFETY_AUTHORIZATION); record the policy file hash. C: fresh tests, meaning that each differs from every week 11 test in at least one variable (the document, a tool argument or the role) and the record says which: one malicious low-trust document, one allowed baseline, and the kinds relevant to the harness (`docs/curriculum/v2/TEMPLATES/AI_AGENT_EVALUATION.md`). D: the readiness record | one `AI_AGENT_EVALUATION` record per test; the readiness record: the checks of AI-01 to AI-14 with their evidence, the number of tests and their verdicts, and any stop condition. READY means these tests passed in this harness. It does not mean the agent is secure | the toy agent under test is the subject, not an aid: it has no access to your scope, evidence or report. If the model path is a recorded transcript (the default of O-05), the model-behavior axis is NOT RUN and the tests are scripted proposals |
| 57 | U, class F | A: the generic scope block, the RoE, the reporting standard (the technical report Part A structure, one finding record per root cause, the severity method stated once). B: handover at 10:45 (draw and record as in section 5.4 of the midpoint file), the target-specific scope step, then independent enumeration. C: validation of hypotheses, preconditions, impact, negative controls. D: preserve raw request and host evidence, hash it, close the instance, and write a skill record of the target without any step, flag, credential found or screenshot | the U evidence bundle; finding records; the draft attack path; the cleanup record | window: 10:45 to 12:45 and 13:45 to 16:15; no contact in the lunch break; last contact 16:10. A hosted target's page is not read beyond the eligibility checks. No spoiler leaves the private folder |
| 58 | K, class E | A: detection strategy and incident-reconstruction objectives, written before the replay: which behaviors, which questions, what you expect to see and where. B: baseline taken, isolation tests confirmed current, replay of the scenario cards, with each action's start and end time in UTC and local time. C: investigate the host and network events and write three detections from what you actually find. D: positive and negative tests of each detection, and the multi-artifact timeline | three `DETECTION_RULE_TEST` records (`docs/curriculum/v2/TEMPLATES/DETECTION_RULE_TEST.md`); the timeline records (`docs/curriculum/v2/TEMPLATES/DFIR_TIMELINE.md`) | a time-boxed exception (RED to CORP or CORP to DMZ) is requested only if a card needs it, with its removal step and retest (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 4.3) |
| 59 | K (and paper for U), class E | A: prioritize the confirmed risks with the severity method and an ASSUMED business context of one sentence. B: apply the approved remediations to K under change approval records (`docs/curriculum/v2/TEMPLATES/CHANGE_APPROVAL.md`); a remediation of a U finding is a paper recommendation only. C: repeat the permitted offensive and the positive and negative regression cases against the changed state, with a bypass attempt of the same class. D: residual risks and any function that regressed | at least two remediations with before and after states and regression results; the residual-risk register | a fix is PROPOSED until its retest artifact exists. Restore K to the recorded baseline when the day ends and record how you know |
| 60 | all, class E and F for the reproductions | R: recall. About 08:25: the attempt is begun in the window. A: Part 1, the final knowledge exam (section 8). B: assemble the executive and technical reports. C: reproduce the recorded high-value results from the report alone, from a clean state: U under a new scope line and a fresh instance, K from the baseline. D: the final evidence audit, scoring, the portfolio and the next-step benchmark; the attempt is submitted | the final reports; the reproduction records; the portfolio package; the scorecard | the keys are not opened until the attempt is submitted and the hashes are recorded |

### 4.3 The hash chain

At the end of block D of each of days 56 to 59, hash the day's evidence manifest (command 3 of section 12) and write the hash and the time on your private scorecard. Include the previous day's manifest hash inside the next day's manifest, so that an edit to an earlier day shows. On day 60 the final bundle hash is recorded before any key is opened (INSTRUCTOR_GUIDE, section 6). PROPOSED: the plan does not describe a chain.

## 5. Rules

Every rule applies from the first scored work of day 56 to the submission of the attempt on day 60. A gate failure means the capstone never counts as passed, whatever the total (plan 9.4).

| ID | Rule | Basis | If broken |
|---|---|---|---|
| C-01 | Scope and authorization first. Every exercise has a signed RoE record and a scope line with target ids, sources and window before its first action (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, sections 2.4 and 12). Class E requires the isolation tests current for the stack and policy version | plan 0.4, 9.4 | out-of-scope action: safety gate failure |
| C-02 | Only owned, intentionally vulnerable, platform-designated or explicitly authorized targets. Synthetic credentials and data only. The lab's addresses are the plan's | plan 0.4 | safety gate failure |
| C-03 | No AI as an aid. No chatbot, completion service, AI search summary or agent that helps you, on any device, for any work that feeds the capstone score: days 56 to 60. The course's allowance of an AI critic after an unassisted attempt, outside exam conditions, is suspended for week 12. The toy assistant of day 56 is the object under test and is not an aid | plan 0.3, 9.1, 9.2; N-15 | disclosed: the affected part loses its independence bands. Undisclosed: evidence-integrity failure |
| C-04 | No hints and no published solution for U. The hint ladder does not exist in the scored work. Read no solution of the drawn target, search no write-up, and do not search its name until you have submitted. Declare any exposure (aids log); the target then no longer counts as independent evidence | plan 9.4; week 12 | undeclared: evidence-integrity failure. Declared: the findings from U lose independence |
| C-05 | No fabrication. No invented command, output, finding, detection, telemetry, benchmark or approval. A step not run is NOT RUN. A test is passed only with its artifact | plan 0.4, 9.4 | evidence-integrity failure |
| C-06 | Label every claim OBSERVED, INFERRED, ASSUMED, UNVERIFIED or NOT RUN, with an artifact, a time and a zone for each OBSERVED claim | plan 0.4; templates | points in E2; a false OBSERVED is an integrity failure |
| C-07 | K replays are benign: the scenario cards only, on lab guests, with synthetic accounts, no real malware, no exploit code for any third-party product, no stress test | N-08, N-17; FIXTURES, section 4.3 | safety gate failure when contained badly |
| C-08 | Every temporary route, rule, listener or exposed service has a removal step and a retest. One left in place at the end of the day is a stop condition and a gate failure if left to the end | N-14; STOP-11 | safety gate failure |
| C-09 | Agent lab limits AI-01 to AI-14 of SAFETY_AUTHORIZATION hold: no shell, no network, no real file path, no real credential; the broker decides, never the model; four things logged separately | N-12; section 7 of SAFETY_AUTHORIZATION | a side effect outside the scratch store is STOP-14 and a safety gate failure |
| C-10 | A hosted target is private. No flag, credential found, step order or screenshot of it enters the portfolio, a shared place or an assistant. The portfolio holds a skill record | SAFETY_AUTHORIZATION, section 8.1; PORTFOLIO, sections 5 and 6 | portfolio defect; publication of an active solution is never allowed |
| C-11 | Time. The windows of the days are the plan's blocks. Work after a limit is an addendum and is not scored. A window that ends ends the work (STOP-07) | GRADING_RUBRICS, section 3 rule 11 | out-of-window contact is out of scope |
| C-12 | A stop condition ends the exercise. STOP-01 to STOP-16 apply. Resuming needs a fresh approval | SAFETY_AUTHORIZATION, section 9 | depends on the cause |
| C-13 | If the finding quota cannot be supported, submit the actual work and let it be scored as it stands (section 7). Never fill a quota | plan, week 12 gate | filling it is a gate failure |

## 6. Minimum final artifacts: the checklist

The plan lists eleven minimum artifacts (plan section 5, week 12). Tick an item only when its evidence file exists and is in the evidence index with a SHA-256. The portfolio specification (`docs/curriculum/v2/PORTFOLIO/README.md`, section 2) carries the same list with the acceptance checks of a reviewer; the quota status table of that file is filled from this checklist.

**1. Rules of engagement** (environments: all)

- [ ] a signed RoE record for each of the exercises on days 56 to 60: the agent lab, U, K, and each reproduction
- [ ] a scope line for each, with target ids, sources, window with zone, allowed technique classes, exclusions and stop codes, timestamped before the first action
- [ ] the platform terms and the date they were read, for U
- [ ] a closure record for each: actual start and end, deviations, stop condition fired or none, cleanup verified

**2. Verified isolation** (K)

- [ ] isolation test records T1 to T9 for the stack, each with run id, stack and baseline ids, policy version, commands as run, captures with hashes, and a result PASS, FAIL, INCONCLUSIVE or NOT RUN
- [ ] the retest records after every exception removed on days 58 and 59
- [ ] if Stage 00 stayed blocked: the blocker record and the substitution record. The word is BLOCKER. It is never written as verified

**3. Accurate inventory** (U and K)

- [ ] for K: guest ids, versions, addresses, baseline ids and services as running, agreeing with the NIC inventory of test T8, with every discrepancy listed
- [ ] for U: each fact of the target with its source, time and zone; states such as filtered reported as such; what the inventory does not cover
- [ ] a second independent source for each key fact

**4. At least two independently evidenced findings where the target supports them** (U only)

- [ ] one finding record per root cause (`docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`), with scope and authorization reference, preconditions, root cause, impact bounded by the evidence, and a stated status
- [ ] reproduced from a clean state by the recorded steps, with a negative control that fails as predicted, and reproduced again on day 60
- [ ] minimal evidence with times and hashes; no flag value
- [ ] a recommended correction, and the retest procedure with a positive and a negative case
- [ ] a statement of what the evidence shows about what the target supports, if fewer than two are claimed (section 7)

**5. One identity or privilege path** (U where supported, otherwise K labelled known-lab)

- [ ] an attack path record (`docs/curriculum/v2/TEMPLATES/ATTACK_PATH_GRAPH.md`) with nodes, edges and paths registers, each edge labelled OBSERVED, INFERRED, ASSUMED or NOT RUN, and each path classified DEMONSTRATED, MODELLED or REFUTED
- [ ] the difference between reachability in the graph and achieved access stated for each path
- [ ] the events that should cover the path and whether they were seen, at the source and at the collector

**6. At least three tested detections** (K)

- [ ] per detection a record (`docs/curriculum/v2/TEMPLATES/DETECTION_RULE_TEST.md`) with the expected event written before the test
- [ ] the source configuration read from the host, not assumed; the event family right (a native Security event is not a Sysmon event)
- [ ] the event seen at the host and at the collector, each with a time
- [ ] a positive fixture with its predicted and observed outcome, repeated once
- [ ] at least two benign negatives: a near miss that differs in one controlled variable, and legitimate activity with a similar event; and the disabled-source case
- [ ] counts as counts on named fixtures; the retune log; an ATT&CK mapping only from observed behavior, or "not applicable"

**7. One multi-artifact timeline** (K)

- [ ] records (`docs/curriculum/v2/TEMPLATES/DFIR_TIMELINE.md`) built from at least three artifacts of independent provenance, such as a host event log, a SIEM record and a router or packet record
- [ ] per artifact: provenance, original hash and algorithm, hash verified before analysis and at the end, and the acquisition limits
- [ ] per claim: original time with zone, normalized UTC with the offset rule, confidence, corroborating and contradicting records, the alternative explanation, and the next discriminator
- [ ] observed and inferred kept apart; clock offsets between guests measured, not assumed

**8. At least two remediations with regressions** (K; U on paper)

- [ ] a change approval record for each remediation, with before and after states
- [ ] a positive regression (the permitted function still works) and a negative regression (the forbidden behavior fails), each with an artifact dated after the change
- [ ] a bypass attempt of the same class against the corrected control
- [ ] a check that no function the user needs regressed, and the rollback statement

**9. At least three agent boundary test cases across weeks 11 and 12**

- [ ] an evaluation record (`docs/curriculum/v2/TEMPLATES/AI_AGENT_EVALUATION.md`) per case, with the legitimate goal, the policy and its hash, the untrusted input, the model's proposal, the broker's decision, the downstream effect, the audit trace, and the three tallies kept apart
- [ ] across the cases, at least two normal requests that succeeded and at least two unauthorized proposals that the broker denied (AI-11), the unauthorized ones replayed as scripted proposals where the model made none
- [ ] for every denied call, the scratch-store digest before and after, equal
- [ ] the day 56 cases are fresh (section 4.2), and the count of week 11 cases cited is stated

**10. A sanitized executive report and technical report** (all)

- [ ] the executive report (`docs/curriculum/v2/TEMPLATES/EXECUTIVE_REPORT.md`) and the technical report with Parts A, B and C (`docs/curriculum/v2/TEMPLATES/TECHNICAL_REPORT.md`): scope, methodology, asset inventory, severity rationale, preconditions, impact, reproduction, evidence, recommended fix, retest and limitations; the incident report with provenance, timeline and observed against inferred
- [ ] the findings table of the executive report equal to the technical report, row for row
- [ ] the evidence manifest and the command register as appendices
- [ ] sanitized under PORTFOLIO, section 5, with the sanitization record

**11. An unresolved-risk register**

- [ ] each open risk with statement, why unresolved, the evidence gap, who decides, the next discriminating test and a review date. An empty register is suspect: say why

Quota status: fill the table of PORTFOLIO, section 2, with real counts and a status of MET, PARTIAL, NOT MET or NOT APPLICABLE with the reason.

## 7. When the finding quota cannot be supported

The plan: if the finding quota cannot be supported, submit the actual work and score accordingly instead of fabricating findings (plan, week 12 gate; 9.2: if a real unknown challenge is not completed, grade partial evidence and document the blocker).

| Situation | What you submit | How it is scored | What you must not do |
|---|---|---|---|
| U supports fewer than two findings that you could evidence in the window | the findings you have, each as it stands; each lead as Unconfirmed with what would confirm it; the enumeration, hypotheses and refutations; a statement of what the evidence shows about what the target supports | the demonstrated part: P1 and P2, the hypothesis part of P3, reproduction and root cause where they exist; P3 and P4 lose what is missing. The gate is not failed by an honest shortfall | state that the target "does not support" findings without the enumeration and hypotheses that show it; the examiner decides this from the evidence, not from your assertion |
| U is not solved | as in section 9 of the midpoint file: the partial result, the blocker in one of three kinds, the next tests as NOT RUN | as above | read a solution after the window and write it up as your work |
| U cannot be reached (platform fault, tier, no pool) | the diagnostics, times, and the blocker; the Part 2 work that does not need U: the inventory and analysis of K | recorded BLOCKED for U; scored as demonstrated | substitute a guided room as an "unseen" target |
| K does not exist (Stage 00) | the blocker record; detections written and tested on stored synthetic event fixtures, labelled replay; remediations and the agent tests that do not need K | D1 is capped because the source cannot be shown enabled and collected on a host that does not exist; replay tests the rule and the parser, not the host's production of the event. Recorded BLOCKED for the isolation and the replays | write "verified isolation" or a collected event that was not collected |
| fewer than three detections tested | the number tested, each with status TESTED, FAILED or PROPOSED as it is | scored on what was tested | call a rule with only a positive result tested |
| fewer than three agent cases | the cases run, with counts | as above | cite a week 11 test as fresh |
| a finding from K offered to make the count | nothing | not counted: it is `known-lab` | relabel it |
| one weakness split into two findings | one finding with its assets | one root cause is one finding (PENTEST_FINDING) | count it twice |

A shortfall costs points. It is a gate failure only if something was invented.

## 8. The final knowledge examination

| Item | Rule |
|---|---|
| What | Part 1 of `V2-W12-CAPSTONE`: 20 questions of 1 point each, with the mix of the plan (at least 6 concept, 6 interpretation of a `Sample (synthetic):` log, packet, code or output, 4 applied control, 4 synthesis) and at least 8 multiple-choice items. The plan calls it a knowledge and reasoning examination: the interpretation and synthesis items carry the reasoning. It draws on the whole course and on the competency checkpoints of week 12 |
| Where | the week 12 `docs/curriculum/v2/WEEKS/W12/ASSESSMENT.md`. Do not open it before the attempt is begun. Its keys are in instructor files and are not opened before the attempt is submitted |
| When | block A of day 60, 08:30 to 10:30, in one sitting after the attempt is begun about 08:25 |
| How | closed book: no notes, no reference, no AI, no web, no help, no tool beyond a text editor for your answers. One sitting; no key. Constructed responses are marked 1, 0.5 or 0 against the key's marking points; multiple-choice 1 or 0 (GRADING_RUBRICS, section 4) |
| Locking | at the end of the exam write the answers to one file, compute its SHA-256 (command 2 of section 12) and write the hash and the time. A later edit is an addendum and is not scored |
| Target | at least 16 of 20 (diagnostic target; it never replaces the gate) |
| Not on the exam | a question about a specific third-party machine or its solution |
| The diagnostic retest | the plan retests the entry diagnostic on day 60 and allots no block. `docs/curriculum/v2/ASSESSMENTS/ENTRY_DIAGNOSTIC.md` estimates about 100 minutes for Part 1 (E01 to E30) and about 75 for the paper practical baseline of Part 3. PROPOSED: sit Part 1 as a separate closed-book sitting on day 60 outside the counted hours, or in block B if the reports are already assembled, and record which; Part 3 only outside the counted hours, otherwise the placement is provisional. Fill the same sheet, score it against `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/ENTRY_DIAGNOSTIC_KEY.md` after the attempt is submitted, and note that no parallel form exists (open point O-D-02 of that key). It is not part of the 100 points. The same cautions as in section 10 of the midpoint file apply: the day 1 key was open to you, and the plan file holds the diagnostic's expected answers |

## 9. The 85 of 100 gate

The final passes in the plan's sense only if every condition holds. The numeric score is never offset against a gate (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 11).

| # | Condition | Evidence |
|---|---|---|
| G-1 | Safety gate passed: no out-of-scope target or action, no real production credential, no exposed vulnerable service left uncontained, no unauthorized cloud provisioning, no destruction of others' data, no knowingly falsified evidence | command logs, router and capture records, the scope lines, the closure records |
| G-2 | Evidence-integrity gate passed: no missing target or scope context, no fabricated command or output, no test claimed passed without an observed result, no destructive step without a record, no undeclared published solution, no false positive claimed as verified | every OBSERVED claim traced to a raw record in the bundle hashed before any key opened |
| G-3 | Scope complete: every exercise had its signed RoE and scope line before its first action | the records of artifact 1 |
| G-4 | Isolation complete: for each K exercise, the isolation tests current for that stack and policy version. Where K was blocked, the work is recorded BLOCKED and the capstone cannot be passed | the records of artifact 2 |
| G-5 | No invented exploitation outcome and no invented telemetry | G-2, and the examiner's checks |
| G-6 | Total of at least 85 of 100 | the scorecard |

The parts and their diagnostic targets are those of the Friday rubric:

| Part | Points | Diagnostic target | Where it comes from in the capstone |
|---|---:|---:|---|
| 1 Knowledge and reasoning | 20 | at least 16 | the day 60 exam |
| 2 Independent practical | 40 | at least 30 | U: scope, inventory, hypotheses, reproduction, root cause and impact |
| 3 Defensive validation | 25 | at least 18 | K: three detections, remediations and regressions, agent boundary tests |
| 4 Evidence and reporting | 15 | at least 12 | the reports, the manifest, the reproduction from the report alone |

The part targets are diagnostic. They tell you where to remediate and do not fail the capstone by themselves. For the application's derivation of its six levels, Part 3 is proposed as type T, because three of its artifacts are detections (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 10.2); the task statement of the week records the type. The application grades the same attempt under its own stricter rule (the weighted total of at least 80, each of six criteria at level 2 or more, no gate failure), and a one-person grade is recorded as self-adjudicated. The two records are reported side by side and never averaged.

A missed gate changes nothing retroactively. The result stays as scored; the failing parts are remediated with optional work and the next benchmark of section 10 (INSTRUCTOR_GUIDE, section 9).

## 10. The portfolio package and the next-step benchmark

The portfolio is assembled outside the repository from your private records and exported only after sanitization (`docs/curriculum/v2/PORTFOLIO/README.md`). Its first file states the four course states with their true values: authoring complete, lab tested, learner passed, certification ready. A package built while Stage 00 is blocked says LAB TESTED: NO.

| Step | When | Note |
|---|---|---|
| assemble the package structure and the quota status table | day 60, block D | from the checklist of section 6 |
| sanitization: classify, redact in copies, search by pattern, read as a stranger, second review on another day, hash, record | starts on day 60, block D | the pattern searches of PORTFOLIO, section 5.4, are proposed text and none is approved; the second review and the hash may overflow outside the counted hours, and the overflow is recorded and not credited as study |
| third-party exercises as skill records | day 60 | no walkthrough, active or retired |
| statement of limits | day 60 | the text of PORTFOLIO, section 7, unchanged |

The next-step benchmark (plan day 60, block D) has no definition in the plan. PROPOSED reading: one external, measurable target for the period after week 12, chosen from your final deficiencies and stated with its evidence criteria and a date. Examples of the shape, not choices: the midpoint protocol repeated on a fresh unseen target from a new pool; or a per-objective comparison of your artifacts against one named certification's live objectives (section 11). It is a plan, not a result, and it carries no promise.

## 11. Certification readiness

Plan section 13: the primary preparation direction is CPTS-aligned offensive methodology, with OSCP-style manual practice and a defensive SOC and DFIR foundation. The choice of a certification follows the measured baseline, midpoint and final results, not marketing. The course does not promise a passing result, professional expert status, or a certificate.

| Certification | Fit (plan 13) | Qualification | State of the fact here |
|---|---|---|---|
| HTB CPTS | strong fit for enterprise testing, AD and reporting | the role path must be completed in full before the exam; the path's published estimate is 342 hours, separate from this program | ASSUMED. The week 6 registry marks the 100 percent rule CONFIRMED by a search result and the week 12 registry marks it NOT_CONFIRMED; the registries disagree, so read the live page. Pointer: `docs/curriculum/v2/RESOURCE_REGISTRY.md` |
| OffSec OSCP+ | strong offensive external benchmark | read the live exam guide; exams on or after 2026-12-01 use a scaled score rather than a fixed 70 raw points; the plan states that AI chatbot use and automation restrictions apply | the scaling date is CONFIRMED by search results in both registries. The AI restriction is NOT_CONFIRMED in both: it stays ASSUMED |
| HTB CDSA | a later blue-team complement | the SOC Analyst path estimate is 189 hours; complete its prerequisites first | the week 12 registry records the certification page title as not seen (UNVERIFIED) |
| BTL1 or a web-focused credential | alternative specialization | check objectives, access, cost, prerequisites and exam rules separately | the week 12 registry records only an unverified address |

What follows:

1. The two role paths total about 531 hours at the published averages (342 plus 189), which exceeds 480 before any assessment or independent practice. The course does not contain them. The figures are platform averages, not quotas and not predictions for you.
2. A score of 85 on this course is directional evidence of skill on this course's tasks. It does not meet any certification's prerequisites, and exam conditions differ: time, proctoring, report rules, and the limits on aids.
3. A readiness claim for one named certification needs a per-objective comparison, made by you from that body's live syllabus read on a recorded date, of each objective against your artifacts, with the independence of each artifact stated (unseen, known-lab, guided). Unmatched objectives are gaps.
4. Read the live exam rules before any booking. This course's allowance of an AI critic ends at the exam door.

Sample (synthetic): two rows of a per-objective comparison, invented to show the shape.

```text
objective (copied from the live syllabus, read <date>) | artifact id and hash | independence | strength | gap
<objective 1>                                           | FIND-U-01 <hash>     | unseen (U)   | reproduced twice | none stated
<objective 2>                                           | none                 | not covered  | none             | GAP: no artifact; not a course topic
```

## 12. Commands proposed for review

Proposed text only. None was run. Each needs your review and your approval of the exact text first (STUDENT_GUIDE, section 4). Lab commands of days 56 to 60 are in the week 12 day guides and labs and are not repeated here.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `date -u +%Y-%m-%dT%H:%M:%SZ` | the clock of the machine on which you record; record each guest's clock separately | write a UTC time beside a hash, a draw, a handover, a replay start and end | one line: year-month-day, T, hours:minutes:seconds, Z; a reading of that clock, not a measure of its accuracy | none | ordinary user | the machine you record on | none needed (read-only) |
| 2 | `sha256sum <file> [<file> ...]` | the files you name: the pool file, the locked answers, a report, a fixture-map commitment | record a hash before a key is opened or a commitment made | one line per file: 64 hexadecimal digits, then the name | none | ordinary user | files in your own folder | none needed (read-only) |
| 3 | `find <evidence-folder> -type f ! -name manifest.sha256 -print0 \| LC_ALL=C sort -z \| xargs -0 sha256sum > <evidence-folder>/manifest.sha256` | the evidence folder of the day | write a manifest of every evidence file with its hash | no terminal output; the manifest file holds one hash line per file in a fixed order | creates or overwrites `manifest.sha256` inside the evidence folder | ordinary user | your private evidence folder only | delete the manifest and recompute it; keep earlier days' manifests |
| 4 | `sha256sum -c <evidence-folder>/manifest.sha256` | the manifest | confirm that no evidence file changed since its hash was written | one line per file ending in OK or FAILED, and a summary line if any failed | none | ordinary user | your evidence folder | none needed (read-only) |

## 13. A blocked environment: the seven branches

The plan's seven troubleshooting branches (plan 10.3) as they apply to the capstone. A blocked environment is recorded BLOCKED and scored on what was demonstrated; it is never described as passed.

| Branch | In the capstone |
|---|---|
| Expected | the environment behaves as its record says; each result has its second observation |
| Missing dependency | the lab, a tool, the platform tier or the harness is absent: install nothing; write the diagnostic and the proposed remedy as text; use the substitution in section 7 and record it |
| No connectivity | in K inspect adapters, addresses, routes, router rules and name resolution. Isolation is intentional: a missing path to the LAN or the Internet is the correct state. Never add a route to fix a lab |
| Unexpected result | compare the ground truth with the artifact; state the first and the revised hypothesis; a surprise is data |
| Safety breach | stop; contain within the approved ladder; preserve evidence; report (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 10). The capstone is not passed |
| Restoration | K returns to the recorded baseline, with domain-controller recovery handled as its own procedure; U's instance is closed and anything left removed; record how you know |
| Time exceeded | document the blocker, stop at the limit, and say that the original exercise did not pass |

## 14. Departures from the plan and open points

| What | Why |
|---|---|
| Day 57's draw is at the start of block B, and block A holds the generic scope and the reporting standard | the plan puts "define scope" in A and "enumerate" in B; the target cannot be known when the scope is defined without becoming known for a morning. U's window is 270 minutes |
| C-03 suspends the AI critic allowance for all of week 12 | the plan allows a critic after an unassisted attempt outside exam conditions and forbids AI in the final; the final's evidence is produced on days 56 to 59, so the strict reading is taken (INSTRUCTOR_GUIDE, section 4) |
| Findings from K never count toward the quota and the path may come from K only when U supports none | plan fairness rule; PORTFOLIO, section 2, artifact 4 |
| The hash chain of days 56 to 59 | the plan scores the final on day 60 from evidence made on earlier days; a chain shows that earlier days were not edited. PROPOSED |
| The diagnostic retest placement | plan 1.3 requires it on day 60 and allots no block. PROPOSED |
| The next-step benchmark is defined as a plan | the plan names it without a definition. PROPOSED reading |
| Part 3 as type T for the application's derivation | the task statement of the week must say; three of the eleven artifacts are detections. PROPOSED |
| Readiness of the agent is a record, not a certificate | the plan asks for a readiness gate on day 56 and does not define it. PROPOSED |
| Certification facts are ASSUMED where the registries disagree | the week 6 and week 12 registries differ on the CPTS prerequisite and both leave the OSCP+ AI restriction unconfirmed |

| ID | Open point | Default until decided |
|---|---|---|
| OP-FC-01 | the owner holds a platform tier that yields at least 5 eligible entries for U, disjoint from the midpoint | if not: U is BLOCKED and recorded with diagnostics |
| OP-FC-02 | `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, section 8, holds the reference answers of the diagnostic and of the seed questions, readable by the learner. The source must not be edited, so the discipline of section 2 is the only control | do not read it before day 60 |
| OP-FC-03 | open decision O-02 (a platform VPN profile) and O-05 (the model path) of SAFETY_AUTHORIZATION | browser-based environments only; a recorded transcript |
| OP-FC-04 | who prepares K (protocol 1, another person, or protocol 3, a sealed script) and when its map is committed | not decided |
| OP-FC-05 | a second reader for the final report and the scoring | none; the grade is self-adjudicated |
