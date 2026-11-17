# Design critique — G9 MENTOR desktop, before and after the Ledger 2 redesign (2026-11-13)

Stage: final refinement. Reviewer: the build session (one reviewer, no user test: see "Limits"). Scope: all 16 screens, the shell, the 21 themes. Method and numbers are reproducible: `.claude/skills/g9-ui-design/scripts/measure.cjs` against the mock transport in Chromium, at the default window (1280 × 800) and the minimum window (960 × 640), 16 screens × 2 sizes = 32 measurements.

Evidence labels: MEASURED (a script printed it, commands below), OBSERVED (seen in a rendered screenshot), TESTED (a test in the repository), INFERRED, NOT RUN.

## Overall impression

Before: an engineering console. It printed operation names (`session.list`), state machines (`PLANNED→READY→RUNNING`), capabilities (`DATA_DELETE`) and requirement ids (`R-002`, `A09`) in running text; creating things happened in forms inside the page body (at the bottom on several screens); the dashboard said "Charts: NO DATA"; panels pushed content past their edge (OBSERVED, `docs/research/research-synthesis-2026-11-13.md`). After: a ledger. Records are ruled regions you read, instruments are raised regions you operate, one primary action per screen sits in the page head and opens its form in place, and every figure is drawn to scale or said in words ("No data", with the reason), never as a zero. The first thing the eye finds on Mission Control is the block to start now, which is the right first thing for a person who opens the window to train.

The remaining opportunity is not visual: the interface has only been rendered in Chromium. The window the operator will use is WebKitGTK, which has not run it (NOT RUN).

## Evidence

| Measure (32 measurements) | Before (`02fd958`) | After (this commit) | Script field |
|---|---|---|---|
| Content past the page edge | 8 elements on 2 screens (Session Tracker 3, Assessments 5; up to 160 px) | 0 | `outside` |
| Text cut with an ellipsis (navigation labels exempt) | 58 on Calendar (event chips: `09:00 D18 blind practical`) | 0 | `truncated` |
| Controls under 24 × 24 px (WCAG 2.2 SC 2.5.8) | 16 on 2 screens (Mission Control 2, Curriculum Navigator 14: `summary` rows 22 px high, a `Lab` button 23 × 27) | 0 | `smallTargets` |
| Heading order and count | 4 (Mission Control: h1 then h3; Learning Journal: two h1) | 0 | `headings` |
| Controls without an accessible name, duplicate ids | 0, 0 | 0, 0 | `unnamed`, `duplicateIds` |
| Page errors in the console | 2 (both a 403 for a resource, caused by measuring the old tree through a linked `node_modules`; not a defect of that UI; excluded from the totals) | 0 | `pageErrors` |
| Total | 86 findings on 6 of 16 screens | 0 on 16 of 16 | |

Commands: `node .claude/skills/g9-ui-design/scripts/measure.cjs http://127.0.0.1:<port> --json`. The before tree was exported with `git archive 02fd958 apps/desktop` into a scratch directory and served by its own Vite process on another port; the after tree is the working tree. Both served the same mock backend.

## Per-screen, before and after

"Before" is MEASURED where a number is given, otherwise OBSERVED in the screenshots of 2026-11-13 (16 baseline images, regenerated with `.claude/skills/g9-ui-design/scripts/shoot.cjs`). "Change" is the specification in `docs/ui/handoff.md` §11.

