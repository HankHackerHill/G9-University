# G9 Cybersecurity Mastery v2.0: midpoint offensive benchmark (day 30)

- Reader: the owner, as the learner. How a Friday is scored, how keys are released and how an unseen practical is prepared when one person holds every role are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`. The band descriptors are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`. The examiner's key for this benchmark is instructor material and is not named here beyond its path: `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/MIDPOINT_KEY.md`.
- Source of requirements: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 0.3 (midpoint), 1.3 (diagnostic retest, no hints, weekly gate), 5 week 6 (the day 30 row, the lab process, the midpoint benchmark, the gate, the authoring expansion), 7.2 (W06-Q08), 9.1 to 9.4 and 13; `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md`.
- Status: PROPOSED, written 2026-11-18. No pool is fixed, no target is drawn, no attempt exists, nothing was run, scanned or tested. Every command in this file is proposed text for the owner's review. Text marked `Sample (synthetic):` is invented to show a format and records no observation.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN, ESTIMATE (a planning figure that was not measured).
- This file holds no answer, no solution, no fixture map and no description of any target's weakness. It cannot: the target is a third party's and unknown to you by design.
- Precedence: if a week 6 file (`docs/curriculum/v2/WEEKS/W06/ASSESSMENT.md`, `docs/curriculum/v2/WEEKS/W06/EVIDENCE_CHECKLIST.md`, the day 30 guide) is looser than this file on a rule of section 3, the stricter text applies and the difference is recorded as a defect.

## 1. Purpose

The benchmark is the plan's day 30 practical (plan section 9.3: "unknown scoped infrastructure target", what cannot be assumed: a pre-known vulnerability or a solved walkthrough). It is also the Friday practical of week 6, so the Friday rubric applies unchanged.

| Question | Answer |
|---|---|
| What does it ask | Can you run the whole offensive loop alone, on one unknown and explicitly assigned authorized target, with no hints, no AI and no published solution, and report only what the evidence supports? The loop is the plan's: scope, enumerate, hypothesize, validate, assess impact, inspect telemetry, remediate, retest, report |
| What does it measure | the four parts of the Friday rubric: closed-book knowledge (20), independent practical (40), defensive validation (25), evidence and reporting (15) |
| What does it not measure | speed to a flag; breadth over several machines (the plan forbids a two-medium-box quota); skill in a third-party engagement; certification readiness; mastery of a domain by itself |
| What is credit | demonstrated and evidenced work. A flag string is not credit. A hypothesis refuted with its negative control is credit |
| What a result supports | your own planning and remediation. A one-person grade is self-adjudicated and supports no external claim (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 1) |
| Aim (plan 9.2) | at least 80 of 100 on the same rubric, with the safety gate and the evidence-integrity gate passed, and the week 6 conditions met: one significant issue independently verified, one privilege boundary demonstrated or assessed, and a report with root cause and risk (plan, week 6 gate) |

Guided labs and flags from days 26 to 29 are learning, not assessment. They are never evidence of unseen-target competence (plan 1.3).

## 2. State on 2026-11-18

