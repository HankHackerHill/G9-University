# The 12-week curriculum: design, source and how to change it

Status: **PROPOSED**. Built with the owner from the Appendix A skeleton (`data/curriculum/g9-ct9-480.seed.v0.json`, 12 weeks, 72 days, 216 objectives, 480 hours). Nothing here is approved until the operator promotes a version in the application (`docs/curriculum/provenance-and-reconciliation.md`, ADR-0022).

## Where things live

| What | Path | Edited by hand |
|---|---|---|
| The lessons and the data the application imports: one file per week | `docs/curriculum/weeks/week-01.md` to `week-12.md` | yes: this is the source |
| The manifest the application imports (schema 1.0.0, provenance `PROPOSED_SEED`) | `data/curriculum/g9-ct9-480.draft.v1.json` | no: generated |
| One `assessment.define` payload per Saturday | `data/curriculum/assessments/W01-BLIND.json` ... `W12-CAPSTONE.json` | no: generated |
| Competency by week matrix, prerequisite check, changes against the seed | `docs/curriculum/coverage.md` | no: generated |
| The builder and its checks | `scripts/build_curriculum.py` | yes |
| The Appendix A skeleton (the baseline every change is compared with) | `data/curriculum/g9-ct9-480.seed.v0.json` | no |

Change a lesson: edit its week file, run `python3 -I scripts/build_curriculum.py --write`, then `python3 -I scripts/check_curriculum_seed.py data/curriculum/g9-ct9-480.draft.v1.json`. Check without writing: omit `--write`; it fails when a rule is broken or a generated file is out of date.

## What the build enforces

