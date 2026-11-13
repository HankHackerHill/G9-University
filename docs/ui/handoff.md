# G9 MENTOR desktop — developer handoff (Ledger 2, 2026-11-13)

Spec for building and changing screens of `apps/desktop`. Direction and reasons: `docs/ui/design-direction.md`. Words: `docs/ui/ux-copy.md`. Token and component reference: `docs/ui/design-system.md`. Exemplar screens to copy from: `src/app/screens/MissionControl.tsx` (dashboard with margin), `PerformanceDashboard.tsx` (stats, charts, measures table), `SessionTracker.tsx` (list that picks an instrument, composer, inline confirmation).

Stack: React 19, strict TypeScript, plain CSS in cascade layers, no CSS framework, no chart library, no icon font. Content security policy: `style-src 'self'`, so no `<style>` and no `style="…"` in markup; a `style` prop on a React element is applied through the CSSOM and is allowed for custom properties and column widths only.

## 1. Overview

| | |
|---|---|
| User | One operator, at this window for hours a day, expert and sceptical |
| Job of every screen | Show what stands (posted, in doubt, owed, advice, planned) and offer the one next action |
| Data rule | Every figure is the backend's. A value that cannot be read or does not exist is said in words (`NoData`, `Stat standing="missing"`), never drawn as zero |
| Window | opens at 1280 × 800, minimum 960 × 640 (`src-tauri/tauri.conf.json`); sidebar 240 px, content up to 1600 px. Check every screen at 1280 × 800 and at 1440 × 900 |

## 2. Layout

| Region | Class | Measure | Behaviour |
|---|---|---|---|
| Shell | `.shell` | grid: side `--side-w` (15rem), head auto, main 1fr | `.shell-main` scrolls; side and head stay |
| Page | `.stack` as the screen root | gap `--s-5` (24 px) | every screen returns `<div className="stack">` |
| Page head | `PageHead` | h1 28 to 36 px, lede 15 px, max 68ch | `actions` holds the one primary action |
| Record with margin | `.with-margin` | `minmax(0,1fr)` + `--margin-w` (19rem), gap 32 px | below 56rem of page width the margin folds under |
| List that picks a detail | `.split` | 3fr / 2fr; `data-lead="end"` gives 2fr / 3fr | folds to one column below 56rem; put `.sticky` on the detail column |
| Two or three equal columns | `.grid-2`, `.grid-3` | auto-fit, min 24rem / 16rem | wraps by itself |
| Dashboard tiles | `.bento > [data-span="3…8"]` | 12 columns | span 3 is 4-across from 60rem, 2-across from 46rem, else 1 |
| Region you read | `Panel` (`.panel`) | top rule, title 16 px | `actions` sit on the title line; `note` is one line under it; `as="aside"` in the margin |
| Region you operate | `Instrument` (`.instrument`), `Composer` (`.composer`), `Stat` (`.stat`), `.slip`, `.now` | raised surface, radius `--r-3`, padding 16/24 px | never nest one raised region in another |

Container queries use the container named `page` (set on `.shell-main`). Do not write `@media (width …)` for layout.

## 3. Design tokens used

Values are in `src/app/theme/tokens.css`; themes override the colours only. Never write a colour, a font size or a spacing value that is not a token.

