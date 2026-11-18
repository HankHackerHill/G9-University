# Executive report template

- Source: plan sections 0.1 (template named, no field list), 5 (week 6 midpoint "commercial-style report"; week 12 "sanitized executive and technical report" and unresolved-risk register), 14.1 (final report: executive and technical, findings, timeline, detections, fixes, limitations).
- Status: PROPOSED. This is a blank template. It holds no report, and no report made with it exists in the repository.
- Reader of the template: the learner, who writes the report, and the examiner, who ticks the quality checks. Reader of a report made with it: a decision-maker who is not a security specialist. In this course that person is the owner, acting as the sponsor of a synthetic engagement.
- Pair: every statement here must agree with the technical report made from `docs/curriculum/v2/TEMPLATES/TECHNICAL_REPORT.md` and with the finding records made from `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`. If they disagree, the technical report is corrected first.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. At the midpoint (day 30) as part of the commercial-style report, and at the final (day 60) as part of the sanitized capstone, and for any week whose evidence checklist asks for a report.

What it is for. It lets one person decide what to do about the risk without reading the technical report. It states what was tested, what was not, what the main findings are in terms of consequences, what remains, and what that person must decide. It is two pages or less in length (PROPOSED limit); a longer summary is a technical report in disguise.

Business context. The lab has no real business, so "business terms" need a stated context. Write the modelled context once, in one sentence, and label it ASSUMED (for example the kind of service the lab asset stands for and what an outage or a disclosure would cost the sponsor). Every business statement afterwards is an inference from that context, and is labelled INFERRED. Do not invent costs, losses, user counts or regulatory outcomes.

