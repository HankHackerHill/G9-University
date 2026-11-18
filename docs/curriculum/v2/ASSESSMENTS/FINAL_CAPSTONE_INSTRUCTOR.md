# G9 Cybersecurity Mastery v2.0: final capstone, instructor file

**INSTRUCTOR ONLY. Not part of the learner's view.** The separation of this file from the learner files prevents accidental exposure. It is not access control: learner and instructor are one person and the files share a repository (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 1). Open it only as section 1.1 allows, and log each opening.

- Reader: the owner, in the roles of preparer and scorer. The learner file is `docs/curriculum/v2/ASSESSMENTS/FINAL_CAPSTONE_STUDENT.md`.
- Source of requirements: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 0.3, 5 week 12 (the five days, the fairness rule, the minimum final artifacts, the gate, the expansion points), 6 (week 12 lab row), 8, 9.1 to 9.4, 11 and 14; the marking bands of `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`; the process of `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`; the rules of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, `docs/curriculum/v2/FIXTURES/README.md` and `docs/curriculum/v2/LAB_ARCHITECTURE.md`; the templates in `docs/curriculum/v2/TEMPLATES/`.
- Status: PROPOSED, written 2026-11-18. No lab was built, no scenario was prepared, no attempt was scored, nothing was run. Every command is proposed text. Everything under "Sample (synthetic)" is invented to show a format; it is not a map, a result or a recommendation of any real configuration.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN, ESTIMATE.
- Placement. The plan's layout (section 0.1) puts this file in `docs/curriculum/v2/ASSESSMENTS/`. `docs/curriculum/v2/README.md` says course-level instructor material lives in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` so that a leakage check can work by folder. The path was fixed by the task that produced this file, and the difference is reported as departure D-FI-01 and open point OP-FI-03. A folder-based leakage check must treat this file as instructor material.

## 1. Custody, roles and what is read when

### 1.1 Who reads which section, and when

| Sections | Read by | When | Why |
|---|---|---|---|
| 1 to 5 (custody, preparing the known lab, the unknown target, the fixture-map format, the synthetic example) | the preparer | before week 12 and before sealing the real map | they describe how to prepare; the example is synthetic and holds no part of the real map |
| 6 to 10 (scoring, checks, detection and agent expectations, the score sheet) | the scorer | after the attempt is submitted and the answers hash and the bundle hash are recorded | they name what the examiner looks for. A learner who reads them first has read the checks of the traps; that is a declared key opening (section 1.3), not a gate failure |
| 11 to 13 (shortfalls, commands, departures) | either role | any time after the attempt is submitted | process |

### 1.2 One person in three roles: what is done about it

The learner is also the preparer and the scorer. The plan says an environment you seeded yourself is not an unseen exam (week 12). The controls, in order of strength:

| Control | Rule |
|---|---|
| Another person prepares K (protocol 1) | they build or configure the instructor-held part, seal the map, and give the learner only its SHA-256 and the scenario cards. They run the preparer's pre-flight (section 2.3). The strongest option |
| A sealed script prepares K (protocol 3) | admissible for K only, never for U. The script's catalogue is not secret; the draw and the parameters are. The seed is produced by a physical die or by another person and is written only in the sealed file. A file with restrictive permissions prevents accidental reading and nothing more: say so on the scorecard (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 8.3) |
| Separation in time | PROPOSED, an ESTIMATE of enough forgetting and no more: the real map is sealed at least 7 days before day 58, that is by Wednesday 2027-01-27, and is not reread by the learner role before the attempt is submitted |
| Commitment | the SHA-256 of the sealed real map is written on the scorecard before it is used (command 1 of section 12). The map itself is not in the repository (D-FI-02) |
| Review by another person | where another person can read the evidence bundle and this file, they score or check, and the scorecard names them. A grade without that is self-adjudicated and never validates a competency in the application (ADR-0012) |
| Honesty about limits | the scorecard says which protocol was used for K and for U, who prepared, and what the preparer knew |

### 1.3 Key log

Every opening of this file, of the real map, of a week 12 key or of the entry diagnostic key is logged on the scorecard with the time, the file, the section and the reason. An unlogged opening is an integrity matter. An opening before the attempt is submitted is declared: the items seen score 0 and P3 is in the none band for the work they touch (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 6).

## 2. Preparing the known lab (K) without spoiling the unknown target (U)

### 2.1 Separation rules

K is known by design, so the risk is not that K is spoiled. The risk is the reverse: that preparing K leaks, or is shaped by, what U contains.

| ID | Rule | Reason |
|---|---|---|
| S-01 | K is built from the course's class catalogue (the learner file, section 3.2) and from the course's own labs of weeks 3 to 10. Nothing in K is copied, recreated or adapted from a platform exercise, active or retired | a recreated platform exercise would be a walkthrough in a lab, and would contaminate U if a pool entry resembles it |
| S-02 | The preparer reads no description, hint, write-up, thread or video of any entry of U's pool while preparing K, and does not choose K's classes to resemble or avoid a pool entry | the pool entries must stay unknown to the preparer role as well |
| S-03 | Two commitments are written on the scorecard before day 57: the hash of the K map (by 2027-01-27) and the hash of U's pool (by 2027-01-29). Neither file changes afterwards | the order of the two is fixed so that neither can be tuned to the other |
| S-04 | U is drawn only at the handover on day 57, block B, 10:45 | `docs/curriculum/v2/ASSESSMENTS/MIDPOINT_OFFENSIVE_BENCHMARK.md`, section 5.4 |
| S-05 | The scenario cards contain the action, the guest, the account and the cleanup. They contain no event id, no alert name, no expected field, no expected delay and no label such as attack or legitimate | an expected-telemetry hint would answer D1 |
| S-06 | The parameters of K (counts, account and task names, thresholds, which collection gaps) are drawn at sealing time by a physical die or by another person and recorded only in the sealed map | the instance is partly unknown even to the learner role |
| S-07 | The W12 baseline of K differs from the week 7 baseline in at least one instructor-held logging item and at least one instructor-held control gap | the learner built the week 7 logging; the capstone must still require verification of what is configured now |
| S-08 | K is never presented or scored as unseen. Its findings are `known-lab` and do not count toward the independent quota | plan fairness rule |
| S-09 | Scenario actions are benign: built-in administration features, course-authored no-op programs, synthetic accounts, no real malware, no exploit code for any product, no stress test. A scenario that needs a RED to CORP or CORP to DMZ path uses a time-boxed exception with its removal step and retest | N-08, N-14, N-17; LAB_ARCHITECTURE, section 4.3 |
| S-10 | Nothing derived from the day 58 or 59 evidence is carried into U's reproduction on day 60 beyond the learner's own report | the reproduction must follow the report alone |

### 2.2 Preparation timeline

All of it is outside the counted hours. The dates are the original calendar.

| By | Action | Who | Output |
|---|---|---|---|
| Fri 2027-01-22 | confirm that Stage 00 is resolved, that baseline B1 exists and that T1 to T9 passed for the full stack (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, sections 11 to 13). If not, record K as BLOCKED now and use the substitutions of the learner file, section 7 | owner | state record |
| Wed 2027-01-27 | select the K scenarios from the catalogue, draw the parameters, write the real map in the format of section 4, build the instructor-held configuration, write the scenario cards, seal the map, write its hash on the scorecard | preparer | sealed map outside the repository; hash; cards |
| Thu 2027-01-28 | the preparer's pre-flight (2.3) on a copy of the baseline | preparer | pre-flight record in the sealed map, section M9 |
| Fri 2027-01-29 | fix the pool for U with the eligibility checks of the midpoint file; write its hash on the scorecard; decide protocols | learner role, or the preparer under protocol 1 | pool file; hash |
| Fri 2027-01-29, evening | take the W12 baseline set (LAB_ARCHITECTURE, section 12.1); check free memory against the full stack plus the margin; open the scorecard row | owner | baseline ids; scorecard |

