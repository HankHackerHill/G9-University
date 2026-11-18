# Instructor key: G9 midpoint offensive benchmark (day 30)

**INSTRUCTOR ONLY. Not part of the learner's view.** The separation from the learner files prevents accidental exposure. It is not access control: learner and instructor are one person and the files share a repository (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 1).

- Reader: the owner, as the examiner. The learner file is `docs/curriculum/v2/ASSESSMENTS/MIDPOINT_OFFENSIVE_BENCHMARK.md`. The marking bands are `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`; this key applies them to an unknown target and does not change them. The Part 1 key of the week 6 theory exam is `docs/curriculum/v2/WEEKS/W06/ANSWER_KEYS/ASSESSMENT_KEY.md`; the entry diagnostic key is in this folder.
- Source of requirements: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 5 week 6, 7.2 and 8.2 (W06-Q08), 9.1 to 9.4.
- Status: PROPOSED, written 2026-11-18. No attempt was scored and nothing was run. Text under "Sample (synthetic)" is invented to calibrate a reader and records no observation.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.
- What this key holds and does not hold. It holds band descriptors, overclaims to reject and questions to ask. It holds no solution, no fixture map and no description of any third-party machine, room or challenge, because the target is a third party's and unknown by design. Where a preparer builds the target (form L of the learner file) the preparer's sealed map is the fixture map and this key is applied to it (section 10).

## 1. Custody and order of use

Open this file only after the attempt is submitted in the window, the locked-answers hash and the evidence-bundle hash are recorded with their times, and the report is hashed. Log each opening on the scorecard with the time and the section (INSTRUCTOR_GUIDE, section 6). An opening before submission is declared: the items it touches score 0 and P3 is in the none band.

| Step | Action | Output |
|---|---|---|
| 1 | gates first: read the command log, the aids log and the report against the scope block and the rules of the learner file, section 3. Record a safety or integrity failure before any point (section 3 of this key) | gate record |
| 2 | classify the outcome: Solved, Partly solved or Not solved (section 2) | the state |
| 3 | Part 1: mark the locked answers with the week 6 key: 1 or 0 for multiple choice, 1, 0.5 or 0 for constructed responses | Part 1 points |
| 4 | Parts 2 to 4: for each sub-criterion write the band, the points and one line that names the artifact (sections 4 and 5). Where two bands are plausible take the lower | sub-criterion lines |
| 5 | sweep for overclaims (section 6) and ask the questions (section 7) | corrections to the lines |
| 6 | add the points; compare each part with its diagnostic target and the total with 80 | totals |
| 7 | derive the application's six levels (INSTRUCTOR_GUIDE, section 10) and enter them in the window | levels |
| 8 | check the deficiency register (section 9.1) and score the diagnostic retest (section 9.2) | register check; domain shares |
| 9 | record on the scorecard and assign the remediation (section 9.3) | scorecard |

The examiner's reference for the target. There is none beyond the learner's evidence. After the bundle hash is recorded you may read the platform's own page for the eligibility record (name, system, difficulty, retired state). Read no third-party write-up before the score is final, and never use one to overrule the evidence. A write-up read afterwards is declared on the scorecard and the target is consumed.

## 2. Establishing the outcome without a solution

Score claims, not flags. The plan asks for "full chain to meaningful verified result" (day 30, block C) and also says to report what was actually demonstrated and to impose no quota. The three states below are PROPOSED readings that let the rubric's bands be applied to any target.

| State | What the evidence must show | What does not establish it |
|---|---|---|
| Solved | a chain from first access to a meaningful verified result: each step has a hypothesis record written before its test, a command log entry and a saved output; the end state crosses a privilege boundary on the target and is shown by two independent artifacts (for example the identity and context before and after, each with a time, and a second observation such as ownership of a resource or a process listing); the critical step was repeated or reset-tested | a flag string; a screenshot without a saved output; a statement; an output whose time falls outside the window |
| Partly solved | at least one significant issue independently verified: an effect on the target that its designer did not intend (unintended access, disclosure, execution, or a writable privileged path), shown with its negative control and its preconditions. The chain is incomplete | a banner or version match; a scanner label; a response difference with no control; a crash; "likely vulnerable" |
| Not solved | no issue verified: enumeration and hypotheses only | n/a |

