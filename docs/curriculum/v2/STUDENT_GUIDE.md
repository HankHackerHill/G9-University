# G9 Cybersecurity Mastery v2.0: student guide

- Reader: the owner, as the learner. How a Friday is scored, how keys are released and how an unseen practical is prepared are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`. This guide holds no answer, no key and no fixture map of any item.
- Source of requirements: plan sections 0.4, 1.2, 1.3, 9, 10.2, 10.3, 10.4, 12 and 13 of `docs/curriculum/v2/MASTER_PLAN_V2.0.md`.
- Status: PROPOSED, written 2026-11-18. Nothing in it was run, deployed, scanned or tested on the G9 host. Every command it mentions is proposed text. Example text is marked `Sample (synthetic):` and records no observation.
- Stage 00 (the libvirt socket, plan section 2) is unresolved: see `docs/curriculum/v2/README.md`. Until it is resolved no local lab exists, and section 12 below says what to do with a blocked lab.

## 1. The method on one page

The loop of every day (plan section 1): scope, hypothesis, reproduction on an authorized target, preserved evidence, root cause, telemetry, mitigation, retest, documented limits.

| Principle | What you do | Where in the day | Plan |
|---|---|---|---|
| Retrieval before review | answer closed book, in writing, before you look at a note or a key | block R, every day | 1.2, 10.4 |
| Worked example, then faded guidance, then an independent test | block A shows the mechanism with a guided lab, block B fades the scaffold, block C removes it on an unseen or modified task | A, B, C | 10.4 |
| Dual evidence | a tool's output plus a second independent observation of the same fact (a socket listing next to a scan; a packet next to a log line) | B, C, D | 10.4 |
| Interrogation | in every lab record answer three questions: what actually made this possible, how would I detect it, what would disprove it | D | 10.4 |
| Spacing | the recall questions of each day return after 1, 3, 7 and 14 days | R | 1.3, 10.4 |
| Interleaving | a small scenario from an earlier topic, inside the existing hours | R, D | 10.4 |
| Productive effort before help | 45 minutes of your own hypotheses before the first hint | C | 1.3 |
| No productivity theater | one concise record per substantial lab; long notes do not replace an independent reproduction | D | 10.4 |
| Learning is not assessment | a flag from a guided lab is not evidence of competence on an unseen target; no grade comes from an attempt count or from tool output | Fridays, midpoint, final | 1.3, 9.1 |
| Review before run | no command runs before its row has been reviewed and approved by you | every block with a command | 0.2, 0.4 |

## 2. The day to the minute

Times are America/Chicago (plan section 1.2). The study accounting is 0.5 + 2.0 + 2.0 + 2.5 + 1.0 = 8.0 hours a day, 40 a week, 480 over 60 weekdays. The two pauses are not study and are not counted.

| Local time | Counted | Code | Block | What it is |
|---|---:|---|---|---|
| 08:00 to 08:30 | 0.5 h | R | Active recall | prior questions, prior mistakes, flashcards |
| 08:30 to 10:30 | 2.0 h | A | Core lesson and guided reproduction | the mechanism, the canonical reading, a guided lab |
| 10:30 to 10:45 | 0 | break | Break | not study |
| 10:45 to 12:45 | 2.0 h | B | Tool mastery, coding, lab preparation | tools and code against independent ground truth |
| 12:45 to 13:45 | 0 | lunch | Lunch | not study |
| 13:45 to 16:15 | 2.5 h | C | Unassisted challenge or investigation | an unseen or modified task, no answer in your copy |
| 16:15 to 17:15 | 1.0 h | D | Defense, evidence, remediation, retest, recall | what the telemetry shows, the fix, the retest, the record |
| 08:00 to 17:15 | 8.0 h | | One day | 75 minutes of pauses inside a 9 h 15 min span |

Friday uses the same clock and different contents (plan section 5, and ADR-0023 decision 2):

| Local time | Block | Friday content |
|---|---|---|
| 08:00 to 08:30 | R | recall of the 1-day and 3-day questions |
| 08:30 to 10:30 | A | closed-book knowledge exam, 20 questions. The assessment attempt in the window is begun before this block |
| 10:45 to 12:45 | B | the plan's review or planning task. Where the plan says "after answers are submitted", lock your exam answers first (section 3) |
| 13:45 to 16:15 | C | scored practical: scope, enumeration, hypothesis, reproduction, defensive validation. No hints, no AI |
| 16:15 to 17:15 | D | evidence and report, then the attempt is submitted, then scoring |

Day 55 is the one exception: block A is a lesson and the exam sits in block D (plan section 5, week 11). PROPOSED for that day: begin the assessment attempt before block C so that the scored practical and the exam are both inside it, and lock the exam answers at the end of block D. Days 30 and 60 are the midpoint and the final; their rules are in section 13.

Rules for the clock:

- The pauses are fixed. Short movement breaks fit inside a block and do not extend it.
- In the window, a day starts at the time chosen in Program setup. 08:00 reproduces this table; another time moves every block by the same amount and keeps both pauses (15 minutes after the second block, 60 minutes after the third).
- The dates in the plan are the original planning calendar. Missed days are not presumed done. You may shift dates and keep the sequence. 2026-12-25 and 2027-01-01 are planned training days; shift them if you must, never delete their hours.
- Study time is what the Session Tracker credited from the backend's monotonic clock. Time the backend cannot vouch for (a suspend, a crash) is not credited until you correct it. A wall-clock subtraction is not elapsed time.
- Instructor preparation (key release, source checks, preparing an unseen practical) is outside the 480 counted hours and is not credited as study. The plan allots it no time: see `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, open points.