| Rule | Why |
|---|---|
| 12 week files, six days each, ids `D01` to `D72`, week titles as in Appendix A | the structure the application enforces; the owner's skeleton is not renamed |
| Each objective starts with its Appendix A wording, then ` — `, then an observable success criterion (at most 500 characters) | the owner's objectives stay recognisable; "done" becomes checkable; the reconciliation diff shows additions, not rewrites |
| Each day's hand-in starts with its Appendix A artifact name, then ` — `, then the acceptance criteria | same reason |
| Each objective links 1 to 3 competencies of the eight packs | the mapping the seed lacked; the application validates the ids |
| Every competency is built by at least two objectives, and none is first taught before its prerequisites | spiral practice; the pack prerequisite edges are respected in time |
| Every block section has at least 25 words; weekday sections `Recall`, `Theory`, `Guided lab`, `Independent lab`, `Defence and verification`, `Evidence and report`; Saturday sections `Blind practical`, `Remediation`, `Report`, `Performance review`, `Assessment` | no stubs; the six weekday blocks and four Saturday blocks of the manifest are each used |
| Each day lists at least one external resource with a version and a licence note; the day's own sheet is added as an internal resource | external references carry a version (manifest rule) |
| Each Saturday defines one assessment (code, kind, competency, difficulty within that competency's band, task statement) | importable with `assessment.define` |
| No hidden or control characters in a source file | the repository scan (`crates/g9-domain/tests/source_text.rs`) |

## Learner model and time

A capable intermediate learner who works 40 hours a week: five weekdays of 7 hours and a Saturday of 5. An experienced practitioner is served by accelerators, not by a different curriculum: each week names what can be treated as review after a diagnostic, and every Saturday assessment is taken unchanged (the canonical structure and planned totals do not change; `provenance-and-reconciliation.md` step 6).

## The daily rhythm (weekday, 420 minutes)

| Block | Minutes | Purpose | What the sheet gives you |
|---|---|---|---|
| Recall | 15 | retrieval from memory of yesterday's work and one item from earlier in the week, before looking at notes | three prompts |
| Theory | 45 | the concepts the day needs, as short claims to check against the references | the claims and what to read |
| Guided lab | 120 | the technique with scaffolding, checkpoints and expected observations | numbered steps |
| Independent lab | 120 | the same skill in a changed situation, without the scaffold, so that success is not memory of the guided lab | a variation to set up and a way to draw it at random |
| Defence and verification | 60 | the control, mitigation or detection that answers what the day exposed, **and a test that proves it works** | what to build and how to prove it |
| Evidence and report | 60 | the hand-in, written so that a stranger could check it | structure and acceptance criteria |

Saturday (300 minutes): blind practical 120, remediation 60, report 60, performance review 60. The blind practical is the week's measurement; it is planned in EXAM mode and graded with the application's non-compensatory pass rule (ADR-0012): every criterion must reach the pass level.

## The weeks

| Week | Theme | Pack | Saturday assessment (competency, difficulty) |
|---|---|---|---|
| 1 | Linux, Python and secure lab engineering | linux-python-systems | W01-BLIND (`linux.blind-troubleshooting`, 3) |
| 2 | Networking, protocols and scoped reconnaissance | networking-protocols-recon | W02-BLIND (`net.failure-diagnosis`, 3) |
| 3 | Web application fundamentals | web-api-appsec | W03-BLIND (`web.authz-idor`, 3) |
| 4 | Advanced web and API security | web-api-appsec | W04-BLIND (`api.token-authz`, 4) |
| 5 | Host security and privilege boundaries | windows-identity-ad | W05-BLIND (`linux.privilege-boundaries`, 3) |
| 6 | Active Directory and identity | windows-identity-ad | W06-BLIND (`ad.delegation-permissions`, 4) |
| 7 | SOC, detection and threat hunting | detection-soc-hunting | W07-BLIND (`det.blind-telemetry`, 4) |
| 8 | DFIR, malware and hunting | dfir-forensics-malware | W08-BLIND (`dfir.blind-incident`, 4) |
| 9 | LLM application and RAG security | llm-rag-security | W09-BLIND (`llm.trust-failure-diagnosis`, 4) |
| 10 | Agent security, MCP and tool authorization | agentic-mcp-supplychain | W10-BLIND (`agent.workflow-authority`, 3) |
| 11 | AI red-team, evaluation and supply chain | agentic-mcp-supplychain | W11-BLIND (`ai.threat-model-redteam`, 4) |
| 12 | Integrated G9/CT9 capstone | all eight | W12-CAPSTONE (`ai.threat-model-redteam`, 5) |

## The blind practical, and why "blind" needs a protocol

Alone, a practical is blind only if you have not seen the faults. The task statement of each assessment names the source to use; the sources, in order of preference:

1. **A scenario prepared by another person** (a colleague, a mentor) from the week's fault catalogue, handed over when the clock starts.
2. **An unseen public challenge** chosen at random from a pool fixed before the week (for example a lab platform's list), never opened beforehand.
3. **Your own fault-injection script**, written during the week but run by a seed you did not read: it applies faults drawn with a die from the catalogue and writes its log to a sealed file you open only at grading.

Whichever is used, record it in the report. Taking the practical when you know the faults is not an assessment, and the application grades what you tell it (`SELF_ADJUDICATED` decisions, ADR-0012).

## Laboratory and safety rules

- Authorization first: every exercise is on systems you own or that the owner authorized in writing, inside the scope statement of day 1. No third-party targets, no real credentials, no real personal data.
- Isolation is demonstrated, not declared (day 1). Guests attach to an isolated network; evidence lives outside the guests.
- Malware work (week 8) is static analysis of benign fixtures you build or that are provided as safe training samples. Do not execute unknown binaries; a real sample is handled only in a disposable guest with no network and a snapshot, after the owner decides to do so.
- AI work (weeks 9 to 11) runs against models and applications you host locally, with synthetic data. Adversarial fixtures stay inside the lab.
- Every external resource is **ASSUMED** until you verify its current terms and version (`provenance-and-reconciliation.md`, resource register).

## Not decided by this document

| Question | Who decides |
|---|---|
| Which blind-practical source (above) each Saturday uses | the operator |
| Verification of each external resource's terms and version | the operator |
| NICE Framework pins on competencies (`K-UNPINNED`, `S-UNPINNED`) | the operator, after pinning a framework version |
| Promotion to `APPROVED_CANONICAL` | the operator, in the application, bound to the checksum of a reviewed diff |