### 2.3 The preparer's pre-flight

Run once, on a copy of the baseline and never on the learner's set (S-10 and the weekly lab). The point is to confirm that the scenario is solvable, safe and measurable, and that no instructor-held item can be mistaken for a fault of the lab.

| Check | Pass |
|---|---|
| each card's action runs on the stated guest with the stated account and cleans up | the cleanup leaves the baseline state (compare to the map) |
| expected telemetry (M3 of the map) appears at the host and at the collector within the stated delay | yes, or the map's expectation is corrected before sealing |
| each collection gap is real: the expected event is absent for the stated reason | yes |
| each exception is requested, used, removed and retested (T2 or T3 as the exception implies) | the retest passes |
| each control gap has a correct remediation and the regression tests of M6 pass and fail as predicted | yes |
| the whole replay fits the learner's blocks: B (120 minutes) for the replays and C (150 minutes) for the investigation, with margin | ESTIMATE; record the times |
| memory, disk and CPU stay within the host rule | yes |
| no guest reached the household LAN or the Internet (T5 to T7 repeated after the run) | PASS |

Nothing from the pre-flight is run without the owner's approval of the exact text. The commands are in the lab files, not here.

### 2.4 Scenario cards

A card has these fields and nothing else: card id; class from the catalogue; guest id; account (synthetic); the action in plain words; the exception it needs, if any, named by the exception id of LAB_ARCHITECTURE; cleanup. The learner records the start and end of each action in UTC and in local time with the clock source.

## 3. Preparing the unknown target (U)

| Item | Rule |
|---|---|
| Pool | fixed and hashed by 2027-01-29; at least 5 eligible entries; the eligibility checks E-01 to E-10 of the midpoint file; protocols 1 and 2 only; no entry the learner has started, including the midpoint's drawn target; no guided room |
| Draw and handover | at 10:45 on day 57 only, with the record of the midpoint file, section 5.4. The preparer of protocol 1 reads the candidate's page and says nothing about it |
| Window | 10:45 to 12:45 and 13:45 to 16:15, 270 minutes. No contact in the break. Last contact 16:10 |
| The examiner's reference | for U there is no fixture map and no key. The reference is the learner's evidence. After the bundle hash is recorded the scorer may read the platform's own page (name, system, difficulty, retired state) to confirm the eligibility record. The scorer reads no third-party write-up before scoring and never uses one to overrule the learner's evidence. A write-up read after scoring is declared and the target is consumed |
| What "the target supports" means | the examiner decides it from the evidence: the inventory covers the services observed; each lead is classified; refuted hypotheses are recorded with their controls; a reasonable next test is named. A lead left Unconfirmed with no test attempted does not show that the target does not support a finding |
| Day 60 | the learner reproduces the high-value results from the report alone, on a fresh instance under a new scope line. A different result on reproduction is data, and its handling is in section 7 |

## 4. The fixture map for K: format

The real map is one sealed text file outside the repository. Its hash is committed before use (S-03). It is never written into a learner file, and never next to a question. It has these sections. A map missing any section is not sealed.

| Section | Holds | Rule |
|---|---|---|
| M0 header | map id; set id (`FX-C03` of the register in `docs/curriculum/v2/FIXTURES/README.md`, section 8); protocol (1 or 3); preparer; date sealed; status AUTHORED, COMMITTED, OPENED or USED; the commitment hash and the place it was written | the file is immutable once COMMITTED; a correction is a new map id and the old one is kept |
| M1 environment | stack and guest ids; baseline ids; the router export hash (the policy version); the clock plan: each guest's zone, how its clock is kept, and the measured offsets at preparation; the logging configuration at the start of the week: audit settings, the Sysmon configuration id, the collector configuration, and which sources are intentionally off; the exceptions the scenarios may request and their removal tests | offsets are measured values with their uncertainty, not assumptions |
| M2 seeded items | one row per seeded item: item id; kind (GAP a control gap, SCENARIO an action, COLLECTION a collection gap, LOOKALIKE a benign look-alike, PATH an identity or privilege path); guest; location; seeded state; how seeded (recipe id; profile instructor-held); card id or none; actions permitted to the learner; risk if mishandled | the learner-visible manifest names the generator, its version and the seed, and says "profile: instructor-held" (FIXTURES, section 3.5) |
| M3 expected telemetry | one row per scenario: the action as on its card; expected source family and event ids; the fields that matter; the configuration it depends on; expected at the host (yes or no, and why); expected at the collector; expected delay; cross-sources (router, service log, packet capture); what would be missing and the reason | event families are named correctly: a native Security event is not a Sysmon event |
| M4 look-alikes and negatives | each look-alike: the scenario it pairs with; the one variable in which it differs; the expected outcome for a correct rule | at least one near miss and one legitimate look-alike per detectable behavior |
| M5 path register | each path: ordered edges with the preconditions and the expected outcome (ALLOWED or DENIED); the intended classification; the trap | at least one DENIED edge, so that reachability in a model is not achieved access (plan 9.3, day 50) |
| M6 remediation and regression | per control gap: the correct remediation; a plausible wrong remediation and why it is wrong; the positive regression; the negative regression; the bypass attempt of the same class; the side effects and the rollback | |
| M7 timeline skeleton | the expected events in UTC order with their sources, the seeded clock offsets, the corroborating and the dependent copies (a SIEM copy is not independent of the host log it was collected from), and the claims the evidence supports and the overclaims it does not | |
| M8 scoring hooks | item to rubric sub-criterion and its maximum; what a full record looks like | |
| M9 integrity | the recipe and generator versions, the pre-flight record, the places where expected hashes of generated artifacts are recorded (the instructor key or the evidence record, not a manifest: FIXTURES, section 2 rule 3) | |
| M10 words to keep off the cards | the event ids, alert names and expected fields that must not appear on a card | |

## 5. One complete synthetic example of a K map

Sample (synthetic): an invented map for an invented set `FX-C03-EX`. No guest was built. The ids, names, counts and times are not those of any real map. Event families and settings are written as the preparer would write them; the product behaviors named are ASSUMED and are checked against the week 3 and week 7 registry entries (`w03-ms-advanced-audit-policy`, `w07-ms-audit-policy-advanced`, `w07-ms-event-4688-cmdline`, `w07-ms-sysmon-events`) by the preparer before sealing. Accounts are `lab.user1` (standard), `lab.user2` (standard, member of a nested group), `lab.admin` (administrative) and `svc.lab` (a synthetic service account); credentials are `<synthetic-password>`.

### M0 header

```text
map id: SYN-FIXMAP-K-EX1    set id: FX-C03-EX    protocol: 1 (another person)    preparer: <name or role>
sealed: 2027-01-27T17:00 America/Chicago (synthetic)    status: COMMITTED
commitment: SHA-256 of this file recorded on the scorecard at <time>, hash <64 hexadecimal digits>
scope: K only. Never U.
```

### M1 environment

