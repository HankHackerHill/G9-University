# G9 Cybersecurity Mastery v2.0: the sanitized final portfolio

- Status: PROPOSED
- Written: 2026-11-18
- Source: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, section 0.1 (the `PORTFOLIO/` folder: "sanitized final work; no public active-machine spoilers"), section 4 (HTB main labs: no active-machine spoilers in the portfolio), section 5 week 12 (minimum final artifacts, two-environment fairness rule, gate, expansion points), sections 9.2, 9.4, 11, 13 and 14.3
- Rules of safety and platform terms: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (sections 8 and 11)
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN.

> **State on 2026-11-18.** No portfolio exists. No artifact named below has been produced, no lab has been built, no attempt has been scored, and no sanitization scan has been run. This file specifies what a portfolio must contain and how it is sanitized. It describes no result.

## 1. What the portfolio is

The portfolio is the learner's sanitized, evidence-backed record of what the course work demonstrated. It records what was done and observed, including what failed and what stayed unproven. It is not a certificate, not a score, and not a claim of professional or certification readiness (plan section 13).

Where it lives. The course source (`docs/curriculum/v2/`) holds only this specification. The learner's own work is kept in a private folder or in the application, not in the course source (`docs/curriculum/v2/README.md`, table of file kinds). The portfolio is therefore a package assembled outside the repository, from the learner's records, and exported only after sanitization.

The four states of the course are recorded in the package's first file with their real values (`docs/curriculum/v2/README.md`, section Status): AUTHORING COMPLETE, LAB TESTED, LEARNER PASSED, CERTIFICATION READY. Each is set only by its own evidence. A portfolio built while Stage 00 is blocked says LAB TESTED: NO.

## 2. What goes in: the plan's minimum final artifacts (week 12)

The plan's minimum list is eleven items. Each row gives the template that exists in the course layout, what a reviewer checks, and the honesty rule. A template path in this table is part of the plan's layout and is written by another task.

| # | Minimum artifact (plan section 5, week 12) | Template or source | Acceptance: what an independent reviewer can check | If the work does not support it |
|---:|---|---|---|---|
| 1 | Rules of engagement | `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 12, and the scope lines | signed RoE records of the capstone exercises: window, targets, sources, exclusions, stop conditions, containment pre-approval, closure record | none missing: an exercise without an RoE is not in the portfolio as evidence |
| 2 | Verified isolation | the isolation test records T1 to T9 of `docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11 | each test with run id, stack, baselines, policy version, commands as run, captures with hashes, and a result PASS, FAIL, INCONCLUSIVE or NOT RUN | if Stage 00 stayed blocked: the blocker record and the substitution record (`docs/curriculum/v2/HOST_READINESS.md`, section 9), and the word BLOCKER. Never "verified" |
| 3 | Accurate inventory | guest ids, versions, addresses, baseline ids, services, as running | agrees with the NIC inventory of test T8 and with the stack of the week; every discrepancy listed | state what ran, not what the design says |
| 4 | At least two independently evidenced findings where the target supports them | `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md` | reproducible by an independent reviewer: scope, environment, preconditions, procedure, observed against expected, minimal evidence with timestamps, root cause, impact, fix, retest, limits. The independent findings come from the unknown authorized target of day 57; a finding on an environment the learner seeded is not unseen evidence (plan two-environment fairness rule) | submit the real number and score accordingly; never invent a finding to meet the quota (plan gate) |
| 5 | One identity or privilege path | `docs/curriculum/v2/TEMPLATES/ATTACK_PATH_GRAPH.md` | the path from precondition to effective access, with the difference between graph reachability and achieved access stated (plan section 9.3, day 50), and the events that cover it | say which step was not achieved |
| 6 | At least three tested detections | `docs/curriculum/v2/TEMPLATES/DETECTION_RULE_TEST.md` | per detection: the source event confirmed at origin and at the SIEM, a positive fixture and its outcome, a benign negative fixture and its outcome, false positives and negatives, and an ATT&CK mapping only from observed evidence | a rule without a collected-event test is a proposal, labelled PROPOSED |
| 7 | One multi-artifact timeline | `docs/curriculum/v2/TEMPLATES/DFIR_TIMELINE.md` | each claim with its artifact, original time and zone, normalized UTC, confidence, corroborating and contradicting records, and an alternative explanation; observed and inferred kept apart | list the missing evidence |
| 8 | At least two remediations with regressions | change records and the retests | before and after states, a positive and a negative regression result, and a check that no function the user needs regressed (plan section 5, day 59) | a fix not retested is PROPOSED |
| 9 | At least three agent boundary test cases across weeks 11 and 12 | `docs/curriculum/v2/TEMPLATES/AI_AGENT_EVALUATION.md` | per case: the legitimate goal, the untrusted input, the model's proposal, the broker's independent decision, the downstream effect, the state before and after, and the audit event, each logged separately; across the cases at least two normal requests and two unauthorized proposals | state how many were run |
| 10 | Sanitized executive and technical report | `docs/curriculum/v2/TEMPLATES/EXECUTIVE_REPORT.md` and `docs/curriculum/v2/TEMPLATES/TECHNICAL_REPORT.md` | scope, methodology, asset inventory, severity rationale, preconditions, impact, reproduction, evidence, recommended fix, retest and limitations (plan section 5, week 12 expansion); an incident report with provenance, timeline and observed against inferred | a report states what was tested and what was not |
| 11 | Unresolved-risk register | a table in the package | each open risk with its evidence, why it is unresolved, what would resolve it, and who decides | an empty register is suspect: say why |

