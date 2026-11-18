# Authoring contract of the v2.0 courseware

Controlling documents: `docs/curriculum/v2/MASTER_PLAN_V2.0.md` (the owner's plan; never edited) and ADR-0023. This file fixes the file formats that `scripts/build_curriculum_v2.py` parses. A file that does not follow it fails the build.

## Files of one week (one folder per week under `docs/curriculum/v2/WEEKS/`, named W01 to W12)

| File | Reader | Written by | Content |
|---|---|---|---|
| `RESOURCES.json` | builder | research, extended by the week's author | the labs and references the week may cite (format below) |
| `WEEK_GUIDE.md` | learner | author | goals, prerequisites, lab stack, competencies, accelerator, tools, risks, pass conditions |
| `DAY_NN.md` (NN = 01 to 60; week w holds 5w-4 to 5w) | learner | author | the day guide (format below) |
| `LABS/GUIDED_LAB.md`, `LABS/INDEPENDENT_LAB.md`, `LABS/FRIDAY_PRACTICAL.md` | learner | author | the week's three lab specifications (plan section 6) |
| `LABS/fixtures/` | learner | author | synthetic fixtures only (logs, toy source, sample output), each listed in `LABS/fixtures/MANIFEST.md` with its SHA-256 or its generation recipe |
| `ASSESSMENT.md` | learner | author | the Friday exam: 20 questions and the practical |
| `EVIDENCE_CHECKLIST.md` | learner | author | expected artifacts, allowed redactions, pass or fail |
| `ANSWER_KEYS/DAY_NN_KEY.md` | instructor | author | key of the day's eight questions, reference observations of the independent challenge, the fourth hint |
| `ANSWER_KEYS/ASSESSMENT_KEY.md` | instructor | author | key of the 20 questions with rationale, practical fixture map and scoring |
| `ANSWER_KEYS/LABS_KEY.md` | instructor | author | reference observations and scoring of the three labs |
| `QUIZZES/DAY_NN_QUIZ.md` | learner | **generated** | the day's eight questions without keys |

Instructor files live only under `ANSWER_KEYS/`. No learner file may hold an answer, a flag, a solution path or a fixture map of an unsolved item.

## Rules for every file

- Plain Markdown, UTF-8, no HTML, no images, no links in `[text](url)` form inside day guides and assessments (the window renders no links); write an address as inline code and list it in `## Resources`.
- No hidden or control characters. No real credentials, hosts, organizations or personal data. Lab addresses come from the plan's topology (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`); credentials are synthetic placeholders such as `<synthetic-password>`.
- Every command is **proposed text for the owner's review**, never an instruction that was run. No output is presented as observed unless it is marked `Sample (synthetic):`.
- Offensive technique is taught for owned, intentionally vulnerable, platform-designated or explicitly authorized targets only, and each such exercise names its scope.
- No walkthrough of a third-party lab, room, machine or challenge: name the exercise, say what to practise and what evidence to keep, never the solution. Never quote vendor course text.
- A claim about a product version, a default, a price or an access tier carries its registry entry or the label ASSUMED.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.

## `RESOURCES.json`

```
{ "week": 1, "checked_on": "YYYY-MM-DD", "method": "...",
  "entries": [ { "id": "w01-...", "url": "https://...", "provider": "...", "title": "...",
      "kind": "MODULE|ROOM|PATH|LAB|WARGAME|MACHINE|CHALLENGE|DOCS|TOOL|STANDARD",
      "access": "FREE|FREE_ACCOUNT|PAID|MIXED|UNKNOWN", "access_note": "...",
      "verification": "SEARCH_CHECKED|ASSUMED|UNVERIFIED", "evidence": "...", "version": "...",
      "topics": ["..."], "suggested_days": ["D01"], "block": "A|B|C|D|any",
      "estimate_minutes": 120, "difficulty": null, "fallback": null, "notes": "..." } ],
  "plan_fact_checks": [ { "claim": "...", "result": "CONFIRMED|NOT_CONFIRMED|CONTRADICTED", "evidence": "..." } ],
  "gaps": ["..."] }
```

An author who needs a resource that is not in the file adds an entry under the same rules (searched on the provider's own domain, or `UNVERIFIED` with the reason). A day guide cites a resource by its `id`.

## `DAY_NN.md`

The headings, their order and their spelling are fixed. `<...>` is replaced; everything else is literal.

```
# G9 Wnn / DAY_NN — <topic>

- Status: PROPOSED
- Day id: DNN
- Original calendar date: YYYY-MM-DD (<weekday>)
- Duration: 8 instructional hours (R 0.5 h; A 2 h; B 2 h; C 2.5 h; D 1 h)
- Lab authorization: <the exact synthetic range, platform and rules this day works under>
- Dependencies: <earlier days, tools, access tier>
- Primary provider: <one provider> | Secondary: <one provider or none>

## Objectives
1. <seed text of column A> — <observable success criterion> [<competency id>, ...]
2. <seed text of column B> — <observable success criterion> [...]
3. <seed text of column C> — <observable success criterion> [...]

## Hand-in
<seed text of column D> — <acceptance criteria a stranger could check>

## Resources
| Id | Use in this day | Block |
|---|---|---|
| <registry id> | <what to do with it and what evidence to keep> | <R, A, B, C or D> |

## R — Active recall (30 min)
## A — <title> (120 min)
## B — <title> (120 min)
## C — <title> (150 min)
## D — <title> (60 min)

## Commands proposed for review
| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|

## Offense, detection, remediation, retest
| Offensive mechanism | Defensive observation | Remediation | Retest |
|---|---|---|---|

## Quiz
### Recall (closed book)
1. [WnnDNN-R1] <question>
2. [WnnDNN-R2] <question>
3. [WnnDNN-R3] <question>
4. [WnnDNN-R4] <question>
5. [WnnDNN-R5] <question>
### Applied
1. [WnnDNN-A1] <scenario and question>
2. [WnnDNN-A2] <scenario and question>
3. [WnnDNN-A3] <scenario and question>

## Hint ladder
1. No hint: <what to try alone for 45 minutes>
2. Conceptual hint: <a small conceptual hint>
3. Scoped technical hint: <a narrow technical hint>
4. Instructor explanation: in the answer key, after the attempt is submitted.

## Troubleshooting and rollback
- Expected: <...>
- Missing dependency: <...>
- No connectivity: <...>
- Unexpected result: <...>
- Safety breach: <...>
- Restoration: <...>
- Time exceeded: <...>

## Evidence and acceptance
- Evidence: <artifact>
- Grading: <criterion>
- Negative test: <the test that separates apparent success from verified success>

## Expert extension (optional)
<work that can be dropped without breaking the 480 hours>

## Spaced recall
- 1 day: WnnDNN-R1, WnnDNN-R2
- 3 days: WnnDNN-R3
- 7 days: WnnDNN-R4
- 14 days: WnnDNN-R5

## Reflection
- What was observed and what was inferred?
- What was not proven?
- Which test would falsify the leading hypothesis?
```

| Rule | Check |
|---|---|
| Objectives | exactly three; each starts with the seed text of its column (printed by `python3 -I scripts/build_curriculum_v2.py --seed-text DNN`), then ` — `, then the criterion; at most 600 characters; the bracket lists zero to three competency ids of the eight packs (`[]` when none fits) |
| Hand-in | starts with the seed text of column D, then ` — ` |
| Resources | one to eight rows; each id is in the week's `RESOURCES.json` (or another week's, written `wNN-...`); at least one row for block C or B names a hands-on exercise (kind MODULE, ROOM, LAB, WARGAME, MACHINE or CHALLENGE) unless the day's line `Lab authorization` says the day is local-lab only |
| Blocks | the five headings with exactly these letters and minutes; each block has at least 120 words on an ordinary day (R: 60); block R gives the warm-up tasks used when the generated queue is short and asks for one failed experiment explained in the learner's words; A gives the canonical reading, the worked explanation, the guided lab and the common wrong models with a discriminating test each; B the prerequisites, host or guest, and validation against independent ground truth; C an unseen or modified task with scope, success criteria and negative controls and no answer; D the telemetry, the control, the positive and negative retest and the record to submit |
| Commands | at least three rows on a technical day (a Friday may have none); no empty cell; `Rollback` says `none needed (read-only)` when that is true |
| Mapping | at least one row; a day without an offensive mechanism writes `not applicable: <reason>` in the first cell |
| Quiz | exactly five recall and three applied questions with these identifiers; a question is one paragraph; no answer, no "hint:" |
| Hint ladder | four numbered lines with these labels; line 4 is literal |
| Troubleshooting | the seven labelled lines, in this order |
| Evidence | at least one line of each of the three labels |
| Spaced recall | the four lines; the five recall identifiers of the day, each exactly once, two of them on the first line |
| Words | 1,400 to 3,200 in the whole file |

A Friday guide uses the same headings: block A is the knowledge exam (it refers to `ASSESSMENT.md`, Part 1; day 55 is the exception the plan makes: A is a lesson and the exam sits in block D), B is the plan's review or planning task, C the scored practical (`ASSESSMENT.md`, Parts 2 and 3), D scoring and the report (Part 4). Its quiz still has eight questions of its own.

## `ANSWER_KEYS/DAY_NN_KEY.md`

```
# Instructor key — G9 Wnn / DAY_NN

Instructor only. Not part of the learner's view.

## Quiz key
### [WnnDNN-R1]
- Answer: <the expected answer>
- Marking: <points, what is accepted, what is rejected as an overclaim>
- Source: <registry id or document>
(the same three lines for each of the eight identifiers, in order)

## Independent challenge: reference observations and scoring
## Hint 4: instructor explanation
## Overclaims to reject
```

## `ASSESSMENT.md` (learner) and `ANSWER_KEYS/ASSESSMENT_KEY.md`

```
# G9 Wnn — Friday assessment (day NN)

- Code: V2-Wnn-EXAM            (week 12: V2-W12-CAPSTONE)
- Kind: BLIND_PRACTICAL        (week 12: CAPSTONE)
- Competency: <one competency id of the eight packs>
- Difficulty: <1 to 5, inside that competency's band>
- Title: <at most 200 characters>

## Rules
## Part 1 — Knowledge exam (20 questions, 20 points)
1. [WnnX-Q01] (CONCEPT|INTERPRET|CONTROL|SYNTHESIS) <question; a multiple-choice question lists options "A." to "D." on following lines>
... to [WnnX-Q20]
## Part 2 — Practical (40 points)
## Part 3 — Defensive validation (25 points)
## Part 4 — Evidence and reporting (15 points)
## Remediation track
## Submission
```

| Rule | Check |
|---|---|
| Question mix | at least 6 CONCEPT, 6 INTERPRET (evidence, log, packet, code or output to interpret, given in the question as `Sample (synthetic):`), 4 CONTROL, 4 SYNTHESIS; at least 8 multiple-choice |
| Practical | an unseen or modified scenario (plan section 9.3 names what cannot be assumed); scope, time, deliverables, what is scored; no fixture map, no fault list beyond a catalogue of classes |
| Length | Rules to Part 4 together at most 19,000 characters (they become the task statement of `assessment.define`) |
| Key | `### [WnnX-Qnn]` for each question with `- Answer:`, `- Why:` and, for multiple choice, `- Distractors:` (why each other option is wrong); `## Practical: fixture map and scoring` with the four score parts and their bands; `## Safety and integrity gate` |

## `WEEK_GUIDE.md`

Starts with `# G9 Wnn — <week title of the plan>`, then the paragraphs `**Purpose.**`, `**Lab stack.**`, `**Competencies.**`, `**Accelerator.**` (the four the window shows), then `## Goals`, `## Prerequisites`, `## Days`, `## Tools`, `## Risks and safety`, `## Pass conditions` (the plan's gate, verbatim in meaning), `## Remediation`.

## `LABS/*.md`

Each lab file has: `## Why it matters`, `## Scope and authorization`, `## Environment` (ids, scoped addresses, resources, realistic setup time labelled estimate), `## Preconditions`, `## Steps` (guided lab only; the independent lab and the Friday practical give the task, not the steps), `## Positive validation`, `## Negative and safety validation`, `## Evidence`, `## Rollback and cleanup`. Reference observations and scoring are in `ANSWER_KEYS/LABS_KEY.md` under the same lab names.

## Build

```
python3 -I scripts/build_curriculum_v2.py                 # check; fails on a broken rule or a stale generated file
python3 -I scripts/build_curriculum_v2.py --week 3        # check one week (other weeks may be absent)
python3 -I scripts/build_curriculum_v2.py --seed-text D11 # print the seed texts of a day
python3 -I scripts/build_curriculum_v2.py --write         # regenerate every generated file
```