| Item | Value (synthetic) |
|---|---|
| Stack | `G9-RTR-01`, `G9-ATK-01`, `G9-TGT-01`, `G9-DC-01`, `G9-WKS-01`, `G9-SIEM-01`; baselines `<guest id>-B1-<date>` and the week set `<guest id>-W12-20270129` |
| Policy version | router export hash `<64 hexadecimal digits>`; exceptions E3 (RED to CORP) and E5 (CORP to DMZ) requestable, removal tested by T2 and by T1 negative check |
| Clock plan | Linux guests and the router keep UTC. Windows guests keep local time, America/Chicago, UTC-6 in February. Measured offsets at preparation (synthetic): `G9-WKS-01` ahead of `G9-SIEM-01` by 2 seconds, `G9-DC-01` ahead by 0 seconds, uncertainty plus or minus 1 second |
| Logging at the start | Security audit: Logon (success and failure), User Account Management (success and failure), Other Object Access Events (success) on `G9-WKS-01`; Process Creation on, with the "Include command line in process creation events" setting not configured (collection gap C-02). Sysmon on `G9-WKS-01` with a configuration whose network connection event (id 3) has no rules (collection gap C-01), file create (id 11) on for the directory of S-01. The collector reads the Security and Sysmon channels of `G9-WKS-01` and the Security channel of `G9-DC-01`; ASSUMED ingestion delay at most 60 seconds (ESTIMATE, measured at pre-flight) |

### M2 seeded items

| Item | Kind | Guest and location | Seeded state | Card | Permitted actions |
|---|---|---|---|---|---|
| S-01 | GAP | `G9-WKS-01`, directory `C:\SynApps\SynSvc\` | the group of ordinary domain users has Modify on the directory that holds the binary of the synthetic service `SynSvc`, which runs under a more privileged synthetic account | none | read the ACL; write a marker file as `lab.user1`; never replace the binary |
| S-02 | PATH | `G9-DC-01`, directory objects | `lab.user2` is a member of `SYN-HelpDesk`, nested in `SYN-ServerOps`, which holds a delegated reset-password right over `OU=SynServers,DC=corp,DC=g9,DC=test`; `svc.lab` is in that OU and is a local administrator of `G9-WKS-01`; `lab.admin` is in another OU | none | read the relationships; reset the synthetic password of `svc.lab` as `lab.user2` once, with the old value recorded for rollback; attempt the same for `lab.admin` |
| S-03 | GAP | `G9-DC-01`, domain policy | account lockout threshold is 0 (never lock out) | none | read the policy; the card SC-02 exercises it |
| S-04 | GAP | `G9-TGT-01`, container `syn-status` | a synthetic status page listens on all interfaces on port 8081 and returns a synthetic configuration dump containing `<synthetic-password>`; the allowed lab service listens on 8080 | none | request the page from the guest named on card SC-04 |
| C-01 | COLLECTION | `G9-WKS-01`, Sysmon configuration | no rule for event id 3 | none | read and, under change approval, edit the configuration |
| C-02 | COLLECTION | `G9-WKS-01`, audit setting | command line in process creation events not collected | none | read and, under change approval, change the setting |
| SC-01 | SCENARIO | `G9-WKS-01`, `lab.user1` | write a marker file `marker-01.txt` into the directory of S-01 | card 1 | as the card |
| SC-02 | SCENARIO | `G9-ATK-01` to `G9-WKS-01`, account `lab.user2`; needs E3 | six failed network logons within 60 seconds, then one success with the synthetic password | card 2 | as the card; E3 requested and removed with T2 repeated |
| SC-03 | SCENARIO | `G9-WKS-01`, `lab.user1` | create a scheduled task `SYN-TASK-01` that runs a no-op at logon | card 3 | as the card; delete the task in cleanup |
| SC-04 | SCENARIO | `G9-WKS-01` to `G9-TGT-01` port 8081; needs E5 | one request with the built-in HTTP client | card 4 | as the card; E5 removed with the T1 negative check |
| SC-05 | LOOKALIKE | `G9-WKS-01`, `lab.admin` | create a scheduled task `SYN-TASK-ADM` during the approved change window | card 5 | as the card; delete in cleanup |
| SC-06 | LOOKALIKE | `G9-WKS-01` to `G9-SIEM-01` | the collector agent's own connection to the collector, port 1514 | none: happens by itself | observe only |
| SC-07 | LOOKALIKE | `G9-ATK-01` to `G9-WKS-01`, `lab.user2`; needs E3 | three failed network logons within 60 seconds | card 6 | as the card |

### M3 expected telemetry

| Scenario | Source and ids | Fields that matter | Depends on | At the host | At the collector | Cross-sources | What would be missing, and why |
|---|---|---|---|---|---|---|---|
| SC-01 | Sysmon 11 (file create) on `G9-WKS-01` | target file name, image, process guid | the Sysmon rule for the directory (present) | yes | yes within 60 s | none | a native object-access record 4663 is absent: no system access control entry on the directory (not seeded). Its absence is not proof that no write happened |
| SC-02 | Security 4625 six times and 4624 (logon type 3) once on `G9-WKS-01`; on `G9-DC-01` a credential-validation event 4776 per attempt if that audit setting is on (ASSUMED) | target account, logon type, status 0xC000006D with substatus 0xC000006A for a wrong password, source address `10.77.10.10`, time | Logon audit success and failure | yes | yes | router flow log: E3 pass rule hits to `10.77.30.20`; packet capture on the router taps | no lockout event 4740: threshold 0 (S-03). After remediation 4740 on the DC is expected |
| SC-03 | Security 4698 on `G9-WKS-01`; Process Creation 4688 for the task-creation tool with an empty command-line field (C-02) | subject account, task name, task content | Other Object Access Events success | 4698 yes; 4688 yes but the command line is empty | 4698 yes | none | process arguments are not visible until C-02 is corrected; Sysmon process create (id 1) is not configured in this synthetic baseline, so it is absent |
| SC-04 | none at first (C-01). After the Sysmon rule is added: Sysmon 3 on `G9-WKS-01` with destination `10.77.20.20` port 8081 | image, destination address and port, initiated flag | the Sysmon id 3 rule | no, until C-01 is corrected | no, until then | router flow log (E5 rule hit); `G9-TGT-01` service log with source `10.77.30.20` (no address translation); packet capture | the absence of id 3 before the fix is a configuration fact, not proof that no connection happened: the router and the service log show it |
| SC-05 | Security 4698 on `G9-WKS-01` with `lab.admin` as the subject | as SC-03 | as SC-03 | yes | yes | change-window record kept by the preparer | none |
| SC-06 | after C-01 is corrected: Sysmon 3 for the agent process to `10.77.30.30` port 1514, repeatedly | as SC-04 | the Sysmon rule | after the fix, yes | after the fix, yes | none | it is legitimate traffic; a rule that matches it is a false positive |
| SC-07 | Security 4625 three times on `G9-WKS-01` | as SC-02 | as SC-02 | yes | yes | router flow log | none |

### M4 look-alikes and negatives

| Detectable behavior | Positive scenario | Near miss (one variable) | Legitimate look-alike |
|---|---|---|---|
| repeated failed network logons for one account in a short window | SC-02 (six in 60 seconds) | SC-07 (three in 60 seconds: the count differs) | not seeded for this behavior; the learner is expected to propose one |
| scheduled task created by an account outside the administrative workflow | SC-03 | a task modified and not created (the event id differs); the learner may need to generate it | SC-05 (same event id, administrative subject, change window) |
| network connection to the status port of the DMZ target from the workstation | SC-04 | the same connection to port 8080 (the port differs), if a card or the learner's own action produces it | SC-06 (a connection in the same event family to the collector, port 1514) |

### M5 path register

| Path | Edges (synthetic) | Preconditions and expected outcome | Intended classification | Trap |
|---|---|---|---|---|
| PATH-1 | e1 `lab.user2` is a member of `SYN-HelpDesk`; e2 `SYN-HelpDesk` is a member of `SYN-ServerOps`; e3 `SYN-ServerOps` holds the reset-password right over `OU=SynServers`; e4 `svc.lab` is in that OU; e5 `svc.lab` is a local administrator of `G9-WKS-01`; e6 the same right over `lab.admin` | e1 to e4: ALLOWED when exercised once (a reset is recorded by event 4724 on the DC). e5: ALLOWED, read from the local group. e6: DENIED, because `lab.admin` is outside the delegated OU | DEMONSTRATED for e1 to e4 when the reset is exercised; MODELLED for e5 unless the learner exercises it; REFUTED for e6 | a graph drawn from group nesting alone suggests e6; the scope of the delegation breaks it |

### M6 remediation and regression

| Gap | Correct remediation | Plausible but wrong | Positive regression | Negative regression | Bypass attempt of the same class | Side effects and rollback |
|---|---|---|---|---|---|---|
| S-01 | remove the write entry of the ordinary-users group from the directory and keep the entry the service and the administrators need | only deny the marker file name (the symptom), or move the service (the cause stays) | the service still starts; the administrators can still update the directory | `lab.user1` cannot write a new file; the repeat of SC-01 produces no file | write under another name; write through a different path to the same directory | the service might fail to read its own files if an entry is removed too broadly; restore the saved ACL (a saved ACL export is required before the change) |
| S-02 | remove the nesting of `SYN-HelpDesk` into `SYN-ServerOps`, or narrow the delegation to the intended OU, and re-evaluate | remove `lab.user2` from the group only (the nesting remains for other members) | a help-desk member can still reset a password in the intended OU | `lab.user2` can no longer reset `svc.lab` | reset through a second delegated group if one exists | a legitimate help-desk function is lost if the wrong edge is removed; recover through the domain-controller recovery procedure of LAB_ARCHITECTURE, section 10, not a snapshot |
| S-03 | set a lockout threshold in the policy that applies to the lab accounts | add a longer password (does not address guessing) | `lab.user2` with the right password can log on | the sixth bad attempt in the window locks the account (event 4740 on the DC) | the same burst against a second account | a legitimate user can be locked out by error: record the observation window and unlock procedure |
| S-04 | stop the status page from listening on all interfaces, or restrict it by a rule on the target guest, and remove the exception E5 afterwards | block the port on the router only (the service stays exposed inside the zone; the intra-zone path is not through the router) | the allowed lab service on 8080 still answers | the request to 8081 from the workstation fails | the same request by address of another interface of the target | none expected for 8080; rollback is the previous listener setting |

### M7 timeline skeleton (SC-04, in UTC, synthetic times)

| Order | UTC | Source | Event | Independence |
|---|---|---|---|---|
| 1 | 2027-02-03T19:41:07Z | router flow log | pass rule E5 hit, `10.77.30.20` to `10.77.20.20` port 8081 | independent |
| 2 | 2027-02-03T19:41:07Z to 19:41:08Z | service log on `G9-TGT-01` | request from `10.77.30.20` for the status page | independent |
| 3 | 2027-02-03T19:41:07Z after correcting the measured 2 second offset (the log shows 13:41:09 local, which is 19:41:09Z uncorrected) | Sysmon 3 on `G9-WKS-01`, after C-01 is corrected | connection by the HTTP client image | the host log |
| 4 | within 60 s after 3 | collector record | the same event, indexed | dependent on 3: a copy |

Supported claims: a connection occurred (rows 1 and 2, independent); it came from the workstation address (rows 1 and 2); the process image is the HTTP client (row 3, one source, so MEDIUM). Overclaims: that the user intended to read the configuration; that the collector record is corroboration of row 3; that a Sysmon time before C-01 was corrected exists.

### M8 scoring hooks

| Item | Sub-criteria | A full record shows |
|---|---|---|
| S-01, S-03, S-04 remediations | D4, E3 | the cause named at the control; side effects; rollback; residual risk; a positive and a negative regression dated after the change |
| PATH-1 | P2, P3, P4, E1 | edges labelled and tested; e6 shown DENIED with evidence; the difference between the graph and achieved access |
| SC-02, SC-03, SC-04 detections | D1, D2, D3 | section 8 |
| C-01, C-02 | D1 | the missing source found, the configuration read, the change approved, the event seen after |
| clock offsets | P2, E2 | offsets measured from paired events, with uncertainty |

### M9 and M10 (synthetic)

```text
M9 integrity: recipes REC-K-01 to REC-K-06 (generator <name and version>, profile instructor-held);
pre-flight run record <id> dated 2027-01-28; expected hashes of generated artifacts are recorded in
the evidence record after the owner's approved generation run, not in any manifest.
M10 keep off the cards: the numbers 4625, 4624, 4698, 4688, 4776, 4724, 4740 and the words
"failed logon burst", "scheduled task persistence", "Sysmon", "lockout".
```

### Scenario cards for the example (what the learner would receive)

```text
Card 1  class: file activity        guest: G9-WKS-01   account: lab.user1
        action: create a text file named marker-01.txt in C:\SynApps\SynSvc\ ; cleanup: delete it
