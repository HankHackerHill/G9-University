# G9 MENTOR — design direction (Ledger 2, 2026-11-13)

Deliverable of the `frontend-design` plan: subject, a compact token system, a layout concept, principles, and the review of that plan against the generic defaults before any code was written. The rules that follow from it are in `design-system.md`; the measurements are in `handoff.md`.

## 1. Subject, audience, job

| | |
|---|---|
| Subject | A training record kept to audit standard: 12 weeks, 6 days a week, timed study blocks, assessments, reviews, evidence. Time is credited only when it can be vouched for; a grade is provisional until approved; advice is never a record. |
| Audience | One person: the owner, a security practitioner (`docs/research/research-synthesis-2026-11-13.md`). Expert, sceptical, at this screen for hours a day. |
| The design's job | Make today executable in one glance and one press, and make the standing of every figure visible without reading. |
| The owner's brief | "the most up to date new cutting edge styles", "full design max output and effort", about twenty themes. |

Where distinctive choices come from: the vernacular of bookkeeping (eye-ease ledger paper, blue-black ink, red rules, the violet copying pencil, graphite for what is only pencilled in) and the fact that this product's raw material is time, which can be drawn to scale.

## 2. Plan

### Colour (house theme, light; every other theme keeps its own palette)

| Name | Hex | OKLCH | Role |
|---|---|---|---|
| Eye-ease paper | `#EEF6F0` | 0.966 0.012 150 | the page; the pale green of ledger stock |
| Blue-black ink | `#121E31` | 0.235 0.04 258 | the record, primary actions |
| Ballpoint blue | `#1649AE` | 0.44 0.17 262 | the pen in hand: selection, focus, links, today, now |
| Red rule | `#B71A18` | 0.5 0.19 28 | owed: overdue, failed, refused |
| Highlighter | `#F8EAB5` under `#8C5500` | 0.935 0.07 95 / 0.5 0.112 68 | in doubt: uncertain time, provisional results |
| Copying pencil | `#6833A1` | 0.45 0.17 302 | advice from a mentor, never a record |

Dark scheme ("night ledger"): paper `#131A24`, ink `#E4F1E6`, accent `#96C0FE`, the same four standings lightened. Both schemes pass the 4.5:1 and 3:1 pairs of `skins.test.ts` (lowest text contrast 4.74 light, 5.87 dark) and every colour is inside sRGB.

Depth is by lightness, not shadow: the page is paper; an instrument you operate (the now block, a composer, a stat, a popover) sits on a lighter raised surface; a record you read (tables, lists) stays on the paper under a rule. Shadows appear only under things that float.

### Type

One family, Recursive (variable, bundled), in three voices set by its axes:

| Voice | Axes | Role |
|---|---|---|
| Record | sans, linear | everything the operator reads and presses: titles 860, section titles 740, body 420 |
| Machine | MONO 1, tabular figures | measured values in tables and lists, identifiers, hashes, the session timer, the day code. A large standalone figure (a stat tile) is set in the record voice with proportional figures: equal-width digits look loose at that size |
| Mentor | CASL 1, slant −9° | advice and the notes in the margin |

Scale (px at 16 px root): 12, 13, 14, 16, 18, 22, 28, 36, 48, 72. Body 14; prose and ledes 15; line length capped at 68 characters. Labels are sentence case in the record voice; nothing is set in capitals for decoration.

### Layout

One idea: **the record in the middle, the reasons in the margin.** What you act on is in the main column; why a rule exists, what a term means and what the mentor suggests sit in a narrow margin column in the mentor voice, the way a ledger carries notes beside its entries. Below 1100 px of content width the margin folds under the section it annotates.