A platform's acceptance of a proof is a second observation if the learner recorded its time and the statement of acceptance and no flag value. The flag value itself in a report is a handling defect (section 6).

## 3. Gates

Record a failure before points. A gate failure never counts as a passed practical, whatever the total, and is never offset by points. These are midpoint cases in the manner of INSTRUCTOR_GUIDE, section 5. Every case is synthetic and generic.

| Case | Gate | Why | Effect |
|---|---|---|---|
| a wide scan covers addresses other than the assigned instance, found in the command log or a capture | safety | out of scope (STOP-02, N-06) | gate failure; P1 at most 2 |
| contact with the target after 16:15, or in a window that the scope line did not list | safety | out of window | gate failure |
| a guessing list that is not in the scope block, or one of the learner's own passwords, or a list from a breach | safety | N-05 and R-08 | gate failure |
| a persistent change left on the target after the window, or a change on a shared instance that affects others, not removed and not recorded | safety | N-14, R-09 | gate failure when not contained; if removed at once, recorded and shown to have affected nobody, no gate failure and points are lost in P1 and D4 |
| a denial-of-service or stress pattern in the command log | safety | N-17 | gate failure |
| a flaw in the platform's own infrastructure used or exploited | safety | STOP-10 | gate failure; if the learner stopped, recorded and reported privately, none |
| no scope block, or one written after the first target command | integrity | missing target or scope context | the scope criterion is set below its floor (level 0 or 1); P1 none |
| a hint feature, a walkthrough, a video, a forum answer or AI used and declared | none | disclosed | P3 in the none band |
| the same, not declared and traceable | integrity | unacknowledged published solution or AI | gate failure |
| a result presented that is in no saved output | integrity | fabricated output | gate failure |
| "retest passed" or "fix verified" with no artifact, or an artifact from before the change | integrity | a test claimed with no observed result | gate failure |
| a scanner lead labelled Confirmed with no manual reproduction | integrity | a false positive claimed as verified, when knowingly unsupported | gate failure; the honest label is Unconfirmed |
| a flag value or a credential found on the target in the private report | none | a handling defect; the value must not reach any portfolio copy or shared place | E2 loses a point; if shared, N-09 applies |
| a timestamp wrong by a whole hour while the raw log is right | none | an error the raw record corrects | E2 loses a point |
| an output written from memory and labelled as such | none | a labelled reconstruction | E1 and E2 lose points |
| the platform instance did not start and the diagnostics are recorded | none | acceptable partial performance | scored as demonstrated; BLOCKED recorded |
| a lead correctly discounted as a false positive after a recorded manual test | none | acceptable, and good evidence | credited in P2 and P4 |

## 4. Band descriptors per rubric part for an unknown target

For each sub-criterion the first table column is the rubric's band and points. The second column is what the band looks like when the target is solved or a significant issue is verified (states Solved and Partly solved). The third is what it looks like when no issue was verified (state Not solved), including the highest band that state can reach. "R" and "V" are the required items and the verification item of GRADING_RUBRICS. The ceilings are INFERRED from the rubric's bands and are not additional rules.

### 4.1 Part 2: independent practical (40)

**P1 Scope and design correctness (5).** The state of the outcome does not change P1.

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 4 to 5 | R1 a scope block before the first target command: target id and address, allowed actions, exclusions, time limit, stop rule. R2 an ordered test plan that says what each test would show. R3 every logged action names a target in scope. V (5): the target's identity confirmed from an independent source before the first action | the same. An unsolved target can earn 5 |
| Partial 3 | the scope block lacks one element, or the plan gives no reason for its order; every action in scope | the same |
| None 0 to 2 | no block, or written after the actions; no plan; any out-of-scope action (cap 2, and a gate failure) | the same |

