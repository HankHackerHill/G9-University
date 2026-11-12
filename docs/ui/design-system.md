# G9 MENTOR desktop — design system ("The Ledger")

Source of truth: `apps/desktop/src/app/theme/` (tokens, base, shell, components, skins), `apps/desktop/src/app/layout/` (shell), `apps/desktop/src/app/components/` (shared components). This document explains the rules; the code and its tests decide.

## 1. Grammar of standing

The interface keeps books on training: time credited or in doubt, grades approved or provisional, plans verified or proposed, advice or record. Every mark says how far it can be trusted, by fill and by its name in words (colour is never the only carrier).

| Standing | Drawing | Token | Used for |
|---|---|---|---|
| Posted | solid ink | `--ink` | `VERIFIED` records, credited time, approved grades, completed sessions |
| Pencilled | outline | `--rule-2` | planned, not started |
| In doubt | amber hatch, or a plain amber tint behind text | `--doubt`, `--doubt-fill`, `--hatch-doubt` | `UNCERTAIN` time, `PROPOSED` plans, provisional results, warnings, the mock-transport banner |
| Advice | violet, dashed, mentor voice | `--advice`, `--advice-fill` | `RECOMMENDATION` (model or skill output; ADR-0014: proposes, never posts) |
| Owed | red ink | `--owed`, `--owed-fill` | overdue, failed, refused, errors |
| Missing | empty dashed line with the reason | `.nodata` | `NO DATA — <reason>`; never a zero or a placeholder |

Running text never sits on a hatch: hatching is drawn as an edge (warnings, banner) or inside a meter.

## 2. Type: one family, three voices

Recursive (variable, OFL-1.1, bundled; `licenses/Recursive-OFL-1.1.txt` ships in `dist/`). The voices are axis settings on four inherited custom properties (`--mono`, `--casl`, `--slnt`, `--crsv`), rebuilt into `font-variation-settings` on every element (`base.css`).

| Voice | Axes | Used for |
|---|---|---|
| Record | MONO 0, CASL 0 | prose, labels, headings |
| Machine | MONO 1 (`--font-machine`) | identifiers, hashes, measured numbers (`td.num`), state stamps, the clock |
| Mentor | CASL 1, slnt −9 (`--font-mentor`) | advice (`.note`, `.mentor`) |

A theme may set the record voice in a system serif (`--font-serif`) or in mono; the machine and mentor voices stay Recursive in every theme.

## 3. Tokens

| Group | Tokens | Notes |
|---|---|---|
| Surfaces | `--paper`, `--sheet`, `--sheet-2`, `--rule`, `--rule-2`, `--field` | OKLCH; Ledger uses `light-dark()` pairs |
| Ink | `--ink`, `--ink-2`, `--ink-3`, `--accent`, `--on-ink`, `--bar`, `--on-bar` | `--accent` defaults to `--ink` |
| Standing | `--owed`, `--doubt`, `--advice`, `--live` and their fills | fills are `color-mix(... transparent)` so they work on any surface |
| Type | `--font`, `--font-heading`, `--font-machine`, `--font-mentor`, `--body-weight`, `--h-weight`, `--h-mono`, `--h-casl`, `--h-slnt`, `--text-xs` … `--text-5xl`, `--measure` (68ch) | |
| Space, radius, motion | `--s-1` … `--s-8` (4 px step), `--r-1` … `--r-4`, `--t-1` … `--t-3`, `--ease-*` | radius follows hierarchy: stamps 3 px, controls 6 px, overlays 14 px, ruled regions none |

Derived tokens (fills, hatches, `--on-ink`, `--focus`) are declared on `:root` and on every `.skin-scope` element, because a custom property that refers to another is resolved where it is declared.

Cascade layers, declared once in `theme.css`: `reset < tokens < base < layout < components < screens < skins < utilities`.