```
┌───────────┬──────────────────────────────────────────────────────────────┐
│ G9 Mentor │ D15  Wed 18 Nov        ▓▓▒▒░░░░░░░░░░░░░░░░░░░░   1:15 of 7 h │  day bar: today to scale
│ Search    │ Web application fundamentals, week 3 of 12           09:30  │
│           ├──────────────────────────────────────────────────────────────┤
│ Today     │ Mission Control                              [Start recall] │  page head: one primary action
│ ▍Mission… │ What is planned, what is credited, what waits for you.      │
│  Classroom│                                                              │
│  Sessions │ ┌─────────────────────────────┬────────────────────────────┐ │
│ Plan      │ │ NOW (ink block)             │ in the margin              │ │
│  Calendar │ │ Recall, 15 min   [Start]    │  3 reviews overdue         │ │
│  …        │ ├─────────────────────────────┤  1 approval waits          │ │
│           │ │ Today's blocks (ruled)      │  note from the mentor      │ │
│ ● Storage │ │ Objectives (ruled)          │                            │ │
│ ○ No model│ ├─────────────────────────────┴────────────────────────────┤ │
│ Aa Themes │ │ Program map: 12 weeks × 6 days, each day drawn by standing│ │
└───────────┴──────────────────────────────────────────────────────────────┘
```

Alignment: left throughout; numbers right-aligned in their column; no centred text except inside a single-figure stat.

Creation has one home: the page head's primary action opens a composer in place, above the list it adds to.

```
[ Plan a session ]  ──press──▶  ┌ Plan a session ──────────────────── ✕ ┐
                                │ Minutes [ 45 ]  Starts [ 14:00 ]       │
                                │ Mode ( Study | Lab | Review )          │
                                │                  [Cancel] [Plan it]    │
                                └────────────────────────────────────────┘
```

### Principles

1. **Time is drawn to scale, in ink.** A duration shown as a bar is to scale, and its fill is its standing: solid is credited, hatched is uncertain, an outline is only planned. The same drawing serves today's rail, a session's intervals and the 12-week map.
2. **The record in the middle, the reasons in the margin.**
3. **Operator's words first; the system's exact name one step away** (a tooltip, the audit screen, an error's code).
4. **One primary action per screen**, solid ink, in the page head.
5. **Instruments are raised; records are ruled.**
6. **Empty says why and what to do**; no zero stands in for a missing value.
7. **Every theme keeps the marks.**

The one bold element is principle 1. Everything else is quiet on purpose.

## 3. Review of the plan against the brief and the generic defaults

| Check | Finding | Change made |
|---|---|---|
| Would any training app get this palette? | No: eye-ease green with blue-black, red rule, highlighter and copying-pencil violet comes from ledger stationery, and each colour has one job in the grammar | none |
| First draft kept the 2026-11-12 green-black ink on green paper | Monochrome green read as a tint, not a material, and gave nothing to mark "here, now" | ink moved to blue-black; one ballpoint blue added for selection, focus and now |
| Near-black with one bright accent (the common dark look) | The dark scheme is blue-black with pale green ink and five semantic colours, not one accent; the default follows the system setting | none |
| Broadsheet look: hairline rules everywhere, no radius, dense columns | The first redesign was close to this, and it is why every screen had the same weight | rules are kept for records only; instruments are raised surfaces with radius by hierarchy; one focal block per screen |
| The card kit: identical rounded cards with one soft shadow | Avoided by the instrument/record split: only things you operate are raised, and no decorative shadow exists | stat tiles differ by standing (posted, in doubt, missing), not by decoration |
| Template chrome: capital eyebrows, middle-dot strings, "WORD — fragment", monospace labels, arrows on buttons | The first redesign used four of these (`NO DATA — reason`, `D15 · week 3 · …`, capital state stamps, mono labels) | empty states are sentences; facts are a definition list; states are words with a standing glyph; the machine voice is reserved for measured values and identifiers |
| A big number with a small label as the hero | The only big number is the session timer, which is the thing being operated | stats show their sample size and definition; none is a hero |
| "Cutting edge" (the owner's words) | Met by technique rather than fashion: OKLCH and `light-dark()`, cascade layers, container queries, view transitions between screens, `@starting-style` entry for composers, variable-font axes as state (weight on press, the casual axis for the mentor), charts drawn in CSS and SVG from tokens | each is a progressive enhancement: WebKitGTK without it shows the same layout without the motion |
| Motion | First draft animated every tile on load | one orchestrated moment only (the now block and the rail draw in on first paint); all other motion answers an action; reduced motion collapses all of it |
| Numbered markers | Used only where the content is a sequence: the attempt steps and the review ladder | none elsewhere |
