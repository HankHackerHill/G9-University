# G9 Cybersecurity Mastery v2.0: course entry point

- Reader: the owner, who is both the learner and the instructor of this course.
- Source of requirements: `docs/curriculum/v2/MASTER_PLAN_V2.0.md` (the owner's plan, a verbatim copy that is never edited). How it is applied in this repository: `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md` (ADR-0023 below).
- Written: 2026-11-18. Labels used in every file of the course: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.

## Status

**Status line (2026-11-18):** `AUTHORING COMPLETE: NO (in progress) | LAB TESTED: NO | LEARNER PASSED: NO | CERTIFICATION READY: NO`

The four states are independent (plan section 14.3). Each is set only by its own evidence, and none implies the next. The owner changes the line, and only when the evidence in the third column exists; no agent and no script changes it.

| State | What it means | Evidence that sets it | Value on 2026-11-18 |
|---|---|---|---|
| AUTHORING COMPLETE | every file of the layout below exists, the courseware build check passes, and the three audits (technical, adversarial, acceptance) are recorded with their defects and resolutions. A finished repository, not a finished training experience | the completion manifest and the audit records in `docs/curriculum/v2/AUDIT_REPORT.md` | NO. The courseware is being written; the audit report does not exist yet |
| LAB TESTED | the lab was built on the G9 host with the owner's approval, the isolation acceptance tests of plan section 3.3 passed, and the lab steps were run and recorded | the owner's approved run records | NO. NOT RUN: nothing was deployed, installed, scanned or executed |
| LEARNER PASSED | the learner took the scored attempts and met the gates (weekly 80 of 100, final 85 of 100, safety gate, evidence-integrity gate) | weekly scorecards and the application's grade decisions | NO. No attempt is recorded |
| CERTIFICATION READY | the learner's artifacts were matched against every current official objective of one named certification, and that body's own prerequisites are met (plan section 13) | a per-objective comparison made by the owner; never produced by this course alone | NO. Not a state this course can establish |

> **BLOCKER, Stage 00 (host readiness): UNRESOLVED.**
>
> - Observation (owner-supplied, 2026-11-14, plan section 2.1): `virsh` could not connect because `/var/run/libvirt/libvirt-sock` does not exist. The CPU reports AMD-V; KVM acceleration is not verified. The route and IPv6 listings prove neither a VPN kill switch nor isolation.
> - Consequence: no local lab exists. No guest, virtual network, router, domain, SIEM or cloud sandbox described in this course has been built, and isolation is not proven.
> - The missing socket is a root-cause question (monolithic or modular daemon, socket activation, connection URI, permissions). It is not permission to install, enable or restart anything.
> - Non-execution gate: writing these files authorized nothing in them. Every command in the course is proposed text. A command is run only after the owner has reviewed that exact command with its target, purpose, expected output, side effects, privilege, safe scope and rollback, and has approved it.
> - Until the blocker is resolved: exercises that need the local lab are deferred or replaced by the hosted alternative the day guide names, the substitution is recorded, and no intrusive local test is made. A blocked lab is recorded as BLOCKED, never as passed.
> - Where it is handled: `docs/curriculum/v2/HOST_READINESS.md` (read-only diagnostics and how to read their results), `docs/curriculum/v2/LAB_ARCHITECTURE.md` (topology and isolation acceptance tests), `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (scope, rules of engagement, incident procedure).

## What exists and what is pending (2026-11-18)

Labels: TESTED (a test or gate in the repository passes), VERIFIED (checked by reading or running, evidence named), SEARCH_CHECKED (a web search returned the page; the page was not opened), NOT RUN, PROPOSED.

| Part | State | Evidence |
|---|---|---|
| Application support for the 60-day shape (backend, schemas, import, planning, block labels, desktop mock and screens) | Done, TESTED | ADR-0023; `crates/g9-curriculum/tests/mastery_v2.rs`, `crates/g9-storage/tests/mastery_v2.rs`, `crates/g9-app/tests/mastery_v2.rs`, `apps/cli/tests/mastery_v2.rs`, `apps/desktop/src/ipc/transports/mock.test.ts` |
| Builder and its checks | Done, TESTED | `scripts/build_curriculum_v2.py`, `scripts/tests/test_gates_build_curriculum_v2.py`; `python3 -I scripts/build_curriculum_v2.py --available` checks the weeks that have all five day guides (a week with fewer is reported as in progress, not judged) and is part of the preflight |
| Master plan, authoring contract, ADR | Done | `MASTER_PLAN_V2.0.md` (verbatim, never edited), `AUTHORING.md`, ADR-0023 |
| Resource registries of weeks 1 to 12 (460 entries: lab, platform and reference addresses with access tier) | Written, SEARCH_CHECKED or ASSUMED or UNVERIFIED per entry; no page was opened (network policy) | `WEEKS/Wnn/RESOURCES.json`, `docs/curriculum/v2/AUTHORING.md` |
| Course-level documents (student and instructor guides, safety and authorization, host readiness, lab architecture, templates, entry diagnostic, midpoint benchmark, capstone, grading rubrics) | Drafted; not yet through the technical, adversarial and acceptance reviews (plan section 14) | the files named in "Layout" |
| Week 1 | Design files and fixtures written (61 fixture files, SHA-256 of each in `WEEKS/W01/LABS/fixtures/MANIFEST.md`, all 61 match: VERIFIED); 2 of 5 day guides (days 1 and 4) written; the builder reports problems for the rest | `python3 -I scripts/build_curriculum_v2.py --week 1` |
| Weeks 2 to 12: day guides (58 of 60 missing), Friday exams and keys, labs, quizzes | NOT WRITTEN | |
| Generated files (`SCHEDULE_60_DAYS.md`, `CURRICULUM_MAP.md`, `RESOURCE_REGISTRY.md`, question bank, answer-key file, spaced-recall plan, `seed.v2.json`, `draft.v2.json`, lesson sheets v2, quiz file) | NOT GENERATED: the builder produces them only when all 12 weeks pass | `scripts/doc_refs_allow.json` lists the references to them as `planned` |
| The window on the v2 course (Program setup import, lesson sheets, quiz panel, hands-on list, key reveal) | NOT BUILT. The window imports the Appendix A draft, whose 12 weeks of courseware are complete | ADR-0023 decision 9 (PROPOSED) |
| Any lab on the G9 host | NOT RUN (Stage 00 blocker) | `HOST_READINESS.md` |

To resume: write week N following `AUTHORING.md` (day guides, Friday assessment, keys, labs), run `python3 -I scripts/build_curriculum_v2.py --week N` until it reports no problem, then `--write` when all 12 weeks pass; delete the `planned` entries of `scripts/doc_refs_allow.json` that the generated files make stale (the checker fails on a stale entry). Nothing in the v2 course is imported into the application until the owner promotes it (`docs/curriculum/promotion.md`).

## What this is

A 12-week apprenticeship of 60 weekdays at 8 accountable study hours a day: 480 hours. It is penetration-testing-first purple-team training: each offensive mechanism is followed by its defensive observation, a remediation and a retest. It is not equal-depth mastery of every security specialty (plan sections 1 and 12), and it makes no promise of professional status or of a certification result (plan section 0.4, last rule).

The working loop of every day: scope, hypothesis, reproduction on an authorized target, preserved evidence, root cause, telemetry, mitigation, retest, documented limits.

| Week | Original dates | Main focus (plan section 1.1) | Days | Scored Friday | Assessment code | Learner's weekly output (plan section 5) |
|---|---|---|---|---|---|---|
| 01 | 2026-11-16 to 2026-11-20 | Linux, KVM lab, shell and Python basics | 1 to 5 | day 5 | V2-W01-EXAM | `W01-Linux-Baseline-and-Hardening.md` |
| 02 | 2026-11-23 to 2026-11-27 | Networking, protocol analysis, scanning, code | 6 to 10 | day 10 | V2-W02-EXAM | `W02-Network-Service-and-Traffic-Assessment.md` |
| 03 | 2026-11-30 to 2026-12-04 | Windows and Active Directory foundations | 11 to 15 | day 15 | V2-W03-EXAM | `W03-AD-Identity-Security-Assessment.md` |
| 04 | 2026-12-07 to 2026-12-11 | Web security I: server side | 16 to 20 | day 20 | V2-W04-EXAM | `W04-Server-Side-Web-Findings.md` |
| 05 | 2026-12-14 to 2026-12-18 | Web security II: client side, identity, APIs | 21 to 25 | day 25 | V2-W05-EXAM | `W05-Web-and-API-Pentest-Report.md` |
| 06 | 2026-12-21 to 2026-12-25 | Infrastructure pentest, privilege escalation, segmentation | 26 to 30 | day 30 (midpoint) | V2-W06-EXAM | `W06-Midpoint-Penetration-Test.md` |
| 07 | 2026-12-28 to 2027-01-01 | SOC, SIEM and detections | 31 to 35 | day 35 | V2-W07-EXAM | `W07-Purple-Team-Detection-Engineering.md` |
| 08 | 2027-01-04 to 2027-01-08 | DFIR and incident response | 36 to 40 | day 40 | V2-W08-EXAM | `W08-Digital-Forensics-Incident-Report.md` |
| 09 | 2027-01-11 to 2027-01-15 | Malware, reverse engineering, vulnerability root cause | 41 to 45 | day 45 | V2-W09-EXAM | `W09-Reverse-Engineering-and-Vulnerability-Research.md` |
| 10 | 2027-01-18 to 2027-01-22 | Advanced Active Directory and enterprise defense | 46 to 50 | day 50 | V2-W10-EXAM | `W10-Enterprise-Active-Directory-Security-Assessment.md` |
| 11 | 2027-01-25 to 2027-01-29 | Cloud and DevSecOps (24 h), AI and agents (16 h) | 51 to 55 | day 55 | V2-W11-EXAM | `W11-Cloud-and-Agentic-AI-Security-Report.md` |
| 12 | 2027-02-01 to 2027-02-05 | AI (8 h), integrated final assessment (32 h) | 56 to 60 | day 60 (final) | V2-W12-CAPSTONE | `W12-Final-Executive-and-Technical-Capstone.md` |

The dates are the plan's original planning calendar, not a record of completion. On 2026-11-18 that calendar stands at day 3 (a Wednesday) and no day is recorded as done. 2026-12-25 and 2027-01-01 are planned training days. The owner may shift dates; the sequence, the 60 days and the 480 hours do not change.

### A day in one table

| Local time (America/Chicago) | Counted | Block | Ordinary day | Friday |
|---|---:|---|---|---|
| 08:00 to 08:30 | 0.5 h | R | active recall, prior mistakes | active recall (1-day and 3-day questions) |
| 08:30 to 10:30 | 2.0 h | A | core lesson and guided lab | closed-book knowledge exam (20 questions) |
| 10:30 to 10:45 | 0 | break | not study | not study |
| 10:45 to 12:45 | 2.0 h | B | tool practice, coding, lab preparation | the plan's review or planning task |
| 12:45 to 13:45 | 0 | lunch | not study | not study |
| 13:45 to 16:15 | 2.5 h | C | independent challenge | scored practical, no hints, no AI |
| 16:15 to 17:15 | 1.0 h | D | defense, evidence, remediation, retest | scoring and report |
| Day, week, course | 8.0 h | | 40 h a week, 480 h in 60 days | one assessment attempt, begun before the knowledge exam |

Day 55 is the one Friday where block A is a lesson and the exam sits in block D (plan section 5, week 11: the AI mini-exam). The method behind the blocks is in `docs/curriculum/v2/STUDENT_GUIDE.md`.

## Prerequisites

| Prerequisite | Needed for | State on 2026-11-18 | Where it is handled |
|---|---|---|---|
| Background of an advanced technical practitioner: a command line, one scripting language, TCP/IP basics | all weeks. The entry diagnostic of day 1 places the learner; it is not an entry gate | ASSUMED; not measured before day 1 | `docs/curriculum/v2/ASSESSMENTS/ENTRY_DIAGNOSTIC.md` |
| 8 accountable hours on each of 60 weekdays; the plan's day runs 08:00 to 17:15 America/Chicago and includes 75 minutes of breaks that are not counted | the 480 hours | the owner's calendar | `docs/curriculum/v2/STUDENT_GUIDE.md` |
| A written scope and rules of engagement for every target | every offensive exercise | PROPOSED | `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` |
| A host that runs the lab: KVM through libvirt, three isolated networks (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`), six guests configured at 16 vCPU, 31 GiB RAM and 390 GiB of thin disk in total (plan section 3.1) | every local-lab exercise | BLOCKED by Stage 00 | `docs/curriculum/v2/HOST_READINESS.md`, `docs/curriculum/v2/LAB_ARCHITECTURE.md` |
| The isolation acceptance tests of plan section 3.3, passed and evidenced | any intrusive local test | NOT RUN | `docs/curriculum/v2/LAB_ARCHITECTURE.md` |
| Accounts on the hosted platforms that the day guides name; the access tier differs by exercise | hosted exercises, and substitutions while the local lab is blocked | ASSUMED; each exercise carries its tier and how that tier was checked | `docs/curriculum/v2/RESOURCE_REGISTRY.md` |
| Evaluation licences for the Windows guests | the local directory lab from week 3 | ASSUMED; availability not checked | the week guide of week 3 |
| An approved cloud sandbox with a billing limit and a destroy-and-verify checklist | the cloud lab of week 11; a hosted or no-cloud substitute is named for the case that none is approved | not approved | the week guide of week 11, `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` |
| The G9 MENTOR desktop application, built, with its first run done | using the course in the window | `docs/runbooks/first-run.md` records what was tested and what is NOT RUN (building and starting the window on the host) | `docs/runbooks/first-run.md` |