## 4. Themes

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
| Regions | `data-regions`: `ruled` (a rule above each region), `boxed` (cards), `window` (bevelled window with a title bar) |
| Effects | `data-effect`: glow, scanlines, neon, clipped, bevel, pixel, grid, notebook, chalk, still (no motion). One per theme, in `skins.css`. |
| High contrast | `data-contrast="high"`: the theme keeps its type and shapes; its colours are removed and the high-contrast block of `tokens.css` applies (pure ink on pure paper); effects are off. |
| Persistence | `localStorage`: `g9.skin`, `g9.theme`, `g9.contrast`. Unknown or unreadable values fall back to Ledger / System / default. |

### Contrast evidence

`skins.test.ts` runs on every `npm test`. For each palette (22: Ledger light and dark plus 20) and for the two high-contrast palettes of `tokens.css`:

| Check | Threshold | Basis |
|---|---|---|
| Every colour inside sRGB (tolerance 0.002 linear) | in gamut | out-of-gamut OKLCH is gamut-mapped by the browser, which would change the measured contrast |
| `ink`, `ink2`, `ink3`, `owed`, `doubt`, `advice`, `live`, `accent` on `paper`, `sheet`, `sheet2` | ≥ 4.5:1 | WCAG 2.2 SC 1.4.3 |
| `ink`, `ink2` on each standing fill composited over `paper` and `sheet`; `owed` on its fill | ≥ 4.5:1 | SC 1.4.3 |
| `paper` on `ink` (primary buttons, selection); `onBar` on `bar` | ≥ 4.5:1 | SC 1.4.3 |
| `rule2` on `paper`, `sheet`, `field` (control borders) | ≥ 3:1 | SC 1.4.11 |
| High-contrast body text on all surfaces | ≥ 7:1 | SC 1.4.6 |

Limits: the computation uses the palette values; textures (scan lines, grids) and text shadows are not part of it. The Ledger entry of the registry must equal `tokens.css` (checked). A deliberately failing palette was used to confirm the test fails (mutation check, 2026-11-12).

## 5. Shell

| Part | Behaviour |
|---|---|
| Table of contents | 16 screens in six groups (`layout/nav.ts`; `nav.test.ts` checks every screen is in exactly one group). Pending approvals show as a count on Approvals/Audit. |
| Day rail | Today's study blocks drawn to scale by planned minutes; credited time solid, uncertain time hatched, from `curriculum.daily_plan` and `session.list`. The window keeps no clock and estimates nothing; a running block shows a lamp. Re-read after every mutating command (provider mutation epoch). |
| Lamps | Model, network, storage, transport, pending approvals, backend clock (`system.status`). Each lamp carries its exact state name. |
| Command palette | `Ctrl K`: screens, themes, scheme, contrast, the ledger key. Combobox over a listbox; arrow keys, Home, End, Enter, Escape. |
| Ledger key | `?`: the grammar of §1 drawn with the real components. |

## 6. Accessibility rules

| Rule | Where |
|---|---|
| One visible focus ring (`:focus-visible`, 2 px, ink) | `base.css` |
| Skip link, landmarks, `aria-current=page` | `Shell.tsx` |
| Reduced motion: animations and transitions collapse to 1 ms | `base.css` |
| Forced colours: controls keep a border | `base.css` |
| Meters are `role=img` with the values in words | `components/Meter.tsx` |
| Theme radios are named by the theme name; description and measured contrast are attached as descriptions | `components/ThemeGallery.tsx` |

## 7. Verification record

| Check | Result | Label |
|---|---|---|
| `npm run typecheck`, `npm run lint`, `npx vitest run` | clean; 249 tests passed | TESTED |
| `vite build` | fonts emitted as files (no `data:` URLs; `assetsInlineLimit: 0`), licence text in `dist/licenses/` | TESTED |
| Production build served with the exact `tauri.conf.json` CSP; 16 screens × 4 themes and theme switching through the UI | 0 CSP violations, 0 page errors, Recursive loaded | TESTED in Chromium; WebKitGTK (the Linux Tauri webview) NOT RUN |
| Screenshots of all 21 themes | reviewed by eye | OBSERVED |
| Interaction latency and cold start against `performance-budgets.md` | — | NOT RUN |