Supporting items the plan also asks for, included in the package:

| Item | Source |
|---|---|
| The twelve weekly outputs, named as in the plan | plan section 5; names in section 3 below |
| The weekly scorecards, the midpoint report with its deficiency register, and the final evidence audit | plan sections 5 (days 30 and 60), 9.1, 9.2; the scorecard template `docs/curriculum/v2/TEMPLATES/WEEKLY_SCORECARD.md` |
| The next-step benchmark (plan day 60) and a statement on certification readiness that uses official current syllabi and the learner's actual results and promises nothing (plan section 5 week 12 expansion, section 13) | the learner |
| Cloud teardown record, or the substitution record where no sandbox was approved | `docs/curriculum/v2/TEMPLATES/CLOUD_TEARDOWN.md`; `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 5 |
| Skill records for third-party exercises (section 6) | the learner |
| The sanitization record (section 5.4) | the learner |
| The statement of limits (section 7) | the learner |

Quota status, filled with the real counts and kept honest (in the package's first file):

```text
| Required | Present (count) | Status: MET / PARTIAL / NOT MET / NOT APPLICABLE (reason) | Evidence reference |
|---|---|---|---|
| RoE records |  |  |  |
| Verified isolation (T1 to T9) |  |  |  |
| Accurate inventory |  |  |  |
| Independently evidenced findings (at least 2 where supported) |  |  |  |
| Identity or privilege path (1) |  |  |  |
| Tested detections (at least 3) |  |  |  |
| Multi-artifact timeline (1) |  |  |  |
| Remediations with regressions (at least 2) |  |  |  |
| Agent boundary test cases (at least 3) |  |  |  |
| Sanitized executive and technical report |  |  |  |
| Unresolved-risk register |  |  |  |
```

## 3. Structure of the final package

The package root is outside the repository. Names are PROPOSED.

```text
g9-mastery-portfolio/
  00_STATUS_AND_LIMITS.md          # the four states, the quota status, the statement of limits, the package hash
  MANIFEST.md                      # every file with its SHA-256 and its sanitization status
  01_RULES_OF_ENGAGEMENT/          # signed RoE records and scope lines, sanitized
  02_ENVIRONMENT/                  # isolation test records T1 to T9, inventory, topology, blocker and substitution records
  03_ASSESSMENTS/                  # weekly scorecards, midpoint report and deficiency register, final evidence audit
  04_FINDINGS/                     # at least two findings where supported, one file each
  05_IDENTITY_PRIVILEGE_PATH/      # the attack-path record
  06_DETECTIONS/                   # at least three tested detections with fixtures and results
  07_TIMELINE/                     # the multi-artifact timeline and the incident report
  08_REMEDIATIONS/                 # change records with positive and negative regression results
  09_AGENT_BOUNDARY_TESTS/         # at least three cases, proposal, decision and effect logged separately
  10_REPORTS/                      # executive report and technical report
  11_RISK_REGISTER.md              # the unresolved-risk register
  12_WEEKLY_OUTPUTS/               # the twelve weekly outputs
  13_SKILL_RECORDS/                # third-party exercises as skill records (section 6)
  14_CLOUD_OR_SUBSTITUTION/        # the teardown record, or the substitution record
  15_SANITIZATION_RECORD.md        # section 5.4