## 3. What to do in each block

| Block | Purpose | Do | Leave with |
|---|---|---|---|
| R, 30 min | retrieve before you look | 1. Answer the generated queue of the day closed book, in writing (section 5). 2. Check each answer against the key of the question's origin day and mark it accurate, partial or missed. 3. If the queue is short, do the warm-up tasks that block R of the day guide gives. 4. Explain one failed experiment of an earlier day in your own words. PROPOSED split, an estimate: 20 minutes for 1 to 3, 10 for 4 | a recall log line per question: identifier, result, one-line cause of a miss; one paragraph on the failed experiment |
| A, 120 min | understand the mechanism and see it work | 1. Read the day's canonical sources (its Resources table; cite registry ids, not addresses from memory). 2. Follow the worked explanation. 3. Before each guided step write what you expect to observe. 4. Do the guided lab. 5. Work through each common wrong model with its discriminating test. A command is reviewed first (section 4) | for each objective, its success criterion restated in your words; the guided lab's observations next to your predictions |
| B, 120 min | tool and code mastery against ground truth | 1. Check the prerequisites and whether the work is on a guest or on the host. The trusted G9 host is never a target (plan section 3.2). 2. Build or run the tool step by step. 3. Validate every result against an independent source (plan section 10.1). 4. Walk the error branches the day guide lists | a validated result with its second observation; the error branches you actually met, if any |
| C, 150 min | show you can do it without the scaffold | 1. Read the task statement and write the scope (targets, ids, allowed actions, exclusions, stop rule) into your record before any action. 2. Work alone for 45 minutes (section 6). 3. State a hypothesis and its falsifier before each test. 4. Keep the negative control. 5. Stop at 150 minutes and save the hand-in with its hash | the saved hand-in; the hint log; the evidence bundle |
| D, 60 min | defense, evidence, retest | 1. Inspect the telemetry the exercise should have produced. 2. Name the control. 3. Apply or propose the remediation (apply only where the lab was approved). 4. Run the positive and the negative retest. 5. Write the lab record (section 10). 6. Answer the day's eight questions closed book. 7. Add new flashcards. PROPOSED split, an estimate: 30 minutes for 1 to 4, 15 for 5, 15 for 6 and 7 | the lab record; the quiz answers; flashcards |

Locking exam answers: on a Friday write the knowledge answers to a file, compute its SHA-256 and write the hash and the time in your record at the end of block A. Any later edit goes into a separate dated addendum that is not scored. This keeps block B and the key release from changing what you answered.

## 4. Commands: review before run

Every command in a day guide is proposed text. Nothing in this course says that running one is authorized. The row of the day guide's table "Commands proposed for review" gives its target, purpose, expected output, side effects, privilege, safe scope and rollback. Before you run one, check each item below and write the result in the lab record.