**P2 Enumeration and evidence fidelity (10).** An unsolved target can earn the full band if the enumeration is rigorous.

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 8 to 10 | R1 the TCP port range declared in the scope block was enumerated (a full range is feasible in the window, so its omission needs a stated reason), services identified for each open port, web content enumerated at the stated rate where a web service exists. R2 every fact names its source and time. R3 limits stated: what was not observed, and what filtered or closed does not prove. R4 no claim beyond the artifacts. V (10): a second independent source for each key fact | the same. The enumeration must stand on its own because there is no result to rest on |
| Partial 5 to 7 | the main elements found, with one of: some facts without source or time; one tool and no cross-check; limits not stated; one claim beyond the artifact | the same |
| None 0 to 4 | a tool output pasted without reading; a list with no sources; guesses stated as facts; large parts of the assigned surface absent | the same |

**P3 Independent hypothesis and reproduction (15).**

| Band | Solved or verified issue | Not solved (highest reachable: 11) |
|---|---|---|
| Full 12 to 15 | R1 falsifiable hypotheses with their predictions written before the tests. R2 no hint and no outside solution (hint log empty, aids log clean). R3 the critical result reproduced from a clean or reset state, or its non-reproducibility documented. R4 the negative control fails as predicted. R5 the final claim equals what the reproduction shows. V (15): reproduced twice or by a second method, and a revised hypothesis recorded wherever a prediction failed. A chain completed once, with no repeat and no negative control, is Partial however far it went | not reachable: there is no result to reproduce, so R3, R5 and V cannot be met |
| Partial 8 to 11 | a hypothesis and an observed result with no clean-state repeat or no negative control; or a prediction written after the test; or one claim beyond what was reproduced | 10 to 11: at least three falsifiable hypotheses, each tested with its prediction written first, controls, refutations recorded, and at least one revised hypothesis after a failed prediction. 8 to 9: two hypotheses tested with predictions and recorded results |
| None 0 to 7 | trial and error with no hypotheses; no reproduction; a result taken from a published solution; an unreproducible effect reported as a finding | untested guesses; commands with no recorded reasoning; any use of a published solution |

**P4 Impact and root-cause reasoning (10).**

| Band | Solved or verified issue | Not solved (highest reachable: 5) |
|---|---|---|
| Full 8 to 10 | R1 the root cause at the control or trust boundary (what made it possible), not the symptom. R2 preconditions and required privileges stated. R3 impact bounded by the evidence and by exploitability. R4 at least one alternative explanation, with what would disprove each. V (10): a discriminating test that separated the leading cause from the strongest alternative | not reachable |
| Partial 5 to 7 | the cause named at symptom or tool level; or preconditions missing; or impact asserted without a reachability argument; no alternatives | 5: the strongest lead analysed as INFERRED, with its alternatives and the discriminating test named and marked NOT RUN, and no root cause claimed as established |
| None 0 to 4 | no cause; a cause contradicted by the evidence; a severity copied from a scanner label or a score; impact overstated | no analysis, or a root cause stated as established for an unverified issue |

### 4.2 Part 3: defensive validation (25), type F

The hosted target has no control the learner can change, unless the instance is dedicated, the platform's terms allow a change and time remains. Where nothing was applied, D2 and D3 stay in the none band and D4 and E3 are scored from paper analysis that is labelled as such (GRADING_RUBRICS, section 6 D4 and section 8.3; INSTRUCTOR_GUIDE, section 8.2). Maxima on paper: D1 8, D2 3, D3 2, D4 5.

**D1 Correct data source and hypothesis (8).**

| Band | Solved or verified issue | Not solved (highest reachable: 6) |
|---|---|---|
| Full 7 to 8 | R1 the specific source that would show the behavior (a log channel, an event family, an application log). R2 shown enabled and collected, by a configuration excerpt or a sample event read from the target after access. R3 the expected event, fields and window written before the log was read. R4 an absent event not read as an absent action. V (8): the event confirmed at the source and at a second place | not reachable: without access the source cannot be shown enabled and collected |
| Partial 4 to 6 | the right source, but enablement or collection not verified; expected fields vague; window unstated | the sources that the observed services would write named as ASSUMED and labelled unverified, with the expectation written down: 4 to 6 |
| None 0 to 3 | an assumed or wrong source (for example a native Security id taken for a Sysmon id); a missing event taken as proof of no activity; no expectation | the same |