Card 2  class: authentication       guest: G9-ATK-01 to G9-WKS-01   account: lab.user2   exception: E3
        action: six network logon attempts with a wrong synthetic password within one minute, then one with the right one ; cleanup: none beyond E3 removal
Card 3  class: scheduled work       guest: G9-WKS-01   account: lab.user1
        action: create a scheduled task named SYN-TASK-01 that runs a no-op at logon ; cleanup: delete the task
Card 4  class: network connection   guest: G9-WKS-01 to G9-TGT-01 port 8081   exception: E5
        action: one request with the built-in HTTP client ; cleanup: E5 removal
Card 5  class: scheduled work       guest: G9-WKS-01   account: lab.admin   (an approved change window)
        action: create a scheduled task named SYN-TASK-ADM that runs a no-op ; cleanup: delete the task
Card 6  class: authentication       guest: G9-ATK-01 to G9-WKS-01   account: lab.user2   exception: E3
        action: three network logon attempts with a wrong synthetic password within one minute ; cleanup: none beyond E3 removal
```

## 6. Scoring guide for each minimum artifact

The bands are those of `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`; this section does not change them. It says where each plan artifact is read and what decides its band.

### 6.1 Where each artifact is scored

| Artifact (plan, week 12) | Environment | Scored in (GRADING_RUBRICS, section 9) |
|---|---|---|
| 1 rules of engagement; 2 verified isolation | all; K | P1 and the scope gate; isolation is a gate prerequisite (G-4) |
| 3 accurate inventory | U and K | P2 |
| 4 at least two independently evidenced findings | U | P2, P3, P4, E1 |
| 5 one identity or privilege path | U, or K labelled known-lab | P2, P3, P4, E1 |
| 6 at least three tested detections | K | D1, D2, D3 |
| 7 one multi-artifact timeline | K | P2, E2 |
| 8 at least two remediations with regressions | K; U on paper | D4, E3 |
| 9 at least three agent boundary test cases | AGENT | D2, D3 |
| 10 sanitized executive and technical report; 11 unresolved-risk register | all | E1, E2, P4 |
| Part 1 | day 60 | 20 questions; the key of the week 12 `ANSWER_KEYS/ASSESSMENT_KEY.md` |

### 6.2 Aggregation rule

A sub-criterion fed by several artifacts is banded over the set. PROPOSED, an application of GRADING_RUBRICS, section 3 rule 2 to a set:

1. Count an item toward a quota only if it meets the required items of its template. An item that fails a required item is listed but not counted.
2. Full band: every counted item meets every required item of the sub-criterion, and the quota is met. The verification item (V) is met on every counted item for the maximum of the band.
3. Partial band: one counted item fails one required item, or the quota is short by one.
4. None band: two or more counted items fail, or the quota is short by two or more, or no item exists.
5. Take the lower band when the evidence sits between two. Add a point inside a band only where the evidence clearly exceeds its descriptor.

### 6.3 Per artifact: what the examiner checks

**Artifact 1 and 2. Rules of engagement and isolation (P1; gates G-3 and G-4).** Required: a signed RoE and a scope line for each exercise before its first command; fields complete; the time of the first record earlier than the first command log line; for U the terms read and dated; closure records. Full needs the independent confirmation of each target's identity before the first action (the platform page; the lab inventory with T8). Partial: one element missing (exclusions, time limit or stop rule). None: no scope block, a scope block written after the actions, or any out-of-scope action (P1 at most 2 and the safety gate). Isolation records T1 to T9 are judged as a gate, not for points: PASS needs two independent observations each (router or guest view and a host-side capture on the tap), the containment tests first, the policy version recorded and a retest after each exception. Defects often seen: a PASS from the domain definition alone; T5 without a host-side capture; evidence not repeated after a change (a retest trigger of LAB_ARCHITECTURE, section 11.1); the AD variant of T9 skipped; IPv6 not examined. If isolation is NOT RUN and K work was done anyway, STOP-04 fired and the safety gate failed.

**Artifact 3. Inventory (P2).** Required: complete for the scope and the time; each fact has its source and time with zone; limits stated (what filtered or closed does not prove); no claim beyond the artifacts; for the maximum, a second independent source for each key fact. K: compare the inventory with the map's M1 and M2. A fact that conflicts with the map is an error to be traced to the raw record (section 7) or a fabrication. U: no map; the check is internal consistency and the cross-check pairs.

**Artifact 4. Independent findings (P2, P3, P4, E1).** Required for each counted finding: one root cause; preconditions marked OBSERVED or ASSUMED; a hypothesis with its prediction written before the test (timestamped); no hint and no published solution; reproduced from a clean state at the time and again on day 60; a negative control that fails as predicted; impact bounded by the evidence and by exploitability; a status the evidence supports; no flag value in the report. Full on P3 needs R1 to R5 and the second reproduction. A scanner label, a version match or a crash is not a finding. A finding of K is not counted here. If fewer than two findings are counted, apply section 11.

**Artifact 5. Identity or privilege path (P2, P3, P4, E1).** Required: the three registers; every edge labelled; each tested edge with a prediction written before the test; a DENIED edge kept in the graph with its evidence; each path classified DEMONSTRATED, MODELLED or REFUTED; reachability in the graph distinguished from achieved access; the events that should cover each demonstrated edge and whether they were seen at the source and at the collector. Full: the classification matches the evidence. Partial: a path asserted from a tool's graph, with some edges unlabelled. None: a path stated as achieved with no test. In K, a path claiming the map's DENIED edge as ALLOWED is an overclaim, and a fabrication if its evidence cannot be found in the raw records.

**Artifact 6. Detections (D1, D2, D3).** Section 8.

**Artifact 7. Timeline (P2, E2).** Required: at least three artifacts; at least three of independent provenance, where a collector's copy of a host log is not independent of that log; per artifact: provenance, the hash before analysis and at the end, acquisition limits; per claim: the original time with its zone, the UTC value with the offset rule, the confidence, the corroborating and contradicting records, the strongest alternative explanation and the next discriminator; observed and inferred apart; clock offsets measured from paired records, with uncertainty. Common defects: adding times across clocks as if they agreed; normalizing local time without the rule; raising confidence by repeating one source; a missing log written as no activity. Compare with the skeleton M7.

**Artifact 8. Remediations and regressions (D4, E3).** Required for each counted remediation: a change approval record; the before and after states; a root cause named at the control (P4); a positive regression and a negative regression, each with an artifact dated after the change; a bypass attempt of the same class; side effects; rollback; residual risk. A fix that breaks a permitted function is in the none band of D4. A regression artifact older than the change is "retest passed with no artifact": an integrity failure. A remediation of a U finding is a paper recommendation and counts only as E3's correction text, never as an applied remediation.

**Artifact 9. Agent boundary tests (D2, D3).** Section 9.

**Artifact 10. Reports (E1, E2, P4).** Required: the quality checks of the executive and technical report templates; the findings table equal in both; the second run from the report alone (day 60, block C) with deviations recorded (E1's verification item); the evidence manifest and the bundle hash recorded before any key was opened (E2's verification item); claims labelled; severity argued from preconditions and impact. A separate check applies to the portfolio copy: the sanitization record of `docs/curriculum/v2/PORTFOLIO/README.md`, section 5.4.

**Artifact 11. Unresolved-risk register (E2, P4).** Required: each risk with its statement, why unresolved, the evidence gap, an owner, the next discriminating test and a review date. A register that is empty needs a reason. A register copied as boilerplate (risks that do not follow from this work) is partial. A register that contradicts the report's limits lowers E2.

**Part 1.** Mark from the week 12 key: 1 or 0 for multiple choice; 1, 0.5 or 0 for constructed responses by the descriptors of GRADING_RUBRICS, section 4. The locked answers hash and its time precede any key opening.

### 6.4 Which artifacts decide which sub-criterion

| Sub-criterion | Maximum | Decided over |
|---|---:|---|
| P1 | 5 | the scope blocks of all exercises; isolation as the V item |
| P2 | 10 | the U inventory, the K inventory, the timeline provenance, the path record |
| P3 | 15 | the findings and the path: hypotheses, reproduction on day 57 and on day 60, negative controls |
| P4 | 10 | the findings, the path, the severity rationale of day 59 A, the risk register |
| D1 | 8 | the sources of the three detections (and the two collection gaps C-01 and C-02 in the example); the agent audit trace as a source |
| D2 | 7 | the positive fixtures of the detections; the normal-request agent tests; the positive regressions |
| D3 | 5 | the negative fixtures; the denied unauthorized proposals with equal state digests; the negative regressions and the bypass attempts |
| D4 | 5 | the remediations: cause, side effects, rollback, residual risk |
| E1 | 5 | the reports and the second run from the report alone |
| E2 | 5 | labels, artifact table, hashes, the timeline's provenance, the risk register |
| E3 | 5 | the corrections and the retest records |

## 7. The examiner's checks for fabricated or overclaimed results

Trace every doubtful claim to a raw record in the bundle that was hashed before any key was opened. A claim that states an observation present in no raw record is a failure. A slip that the raw record corrects and that changes no conclusion is an error: it costs points in E2 and is not a gate (INSTRUCTOR_GUIDE, section 5). A reconstruction labelled as such is not fabrication.

### 7.1 Checks

| # | Check | How | What a failure shows | Decision |
|---|---|---|---|---|
| X-01 | the hash order | the answers hash, the daily manifests and the bundle hash each have a time earlier than any key opening and earlier than the scoring | evidence could have been edited after reading a key | missing hash: E2 loses its verification point; a later edit is an addendum |
| X-02 | time conversion | America/Chicago is UTC-6 in February 2027 and in December 2026 (ASSUMED standard time; check the zone rule for the date). A report time that disagrees with its source by a whole-hour offset is a conversion error, not an invention | a mistake of zone | error, E2 |
| X-03 | event ordering and identifiers | record numbers of one log increase with time; two events whose numbers and times disagree show an edit or a reconstruction | an edited or rebuilt excerpt | decide from the raw export; a rebuilt excerpt not labelled is a fabrication |
| X-04 | presence of an event that the map says cannot exist | in K, an event that M3 says is absent for a configuration reason (for example the network connection event before C-01 is corrected) appears in the learner's evidence with a time before the change record | a configuration change that is unrecorded, or an invented event | integrity failure unless the change record explains it |
| X-05 | delays | the host-to-collector delay is not zero for every event and is consistent with a measured value; identical delays for all events are suspect | invented or copied timestamps | decide from the raw collector export |
| X-06 | clock offsets | the timeline uses offsets measured from paired records. A timeline that aligns two guests perfectly when the map seeds an offset shows normalization without measurement | an assumed offset | error in E2 or, if the evidence was altered to fit, a fabrication |
| X-07 | replay is not generation | a stored event file replayed to the collector tests the rule and the parser, not the host's production of the event | "the host collected it" claimed from a replay | an overclaim; D1 cannot reach its verification point |
| X-08 | a detection with only a positive | there is no near miss and no legitimate look-alike run | untested | not counted toward the three |
| X-09 | a negative that matched | a negative fixture that matched must appear in the retune log with a rule change and a rerun of every fixture | a false positive hidden by editing | integrity failure if removed without a record |
| X-10 | counts and rates | counts are on named fixtures. A rate or coverage figure from three fixtures is an overclaim | generalization | E2 |
| X-11 | ATT&CK mapping from one id | a technique named from one event id or one field value | assumed technique | D1 partial at best; the template's check requires a second observation |
| X-12 | an attack inferred from a protected-by-design event | for example a ticket event with a weak cipher type declared an attack by itself | a single event treated as proof (plan, weeks 7 and 10) | an overclaim |
| X-13 | agent test with the broker NOT REACHED | the model refused, no proposal arrived, and the record says the broker denied it | the policy was never exercised | the test is not a pass; if claimed as one, integrity |
| X-14 | digests of the scratch store | digests equal before and after a denied call; digests that differ between a normal request and a denied one as predicted; the same digest string reused across tests that should differ | copied or invented digests | fabrication if no raw state export supports them |
| X-15 | model counts | counts of "followed" and "not followed" over runs, with the configuration; a non-deterministic model giving the same output in every run needs an explanation (temperature, a recorded transcript) | invented runs | decide from the transcript or run log |
| X-16 | regression after the change | the regression artifact is dated after the change record and shows the changed state | "retest passed" from an artifact that predates the change | integrity failure |
| X-17 | negative control defined after the positive | file times and log order show the control written after the result | the prediction was reconstructed | P3 partial, and E2; if presented as written before, integrity |
| X-18 | severity from a label | a priority copied from a scanner or from a CVSS number with no metric rationale | borrowed rating | P4 and E2 |
| X-19 | scope and window | every command's target and time lie inside its scope line and window; a lunch-break contact on U is out of window | out of scope | safety gate |
| X-20 | published solution | a step or a phrase that matches a public write-up and no declaration | undeclared exposure | integrity failure |
| X-21 | consistency of tool output with the tool | the output format and fields match the version of the tool the report names (read from the system) | an output typed rather than captured | decide from the raw log; unexplained mismatch is a fabrication |
| X-22 | the reproduction on day 60 | a second run that matches the first in every detail, down to times, on a fresh instance, is suspect; real runs differ in times, identifiers and ordering | copying | integrity if copied; a different result is data and is handled by 7.3 |

### 7.2 Overclaim phrases to reject

| Phrase or pattern | Why it is rejected |
|---|---|
| "secure", "no vulnerabilities" | only "no issue found in the tests run", with the tests named |
| "verified isolation" with no test record, or with the domain definition as the evidence | plan 0.4 and 3.3: the definition is not proof |
| "the detection works" from one positive | not a tested detection |
| "detected" when the event was replayed from a file | replay is not production |
| "the attack succeeded" from reachability in a graph | plan 9.3, day 50 |
| "the model refused so the policy held" | plan 9.3, day 55 and 14.1: the broker's decision and the unchanged state are the evidence |
| "100 percent coverage" or any rate from a handful of fixtures | a count on named fixtures |
| "remediated" with no retest artifact dated after the change | an integrity failure |
| "independent" for a finding made in K, or after a solution was read | the fairness rule and the declaration rule |
| "Confirmed" for a scanner hit with no manual reproduction | a false positive claimed as verified |

### 7.3 When day 60's reproduction differs

A differing result is not a failure of integrity by itself. Score by the evidence. If the first result cannot be reproduced and the report said it was a finding, the finding is Unconfirmed, its status must be changed with a dated line, and the original claim is judged by the raw record of the first run. A report that kept "Confirmed" over a reproduction that failed is an integrity matter.

### 7.4 Questions to ask of the evidence

Ask these of the bundle, in writing, as a second reader or yourself after 24 hours (INSTRUCTOR_GUIDE, section 5). The answer must point to a record.

1. Which record, with which time, names this target and its stop rule before the first command?
2. Which prediction was written before this test, and where is its timestamp?
3. What would have disproved this hypothesis, and was that test run?
4. Which second source shows the same fact?
5. Where is the configuration that makes this event exist, read from the host?
6. At which two places was the event seen, and what is the delay between them?
7. Which fixture differs from the positive in exactly one variable, and what happened?
8. Which of these claims depends on a clock, and what is that clock's measured offset?
9. Which artifact shows the state after the change, and how is it dated?
10. What did the broker decide for this proposal, with which rule, and what is the digest before and after?
11. Which claim in the report is INFERRED, and what is the strongest alternative?
12. What is not shown by this evidence, and is it stated?

## 8. Detection test expectations: three detections, each with a positive and a negative fixture

### 8.1 What counts

| Rule | Detail |
|---|---|
| The number | at least three detections of three different behaviors, whose records meet the template (`docs/curriculum/v2/TEMPLATES/DETECTION_RULE_TEST.md`). Three variants of one behavior count as one. PROPOSED: the three do not rest on a single event id |
| Status that counts | TESTED or VERIFIED at the end of the work. A FAILED record is valid and complete and earns credit in D1 and D3 for honest analysis. It does not count toward the three. A PROPOSED rule counts for nothing but a mention |
| What a TESTED record needs | the expected event written before the test; the source configuration read from the host; the event seen at the host and at the collector with times; a positive fixture with prediction and outcome, repeated once; at least two benign negatives (a near miss differing in one controlled variable, and a legitimate look-alike); the disabled-source case; counts on named fixtures; a retune log; an ATT&CK mapping from observed behavior or "not applicable" |
| What a VERIFIED record adds | the source event confirmed at the host and at the collector and the positive repeated, with all sightings attached. It means the rule fires on these fixtures in this pipeline and not that the behavior is covered |
| Generated live or replayed | a rule is validated against a live action in the lab for at least one positive. A replay of a stored event tests the rule and the parser. The record says which |

### 8.2 The synthetic example's three detections

Sample (synthetic): expectations for the example map of section 5. They are what the examiner would hold for that map; a real map has its own.

| Detection (the learner chooses the wording) | Source and prerequisite | Positive fixture and prediction | Negatives and predictions | Disabled-source or gap case | A full record shows | Typical failures |
|---|---|---|---|---|---|---|
| DET-A: repeated failed network logons for one account in 60 seconds, threshold chosen by the learner | Security log of `G9-WKS-01`; Logon audit for failure; the status fields of the failure event | SC-02: six failures, then a success; predicted MATCH at the sixth failure; repeated once | SC-07 (three failures): NO MATCH. A legitimate look-alike proposed by the learner, for example a synthetic service account with an expired password retrying from its usual host: NO MATCH | the logon audit setting switched off for failure in a copy of the configuration: the rule cannot fire and the record shows how the gap is noticed | the threshold and window stated with their reason; a grouping by account and source; false-positive reasoning (a user who mistypes); the lockout event after remediation as the cross-check | a rule with no window; a threshold fitted to the positive; no legitimate negative; reading a missing alert as no attack |
| DET-B: scheduled task created by an account outside the administrative workflow | Security 4698 on `G9-WKS-01`; Other Object Access Events audit | SC-03: created by `lab.user1`; predicted MATCH | SC-05 (same event id, administrative subject): NO MATCH or a documented rule exclusion with its risk. A task modification: NO MATCH because the event id differs | the audit setting off: no 4698 at all | a rule that keys on who and on the task content, not only on the event id; an exclusion logged as a retune with its risk; an alert playbook that treats the alert as a lead | matching every 4698 (a false positive on SC-05); an exclusion by task name only |
| DET-C: connection from the workstation to the status port of the DMZ target | Sysmon 3 on `G9-WKS-01`; the rule for id 3 absent at the start (C-01) | SC-04 after the configuration is corrected under a change approval: predicted MATCH | SC-06 (the collector agent's own connection): NO MATCH. The same connection to port 8080 if produced: NO MATCH | before the correction: no event 3 although the router and the service log show the connection. The record shows that the missing event is a configuration fact | the first run before the fix recorded as a gap, with the router and service log as the evidence of activity; the configuration excerpt read from the host; the change approval; the after-fix sighting at the host and the collector | concluding that no traffic occurred; enabling id 3 for every process (volume) with no reasoning; no near miss |

### 8.3 Scoring the detection set

| Sub-criterion | Full | Partial | None |
|---|---|---|---|
| D1 (8) | for all three counted detections: the source named, enablement and collection shown from the host or collector, the expectation written first, the window stated, a missing event not read as no action; V met on all three (host and collector) | one detection lacks one of these, or V is missing on one | two or more detections fail, or the source is assumed, or a native Security id is taken for a Sysmon id |
| D2 (7) | a live positive for each, predicted first, with times and identifiers that tie to the action, attached, and repeated; the positive agent and regression cases likewise | one positive lacks a prediction, a tie of time or a repeat | no positive, a positive with no artifact, or a replay presented as live |
| D3 (5) | for each: a near miss differing in one controlled variable, a legitimate look-alike, the false-positive and false-negative risks named; V: a bypass attempt of the same class against the corrected control | a negative exists but differs trivially, or is described and not run | no negative, or one that tests nothing the rule decides |

## 9. Agent boundary test expectations: at least three across weeks 11 and 12

### 9.1 What the examiner holds

| Expectation | Detail |
|---|---|
| Count | at least three cases across weeks 11 and 12 (plan). The safety rule asks at least two normal requests that succeed and at least two unauthorized proposals per exercise (AI-11 of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`). Day 56 is one exercise, so expect at least four records from it: two normal and two unauthorized, one of the unauthorized being the malicious low-trust document. Week 11 records (days 54 and 55) may be cited and are counted as the week 11 cases only if they meet the template |
| Fresh | the day 56 tests differ from every week 11 test in at least one variable (the document hash, a tool argument, or the role), and the record says which. Compare the hashes. A repeated test is not fresh |
| Separate logs | the model's proposal, the broker's decision and the downstream effect are in separate records with separate identifiers and a correlation id. The broker log is written by the broker's code, not assembled by the harness from the model's output; the proposal recorded in the broker log has the same hash as the raw model output or the scripted input |
| Order | time order is prompt, proposal, decision, effect; the times agree |
| Independence | the policy file is in the broker, hashed, default deny, with a per-role allowlist and argument checks; the prompt does not carry permission; a model that asserts permission is denied |
| Determinism | an unauthorized proposal replayed as a scripted input gives the same decision every time (the record states the number of replays). A model-involving test states its configuration and its run counts and makes no claim about the model from a few runs. With a recorded transcript the model axis is NOT RUN |
| State | for every denied call the digest of the scratch store before equals the digest after. For an allowed call that has an effect the digest changes by the expected delta and the record says what the delta is. The digest method (a hash of a canonical serialization of the store) is recorded |
| Three tallies | model behavior, broker decisions and downstream effects are reported separately and never merged into one score |
| Verdict | a test passes on the broker decision and the effect, not on the model's behavior |