Sanitizing. A report made from a platform-designated active target is never published: no flags, credentials found, step orders or screenshots (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 8.1). Replace asset names with the target ids of the scope line, use synthetic names only, and keep the lab addresses of the plan (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`). No real host, person or organization appears.

Done when the quality checks below are ticked, the findings table matches the technical report row for row, and the evidence bundle of the engagement carries a recorded SHA-256 hash that the report names.

## Status vocabulary

| Where | Words |
|---|---|
| The report | draft (not yet checked against the technical report and the evidence) and final (every quality check ticked, the findings table equal to the technical report's) |
| A finding | Confirmed, Unconfirmed, False positive, Not retested, as defined in `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`; never a word of your own |
| A claim | OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN |

## Sections

| # | Section | What it holds | Rule |
|---|---|---|---|
| 0 | Document control | report id, version, date with zone, author, status (draft or final), the engagement id, the technical report id, the evidence bundle hash and its algorithm, handling note (synthetic training engagement; sanitized) | the bundle hash was recorded before this report was written |
| 1 | Audience | who reads this report, what each reader will decide, and how much security knowledge to assume | one sentence per reader. A reader with no decision to make is not an audience |
| 2 | Scope | the assets tested by target id, the window with its zone, the authorization reference (scope line and RoE record ids), the exclusions, and the environment type (owned lab, intentionally vulnerable, platform-designated, sandbox) | copied from the scope line, not paraphrased |
| 3 | Summary of risk in business terms | the overall risk in three to six sentences: what could be lost or interrupted, how easily, under what conditions, and what already limits it | no tool names, no identifiers, no jargon without a gloss; every sentence is OBSERVED, INFERRED or ASSUMED |
| 4 | What was tested and what was not | a table of areas with the depth reached, the method (manual or tool-assisted), the evidence, and the limit | the "not tested" rows are as visible as the tested ones. An area with no findings is "no issue found in the tests run", never "secure" |
| 5 | Top findings | the highest-priority findings, at most five, with the plain-language consequence, priority, status and the finding id | statuses use the vocabulary of the finding template; an Unconfirmed finding is listed as Unconfirmed and is not ranked as if it were Confirmed |
| 6 | Fixes and retests | what was corrected in the lab, what was retested and with what result, and what was recommended and not applied | a retest appears here only if its artifact is in the evidence bundle |
| 7 | Residual risk | what remains open after the work, why, and what is not known | summarizes the unresolved-risk register of the technical report; the register itself is not repeated |
| 8 | Decisions needed | the decisions the reader must take, with options, a recommendation, the consequence of delay, who decides and by when | an item the reader cannot act on is removed |
| 9 | Pointers | the technical report id, the evidence bundle hash and where the evidence is kept | no evidence is attached to this report |

## Blank report

Copy the block. Fill it in plain language.

```markdown
# Executive report: <engagement title>

Report id:           Version:           Date (zone):
Author:              Status: draft / final
Engagement id:       Technical report id:
Evidence bundle hash (algorithm):       Recorded at (UTC):
Handling: synthetic training engagement, sanitized

## 1. Audience
Reader, decision, assumed knowledge:

## 2. Scope
Assets (target ids):
Window (with zone):
Authorization (scope line id, RoE id):
Exclusions:
Environment type:
Modelled business context (ASSUMED):

## 3. Summary of risk in business terms
Overall risk and why:
What could be lost or interrupted, and how easily:
What already limits it:

## 4. What was tested and what was not
| Area | Tested to what depth | Method | Evidence | Limit or not tested |
|---|---|---|---|---|

## 5. Top findings
| Finding id | In plain language | Business consequence | Priority | Status | Next step |
|---|---|---|---|---|---|

## 6. Fixes and retests
| Finding id | Change applied or recommended | Retest result | Artifact in the bundle |
|---|---|---|---|

## 7. Residual risk
Open after this work, and why:
Not known:

## 8. Decisions needed
| Decision | Options | Recommended | If delayed | Decided by | By when |
|---|---|---|---|---|---|

## 9. Pointers
Technical report:
Evidence bundle and its hash:
```

## Quality checks before you submit

An examiner ticks each line from the report, the technical report and the evidence bundle.

Fit for the reader

- [ ] A non-specialist can state, after five minutes, the overall risk, the top two findings and the decision asked of them.
- [ ] The summary contains no tool name, no vulnerability identifier and no unexplained term.
- [ ] The report is two pages or less, or the excess is moved to the technical report.
- [ ] The audience section names each reader and the decision that reader takes.

Honest about scope

- [ ] The scope matches the scope line and the RoE in target ids, window and exclusions.
- [ ] What was not tested is listed as prominently as what was, with its reason.
- [ ] No area is called secure, safe or clean because no finding was made in it.
- [ ] The modelled business context is stated once and labelled ASSUMED; no cost, loss, user count or regulatory outcome is invented.

Consistent with the evidence

- [ ] Every finding appears in the technical report with the same id, status and priority.
- [ ] Priorities follow the rationale of the finding records (impact and preconditions), and no priority is copied from a tool.
- [ ] An Unconfirmed finding is labelled Unconfirmed, and a false positive is not presented as a finding.
- [ ] Every retest named has its artifact in the evidence bundle; "retest passed" without an artifact is not accepted.
- [ ] Each sentence of the risk summary is traceable to an OBSERVED fact, or labelled INFERRED or ASSUMED.

Residual risk and decisions

- [ ] Residual risk states what remains and what is unknown, and matches the unresolved-risk register.
- [ ] Each decision has options, a recommendation, a consequence of delay, an owner and a date, and the owner can act on it.
- [ ] A risk acceptance is a decision recorded with a name and a date, never a default.

Safety and handling

- [ ] The report names the evidence bundle hash that was recorded before the report was written.
- [ ] Nothing in it is real: no host, person, organization, credential or data; no flag or step order of an active third-party target.

## Sample (synthetic)

Nothing below was observed. The engagement, the findings and the wording are invented to show tone and level of detail.

```text
Sample (synthetic): not a result of any engagement.
Summary of risk (sample): In the modelled context (ASSUMED: a small records service that
holds customer-like synthetic data), one defect lets any signed-in user read another
user's records. It needs only a valid low-privilege account (OBSERVED in the synthetic
sample) and no special tools. Nothing shown limits repeated access, and no alert was
shown to fire. The defect does not, on the evidence in the synthetic sample, allow
changing or deleting records. Overall risk in the sample: moderate, rising to high if
the service held real personal data (INFERRED).
Not tested (sample): the administrative interface, denial of service, and every system
outside the listed target ids.
```

## Departures from the plan

| What | Why |
|---|---|
| The section list is this template's own | the plan names the template and gives it no field list; the sections follow the contents the plan lists for the final report and the executive purpose. PROPOSED |
| "Business terms" is tied to a declared, ASSUMED business context | the lab has no real business; the plan asks for business impact. Without a stated context, impact statements would be invented. INFERRED |
| A two-page limit and a five-finding limit | not in the plan; they keep the report a decision document. PROPOSED |
