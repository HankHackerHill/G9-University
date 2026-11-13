---
name: g9-ui-design
description: Design, build, review or change anything the operator sees in the G9 MENTOR desktop app (apps/desktop) so that it matches the Ledger design system. Use this whenever work touches apps/desktop/src/app (screens, components, layout, theme CSS) or docs/ui, and for any screen, form, table, chart, stat, empty state, error or warning text, button label, theme or screenshot of the app, including requests such as "add a screen", "make this look right", "critique this screen", "why does this look off", "write the copy for" or "show me the app", even when design is not mentioned. Not for backend crates, IPC schemas or the mentor runtime skills under .agents/skills.
---

# G9 UI design

The desktop app keeps books on training. Its design system, "Ledger", exists to make one thing visible without reading: **how far each figure can be trusted**. Everything below follows from that, so when a case is not covered here, ask what the operator needs to know about the standing of what they see, and draw that.

## The grammar every screen uses

| Standing | Drawn as | Token | Means |
|---|---|---|---|
| Posted | solid ink | `--ink` | verified records, credited time, final grades |
| Planned | outline | `--rule-2` | not started |
| In doubt | highlighter hatch | `--doubt`, `--hatch-doubt` | uncertain time, proposed plans, provisional results, warnings |
| Owed | red ink | `--owed` | overdue, failed, refused, errors |
| Advice | violet, dashed, mentor voice | `--advice` | model or skill output; never a record |
| Missing | pencilled box with the reason in words | `NoData`, `Stat standing="missing"` | never a zero, never a dash |
| The pen in hand | ballpoint blue | `--accent` | selection, focus, today, the current step; never a status |

Two kinds of surface: a **record** is something you read (tables, lists) and sits on the paper under a rule (`Panel`); an **instrument** is something you operate (the timer, a composer, a stat, a voucher) and is raised (`Instrument`, `Composer`, `Stat`, `.slip`, `.now`). Do not raise a record or nest instruments.

## Where the rules live

Read the section you need; do not work from memory, because these files change.

| Need | Read |
|---|---|
| Layout, tokens, components and their props, states, edge cases, charts, the per-screen table | `docs/ui/handoff.md` |
| Any word the operator reads | `docs/ui/ux-copy.md`, then use `src/lib/copy.ts` and `src/lib/error-text.ts` |
| Why the system looks the way it does, the seven principles | `docs/ui/design-direction.md` |
| Tokens and component reference, verification record | `docs/ui/design-system.md` |
| Code to copy | `references/recipes.md` in this skill, and the exemplar screens it names |
| A review list for a finished screen | `references/checklist.md` in this skill |

## Workflow

1. **Find the archetype.** Almost every screen is one of four; `references/recipes.md` has the skeleton of each.
   - a dashboard with a margin (Mission Control): stats, charts, records; reasons in the margin
   - a list that picks an instrument (Session Tracker): `.split`, `.row-pick` lines, a sticky detail
   - a queue you work through (Retention Center): stats, grouped `.rows` with per-row actions
   - a page of settings sections (Settings/Recovery)
2. **Name the one primary action.** It goes in the page head and opens a `Composer` in place. If nothing is created on the screen, there is no primary action; do not invent one.
3. **Build from the shared components and classes.** A new visual pattern needs a reason that holds for more than one screen. Screen-only CSS goes in `src/app/theme/screens/<screen>.css` inside `@layer screens { … }`, imported by the screen, tokens only.
4. **Write the words last, from the vocabulary.** Operator's words first (`stateLabel`, `operationLabel`, `domainLabel`); the backend's exact name stays one step away, in a `title`, in `code`, or on the audit screen.
5. **Cover every state before calling it done:** first load, loaded, empty, a failed read, no model, a mutation in flight, an approval pending, an action refused. A failed read shows one `ErrorDisplay`; everything that depended on it says "No data" with the code.
6. **Move the tests with the screen.** Keep each test's intent; change the words it looks for. Do not delete an assertion to make a redesign pass.
7. **Look at it.** Run the dev server and `scripts/shoot.cjs`, then read the screenshots: 1280×800 (the default window) and 1440×900, light and dark, and once in the boxed theme `classic-desktop`. Code that type-checks can still overflow, collide or look empty.
8. **Run the audit:** `python3 scripts/ds_audit.py apps/desktop/src --check`. It fails on a class no stylesheet defines and on a colour outside the tokens.

## Rules that are not negotiable

These come from the repository's integrity rules (`CLAUDE.md`), not from taste.

- Every figure is the backend's. The window sums and scales what it was given; it never estimates, never ticks a clock, never shows a zero for a value it does not have.
- Render the warnings of every read and every action on the screen (`mergeWarnings`, `Warnings`). A read that "only" feeds a chart still discloses.
- What is not built says so in words and names the operation; no mock-up, no disabled placeholder, no sample data.
- Model and skill output is advice: `Note` or `ModelUnavailable`, violet and dashed, never styled as a record.
- Protected operations go through the approval voucher (`ApprovalCard`); the page only asks.
- Content security policy is `style-src 'self'`: no `<style>`, no `style="…"`. A React `style` prop may set CSS custom properties and column widths.
- No colour, type size or spacing outside the tokens. No new dependency for UI.

## Things that mark a screen as off-system

Check a draft against these before you look for anything subtler: a label in capitals; an enum name or an operation name used as a label; a middle dot, an arrow or an em dash joining a label; a dash or `0` standing for a missing value; a `NoData` box where a muted sentence would do ("Nothing is due."); a chart with one segment; text in the colour of a chart mark; more than one `.primary` button outside an open composer; a raised card around a table; a number set large in the machine voice that is not the session timer; `Cancel` as the safe button of a confirmation.

## Scripts

| Script | Use |
|---|---|
| `scripts/shoot.cjs <outDir> <spec.json>` | drives the running app (mock backend) with Playwright and writes screenshots; the header comment documents the spec |
| `scripts/ds_audit.py <src> [--check]` | token coverage, undefined and unused classes, inline styles |

Start the dev server first: `cd apps/desktop && npx vite --host 127.0.0.1 --port 5173`. It serves the mock backend, so nothing shown is read from or saved to the real database.

## Reporting

When you finish UI work, say what you verified and how: type-check, lint, the test files you ran with counts, the screenshots you looked at (sizes, schemes, themes), the audit result. Say plainly what you did not run (the Tauri shell and WebKitGTK are usually not available outside the G9 host).