| Token | Value (Ledger light / dark) | Usage |
|---|---|---|
| `--paper` | `oklch(0.966 0.012 150)` / `oklch(0.215 0.022 258)` | page |
| `--sheet`, `--sheet-2` | one and two steps from paper | sidebar, hover, quiet fills |
| `--raised` | paper mixed 62 % white / `--sheet` | instruments, fields, buttons |
| `--rule`, `--rule-2` | hairline / stronger outline | region rules, control outlines |
| `--ink`, `--ink-2`, `--ink-3` | text, secondary text, captions | all text; contrast 4.5:1 or better on paper, sheet, sheet-2 |
| `--accent` (+ `--accent-fill`) | ballpoint blue | selection, focus, links, today, the current step. Never a status |
| `--owed` (+ `--owed-fill`) | red rule | overdue, failed, refused, errors |
| `--doubt` (+ `--doubt-fill`, `--hatch-doubt`) | highlighter | uncertain time, proposed, provisional, warnings |
| `--advice` (+ `--advice-fill`) | copying pencil | model or skill output only |
| `--live` (+ `--live-fill`) | green | running now, a receipt for what was just posted |
| `--ramp-1 … --ramp-4` | ink at 40/60/80/100 % | ordered parts of one whole (charts only) |
| `--s-1 … --s-8` | 4, 8, 12, 16, 24, 32, 48, 64 px | all spacing |
| `--text-xs … --text-6xl` | 12, 13, 14, 16, 18, 22, 28, 36, 48, 72 px | all type sizes |
| `--r-1 … --r-4` | 3, 7, 12, 18 px | stamps, controls, instruments, overlays. Records have no radius |
| `--control-h`, `--control-h-sm` | 34, 28 px | buttons, inputs, selects |
| `--t-1`, `--t-2`, `--t-3` | 110, 200, 340 ms | press, state change, draw-in |

Type voices: record (default), machine (`.num`, `code`, `.hash`, `td.num`: measured values and identifiers only), mentor (`.note`, `.margin-note`: advice only). Do not set labels in the machine voice or in capitals.

## 4. Components

React components are in `src/app/components/`. Class-only patterns are in `src/app/theme/components.css`.

| Component | Props | Use |
|---|---|---|
| `PageHead` | `title`, `lede`, `actions?` | once per screen, first child |
| `Panel` | `title`, `actions?`, `note?`, `as?: 'section' \| 'aside'` | a ruled region |
| `Instrument` | `title?`, `actions?`, `label?` | a raised region with controls |
| `Composer` | `title`, `onClose` | the form the primary action opens, in place, directly under the page head. Focuses its first field; Escape closes |
| `Field` | `label`, `hint?`, `error?`, `wide?`, children `({ id, describedBy, invalid }) => control` | every labelled control. Lay fields out in `.form-grid`; `.form-actions` under it holds the buttons, primary first, then `Cancel`, then an optional `.form-note` |
| `Stat` | `label`, `value`, `unit?`, `foot?`, `standing?: 'posted' \| 'doubt' \| 'owed' \| 'missing'` | one measured value. `missing` shows words ("No data", "Not enough data") and the reason in `foot` |
| `StateBadge` | `state` | a backend state in the operator's words (`stateLabel`); exact name in `title` and `data-state` |
| `ProvenanceBadge` | `label: 'VERIFIED' \| 'PROPOSED' \| 'RECOMMENDATION'` | shown as Verified, Proposed, Advice |
| `NoData` | `reason`, `title?` (default "No data"), `action?` | an empty or unreadable region. `reason` is a sentence |
| `ModelUnavailable` | `reason`, `what?` | where model output would be. Renders "<what>: no model" |
| `Loading` | `what` | "Loading <what>…" |
| `ErrorDisplay` | `error`, `onRetry?` | a failed command (`role="alert"`, `data-category`). One per failed read |
| `Warnings` | `warnings`, `label?`, `testId?` | backend disclosures of every read and action on the screen, merged with `mergeWarnings` |
| `Table` | `caption`, `headers`, `rows`, `numeric?`, `footer?`, `barred?`, `captionVisible?`, `widths?`, `emptyText?` | records in columns. Scrolls inside its own wrapper |
| `ApprovalCard` | `approval`, `zone`, `now?`, `onDecide?`, `onApply?`, `disabled?` | the voucher for a protected operation (`.slip`) |
| `Margin`, `MarginNote` | `label?` / `title?` | the margin column and a note in it |
| `Meter` | `posted`, `doubted?`, `of`, `label`, `owed?` | one ratio against its plan |
| `Bars`, `Segmented`, `ProgramMap`, `IntervalStrip`, `Legend`, `ChartFigure`, `onlyPart` | see `charts.tsx` | charts (section 8) |
| `Tabs` | `tabs`, `label` | two to four views of the same record. Not for navigation |
| `Note` | `label?` | advice from a model or a skill (violet, dashed, mentor voice). Never for a confirmation |
| `MarkdownPreview` | see file | sanitized Markdown |

