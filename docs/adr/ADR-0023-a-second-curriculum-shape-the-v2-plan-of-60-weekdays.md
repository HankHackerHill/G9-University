# ADR-0023: A second curriculum shape: the owner's v2.0 plan of 60 weekdays is applied next to the Appendix A shape

**Status:** Accepted (2026-11-18) for decisions 1 to 8 and 10 (applied in the backend, the schemas, the builder and the desktop mock: TESTED). Decision 9 and the generated files of decisions 4 to 7 are PROPOSED: they wait for the courseware (`docs/curriculum/v2/README.md`, "Status"). Owner request of 2026-11-18: "Apply curriculum and fill gaps only if needed; add quizzes, tests, questions, hands-on learning (Hack The Box, TryHackMe, etc.)", with the owner's document `docs/curriculum/v2/MASTER_PLAN_V2.0.md` ("G9 Cybersecurity Mastery, Final Master Plan v2.0", prepared 2026-11-17; a verbatim copy of the upload, SHA-256 beginning `ef12d2d1469f270b`).

## Context

The application enforced one curriculum shape, taken from Appendix A of the v6.0 handoff: 12 weeks of six days (five weekdays of 420 minutes with six fixed blocks, a Saturday of 300 minutes with four), three objectives a day, 72 days, 480 hours (`g9_domain::curriculum::CURRICULUM_INVARIANTS`, `g9-curriculum` validation, gate A01).

The owner's v2.0 plan keeps 12 weeks and 480 hours and changes the shape and the syllabus: 60 weekdays of 480 minutes in five blocks (R active recall 30, A core lesson and guided lab 120, B tool practice 120, C independent challenge 150, D defense and documentation 60), a scored Friday (20-question knowledge exam and an unseen practical), a midpoint on day 30 and a capstone on day 60, and a penetration-testing-first sequence (Linux, networking, Windows and AD, web I and II, infrastructure, SOC, DFIR, malware and reverse engineering, advanced AD, cloud and AI, capstone). The plan states that its schedule must not be changed (its sections 5 and 15), and asks for daily quizzes, Friday exams, answer keys kept apart from the learner's view, and verified hands-on exercises on named platforms.

A 60-day manifest cannot be imported under the Appendix A invariants. Folding the plan into 72 days (moving each Friday exam to a Saturday, renumbering days) would change the owner's calendar, which the plan forbids.

## Decision