```

The twelve weekly outputs (plan section 5):

| Week | File |
|---:|---|
| 01 | `W01-Linux-Baseline-and-Hardening.md` |
| 02 | `W02-Network-Service-and-Traffic-Assessment.md` |
| 03 | `W03-AD-Identity-Security-Assessment.md` |
| 04 | `W04-Server-Side-Web-Findings.md` |
| 05 | `W05-Web-and-API-Pentest-Report.md` |
| 06 | `W06-Midpoint-Penetration-Test.md` |
| 07 | `W07-Purple-Team-Detection-Engineering.md` |
| 08 | `W08-Digital-Forensics-Incident-Report.md` |
| 09 | `W09-Reverse-Engineering-and-Vulnerability-Research.md` |
| 10 | `W10-Enterprise-Active-Directory-Security-Assessment.md` |
| 11 | `W11-Cloud-and-Agentic-AI-Security-Report.md` |
| 12 | `W12-Final-Executive-and-Technical-Capstone.md` |

Header of every artifact in the package: a status label (OBSERVED, INFERRED, ASSUMED, UNVERIFIED, PROPOSED or NOT RUN, for the artifact's main claim); the scope line id and RoE id; the date and time (UTC with the original offset); the names and SHA-256 of its evidence files; a line "AI use: critic after an unassisted attempt / not used"; a line "Redaction applied: yes, see the sanitization record".

## 4. Evidence integrity inside the package

| Rule | Source |
|---|---|
| VERIFIED appears only next to evidence that the package embeds or cites. Everything else is OBSERVED, INFERRED, ASSUMED, UNVERIFIED, PROPOSED or NOT RUN | plan section 0.4 |
| No invented command, output, finding, detection, telemetry or benchmark. Where a step was not run, the artifact says NOT RUN | plan sections 0.4, 9.4 |
| Times are UTC with the original offset and zone kept | plan section 5, week 8 |
| Each evidence file is hashed; the manifest lists the hashes | plan section 11 |
| A solution read, a hint beyond the ladder, or an AI-generated solution is declared in the artifact; the exercise then does not count as independent or unseen | `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 8.1 |
| Failed attempts, false positives that were corrected, and blocked setups with their diagnostics are included. Acceptable partial performance is stated as partial | plan section 9.4 |
| No AI output stands as evidence. AI use is stated; none was used in the midpoint, the final or a Friday scored attempt | plan sections 9.1, 10.4 |

## 5. Redaction rules

### 5.1 Principles

1. Remove identity, secrets, real infrastructure and spoilers. Keep the technical meaning.
2. Never edit the original. The original stays in the private evidence folder. A redacted copy is made beside it and is the only one that enters the package (`docs/curriculum/v2/STUDENT_GUIDE.md`, the redaction line).
3. Replace consistently. The same real value gets the same token everywhere in the package, so that the reasoning still reads. The mapping from real values to tokens is kept in the private evidence folder and never enters the package.
4. Prefer text. Plain Markdown, no images. If an image is unavoidable, flatten and re-render it after redaction; do not rely on a box or blur in a layered file.
5. Include minimal excerpts, not whole logs or captures.
6. The lab's own synthetic identifiers are not redacted: the addresses of `10.77.0.0/16`, `corp.g9.test`, the guest ids and the synthetic MAC plan (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 3) are synthetic by design.

### 5.2 What is redacted