**D2 Positive validation (7).**

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 6 to 7 | R1 a positive case run against the control (the permitted action still works after a fix, or the detected action is detected). R2 its outcome predicted first. R3 timestamps that tie to the action. R4 the artifact attached. V (7): repeated, or confirmed at a second place. Reachable only where a fix was applied on a dedicated instance where the terms allow, or in form L | not reachable |
| Partial 4 to 5 | the positive observed, with times that do not tie, or no prediction, or a single run | not reachable |
| None 0 to 3 | no positive case; a positive claimed with no artifact; the positive not run after the change. A paper-only analysis is here, at 0 to 3 | the same |

**D3 Negative testing (5).**

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 4 to 5 | R1 at least one case that must not be permitted or must not be bypassed, run and recorded (a second principal denied; the same input now failing). R2 a near miss differing in one controlled variable. R3 false-positive and false-negative risks named. V (5): a bypass attempt of the same class against the corrected control. Reachable only with an applied fix or in form L | not reachable |
| Partial 3 | a negative exists but differs trivially, or is described and not run, or the risks are not named | not reachable |
| None 0 to 2 | no negative, or one that tests nothing the control decides. A paper-only analysis is here, at 0 to 2 | the same |

**D4 Hardening and mitigation assessment (5).**

| Band | Solved or verified issue | Not solved (highest reachable: 3) |
|---|---|---|
| Full 4 to 5 | R1 the mitigation acts on the root cause named in P4. R2 side effects and regression risks stated. R3 rollback stated. R4 residual risk stated. V (5): applied in an approved setting, or analysed on paper with the label and the reason why it was not applied, and the permitted function checked again in that analysis | not reachable |
| Partial 3 | a plausible mitigation with a weak link to the root cause, or residual risk not stated | 3: a hardening measure tied to the exposure that was observed, with side effects stated, and no root cause claimed |
| None 0 to 2 | generic advice ("patch it", "harden the server"); a mitigation that breaks a permitted function; none | the same |

### 4.3 Part 4: evidence and reporting (15)

**E1 Reproducibility (5).**

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 4 to 5 | R1 environment and versions. R2 exact commands with status words. R3 inputs and hashes. R4 expected against observed for each step. R5 cleanup. V (5): the steps followed again from the report alone, by another person or by the learner from a clean state after 24 hours, with deviations recorded. Timing note below | the same, for the steps that were performed |
| Partial 3 | the steps present, but a version, an input or the cleanup missing, so a reviewer would have to guess | the same |
| None 0 to 2 | a narrative with no steps; steps that cannot be followed; no target or scope reference | the same |

Timing note for E1's verification item. The rubric asks for a second run after writing the report. It cannot happen inside block D for a hosted target whose window has closed, and work after the limit is an addendum (GRADING_RUBRICS, section 3 rule 11). PROPOSED: the verification item counts only if the second run, or a desk re-derivation of every stated observation from the saved raw outputs by a second reader or by you after 24 hours, is recorded before the score is finalized in the window. The owner may hold the E1 line open for up to 5 weekdays, outside the counted hours, for that purpose; otherwise E1 is capped at 4. A second run against the instance needs its own scope line and window. See open point OP-MK-03.

**E2 Precision and provenance (5).**

| Band | Solved or verified issue | Not solved |
|---|---|---|
| Full 4 to 5 | R1 every claim labelled OBSERVED, INFERRED, ASSUMED or NOT RUN. R2 every observation names its artifact, time and zone. R3 the bundle listed with SHA-256 hashes. R4 limits and unknowns stated. R5 severity argued from preconditions and impact (a lead is not rated above its evidence). V (5): the bundle hash written before any key was opened, and it matches the bundle scored | the same |
| Partial 3 | labels present but some claims untagged, some times without a zone, some hashes missing | the same |
| None 0 to 2 | no provenance; inference unlabelled; times without sources | the same |

**E3 Remediation and retest (5).**

| Band | Solved or verified issue | Not solved (highest reachable: 2) |
|---|---|---|
| Full 4 to 5 | R1 a specific recommended correction. R2 a retest procedure with a positive and a negative case. R3 the observed result of the retest, or an explicit statement of why it could not be run. V (5): the retest was run and its artifacts attached. On paper with the reason stated the best score is 4 | not reachable: there is no verified issue to correct |
| Partial 3 | a specific correction and a described retest that was not run, with no reason given | not reachable |
| None 0 to 2 | a generic correction; no retest; "retest passed" with no artifact (also an integrity failure) | generic hardening of what was seen: 0 to 2 |