1. **Two structure profiles.** `g9_domain::curriculum` defines the shapes the application accepts. A manifest declares its shape in `structure`; validation, planning and import use the profile that matches, and a `structure` that matches neither is refused (`structure.mismatch`), as before.

   | | `CT9_480` (Appendix A, unchanged) | `MASTERY_V2` (owner's v2.0 plan) |
   |---|---|---|
   | Weeks, days a week, days | 12, 6 (Monday to Saturday), 72 | 12, 5 (Monday to Friday), 60 |
   | Objectives a day, total | 3, 216 | 3, 180 |
   | Ordinary day | 420 min: recall 15, theory 45, guided_lab 120, independent_lab 120, defense_verification 60, evidence_reporting 60 | 480 min: recall 30, theory 120, guided_lab 120, independent_lab 150, defense_verification 60 |
   | Assessment day | Saturday, 300 min: blind_practical 120, remediation 60, report 60, performance_review 60 | Friday, 480 min: recall 30, theory 120, performance_review 120, blind_practical 150, report 60 |
   | Pauses in the day layout | none | 15 min after the second block, 60 min after the third (the plan's 08:00 to 17:15 day) |
   | Week, total | 2,400 min, 28,800 min | 2,400 min, 28,800 min |

2. **No migration.** The tables already admit the v2 shape (`day_number` 1 to 72, `day_of_week` 1 to 6, three objectives, `block_code` from ten names, minutes free). The v2 blocks use the existing block codes; what a code is called on screen and how long it lasts comes from the profile, and the lesson sheet names each block of each day. The codes of the v2 day:

   | Plan block | Block code, ordinary day | Block code, Friday | Label (profile default) |
   |---|---|---|---|
   | R, 30 | `recall` | `recall` | Active recall |
   | A, 120 | `theory` | `theory` | Core lesson (Friday: Knowledge exam) |
   | B, 120 | `guided_lab` | `performance_review` | Tool practice (Friday: Review and planning) |
   | C, 150 | `independent_lab` | `blind_practical` | Independent challenge (Friday: Scored practical) |
   | D, 60 | `defense_verification` | `report` | Defense and documentation (Friday: Scoring and report) |

   Modes stay those of the codes (`repos::curriculum::block_mode`): the scored practical is planned in EXAM; the knowledge exam's block is `theory` (STUDY). Exam integrity does not rest on the block's mode: the backend evaluates every command in EXAM while an attempt is open (ADR-0019), and the Friday assessment is one attempt begun before the knowledge exam and submitted with the report.

3. **Manifest format 1.1.0.** One optional field, `structure.assessment_day_of_week` (5 for `MASTERY_V2`; absent means 6). `saturday_minutes` and `saturday_blocks` keep their names and describe the assessment day; renaming them would change the checksum of every existing manifest. A 1.0.0 manifest is read as before and its checksum does not change.

4. **Seed and draft, as for Appendix A.** The seed `data/curriculum/g9-cyber-mastery-480.seed.v2.json` is generated from the tables of section 5 of the master plan: for each day the texts of columns A, B and C are the three objectives and column D is the evidence artifact (Markdown emphasis removed, nothing else changed). The draft `data/curriculum/g9-cyber-mastery-480.draft.v2.json` (`curriculum_id` `g9-cyber-mastery-480`, version `2.0.0-draft1`, `PROPOSED_SEED`) keeps each seed text as the start of its field and adds an observable criterion after ` — `. `g9 reconcile` and `curriculum.promote` work on the pair as they do for Appendix A; nothing is promoted by a script or an agent.

5. **One source tree, generated artifacts.** The courseware is written under `docs/curriculum/v2/` in the layout of section 0.1 of the plan (`docs/curriculum/v2/AUTHORING.md` is the contract). `scripts/build_curriculum_v2.py` checks it and generates the seed, the draft, the lesson sheets, the quiz file, the answer-key file, one `assessment.define` payload per Friday, and the indexes (`SCHEDULE_60_DAYS.md`, `CURRICULUM_MAP.md`, `RESOURCE_REGISTRY.md`, the question bank, the spaced-recall plan). Nothing generated is edited by hand.

6. **Questions and keys are separate files.** Each day has five recall and three applied questions in its guide and a key in `ANSWER_KEYS/`; each Friday has a 20-question exam and a practical in `ASSESSMENT.md` and a key with rationales. The builder fails when a key is missing, when key text appears in a learner file, or when an identifier is reused. The application shows the questions with the day; the keys are a separate lazy chunk that is read only when the operator asks to see a key, and an exam key only after that week's attempt is submitted. The operator is learner and instructor in one person: the separation prevents accidental exposure, it is not access control.

7. **Spaced recall is generated, the review engine is unchanged.** Each day tags its five recall questions with the offsets 1, 1, 3, 7 and 14 days; the builder places them in the recall block of the training day on or after that calendar offset. The application's review engine keeps its schedule (1, 3, 7, 21, 60 days; `g9-retention`).

8. **Friday assessments use the existing kinds and rubric.** One definition a week (`V2-W01-EXAM` to `V2-W11-EXAM`, kind `BLIND_PRACTICAL`; `V2-W12-CAPSTONE`, kind `CAPSTONE`), graded with the baseline rubric and the non-compensatory pass rule (ADR-0012). The plan's 100-point score (knowledge 20, practical 40, defensive validation 25, evidence and reporting 15; weekly gate 80, final 85) is recorded on the weekly scorecard; `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` maps its parts to the rubric criteria. The application's pass rule is the stricter of the two and is not changed.

9. **The window imports the v2 draft. PROPOSED, NOT APPLIED (2026-11-18).** The window keeps importing the Appendix A draft `g9-ct9-480` `1.0.0-draft1`, whose courseware and lesson sheets are complete. The switch is made when the v2 courseware is complete (`docs/curriculum/v2/README.md`, "Status"): then Program setup imports `g9-cyber-mastery-480` `2.0.0-draft1` and the lesson sheets are chosen by the curriculum of the active plan. Until then a v2 curriculum can be imported (the backend and the mock accept the shape) but the window has no sheets for it. The Appendix A seed and `g9-ct9-480` `1.0.0-draft1` stay in the repository with their gates and remain importable from the command line; the command-line test tooling (`simulate-week`, the smoke test, the performance fixture) keeps using the Appendix A seed.

10. **Resources are verified by search result, and say so.** Each week keeps a registry (`WEEKS/Wnn/RESOURCES.json`) of the labs and references it uses: address, provider, access tier, how it was verified and when. A day guide may cite only registry entries. Pages could not be opened from the authoring session (network policy), so the strongest label is `SEARCH_CHECKED`; nothing is labelled verified by reading.

## What this does not decide or do

| Item | State |
|---|---|
| Any lab on the G9 host (KVM, libvirt, Wazuh, AD, cloud sandbox) | NOT RUN. The plan's Stage 00 blocker (the libvirt socket) is unresolved; every command in the courseware is proposed text |
| Promotion of the v2 draft to `APPROVED_CANONICAL` | the operator's (`docs/curriculum/promotion.md`) |
| Competencies for reverse engineering, fuzzing, cloud IAM and containers | the eight packs have none; objectives of weeks 9 and 11 link the nearest existing competency or none. PROPOSED: a ninth pack |
| Loading quiz questions into the review queue (`retention.create_item`) | PROPOSED |
| The review engine's offsets (1, 3, 7, 21, 60) against the plan's (1, 3, 7, 14) | unchanged; the plan's offsets are carried by the generated recall queues |
| The plan's root directory `G9-CYBERSECURITY-MASTERY/` | the corpus lives in `docs/curriculum/v2/` of this repository, so that one source feeds the application; day files are numbered `DAY_01` to `DAY_60` |

## Alternatives considered

| Alternative | Why not |
|---|---|
| Fold the plan into the 72-day shape | Changes the owner's calendar and day numbers (day 30 midpoint, day 60 final), which the plan forbids |
| New block codes for v2 | `study_blocks.block_code` has a CHECK list; changing it means rebuilding a table that study sessions reference, in a product whose upgrade path was just verified. The codes are storage categories; the profile and the sheet give the names |
| Replace the Appendix A invariants | Every existing gate, fixture and test of the 72-day shape would be rewritten for no gain; the seed stays the baseline of the v1 draft |
| Keep the courseware outside the repository | Two sources; the application could show text that the checked files do not hold |
| Put the keys only in the repository | The learner could not check an answer in the window after an attempt |

## Consequences

- `curriculum.validate`, `curriculum.import`, `curriculum.daily_plan`, `curriculum.program` and `session.plan_day` accept both shapes. A Saturday of a v2 program is a rest day.
- The first-run flow imports 12 weeks, 60 days, 180 objectives. A data directory that already holds `g9-ct9-480` keeps it; importing v2 adds a version of another curriculum.
- `ARCHITECTURE.md` does not fix the day count; no deviation from it. The deviation is from Appendix A of the v6.0 handoff (source material), on the owner's instruction.