| Category | Examples | Action and token | How it is checked |
|---|---|---|---|
| Secrets | passwords, password hashes of any real account, tokens, API keys, session cookies, JSON Web Tokens (expired ones too, because they carry claims), private keys, cloud access key ids, keys in environment output, licence keys | remove; `<redacted:secret>`. Lab passwords are replaced by `<synthetic-password>` even though they are synthetic | pattern search and review (5.4) |
| The owner's network identity | the public IP address, household LAN addresses, VPN endpoint addresses and ranges, the host's name, real MAC addresses, wireless names | `<redacted:ip-public>`, `<redacted:ip-lan>`, `<redacted:host>`, `<redacted:mac>` | pattern search and review |
| Accounts and personal data | the owner's name, email addresses, user names on the host, home directory paths, platform handles | `<redacted:user>`, `<redacted:path>`, `<redacted:handle>` | pattern search and the private term list |
| Platform data | VPN profile content, platform account ids, session or instance identifiers, URLs that carry a token, the addresses of active machines | remove; `<redacted:platform>` | review |
| Cloud data | account ids (12-digit in the common provider), ARNs that contain them, access key ids, organization-revealing resource names | `<redacted:cloud-account>`; the region and the sandbox label may stay | pattern search and review |
| Active-content spoilers | a flag, an answer to a room question, a credential found on an active target, a step-by-step exploit chain, a payload sequence, a screenshot showing any of them, a file name that reveals a solution, a hash of a flag | remove entirely. The exercise appears as a skill record (section 6) | review |
| Vendor text | verbatim course, module or room text | remove; paraphrase and cite by name | review |
| Instructor material | answer keys, fixture maps, scoring keys, the contents of a commitment | never included. A hash commitment may be quoted | review |
| Third-party vulnerabilities | a flaw found in a platform's own infrastructure or in third-party software outside the designated target | never included; private disclosure only (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, STOP-10) | review |
| Weaponizable detail | exploit code or payloads beyond what an owned, intentionally vulnerable local target needs | include only the minimum for the local target, labelled; none for third-party targets | review |
| Metadata | document properties, tracked changes, hidden text, comments, editor history, archive paths, image metadata | strip by exporting clean plain text | review of the final files |

### 5.3 Active and retired third-party content

The plan bars public active-machine spoilers. This course goes further: the portfolio carries no walkthrough or solution of any third-party lab, room, machine, challenge or Sherlock, active or retired. A retired item may be written about in the learner's own words only when the platform's current guidelines allow write-ups of that item and the retired state was confirmed on the day (registry `w03-htb-writeup-guidelines`, SEARCH_CHECKED 2026-11-18, `docs/curriculum/v2/WEEKS/W03/RESOURCES.json`), and even then the default is a skill record. Open decision O-PF-01.

### 5.4 Process and verification

| Step | Action |
|---|---|
| 1. Classify | list each file and mark which categories of 5.2 it can contain |
| 2. Redact | make a redacted copy; apply the tokens; record the private mapping |
| 3. Verify by search | run the proposed pattern searches below over the package copy; review every match; a search with no match is not proof of absence |
| 4. Verify by reading | read every file once against the table of 5.2, as a stranger would |
| 5. Second review | a second reading on another day, or by the instructor role, with the list of categories |
| 6. Hash | compute the SHA-256 of every file and of the package; write `MANIFEST.md` and the package hash in `00_STATUS_AND_LIMITS.md` |
| 7. Record | write the sanitization record: date and time, who, which searches were run and their results, which categories were found and removed, the tool names used. Searches that were not run are marked NOT RUN |

Proposed pattern searches (class A, none run, none approved). Each searches the package copy only and writes nothing; the privilege is user; the expected result is no match, or a list of matches that are each reviewed; the rollback is none needed (read-only). `<package-dir>` is the package root; `<private-term-list>` is a file in the private evidence folder holding the owner's own identifiers, one per line (name, handle, public IP, host name), and is never placed in the package.

```text
grep -r -n -E '([0-9]{1,3}\.){3}[0-9]{1,3}' <package-dir>
grep -r -n -E '[0-9]{12}' <package-dir>
grep -r -n -i -E 'AKIA[0-9A-Z]{16}|BEGIN [A-Z ]*PRIVATE KEY|api[_-]?key|secret|token|password' <package-dir>
grep -r -n -E '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+' <package-dir>
grep -r -n -E '/home/[a-z]|[A-Za-z]:\\Users' <package-dir>
grep -r -n -i -F -f <private-term-list> <package-dir>
```