Nothing in this table is satisfied by reading it. No software is to be installed, no account created and no cost incurred on the strength of this file; each is the owner's decision.

## Layout

The corpus lives in `docs/curriculum/v2/` (the plan's root name `G9-CYBERSECURITY-MASTERY/` is not used; see the departures table). The tree shows week 1 in full; weeks 2 to 12 have the same shape. The day files are numbered `DAY_01` to `DAY_60` across the course: week w holds days 5w-4 to 5w, so `WEEKS/W01/` holds `DAY_01` to `DAY_05` and `WEEKS/W12/` holds `DAY_56` to `DAY_60`.

```
docs/curriculum/v2/
  README.md                             # this file
  MASTER_PLAN_V2.0.md                   # the owner's plan; never edited
  AUTHORING.md                          # file formats the build check enforces
  STUDENT_GUIDE.md                      # learning method, daily blocks, hints, recall, gates
  INSTRUCTOR_GUIDE.md                   # scoring, gates, key release, unseen practicals, remediation
  SAFETY_AUTHORIZATION.md               # scope, rules of engagement, cloud guardrails, incident procedure
  HOST_READINESS.md                     # Stage 00 read-only diagnostics and their interpretation
  LAB_ARCHITECTURE.md                   # guests, networks, isolation acceptance tests, recovery
  SCHEDULE_60_DAYS.md                   # GENERATED: per-day index and dependency gates
  CURRICULUM_MAP.md                     # GENERATED: competency, day, resources, evidence, rubric
  RESOURCE_REGISTRY.md                  # GENERATED from the per-week registries
  REFERENCES.md                         # curated primary sources and their verification state
  AUDIT_REPORT.md                       # technical, adversarial and acceptance audits; completion manifest
  WEEKS/
    W01/
      RESOURCES.json                    # the week's registry of labs and references
      WEEK_GUIDE.md                     # goals, prerequisites, tools, risks, pass conditions
      DAY_01.md                         # day guides DAY_01 to DAY_05
      LABS/
        GUIDED_LAB.md
        INDEPENDENT_LAB.md
        FRIDAY_PRACTICAL.md
        fixtures/
          MANIFEST.md                   # synthetic fixtures with hash or generation recipe
      QUIZZES/
        DAY_01_QUIZ.md                  # GENERATED: the day's eight questions without keys
      ASSESSMENT.md                     # the Friday exam: 20 questions and the practical
      EVIDENCE_CHECKLIST.md             # expected artifacts, allowed redactions, pass or fail
      ANSWER_KEYS/                      # INSTRUCTOR ONLY
        DAY_01_KEY.md
        ASSESSMENT_KEY.md
        LABS_KEY.md
    W02/                                # W02 to W12: same shape
  ASSESSMENTS/
    ENTRY_DIAGNOSTIC.md
    MIDPOINT_OFFENSIVE_BENCHMARK.md
    FINAL_CAPSTONE_STUDENT.md
    GRADING_RUBRICS.md                  # marking rubrics of the four score parts
    QUESTION_BANK.md                    # GENERATED: every learner question, without keys
    SPACED_RECALL.md                    # GENERATED: the recall queue of every day
    ANSWER_KEY.md                       # GENERATED, INSTRUCTOR ONLY: every quiz and exam key collected
    INSTRUCTOR/                         # INSTRUCTOR ONLY: capstone fixture map and course-level keys
  TEMPLATES/
    LAB_RECORD.md
    PENTEST_FINDING.md
    EXECUTIVE_REPORT.md
    TECHNICAL_REPORT.md
    DETECTION_RULE_TEST.md
    DFIR_TIMELINE.md
    ATTACK_PATH_GRAPH.md
    AI_AGENT_EVALUATION.md
    CHANGE_APPROVAL.md
    CLOUD_TEARDOWN.md
    WEEKLY_SCORECARD.md
  FIXTURES/                             # course-level synthetic fixtures; week fixtures stay in the week
  PORTFOLIO/                            # sanitized final work; no spoilers of active third-party content
```

| Kind of file | Rule |
|---|---|
| Hand-written source | edited by an author under `docs/curriculum/v2/AUTHORING.md`: the day guides, week guides, labs, assessments, keys, guides and templates |
| GENERATED | produced by the courseware builder `scripts/build_curriculum_v2.py` from the hand-written source; never edited by hand. The check and the regeneration command are in the section Build of `docs/curriculum/v2/AUTHORING.md`. Generated files named above: `SCHEDULE_60_DAYS.md`, `CURRICULUM_MAP.md`, `RESOURCE_REGISTRY.md`, every `QUIZZES/DAY_NN_QUIZ.md`, `ASSESSMENTS/QUESTION_BANK.md`, `ASSESSMENTS/SPACED_RECALL.md`, `ASSESSMENTS/ANSWER_KEY.md`. The builder also writes the importable manifests, lesson sheets, answer keys and one Friday payload per week into data/curriculum/ (see the window section below) |
| INSTRUCTOR ONLY | every file below a folder named `ANSWER_KEYS`, everything in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/`, and the generated `docs/curriculum/v2/ASSESSMENTS/ANSWER_KEY.md`: answers, rationales, fixture maps, reference observations, scoring keys. No learner file holds an answer, a flag, a solution path or a fixture map of an unsolved item |
| Readable by both | `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` and `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` describe how work is marked and how the course is run. They hold no answer and no fixture map of any item; a learner who reads them learns the scoring, not a solution |
| Learner's own work | lab records, evidence, reports and scorecards are the learner's files. They are kept in the application (Learning Journal, attachments) or in a private folder, not written into the course source. A day guide's line `Status: PROPOSED` is part of the source and is not edited to record progress |

The separation of instructor files prevents accidental exposure. It is not access control: learner and instructor are one person and the files are in one repository. The release rules are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`.

## Where to start

| Order | Open | To |
|---|---|---|
| 1 | this file | know the state of the course and the blocker |
| 2 | `docs/curriculum/v2/STUDENT_GUIDE.md` | learn the method before day 1 |
| 3 | `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` | write the scope and rules of engagement that every exercise refers to |
| 4 | `docs/curriculum/v2/HOST_READINESS.md`, then `docs/curriculum/v2/LAB_ARCHITECTURE.md` | review the Stage 00 diagnostics as proposed text and decide which to approve |
| 5 | `docs/curriculum/v2/SCHEDULE_60_DAYS.md` | see every day, its dependencies and its gate |
| 6 | the week guide and the first day guide of week 1 | start |
| When grading | `docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` | score a Friday and decide the gates |
| When a resource fails | `docs/curriculum/v2/RESOURCE_REGISTRY.md` | find the fallback and the verification state of a lab or reference |

## Using the course in the G9 MENTOR window

State of this section: PROPOSED. It describes the application behaviour that ADR-0023 (Accepted, 2026-11-18) decides. This file does not verify that the build of the application in use contains it; a tested state is recorded in `CHANGELOG.md` and `docs/runbooks/first-run.md`, not here.

| Step | Where in the window | What the owner does | What the window then holds or shows |
|---|---|---|---|
| 1 | Program setup (the panel "Set up your program" on Mission Control; the same panel under Settings, headed Program) | imports the curriculum `g9-cyber-mastery-480`, version `2.0.0-draft1`, and confirms the approval in the application's own confirmation window | 12 weeks, 60 days, 180 objectives, 28,800 planned minutes, provenance PROPOSED_SEED. Plans stay labelled Proposed until the owner promotes a version (`docs/curriculum/promotion.md`); nothing is promoted by a script or an agent |
| 2 | Program setup | chooses the Monday on which week 1 starts and the time a day starts. 08:00 reproduces the plan's timetable; another time shifts every block by the same amount | one stored program start. The day layout has a pause of 15 minutes after the second block and of 60 minutes after the third. Saturday and Sunday are rest days |
| 3 | Mission Control or Daily Classroom | plans today's blocks | five planned sessions: active recall 30 min, core lesson 120, tool practice 120, independent challenge 150, defense and documentation 60. On a Friday: active recall 30, knowledge exam 120, review and planning 120, scored practical 150, scoring and report 60 |
| 4 | Daily Classroom | reads the day | each block's lesson sheet (generated from the day guide), the day's quiz (eight questions without keys) and the day's resources. An address is text to copy: the window opens no link and performs no network action |
| 5 | Session Tracker | starts, pauses and completes each block | time credited by the backend's monotonic clock |
| 6 | Learning Journal | writes the lab record and attaches the day's evidence | notes kept as versions under a content hash |
| 7 | Daily Classroom | asks for a key | a day key is read only when the operator asks for it; a Friday exam key only after that week's attempt is submitted |
| 8 | Assessments, "Define an assessment" | defines the week's Friday assessment from its generated payload in the folder data/curriculum/assessments-v2/ (one `assessment.define` payload per Friday: V2-W01-EXAM to V2-W11-EXAM of kind BLIND_PRACTICAL, V2-W12-CAPSTONE of kind CAPSTONE) | one assessment definition per week, defined as the week comes |
| 9 | Assessments | begins the attempt before the knowledge exam, submits it with the report, reviews the evidence, chooses a level 0 to 4 for each of six criteria, approves the final grade | one attempt per Friday. The application passes it only under its non-compensatory rule (ADR-0012), which differs from the plan's 80 of 100; the plan's 100-point score is recorded on the weekly scorecard. A grade decided by the learner is recorded as self-adjudicated and never validates a competency |
| 10 | Retention Center | optional: reviews items created in the application | the application's own ladder (1, 3, 7, 21, 60 days) is unchanged. The course's 1, 3, 7 and 14-day recall is carried by the generated queue in each day's recall block |

What the window does not do: it runs no lab command, scan or installation, reaches no network, and opens no hosted lab. Hosted exercises are done in a browser outside the application, under the scope written for that exercise. The folder named in step 8 and the curriculum manifest of step 1 are generated outputs of the courseware builder (ADR-0023, decisions 4 and 5); until the builder has written them, steps 1 and 8 cannot be done.

## Rules that hold in every file

| Rule | Source |
|---|---|
| Offensive technique is used only on owned, intentionally vulnerable, platform-designated or explicitly authorized targets, and each exercise names its scope and target id | plan sections 0.4 and 9.4 |
| Every command is proposed text for review, with target, purpose, expected output, side effects, privilege, safe scope and rollback. No file states that running it is authorized | plan sections 0.2 and 0.4 |
| Credentials and data are synthetic. No real credential, host, organization or personal data appears | plan section 0.4 |
| No output is presented as observed. Example output is marked `Sample (synthetic):` | plan sections 0 and 16 |
| No walkthrough or solution of a third-party lab, room, machine or challenge, and no vendor course text | plan section 0.3 |
| A version-sensitive fact (product version, default, price, access tier, exam rule) carries its registry entry or the label ASSUMED | plan sections 4 and 10.2 |
| VERIFIED is written only next to evidence that is in the repository | plan section 0.4 |

## Departures from the plan

| Plan | Here | Why |
|---|---|---|
| Root directory `G9-CYBERSECURITY-MASTERY/` | `docs/curriculum/v2/` of this repository | one source feeds the application and the check scripts (ADR-0023) |
| `DAY_01.md` to `DAY_05.md` in every week folder | `DAY_01.md` to `DAY_60.md`; week w holds days 5w-4 to 5w | a day id is unique in the course; day 30 and day 60 keep the plan's numbers |
| `ASSESSMENTS/FINAL_CAPSTONE_INSTRUCTOR.md` beside the learner files | instructor material of the course level is in `docs/curriculum/v2/ASSESSMENTS/INSTRUCTOR/` | instructor files are separable by path, so a leakage check can be made by folder |
| `ASSESSMENTS/ANSWER_KEY.md` as an authored file | generated from the week key files; it stays in `ASSESSMENTS/` and is marked instructor only | the collected key cannot drift from the week keys |
| `QUIZZES/` as authored student files | generated from the day guides | each question has one source; the build check fails on a missing key or a reused identifier |
| `CURRICULUM_MAP.md`, `RESOURCE_REGISTRY.md`, `SCHEDULE_60_DAYS.md`, the question bank and the spaced-recall plan as authored files | generated | the indexes cannot drift from the day guides |
| No per-week registry and no format contract | `RESOURCES.json` in each week and `AUTHORING.md` added | a day guide may cite only registry entries; the build check needs fixed formats |
| `MASTER_PLAN_V2.0.md` "faithfully maintained" | kept verbatim; a correction is recorded as a departure in the file that applies it | the owner's document stays the source of requirements |
| Weekly gate of 80 of 100 with category minima as diagnostic targets | unchanged on the weekly scorecard; the application additionally applies its own non-compensatory rule to the same attempt | ADR-0012 and ADR-0023, decision 8; the mapping is in the instructor guide |
| Review at 1, 3, 7 and 14 days | carried by the generated recall queue; the application's review engine keeps 1, 3, 7, 21, 60 | ADR-0023, decision 7 |