| Screen | Before | Change | After |
|---|---|---|---|
| Mission Control | 2 small targets, heading skipped from h1 to h3; no sense of the 12 weeks | The now block is the action; four stats with their sample sizes; today's blocks as `Bars`; the program as a 12-week × 6-day `ProgramMap`; reviews, approvals and the next calendar item in the margin | 0 findings |
| Daily Classroom | no layout defect measured; system names in text | Objectives as rows; today's blocks in order with their state and a link each; what to hand in; the lesson from the mentor in the margin | 0 |
| Calendar | 58 ellipsis-cut event chips | One primary action ("Add an event", a composer); week navigation as a segmented control; chips wrap; events of the selected day with edit and delete; proposal vouchers under the control that asked | 0 |
| Curriculum Navigator | 14 rows 22 px high | One `details` per day, 24 px minimum summary; version and provenance in the margin, with what "proposed" means here | 0 |
| Session Tracker | 3 elements past the page edge (a row and a select by 160 px) | A list that picks a detail: sessions grouped (needs your correction, today, planned by you, other days); the timer instrument and the correction form in the detail column; "Plan a session" composer | 0 |
| Competency Graph | no defect measured; overflow at 960 × 640 found and fixed during the redesign (a grid item without `min-width: 0`) | Competencies that pick; the state distribution as `Segmented` or a sentence; four dimensions as a table; evidence | 0 |
| Assessments | 5 elements past the page edge (a table and its caption by 95 px) | Attempts grouped by where they stand; the lifecycle as one shared step indicator whose next step is the primary button; grading form; the voucher under the control that asked | 0 |
| Retention Center | system names in text | Four stats; the queue grouped overdue, due today, beyond budget with "Recalled", "Did not recall", "Skip" on each row; the ladder (1, 3, 7, 21, 60 days) in the margin | 0 |
| Intervention Center | system names in text | Interventions as rows with state and provenance; the lifecycle as steps; the diagnosis from the mentor | 0 |
| Learning Journal | two h1 | A single page head; notes that pick; editor instrument (title, area, classification, Markdown, preview); evidence (attach a file, use the note as evidence); rendered Markdown without links | 0 |
| Performance Dashboard | "Charts: NO DATA" | Four stats, study time by day, competencies by state, reviews by ladder step, a measures table that stays the accessible source of every chart | 0 |
| Executive Workspace | already honest (one "No data" and a table of planned operations) but in system names (`goals.list`, `decisions.record`) | The same structure in the operator's words; why business records are kept apart in the margin | 0 |
| Mentor Memory Explorer | system names in text | Hypotheses as rows with provenance; what a hypothesis is and who decides, in the margin | 0 |
| Research Center | system names in text | Stats for network and sources; a sources table | 0 |
| Approvals/Audit | system names in text | Stats (waiting for you, chain status); vouchers (`.slip`) for what waits; decided requests folded; the audit table | 0 |
| Settings/Recovery | one long page, sections stacked with no way to jump between them | Sections (backups, export, model, appearance, about) with "On this page" links; where to keep a copy and what an approval is for | 0 |

## Usability

| Finding | Severity | Status |
|---|---|---|
| Creating things lived in forms inside the page body, at the bottom on several screens | 🟡 | Fixed: one primary action in the page head opens a `Composer` in place |
| Content past the page edge at both window sizes | 🔴 | Fixed: every child of a layout grid has `min-width: 0`; tables scroll inside their own wrapper; `layout-guard.test.ts` checks that every `fr` track has a definite minimum |
| Event chips cut with an ellipsis | 🟡 | Fixed: chips wrap; truncation is allowed only for navigation labels |
| Targets under 24 px | 🟡 | Fixed (`summary`, `a.row-title`, the empty-state link) |
| System names, state machines, capabilities and requirement ids in running text | 🟡 | Fixed by a copy pass: the operator's words first (`src/lib/copy.ts`, `docs/ui/ux-copy.md`), the exact name in `title` and `code` |
| A confirmation that did not say what it covered | 🔴 | Fixed in the backend (ADR-0014 addendum 2); the in-window card shows the same facts and says that the confirmation window decides |
| Screen stylesheets had no effect (a rule of layer `screens` ranked below `components`) | 🔴 | Fixed and guarded: `theme.css` is the first import of `main.tsx`; every screen stylesheet repeats the layer order; `layer-order.test.ts` |
| `LEVEL_ANCHORS` (the rubric level descriptions) is defined in a screen file | 🟢 | Open: move to `src/lib/rubric.ts` |
| `copy.ts` has no operator wording for reliability classes, grade sources, evidence kinds and audit outcomes; the backend's name is shown | 🟡 | Open |
| The mock transport differs from the backend in two places: `assessment.submit` ignores `notes`; `assessment.begin` does not infer retakes | 🟡 | Open (the screens were tested against the mock) |
| "Expires after 10 minutes" in an approval card is carried copy, not read from the backend | 🟢 | Open |

## Visual hierarchy