### 4.4 What each outcome can reach, in one table

| Part | Solved or verified issue | Not solved |
|---|---|---|
| 1 | 20 | 20 |
| 2: P1, P2, P3, P4 | 5, 10, 15, 10 = 40 | 5, 10, 11, 5 = 31 |
| 3, paper only: D1, D2, D3, D4 | 8, 3, 2, 5 = 18 | 6, 3, 2, 3 = 14 |
| 3, a fix applied and retested, or form L | 8, 7, 5, 5 = 25 | 14 |
| 4, paper-only remediation: E1, E2, E3 | 5, 5, 4 = 14 | 5, 5, 2 = 12 |
| 4, a retest run | 15 | 12 |
| Total | 92 on paper, 100 with an applied fix | 77 |

It matches section 8.1 of the learner file.

## 5. Calibration excerpts

Sample (synthetic): invented excerpts to show what a band looks like in the evidence. They describe no real target and no real weakness.

### 5.1 P3, hypothesis records

```text
FULL band (one record of a set of four, each like this)
H-02 | written 14:12 America/Chicago | service S on the assigned address returns data without credentials |
  if true: one listing request with no credential returns entries; if false: an authentication error |
  test: one request, target id <id> | run 14:19 | observed: entries returned (OBSERVED, output <artifact id>, saved 14:19) |
  verdict: supported | control: the same request with a wrong credential returns an authentication error (OBSERVED, <artifact id>) |
  repeat: after the instance was reset at 15:02, the same request returned the same entries (<artifact id>) | next: H-03

PARTIAL band
H-02 | written 14:25 | service S allows anonymous access | test run 14:19, output <artifact id> | verdict: supported
  (the record was written after the test; there is no control and no repeat)

NONE band
"Tried several things against S and got in." No hypothesis, no prediction, no output reference.
```

### 5.2 P4, cause statements

```text
FULL band
Cause: service S evaluates no authorization decision before returning directory entries, so the trust boundary between
unauthenticated callers and stored entries is absent (a control missing at the service, not a tool or a version).
Preconditions: network reach to the service port; no credential (OBSERVED, H-02). Impact: disclosure of the entries
that the listing returned (OBSERVED); no claim is made about other data (NOT RUN). Alternative: the entries are public
by design; what would disprove it: the service documentation or a policy that marks them public; tested by comparing
against the second listing request with a different base (<artifact id>), which returned different entry types.

PARTIAL band
Cause: "an outdated version of S". (A symptom at the version level; no preconditions; no alternative.)

NONE band
Cause: "S is insecure". Severity: Critical, copied from a scanner label.
```

## 6. Overclaims to reject

| Claim or pattern | Why it is rejected | Effect |
|---|---|---|
| "root obtained", "full compromise", with no identity and context shown before and after, no times, and no second observation | the end state is not evidenced | the chain is classified Partly solved at best; the claim is Unconfirmed |
| "vulnerable to" a named CVE identifier, from a banner or version | a version match is not demonstrated exploitability (W06-Q02) | Unconfirmed; P4 none for that claim |
| a scanner label ("critical") used as the priority | a priority is argued from preconditions and impact, never copied | P4 and E2 |
| "remote code execution" from a crash, a response difference or an error message | a crash is not a vulnerability and not remotely exploitable by itself; attacker control and reachability must be shown | P4 none for that claim |
| "no hints used" while the report or the hypothesis log follows a public write-up's order and wording | possible undeclared exposure | ask question 5 of section 7; undeclared and traceable is an integrity failure |
| a flag value, or a credential found on the target, in the report | a handling defect and a possible spoiler | E2 loses a point; the value is removed from any portfolio copy |
| "all ports scanned" while the command log shows a limited range or the top ports only | a claim beyond the artifact | P2 partial |
| "filtered means no service" or "closed means firewalled" | a state is not a conclusion (plan, week 2) | P2 partial |
| a path described as achieved when an edge was not tested | an unlabelled edge is not OBSERVED | P3 partial |
| intent or attribution ("the author left a backdoor") | intent is not observable from the artifacts | an overclaim; P4 |
| a CVSS score with no vector, or a vector whose metrics contradict the stated preconditions | a score is one input and is the learner's own calculation | P4 and E2 |
| "the control would have detected it" with no telemetry read | an inference that must be labelled INFERRED | D1 none for that claim |
| "no persistence left" with no cleanup record | the claim has no artifact | P1 and D4 |
| "finished within 150 minutes" when the log shows contact after 16:10 or 16:15 | out of window | safety gate |
| times that do not convert (a local time written as UTC) | a zone error; the raw record decides | E2 loses a point; fabrication only if the record is altered |
| a negative control, or a prediction, written after the result and presented as written before | the sequence is a claim | P3 partial; if presented as written before, an integrity matter |
| a hypothesis log with identical times for several records | likely written in one pass | question 2 of section 7; P3 partial |
| a "second run" identical to the first in every detail, down to times and identifiers | real runs differ | an integrity matter if copied |