The first search lists every IPv4 address; the reviewer keeps the addresses of `10.77.0.0/16` and the documentation ranges and redacts the rest. Which dedicated secret-scanning tool, if any, is used is open decision O-PF-04; none is run.

## 6. Third-party exercises as skill records

Third-party exercises that were done independently or with hints appear as skill records, not as write-ups.

```text
Skill record
Exercise:             <provider> / <exercise name>        Date: <UTC, original offset>
Scope line id:        <id>          Platform terms read on: <date>
State seen on the day: <active / retired / not stated>
Class of weakness practised: <for example: server-side input handling, access control, log triage>
Independence:         <no hint / hint ladder level reached: 1 to 3> ; solution exposure: <none / declared: what was read>
Method in my own words: <hypothesis, what was observed, root cause, the control that fails; no flag, no payload sequence, no screenshot>
Evidence kept privately: <names and hashes; not included>
What I could not show:  <...>
```

## 7. Sharing and the statement of limits

| Rule | Detail |
|---|---|
| Default | private. Sharing with anyone is the owner's decision, made after the sanitization record and a second review |
| What is never shared | instructor keys, fixture maps, scoring keys, the private mapping, the private term list, the original evidence |
| Record | who received which package, on what date, with its hash |
| Revocation | a shared package cannot be recalled; treat sharing as permanent |
| Statement of limits (required in `00_STATUS_AND_LIMITS.md`) | "This portfolio records work done under an internal 480-hour course. It is not a certificate. It does not establish professional status or readiness for any certification. The four course states have the values stated above. Quotas not met are stated as not met." |
| Certification readiness | a readiness claim for one named certification needs a per-objective comparison of the learner's artifacts with that body's current objectives and prerequisites, made by the owner; the course alone does not produce it (plan section 13) |

## 8. Acceptance of the portfolio itself

| Criterion | Check |
|---|---|
| The package follows section 3 | folder and file list |
| The quota table of section 2 is filled with real counts | each row has an evidence reference or a stated reason |
| Every evidence file is hashed and listed in `MANIFEST.md` | recompute with `sha256sum` (proposed, read-only) |
| Every claim carries a label | review |
| Redaction is verified | the sanitization record with the searches and the second review |
| No spoiler, flag, solution or vendor text | review against 5.2 and 5.3 |
| No instructor material | review |
| The statement of limits and the four states are present | `00_STATUS_AND_LIMITS.md` |
| The unresolved-risk register exists | `11_RISK_REGISTER.md` |
| A stranger can reproduce each finding from its text | the reviewer's reading |

The numeric score (85 of 100 for the final, the safety gate and the evidence-integrity gate) belongs to the final scorecard and the final rubric (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`). The portfolio is evidence for that score, not the score.

## 9. Departures from the plan

| What | Why |
|---|---|
| The portfolio is assembled outside the course source; `docs/curriculum/v2/PORTFOLIO/` holds only this specification | the plan lists `PORTFOLIO/` in the repository layout, while the course keeps the learner's own work out of the course source (`docs/curriculum/v2/README.md`) |
| Third-party exercises appear as skill records with no walkthrough, active or retired, by default | the plan bars active-machine spoilers; the course rule is stricter (`docs/curriculum/v2/AUTHORING.md`: no walkthrough of a third-party lab, room, machine or challenge) |
| Plain text, no images | the course files carry no images; images hide metadata and redaction failures |

## 10. Open points for the owner

| ID | Decision | Default until decided |
|---|---|---|
| O-PF-01 | Whether a learner-written account of a retired third-party item is ever included | not included; skill records only |
| O-PF-02 | The package's form and location (a folder, an archive, or an export from the application) | a private folder |
| O-PF-03 | The audience, if any (a mentor, an employer, nobody) | nobody |
| O-PF-04 | The secret-scanning tool for the sanitization step, if any | none; pattern searches and review only |
