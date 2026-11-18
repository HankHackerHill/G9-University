# Lab record template

- Source: plan section 11.1 (standard lab record). The plan's block D asks for "one concise lab record" per substantial lab (sections 10.1 and 10.4).
- Status: PROPOSED. This is a blank template. It holds no result, and no record made with it exists in the repository.
- Reader: the learner, who fills it, and the examiner (the owner, as instructor), who ticks the quality checks.
- Related: what each field must hold is also summarised in `docs/curriculum/v2/STUDENT_GUIDE.md` section 10; the scope line is defined in `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.4; the marking of the evidence is in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`.
- Labels in a record made with this template: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN. A claim without one of them is not a claim.

## How to use

When. Write one record for each substantial lab: a block C independent challenge, a guided lab, or any exercise whose result you intend to claim (a reproduced issue, a fix, a detection, an analysis). One command of warm-up work belongs in your command vault (`docs/curriculum/v2/STUDENT_GUIDE.md` section 7), not in a record. Writing more is not a substitute for reproducing the result independently (plan section 10.4).

Order of work. The header lines and the hypothesis are written before the first command, with a time: a scope block or a hypothesis written after the action earns nothing in the marking. The commands section is filled as you go. Every other section is written during or after the lab. The status is set last.

Done when all five hold:

1. Every field is filled, or says NOT RUN with the reason. An empty field is not accepted.
2. The status follows the rule below, and no evidence in the record is stronger than the status claims.
3. The evidence files are listed with a SHA-256 each, and the hash of the bundle is written in the record with its time. Computing a hash is a command: it needs your approval like any other (proposed text: `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 11, row 3).
4. Every temporary change is shown reversed, or the record says it was not.
5. Every tick in "Quality checks before you submit" is honest.

Where it is kept. The record is your own file: in the Learning Journal or a private folder, never in the course source. While an assessment attempt is open, keep your notes in a private file and attach them when you submit (`docs/curriculum/v2/STUDENT_GUIDE.md` section 10).

What never goes in. Real credentials, real hosts, real people or organisations; flags, step orders, payload sequences or screenshots of an active third-party machine, challenge or room; text copied from a vendor course. Use synthetic credentials such as `<synthetic-password>` and the lab addresses of the plan only (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`).

## Status vocabulary

| Status | Meaning | Needs |
|---|---|---|
| PROPOSED | planned; nothing was run | the scope block, the hypothesis, the commands written as text |
| TESTED | run, and the result was observed once | saved output and the time of each step |
| FAILED | run, and the expected result was not obtained | the observed result, your revised hypothesis, what you will test next. A FAILED record is complete and valid |
| VERIFIED | observed, and confirmed by a second independent observation (another tool, another source or another person) | both observations attached, the bundle hash, the limits of the claim stated |
| BLOCKED | could not run: a missing dependency, a path that must not exist, an unavailable account, time. Added by the course (departures) | the diagnostic that shows the blocker and the substitution used. A blocked lab is never recorded TESTED or VERIFIED |

Rules: choose the lowest status that all the evidence supports. VERIFIED means this observation was confirmed twice in this environment. It does not mean secure, exploitable elsewhere, or generally true.

Each command inside the record carries its own word: PROPOSED (written), REVIEWED (checked against the review table of `docs/curriculum/v2/STUDENT_GUIDE.md` section 4), APPROVED (you approved the exact text), EXECUTED (run, with the time and what happened), NOT RUN. A result is OBSERVED only after EXECUTED.

## Fields

The field names below are the plan's, in the plan's order. Keep them unchanged.