| Class pattern | Markup | Use |
|---|---|---|
| `.rows` | `<ul class="rows"><li><div class="row-main"><span class="row-title"/><span class="row-sub"/></div><div class="row-end"/></li>` | a ruled list of records. `row-sub[data-standing="owed"]` is red ink |
| `.rows` with `.row-pick` | `<li data-selected><button class="row-pick" aria-pressed>…` | a list whose lines select what the detail column shows |
| `dl.pairs` | `<dl class="pairs"><dt/><dd/>…` | facts about the thing in view, name left, value right; `dd.num` for measured values |
| `.facts` | `<dl class="facts"><div><dt/><dd/></div>…` | three to six facts in a line, name above value |
| `.steps` | `<ol class="steps"><li data-step="done \| current">` | a real sequence only (attempt lifecycle, review ladder) |
| `.seg` | `<div class="seg" role="group" aria-label><button aria-pressed>` | two to four exclusive choices |
| `.check` | `<label class="check"><input type="checkbox"/> text</label>` | a boolean |
| `.banner` | `role="alert"` or `role="alertdialog"` | something that needs the operator now (interrupted time, an inline confirmation) |
| `.receipt` | `<p class="receipt" role="status">` | what you just did is posted. One line |
| `.toolbar` | `<div class="toolbar" role="group" aria-label>` | a row of sibling actions |
| `.button-link` | `<Link class="button-link">` | a navigation that looks like a button |
| `.day-group` | `<details class="day-group">` | records folded under one line |
| `.tree`, `.md`, `textarea.editor`, `.cal`, `.cal-event[data-kind]` | see `components.css` | curriculum tree, rendered Markdown, the editor, the week grid |

Buttons: default (raised outline), `.primary` (solid ink; one per screen plus one per open composer or instrument), `.quiet` (no outline; tertiary), `.danger` (red ink; irreversible), `.sm` (28 px, inside rows and tables).

## 5. States and interactions

| Element | State | Behaviour |
|---|---|---|
| Button | hover | surface one step darker; weight 560 → 620 over `--t-1` |
| Button | active | translate 1 px |
| Button | disabled | 45 % ink, no hover, still focusable by assistive technology through its label |
| Button `.primary` | disabled | sheet-2 fill, ink-3 text |
| Any control | `:focus-visible` | 2 px `--focus` outline, 2 px offset. Never removed |
| Field | invalid | `aria-invalid="true"`, red outline, `error` text under the control (`role="alert"`) |
| Row with `.row-pick` | hover / pressed | sheet fill / accent fill with a 3 px accent bar |
| Composer | open | draws in over `--t-2` (`@starting-style`), focus on first field |
| Composer | submit refused | stays open with what was typed; the error is shown above the split |
| Composer | submit accepted | closes; a `.receipt` says what was posted; the new record is selected |
| Mutation in flight | any | every control that would send a revision is disabled until the list has reloaded (`locked = busy \|\| list.loading`) |
| Protected operation | `APPROVAL_REQUIRED` | `ErrorDisplay` (info) and the `ApprovalCard` directly under the control that asked; Approve → native confirmation → Apply with approval |
| Irreversible step with a consequence | pressed | inline `.banner` `role="alertdialog"` with a title that names the action, the consequence, a `.danger` button labelled with the action and a default button that keeps things as they are; focus moves to the safe button |
| Read in flight | first load | `Loading`; on reload keep the old content and disable controls |
| Read failed | any | `ErrorDisplay` once; dependent regions show `NoData` with the code, dependent stats show `missing` |

## 6. Responsive behaviour