| Check | Question | If the answer is no |
|---|---|---|
| Target | is the target in the written scope, named by its id and address, and is it an owned, intentionally vulnerable, platform-designated or explicitly authorized system? | do not run it |
| Purpose | can I say in one sentence what question this command answers? | do not run it |
| Expectation | did I write the expected output before running it? | write it first |
| Side effects | can I name every change it makes, and are they acceptable? | do not run it |
| Privilege | does it need elevated privilege, and does the row say why? | run nothing with elevated privilege that the row does not require |
| Safe scope | is it on a guest, never on the trusted G9 host, and never on a LAN, a household network or the public Internet? | do not run it |
| Rollback | do I know how to undo it, and would the undo work if the command half-ran? | write the undo first |
| Gate state | is the lab it needs built and are its isolation tests passed (plan section 3.3)? | with Stage 00 unresolved the answer is no: no intrusive local command (section 12) |
| Approval | is my approval recorded with a date, a time and the exact command text? | record it |

Status words for a command in your record: PROPOSED (written, not reviewed), REVIEWED (checked against the table), APPROVED (you approved the exact text), EXECUTED (run, with the time and what actually happened), NOT RUN. A result is OBSERVED only after EXECUTED. A hosted exercise on a platform is run in a browser under that platform's own terms and the scope written for it.

## 5. Active recall (block R)

- Retrieve first. Write each answer from memory before you open a note, a key or a reference. An answer written after reading is a copy, not a retrieval.
- Mark with the key of the question's origin day. Open that key only after your answer is written. Use its marking line: points, what is accepted, what is rejected as an overclaim. Note the cause of a miss: missing fact, wrong model, wrong limit, or misread question.
- A wrong model is the useful miss. Write the discriminating test that would have separated your answer from the right one.
- Failed experiment: choose one lab or test of an earlier day that did not give the expected result. Write what you did, what you expected, what happened, what you now think explains the difference, and what you could run to check that explanation. Do not write what the answer key says; write what you think.
- Flashcards: one question per card, written in your own words, no copied vendor text. A card holds a mechanism or a limit, not a tool name. Keep them in a local file, in the Retention Center of the window (it keeps its own ladder of 1, 3, 7, 21 and 60 days), or in the local spaced-repetition application the plan lists. Installing that application is your decision; the course needs none. Do not put the same card in two systems unless you want it asked twice.
- Interleaving: if the queue gives you an earlier topic, answer it as a scenario, not as a definition.

## 6. The 45-minute rule and the four-step hint ladder

The rule (plan section 1.3): before the first hint, spend 45 minutes of your own hypothesis work and struggle on the independent challenge. A full solution is never the first response.

| Step | Name | Opens when | What it may contain | What you record |
|---|---|---|---|---|
| 1 | No-hint baseline | at the start of block C | nothing: the day guide says what to try alone | the hypotheses you formed, the tests you ran, what each showed, the minute you stopped |
| 2 | Conceptual hint | after 45 minutes of step 1, with your notes up to date | a small conceptual hint: which mechanism or trust boundary to look at, not where in the target | the minute you took it; what you tried since step 1 |
| 3 | Scoped technical hint | after you worked on step 2 and wrote down what it changed. PROPOSED pacing, not in the plan: at least 15 minutes of further work after step 2 | a narrow technical hint about one component or one test | the minute you took it; what you tried since step 2 |
| 4 | Instructor explanation | after the attempt is submitted: the C hand-in is saved and hashed. It is in the answer key of the day | the reference observations and the explanation | the minute you opened it; the differences between your result and the reference |

Rules:

- Take the steps in order. Never skip one.
- Environment trouble is not struggle time. If a guest does not boot or a route is missing, pause the 45 minutes, record the cause, and follow section 12.
- A result reached with step 2 or 3 is a guided result. In the application's competency model, hinted application counts as guided evidence, not independent evidence (`docs/adr/ADR-0005-competency-state-model.md`). Say so in the record.
- The scored attempts have no hint ladder: Fridays (plan section 9.1), the midpoint and the final. No hint, no key, no published solution.
- A published walkthrough of a third-party lab is a step-4 resource. Opening one before you submitted is a hint of the highest level: record it, and the objective is guided at best. In a scored attempt it is an evidence-integrity failure unless acknowledged (plan section 9.4; `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`). Never choose a challenge whose complete solution you already read (plan, week 12).
- The key of the day is opened only after your eight quiz answers are written and the hand-in is saved.

## 7. The command vault

The vault is your own reference of commands and the reasons behind them (plan section 1.3). It is a private file, not part of the course source.

Rules:

- Write each entry in your own words. Do not paste vendor course text.
- Write it from memory first, then check it against the tool's own documentation and fix it.
- A command is text. The entry says whether it was only reviewed or was executed, on which approved target, at what time. Never record an output as observed unless you observed it.
- Categorize every entry on five axes, so a failure or a detection can be found from a command and the reverse: protocol, service, failure mode, detection, remediation.
- No real credential, host, organization or personal data. Use synthetic placeholders such as `<synthetic-password>`.
- Review the vault on Fridays in block B and delete entries you can no longer explain.

Entry format:

```
Command:        <exact text, one line>
Mechanism:      <what the tool does to the system or the network, in your words>
Protocol:       <ARP | ICMP | TCP | DNS | HTTP | TLS | SSH | SMB | LDAP | Kerberos | other>
Service:        <what it talks to or inspects>
Preconditions:  <privilege, state, scope, what must already be true>
Evidence made:  <what it produces and where it lands>
Failure modes:  <how it fails or misleads, and how to tell>
Detection:      <which log, event or sensor shows it, and what is not shown>
Remediation:    <what removes the weakness it exposes, and how to retest>
Status:         <PROPOSED | REVIEWED | EXECUTED on <approved target> at <time> | NOT RUN>
Source:         <the tool's own documentation, by registry id>
```

Sample (synthetic): one entry, to show the depth. The output lines are invented for the sample.

```
Command:        ss -tulpn
Mechanism:      lists sockets the kernel tracks: listening TCP and UDP endpoints with the owning process
Protocol:       TCP, UDP (local socket table, no packets sent)
Service:        any local listener
Preconditions:  none for the table; process names of other users need elevated privilege
Evidence made:  a listing of local address, port and process
Failure modes:  without privilege the Process column omits other users' processes; a listener on
                127.0.0.1 is not reachable from another host, a listener on 0.0.0.0 is only reachable
                if a firewall allows it
Detection:      nothing leaves the host; a host audit rule on the command is the only trace
Remediation:    bind a service to the interface it needs; remove the service; add a firewall rule
Status:         NOT RUN
Source:         <registry id of the system manual>

Sample (synthetic):
tcp  LISTEN 0 128 0.0.0.0:22  0.0.0.0:*  users:(("sshd",pid=801,fd=3))
tcp  LISTEN 0 128 127.0.0.1:8080  0.0.0.0:*
```

## 8. Spaced recall: 1, 3, 7 and 14 days, and the generated queue

The plan's offsets are 1, 3, 7 and 14 days after exposure, with an optional consolidation at 28 days where the sequence allows (plan sections 1.3 and 10.4).

How the queue is made (read from `scripts/build_curriculum_v2.py::queue_day` and `scripts/build_curriculum_v2.py::recall_queues`; INFERRED from the source, not run):

- Each day guide ends with a Spaced recall section that tags its five recall questions: two at 1 day, one at 3, one at 7, one at 14.
- The builder places a question in the recall block of the first training day on or after the date of its day plus the offset in calendar days, on the plan's calendar (week 1 starts Monday 2026-11-16).
- A question whose date falls after day 60 is listed as post-course review.
- You see the queue in the recall block of the day in the window, and in the generated `docs/curriculum/v2/ASSESSMENTS/SPACED_RECALL.md`. If a day has no due question, use the warm-up tasks in block R of the day guide.

Worked calculation from the rule above (derived, not observed): the questions of day 1 (Monday 2026-11-16) fall due on day 2 (+1), day 4 (+3, Thursday 11-19), day 6 (+7, Monday 11-23) and day 11 (+14, Monday 11-30). The questions of day 5 (Friday 11-20) fall due on day 6 for +1 and +3 (the +1 date is a Saturday, so the first training day on or after it is Monday), day 10 for +7 and day 15 for +14. A Monday therefore carries the 1-day and 3-day questions of the previous Friday.

Limits:

- The queue is attached to the day number on the plan's calendar. If you pause or shift days, the real interval between exposure and recall stretches. INFERRED. Do overdue questions at the first recall block you reach; do not skip a recall block to catch up on a lab.
- The queue is fixed in the courseware. A question you missed twice is not re-queued by it. Re-queue it yourself as a flashcard, and consider a 28-day check for the ones you missed twice.
- The application's review engine (1, 3, 7, 21 and 60 days) is a separate mechanism for items you create in the application. It does not read the queue. Loading the course's questions into it is PROPOSED, not built (ADR-0023).

