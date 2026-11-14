# G9 MENTOR desktop — design system ("The Ledger", second edition: Ledger 2)

Source of truth: `apps/desktop/src/app/theme/` (tokens, base, layout and components, shell, skins, per-screen stylesheets), `apps/desktop/src/app/layout/` (shell), `apps/desktop/src/app/components/` (18 shared components). This document explains the rules and records the audit; the code and its tests decide. Developer specification (measures, states, motion, per-screen layout): `docs/ui/handoff.md`. Words: `docs/ui/ux-copy.md`. Reasons: `docs/ui/design-direction.md`. Before and after: `docs/ui/design-critique-2026-11-13.md`. The first edition (2026-11-12) described the same grammar on an engineering-console layout; this edition replaces it.

## 1. Audit (2026-11-13)

| Item | Result | Label |
|---|---|---|
| Stylesheets | 14 (7 theme, 7 per-screen), 4,177 lines; every per-screen stylesheet repeats the layer order, then writes inside `@layer screens` | MEASURED, TESTED (`layer-order.test.ts`) |
| Classes | 173 defined, 162 used; 13 unused are reported for review (`advice`, `axis`, `badge-off`, `chart`, `doubt`, `grid`, `grid-3`, `kpi`, `mono`, `owed`, `pencilled`, `pmap-head`, `posted`) and do not fail the audit | MEASURED (`ds_audit.py apps/desktop/src`) |
| Classes used but defined nowhere | 0 real; two dynamic prefixes (`badge-` + a state, `badge-state`) are allow-listed | MEASURED |
| Colours outside `tokens.css` | 0 | MEASURED |
| Font sizes that are neither a token nor relative | 0 | MEASURED |
| `style` props (applied through the CSSOM; the CSP is `style-src 'self'`) | 11, each a custom property or a column width: charts 5, theme gallery 2, day rail 2, meter 1, table 1 | MEASURED |
| Inconsistencies found and fixed during the redesign | per-screen step indicators and lifecycle strips (replaced by one shared `.steps`); window-theme bars that hit vouchers and instruments (scoped to `.panel > .panel-head`); bevels on radios and checkboxes; stamps unreadable in title bars; ordinal chart ramps under 2:1 in four themes; screen stylesheets that lost to component rules (layer rank) | OBSERVED, each with a test or a script check |
| Token naming | one scale per concern (`--s-*`, `--text-*`, `--r-*`, `--t-*`, `--ramp-*`); a theme overrides colours and type only | OBSERVED |

Priority actions that remain: run the layout in WebKitGTK on the host (NOT RUN); move `LEVEL_ANCHORS` out of the Assessments screen into `src/lib/rubric.ts`; give `copy.ts` operator wording for reliability classes, grade sources, evidence kinds and audit outcomes; the three skill evals are NOT RUN.

## 2. Grammar of standing

The interface keeps books on training: time credited or in doubt, grades approved or provisional, plans verified or proposed, advice or record. Every mark says how far it can be trusted, by fill and by its name in words (colour is never the only carrier).

| Standing | Drawing | Token | Used for |
|---|---|---|---|
| Posted | solid ink | `--ink` | `VERIFIED` records, credited time, approved grades, completed sessions |
| Planned | outline | `--rule-2` | planned, not started |
| In doubt | highlighter hatch, or a plain highlighter tint behind text | `--doubt`, `--doubt-fill`, `--hatch-doubt` | `UNCERTAIN` time, `PROPOSED` plans, provisional results, warnings, the mock-transport banner |
| Advice | violet, dashed, mentor voice | `--advice`, `--advice-fill` | `RECOMMENDATION` (model or skill output; ADR-0014: proposes, never posts) |
| Owed | red rule or red ink | `--owed`, `--owed-fill` | overdue, failed, refused, errors |
| Missing | empty dashed line with the reason | `.nodata`, `Stat standing="missing"` | "No data" or "Not enough data" with the reason; never a zero or a placeholder |
| Live | green | `--live`, `--live-fill` | running now; the receipt for what was just posted |
| Selection | ballpoint blue | `--accent`, `--accent-fill` | selection, focus, links, today, the current step. Never a status |