| Page width (container `page`) | Changes |
|---|---|
| 60rem and wider (the default 1280 px window gives about 61rem) | reference layout: margin and split side by side, four tiles across |
| 56rem to 60rem | `.bento` span 3 goes 2-across |
| below 56rem | `.with-margin` and `.split` fold to one column, margin after the main column |
| below 46rem | `.bento` one column; `.now` stacks its action under the text; page padding 16 px |

The Tauri window has a minimum size of 960 × 640 (`src-tauri/tauri.conf.json`), which leaves about 42rem of page: everything is one column there. There is no touch layout.

## 7. Edge cases

| Case | Rule |
|---|---|
| Empty list | `NoData` with a title that names the state ("No sessions yet") and a reason that says how records get here |
| Nothing to do | a sentence in `.muted` ("Nothing is due."), not a `NoData` box |
| One part holds everything in a part-to-whole chart | a sentence (`onlyPart`), not a one-segment bar |
| Long text | titles wrap (`overflow-wrap: anywhere`); identifiers and hashes in `code` / `.hash` wrap anywhere; never truncate a value with an ellipsis except navigation labels |
| Long lists | group and fold (`.day-group`); do not paginate in the window |
| Rest day | `daily_plan.unscheduled`: say "Rest day", draw no blocks |
| Not built | a `NoData` that says the part is not built, naming the operation in `code`; never a disabled mock-up |
| No model | `ModelUnavailable`; planning, notes, timers and reviews must not depend on it |
| Clock | "today" and "now" come from `useToday()` (backend clock and zone); the window keeps no clock and never ticks a timer |

## 8. Charts

| Rule | Detail |
|---|---|
| Form by job | several ratios against their own whole: `Bars`; one whole in ordered parts (six at most): `Segmented`; days of the program: `ProgramMap`; intervals of one session: `IntervalStrip`; one figure: `Stat` |
| Fill is standing | solid ink posted; hatch in doubt; outline planned; red owed; `quiet` nothing there yet; ordered parts take `ramp-1…4` |
| Text | always ink, never the colour of a mark |
| Legend | `Legend` whenever two or more marks appear |
| Table view | every chart sits in a `ChartFigure` with `table` (a `Table` of the same figures) |
| Caption | one sentence: what one mark is and what the fill means |
| Marks | bars 16 px high, 4 px radius at the data end, 2 px gaps between segments, no border around a mark that is not itself the datum |
| Never | two scales in one chart, a colour per category beyond the grammar, a figure the backend did not give |

## 9. Motion

| Element | Trigger | Animation | Duration | Easing |
|---|---|---|---|---|
| Screen change | navigation | view transition of the page title and content | `--t-2` | `--ease-out` |
| Composer | open | opacity 0 → 1, translate −6 px → 0 | `--t-2` | `--ease-out` |
| Rail and meter fill | value change | width | `--t-3` | `--ease-settle` |
| Running lamp | while RUNNING | opacity pulse | 1.8 s | `--ease-out` |
| Button | press | translate 1 px, weight | `--t-1` | `--ease-press` |

`prefers-reduced-motion: reduce` removes all of it. Nothing animates on scroll and nothing loops except the running lamp.

## 10. Accessibility

| Topic | Rule |
|---|---|
| Landmarks | `nav[aria-label="Screens"]`, `header` (the day), `main#main`, `aside` (margin) with a label; every `Panel` and `Instrument` is a labelled region |
| Headings | one `h1` (page head); `h2` per region; `h3` inside a region |
| Focus order | skip link, search, navigation, status, appearance, day rail, page head action, then the page top to bottom; a composer takes focus when it opens and returns it to the action when it closes |
| Names | every control has a visible label; a row button's name starts with the record's title |
| Status | `.receipt`, `Loading`, `NoData` are `role="status"`; errors and interruptions are `role="alert"` |
| Colour | no state by colour alone: every stamp has a word and a glyph, every chart a legend and a table |
| Contrast | text 4.5:1, marks and control outlines 3:1, in all 21 themes and both schemes (`skins.test.ts`) |
| Keyboard | everything is a native button, link or form control; `Ctrl K` palette, `?` ledger key, Escape closes composer and overlays |
| Targets | 34 px controls, 28 px in rows; rows that select are full-width buttons |