## 9. The adaptive rule by baseline score

The entry diagnostic is taken on day 1, retested at the midpoint (day 30) and at the final (day 60) (plan section 1.3). It is an ungraded placement, not a claim of incompetence or of mastery (plan section 9.2).

| Score on a domain | Track | What changes | What does not change |
|---|---|---|---|
| below 70% | Guided | do every guided lesson of the domain's weeks as written: all readings of block A, all steps of block B | block C is always independent; the hours; the Friday assessment; the gates |
| 70% to 89% | Independent labs | keep every independent lab and challenge; reduce reading you can show you already hold: write the mechanism claims of the reading from memory in 5 minutes, and skip the reading only if they are right and the objective's success criterion is already met | the hours; block C; the Friday assessment; the gates |
| 90% or more and an independent practical pass in that domain | Expert extension | replace the redundant introduction of block A with the day's optional expert extension and the week's accelerator | the hours; block C; block D; the Friday assessment; the gates. The extension is optional work and never becomes required |

The hours are 8 a day on every track. A track decides what you do with the time, never how long the course is. Failing topics substitute optional work of the next week; scheduled hours are never expanded and completion is never falsified (plan section 1.3).

Which questions form a domain is not defined by the plan. PROPOSED grouping of the 30 questions of plan section 7.1 (the question texts and keys are in the entry diagnostic and its instructor key, not here):

| Domain | Questions | Count | Weeks it steers |
|---|---|---:|---|
| Linux and virtualization | E01 to E04, E21, E29 | 6 | 1, 6, 9 |
| Networking | E05 to E09, E22 | 6 | 2, 6 |
| Python | E10, E11 | 2 | 1, 2 |
| Windows and Active Directory | E12 to E15, E23 | 5 | 3, 6, 10 |
| Web and API | E16 to E18, E24 | 4 | 4, 5 |
| Detection and DFIR | E19, E20, E25 | 3 | 7, 8 |
| Reverse engineering and reporting | E26, E30 | 2 | 6, 9, 12 |
| Cloud and AI | E27, E28 | 2 | 11, 12 |

How to apply it:

- Score each question 0, 0.5 or 1 against the key's marking points (PROPOSED), then compute the share per domain.
- The groups are small. One question moves a two-question domain by 50 points and a three-question domain by 33 points. Read the questions you missed, not only the percentage.
- "Independent practical pass" is not defined by the plan. PROPOSED: a scored practical in that domain (the day-1 practical baseline, a Friday or the midpoint) that reached its category minimum (practical part at least 30 of 40, plan section 9.1). A domain with no practical evidence cannot be placed in the top track.
- Re-place a domain at the midpoint and at the final diagnostics. PROPOSED: two consecutive Fridays with the matching category below its diagnostic target move the domain back to the Guided track until a Friday meets it.

## 10. The lab record

Keep one concise record per substantial lab. Writing more is not a substitute for reproducing the result independently (plan section 10.4). The template is `docs/curriculum/v2/TEMPLATES/LAB_RECORD.md`; it follows plan section 11.1. Its fields, and what each must hold:

| Field | What to write |
|---|---|
| Lab id and title; date | the day id and the exercise name; the date with the zone |
| Module and approved target | the registry id of the exercise and the exact target id and address in scope |
| Status | PROPOSED (planned), TESTED (run, result observed once), FAILED (run, did not give the expected result), VERIFIED (observed and confirmed by a second independent observation, evidence attached) |
| Authorized scope and exclusions | what is in, what is out, the stop rule |
| OS, version, network conditions | versions as read from the system; whether isolation tests passed (state NOT RUN if not) |
| Objective and preconditions | what you set out to learn and what had to be true |
| Hypothesis | one falsifiable sentence and what would disprove it |
| Commands or steps | the exact text and its status word from section 4 |
| Actual observations | each with a time, a zone, a clock source and the excerpt that supports it |
| Root cause and trust boundary | what made it possible, at the control, not the symptom |
| Impact and realistic limits | what the evidence supports and what it does not |
| Defensive signals and telemetry gaps | what was logged, where, and what was not |
| Remediation or compensating control | the change that removes the cause |
| Positive retest and negative retest | the permitted behavior still works; the forbidden behavior fails |
| Recovery and cleanup | what was reversed and how you know |
| Confidence | OBSERVED, INFERRED, unresolved: one list each |
| Reflection and recall prompts | the three interrogation questions and the cards you added |