Running text never sits on a hatch: hatching is an edge (warnings, banner) or sits inside a meter or chart mark.

Two kinds of region: **records** are ruled (`Panel`: a rule above, no radius) and are read; **instruments** are raised (`Instrument`, `Composer`, `Stat`, `.slip`, `.now`: radius `--r-3`) and are operated. A raised region is never inside another.

## 3. Type: one family, three voices

Recursive (variable, OFL-1.1, bundled; `licenses/Recursive-OFL-1.1.txt` ships in `dist/`). The voices are axis settings on four inherited custom properties (`--mono`, `--casl`, `--slnt`, `--crsv`), rebuilt into `font-variation-settings` on every element (`base.css`).

| Voice | Axes | Used for |
|---|---|---|
| Record | MONO 0, CASL 0 | prose, labels, headings |
| Machine | MONO 1 (`--font-machine`) | identifiers, hashes, measured numbers (`td.num`), state stamps, the clock; never labels, never capitals |
| Mentor | CASL 1, slnt −9 (`--font-mentor`) | advice (`.note`, `.margin-note`) |

A theme may set the record voice in a system serif (`--font-serif`) or in mono; the machine and mentor voices stay Recursive in every theme.

## 4. Tokens

Values: `src/app/theme/tokens.css`. Themes override colours and type only. Never write a colour, a font size or a spacing value that is not a token.

| Group | Tokens | Values |
|---|---|---|
| Surfaces | `--paper`, `--sheet`, `--sheet-2`, `--raised`, `--rule`, `--rule-2`, `--field` | OKLCH; Ledger uses `light-dark()` pairs; `--raised` is paper mixed with white (sheet in dark) |
| Ink | `--ink`, `--ink-2`, `--ink-3`, `--accent`, `--on-ink`, `--bar`, `--on-bar` | text, secondary, captions; contrast 4.5:1 or better on paper, sheet and sheet-2 |
| Standing | `--owed`, `--doubt`, `--advice`, `--live`, `--accent` and their `-fill` | fills are `color-mix(... transparent)` so they work on any surface |
| Ordinal ramp | `--ramp-1 … --ramp-4` | ink mixed 50 / 66 / 83 / 100 % over white (light) and 58 / 72 / 86 / 100 % over black (dark); parts of one whole in charts only; one hue, lightness monotone with steps of at least 0.06, the faintest step at least 2:1 on the raised surface, in every palette (tested) |
| Type | `--font`, `--font-heading`, `--font-machine`, `--font-mentor`, `--text-xs … --text-6xl` (12, 13, 14, 16, 18, 22, 28, 36, 48, 72 px), `--measure` (68ch) | |
| Space | `--s-1 … --s-8` | 4, 8, 12, 16, 24, 32, 48, 64 px |
| Radius | `--r-1 … --r-4` | 3, 7, 12, 18 px: stamps, controls, instruments, overlays; records have none |
| Controls | `--control-h`, `--control-h-sm` | 34, 28 px |
| Motion | `--t-1`, `--t-2`, `--t-3`; `--ease-out`, `--ease-press`, `--ease-settle` | 110, 200, 340 ms; `prefers-reduced-motion` removes all of it |

Derived tokens (fills, hatches, `--on-ink`, `--focus`) are declared on `:root` and on every `.skin-scope` element, because a custom property that refers to another is resolved where it is declared.

Cascade layers, declared once in `theme.css` and repeated by every per-screen stylesheet: `reset < tokens < base < layout < components < screens < skins < utilities`. The first stylesheet that names a layer fixes its rank, so `theme.css` is the first import of `src/main.tsx`.

## 5. Components