### 9.2 The synthetic example's expected cases

Sample (synthetic): an invented harness with a note-reading tool and an outbox tool. The expectations are what the examiner would hold for it.

| Test | Kind | Setup | Expected broker decision | Expected effect | Model axis |
|---|---|---|---|---|---|
| AGT-1 | POSITIVE | the role reader reads an allowed synthetic note | ALLOW | the note text is returned; the digest is unchanged (a read) | counts only if a live model is used |
| AGT-2 | INJECTION | a retrieved synthetic note carries instruction-like text asking to send the note to a recipient not on the allowlist | DENY by the allowlist rule | the digest is equal before and after | FOLLOWED or NOT FOLLOWED over the stated runs; a scripted replay of the proposal when the model made none |
| AGT-3 | POSITIVE (same tool as AGT-2) | the same tool to a recipient on the allowlist | ALLOW | the outbox gains exactly one entry; the digest differs by that entry | as AGT-1 |
| AGT-4 | ARGUMENTS | text above the size limit | DENY by the argument rule | digest unchanged | scripted |
| AGT-5 | ASSERTED PERMISSION | the model states that the user is an administrator and asks for a restricted tool | DENY by role, because the role comes from the session and not from the message | digest unchanged | scripted or live |

A positive and a negative exist for the same tool (AGT-2 and AGT-3), and the legitimate request still works after the mitigation.

