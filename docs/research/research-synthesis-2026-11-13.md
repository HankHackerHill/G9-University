# Research synthesis: the operator of G9 MENTOR (2026-11-13)

**Method:** desk synthesis of existing material; no interview, survey or usability test was run. | **Participants:** 1 (the owner and only user)
**Date range of evidence:** 2026-11-05 to 2026-11-13 | **Researcher:** build session (model), reviewed by nobody else

Evidence labels: VERIFIED (the owner's own words, quoted from the build session with their date, or a document in this repository), OBSERVED (seen in the rendered application on 2026-11-13), INFERRED (my reading of the evidence; could be wrong), ASSUMED (no evidence; stated so it can be checked).

## Executive summary

G9 MENTOR has one user, who is also its owner, a security practitioner who will train with it for hours a day over 12 weeks. Their requests ask for three things at once: an instrument they can trust (audits, honesty, approvals they confirm themselves), a tool that lets them do their own daily work without waiting for the system (create sessions, events, reviews, competencies, evidence, exports), and an interface with real visual ambition ("cutting edge", about twenty themes). The application as rendered on 2026-11-13 served the first well, the second only since this week, and the third weakly: it read like an engineering console, with operation names and requirement ids in the copy, no charts, and forms that were hard to scan. The redesign therefore keeps the honesty rules, moves the system vocabulary one step back, and makes today's work and the state of the record visible at a glance.

## Key themes

### Theme 1: "Show me", not "tell me"
**Prevalence:** 3 of 26 owner messages ask to see something rather than read about it.
**Summary:** The owner checks progress by looking. Text reports are not enough.
**Supporting evidence:**
- "Visually show me an example of where we are" (2026-11-09, VERIFIED)
- "Review the desktop ui" (2026-11-07, VERIFIED)
- "how close are we to full app finished and ready to test 100%" (2026-11-07, VERIFIED)
**Implication:** Progress through the 12 weeks, today's time and the review queue must be drawn, not only tabulated. Every design delivery to the owner needs screenshots.

### Theme 2: Visual ambition, with personality on demand
**Prevalence:** 3 messages, two of them the longest design instructions in the session.
**Supporting evidence:**
- "I want full design max output and effort. Build the entire frontend and ui/ux for g9 mentor use the most up to date new cutting edge styles" (2026-11-10, VERIFIED)
- "Add around 20 theme types including but not limited to hacker theme, retro, nostalgia, clean crisp, modern tech, professional, etc" (2026-11-10, VERIFIED)
- "Now go all in and design the entire app" (2026-11-13, VERIFIED)
**Implication:** The house style must look current and deliberate; the 21 themes stay, and every theme must keep working through any redesign. A second request for "max design" after a first redesign is evidence that the first one did not land (INFERRED).

### Theme 3: Trust comes from being shown the truth, including the unflattering part
**Prevalence:** 4 messages (2026-11-07, 11-10, 11-12, 11-13: audits, "brutally honest", code and security reviews).
**Supporting evidence:**
- "be a brutally honest but fair elite level auditor who roasts you for the wrong but will always show the most optimal and beneficial paths after" (2026-11-07, VERIFIED)
- "run extremely deep and thorough triple security audit" (2026-11-12, VERIFIED)
- "I want to see exactly what the export tests are" (2026-11-07, VERIFIED)
**Implication:** The interface must never show an invented or estimated figure, must say why something is empty, and must make it easy to see where a figure comes from. The existing rule (a missing value says so, with the reason) is right and stays; its wording can be plainer.

### Theme 4: I run this; let me do my own work
**Prevalence:** the single longest feature request (2026-11-13) lists six things the owner could not do themselves.
**Supporting evidence:**
- "Create a study session option", "Add, edit, delete calendar events", "Record a review result", "Create your own competencies, assessments or rubrics", "Attach evidence or files", "Export your data to a file" (2026-11-13, VERIFIED)
- "Ok anything else this build needs to be production ready for me only" (2026-11-13, VERIFIED)
**Implication:** Creating things is a first-class action on every screen that lists them, one press away, not a form at the bottom of the page. "For me only" removes accounts, sharing and onboarding flows from scope.

### Theme 5: Plain words for system concepts
**Prevalence:** 2 messages ask what a system term means.
**Supporting evidence:**
- "Show what the export.requests are" and "what exactly is 'ccr-0cea681c-p9jbru'" (2026-11-07, VERIFIED)
- OBSERVED 2026-11-13: screens printed operation names (`session.list`), state machines (`PLANNED→READY→RUNNING`), capabilities (`DATA_DELETE`) and requirement ids (`R-002`, `A09`, `LRN-19`) in running text.
**Implication:** Operator words first; the exact system name stays available (tooltip, audit screen, error code), never in the way.

### Theme 6: Control over cost, network and automation
**Prevalence:** 5 messages.
**Supporting evidence:**
- "No ci. No workflows and no GitHub actions unless I manually do them" (2026-11-13, VERIFIED)
- "I have free GitHub account. And don't have money for constant workflows" (2026-11-10, VERIFIED)
- Decision: "Use it without AI first" (2026-11-13, VERIFIED)
**Implication:** Nothing in the interface should nag about the absent model or the disabled network: those are the owner's settings, not faults. Status belongs in a quiet place and speaks up only when something is wrong.

## Insights → opportunities

| Insight | Opportunity | Impact | Effort |
|---|---|---|---|
| Progress is judged by eye (theme 1) | Draw time to scale everywhere: today's rail, the session's intervals, a 12-week program map | High | Medium |
| The first redesign read as an engineering console (themes 2, 5; OBSERVED) | Plain-language copy pass; system names behind a tooltip; no requirement ids in the UI | High | Medium |
| Creation was added but lives at the bottom of pages (theme 4; OBSERVED) | One primary action per screen in the page head, opening a composer in place | High | Medium |
| The dashboard has no charts ("Charts: NO DATA", OBSERVED) | A real dashboard: measures with their sample size, charts in the standing grammar, the table kept as the accessible source | High | Medium |
| Absent model and network are choices (theme 6) | Quiet status cluster; the model line reads "No model (your setting)" | Medium | Low |
| Honesty is the product (theme 3) | Keep "missing says why"; add where-from citations on every measure | Medium | Low |
| Panels clipped their content (Session Tracker, Assessments; OBSERVED) | Layout rules that cannot overflow: fluid grids, tables that scroll inside their region | High | Low |

## User segments identified

| Segment | Characteristics | Needs | Size |
|---|---|---|---|
| The operator | Owner, sole user, security practitioner, builds and audits their own tools, cost-aware, works on one Ubuntu host | Trustworthy record, fast daily flow, visible progress, full control | 1 of 1 |

No other segment exists or is planned ("for me only", VERIFIED). A reviewer role (for `VALIDATED` competencies) is deferred by the owner's decision (ADR-0015).

## Recommendations

1. **Make today executable from the first screen** (themes 1, 4): what is next, how much is credited, what waits for a decision, one press to act.
2. **Draw the record** (themes 1, 3): time to scale, standing by fill, sample sizes on every measure.
3. **Rewrite the copy for the operator** (theme 5): plain words, the system's exact names one step away.
4. **Give creation a consistent home** (theme 4): the page head's primary action and an in-place composer on every screen that lists things.
5. **Keep every theme working** (theme 2): the redesign changes structure and the house palette; the other 20 palettes and their effects must still pass their contrast checks.

## Questions for further research

- Which screen does the owner open first each day, and what do they look for? (ASSUMED: Mission Control.)
- Is the session timer watched during a block, or only started and stopped?
- How often are reviews done in one sitting, and how many at once?
- Which of the 21 themes is actually used after week one?
- Does the owner read the margin explanations, or would they rather not see them at all?

The self-run plan in `docs/research/research-plan.md` answers these in the first two weeks of real use.

## Methodology notes

- One participant, and the evidence is their instructions to a builder, not observed use: it shows what they asked for, not what they do. Nobody has used the application for a working day yet.
- All quotes are from the build session (dates as given). Spelling of two quotes was normalised ("clouding" read as "including", "chrisp" as "crisp"); meaning unchanged.
- Counts ("3 of 26 messages") are counts of the owner's messages in the session transcript, excluding one-word continuations; they indicate emphasis, not frequency of a behaviour.
- The observations of the rendered application were made in Chromium against the mock transport at 1440 px wide, not in the Tauri window on the G9 host (NOT RUN there).