Specification (props, markup, rules): `handoff.md` §4. States and interactions: §5. Edge cases: §7. Summary with the checks that exist:

| Component | Variants | States | Accessibility | Direct test |
|---|---|---|---|---|
| `PageHead` | with or without the primary action | — | the screen's one `h1`; lede at most 68ch | screens' tests, `measure.cjs` (one h1) |
| `Panel`, `Instrument`, `Composer` | `as="aside"`; composer opens in place | composer: open, refused (stays open), accepted (closes, receipt) | labelled regions; the composer takes focus, Escape closes, focus returns to the action | screens' tests |
| `Field` | `wide`; hint; error | invalid (`aria-invalid`, `role="alert"` text) | one label per control; `describedBy` wired | screens' tests |
| `Stat` | `standing`: posted, doubt, owed, missing | missing says words and the reason | value and label in one region | `performance-dashboard.test.tsx` |
| `Table` | numeric columns, footer, barred, visible caption, widths | scrolls inside its own wrapper | caption, header scopes | screens' tests, `layout-guard.test.ts` |
| `StateBadge`, `ProvenanceBadge` | backend states in words; Verified, Proposed, Advice | — | word and glyph, never colour alone; exact name in `title` | screens' tests |
| `NoData`, `ModelUnavailable`, `Loading` | with title and action | — | `role="status"` | `warnings.test.tsx`, `screens.test.tsx` |
| `ErrorDisplay`, `Warnings` | per category and severity; merged warnings; a standing fact is a quiet note (`.warnings-quiet`), not an alarm | — | `role="alert"` / live region and region landmark; quiet notes are a labelled list and are not announced | `ErrorDisplay.test.tsx`, `Warnings.test.tsx` |
| `ProgramSetup`, `FlowOutcome` | the three setup steps (`.setup-steps`: done, to do, waiting); the voucher and refusals under the control that asked | to do, waiting (import first), done; start form with a Monday check | an ordered list named "Setup steps"; the form is labelled; the date error is `role="alert"` | `setup-flow.test.tsx`, `program-screens.test.tsx` |
| `ApprovalCard` | the voucher (`.slip`) | pending, approved, applied, expired | exact values, fingerprint, expiry; says the confirmation window decides | `approvals-surface.test.tsx` |
| `Meter`, `Bars`, `Segmented`, `ProgramMap`, `IntervalStrip`, `Legend`, `ChartFigure` | by job (section 7) | — | `role="img"` with values in words; a table beside every chart | `performance-dashboard.test.tsx`, `mission-control-errors.test.tsx` |
| `Tabs`, `Note`, `Margin`, `MarkdownPreview`, `CalendarGrid`, `ThemeGallery` | — | — | `Tabs` arrow keys; `MarkdownPreview` renders no link | `CalendarGrid.test.tsx`, `markdown.test.ts`, `appearance.test.tsx` |

Buttons: default (raised outline), `.primary` (solid ink; one per screen plus one per open composer or instrument), `.quiet`, `.danger` (red ink; irreversible), `.sm` (28 px, inside rows and tables).

## 6. Patterns the operator's own work needs