## 7. Questions to ask of the evidence

Ask in writing, as a second reader or yourself after 24 hours. The answer must point to a record. A sound answer names the artifact id, its time and what it shows. An answer that depends on memory is marked as such.

| # | Question | A sound answer points to |
|---|---|---|
| 1 | which record, with which time, names this target and its stop rule before the first target command? | the scope block and its saved time, earlier than the first command log line |
| 2 | for hypothesis H-n, where is the prediction and when was it written, compared with the test time? | the hypothesis log row and the output's time |
| 3 | what would have shown the hypothesis false, and was that test run? | the control record |
| 4 | which second source shows this inventory fact? | a pair such as a scan against a socket or banner observation, or a packet against a response |
| 5 | in which step did you first learn what to try, and from where? | the hypothesis that preceded it, or a declared exposure |
| 6 | which output shows the identity and context before the boundary and which after? | two saved outputs with times |
| 7 | what is the root cause, in terms of the missing control, and what is the strongest alternative? | the cause statement and the alternatives table |
| 8 | what does the evidence not show about impact? | the limits statement |
| 9 | where is the target's own record of your action, and how do you know it is enabled and collected? | a configuration excerpt or a sample event read from the target |
| 10 | which mitigation acts on that cause, what does it break, and how would you undo it? | the mitigation text with side effects and rollback |
| 11 | which retest artifact is dated after the change, and does it show the changed state? | the retest record, or the statement that none was run |
| 12 | what did you leave on the target, and where is the record of removing it? | the cleanup record |
| 13 | which claims in the report are INFERRED, and which test would separate them from their alternatives? | the labelled claims and the discriminating tests |
| 14 | what hash was recorded before any key was opened, and does it match the bundle scored? | the hash record and a verification by `sha256sum -c` (proposed text, section 11) |
| 15 | what would you do first in the next hour if the window were longer, and why? | the next-tests list, marked NOT RUN |

## 8. Worked scoring (synthetic)

Sample (synthetic): three invented attempts, to show the arithmetic and how the two records diverge. None is a record of an attempt. Part 3 is type F in all three.

| Line | 1: solved, paper-only Part 3 | 2: partly solved | 3: not solved |
|---|---:|---:|---:|
| Part 1 | 17.0 | 16.0 | 18.0 |
| P1, P2, P3, P4 | 5, 9, 14, 9 = 37 | 5, 8, 12, 8 = 33 | 5, 9, 10, 4 = 28 |
| D1, D2, D3, D4 | 7, 2, 2, 4 = 15 | 6, 1, 1, 4 = 12 | 5, 0, 0, 2 = 7 |
| E1, E2, E3 | 4, 5, 4 = 13 | 4, 4, 3 = 11 | 4, 4, 1 = 9 |
| Total | 82.0 | 72.0 | 62.0 |
| Plan gate (80, both gates passed, week conditions) | MET | NOT MET | NOT MET |
| Parts against their targets (16, 30, 18, 12) | Part 3 below its target; the others met | Parts 3 and 4 below | Parts 2, 3 and 4 below |
| Gate failures | none | none | none (acceptable partial performance, plan 9.4) |

