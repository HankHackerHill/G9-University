# Review checklist for a screen

Use it on a screenshot and the code together. Each line is a question with a yes or no answer; a "no" is a finding.

## First two seconds

| Check | Why |
|---|---|
| Does the eye land on the one thing the operator should do or read first? | One focal element per screen |
| Is there at most one `.primary` button outside an open composer or instrument? | One primary action |
| Can you tell what is posted, in doubt, owed and planned without reading? | The grammar is the product |

## Integrity

| Check | Why |
|---|---|
| Is every number one the backend returned, or a sum of such numbers? | The window never estimates |
| With each read failing in turn, does the screen show one error and "No data" where the value was, and no zero? | A failed read is not a zero |
| Are the warnings of every read and action on the screen rendered? | A warning is a disclosure |
| Does anything that is not built say so, with the operation named? | No pseudo-production |
| Is model or skill output drawn as advice (violet, dashed), and its absence as "no model"? | Advice is never a record |

## Structure

| Check | Why |
|---|---|
| Are records ruled (`Panel`) and instruments raised (`Instrument`, `Composer`, `Stat`, `.slip`), with no raised card around a table and no nested raised regions? | Depth carries meaning |
| Does the layout hold at 1280×800 with both columns, and fold cleanly below 56rem? | The default window |
| Does a wide table scroll inside its own wrapper, without widening the page? | `Table` wraps itself |
| Is a long list grouped and folded, with what needs the operator first? | No paging in the window |
| Is anything truncated with an ellipsis other than a navigation label? | Values are never cut |

## Words

| Check | Why |
|---|---|
| Is every label sentence case, in the operator's words, from `src/lib/copy.ts`? | One vocabulary |
| Does every button start with a verb and name its object, and keep that verb through the flow? | "Plan a session", "Plan session", "Session planned" |
| Does every empty state say why it is empty and what to do? | Empty is direction |
| Does a confirmation name the action in its title, state the consequence, and label the safe button by what stays? | "Keep it open", not "Cancel" |
| Are there capitals for emphasis, middle dots, arrows, em dashes in labels, or "N/A"? | Off-system tells |

## Charts

| Check | Why |
|---|---|
| Is the form right for the job (ratios against their own whole: `Bars`; one whole in ordered parts: `Segmented`; one figure: `Stat`)? | Form by job |
| Does a chart with one part become a sentence? | `onlyPart` |
| Is there a legend for two or more marks, a caption, and a table view? | Identity never by fill alone |
| Is all text ink, with no label in a mark's colour? | Text wears text tokens |

## Accessibility

| Check | Why |
|---|---|
| One `h1`; an `h2` per region; every region and form named? | Landmarks |
| Can every action be reached and used with the keyboard, with a visible focus ring? | Native controls only |
| Does a composer take focus when it opens and close on Escape? | `Composer` does both |
| Is every state stamp a word with a glyph, not colour alone? | `StateBadge` |
| Does it read correctly in Ledger light, Ledger dark and `classic-desktop`? | 21 themes share the marks |

## Evidence to give with the review

Type-check and lint status, the test files run with counts, the screenshots looked at (size, scheme, theme), the result of `scripts/ds_audit.py --check`, and what was not run.