| Pattern | How it is built | Rules |
|---|---|---|
| Create a record | the page head's one primary action opens a `Composer` in place under the head; fields in `.form-grid`, buttons in `.form-actions` (primary first, then Cancel) | submit refused: stays open with what was typed and the error above; accepted: closes, a `.receipt` says what was posted, the new record is selected |
| Attach a file | Learning Journal editor: a native file input; the UI reads the bytes and sends them (the backend opens no host path); the receipt names the file and says when identical bytes were already in the store | names are one line of visible text (the backend refuses others); a refused file says why in words; a SECRET file never leaves in an export |
| Choose evidence | Assessments: the newest items of `evidence.list` as checkboxes in the submission form (`evidence_refs` are the ones checked); the Learning Journal turns a note into evidence in one press | an evidence item that was invalidated by an erasure is shown redacted, never dropped |
| Inline confirmation | an irreversible step opens a `.banner` with `role="alertdialog"`: a title that names the action, the consequence, a `.danger` button labelled with the action, a default button that keeps things as they are; focus moves to the safe button | never "Are you sure?"; protected operations additionally ask the native confirmation window, which names the record (ADR-0014 addendum 2) |
| Protected operation | `APPROVAL_REQUIRED` shows an info `ErrorDisplay` and the `ApprovalCard` under the control that asked: Approve, native confirmation, Apply with the approval | the card shows the exact values and their fingerprint; the confirmation window decides |
| Empty and unreadable | `NoData` with a title that names the state and a reason that says how records get here; "Nothing is due." for a plain nothing; a read that failed shows one `ErrorDisplay` and `NoData` with the code in the regions that depended on it | never a zero, never a blank |
| Charts | `Bars` (several ratios), `Segmented` (one whole in at most six ordered parts), `ProgramMap` (12 weeks × 6 days), `IntervalStrip` (one session), `Stat` (one figure) | fill is standing; text is ink; a legend whenever two marks appear; a table of the same figures beside every chart; one part holding everything is a sentence, not a bar |

## 7. Themes

21 themes: Ledger (house style, follows System / Light / Dark) and 20 single-scheme themes. Registry: `skins.ts`. Picker: shell sidebar, Settings → Appearance (live preview gallery), command palette (`Theme: <name>`).

| Group | Themes |
|---|---|
| House | Ledger |
| Studio | Clean Crisp, Professional, Modern Tech, Graphite, Midnight, Fjord, Deep Sea, Sakura |
| Paper and craft | Notebook, Typewriter, Archive, E-ink, Blueprint, Chalkboard |
| Retro and terminal | Hacker, Amber Terminal, Synthwave, Cyberpunk, Classic Desktop, Pocket Console |

| Rule | Mechanism |
|---|---|
| Application | `applySkin` writes inline custom properties on `<html>` through the CSSOM. The content security policy (`style-src 'self'`) permits this; no `<style>` element or `style` attribute is written into markup. Applied before the first render in `main.tsx` (no flash). |
| Regions | `data-regions`: `ruled`, `boxed` (cards), `window` (bevelled window with a title bar; the bar rule is scoped to `.panel > .panel-head` so vouchers and instruments keep their own skin) |
| Effects | `data-effect`: glow, scanlines, neon, clipped, bevel, pixel, grid, notebook, chalk, still (no motion). One per theme, in `skins.css`; bevels exclude radios, checkboxes and file inputs. |
| High contrast | `data-contrast="high"`: the theme keeps its type and shapes; its colours are removed and the high-contrast block of `tokens.css` applies; effects are off |
| Persistence | `localStorage`: `g9.skin`, `g9.theme`, `g9.contrast` (theme ids only). Unknown or unreadable values fall back to Ledger / System / default. |

### Contrast evidence

`skins.test.ts` runs on every `npm test`. For each palette (22: Ledger light and dark plus 20) and for the two high-contrast palettes of `tokens.css`:

| Check | Threshold | Basis |
|---|---|---|
| Every colour inside sRGB (tolerance 0.002 linear) | in gamut | out-of-gamut OKLCH is gamut-mapped by the browser, which would change the measured contrast |
| `ink`, `ink2`, `ink3`, `owed`, `doubt`, `advice`, `live`, `accent` on `paper`, `sheet`, `sheet2` | ≥ 4.5:1 | WCAG 2.2 SC 1.4.3 |
| `ink`, `ink2` on each standing fill composited over `paper` and `sheet`; `owed` on its fill | ≥ 4.5:1 | SC 1.4.3 |
| `paper` on `ink` (primary buttons, selection); `onBar` on `bar` | ≥ 4.5:1 | SC 1.4.3 |
| `rule2` on `paper`, `sheet`, `field` (control borders) | ≥ 3:1 | SC 1.4.11 |
| The ordinal ramp: one hue, lightness moving one way, neighbouring steps at least 0.06 apart in OKLCH lightness, all inside sRGB, the faintest step at least 2:1 on the raised surface | as stated | dataviz sequential-scale rule; the ramp was also run through the dataviz validator |
| High-contrast body text on all surfaces | ≥ 7:1 | SC 1.4.6 |

