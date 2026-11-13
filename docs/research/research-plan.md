# Research plan: validating the redesign with its one user (self-run)

Format: `design:user-research` deliverable "research plan". The standard methods assume 5 to 8 participants; here there is one, the owner, who is also the researcher. The plan therefore uses methods that work with n = 1 and cost minutes, not days: a short diary, timed first-use tasks, and counts read from the application's own records. Nothing here collects data outside the G9 host, and nothing is automatic: the application has no telemetry and gets none.

## Objectives

| # | Question | Decision it informs |
|---|---|---|
| O1 | Can I start today's first block within 30 seconds of opening the window? | Whether Mission Control's "next" block is the right front door |
| O2 | Do I trust the time and grade figures without checking them elsewhere? | Whether the standing marks (solid, hatched, outlined) and the "where from" citations do their job |
| O3 | Do I find and finish the six things I asked for (session, event, review, competency or assessment, evidence, export) without help? | Whether the page-head action and the composer are discoverable |
| O4 | Which screens do I never open? | What to cut or merge after week 2 |
| O5 | Is the copy clear without the system's vocabulary? | Whether any term needs to come back |

## Method 1: first-use task run (day 1, about 25 minutes)

Run once, before reading any documentation, with a stopwatch (a phone is fine). Say aloud what you expect before each click and write one line when the result surprised you. Stop a task after 3 minutes and mark it failed: a failure is a finding about the design, not about you.

| # | Task | Success | Time limit |
|---|---|---|---|
| T1 | Find out what you should do first today and start it | A session is running | 1 min |
| T2 | Pause it, then complete it | State reads Completed and time is credited | 1 min |
| T3 | Plan an extra 30-minute review session for this afternoon | It is listed for today | 2 min |
| T4 | Add a dentist appointment tomorrow at 15:00 for an hour, then move it to 16:00 | The calendar shows it at 16:00 | 3 min |
| T5 | Delete that appointment | It is gone after you confirmed the approval | 2 min |
| T6 | Write a three-line note and attach a small text file to it | The file is listed under the note | 3 min |
| T7 | Add one review item, then record that you recalled it | Its next due date is shown | 2 min |
| T8 | Create a competency and define an assessment for it | The assessment can be begun | 3 min |
| T9 | Export your learning data and find the folder on disk | You opened `manifest.json` | 3 min |
| T10 | Find out why a figure on the dashboard has no value | You can say what is missing | 1 min |

Record per task: seconds, completed (yes, with help, no), and the surprise line.

## Method 2: two-week diary (2 minutes at the end of each study day)

Three fixed questions, answered in a note in the Learning Journal titled "Diary" (so the answers are in your own records and in your exports):

1. What did I open first today, and did it tell me what I needed?
2. What did I look for and not find, or find in the wrong place?
3. One number: how much did the interface get in my way today, 0 (not at all) to 4 (constantly)?

## Method 3: counts from the application's own records (end of week 1 and week 2)

No instrumentation is added. These are read from data the application already keeps:

| Measure | Where to read it | What it tells |
|---|---|---|
| Sessions planned by you vs planned by the curriculum | Session Tracker, or `study_sessions` in an export | Whether "plan a session" is used |
| Uncertain time left uncorrected at the end of a day | Performance Dashboard | Whether the correction flow is noticed |
| Review results recorded per day, and skips | Retention Center, or `review_attempts` in an export | Whether reviews fit the day |
| Approvals rejected or expired | Approvals/Audit | Whether the confirmation dialog interrupts at the wrong moment |
| Days with a retained backup | Settings/Recovery | Whether backing up is easy enough to be a habit |

## Analysis (30 minutes at the end of week 2)

| Signal | Threshold | Action |
|---|---|---|
| A task in method 1 failed or needed help | any | Redesign that flow first |
| A task took more than twice its limit | any | Check the copy and the position of its action |
| The same "did not find" appears in the diary on 3 or more days | 3 days | Move or rename the thing |
| Diary score 3 or 4 on more than 2 days | 2 days | Read those days' notes; fix the top cause |
| A screen not opened in two weeks | none opened | Candidate to merge or hide |

## What this plan cannot show

- Whether the design would work for anyone else: there is one user by decision.
- Anything about a second reviewer or an AI model: both are deferred.
- Learning outcomes: two weeks of interface use is not evidence about training quality.

## Timeline

| When | What | Time |
|---|---|---|
| Day 1 | Method 1 | 25 min |
| Days 1 to 12 | Method 2 | 2 min a day |
| Day 6, day 12 | Method 3 | 10 min each |
| Day 12 | Analysis; list of changes | 30 min |
