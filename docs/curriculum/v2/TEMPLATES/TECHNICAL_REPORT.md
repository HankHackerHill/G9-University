# Technical report template

- Source: plan section 5, week 12 authoring expansion (the penetration-test report structure: scope, methodology, asset inventory, severity rationale, preconditions, impact, reproduction, evidence, recommended fix, retest, limitations; and the incident report with artifact provenance, timeline and observed-versus-inferred analysis). Also plan sections 5 (weeks 6 and 8 outputs), 9.4 and 14.1.
- Status: PROPOSED. This is a blank template. It holds no report, and no report made with it exists in the repository.
- Reader of the template: the learner, who writes the report, and the examiner, who ticks the quality checks. Reader of a report made with it: a technical peer who must be able to reproduce the work and challenge it.
- Built from: finding records (`docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`), timeline records (`docs/curriculum/v2/TEMPLATES/DFIR_TIMELINE.md`), detection records (`docs/curriculum/v2/TEMPLATES/DETECTION_RULE_TEST.md`), agent evaluations (`docs/curriculum/v2/TEMPLATES/AI_AGENT_EVALUATION.md`) and attack path graphs (`docs/curriculum/v2/TEMPLATES/ATTACK_PATH_GRAPH.md`). The report quotes them and never contradicts them. Its summary for decision-makers is made with `docs/curriculum/v2/TEMPLATES/EXECUTIVE_REPORT.md`.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. At the midpoint (day 30), at the final (day 60), and in any week whose evidence checklist asks for a report. A week that covers only a defensive or forensic exercise uses Part B (and Part C where it applies) and leaves Part A out, and says so on its first page.

Which parts. Part A is the penetration-test report. Part B is the incident report. Part C collects the defensive validation work of the same engagement: tested detections, remediations with regressions, and agent boundary tests. The unresolved-risk register belongs to the Limitations section and feeds the executive report.

Done when the quality checks below are ticked, every finding and every timeline row traces to a saved artifact in the evidence manifest, the manifest and the bundle hash were recorded before any answer key or fixture map was opened, and a second run from the report alone (by another person, or by you from a clean state after 24 hours) is recorded with its deviations.