Limits: the computation uses the palette values; textures (scan lines, grids) and text shadows are not part of it. The Ledger entry of the registry must equal `tokens.css` (checked). A deliberately failing palette was used to confirm the test fails (mutation check, 2026-11-12).

## 8. Shell

| Part | Behaviour |
|---|---|
| Table of contents | 16 screens in six groups (`layout/nav.ts`; `nav.test.ts` checks every screen is in exactly one group); a count of pending approvals on Approvals/Audit; a sticky fade marks that the list scrolls at small heights |
| Search and help | the palette opens from the search field (`Ctrl K`); a "?" button opens the ledger key |
| Day rail | Today's study blocks drawn to scale by planned minutes; credited time solid, uncertain time hatched, from `curriculum.daily_plan` and `session.list`. The window keeps no clock and estimates nothing; a running block shows a lamp. Re-read after every mutating command. |
| Lamps | one column: Storage, Model, Network, each with its exact state name; the backend clock in the head |
| Tools | theme select and the scheme control (System / Light / Dark) at the foot of the sidebar |
| Command palette | `Ctrl K`: screens, themes, scheme, contrast, the ledger key. Combobox over a listbox; arrow keys, Home, End, Enter, Escape. |
| Window | opens at 1280 × 800, minimum 960 × 640; sidebar 15 rem, page up to 1600 px; below 56 rem of page width the margin and the split fold to one column |

## 9. Accessibility rules

| Rule | Where |
|---|---|
| One visible focus ring (`:focus-visible`, 2 px, offset 2 px), never removed | `base.css` |
| Skip link, landmarks, `aria-current=page` | `Shell.tsx` |
| Targets at least 24 × 24 CSS px (controls are 34 px, 28 px in rows; `summary` and row links have padding for it) | `base.css`, `components.css`; MEASURED by `measure.cjs` |
| Reduced motion: animations and transitions collapse | `base.css` |
| Forced colours: controls keep a border | `base.css` |
| Meters and charts are `role="img"` with the values in words, and a table repeats them | `Meter.tsx`, `charts.tsx` |
| Theme radios are named by the theme name; description and measured contrast are attached | `ThemeGallery.tsx` |
| One `h1` per screen; `h2` per region; names on every control; unique ids | MEASURED by `measure.cjs` |

## 10. Verification record (2026-11-13)

| Check | Result | Label |
|---|---|---|
| `npx tsc --noEmit`, `npx eslint .`, `npx vitest run` | clean; @@VITEST@@ | TESTED |
| `vite build` | fonts emitted as files (no `data:` URLs; `assetsInlineLimit: 0`), licence text in `dist/licenses/` | TESTED |
| `ds_audit.py apps/desktop/src --check` | exit 0 (section 1) | MEASURED |
| `measure.cjs`: 16 screens × 1280 × 800 and 960 × 640 | 32 measurements, 32 clean: no overflow, no ellipsis outside navigation, no target under 24 px, no unnamed control, one h1, no duplicate id, no page error | MEASURED in Chromium |
| Contrast and ramp tests, 22 palettes | pass | TESTED |
| Screenshots of all 16 screens in Ledger light and dark and six themes | reviewed by eye and delivered to the owner | OBSERVED |
| WebKitGTK (the Linux Tauri webview), the native confirmation window, screen reader, forced colours in the real window | — | NOT RUN |
| Interaction latency and cold start against `performance-budgets.md` | — | NOT RUN |
| The three evals of `.claude/skills/g9-ui-design/evals/` | — | NOT RUN |