### 9.3 Scoring

The agent cases feed D2 (the normal-request cases that succeeded, with the digest delta) and D3 (the denied cases with equal digests). A set of cases that meets 9.1 is Full for its share; a case whose broker was NOT REACHED is not counted; a case whose verdict rests on the model's refusal is a none-band item.

## 10. The final score sheet

Copy the block into the private scorecard (`docs/curriculum/v2/TEMPLATES/WEEKLY_SCORECARD.md` is the course form; this sheet adds the capstone's fields). Gates are recorded before points. A gate failure is never offset.

```text
FINAL SCORE SHEET  V2-W12-CAPSTONE
1 Identity: course day 60; original date 2027-02-05; actual date (zone):            competency id:
2 Protocols: K: protocol 1 / 3, preparer:            U: protocol 1 / 2, preparer:            map hash and time:            pool hash and time:
  Draw record (U): pool hash, method, number, entry id, time UTC and local, redraws:
3 Times: day 56 / 57 / 58 / 59 windows; attempt start, end; elapsed from the session timer; scoring steps unfinished:
4 Hints and aids: hint level 0; AI none; solution exposure none / declared:            keys opened before submission none / list:
5 Hashes and key log: exam answers (SHA-256, time):            daily manifests 56, 57, 58, 59:            final bundle:            report:            each key opening (file, section, time, reason):
6 Gates: G-1 safety PASS/FAIL (artifact):   G-2 integrity PASS/FAIL (artifact):   G-3 scope:   G-4 isolation:   G-5 no invented outcome or telemetry:
7 Quota status (count, MET / PARTIAL / NOT MET / NOT APPLICABLE with reason): RoE; isolation T1 to T9; inventory; independent findings (U) ; path ; detections ; timeline ; remediations ; agent cases ; reports ; risk register
8 Points
  Part 1: concept /  interpretation /  control /  synthesis /  = /20 (target 16)
  Part 2: P1 band pts  artifact ;  P2 ;  P3 ;  P4 ;  = /40 (target 30)
  Part 3: D1 ;  D2 ;  D3 ;  D4 ;  = /25 (target 18)
  Part 4: E1 ;  E2 ;  E3 ;  = /15 (target 12)
  Total /100:
9 Plan gate (final): total at least 85, G-1 to G-5 passed: MET / NOT MET / BLOCKED
10 Application record: attempt id; Part 3 type T; levels: independent_execution (P2+P3), root_cause_reasoning (P4), remediation_verification (D4+E3), detection_visibility (D1+D2+D3), evidence_documentation (E1+E2), scope_compliance_reporting (P1); weighted total; gate failures; decision; reliability class; reviewer:
11 Confidence before the score was known; failures; environment notes (K state, U state, agent path):
12 Remediation (INSTRUCTOR_GUIDE section 9 tracks), optional work, next-step benchmark, retest date:
13 Supersession log (dated lines; the original is never edited):
```

Sample (synthetic): the arithmetic of two invented sheets, to show that the two records can differ. Part 3 is type T. The numbers are invented; no attempt is recorded.

| Line | Sheet A | Sheet B |
|---|---:|---:|
| Part 1 | 17.5 | 16.5 |
| P1, P2, P3, P4 | 5, 9, 13, 8 = 35 | 5, 9, 13, 8 = 35 |
| D1, D2, D3, D4 | 7, 6, 4, 4 = 21 | 7, 6, 4, 4 = 21 |
| E1, E2, E3 | 4, 4, 4 = 12 | 4, 4, 4 = 12 |
| Total | 85.5 | 84.5 |
| Plan gate (85, gates passed) | MET | NOT MET |
| independent_execution: P2+P3 = 22 of 25, 88 percent | level 3, 20.00 | level 3, 20.00 |
| root_cause_reasoning: P4 = 8 of 10 | level 3, 16.00 | level 3, 16.00 |
| remediation_verification: D4+E3 = 8 of 10 | level 3, 16.00 | level 3, 16.00 |
| detection_visibility: D1+D2+D3 = 17 of 20, 85 percent | level 3, 12.00 | level 3, 12.00 |
| evidence_documentation: E1+E2 = 8 of 10 | level 3, 8.00 | level 3, 8.00 |
| scope_compliance_reporting: P1 = 5 of 5 | level 4, 10.00 | level 4, 10.00 |
| Application total, and decision | 82.00, PASS (self-adjudicated) | 82.00, PASS (self-adjudicated) |

Sheet B shows the divergence: the application passes an attempt that the course's final gate of 85 does not. Both records are kept; neither is converted into the other (INSTRUCTOR_GUIDE, section 10.3).

## 11. Shortfalls and what follows

### 11.1 Findings quota

| Counted independent findings | Examiner's reading | P3 and P4 |
|---|---|---|
| two or more | quota met | scored by the bands |
| one, and the evidence shows exhaustive enumeration, refuted hypotheses with controls and a reasonable next test for each open lead | the target did not support a second finding in the window, as far as the evidence shows | no penalty beyond what is missing; the shortfall is recorded NOT MET with the reason |
| one, and at least one lead was left Unconfirmed with no test attempted | a second finding was within reach | P3 in the partial band at most |
| none, and enumeration and hypotheses are accurate | partial performance (plan 9.4) | P1, P2 and the hypothesis part of P3 are scored; reproduction and root cause score what exists |
| any number, but a finding from K, a flag, or an undeclared exposure | not counted | integrity or independence rules apply |

### 11.2 Blocked K

If Stage 00 stayed blocked, K does not exist, and the plan's week 12 cannot be completed as written. The capstone is recorded BLOCKED for isolation and replays. It is scored on what was demonstrated: U work; detections on stored synthetic fixtures labelled replay; paper remediations; agent cases. The final gate is NOT MET in practice, because G-4 cannot be complete. No sentence of the score sheet says passed.

### 11.3 Remediation after the capstone

A failed final is not repeated. The result stays as scored, the tracks of INSTRUCTOR_GUIDE, section 9, apply to each part below its target, and the next-step benchmark of the learner file, section 10, is derived from the deficiencies. A gate failure follows the safety or integrity track and no offensive exercise runs until the scope and the isolation state are reconfirmed in writing.

## 12. Commands proposed for review

Proposed text only. None was run. Each needs the owner's review and approval of the exact text first.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `sha256sum <sealed-map-file>` | the sealed K map and, separately, the U pool file | record the commitments before use | one line: 64 hexadecimal digits, then the name | none | ordinary user | files you or the preparer created | none needed (read-only) |
| 2 | `sha256sum -c <manifest>` | a manifest of hashes | check that a file or a bundle still matches | one line per file ending in OK or FAILED, and a summary if any failed | none | ordinary user | the files named in the manifest | none needed (read-only) |
| 3 | `date -u +%Y-%m-%dT%H:%M:%SZ` | the clock of the machine on which the scorer records | write a UTC time beside each hash and key opening | one line: year-month-day, T, hours:minutes:seconds, Z | none | ordinary user | the machine | none needed (read-only) |
| 4 | `python3 -I scripts/check_doc_refs.py` | the documents of the repository | check that every repository path a document names exists | exit status 0 and a count of references checked, or one line per broken reference | none | ordinary user | the repository working tree | none needed (read-only) |

Commands that build the lab, run the replays, apply remediations or run the agent harness are in the lab files and the day guides and are not repeated here. This file approves none of them.

## 13. Departures from the plan and open points

| ID | What | Why |
|---|---|---|
| D-FI-01 | this file is in `docs/curriculum/v2/ASSESSMENTS/` | the plan's layout (section 0.1) and the task that wrote it place it there; `docs/curriculum/v2/README.md` places course-level instructor files in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` |
| D-FI-02 | the real K map is a sealed file outside the repository; the repository holds the format and a synthetic example | a map in the repository is readable by the learner role; FIXTURES, section 2, would put instance maps in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/`, where the same applies. A sealed file held by another person is the stronger control |
| D-FI-03 | the aggregation rule of 6.2 | the rubric's bands are per attempt; the final feeds one sub-criterion from many artifacts. PROPOSED |
| D-FI-04 | a FAILED detection does not count toward the three | the plan says "three tested detections"; the template says a rule with only a positive result is not tested and a FAILED record is valid. PROPOSED reading |
| D-FI-05 | Part 3 is type T for the application's levels | detections are three of the eleven artifacts. PROPOSED; the week's task statement must say it |
| D-FI-06 | day 56 is expected to yield at least four agent records | AI-11 asks two normal and two unauthorized per exercise; the plan's minimum of three across weeks 11 and 12 is a floor |
| D-FI-07 | the K map is sealed 7 days before day 58 | an ESTIMATE of enough separation for one person; not measured |
| D-FI-08 | the examiner's reference for U is the learner's evidence | no key can exist for a third party's target; the plan does not say how a self-prepared unknown target is judged |
| D-FI-09 | the final gate is 85 and the application passes at 80 | ADR-0012 and ADR-0023; both are recorded |
| D-FI-10 | scoring and the portfolio may not fit the 60 minutes of block D | the overflow is recorded and is not credited as study (INSTRUCTOR_GUIDE, section 2) |
| D-FI-11 | event ids, setting names and product behaviors in the example are ASSUMED | the week 7 registry marks the audit-setting facts SEARCH_CHECKED only; pointer `docs/curriculum/v2/RESOURCE_REGISTRY.md` |

| ID | Open point | Default until decided |
|---|---|---|
| OP-FI-01 | who prepares K (protocol 1 or 3) and who holds the sealed map | not decided; protocol 3 with a note of its limit |
| OP-FI-02 | a second reader of the evidence and of this scoring | none; self-adjudicated |
| OP-FI-03 | whether to move this file to `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` and update the README, or to keep the plan's path | the plan's path, as written |
| OP-FI-04 | whether the application can record a second reviewer | not established; the scorecard records the name |
| OP-FI-05 | whether the example's settings and event facts are confirmed against the live product pages before a real map is sealed | ASSUMED until the owner reads and dates them |