Attempt 1 in the application's terms:

| Criterion (weight) | Set | Share | Level | Score |
|---|---|---:|---:|---:|
| independent_execution (25) | P2 + P3 = 23 of 25 | 92 percent | 3 | 20.00 |
| root_cause_reasoning (20) | P4 = 9 of 10 | 90 percent | 3 | 16.00 |
| remediation_verification (20) | D2 + D3 + D4 + E3 = 12 of 22 | 54.5 percent | 2 | 10.00 |
| detection_visibility (15) | D1 = 7 of 8 | 87.5 percent | 3 | 12.00 |
| evidence_documentation (10) | E1 + E2 = 9 of 10 | 90 percent | 3 | 8.00 |
| scope_compliance_reporting (10) | P1 = 5 of 5 | 100 percent | 4 | 10.00 |
| Total | | | | 76.00 |

The plan gate is MET at 82.0 and the application FAILS at 76.00, because paper-only defensive analysis cannot lift `remediation_verification` above level 2. Both records are written down, with the reason, and neither is converted into the other. The application's decision would pass here only if other criteria were near their maxima, for example `independent_execution` at level 4 and the rest at level 3 or better, with `remediation_verification` at level 2 and `scope_compliance_reporting` at level 4 (a total of 81 with `root_cause_reasoning`, `detection_visibility` and `evidence_documentation` at level 3). This is the structural cost of a hosted target (open point OP-MK-02).

Attempt 2 shows an honest verified issue with a thin defensive part: below 80, so the week is NOT MET without any gate failure. Attempt 3 shows an unsolved target with disciplined work: below 80, no gate failure, and a deficiency register whose rows come from the hypothesis log and the blocker.

## 9. The register, the retest and the decision

### 9.1 Checking the deficiency register

| Check | Pass |
|---|---|
| every row has an artifact id and a time | no row without evidence |
| no deficiency is entered that the evidence shows the learner did not have | the row's evidence supports the deficiency |
| the class is one of K, M, T, E, C, V | present |
| the row says what the evidence does not show | present, so cause and circumstance are not confused |
| the remedy is optional work and hours are not added; required labs, block C and Friday assessments are never substituted | consistent with INSTRUCTOR_GUIDE, section 9 |
| the register does not contradict the report's limits | otherwise E2 loses a point |
| no blame or intent is written | present |

The register is checked and is not separately scored. An absent register is a missing deliverable and is recorded.

### 9.2 Scoring the diagnostic retest

After the attempt is submitted: open the entry diagnostic's key in this folder; mark 1, 0.5 or 0 per question by the key's marking points; compute the share per domain with the grouping of `docs/curriculum/v2/STUDENT_GUIDE.md`, section 9; compare with day 1. Small domains move by large steps (one question moves a two-question domain by 50 points), so read the missed questions as well as the percentages. State on the scorecard whether the same items as day 1 were used, and that the key was open after day 1, so that growth is overstated by an unknown amount (INFERRED). Re-place each domain on its track. The retest is not part of the 100 points and is not a claim of mastery.

### 9.3 The decision record and remediation

| Trigger | Track (INSTRUCTOR_GUIDE, section 9) | Week 6 record |
|---|---|---|
| Part 1 below 16 | K | the missed questions into flashcards and the 1, 3, 7-day queue |
| Part 2 below 30, or a sub-criterion in the none band | P | the next three block C tasks show the habit named by the band |
| Part 3 below 18 | D | the next three block D records; consider OP-MK-02 first, because on a hosted target the ceiling is 18 |
| Part 4 below 12 | E | rewrite the report to the week's evidence checklist and follow it cold |
| total below 80, no gate failure | W | week 6 recorded NOT MET; the schedule continues; the deficiency register carries it |
| safety gate failure | S | stop; the safety track; re-confirm scope and isolation in writing |
| integrity gate failure | I | three consecutive lab records traced line by line to raw artifacts |
| the environment prevented the attempt | B | BLOCKED, not a failure; a new attempt with a fresh draw, no credited hours |