| Field | What to write (one line) |
|---|---|
| Lab ID / Title | the day id and block with the exercise name, and one title that says what was tested, not what was found |
| Date | the date with its zone (the plan's local zone is America/Chicago), and the original calendar date if the day was shifted |
| Module and approved target | the registry id of the module or exercise, and the target id exactly as in the scope line (`G9-TGT-01`, `HOSTED:<provider>:<slug>`, `DATA:<set>`); "approved" means a signed scope line existed before the first action |
| Status | one word from the vocabulary above, written last |
| Authorized scope and exclusions | the scope line copied whole: target ids, addresses, source, window, allowed technique classes, exclusions, stop codes, RoE id, date the platform terms were read; plus the stop rule in your words |
| OS, version, and network conditions | versions as read from the system (the command and its output, not memory), the guest and baseline ids, the adapters and routes as they were, and whether the isolation tests are current; write NOT RUN for any that were not, because a switch name proves no isolation |
| Objective and preconditions | the one question the lab answers, and each thing that had to be true with how you checked it; a missing precondition makes the lab BLOCKED, not a reason to improvise |
| Hypothesis | one falsifiable sentence written before the first test, the observation it predicts and the observation that would disprove it; revise by adding a dated line, never by editing the first |
| Commands / steps (exact commands, reviewed or executed status) | each exact command text with its status word, its target, and the expected output written before the run; a stored-output reference for each EXECUTED command |
| Actual observations (timestamps, relevant evidence excerpts) | each observation with its time, zone and clock source (UTC beside local time when two sources differ), the artifact name and the excerpt, labelled OBSERVED; end with the list of evidence files and their SHA-256 |
| Root cause and trust boundary | what made it possible at the control or boundary (what trusted what), not the symptom or the tool; name the alternative causes you excluded and the test that excluded each |
| Impact and realistic exploitability limitations | what the evidence supports, with the preconditions and privilege it needs, and what it does not support: a crash is not a vulnerability, a version match is not an impact, a missing mitigation is not an exploit path |
| Defensive signals and telemetry gaps | what was logged, where, confirmed at the source and again at the collector; what was not logged; a missing event is a gap, never proof that nothing happened |
| Remediation or compensating control | the change that removes the cause, its side effects and its rollback; a compensating control is labelled as one and never replaces the fix |
| Positive retest / negative retest | positive: the permitted behaviour still works; negative: the forbidden behaviour now fails, in a near-miss form that differs from the positive in one variable; each with its artifact and time. "Retest passed" with no artifact is an evidence-integrity failure |
| Recovery / cleanup | each temporary change (account, rule, route, listener, file) and how its reversal was verified, the baseline id restored, and the isolation test rerun if a route, rule or listener was added |
| Confidence (observed, inferred, unresolved) | three lists, and every claim of the record sits in one of them; end with the independence declaration: hint level used (0 to 3), published solution or walkthrough read (yes or no, and when), AI assistant used (none, or as a critic after an unassisted attempt) |
| Reflection and spaced recall prompts | the three interrogation questions, one sentence each (what actually made this possible, how would I detect it, what would disprove it), and the 1, 3, 7 and 14 day prompts tagged with the day id; note the cards you added |

## Blank record

Copy the block. Keep the field names.

```markdown
# Lab ID / Title
Date:
Module and approved target:
Status: PROPOSED / TESTED / FAILED / VERIFIED
Authorized scope and exclusions:
OS, version, and network conditions:
Objective and preconditions:

## Hypothesis
## Commands / steps (exact commands, reviewed or executed status)
## Actual observations (timestamps, relevant evidence excerpts)
## Root cause and trust boundary
## Impact and realistic exploitability limitations
## Defensive signals and telemetry gaps
## Remediation or compensating control
## Positive retest / negative retest
## Recovery / cleanup
## Confidence (observed, inferred, unresolved)
## Reflection and spaced recall prompts
```

## Quality checks before you submit

An examiner ticks each line from the record and its evidence bundle only.

Scope and authorization

- [ ] The scope line exists, names a target id and an address, and carries a time earlier than the first command log line.
- [ ] Every action in the record names a target that the scope line lists, inside the window.
- [ ] The isolation state is stated: tests current and passed, or NOT RUN, or the lab is BLOCKED.

Reproducibility

- [ ] A stranger could repeat the lab from the record alone: versions, inputs with their hashes, exact commands with status words, expected against observed for each step.
- [ ] The expected output of each command was written before the run.
- [ ] The result was reproduced from a clean or reset state, or its non-reproducibility is recorded.

Evidence

- [ ] Each observation has a time, a zone, a clock source and the artifact it came from.
- [ ] OBSERVED, INFERRED, ASSUMED and NOT RUN claims are separated, and no inference sits in the observations section.
- [ ] The evidence files are listed with SHA-256, the bundle hash is written with its time, and redactions are separate copies that never replace an original.
- [ ] No output appears that is in no saved record.

Reasoning

- [ ] The hypothesis was written before the test and states what would disprove it.
- [ ] The root cause is stated at the control or trust boundary, and at least one alternative cause was excluded by a named test.
- [ ] Impact is bounded by the evidence and by exploitability, not by a tool label.

Defense and retest

- [ ] The defensive source was shown enabled and collected at the source and at the collector, or the gap is stated.
- [ ] The remediation acts on the stated root cause, with side effects and a rollback.
- [ ] A positive and a near-miss negative retest were run, each with an artifact, or the record says why a retest was not run.

Cleanup and honesty

- [ ] Every temporary change is shown reversed with evidence, or the record says it is not.
- [ ] The status is the lowest that the evidence supports; VERIFIED has two independent observations.
- [ ] The independence declaration is present; a published solution, if read, is declared and the lab is not counted as independent evidence.
- [ ] No real credential, host, person, organisation or third-party solution text appears anywhere in the record.

## Sample (synthetic)

Nothing below was observed. The ids, the date and the outcomes are invented to show the shape of a retest pair and of an honest status line.

```text
Sample (synthetic): not a result of any lab.
Lab ID / Title:  SYN-LAB-001, toy report file readable by an unintended group
Date:            2099-01-01 (America/Chicago)
Hypothesis:      SYN-H1 (written before the test): a synthetic account in the group
                 `syn-readers` can read the toy report file because its mode grants group
                 read. Disproved if the read fails with a permission error.
Positive retest: predicted: the owner account still reads the file. Observed (synthetic):
                 read succeeded, output stored as `syn-pos-01.txt`.
Negative retest: predicted: the `syn-readers` account is now denied. Observed (synthetic):
                 permission denied, output stored as `syn-neg-01.txt`. Differs from the
                 positive in one variable: the account.
Status:          TESTED (one observation of each; a second tool or reader was not used,
                 so it is not VERIFIED).
```

## Departures from the plan

| What | Why |
|---|---|
| The status BLOCKED is added to the four words of the plan | a blocked lab must not be recorded TESTED or VERIFIED (`docs/curriculum/v2/STUDENT_GUIDE.md` section 12); the plan uses BLOCKED for day files in section 10.1 |
| The independence declaration sits inside "Confidence" | the plan has no field for hint use or solution exposure, and the field list is kept exact; the course requires solution exposure to be declared (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 8.1) |
| The meaning of each status word is defined here | the plan lists the words and defines none. The definitions follow `docs/curriculum/v2/STUDENT_GUIDE.md` section 10. PROPOSED |