| Item | State | Consequence |
|---|---|---|
| Pool of candidate targets | not fixed | section 5 says how and by when |
| Hosted platform accounts and tiers the owner holds | not recorded. ASSUMED unknown | the pool can only use a platform whose tier you hold on the day the pool is fixed |
| Registry evidence for candidates | `docs/curriculum/v2/WEEKS/W06/RESOURCES.json` lists retired Hack The Box machine pages and TryHackMe beginner rooms. Every entry is SEARCH_CHECKED on 2026-11-18, which means a search result was seen and the page was not opened. `docs/curriculum/v2/WEEKS/W12/RESOURCES.json` names no individual candidate: its pool entries are index pages and its gap list says so | each candidate is re-checked by you by opening its page (section 5.3) |
| Access tier of retired Hack The Box machines | the week 6 registry records them as PAID (a subscription tier above the free plan), SEARCH_CHECKED, not read. ASSUMED here, see `docs/curriculum/v2/RESOURCE_REGISTRY.md` (generated later from the week registries) | without that tier these entries are not in your pool |
| Stage 00 (libvirt socket) | unresolved (`docs/curriculum/v2/README.md`) | the hosted form of this benchmark needs no local guest (INFERRED from the week 6 row of `docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 13). The local form of section 5.1 does, and is unavailable while Stage 00 is blocked |
| How a hosted platform's VPN profile is reached | open decision O-02 of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`; the default is browser-based environments only | an entry that needs a VPN profile on the host is ineligible until you decide O-02 and record it (eligibility E-07) |
| Platform terms | none read by the owner (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 8.2) | class F is unavailable for a platform until you record the date its terms were read |
| Days 26 to 29 | not done | this benchmark follows them in sequence; a shifted calendar moves the date and not the order |
| The date | the original calendar puts day 30 on Friday 2026-12-25, a planned training day. The plan allows shifting dates and never deleting hours | if shifted, the pool, the draw and every hash move with the new date; the pool is not changed after it is fixed |

## 3. Rules

Every rule applies from 08:00 on the day until the attempt is submitted. The last column says what a breach costs under `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`. A gate failure means the practical never counts as passed, whatever the total (plan 9.4).

| ID | Rule | Basis | If broken |
|---|---|---|---|
| R-01 | One target. Exactly one target, named by its target id (`HOSTED:<provider>:<slug>`) and assigned to you by the procedure of section 5. No second machine, no pivot to any host the platform shows beyond it | plan, week 6 lab process: the sole target must be explicitly assigned; N-04 and N-06 of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` | safety gate failure; P1 at most 2 |
| R-02 | Authorization first. A signed rules-of-engagement record and a class F scope line (section 4) exist before the first packet; the platform's terms were read and the date is written down | SAFETY_AUTHORIZATION, sections 2.2 to 2.4 and 12 | no first action is available; an action taken anyway is out of scope |
| R-03 | No hints. The hint ladder does not exist in a scored attempt: hint level 0. A platform hint, a discussion thread, an official or community write-up, a video, a chat community and another person's help are hints. The preparer of protocol 1 gives logistics help only, and each contact is logged | plan 1.3 and 9.1; STUDENT_GUIDE, section 6 | P3 in the none band; an undisclosed use is also an evidence-integrity failure |
| R-04 | No AI. No chatbot, code or command completion service, AI search summary or agent on any device you use, from 08:00 until the attempt is submitted. An assistant may act as a critic only after submission and outside the scored work (STUDENT_GUIDE, section 11) | plan 0.3, 9.1, 9.2; N-15 | disclosed: P3 in the none band. Undisclosed: evidence-integrity failure |
| R-05 | No published solution. Do not search for, read or watch a solution of the target. Do not search the target's name at all until you have submitted. Allowed references are not hints: the documentation of a tool you use, and CVE and CWE records looked up by identifier or by a product and version you observed on the target. A third-party exploit or proof of concept is allowed only if you declare its source in the aids log and read it through before it runs (plan 0.4: no unreviewed code execution). If a search result turns out to be a walkthrough of this target, stop reading, write the time and the source in the aids log, and continue: the exposure is declared and the objective is no longer independent | plan 9.4 (unacknowledged use of a published solution); plan, week 12: never a challenge whose solution you read | undeclared: evidence-integrity failure. Declared: P3 in the none band |
| R-06 | Time box. The attack window is block C: 13:45 to 16:15 America/Chicago, 150 minutes, and it includes the target-specific scope step. No target contact after 16:10; 16:10 to 16:15 closes the instance and removes anything you left. Work after the limit is an addendum and is not scored | plan 1.2 and 9.1; GRADING_RUBRICS, section 3 rule 11; STOP-07 | contact after the window is out of scope |
| R-07 | Stay on the target. Only the address the platform assigns to the instance. No scan or contact of any other platform address, of other users' machines, of the platform's own infrastructure, of the household LAN or of the Internet beyond what the platform's own environment needs | N-06; section 3.4 of SAFETY_AUTHORIZATION | safety gate failure |
| R-08 | Credentials. Only credentials the platform provides or that you find on the assigned target and use against that target only. Online guessing only against the target, only with a list that you wrote into the scope block before the first attempt, and rate-limited. Never a real credential, a credential from a breach, or one of your own passwords | N-05; plan, week 6: no uncontrolled credential collection | safety gate failure |
| R-09 | Leave nothing behind. No denial-of-service or stress test, no destructive action, no persistence left after the window, nothing that spoils the instance for the next user. On a shared instance make no configuration change on the target. A fix is applied only on a dedicated instance and only where the platform's terms allow it (section 8) | N-14 and N-17 | safety gate failure when the exposure or the change is not contained; otherwise points in P1 and D4 |
| R-10 | Evidence is raw, timed and kept as you go: the command log, the outputs, the hypotheses and the hint and aids logs (section 7). Nothing is reconstructed from memory without saying so | plan 9.4; INSTRUCTOR_GUIDE, section 5 | fabricated or unreconstructed claims: evidence-integrity failure |
| R-11 | Honest claims. Every claim carries OBSERVED, INFERRED, ASSUMED, UNVERIFIED or NOT RUN. A scanner result is a hypothesis. A flag value is never copied into the report: record the platform's acceptance as a time and a statement, not the string | plan 0.4, 9.4; PENTEST_FINDING status rules; SAFETY_AUTHORIZATION, section 8.1 | a false positive claimed as verified: evidence-integrity failure |
| R-12 | Private by default. The report of a platform-designated target is never published. The portfolio receives a skill record and the sanitized deficiency register (section 7), never steps, credentials found, flags or screenshots | SAFETY_AUTHORIZATION, section 8.1; `docs/curriculum/v2/PORTFOLIO/README.md`, sections 5.3 and 6 | portfolio defect |
| R-13 | Stop conditions. STOP-01 to STOP-16 of SAFETY_AUTHORIZATION apply. A stop condition ends offensive work. The rest of the block is for the evidence you already have and for the report; resuming needs a fresh approval, which a scored attempt does not give for STOP-02, STOP-03, STOP-05, STOP-08, STOP-09 or STOP-10 | SAFETY_AUTHORIZATION, section 9 | depends on the cause (section 4.3) |

## 4. Scope, authorization and stop conditions

### 4.1 What the scope block holds

The first record of the attempt for the target is a scope block, timestamped before the first command. It is also the P1 evidence of the rubric (scope block, ordered test plan, independent confirmation of the target's identity).

| Element | Content |
|---|---|
| Authorization | the scope line id (`D30-C`), the RoE record id, the platform, the date its terms were read |
| Target | the target id and the address the platform shows for the instance, with the time you read it and the source (the platform's page or the in-browser environment's own display) |
| Source | where your traffic comes from: a platform-provided in-browser attack environment if the exercise offers one (availability is ASSUMED, not established), otherwise the entry is ineligible under the default of O-02 |
| Window | 2026-12-25T13:45 to 2026-12-25T16:15 America/Chicago (UTC-6 on that date; ASSUMED standard-time offset, confirm on the clock you record) |
| Allowed technique classes | named, because a class not named is not allowed (SAFETY_AUTHORIZATION, section 2.4). Default set for this benchmark: TCP connect scan and service and version detection of the assigned address; manual protocol interaction with discovered services; web content discovery at a stated request rate; use of credentials found on the target against that target; local enumeration after access; privilege escalation on the target; read-only inspection of the target's own logs |
| Denied | every address but the assigned one; other users and other instances; the platform's infrastructure and website beyond normal use; the host, the household LAN, the Internet; denial of service and stress; guessing beyond the written list; any persistence that survives the window; any tool or script you did not read first |
| Guessing list | the exact list of credentials you allow yourself to try online, or "none" |
| Stop rule | STOP-01 to STOP-16, plus: three consecutive tests that did not match their prediction and are not understood (STOP-15) end the block's offensive work after you write the hypothesis |
| Time limit | 150 minutes, with the checkpoints of section 6.3 |
| Test plan | the ordered hypotheses to test first and what each test would show |
| Identity confirmation | the independent source that confirmed the target's identity before the first action, such as the platform's instance page showing the same name and address |

Sample (synthetic): a scope line in the format of SAFETY_AUTHORIZATION, status PROPOSED, not approved. Angle brackets are placeholders.

```text
SCOPE D30-C | TARGET HOSTED:<provider>:<slug> | CLASS F | ADDR the address the platform shows for this instance | FROM the platform's in-browser attack environment | WINDOW 2026-12-25T13:45/2026-12-25T16:15 America/Chicago | ALLOW TCP connect scan and version detection of the assigned address, manual interaction with its services, content discovery at the stated rate, use of credentials found on this target against this target, local enumeration and privilege escalation on this target | DENY every other address, other users, the platform's own infrastructure, the host, the household LAN, the Internet, denial of service, guessing beyond the written list, persistence | STOP STOP-01..STOP-16 | ROE <record id> | TERMS <platform> <date read>
```

### 4.2 Rules-of-engagement record

Use the template of SAFETY_AUTHORIZATION, section 12. It is signed before the first action. When you draw the target yourself (protocol 2) you may sign it before the draw with the pool's target ids listed in its section 2 and add the drawn id by a dated amendment line; the other entries are then excluded by the draw. Under protocol 1 the preparer hands you the target id at the start of block C, and you complete and sign the record then. The signing time is the start of the window.

### 4.3 Stop conditions in this benchmark

| Code | What it looks like here | Effect on the attempt |
|---|---|---|
| STOP-01 | doubt whether an address or action is in scope | stop; re-read the scope block; if still in doubt it is out of scope. Not a gate failure when no action was taken |
| STOP-02 | any traffic or response involving an address that is not the target (for example a scan tool that wandered across the platform network) | stop the tool; record the address and time. Safety gate failure when your tool sent the traffic |
| STOP-05 | a credential, token or personal datum that is not synthetic or platform-provided shows up in the log or the bundle | stop; do not copy it further. Safety gate failure when it was a real credential used |
| STOP-07 | the window ended | stop; document the blocker; the remainder is report only |
| STOP-08 | doubt about the integrity of evidence: an output was not saved, a result cannot be reproduced | stop scoring that evidence; say so in the report |
| STOP-09 | a platform warning, block or contact | stop; read the notice; do not work around it |
| STOP-10 | you find a flaw in the platform's own infrastructure or in software outside the designated target | stop exploiting; keep minimal evidence; report privately through the platform's disclosure channel; no public disclosure |
| STOP-15 | three consecutive unexplained results | pause offensive work, write the hypothesis, continue only with a plan |

## 5. Choosing a target that is unseen

An unseen target is one whose faults you do not know. The plan forbids treating an environment you seeded as unseen (plan section 5, week 12 fairness rule; INSTRUCTOR_GUIDE, section 8). For the midpoint only protocols 1 and 2 of the instructor guide are admissible; protocol 3 (a sealed self-made script) is not, because a script you wrote cannot produce an unknown target.

### 5.1 Two forms

| Form | Target | When available | Part 3 |
|---|---|---|---|
| H, hosted (default) | one platform-designated retired or beginner exercise drawn from a pool | when the platform tier is held and the entry is eligible | mostly paper analysis, with a fix and retest only on a dedicated instance and where the terms allow (section 8) |
| L, local simulated corporate finding (plan 0.3 allows "unknown target / simulated corporate finding") | a scenario on the G9 lab built by another person (protocol 1 only), with the fixture map sealed by hash | only when Stage 00 is resolved and the isolation tests T1 to T3 and T5 to T9 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` are current for the week 6 stack (RTR, ATK, TGT, DC, WKS) | can reach the full 25 points, because a control can be changed and retested |

Form L has the same rules as form H except that the scope line is class E and the sources, addresses and exceptions are the lab's (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`). If no eligible hosted pool exists and Stage 00 is unresolved, the benchmark is recorded BLOCKED with the diagnostics; a guided room is never substituted for an unknown target.

### 5.2 The pool

| Item | Rule |
|---|---|
| When | fixed before week 6 starts: by Friday 2026-12-18 on the original calendar |
| Size | at least 5 eligible candidates (INSTRUCTOR_GUIDE, section 8.2) |
| Balance | PROPOSED: at least two Linux and two Windows entries, because week 6 teaches privilege boundaries on both |
| Form of the record | a text file listing, for each candidate: platform, name, target id, state seen (retired or beginner), date seen, access tier seen, result of each eligibility check. No description of what the target contains, and nothing you learned about it beyond that |
| Commitment | the SHA-256 of the pool file and its time are written on the scorecard before week 6 (command 2 of section 11). Under protocol 1 the preparer holds the file and gives you the hash |
| After it is fixed | no candidate is started, attempted, dry-run, searched or read about, by you or by an assistant, until the attempt is submitted. Reading a candidate's page for the eligibility check is the only exception and happens once, before the pool is fixed |
| Not in the pool | any exercise a day guide of weeks 1 to 6 uses as a guided or independent task, such as the rooms the week 6 registry assigns to days 26 to 29; any exercise you have attempted, even in part |

### 5.3 Eligibility

All checks pass, or the candidate is out. Each check is made by you, on the platform's own page, on a recorded date. The registry's SEARCH_CHECKED label is not a check.

| ID | Check | Why |
|---|---|---|
| E-01 | platform-designated for training, and the exercise is named by its target id | plan 0.4: designated targets only |
| E-02 | the page says retired (or the exercise is on the platform's beginner tier). Never an active machine whose solution the platform bars from publication. A machine can be both active and expired: the page must say retired | plan section 4 (no active-machine spoilers); registry check `plan_fact_checks` in `docs/curriculum/v2/WEEKS/W06/RESOURCES.json` |
| E-03 | never started, in guided or unguided form, in any session | unseen |
| E-04 | no solution, hint, walkthrough, video or discussion of it read, heard or seen. If you cannot be sure, it is out | plan, week 12 |
| E-05 | the access tier is held now, read from your own account and not from the registry | registry access fields are search results |
| E-06 | the platform's terms and its write-up and spoiler policy were read and dated | SAFETY_AUTHORIZATION, section 8 |
| E-07 | reachable without a VPN profile on the host, by the platform's in-browser environment, or open decision O-02 is decided and recorded | SAFETY_AUTHORIZATION, section 3.4 |
| E-08 | beginner or easy difficulty by the platform's own label, so that one chain fits the 150-minute block. The plan forbids a two-medium-box quota; it does not make one medium machine possible in 150 minutes. ESTIMATE | plan, week 6: no quota; plan 1.2 |
| E-09 | no visible hint, task list or question set that would act as a hint inside the exercise. Guided rooms with task questions fail this check | R-03 |
| E-10 | one target, not a multi-host network | R-01 |

Registry candidates (the owner chooses; none is approved, none was opened by this file):

| Registry id | Name and operating system as recorded | Registry note on fit |
|---|---|---|
| `w06-htb-machine-nibbles` | Nibbles, Linux, retired page | paid tier; SEARCH_CHECKED |
| `w06-htb-machine-bashed` | Bashed, Linux, retired page | paid tier; SEARCH_CHECKED |
| `w06-htb-machine-cap` | Cap, Linux, retired page | paid tier; SEARCH_CHECKED |
| `w06-htb-machine-jerry` | Jerry, Windows, retired page | paid tier; SEARCH_CHECKED. Other retired pages seen in the same queries and named only in its notes: Legacy and Optimum (Windows), Lame and Shocker (Linux) |
| `w06-thm-blue`, `w06-thm-basic-pentesting` | TryHackMe beginner rooms | guided rooms with task questions: they fail E-09 and the registry itself calls them a weak fit; ineligible as an unknown target |

The registry states that the descriptions on the machine pages name the vulnerability class, so the page text is a leak. Read a candidate's page only for E-01 to E-10 and do not read its description; under protocol 1 the preparer does that reading.

### 5.4 The draw

| Step | Protocol 1 (another person) | Protocol 2 (you, from your fixed pool) |
|---|---|---|
| Who | a person who is not you, who holds the pool file and the rules | you |
| When | at the start of block C, 13:45 | at the start of block C, 13:45, not earlier |
| How | a physical die. Number the pool 1 to n. For n up to 6 use one die and roll again on a number above n; for more entries use a larger die | the same |
| Record | pool hash, method, the number rolled, the entry, the time in UTC and local time | the same, written before you open the entry's page |
| Handover | the preparer starts the instance if the platform allows it and gives you the target id and the address, and nothing else: no description, no hint | you open the entry only to start the instance. Do not read its description, tabs, comments or any write-up section. Write down what you could not avoid seeing |
| Redraw | once, only if a check E-01 to E-10 fails at handover, before your first action; the failed entry is struck from the pool; the number of redraws goes on the scorecard | the same. A redraw never happens because the target looks hard |

Protocol 2 has a residual leak that protocol 1 does not: the machine page may name the weakness class. State on the scorecard which protocol was used and what you saw.

### 5.5 If the environment fails

Time lost to a platform fault (the instance does not start, the browser environment is unreachable) is recorded with its times. If more than 20 minutes of the window are lost before the first action (ESTIMATE of a reasonable margin), the attempt is recorded BLOCKED for the environment, the drawn entry is struck from the pool and the scored practical is rescheduled as a new attempt linked to the first, with a fresh draw and no credited hours (INSTRUCTOR_GUIDE, section 9). A BLOCKED attempt is scored on what was demonstrated and is never described as passed.

## 6. The day, block by block

Times are America/Chicago (plan 1.2). The Friday clock is the plan's: 0.5 + 2 + 2 + 2.5 + 1 = 8 hours, with 15 minutes after block A and 60 minutes after block B, which are not study time. One assessment attempt is begun in the window about 08:25, before the knowledge exam (INSTRUCTOR_GUIDE, section 2).

### 6.1 Before the day, outside the counted hours

| When | Action | Output |
|---|---|---|
| by 2026-12-18 | fix the pool; check E-01 to E-10; read and date the platform terms; decide O-02 or filter the entries; choose protocol 1 or 2 and name the preparer | pool file; its hash on the scorecard |
| the evening before | open the scorecard row; create the evidence folder; check the recording and hashing commands of section 11 on a harmless file, not on a target; read no candidate page | a dry-run record |
| 08:00 on the day | no AI, no solution, no hint from this moment (R-03 to R-05) | none |

### 6.2 The timeline

The middle column is the plan's day 30 text. The third column is what this file adds.

| Local time | Block | Plan's text for day 30 | What you do | Output |
|---|---|---|---|---|
| 08:00 to 08:30 | R | the Friday recall block: the 1-day and 3-day questions | recall queue; not part of the attempt | recall log |
| about 08:25 | | | define the week 6 assessment in the window if it is not defined; begin the attempt | attempt start time |
| 08:30 to 10:30 | A | Midpoint theory exam and scope review | Part 1, the closed-book knowledge exam of 20 questions, in the first 90 minutes (ESTIMATE). No notes, no AI. Lock the answers: write them to a file, compute the SHA-256, write the hash and the time. Then about 30 minutes of scope review that does not need the target: the rules of section 3, the platform terms and tier check, the standing exclusions, the stop table of section 4.3, the generic part of the scope block and the RoE | locked answers with hash; generic scope block |
| 10:30 to 10:45 | break | | not study | |
| 10:45 to 12:45 | B | Review exam-style limitations; no walkthrough | first about 80 minutes (ESTIMATE): the diagnostic retest of section 10, closed book, locked by hash. Then about 40 minutes: review what the benchmark can and cannot show (no hints, one target, 150 minutes, no AI), what a partial result looks like (section 9), the evidence discipline and the report skeleton, and a dry run of your recording setup on a non-target. No walkthrough of any exercise, and no new-target exposure | locked retest answers; report skeleton ready |
| 12:45 to 13:45 | lunch | | not study | |
| 13:45 to 16:15 | C | Independent unknown target, full chain to meaningful verified result | the attack window of 150 minutes (section 6.3) | evidence bundle |
| 16:15 to 17:15 | D | Commercial-style report; midpoint deficiency register | section 6.4 | report, register, scorecard |

The retest has no block in the plan, which requires it on day 30 (plan 1.3). Placing it in block B takes time from the review that block B holds. See the departures table.

### 6.3 Block C: checkpoints

The checkpoints are PROPOSED planning aids. They are not rules and they do not change the score. The scored claim is the evidence, not the pace.

| Time | Minute | Checkpoint |
|---|---|---|
| 13:45 | 0 | handover and draw record (section 5.4); you receive the target id and address and nothing else |
| 13:45 to 14:00 | 0 to 15 | complete the scope block, confirm the target's identity from an independent source, sign the RoE, write the test plan. No target contact before this is done and timestamped (ESTIMATE: 15 minutes) |
| 14:30 | 45 | the enumeration inventory exists with a source and a time for each fact. This is also the 45-minute point of the hint ladder in a guided day; in a scored attempt no hint follows it |
| 15:15 | 90 | at least one falsifiable hypothesis has been tested with its predicted result recorded before the test |
| 15:45 | 120 | decide: continue the attack to 16:10, or switch to close-out (evidence index, the target's own logs if you have access, the negative controls, the partial-result statement) |
| 16:10 | 145 | last target contact; instance closed and anything you left removed; cleanup written down |
| 16:15 | 150 | window ends |

### 6.4 Block D: the 60 minutes

ESTIMATE of a split; the plan fixes the block, not its inside.

| Minutes | Work |
|---|---|
| 0 to 5 | hash the raw evidence bundle and write the time before you draft anything (the hash covers the raw records, so a later edit of the report cannot change them) |
| 5 to 40 | the report: the technical report Part A and the executive report, from the records you kept in block C |
| 40 to 55 | the midpoint deficiency register (section 7.3) |
| 55 to 60 | hash the final report and the bundle, submit the attempt in the window with the evidence, log the time |
| after 17:15 | key-based marking, banding and the application's grading may overflow; the overflow is recorded on the scorecard, is not credited as study, and no key is opened early (INSTRUCTOR_GUIDE, section 2) |

## 7. Deliverables

### 7.1 The evidence bundle

| # | Item | What it holds | P or E criterion it supports |
|---|---|---|---|
| 1 | scope block and RoE | section 4.1, timestamped before the first target command | P1 |
| 2 | test plan | the ordered hypotheses and what each test would show | P1, P3 |
| 3 | hypothesis log | per hypothesis: id; time written (zone); the hypothesis; the result predicted if it is true and if it is false; the test; the time run; the observed result; the verdict (supported, refuted, unresolved); the control; the next step. Each record is written before its test | P3 |
| 4 | command log | each command verbatim with its status word, target id, time with zone, and where its output is stored. A target column is mandatory | P1, P2, E1 |
| 5 | raw outputs | the unedited output of each command, saved as you go | P2, E2 |
| 6 | inventory | each fact of the target: the source (command, log, packet, file), the time with zone, the limits (what filtered or closed does not prove) | P2 |
| 7 | evidence index | each artifact: name, SHA-256, time saved, the claim it supports | E2 |
| 8 | hint and aids logs | the hint log is empty; the aids log lists tools with versions and any third-party code with its source; the solution-exposure statement says none or what | P3, E2 |
| 9 | cleanup record | the instance closed, what you left and removed, the time | P1, D4 |
| 10 | locked answers | the knowledge exam file and the diagnostic retest file, each with its SHA-256 and time | Part 1 |

Sample (synthetic): two lines of a hypothesis log, to show the discipline. Nothing was tested.

```text
H-03 | written 13:58 America/Chicago | service X on the assigned address accepts an anonymous bind | if true: the listing request returns entries without credentials; if false: it returns an authentication error | test: one listing request, target id <id> | run 14:07 | observed: authentication error (OBSERVED, output saved as <artifact id>) | verdict: refuted | control: the same request with a deliberately wrong credential returns the same error class | next: H-04
```

### 7.2 The commercial-style report

Commercial-style means the structure and sobriety of a report that a client's engineer and a client's manager could both act on. It does not mean marketing language, and it does not mean copying a vendor's sample. The business context of a lab target is modelled, so state it once in one sentence and label it ASSUMED; every business statement after it is INFERRED.

| Report part | Template | Rubric criteria it feeds |
|---|---|---|
| Executive summary for a decision-maker | `docs/curriculum/v2/TEMPLATES/EXECUTIVE_REPORT.md` | E1, E2, P4 |
| Scope, methodology, asset inventory, severity rationale, findings, limitations | `docs/curriculum/v2/TEMPLATES/TECHNICAL_REPORT.md`, Part A | P1 to P4, E1 to E3 |
| One finding record per root cause | `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md` | P3, P4, E1, E3 |
| The path from first access to the result, with each edge labelled | `docs/curriculum/v2/TEMPLATES/ATTACK_PATH_GRAPH.md`, when a path of more than one step exists | P3, P4 |
| The decision record for reducing false positives (plan, week 6 expansion): which candidate findings were discounted and why | a table in the technical report, Limitations | P2, P4 |
| Defensive analysis of the one significant issue | the technical report, Part C, labelled paper analysis where nothing was applied | D1 to D4, E3 |

Rules that bind the report:

- Severity is argued from preconditions and impact, never copied from a scanner or from a score (plan, week 6 expansion). If you quote a CVSS score, write its version and its full vector, which metrics you chose and why, and label it your own calculation.
- Unconfirmed items and false positives have their own blocks. A finding you could not reproduce is Unconfirmed.
- No flag value, no credential found on the target, no screenshot and no step order leaves the private folder. `W06-Midpoint-Penetration-Test.md` is the plan's name for the week's output; its portfolio copy follows PORTFOLIO sections 5.3 and 6, and the original stays private.
- The report is made from the records of block C. Do not copy report text from any sample, including the sample report that a platform's reporting module ships (`w06-htb-documentation-reporting`, SEARCH_CHECKED, not read).
- A statement that the target was or was not "secure" is not made. Write "no issue found in the tests run" and name the tests.

### 7.3 The midpoint deficiency register

A register of what the evidence shows you could not yet do, so that weeks 7 to 12 target it. It is a working document, not a confession and not a score.

| Column | Content |
|---|---|
| ID | `DEF-01` and so on |
| Domain | the competency area (Linux, networking, Windows and AD, web, detection, DFIR, reverse engineering, cloud, AI, reporting, method) |
| Deficiency | one observed statement: what happened and what you could not do |
| Evidence | the artifact id and the time that show it; a deficiency with no artifact is not entered |
| Class | K knowledge, M method (planning, hypothesis, validation), T tool, E evidence discipline, C time control, V environment or platform |
| What the evidence does not show | the limit of the statement; whether the cause is knowledge or circumstance is not assumed |
| Effect | the sub-criterion points it cost, if known |
| Remedy | the optional work of the next weeks that addresses it: the day guides' expert extensions, a week accelerator, a repeated exercise. Required labs, block C and Friday assessments are never substituted and hours are never added (INSTRUCTOR_GUIDE, section 9) |
| Retest | the earliest day on which a matching exercise tests it again |
| Status | OPEN, or CLOSED with the evidence of the retest |

Rules: enter only what an artifact shows; do not enter a deficiency that the evidence shows you did not have; do not enter blame or intentions; carry the diagnostic retest's domain shares as context (section 10). The register is checked for traceability and is not separately scored. A register that contradicts the report's stated limits lowers E2 (precision and provenance).

Sample (synthetic): two rows, invented to show the format.

```text
DEF-01 | networking | the inventory lists three services and no cross-check against a second source | evidence: inventory item 4, no matching second-source record | class: E | not shown: whether a second tool would have disagreed | effect: P2 verification point | remedy: record a second observation for each key fact in the next three block C records | retest: day 33 block C | status OPEN
DEF-02 | method | hypothesis H-05 was written after its test (log times 14:41 and 14:38) | evidence: hypothesis log rows H-05 | class: M | not shown: whether the prediction would have changed | effect: P3 prediction point | remedy: write the prediction before each test in weeks 7 and 8 | retest: day 35 | status OPEN
```

## 8. How it is scored

The Friday rubric applies unchanged (`docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`). Gates first: record any safety or integrity failure before adding a point. Then the four parts.

| Part | Points | What is scored on day 30 | Evidence read |
|---|---:|---|---|
| 1 Knowledge | 20 | the midpoint theory exam of 20 questions, closed book (week 6 `ASSESSMENT.md`, Part 1; its key is instructor material) | the locked answer file against the key |
| 2 Independent practical | 40 | P1 scope and design 5; P2 enumeration and evidence fidelity 10; P3 independent hypothesis and reproduction 15; P4 impact and root-cause reasoning 10, all on the unknown target | scope block, test plan, inventory, hypothesis log, command log, runs, cause statement |
| 3 Defensive validation | 25 | type F: D1 data source and hypothesis 8; D2 positive validation 7; D3 negative testing 5; D4 hardening and mitigation 5, for the one significant issue | the target's own logs if you reached them; the mitigation text; a retest where one was applied |
| 4 Evidence and reporting | 15 | E1 reproducibility 5; E2 precision and provenance 5; E3 remediation and retest 5 | the report, the hash record, the artifact table |

Two layers decide the week (INSTRUCTOR_GUIDE, section 3): the Friday score of at least 80 with both gates passed, and the week conditions (one significant issue independently verified; one privilege boundary demonstrated or assessed; a report with root cause and risk). The application records its own, stricter decision beside it (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 10). The two are never averaged.

### 8.1 What each outcome can reach

These are ceilings read from the rubric's bands, so that nothing about scoring is a surprise. They are INFERRED from GRADING_RUBRICS, sections 3, 5, 6, 7 and 8.3, and they are neither promises nor extra rules. The key in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/MIDPOINT_KEY.md` states the sub-criterion rules.

Outcome states used here:

- Solved: a chain from first access to a meaningful verified result, each step with a reproduction record. Meaningful verified result (PROPOSED definition): a state that crosses a privilege boundary on the target, shown by two independent artifacts, such as the identity before and after, with times. A flag string is not the evidence.
- Partly solved: at least one significant issue independently verified, with its negative control, and the chain not completed.
- Not solved: no issue verified. Enumeration and hypotheses only.

| Part | Solved, or partly solved | Not solved |
|---|---|---|
| 1 Knowledge | 20 | 20 |
| 2 Independent practical | up to 40 | up to 31 (P1 5, P2 10, P3 11, P4 5) |
| 3 Defensive validation, form H, paper analysis only | up to 18 (D1 8, D2 3, D3 2, D4 5) | up to 14 (D1 6, D2 3, D3 2, D4 3) |
| 3 Defensive validation, a fix applied and retested on a dedicated instance where the terms allow, or form L | up to 25 | up to 14 |
| 4 Evidence and reporting, paper-only remediation | up to 14 (E3 4) | up to 12 (E3 2) |
| 4 Evidence and reporting, with a retest run | up to 15 | up to 12 |
| Total | up to 92 on paper, up to 100 with an applied fix | up to 77 |

What follows from the table, stated plainly:

1. A hosted target with paper-only defensive analysis leaves a margin of 12 points to the aim of 80. Part 1 and the practical must be near their ceilings.
2. An unsolved attempt cannot reach 80 as the rubric reads, and the week 6 gate is not met either. That is the correct outcome of honest partial work (plan 9.2), recorded as NOT MET without a gate failure. It is not a reason to claim success you did not obtain.
3. A solved attempt that carries any gate failure is not a passed practical, whatever its total.

## 9. When the target is not solved at the time limit

Plan W06-Q08 asks what a student does when an unknown target remains unsolved at the time limit. The answer is to document the partial result honestly and to avoid inventing success. Do this:

| Step | What to do |
|---|---|
| 1 | stop at 16:10. Close the instance. Remove anything you left. Write the cleanup. Do not use the time to try "one more thing" outside the window |
| 2 | classify the state: Solved, Partly solved or Not solved (section 8.1). Say which, in the first line of the report |
| 3 | give the enumeration inventory as it stands: each fact with its source and time, the limits of each, and the states you could not resolve (filtered is not absent) |
| 4 | give the hypothesis table: for each hypothesis the prediction, the test, the result and the verdict (supported, refuted, unresolved). Refuted hypotheses count as work |
| 5 | state the blocker in one of three kinds: a missing technical step that you could not find, an environment or platform fault, or time. Do not blur them |
| 6 | give the next discriminating tests in the order you would run them, each with what it would show. These are NOT RUN. They are not findings |
| 7 | write the state of each candidate finding as Unconfirmed, with what would confirm or refute it. Do not rate a lead above the evidence |
| 8 | list what the evidence does not show |
| 9 | open the deficiency register from the blocker and the hypotheses |
| 10 | do not read a solution before you submit. After you submit you may; if you do, declare it on the scorecard: the target is then contaminated and cannot serve as an unseen target again |

Do not write: that access was obtained when it was not; an inferred credential as a found one; "partial success" wording that implies access; an output you did not save; a screenshot or output reconstructed from memory and presented as captured; or a finding upgraded from Unconfirmed to Confirmed because the time ran out.

How the demonstrated part is scored: accurate enumeration and hypotheses earn P1, P2 and the hypothesis part of P3; reproduction and root cause score what exists; the defensive part scores the data source you can name, labelled as unverified where you could not look (GRADING_RUBRICS, section 8.3). An environment fault is scored as demonstrated work and recorded BLOCKED for the part that needed it. A failed Friday is not repeated; an optional new attempt with a fresh draw may be recorded and credits no hours (INSTRUCTOR_GUIDE, section 9).

## 10. The diagnostic retest

The plan keeps the entry diagnostic on day 1 and retests it at the midpoint and at the final (plan 1.3). It is a placement tool and an ungraded measure of change. It is not part of the 100 points and it is not a claim of mastery (plan 9.2).

| Item | Rule |
|---|---|
| What | the 30 questions E01 to E30 of `docs/curriculum/v2/ASSESSMENTS/ENTRY_DIAGNOSTIC.md`, or the parallel form that file designates for the retest. If it designates none, the same 30 are used and the scorecard says that the day 1 key was open to you after day 1, so growth is overstated by an unknown amount (INFERRED) |
| A second contamination to know about | the plan file `docs/curriculum/v2/MASTER_PLAN_V2.0.md` holds the diagnostic's expected answers in its section 8.1. If you have read them, the retest measures recall of them. State on the scorecard whether you did |
| When | block B, the first 80 minutes (ESTIMATE: 20 concise responses at about 2 minutes and 10 applied items at about 4) |
| How | closed book; no notes, no AI; written answers; locked at the end by SHA-256 and time |
| Scoring | after the attempt is submitted, against the diagnostic's instructor key in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/`: 1, 0.5 or 0 per question, then the share per domain, using the domain grouping of `docs/curriculum/v2/STUDENT_GUIDE.md`, section 9 |
| Use | compare each domain with day 1; re-place the domain on its track (below 70 percent Guided, 70 to 89 Independent labs, 90 or more with an independent practical pass Expert extension). Today's practical is the independent practical evidence for the domains it touched. The deficiency register carries the result as context |
| The practical baseline | if the entry diagnostic defines a non-invasive practical baseline and a retest of it, follow that file. The midpoint practical is also evidence |

## 11. Commands proposed for review

Proposed text only. None was run. Each needs your review and your approval of the exact text first (STUDENT_GUIDE, section 4). Where the exercise is browser-only, the recording command is not available there and you keep the command log by hand with a time on every line.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `date -u +%Y-%m-%dT%H:%M:%SZ` | the clock of the machine on which you record evidence; record the platform environment's clock too, because the two can differ | write a UTC time beside each hash, the draw, the handover and the last contact | one line in the form year-month-day, the letter T, hours:minutes:seconds, the letter Z; a reading of that clock, not an observation of its accuracy | none | ordinary user | the machine you record on | none needed (read-only) |
| 2 | `sha256sum <file> [<file> ...]` | the files you name: the pool file, the locked answer files, the evidence bundle files, the report | record a SHA-256 before a key is opened and when a commitment is made | one line per file: 64 hexadecimal digits, then the file name | none | ordinary user | files you created in your own folder | none needed (read-only) |
| 3 | `sha256sum -c <manifest>` | a manifest file that lists names and hashes | confirm that files still match the hashes you recorded | one line per file ending in OK or FAILED, and a summary line if any failed | none | ordinary user | your evidence folder | none needed (read-only) |
| 4 | `script -q -a <log-file>` | an interactive shell in the platform's in-browser environment, if the exercise provides one | record the raw terminal session as evidence | no output of its own; the log file grows until the shell exits | writes a log that holds everything displayed, which may include credentials shown by the platform; handle it as evidence and never publish it | ordinary user in that shell | the platform's environment for the assigned instance only | end the recording by exiting the inner shell; keep the file under the evidence rules; do not delete it before the attempt is closed |

## 12. Registry pointers

Version-sensitive facts are stated as ASSUMED with the registry pointer `docs/curriculum/v2/RESOURCE_REGISTRY.md`, unless the week registry already confirms them. SEARCH_CHECKED is the strongest label the registry schema allows; it means a search result was seen and the page was not opened.

| Registry id (in `docs/curriculum/v2/WEEKS/W06/RESOURCES.json`) | Use in this benchmark | Label there |
|---|---|---|
| `w06-nist-sp-800-115` | the rules-of-engagement template and the plan, execute, post-execute structure behind the scope block | SEARCH_CHECKED |
| `w06-first-cvss-v4-specification` | one input to the severity rationale, never the rating itself | SEARCH_CHECKED |
| `w06-nvd`, `w06-cwe` | CVE preconditions and the root-cause vocabulary; a version match is a lead, not an impact | ASSUMED |
| `w06-mitre-attack` | mapping of observed behavior to telemetry; record the version you used | ASSUMED; the release current in 2026-11 is not confirmed |
| `w06-htb-documentation-reporting` | report structure; do not copy its sample | SEARCH_CHECKED |
| `w06-htb-machine-nibbles`, `-bashed`, `-cap`, `-jerry` | pool candidates, section 5.3 | SEARCH_CHECKED; paid tier |
| `w06-htb-cpts-certification`, `w06-offsec-oscp-exam-guide`, `w06-offsec-oscp-scoring-faq` | context only: exam rules differ from ordinary lab use and are reread live before any booking (plan 13) | SEARCH_CHECKED; the registry did not confirm the AI restriction on OSCP+ exams |

## 13. Departures from the plan and open points

| What | Why |
|---|---|
| The diagnostic retest sits in block B | plan 1.3 requires it on day 30 and gives it no block; block B holds only a review. PROPOSED placement |
| The scope review in block A is generic and the target-specific scope step is the first 15 minutes of block C | the target is drawn at 13:45 so that a morning of knowledge of its name cannot turn into research (R-05) |
| The draw is at the start of block C, not earlier | INSTRUCTOR_GUIDE, section 8.1: handover at the start of the clock only |
| The raw evidence bundle is hashed at the start of block D, before the report | stricter than the guide's order; the hash then covers what the report must not change |
| Part 3 of a hosted target is mostly paper analysis, with ceilings in section 8.1 | the plan's day 30 row has no defensive task; its Friday score has 25 defensive points; a hosted retired machine has no telemetry or control the learner can change unless the instance is dedicated. The structure costs up to 7 points on D2 and D3 and one on E3 unless a fix is applied and retested. Open point OP-MP-02 |
| An unsolved attempt cannot reach 80 | the plan sets 80 as an aim (9.2) and as the weekly gate (1.3), and asks that partial evidence be graded and not inflated |
| "Meaningful verified result" is defined | the plan names it without a definition. PROPOSED: a privilege boundary crossed on the target, shown by two independent artifacts |
| Guided rooms are ineligible | their task questions act as hints (R-03); the week 6 registry calls them a weak fit |
| Retired Hack The Box machines need a paid tier | recorded by the week 6 registry, SEARCH_CHECKED; the plan's pool of "retired or beginner content" is not free. ASSUMED |
| Form L (a local simulated corporate finding) is offered as an alternative | plan 0.3 allows a simulated corporate finding; it removes the Part 3 ceiling but needs Stage 00 resolved and another person to prepare it |
| Day 30 falls on 2026-12-25 | the plan keeps planned training days on 2026-12-25 and 2027-01-01 and allows shifting without deleting hours |

| ID | Open point | Default until decided |
|---|---|---|
| OP-MP-01 | whether the owner holds a platform tier that yields at least 5 eligible candidates | if not: the benchmark is BLOCKED with diagnostics, or form L when Stage 00 is resolved |
| OP-MP-02 | whether to accept paper-only Part 3 on a hosted target, or to add a small local fixture exercise earlier in week 6 for the defensive part (plan 9.3 allows an unseen practical to be modified; the instructor guide proposes a pre-built local fixture, section 8.2) | paper analysis, ceilings as in section 8.1 |
| OP-MP-03 | open decision O-02 (platform VPN profile) | browser-based environments only |
| OP-MP-04 | a second reader for the report and the scoring, recorded by name on the scorecard | none; the grade is self-adjudicated |
| OP-MP-05 | which form of the diagnostic the retest uses | the same 30 items, with the contamination stated |