Handling. A report about a platform-designated active target is private: no flags, credentials found, step orders or screenshots are shared or placed in a portfolio (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 8.1). Use target ids and synthetic names, `<synthetic-password>` style placeholders, and the lab addresses of the plan (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`). A report states the use of any assistant: no AI is used in the midpoint, the final or any scored Friday attempt, and elsewhere it is a critic after an unassisted attempt (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 7).

## Part A: penetration-test report

The order is the plan's. The plan lists severity rationale, preconditions, impact, reproduction, evidence, recommended fix and retest as parts of one flat structure; they are written once for each finding, inside section 5, in that order.

| # | Section | What it holds | Evidence rule |
|---|---|---|---|
| 1 | Scope | authorization reference (scope line and RoE ids), target ids, addresses, source of the traffic, window with zone, allowed technique classes, exclusions, stop rule, environment type, isolation state | copied from the scope line; the isolation tests are listed as passed with ids, or NOT RUN |
| 2 | Methodology | the order of work, which parts were manual and which tool-assisted, each tool with its version and where it ran, the aids used (none in a scored attempt), the time spent, the command register summary | versions read from the system, not remembered; every command carries a status word (PROPOSED, REVIEWED, APPROVED, EXECUTED, NOT RUN) |
| 3 | Asset inventory | a table of each asset: target id, address, role, operating system, services and versions as observed, the source and time of each fact, in scope or not | every fact has a source and a time with zone; a state such as filtered or closed is reported as that and not as "absent"; the inventory is cross-checked against ground truth (a socket listing, the lab inventory) and says what it does not cover |
| 4 | Severity rationale | the rating method, stated once: the scale, how impact and preconditions are weighed, how a CVSS score is treated, and a table of the ratings with a one-line reason each | the argument per finding is in section 5; the method is applied identically to every finding |
| 5 | Findings | one block per finding, in this order: preconditions and required privileges; impact; reproduction; evidence; recommended fix with compensating controls; retest. The block header carries the id, title, assets, status, priority and the identifiers that were validated | each block is the record of `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`, with the same id, status and priority; Unconfirmed items and false positives have their own blocks |
| 6 | Limitations | what was not tested; tool and environment limits; time limits; assumptions; unconfirmed items and false positives; the unresolved-risk register | each limit is specific to this work; a boilerplate sentence does not count |

Unresolved-risk register (inside section 6):

| Risk id | Statement | Why unresolved | Evidence gap | Owner | Next discriminating test | Review date |
|---|---|---|---|---|---|---|

## Part B: incident report

For forensic, SOC and incident work (weeks 7 and 8, and the multi-artifact timeline of the capstone). The order is the plan's: artifact provenance, timeline, observed versus inferred.

| # | Section | What it holds | Evidence rule |
|---|---|---|---|
| B1 | Case summary | case id, the question the report answers, the artifacts examined, and the conclusion in two sentences with its confidence | the conclusion is no stronger than the weakest link it depends on |
| B2 | Artifact provenance | one row per artifact: id, where it came from, how and when it was acquired or supplied, who handled it, the original hash and algorithm, the hash verified before analysis and at the end, the acquisition limits | a hash establishes that the bytes did not change since the hash was computed. It does not establish that the acquisition was accurate. Say which |
| B3 | Timeline | the events in UTC order: original time with its zone or offset, normalised UTC time, the artifact, the event claim, its label and confidence | built from the records of `docs/curriculum/v2/TEMPLATES/DFIR_TIMELINE.md`; the original time is kept beside the normalised one; clock offsets and their uncertainty are stated |
| B4 | Observed versus inferred | a table of claims: the claim, OBSERVED or INFERRED or UNVERIFIED, the supporting artifacts, the strongest alternative explanation, the test that would separate them | intent and attribution are not asserted from weak indicators; "unknown" is a valid entry |
| B5 | Response recommendations | containment, eradication and recovery steps, ordered so that volatile evidence is captured first; the link to the lesson's detections | framed as incident response inside risk management as NIST SP 800-61 Rev. 3 does (the plan states it is final, April 2025: ASSUMED here, see the registry pointer in the departures table); no step is presented as already done unless an artifact shows it |
| B6 | Gaps and uncertainty | missing evidence and what it would show; the limits of each artifact; how each gap would change the conclusion | a missing log is a gap and not a finding of no activity |

## Part C: defensive validation results

Include it when the engagement covered detections, fixes or an agent boundary. Each row points to the full record and quotes only its outcome.

| Item | Record | Positive result | Negative result | Counts or limits | Status |
|---|---|---|---|---|---|
| detection | the id of the detection record | outcome on the positive fixture | outcome on the near-miss negatives | true and false matches on named fixtures, with the number of fixtures | the record's status |
| remediation | the finding id | the permitted function still works | the forbidden behaviour fails | what the regression did not cover | retest artifact id |
| agent boundary test | the id of the agent evaluation | the normal request succeeded | the unauthorised proposal was denied by the broker and the state digest is unchanged | the number of runs and of tests | the record's result |

The plan's minimum for the final: at least three tested detections, at least two remediations with regression tests, and at least three agent boundary test cases across weeks 11 and 12. If the work supports fewer, report the actual work. A quota is never filled with an invented result.

## Appendices

| Appendix | Holds |
|---|---|
| Evidence manifest | for every artifact: id, file name, SHA-256, time and zone it was saved, who saved it, the section of the report that cites it; and the bundle hash with its time |
| Command register | every command of the work: exact text, target id, status word, time, where the output is stored |

## Blank report

Copy the block. Delete Part A, B or C if the engagement did not cover it, and say so on the first page.

```markdown
# Technical report: <engagement or case title>

Report id:        Version:        Date (zone):
Author:           Status: draft / final
Executive report id:
Evidence bundle hash (algorithm):        Recorded at (UTC):
Aids used (none in a scored attempt):
Parts included: A / B / C

## Part A. Penetration-test report
### 1. Scope
### 2. Methodology
### 3. Asset inventory
| Target id | Address | Role | OS and services as observed | Source and time (zone) | In scope |
|---|---|---|---|---|---|
### 4. Severity rationale
Method:
| Finding id | Priority | One-line reason |
|---|---|---|
### 5. Findings
#### 5.1 <finding id>: <title>
Assets / Status / Priority / Identifiers validated:
Preconditions and required privileges:
Impact:
Reproduction (expected and observed for each step; negative control):
Evidence (artifact ids):
Recommended fix and compensating controls:
Retest (positive, negative, observed result):
### 6. Limitations
Not tested:
Tool and environment limits:
Assumptions:
Unconfirmed items and false positives:
Unresolved-risk register:

## Part B. Incident report
### B1. Case summary
### B2. Artifact provenance
| Artifact id | Source | Acquired how and when | Handler | Original hash (algorithm) | Verified before and at the end | Limits |
|---|---|---|---|---|---|---|
### B3. Timeline
| # | Original time (zone) | UTC | Artifact | Event claim | OBSERVED or INFERRED | Confidence |
|---|---|---|---|---|---|---|
### B4. Observed versus inferred
| Claim | Label | Support | Strongest alternative | Test that separates them |
|---|---|---|---|---|
### B5. Response recommendations
### B6. Gaps and uncertainty

## Part C. Defensive validation results
| Item | Record | Positive result | Negative result | Counts or limits | Status |
|---|---|---|---|---|---|

## Appendices
Evidence manifest:
Command register:
```

## Quality checks before you submit

An examiner ticks each line from the report and the evidence bundle.

Whole report

- [ ] The scope of the report equals the scope line and the RoE in target ids, window and exclusions; nothing outside it is described as tested.
- [ ] Every claim carries OBSERVED, INFERRED, ASSUMED, UNVERIFIED or NOT RUN, and every OBSERVED claim names an artifact id, a time and a zone.
- [ ] The evidence manifest lists every cited artifact with a SHA-256, and the bundle hash was recorded before any answer key or fixture map was opened.
- [ ] A second run from the report alone was made, by another person or by you from a clean state after 24 hours, and its deviations are recorded.
- [ ] Every command shown has a status word and a target id; none is shown as run without a stored output.
- [ ] The use of aids is stated, and no AI was used in a scored attempt.
- [ ] No real credential, host, person or organisation appears, and nothing of an active third-party target is disclosed.

Part A

- [ ] The methodology names every tool with a version read from the system, and says which results were manual.
- [ ] The asset inventory has a source and a time with a zone for every fact, reports filtered and closed states as such, and says what it does not cover.
- [ ] The severity method is stated once and applied to every finding the same way; no rating is copied from a tool or a score.
- [ ] Each finding block agrees with its finding record in id, status and priority, and its reproduction can be followed by a stranger.
- [ ] Every finding has a retest with a positive and a negative case and an artifact, or the reason it has not run.
- [ ] False positives and Unconfirmed items are listed, not dropped.
- [ ] The limitations are specific to this work, and the unresolved-risk register has an owner, a next test and a review date for each risk.

Part B

- [ ] Every artifact cited has a provenance row, a hash recorded at acquisition or receipt, and a hash verified before analysis and at the end.
- [ ] Every timeline row has an artifact, an original time with its zone or offset, and a UTC time; clock offsets and their uncertainty are stated.
- [ ] Every inference has the strongest alternative explanation and the test that would separate them; intent and attribution are not asserted from weak indicators.
- [ ] A missing log is written as a gap, never as proof that nothing happened.
- [ ] The response plan captures volatile evidence before any containment that would destroy it, and does not claim a step done without an artifact.

Part C

- [ ] Each detection, remediation and agent test quotes a positive and a negative result with an artifact and its limits; counts are given as counts on named fixtures, not as a rate that pretends to generalise.
- [ ] The minimum number of items is met by real work, or the shortfall is stated.

## Sample (synthetic)

Nothing below was observed. The case, the artifacts and the times are invented to show how an observed claim and an inferred claim are separated.

```text
Sample (synthetic): not a result of any case.
B4 rows
Claim: account syn-svc-01 logged on at 2099-01-01T10:00:00Z. Label: OBSERVED. Support:
  artifact SYN-A1 (event export, hash recorded at receipt), original time 05:00:00-05:00.
  Strongest alternative: none for the record's existence; the clock offset of the
  source is unknown (UNVERIFIED), so the UTC value is only as good as that clock.
Claim: the logon was made by the account's owner. Label: INFERRED. Support: SYN-A1 only.
  Strongest alternative: a different person used the credential. Separating test: a
  second source tying the logon to a session, such as a host or network record for the
  same minute; not run.
Claim: the actor intended to read the synthetic share. Label: UNVERIFIED. Intent is not
  observable from the artifacts listed. No further claim is made.
```

## Departures from the plan

| What | Why |
|---|---|
| The plan's flat list is split into report-level sections and a per-finding block | severity rationale, preconditions, impact, reproduction, evidence, fix and retest belong to a finding, while scope, methodology, inventory and limitations belong to the engagement. The plan's order is kept. INFERRED |
| A report-level "Severity rationale" section states the method once | the plan lists the item at report level; the argument for each finding stays in the finding. PROPOSED |
| Part C and the unresolved-risk register are included | the plan lists detections, fixes and limitations among the final report's contents (section 14.1) and an unresolved-risk register among the final artifacts (section 5, week 12). The table layouts are PROPOSED |
| NIST SP 800-61 Rev. 3 and its framing in CSF 2.0 are named | the plan states that the revision is final (April 2025) and frames incident response inside risk management; version-sensitive, so ASSUMED here. The checked state is in `docs/curriculum/v2/RESOURCE_REGISTRY.md`, generated later from the week registries |
