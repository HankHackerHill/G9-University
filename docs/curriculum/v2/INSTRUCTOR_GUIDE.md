# G9 Cybersecurity Mastery v2.0: instructor guide

- Reader: the owner, as the instructor. The learner and the instructor are the same person; section 1 says what that costs and how the guide limits it.
- Source of requirements: plan sections 0.3, 1.3, 9.1 to 9.4, 10.2, 10.3 and 14 of `docs/curriculum/v2/MASTER_PLAN_V2.0.md`; ADR-0012 (the application's pass rule) and ADR-0023 (how the plan is applied).
- This guide holds no key, no fixture map and no fault of any item. It describes process. The marking bands are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`; the keys are in the `ANSWER_KEYS/` folders and in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/`.
- Status: PROPOSED, written 2026-11-18. No attempt was scored, no practical was prepared, no lab was built. Every command named here is proposed text and none was run.
- Stage 00 (the libvirt socket) is unresolved: see `docs/curriculum/v2/README.md`. Until it is resolved, a Friday practical that needs a local guest cannot be prepared on it, and section 8 says what stands in.

## 1. One person in three roles

| Role | Work | Risk when the same person is the learner |
|---|---|---|
| Preparer | chooses or builds the unseen practical and holds its fixture map | the learner knows what the preparer knows, so the practical is not unseen |
| Scorer | reads the evidence, applies the key and the bands, enters levels in the window | the scorer has an interest in the result and can edit evidence after reading the key |
| Source checker | validates resources before a week starts | trusting a registry label as a check |

What the guide does about it:

- The practical is made unseen by one of three protocols, in order of preference (section 8).
- The evidence cannot change after the key is read: its hash is recorded before any key is opened (sections 3 and 6).
- Every sub-criterion score carries one line naming the evidence, and an ambiguous case takes the lower band (rubrics, section 3).
- Where another person can read the evidence bundle and the scoring sheet, they do, and their name goes on the scorecard.

What a one-person grade supports. The application classifies a grade decision by comparing the reviewer with the attempt's learner. If they are the same actor, the decision is `SELF_ADJUDICATED`, and a self-adjudicated decision never counts as validation evidence for a competency (ADR-0012; `crates/g9-assessment/src/grading.rs::ReliabilityClass`, VERIFIED by reading the code, not by running it). A one-operator installation records the owner as both, so every decision there is self-adjudicated. Whether the window can record a second actor as reviewer is not established here (PROPOSED check). A one-person grade supports the owner's own planning and remediation. It does not validate a competency in the application and it supports no external claim.

The separation of instructor files from learner files (README, "Layout") prevents accidental exposure. It is not access control: the keys are in the same repository as the lessons.

## 2. A Friday, step by step

A Friday is one assessment attempt in the window, begun before the knowledge exam and submitted with the report (ADR-0023, decision 2). The order of the steps below is fixed even when time runs short.

| When | Step | Output |
|---|---|---|
| Before the week, outside counted hours | validate the week's sources (section 7); choose or have prepared the unseen practical (section 8) and record its hash commitment; confirm the environment state (Stage 00, isolation tests for local-lab weeks); check fixture hashes against the week's `LABS/fixtures/MANIFEST.md`; open the scorecard row with the protocol | a validation record; the scorecard row; the commitment hash |
| Friday 08:00 to 08:30, block R | the recall queue. Not part of the attempt | recall log |
| About 08:25 | define the week's assessment from its generated payload if it is not defined yet, then begin the attempt. Do not open `ASSESSMENT.md` or the payload before this step: the task statement holds the 20 questions | the attempt, with its start time |
| 08:30 to 10:30, block A | Part 1, the closed-book knowledge exam. No notes, no AI. Day 55 differs: block A is a lesson and the exam sits in block D. At the end, write the answers to a file, compute its SHA-256 and record the hash and time | locked answers |
| 10:45 to 12:45, block B | the plan's review or planning task for the week. A guide review happens only after the answers are locked | planning notes |
| 13:45 to 16:15, block C | Parts 2 and 3 of the practical. No hint, no key, no AI. The preparer gives the scenario at the start of the clock and only then (protocol 1) | the evidence bundle, the hint log (empty), the command log |
| 16:15 to 17:15, block D | Part 4, the report. Then: hash the evidence bundle and record the time; submit the attempt in the window with the evidence; only then open the key and the fixture map, and log the time; score (section 3); record on the scorecard; review the evidence, enter the six levels, grade, approve final in the window | the report; the hash; the key log; the scorecard; the application's grade decision |
| After | assign the remediation (section 9); the missed question identifiers go into the recall flashcards | remediation entries |

Day 55 is the exception (plan section 5, week 11): block A is a lesson, block B inspects the toy-agent policy interface, block C is the practical, and the AI mini-exam sits in block D. PROPOSED for that day: begin the attempt before block C so that the practical and the exam are inside it, lock the exam answers at the end of block D, and keep every other step and its order.

The plan allots 60 minutes to "scoring and report". Report writing, submission, key-based marking, band scoring and the application's grading may not fit (ESTIMATE, not measured: report 25 minutes, submission 5, marking of Part 1 15, banding of Parts 2 to 4 15, window grading 10). If time runs out, stop at a step boundary, record on the scorecard which steps are unfinished, and finish them outside the counted hours. Hours are never added to the course and the unfinished scoring is never skipped by opening a key early.

The attempt has a time limit set in its definition. The application decides the time-limit gate from the monotonic session timer, not from a wall-clock subtraction (`crates/g9-assessment/src/grading.rs::time_limit_gate`).

While an attempt is open, the backend evaluates writes in EXAM mode (ADR-0019). A write to the personal area, which holds the Learning Journal, may be refused until the attempt is submitted (INFERRED; NOT RUN). Keep working notes in a private file and attach them on submission.

## 3. Scoring a Friday

The four parts and their sub-criteria are the plan's (section 9.1). The bands and the observable evidence of each are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`.

| Part | Points | Sub-criteria and points | Diagnostic target |
|---|---:|---|---:|
| Knowledge | 20 | 20 questions of 1 point: at least 6 concept, 6 evidence or code interpretation, 4 applied control, 4 synthesis | at least 16 |
| Independent practical | 40 | scope and design correctness 5; enumeration and evidence fidelity 10; independent hypothesis and reproduction 15; impact and root-cause reasoning 10 | at least 30 |
| Defensive validation | 25 | correct data source and hypothesis 8; positive validation 7; negative testing 5; hardening and mitigation assessment 5 | at least 18 |
| Evidence and reporting | 15 | reproducibility 5; precision and provenance 5; remediation and retest 5 | at least 12 |

Gates and targets:

- The weekly gate is at least 80 of 100, and the safety gate and the evidence-integrity gate pass. The category minima are diagnostic targets: a part below its target steers remediation and never replaces the gate.
- The final gate is at least 85 of 100 with no safety or evidence-integrity violation (plan sections 1.3 and 9.2).
- A week is met when both layers hold: the Friday score and the week's own gate conditions in the `## Pass conditions` of its week guide (plan section 5). INFERRED reading; see departures.

Procedure:

1. Preconditions: the attempt is submitted; the knowledge answers and the evidence bundle each have a recorded hash and time from before any key was opened. If a hash is missing, record that on the scorecard: E2 then cannot earn its verification point, and the evidence is scored as it stands.
2. Gates first. Read the command log, the capture or router logs and the report against the scope. Record a failure of the safety gate or the integrity gate before adding points (section 5).
3. Part 1: mark each answer with the exam key: 1 or 0 for multiple choice; 1, 0.5 or 0 for constructed response by the descriptors of the rubrics.
4. Parts 2 to 4: for each sub-criterion write the band, the points and one line naming the artifact that supports it. A claim with no artifact scores in the none band. When two bands are plausible, take the lower.
5. Add the points. Compare each part with its diagnostic target and the total with 80 (85 on the final).
6. Derive the application's six levels from the same points (section 10).
7. Record on the weekly scorecard: the time taken, the failures, your confidence before you knew the score, the environment notes, the protocol of the practical, the hashes and times, and the key log (plan section 9.1 asks for time, failures, confidence and environment notes). No grade comes from an attempt count or from tool output.
8. Open the remediation entries for every part below its target (section 9).

Closed book: the knowledge exam has no notes unless the exercise is explicitly open-reference. The Friday practical has no hints. No AI during any benchmark (plan section 9.1).

## 4. Midpoint (day 30) and final (day 60)

| | Midpoint, Friday 2026-12-25 | Final, Friday 2027-02-05 |
|---|---|---|
| Practical source | one unknown, assigned target. Protocols 1 or 2 only (section 8) | an unfamiliar authorized offensive target, protocols 1 or 2 only, plus a separate instrumented local lab for known attack-to-detection work. The local lab is known by design and is not an unseen exam |
| Hints, AI, published solutions | none | none |
| Scoring | the four parts; the aim is at least 80 of 100 on the same rubric | the four parts; the target is at least 85 of 100, with scope, isolation and evidence-integrity gates complete |
| Report | commercial-style report and a midpoint deficiency register | sanitized executive and technical report, timeline, portfolio, unresolved-risk register |
| Not completed | the partial evidence is graded and the blocker documented; no exploit success is invented; there is no quota of boxes | the partial evidence is graded; if the finding quota cannot be supported, the actual work is scored as it stands |
| Also on that day | the diagnostic retest (plan section 1.3) | the diagnostic retest; the next-step benchmark |

The final's unknown-target work happens on day 57 and the instrumented lab work on days 58 and 59 (plan section 5, week 12); the day 60 attempt scores their evidence. Apply the unseen-target protocol from day 57. Do not select a challenge whose complete solution the learner already read (plan, week 12). The midpoint and final point splits are not separate in the plan; this guide keeps the four parts (departures).

## 5. The safety gate and the evidence-integrity gate

Both gates are the plan's (section 9.4). A failure of either means the attempt never counts as a passed practical assessment, whatever the total. The points are still scored for diagnosis.

The safety gate fails on: out-of-scope target activity; use of real production credentials; an exposed vulnerable service that is not contained or remediated; unauthorized public cloud provisioning; destruction of others' data; knowingly falsified evidence.

The evidence-integrity gate fails on: missing target or scope context; fabricated commands or outputs; a test claimed passed with no observed result; a destructive tool run without a record; unacknowledged use of a published solution; a false positive claimed as a verified finding.

How to judge (PROPOSED):

1. Trace every doubtful claim to a raw record in the evidence bundle that was hashed before the key was opened.
2. A claim that states an observation that is in no record is a failure. An error that the raw record corrects, and that changes no conclusion, costs points in precision and provenance (E2), not a gate.
3. A reconstruction labelled as reconstructed from memory is not fabrication.
4. The gate is binary and the response is proportional: record it, stop, contain, report, re-confirm scope, and let the remediation track decide what follows (section 9). Do not offset a failure with points.
5. Exposure that is noticed at once, stopped, recorded with its window and shown to have been reached by no outside connection is not a gate failure; its cost is in the points that the record shows. The same exposure left until the end of the attempt is.

Examples. Every case is synthetic and generic. None describes a technique or a real target.

| Case | Gate | Why | Effect |
|---|---|---|---|
| The scope names `10.77.20.20` as the only target. A loop in a helper script sends probes to every address of `10.77.20.0/24` | safety | the other addresses are out of scope even though they are in the same lab network | gate failure; P1 at most 2 |
| During a RED-to-DMZ exercise a connection is opened to a CORP address "to check something" | safety | the exercise authorizes RED to DMZ only; RED to CORP is denied and logged by design (plan section 3.2) | gate failure |
| A login form on a lab target is tested with the learner's own work-account password | safety | a real credential; only synthetic credentials are allowed | gate failure |
| A vulnerable test container is published on all interfaces of a machine that is attached to the household LAN, and left until the end | safety | an exposed vulnerable service not contained | gate failure |
| The same exposure is seen after a few minutes, the container is stopped, and the record shows the window and that no connection reached it | none | contained and remediated at once, with a record | no gate failure; P1, D4 and E2 reflect it |
| A cloud resource is created in a personal account instead of the approved sandbox | safety | unauthorized public cloud provisioning | gate failure |
| A cleanup step deletes a shared directory that the learner did not create | safety | destruction of others' data | gate failure |
| The report lists the output of a command that appears in no session log or capture | integrity | a fabricated output | gate failure |
| The report says "retest passed" and attaches a capture taken before the change | integrity | a test claimed passed with no observed result | gate failure |
| A scanner finding is labelled Confirmed, and no manual reproduction exists | integrity | a false positive claimed as verified, when the label is knowingly unsupported | gate failure; the honest label is Unconfirmed |
| A finding names no target id and no scope reference | integrity | missing target or scope context | the critical criterion scope compliance and reporting is set below its floor (level 0 or 1); in the plan this is an integrity failure |
| A reset or wipe of a fixture appears in the system and not in the command log | integrity | a destructive tool run without a record | gate failure when it was unauthorized; otherwise the scope criterion below its floor |
| An exploit step in the report matches a public write-up and the report does not say so | integrity | unacknowledged use of a published solution | gate failure |
| A timestamp is wrong by an hour and the raw log shows the right one | none | an error that the raw record corrects | E2 loses a point |
| An output was written from memory and the report says so | none | a labelled reconstruction | E1 and E2 lose points |
| The lab could not be built and the learner documents the diagnostics and uses the named hosted alternative | none | acceptable partial performance | the demonstrated part is scored; BLOCKED is recorded |
| A scanner finding is reported as a false positive after a manual test that is recorded | none | acceptable, and good evidence | credited in P2 and P4 |
| A wrong hypothesis is recorded, tested and discarded with evidence | none | acceptable | credited in P3 |
| A challenge is not finished and the enumeration and hypotheses are accurate | none | acceptable partial performance | the demonstrated part is scored |

Application gate codes. The application enumerates four gates: `OUT_OF_SCOPE_TARGET_OR_ACTION`, `UNSAFE_OR_DESTRUCTIVE_UNAUTHORIZED_ACTION`, `FABRICATED_OR_ALTERED_EVIDENCE` and `TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION`. The plan names more conditions than the application has gates, so the nearest code is chosen (the mapping table is in the rubrics, section 8). A condition with no code is recorded by setting `scope_compliance_reporting`, the critical criterion, below its floor. The time-limit gate is not in the plan.

## 6. Releasing keys

A key is opened only after the attempt it concerns is submitted. In the window, a day key is read only when the operator asks for it, and a Friday exam key only after that week's attempt is submitted (ADR-0023, decision 6). The files in the repository can be read at any time; the rule below is a discipline and is not enforced there.

| Key | File | Open only when | Log |
|---|---|---|---|
| Day quiz and hint 4 | `WEEKS/Wnn/ANSWER_KEYS/DAY_NN_KEY.md` | the eight answers are written and the block C hand-in is saved and hashed | time, file, section |
| An earlier day's key, for the recall queue | the same file | your written answer to that question exists | time, file |
| Friday exam, practical fixture map, practical scoring | `ANSWER_KEYS/ASSESSMENT_KEY.md` | the attempt is submitted in the window and the evidence bundle hash and the answers hash are recorded | time, file |
| Lab reference observations | `ANSWER_KEYS/LABS_KEY.md` | the guided lab: its hand-in is saved. The independent lab and the Friday practical: the attempt is submitted | time, file, lab |
| Midpoint and final fixture maps and course keys | `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` | the attempt is submitted | time, file |
| Entry diagnostic key | `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` | the diagnostic answers are locked by hash on day 1 | time, file |
| A fixture map made by another person | their sealed file | the attempt is submitted; verify its SHA-256 against the commitment given before the attempt | the match or mismatch |
| The collected key `docs/curriculum/v2/ASSESSMENTS/ANSWER_KEY.md` | generated | treat it as sealed until the final is submitted: it holds every key of every week, including weeks not yet taken. Use the per-week files | any opening |

Rules (the consequences are PROPOSED; the plan only withholds solutions until evaluation is submitted):

- Log every opening in the scorecard's key log with the time. An unlogged opening is an integrity matter.
- Opening a day key early: the objective is guided, not independent. Record it.
- Opening an exam key or a fixture map before submission: the questions seen score 0; P3 is in the none band because the work was not independent; if it is not declared, it is an unacknowledged published solution (section 5). If it is declared, it is not a gate failure.
- A key is never copied into a learner file, and a key line is never written beside an unsolved item. Flashcards made from missed items are the learner's private file and are written in the learner's words.
- Hash before key: the evidence bundle and the knowledge answers are hashed and timed before any key is opened, so that scoring cannot be influenced by editing. A later edit is a dated addendum, shown separately and not scored.

## 7. Source validation before a week starts

Who: the owner as instructor. When: before the Monday of the week, outside counted hours. Inputs: `WEEKS/Wnn/RESOURCES.json`, the week guide and the Resources tables of the week's day guides. Output: a dated validation record kept in the repository, and registry fields updated where a check differs.

The registries record labs and references that were checked only by search result; the authoring session could not open pages (ADR-0023, decision 10). The strongest label that the registry schema allows is `SEARCH_CHECKED`; `VERIFIED` is not one of its values (`schemas/curriculum/resource-registry.schema.json`). A page that you open and check is evidence only when you write the record.

| # | Check | How | Record | If it fails |
|---|---|---|---|---|
| 1 | every resource id cited in the week's day guides is in the registry | the builder check for the week (section 11, command 1) | the exit status | fix the guide or add an entry under the registry rules |
| 2 | the address opens and the title matches the entry | open the page from a network you permit; the window opens no link | date, page title seen | mark the entry UNVERIFIED and use its fallback |
| 3 | the access tier | read the page: free, free with an account, paid or mixed; note any subscription needed | the tier seen, against `access` | change `access`; pick the fallback if the tier is not one you hold |
| 4 | the version and the date | read the product version, edition or release shown | the value seen, against `version` | update `version`; re-read the objectives that depend on it |
| 5 | named rooms, labs and machines | whether it exists, is active or retired, free or subscription, and the platform's rule on write-ups; no active-machine spoilers in the portfolio | the status seen | replace it; note the fallback |
| 6 | the time estimate | compare with the platform's own estimate; keep the word estimate | both figures | adjust the day's use |
| 7 | the fallback | open it and confirm it serves the same objective and yields the same evidence | date, what it yields | add a new fallback |
| 8 | the platform's terms | read what they allow about accounts, automation and publishing results | date, the rule | do not use the exercise in a way the terms forbid |
| 9 | no solution read | confirm that you have read no walkthrough of any exercise you will use as an unseen practical | one line | choose another exercise (plan, week 12) |
| 10 | plan facts the week uses | reread the entries of `plan_fact_checks` with NOT_CONFIRMED or CONTRADICTED; certification rules are read again from the official page before any booking (plan section 13) | which facts, what the page says | state the fact as ASSUMED in what you write |
| 11 | local lab dependencies | the Stage 00 state and whether the isolation acceptance tests (plan section 3.3) passed | state | mark the week's local exercises BLOCKED in advance and confirm the hosted alternative |
| 12 | fixtures | the hash of each fixture against `LABS/fixtures/MANIFEST.md` | match or mismatch | regenerate from the recipe in the manifest; never edit a fixture |

After a check that changes an entry, edit `RESOURCES.json` (source, edited by hand), then regenerate (section 11, command 2 is proposed only when you decide to). A registry change is required for any change of a resource (plan section 10.2). Whether the schema should gain a value that means "opened and read by the owner" is an open point.

Never label an entry VERIFIED in prose without a dated record in the repository that names what you saw. Nothing in a day guide may be called verified because the registry holds the entry.

## 8. Preparing an unseen practical when learner and instructor are one person

An unseen practical is unseen only if the learner does not know the faults or the target. The plan requires an unseen or modified scenario every Friday and an unknown target at the midpoint and the final, and forbids treating an environment you seeded yourself as unseen (plan sections 9.3 and 5, week 12). Three protocols, in order of preference. Use the first one that is possible and write which one you used on the scorecard. Every rule in this section is a PROPOSED addition to the plan (see the departures table).

### 8.1 Protocol 1: another person prepares it

| Item | Rule |
|---|---|
| Who | a person who is not the learner and who knows the week's topic. They are told the rules: an authorized synthetic lab only; synthetic credentials and data; no real host, organization or secret; no vendor course text |
| What they receive | the plan's row for the Friday (what cannot be assumed), the week's catalogue of fault classes from the instructor files, the scoring bands, the environment id and the addresses of the scope, the time limit. Not the learner's notes |
| What they produce | a variant that differs from the guided and the independent labs; a fixture map listing every fault, how it is observed and what the correct remediation is; a brief stating scope, target ids, time limit, deliverables and what is scored, with no hint; positive and negative test expectations for the defensive part |
| Commitment | before the attempt they give the learner the SHA-256 of the fixture map and keep the map. The learner writes the hash and the time on the scorecard |
| Pre-flight | the preparer runs the scenario once to confirm that it is solvable in the time and that the guest has no route to the household LAN or the Internet |
| Handover | at the start of the clock only. They give logistics help (is the environment up) and nothing else; each such contact is logged |
| After | after submission they reveal the map and the learner verifies the hash. They may also read the evidence and score it, which can make the review external |

Limit: it needs a second person, and a preparer's mistake (a fault that cannot be found, an unsafe design) costs the learner's time. The pre-flight is the control.

### 8.2 Protocol 2: an unseen public challenge from a pool fixed in advance

| Item | Rule |
|---|---|
| The pool | fixed before the week starts: at least 5 candidates for the Friday, written with the date and hashed. Candidates are platform-designated exercises (registry kinds CHALLENGE, LAB, ROOM or MACHINE) that no day guide already uses, or entries added under the registry rules |
| Eligibility | a platform-designated target; a tier you hold, checked at source validation; never attempted; no solution read; the platform's spoiler and write-up policy respected; difficulty near the week's difficulty; compatible with the plan's row of what cannot be assumed |
| The draw | at the start of the Friday by a physical die or by a seed recorded before the draw; the pool, the method and the result go on the scorecard |
| Scope | that target only. A hosted target is reached from a browser or the platform's own environment under its terms, never from the local lab's guests (plan section 3.2: no guest route to the Internet during active training) |
| After | read no solution before the report is submitted. Never publish a solution |
| The plan's own sources | days 10, 20, 35 and 40 already name public sources in their block C (section 5 of the plan); this protocol fits them |

Limits: a public challenge does not carry the week's own structure. Where it gives no source or telemetry, Part 3 is scored on a paper analysis (D1 and D4) and on positive and negative tests against a small local fixture that was built before the attempt, labelled as such (PROPOSED). The grader checks the chosen challenge against the plan's row for that day.

### 8.3 Protocol 3: a sealed fault-injection script

The weakest protocol. Use it only when the first two are impossible, and label the result self-prepared.

| Item | Rule (PROPOSED) |
|---|---|
| Catalogue | at least 12 faults for the week, each with an id, a class, a parameter space of at least 5 values, a way to observe it and its repair. The catalogue is written during the week and is not secret; the draw is |
| Draw | 2 or 3 faults by die or by a seed written by another person on paper or produced by a hardware die. The seed is recorded only in the sealed file |
| Scope of action | the script acts only on a disposable guest or a fixture folder, on target ids named in it, never on the trusted G9 host, never on data you did not create |
| Output | it writes the applied-fault log to a sealed file. The SHA-256 of that file is written on the scorecard before the attempt |
| Sealing | the log is held by another person, or encrypted with a key that another person holds. A file that only has restrictive permissions prevents accidental reading and nothing else; say so |
| Script status | proposed text. It is not run without your approval, in the lab (blocked by Stage 00) or against a fixture folder. It has a tested rollback |
| Pre-flight | run it against a copy, confirm that the baseline is recoverable and that the guest has no outside route |

Limits: the script's author knows the catalogue and the parameter spaces. The result is a modified scenario in the plan's sense, not an unseen one, and never an unknown target. It is not admissible for the day 30 target or the day 57 target. The application records the grade as self-adjudicated regardless of the protocol.

### 8.4 Which protocol fits which Friday

| Day | What cannot be assumed (plan section 9.3) | Source named by the plan for block C | Admissible protocols |
|---:|---|---|---|
| 5 | root access, or that one specific command will succeed | a blind Linux hardening or troubleshooting scenario | 1, 3; protocol 2 stands in while the lab is blocked |
| 10 | that every dropped packet is malicious | PCAP triage or a CyberDefenders network exercise | 2, 1; 3 only with a generated dataset |
| 15 | that a Kerberos ticket event implies an attack | an unknown identity-misconfiguration assessment | 1, 3 |
| 20 | that all scanner findings are real | a Juice Shop or PortSwigger mystery challenge | 2, 1 |
| 25 | that a valid login authorizes any object | a local web and API mini pentest | 1, 3 |
| 30 | a pre-known vulnerability or a solved walkthrough | an unknown target, full chain | 1 or 2 only |
| 35 | that missing logs imply no activity | a LetsDefend or CyberDefenders simulated case | 2, 1 |
| 40 | that every suspicious process is malware | an unknown DFIR training case | 2, 1; 3 only with a generated evidence set |
| 45 | that a crash is remote code execution | a small program or a curated trace | 1, 3, 2 |
| 50 | that reachability in a graph is achieved access | an AD attack-path or misconfiguration lab | 1, 3, 2 |
| 55 | that a model refusal alone is a sufficient defense | at least two controlled unauthorized tool proposals and normal requests | 1, 3 |
| 60 | that a planted environment is an unseen exam | independent reproduction; the unknown target work is on day 57 | offensive target: 1 or 2 only; instrumented lab: any, and it is not unseen |

The source column is the plan's text for the block C task of that day, not a verified resource. Until Stage 00 is resolved, a scenario that needs a local guest has no local form; use protocol 2 or a fixture variant and say so on the scorecard.

Every unseen practical, whichever protocol: a written scope block with target ids and addresses, the time limit, the deliverables, what is scored, the safe-scope statement, and a rollback and cleanup. The learner file `ASSESSMENT.md` of the week holds the rules and a catalogue of classes; the fixture map is kept apart.

## 9. Remediation tracks

The plan's rule (section 1.3): failing topics trigger substitution of optional next-week work. Scheduled hours are not expanded and completion is not falsified. Optional work here means the day guides' optional expert extensions and the week guide's accelerator. Required labs, block C, the Friday assessment and the gates are never substituted. The plan states no amount of optional time; if there is too little, do the first two items of the track and record what was carried in the deficiency register.

A failed Friday is not repeated: the plan defines no retake. The result stays as scored; remediation is shown by the next Friday's matching part and by the midpoint and final diagnostics. The window can record a retake as a new attempt linked to the first; using it is PROPOSED, optional, unscheduled and credits no hours, and only with a fresh unseen variant. The scorecard keeps the original result and adds remediation lines; it never overwrites (finalized records are superseded, not edited).

| Trigger | Track | What replaces what | Counts as done when | Retest |
|---|---|---|---|---|
| Part 1 below 16 | K, knowledge | for each missed question: write in your words the mechanism, its limit and why each wrong option is wrong; add a flashcard; queue the question at 1, 3 and 7 days | every missed question has its explanation and a card | the same competency's questions in the next Friday and the diagnostic retest |
| Part 2 below 30, or a sub-criterion in the none band | P, practical | P1: write the scope block and test plan before acting in the next 3 block C tasks. P2: record a second source for each key fact. P3: write the falsifiable hypothesis before every test and keep the 45-minute discipline. P4: restate the root cause at the control level for 3 earlier findings and add an alternatives table | the next 3 block C records show the habit, with timestamps | next Friday Part 2 |
| Part 3 below 18 | D, defensive | D1: write the expected event and show the source enabled before each defensive test. D2 and D3: a positive and a near-miss negative case for each control. D4: a mitigation with side effects, rollback and residual risk | the next 3 block D records show them | next Friday Part 3 |
| Part 4 below 12 | E, evidence | rewrite the Friday report to the week's evidence checklist; follow it cold, after 24 hours or by another person, and fix each deviation; hash the bundle | a second run from the report alone, with deviations recorded | next Friday Part 4 |
| Total below 80, no gate failure | W, week | the tracks of every part below its target. The week is recorded NOT MET and the schedule continues | as above | midpoint or final |
| Safety gate failure | S, safety | stop. Follow `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`. No offensive exercise until the scope and the isolation state are re-confirmed in writing. The next 3 offensive exercises are run with the command log reviewed by another person before and after | the written re-confirmation and 3 reviewed exercises | the next scored attempt |
| Integrity gate failure | I, integrity | three consecutive lab records are traced line by line to raw artifacts by a second reader, or by you after 24 hours | three clean traces | the next scored attempt |
| Blocked lab | B, blocked | not a failure. Record the substitution. Schedule the deferred exercise in optional time once the blocker is resolved | the exercise is done and recorded, or reported as not demonstrated | the midpoint or final |
| The same part below target on two consecutive Fridays | R, repeat | PROPOSED: the domain returns to the Guided track (student guide, section 9) until a Friday meets the target | a Friday that meets it | that Friday |

## 10. Mapping the plan's score to the application's rubric

The application grades each attempt on six criteria with weights that sum to 100: `independent_execution` 25, `root_cause_reasoning` 20, `remediation_verification` 20, `detection_visibility` 15, `evidence_documentation` 10 and `scope_compliance_reporting` 10, the last one a critical gate (`crates/g9-domain/src/assessment.rs::BaselineRubric`, VERIFIED by reading). Each criterion gets a level 0 to 4, which the application maps to 0, 25, 50, 80 and 100 percent of its weight (`crates/g9-assessment/src/rubric.rs`, `docs/adr/ADR-0012-assessment-pass-rule-and-reliability.md`).

### 10.1 What the application passes

An attempt passes only if all three hold:

1. the weighted total is at least 80;
2. every one of the six criteria reaches level 2, at least 50 percent of its weight (its pass level);
3. no gate failure is recorded (the four codes of section 5).

A critical-gate criterion fails exactly when it misses the floor. This is stricter than the plan's rule of 80 of 100, which has no per-criterion floor and treats the category minima as diagnostic targets. The plan's 100-point score is recorded on the weekly scorecard and is not replaced.

| Levels reached (weights in brackets) | Weighted total | Every criterion at level 2 or more | Application result |
|---|---:|---|---|
| all six at level 3 | 80.00 | yes | PASS, with no margin |
| five at level 3, `scope_compliance_reporting` at level 4 | 82.00 | yes | PASS |
| all at level 4 except `independent_execution` (25) at level 2 | 87.50 | yes | PASS |
| five at level 3, `detection_visibility` (15) at level 2 | 75.50 | yes | FAIL: total below 80 |
| all at level 4 except `detection_visibility` at level 1 | 88.75 | no | FAIL: one criterion below its floor, although the total is above 80 |
| any levels, one recorded gate failure | any | any | FAIL |

### 10.2 The mapping

The application's six criteria are performance criteria. The 20 points of Part 1 have no counterpart and stay on the scorecard only. The levels are derived from the sub-criterion points of the rubrics (PROPOSED mapping; the plan states no mapping).

| Plan part | `independent_execution` (25) | `root_cause_reasoning` (20) | `remediation_verification` (20) | `detection_visibility` (15) | `evidence_documentation` (10) | `scope_compliance_reporting` (10, gate) |
|---|---|---|---|---|---|---|
| 1 Knowledge (20) | none | none | none | none | none | none |
| 2 Practical (40) | P2, P3 | P4 | none | none | none | P1 |
| 3 Defensive validation (25) | none | none | type F: D2, D3, D4. Type T: D4 | type F: D1. Type T: D1, D2, D3 | none | none |
| 4 Evidence and reporting (15) | none | none | E3 | none | E1, E2 | none |

P1 is the plan's scope and design correctness; P2 enumeration and evidence fidelity; P3 independent hypothesis and reproduction; P4 impact and root-cause reasoning; D1 correct data source and hypothesis; D2 positive validation; D3 negative testing; D4 hardening and mitigation assessment; E1 reproducibility; E2 precision and provenance; E3 remediation and retest. Part 3 is type F when the week's practical validates a fix or a control, and type T when it validates a detection or a telemetry source; the week's task statement says which. If it does not say, use type F.

The sets and their maxima:

| Criterion | Set | Maximum |
|---|---|---:|
| `independent_execution` | P2 + P3 | 25 |
| `root_cause_reasoning` | P4 | 10 |
| `remediation_verification` | type F: D2 + D3 + D4 + E3; type T: D4 + E3 | 22 or 10 |
| `detection_visibility` | type F: D1; type T: D1 + D2 + D3 | 8 or 20 |
| `evidence_documentation` | E1 + E2 | 10 |
| `scope_compliance_reporting` | P1 | 5 |

Rule from points to a level:

| Share of the set's maximum | Level | Score of the weight |
|---|---:|---:|
| 100 percent | 4 | 100 |
| 80 percent or more, below 100 | 3 | 80 |
| 50 percent or more, below 80 | 2 | 50 |
| 25 percent or more, below 50 | 1 | 25 |
| below 25 percent | 0 | 0 |

- Level 4 needs every point of the set, which includes the verification item of each sub-criterion.
- A secondary read may lower a level, never raise it, and only with a written reason: for example, a root-cause statement that depends on an event that D1 shows was never collected lowers `root_cause_reasoning`. Other useful secondary reads: P1 against `independent_execution`, P2 against `evidence_documentation`, E2 against `scope_compliance_reporting`.
- Any gate failure is recorded as such, whatever the levels.
- A condition of section 5 that has no gate code sets `scope_compliance_reporting` to level 0 or 1.
- The levels are what the owner enters in the window. The application computes the total and the decision.

The worked cases with synthetic numbers are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`, section 11. In its example A the plan total is 85.0, the plan gate is met, and the application's total is 84.75 but its decision is FAIL because `detection_visibility` has level 1.

### 10.3 What is recorded where

| Record | Holds | Where |
|---|---|---|
| the plan's 100-point score and its parts | the knowledge, practical, defensive and evidence points, the targets, the plan gate | the weekly scorecard `docs/curriculum/v2/TEMPLATES/WEEKLY_SCORECARD.md` |
| the application's grade | six levels, the weighted total, gate failures, the decision, the reliability class | the Assessments screen of the window (steps: review evidence, grade, approve final) |

The two are reported side by side and are never averaged or converted into each other. The plan's gate is the course's gate; the application's decision is a second, stricter record, and the competency evidence in the application follows its decision. Never describe a week as passed without naming which record. When they disagree, state the reason on the scorecard.

Two points about the application's own design: the baseline rubric makes 45 percent of its weight machine-checkable (`independent_execution` and `remediation_verification`), below the design target of 60 percent (`crates/g9-assessment/src/rubric.rs`); and no deterministic checker exists for these practicals, so every level is entered by a person and, in a one-operator installation, is self-adjudicated. A checker for the fixture-based parts is PROPOSED.

## 11. Commands proposed for review

Proposed text only. None was run. Each needs your review and your approval of the exact text before it is run.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `python3 -I scripts/build_curriculum_v2.py --week N` (N is the week number) | the week's files in the working tree | check the week's day guides, week guide, labs, assessment, keys and registry against the formats of `docs/curriculum/v2/AUTHORING.md` | exit status 0 and no problem line when the week is valid; one line per broken rule otherwise (exit statuses from the script's own header; the line format is NOT RUN) | none: it reads files, compares no generated file for a single week and writes nothing | ordinary user | the repository working tree | none needed (read-only) |
| 2 | `python3 -I scripts/build_curriculum_v2.py --write` | the generated files of the courseware | regenerate every generated file after a registry or guide change | the generated files rewritten; exit status 0 when all weeks are valid | overwrites generated files under `docs/curriculum/v2/` and `data/curriculum/`; the diff shows the change | ordinary user | the repository working tree; run only when every week is meant to be valid | restore the previous generated files from version control, which the owner commits |
| 3 | `sha256sum <file>` | a file you name: the locked answers, the evidence bundle archive, a fixture map | record a hash before a key is opened and when a commitment is made | one line with 64 hexadecimal digits and the file name | none | ordinary user | files you created in your working folder | none needed (read-only) |
| 4 | `date -u +%Y-%m-%dT%H:%M:%SZ` | the host clock | write a UTC time beside a hash or a key opening | one line, a UTC time | none | ordinary user | the host clock; the line records the clock reading and its sync state is separate | none needed (read-only) |
| 5 | `python3 -I scripts/check_doc_refs.py` | the documents of the repository | check that every repository path named in a document exists | exit status 0 and a count of references checked, or one line per broken reference | none | ordinary user | the repository working tree | none needed (read-only) |

## 12. Departures from the plan, interpretations and open points

| Item | What this guide does | Why |
|---|---|---|
| Half points in Part 1 | a constructed-response question may score 1, 0.5 or 0, with a maximum of 20 | the plan says 1 point per question; a partly right answer needs a score that shows the gap. PROPOSED |
| Two layers in the weekly gate | the Friday score and the week's own conditions must both hold | plan sections 5 and 9.1 each state a gate and neither says it replaces the other. INFERRED |
| Gate lines that name a percentage ("at least 80% quiz" in week 1, "at least 80%" in week 3, "at least 80% knowledge" in weeks 7 and 8) | read as the knowledge exam at 16 of 20 unless the week guide says another measure | the plan's section 9.1 calls the category minima diagnostic targets, which sits uneasily with those gate lines; the conflict is reported, not resolved silently |
| No separate point split for the midpoint and the final | the four parts and their sub-criteria are kept | the plan gives none. PROPOSED |
| Final evidence is produced on days 57 to 59 and scored on day 60 | the unseen-target protocol applies from day 57 | the plan schedules the unknown target on day 57 and the final on day 60 |
| Unseen practical protocols | the three protocols, the commitment hashes and the pool are additions | the plan requires unseen or modified scenarios and forbids treating a self-seeded environment as unseen; it gives no method for one person. The method follows the repository's earlier curriculum design, `docs/curriculum/design.md` |
| Protocol 3 for the midpoint and the final | not admissible | a self-made script cannot produce an unknown target (plan, week 12 fairness rule) |
| The plan's failures that have no exact application gate | mapped to the nearest gate, or the scope criterion is set below its floor | the application enumerates four gates (ADR-0012) |
| `TIME_LIMIT_EXCEEDED_WITHOUT_SUBMISSION` | honored | not in the plan; the application's rule is stricter and is not changed (ADR-0023, decision 8) |
| The 20 knowledge points | no application criterion | the six baseline criteria are performance criteria; the points stay on the scorecard |
| The mapping of sub-criteria to criteria and the share-to-level rule | defined here | the plan gives none. PROPOSED |
| Time for scoring | the plan's 60 minutes for scoring and the report may not suffice | an ESTIMATE of 70 minutes is in section 2; overflow is recorded and is not credited as study. Open |
| Instructor preparation (source checks, preparing a practical, key custody) | outside the 480 counted hours | the plan counts none. Open |
| Second reviewer in the window | whether it can be recorded is not established | check the window; the scorecard records the reviewer's name meanwhile. PROPOSED check |
| A registry value for "opened and read by the owner" | none exists in the schema | `SEARCH_CHECKED` is the strongest label; PROPOSED schema change |
| Certification facts (342 and 189 hours, the 2026-12-01 OSCP+ scoring change, the AI restriction at that exam, the CPTS prerequisite) | stated only as the plan states them, ASSUMED, with a pointer to `docs/curriculum/v2/RESOURCE_REGISTRY.md` | the week registries record them by search result only, and in places disagree; the registry entry wins over this file |