The week's own conditions are read next to the score: one significant issue independently verified; one privilege boundary demonstrated or assessed; a report with root cause and risk (plan, week 6 gate). A week with a total of 80 or more and an unmet condition is NOT MET (INSTRUCTOR_GUIDE, section 3). The window decision is entered beside it, with its reliability class, which is SELF_ADJUDICATED when the reviewer is the learner.

## 10. Form L: a local simulated corporate finding

Form L has the same rules and the same bands, with these differences.

| Item | Rule |
|---|---|
| Preparer | another person only (protocol 1); a script is not admissible for the midpoint |
| Fixture map | the preparer's sealed file, in the format of section 4 of `docs/curriculum/v2/ASSESSMENTS/FINAL_CAPSTONE_INSTRUCTOR.md`; its hash was given before the attempt; the learner verifies it after submission |
| Scoring against the map | list the map's faults and mark each as found and evidenced, found and not evidenced, missed, or a false claim (a claim with no fault behind it); P2 and P3 are scored from what was evidenced; a false claim is an overclaim |
| Part 3 | the preparer's telemetry expectations apply; D2, D3 and E3 can reach their full bands when the fix and the retest were run |
| Gate | class E scope line, signed RoE, isolation tests current for the week 6 stack (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 13) |
| Availability | none while Stage 00 is unresolved |

## 11. Commands proposed for review

Proposed text only. None was run. Each needs the owner's review and approval of the exact text first.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `sha256sum -c <manifest>` | the manifest of the evidence bundle and of the locked answers | confirm that what is scored is what was hashed before any key was opened | one line per file ending in OK or FAILED, and a summary if any failed | none | ordinary user | the learner's evidence folder | none needed (read-only) |
| 2 | `sha256sum <file> [<file> ...]` | the sealed map of form L, the pool file | verify a commitment after submission | one line per file: 64 hexadecimal digits, then the name | none | ordinary user | the files named | none needed (read-only) |
| 3 | `date -u +%Y-%m-%dT%H:%M:%SZ` | the clock of the machine on which you score | log each key opening in UTC | one line: year-month-day, T, hours:minutes:seconds, Z | none | ordinary user | the machine | none needed (read-only) |
| 4 | `python3 -I scripts/check_doc_refs.py` | the documents of the repository | check that every repository path a document names exists | exit status 0 and a count of references checked, or one line per broken reference | none | ordinary user | the repository working tree | none needed (read-only) |

## 12. Departures from the plan and open points

| ID | What | Why |
|---|---|---|
| D-MK-01 | three outcome states (Solved, Partly solved, Not solved) | the rubric's bands need a way to apply to an unknown target; the plan says to score what was demonstrated. PROPOSED |
| D-MK-02 | an unsolved attempt cannot reach P3 Full, P4 Full, D2, D3 or more than 2 of E3 | the rubric's required items need a result to reproduce and a cause to name. INFERRED from GRADING_RUBRICS; the plan asks only that partial evidence be graded |
| D-MK-03 | paper-only Part 3 stays in the none band for D2 and D3 | the rubric's none band: no positive case run after a change. The plan's day 30 row has no defensive task; its Friday score has 25 defensive points |
| D-MK-04 | the E1 verification item can be held open for up to 5 weekdays | the rubric's second run cannot happen in block D for a closed hosted window. PROPOSED; open point OP-MK-03 |
| D-MK-05 | a flag value in a private report is a handling defect, not a gate failure | the plan's gates do not list it; N-09 applies to publication. PROPOSED |
| D-MK-06 | the examiner's reference for the target is the evidence | no key can exist for a third party's machine |

| ID | Open point | Default until decided |
|---|---|---|
| OP-MK-01 | a second reader for the scoring | none; the grade is self-adjudicated |
| OP-MK-02 | whether to accept the structural ceiling of a hosted target on Part 3, or to add a small local fixture exercise earlier in week 6 so that D2 and D3 can be scored on a real control | accept the ceiling; see the learner file, OP-MP-02 |
| OP-MK-03 | the timing of E1's verification item | cap E1 at 4 unless the second run is recorded before the score is finalized |
| OP-MK-04 | the week 6 `ASSESSMENT_KEY.md` and `ASSESSMENT.md` are written by another task; any difference with this key is a defect to reconcile | this key and the learner file, as the stricter text, apply |