## 11. Per-screen specification

Route and title of each screen are fixed by `src/app/router.tsx`. "Primary" is the page head action.

| Screen | Layout | Primary | Main column | Detail or margin |
|---|---|---|---|---|
| Mission Control | stack, `.bento`, `.with-margin` | none (the now block is the action) | now block; four stats; today's blocks (`Bars`); objectives; the program (`ProgramMap`) | reviews, approvals, next on the calendar, from the mentor |
| Daily Classroom | `.with-margin` | link "Open the session tracker" | objectives (`.rows`); today's blocks in order with state and a link each | what to hand in (evidence artifact, link to the journal); lesson from the mentor |
| Calendar | stack | "Add an event" (composer) | toolbar (week navigation as `.seg`, "Propose review slots"); the week grid; events of the selected day (`.rows` with edit and delete) | proposal and approval vouchers appear under the control that asked |
| Curriculum Navigator | `.with-margin` | none | the week: one `details` per day with its objectives and blocks | version and provenance of the curriculum; what "proposed" means here |
| Session Tracker | `.split[data-lead="end"]` | "Plan a session" (composer) | sessions grouped: needs your correction, today, planned by you, other days | timer instrument, correction form, corrections |
| Competency Graph | `.split` | "Add a competency" (composer) | competencies (`.rows` that pick), state distribution (`Segmented` or a sentence) | the selected competency: state, four dimensions as a table, evidence |
| Assessments | `.split[data-lead="end"]` | "Define an assessment" (composer) | attempts grouped by where they stand; definitions | the selected attempt: `.steps`, the next step as primary button, grading form, grade, voucher |
| Retention Center | stack, `.bento`, `.with-margin` | "Add a review item" (composer) | four stats; queue grouped overdue, due today, beyond budget; per row "Recalled", "Did not recall", "Skip" | the ladder as `.steps` (1, 3, 7, 21, 60 days) and what each answer does |
| Intervention Center | `.with-margin` | none | interventions (`.rows`) with state and provenance | the lifecycle as `.steps`; diagnosis from the mentor |
| Learning Journal | `.split[data-lead="end"]` | "New note" | notes of this session (`.rows` that pick) | editor instrument (title, area, classification, Markdown, preview), evidence (attach a file, use the note as evidence), vouchers |
| Performance Dashboard | stack, `.bento`, `.grid-2` | none | four stats; study time by day; competencies by state; reviews by ladder step; measures table | none |
| Executive Workspace | `.with-margin` | none | what is not built, as one `NoData` and a table of the planned operations | why business records are kept apart (data domain) |
| Mentor Memory Explorer | `.with-margin` | none | hypotheses (`.rows`) with provenance | what a hypothesis is and who decides |
| Research Center | stack, `.bento` | none | stats: network, sources; sources table | none |
| Approvals/Audit | stack, `.bento` | none | stats: waiting for you, chain status; vouchers waiting; decided requests folded; audit table | none |
| Settings/Recovery | `.with-margin` | none | sections: model, backups, export, erase, appearance, about | "On this page" links; what a backup holds and where to keep a copy |

## 12. Rules for a change

| Do | Do not |
|---|---|
| reuse a component or class from section 4 | add a new visual pattern for one screen |
| put screen-only rules in `src/app/theme/screens/<screen>.css` inside `@layer screens { … }` and import it from the screen file | edit `tokens.css`, `base.css`, `components.css`, `shell.css` for one screen |
| take every word from `docs/ui/ux-copy.md` and `src/lib/copy.ts` | print a backend enum, an operation name or an id as a label |
| render the warnings of every read and action | drop a read's warnings because it "only" feeds a chart |
| keep `data-testid` hooks and landmark names when moving markup | delete or weaken a test to make a redesign pass |
| check the screen in Ledger light, Ledger dark and one boxed theme (Classic Desktop) | judge a layout from code alone |