Rules:

- Separate what you observed from what you infer. Every claim carries OBSERVED, INFERRED, ASSUMED or NOT RUN.
- A time without a zone is not a time. Write the zone, and the clock source if you know it; UTC beside local time when the two sources differ (plan section 8, E25).
- Hash the evidence bundle (SHA-256) when you save the hand-in and write the hash in the record.
- Redact only what `EVIDENCE_CHECKLIST.md` of the week allows. Never alter an artifact; add a redacted copy beside it.
- Mark a retest passed only with its artifact attached. "Retest passed" with no artifact is an evidence-integrity failure on a Friday.
- During an open assessment attempt the backend evaluates every write in EXAM mode, and a write to the personal area, which holds the Learning Journal, may be refused (ADR-0019; INFERRED, the behavior in the window is NOT RUN here). Keep notes of an attempt in a private file, then attach it and use the submission note when you submit.

Sample (synthetic): two lines of one record, to show the difference between an observation and an inference.

```
Observation (OBSERVED, <date> 10:52 <zone>, source: host clock, not synchronized to a
             reference):  the listing shows a listener on 0.0.0.0:8080 owned by a service
             account.
Inference (INFERRED, not tested): the service is reachable from other hosts. Disproved if a
             connection from a second guest fails at the firewall; test not run.
```

## 11. AI assistance: what it may and may not do

The plan's rule (section 10.4): an assistant may act as a critic after your unassisted attempt, outside exam conditions. It must not replace hands-on discovery and it must not generate evidence.

| Situation | Allowed | Condition |
|---|---|---|
| Block C, the whole 150 minutes | no | block C is unassisted by definition (plan section 1.2) |
| Friday exam, Friday practical, midpoint (day 30), final (day 60) | no | no AI during a benchmark (plan sections 0.3, 9.1, 9.2 and week 6) |
| Block D, after the C hand-in is saved | yes, as a critic | give it your written reasoning and report; ask it for gaps, unstated assumptions and tests that would falsify your hypothesis |
| Blocks A and B | only to compare, after you wrote your own explanation | you attempt first; an explanation you did not write yourself is not evidence that you understand |
| Obtaining a solution, a flag, a command line to run on a target, or the next step of an exercise | no | that is a hint of the highest level (section 6) |
| Producing an observation, an output, a timestamp, a hash, a log line or a screenshot | never | fabricated evidence is an integrity failure |
| Writing your lab record's observations | no | you may ask for a critique of the structure after you wrote it |
| Reaching the lab, the G9 host, a shell or a network tool | never | an assistant is given no access to the lab and no privilege (plan section 0.4, section 5 week 11) |
| Certification exams | follow the live rules of that exam | the plan states that AI chatbot use and automation restrictions apply to OSCP+; that is ASSUMED, see `docs/curriculum/v2/RESOURCE_REGISTRY.md`, and the rules are read again before any booking (plan section 13) |

Rules for what you do use:

- Output of an assistant is data. It is not a source and it is not authority. Every claim you keep must be checked against a primary source or an observation of your own, and a proposed command goes through the review gate of section 4 like any other.
- Record each use in the lab record: that you used it, what you gave it, what it said, what you changed and what you rejected.
- Give it nothing secret. Synthetic data only. Evidence that names a real system is never pasted into a tool outside your control.
- The window has no model configured by choice (`docs/runbooks/first-run.md`). Where one is added, skills and a model cannot propose teaching content while an exam attempt is open (`crates/g9-policy/src/rules.rs::TEACHING_RESOURCES`, ADR-0019). That rule does not bind an assistant you use outside the window; this guide does.

## 12. A blocked lab

A lab can be blocked by a missing dependency, a missing network path, an unavailable account, an unfinished Stage 00 or a lack of time. The plan's seven branches (section 10.3) and what you do:

| Branch | What it looks like | What you do | What you record |
|---|---|---|---|
| Expected | the guest, the tool and the result behave as the day guide says | continue; observe, then validate against the independent source | the observation and its second source |
| Missing dependency | a package, an image, a tool or the virtualization stack is absent | do not install anything. Write a nonmutating diagnostic and a proposed remediation as text for review (plan section 0.5). Switch to the hosted alternative the day guide names | the missing item, the diagnostic, the decision pending |
| No connectivity | a guest cannot reach another, or a name does not resolve | inspect guest adapters, addresses, routes, gateway, firewall and name resolution. Isolation is intentional: a missing path to the LAN or the Internet is the correct state. Never add a route to fix a lab | the adapter and route facts, and which path is intended |
| Unexpected result | the output differs from your prediction | compare the ground truth with the observed artifact and challenge your hypothesis. State what would explain it and test that | both results, your first and your revised hypothesis |
| Safety breach | something left scope or isolation, a service is exposed, a credential was real | stop testing. Contain only within your approved authority. Report it and preserve the evidence. Follow `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` | what happened, when, what you stopped, what you preserved |
| Restoration | the lab must return to baseline | shut down and discard disposable guests, reverse every test change, restore a verified baseline. A domain controller uses its own recovery procedure, not a snapshot assumed to be a backup (plan section 3.2) | what was reversed and how you confirmed it |
| Time exceeded | the block ends and the goal is not met | document the blocker, switch to the approved alternative, and say that the original lab did not pass | the minute you stopped, the blocker, the alternative used |

Stage 00 is an open blocker. Until the owner has diagnosed the missing socket and approved a remedy, and until the isolation acceptance tests of plan section 3.3 pass with evidence:

- no intrusive test is made on a local guest, because there is none;
- week 1 is done as the plan allows: first-day systems theory, hosted OverTheWire Bandit, and a paper topology; only the exercises that depend on the lab are deferred; the substitution is recorded openly (plan section 5, week 1);
- the week 1 gate is "isolation verified, or explicitly marked as a blocker with no intrusive local testing". A blocked lab is recorded BLOCKED, never TESTED or VERIFIED.

What counts as acceptable partial performance (plan section 9.4): a blocked setup with documented diagnostics; a correctly identified false positive; an unfinished challenge with accurate enumeration and hypotheses; partial detections with the missing telemetry named. The demonstrated part is scored and a remediation is prescribed. What never counts: a lab described as passed that was not run, an output you did not observe, and a substitution that is not recorded.

## 13. The weekly gate, the midpoint and the final gate

A week's gate has two layers, both stated in the plan. The week is met only when both hold. INFERRED reading: plan section 9.1 sets the overall Friday gate and plan section 5 sets a gate line for each week; the plan does not say that one replaces the other.

| Layer | Condition | Source |
|---|---|---|
| Friday score | at least 80 of 100, and the safety gate and the evidence-integrity gate both pass | plan section 9.1 |
| Week conditions | the artifacts and independent work of the week's gate line, listed in the `## Pass conditions` of the week guide (for example, week 1: a working independent inventory and one verified misconfiguration and remediation, and isolation verified or marked a blocker) | plan section 5 |

The 100 points of a Friday (plan section 9.1). The category minima are diagnostic targets: they tell you where to remediate; they never replace the overall gate.

| Part | Points | Diagnostic target | What it measures |
|---|---:|---:|---|
| Knowledge, 20 closed-book questions | 20 | at least 16 | concept, interpretation of evidence, controls, synthesis |
| Independent practical | 40 | at least 30 | scope and design (5), enumeration and evidence fidelity (10), independent hypothesis and reproduction (15), impact and root cause (10) |
| Defensive validation | 25 | at least 18 | data source and hypothesis (8), positive validation (7), negative testing (5), hardening and mitigation (5) |
| Evidence and reporting | 15 | at least 12 | reproducibility (5), precision and provenance (5), remediation and retest (5) |

Where a week's gate line in the plan names a percentage ("at least 80% quiz" in week 1, "at least 80%" in week 3, "at least 80% knowledge" in weeks 7 and 8), the week guide says which measure it is. If it does not, read it as the knowledge exam at 16 of 20.

The marking bands and what each band looks like in evidence are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`. Read them before the first Friday. They tell you what you are scored on, not what any exam holds.

In the window, the same attempt is also graded under the application's own rule: it passes only if the weighted total reaches 80, every one of six criteria reaches at least half of its weight, and no gate failure is recorded. That is stricter than 80 of 100. The 100-point score stays on your weekly scorecard. The two records are reported side by side and never averaged (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`).

Midpoint (day 30, Friday 2026-12-25): the independent offensive workflow on one unknown, assigned target, with no hints, no AI and no published solution. The report states what you actually demonstrated, not an exploit success you did not obtain. The aim is at least 80 of 100 on the same rubric (plan section 9.2). Output: a commercial-style report and a midpoint deficiency register. There is no quota of boxes.