- **What draws the eye first:** the page title (28 to 36 px), then the one primary action in the head, then the first record or instrument. On Mission Control the now block. Correct for each screen's job (`handoff.md` §1: show what stands, offer the one next action).
- **Reading flow:** one column of records with a margin of context on the right at 1280 px (about 61 rem of page); below 56 rem the margin folds under the record. The window's minimum width is therefore one column everywhere.
- **Emphasis:** ink is the only strong colour. Blue means selection, focus, today and the current step, never a status. Red means owed, highlighter hatch means in doubt, violet dashed means advice, outline means planned. Colour is never the only carrier: every stamp has a word, every chart a legend and a table.
- **Whitespace:** a 4 px scale (`--s-1` to `--s-8`); regions are separated by rules, not boxes; raised surfaces are never nested.

## Consistency

| Check | Result | Label |
|---|---|---|
| Classes used in a component that no stylesheet defines | 0 real (2 dynamic prefixes, `badge-` + state and `badge-state`, are allow-listed) | MEASURED (`ds_audit.py apps/desktop/src --check`, exit 0) |
| Classes defined / used | 173 / 162; 13 unused are reported for review | MEASURED |
| Colours outside `tokens.css` | 0 | MEASURED |
| Font sizes that are not a token or relative | 0 | MEASURED |
| `style` props | 11, all custom properties or column widths (charts 5, theme gallery 2, meter 1, table 1, day rail 2) | MEASURED |
| Per-screen overrides of shared components (steps, lifecycle, retention) | removed in favour of one shared step indicator | OBSERVED in the diff |
| Layer order | `reset < tokens < base < layout < components < screens < skins < utilities`, checked on every screen stylesheet | TESTED (`layer-order.test.ts`) |

## Accessibility

| Topic | Result | Label |
|---|---|---|
| Colour contrast | text 4.5:1, marks and control outlines 3:1, high-contrast text 7:1, in 22 palettes (Ledger light and dark plus 20) and the two high-contrast palettes; ordinal chart ramps keep at least two to one between steps in every palette | TESTED (`skins.test.ts`, 100 theme tests; the ramp validated with the dataviz validator) |
| Target size | no control under 24 × 24 CSS px on 32 measurements | MEASURED |
| Names and landmarks | no unnamed control, one h1 per screen, no duplicate ids; labelled regions | MEASURED |
| Keyboard | native buttons, links and form controls; `Ctrl K` palette; `?` ledger key; Escape closes composers and overlays; focus returns to the action | TESTED (screen tests) |
| Motion | `prefers-reduced-motion` removes all of it; only the running lamp loops | TESTED (CSS guard) |
| Screen reader, forced colours and reduced transparency in the real window | — | NOT RUN |
| WebKitGTK rendering (the Tauri window on Linux) | — | NOT RUN |

## What works well

- The grammar of standing is one vocabulary on every screen and every theme, so a mark that says "in doubt" looks the same in the day rail, a stat, a table and a chart.
- "No data" is said in words with its reason; the interface never draws an estimate.
- Records and instruments are visibly different, which tells the operator where input is expected.
- 21 themes survive the redesign: each palette is contrast-tested, each ramp is tested, window themes keep their bars readable.
- Layout was measured at both window sizes instead of judged by eye, and the checks are scripts that stay in the repository.

## Priority recommendations

1. **Run the shell on the host and measure there.** WebKitGTK is the engine the operator uses; run `measure.cjs` against it (or its screenshots) before calling the layout final. Cheapest first step: `npm run tauri:build`, open each screen at the default and minimum window.
2. **Close the three open copy and fidelity gaps** (the `copy.ts` labels, the mock's two differences, `LEVEL_ANCHORS`): each is small, and the mock gaps mean two screens were tested against behaviour the backend does not have.
3. **Run the two-week self-study of `docs/research/research-plan.md`** (first-block latency, correction flow, export): the only evidence that can say whether the redesign changed daily use, not only appearance.

## Limits

- One reviewer, one operator, no usability session; every claim about daily use is INFERRED.
- Chromium only. WebKitGTK, the native confirmation window and the real window chrome are NOT RUN.
- Contrast computations use palette values; textures, scan lines and text shadows are not part of them.
- The three skill evals of `.claude/skills/g9-ui-design/evals/` are NOT RUN.