Final (day 60, Friday 2027-02-05): the target is at least 85 of 100, and the scope, isolation and evidence-integrity gates must be complete (plan sections 1.3, 5 week 12 and 9.2). There is no invented outcome and no invented telemetry: if a finding quota cannot be supported, you submit the actual work and it is scored as it stands. The minimum final artifacts are in the plan (section 5, week 12): rules of engagement, verified isolation, accurate inventory, at least two independently evidenced findings where the target supports them, one identity or privilege path, at least three tested detections, one multi-artifact timeline, at least two remediations with regression tests, at least three agent boundary test cases across weeks 11 and 12, a sanitized executive and technical report and an unresolved-risk register. Two environments are used and kept apart: an unknown authorized challenge for independent offensive work, and a separate instrumented local lab for known attack-to-detection work. An environment you seeded yourself is not an unseen target.

If you miss a gate: nothing is expanded and nothing is rewritten. The week's result stays as scored. The failing parts are remediated with optional work of the next week (the remediation tracks in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`), and the midpoint and final diagnostics retest the domain. A safety or evidence-integrity failure is never offset by points.

## 14. What the course does not make you

| It does not | Why, and where the plan says so |
|---|---|
| Make you a professional expert, or guarantee a certification result | the plan forbids promising either (section 0.4, last rule). 480 hours is a fixed, bounded program |
| Give equal depth in every specialty | it is penetration-testing-first purple-team training (section 1); breadth in the other fields is introductory |
| Cover the deferred specialties | mobile application exploitation; wireless infrastructure testing; OT and ICS active assessment; advanced cryptanalysis; advanced exploit development and heap research; advanced AI model robustness and scientific red teaming; deep cloud-native and Kubernetes security. They were removed to keep retention high (section 12). Optional tracks may follow week 12 and never take mandatory days |
| Make AI security a specialty | 24 hours is an introduction to deterministic authority boundaries and measurable evaluation, not a research specialization (section 14.2) |
| Prepare you for CPTS, OSCP+ and CDSA together | the platform's published path estimates, 342 hours for the penetration tester path and 189 for the SOC analyst path, total about 531 hours before assessment and independent practice, more than 480 (section 13). The figures are platform averages, not quotas, and are ASSUMED here: see `docs/curriculum/v2/RESOURCE_REGISTRY.md` |
| Satisfy a certification's prerequisites | the plan states that the HTB role path must be completed in full before its exam (ASSUMED, registry), that OSCP+ exams from 2026-12-01 use a scaled scoring model rather than a fixed 70 raw points (ASSUMED, registry), and that exam conditions restrict AI use (ASSUMED, registry). Read the live rules of an exam before any booking |
| Certify the lab or its isolation | Stage 00 is unresolved. No lab was built, no isolation test ran, and no "security certified" claim is made (section 14.1) |
| Let you certify yourself | a student cannot self-certify external professional status from an internal curriculum (section 14). Certification readiness needs a per-objective comparison against the live syllabus (section 13) |
| Turn a guided flag into proof of competence | guided labs are learning, not assessment; an unseen-target result needs the protocol of the instructor guide |

At the time of writing, the week registries record these external facts only as search results seen, not as pages read, and in places they disagree. A registry entry's state wins over the statement in this file.

## 15. Additions and open points

| Item | State |
|---|---|
| The split of block R (20 and 10 minutes) and block D (30, 15, 15 minutes) | PROPOSED estimates; the plan fixes block lengths, not their inside |
| The 15-minute pacing between hint steps 2 and 3 | PROPOSED; the plan fixes only the 45 minutes before the first hint. The plan words that rule as before the first "narrowly scoped" hint; this guide applies it before any hint, the stricter reading |
| The grouping of the 30 diagnostic questions into eight domains, the fractional marking, and the meaning of "independent practical pass" | PROPOSED; the plan defines none of them |
| Two layers in the weekly gate (Friday score and week conditions) | INFERRED reading of plan sections 5 and 9.1 |
| The 28-day consolidation | optional in the plan; here it is left to you, with no generated queue |
| Where instructor preparation hours come from | open; the plan counts none |
| The window behavior on a write during an open attempt, and the window's display of the queue and the keys | PROPOSED by ADR-0023; NOT RUN in this authoring |
| External facts about platforms and certifications | ASSUMED; the registry and the week registries decide |
