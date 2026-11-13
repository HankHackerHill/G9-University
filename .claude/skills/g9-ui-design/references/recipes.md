# Recipes

Skeletons to copy. Paths are relative to `apps/desktop/src/app/`. Each recipe names the finished screen it was taken from; read that file for the complete version, including the error and loading branches left out here.

## Contents

1. Screen root and page head
2. Dashboard with a margin
3. List that picks an instrument
4. Primary action and composer
5. Form fields
6. Stats that can be missing
7. Empty, loading, failed, no model
8. Rows of records
9. Charts
10. Approval voucher
11. Inline confirmation
12. Receipt after a change
13. Test patterns

## 1. Screen root and page head

Every screen returns a `.stack` whose first child is the page head, then errors, then warnings.

```tsx
export function Example() {
  const list = useCommand('example.list', {});
  return (
    <div className="stack">
      <PageHead title="Example" lede="One sentence on what you do here and the rule that matters." />
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      <Warnings warnings={mergeWarnings(list.warnings)} />
      {/* regions */}
    </div>
  );
}
```

## 2. Dashboard with a margin

From `screens/MissionControl.tsx`. Stats in a `.bento` row, records in the main column, what waits for the operator in the margin.

```tsx
<section className="bento" aria-label="Today in numbers">
  <div data-span="3">{/* a Stat */}</div>
  <div data-span="3">{/* a Stat */}</div>
</section>
<div className="with-margin">
  <div className="stack">
    <Panel title="Today’s blocks">{/* a chart or rows */}</Panel>
  </div>
  <Margin label="What waits for you">
    <Panel title="Reviews" as="aside">{/* rows, or a muted sentence */}</Panel>
  </Margin>
</div>
```

## 3. List that picks an instrument

From `screens/SessionTracker.tsx`. The list is a record; the detail is the instrument. `data-lead="end"` gives the detail the wider column.

```tsx
<div className="split" data-lead="end">
  <Panel title="Sessions">
    <ul className="rows">
      {items.map((s) => (
        <li key={s.id} data-selected={selected?.id === s.id}>
          <button type="button" className="row-pick" aria-pressed={selected?.id === s.id} onClick={() => pick(s)}>
            <span className="row-main">
              <span className="row-title">{s.title}</span>
              <span className="row-sub">{s.summary}</span>
            </span>
            <span className="row-end">
              <StateBadge state={s.state} />
            </span>
          </button>
        </li>
      ))}
    </ul>
  </Panel>
  <div className="sticky stack">
    <Instrument title={selected.title} actions={<StateBadge state={selected.state} />}>
      {/* the figure, the controls, the facts */}
    </Instrument>
  </div>
</div>
```

Group a long list instead of paging it: what needs the operator first, then today, then the rest folded in `details.day-group`.

## 4. Primary action and composer

The page head holds the one primary action. It opens a `Composer` directly under the head; a refused submit keeps it open, an accepted one closes it and shows a receipt.

```tsx
const [composing, setComposing] = useState(false);

<PageHead
  title="Session Tracker"
  lede="…"
  actions={
    <button type="button" className="primary" disabled={composing} onClick={() => setComposing(true)}>
      Plan a session
    </button>
  }
/>
{composing ? (
  <Composer title="Plan a session" onClose={() => setComposing(false)}>
    <form aria-label="Plan a session" onSubmit={submit}>
      <div className="form-grid">{/* Fields */}</div>
      <div className="form-actions">
        <button type="submit" className="primary" disabled={!canSubmit}>Plan session</button>
        <button type="button" onClick={() => setComposing(false)}>Cancel</button>
        <span className="form-note">The one thing to know before sending.</span>
      </div>
    </form>
  </Composer>
) : null}
```

The verb stays the same through the flow: "Plan a session", "Plan session", "Session planned".

## 5. Form fields

`Field` wires label, hint and error to the control. The label is a noun phrase; the hint says what is allowed.

```tsx
<Field label="Length in minutes" hint="1 to 600." error={minutesError}>
  {({ id, describedBy, invalid }) => (
    <input id={id} type="number" min={1} max={600} value={minutes} aria-invalid={invalid} aria-describedby={describedBy}
      onChange={(e) => setMinutes(e.target.value)} />
  )}
</Field>
<Field wide label="Reason" hint="Kept with the correction.">
  {({ id, describedBy }) => <input id={id} type="text" required aria-describedby={describedBy} value={reason} onChange={…} />}
</Field>
```

Choices are words, not enum names: `<option value="LAB">Lab</option>`. Two to four exclusive choices can be a `.seg` of `aria-pressed` buttons; a boolean is a `label.check`.

## 6. Stats that can be missing

A figure that could not be read, or that has too few records, is said in words. Never render `0` in its place.

```tsx
{due.data === null ? (
  <Stat label="Reviews due" value="No data" standing="missing"
    foot={due.error === null ? 'The review queue is still being read.' : `The review queue could not be read (${due.error.code}).`} />
) : (
  <Stat label="Reviews due" value={total} standing={due.data.overdue.length > 0 ? 'owed' : 'posted'}
    foot={`${due.data.overdue.length} overdue`} />
)}
```

A measure with a minimum sample: `value="Not enough data"`, `foot="n = 3, minimum 4."`.

## 7. Empty, loading, failed, no model

```tsx
{list.loading && list.data === null ? <Loading what="sessions" /> : null}

{/* nothing recorded yet: say how records get here */}
<NoData title="No sessions yet" reason="Sessions appear here when a curriculum day is planned or when you plan one yourself." />

{/* nothing to do: a sentence, not a box */}
<p className="muted">Nothing is due.</p>

{/* depends on a read that failed: the ErrorDisplay is shown once elsewhere */}
<NoData reason={`Today’s plan could not be read (${plan.error.code}), so its blocks cannot be shown.`} />

{/* where model output would be */}
<ModelUnavailable reason={status.model.reason} what="Advice" />

{/* not built */}
<NoData title="Not built yet" reason="Listing stored notes is planned, not built. This list holds the notes saved in this session." />
```

## 8. Rows of records

```tsx
<ul className="rows">
  <li>
    <div className="row-main">
      <span className="row-title">{item.prompt}</span>
      <span className="row-sub" data-standing="owed">Overdue since {formatDateTime(item.due_at, zone)}</span>
    </div>
    <div className="row-end">
      <button type="button" className="sm">Recalled</button>
    </div>
  </li>
</ul>
```

Facts about one thing: `dl.pairs` (name left, value right; `dd.num` for measured values). Three to six facts in a line: `dl.facts`. Columns of the same kind of record: `Table` (`numeric` for measured columns, `widths` when a column needs a fixed size, `footer` for totals).

## 9. Charts

Pick the form by the data's job; every chart has a caption, a legend when two or more marks appear, and a table view.

```tsx
<ChartFigure
  caption="Each bar is as long as its block; the solid part is credited."
  table={<Table caption="Blocks for D15" headers={['Block', 'Planned', 'Credited', 'In doubt']} numeric={[1, 2, 3]} rows={…} />}
>
  <Bars label="Today’s blocks: credited and in-doubt time against the plan" rows={blocks.map((b) => ({
    key: b.id, name: b.name, of: b.planned_ms, posted: b.credited_ms, doubted: b.uncertain_ms, value: `${formatDuration(b.credited_ms)} of ${formatMinutes(b.minutes)}`,
  }))} />
  <Legend items={[{ mark: 'posted', label: 'Credited' }, { mark: 'doubt', label: 'In doubt' }, { mark: 'pencil', label: 'Planned' }]} />
</ChartFigure>
```

One whole in ordered parts uses `Segmented` with the ramp marks, and falls back to a sentence when one part holds everything:

```tsx
const one = onlyPart(parts);
one !== null
  ? <p>All {one.n} competencies are <strong>{one.name.toLowerCase()}</strong>.</p>
  : <ChartFigure caption="…" table={…}><Segmented parts={parts} label="Competencies by state" /><Legend items={…} /></ChartFigure>
```

Days of the program: `ProgramMap` with `programMap(sessions, todayId)` from `src/lib/program.ts`. Intervals of one session: `IntervalStrip`.

## 10. Approval voucher

A protected operation answers `APPROVAL_REQUIRED`. Use `useApprovalFlow`; show the error (it is information, not a failure) and the voucher directly under the control that asked.

```tsx
const flow = useApprovalFlow<'calendar.delete_event'>();
…
{flow.error !== null ? <ErrorDisplay error={flow.error} /> : null}
{flow.cancelled ? <ConfirmationCancelled /> : null}
{flow.approval !== null ? (
  <ApprovalCard approval={flow.approval} zone={today.zone} now={today.now ?? undefined}
    onDecide={(d) => void flow.decide(d)} onApply={() => void flow.applyWithApproval()} disabled={flow.pending} />
) : null}
```

## 11. Inline confirmation

Only for a step that cannot be undone or leaves something uncredited. Title is the action as a question; the destructive button names the action; the safe button says what stays and takes focus.

```tsx
<div className="banner" role="alertdialog" aria-labelledby={`${id}-title`} aria-describedby={`${id}-desc`}>
  <strong id={`${id}-title`}>Complete with time still in doubt?</strong>
  <p id={`${id}-desc`}>One interval has no correction: … If you complete now, that time stays in doubt and uncredited until you record a correction.</p>
  <div className="row">
    <button type="button" className="danger" onClick={confirm}>Complete without crediting</button>
    <button type="button" ref={safeRef} onClick={dismiss}>Keep it open</button>
  </div>
</div>
```

## 12. Receipt after a change

```tsx
<p className="receipt" role="status">Session planned: {formatDuration(planned.planned_ms)}. It is selected below.</p>
```

Never use `.note` for this: `.note` is advice from a model.

## 13. Test patterns

From `screens/session-plan.test.tsx` and `screens/mission-control-errors.test.tsx`.

```tsx
// drive the primary action, then the form by its labels
await user.click(await screen.findByRole('button', { name: 'Plan a session' }));
const form = await screen.findByRole('form', { name: 'Plan a session' });
await user.type(within(form).getByLabelText('Length in minutes'), '50');

// a failed read is an error once, and "No data" everywhere it was needed, never a zero
const failing = createInterceptHarness(async (op, _p, next) => (op === 'session.list' ? STORAGE_BUSY : next()));
renderWithIpc(<MissionControl />, failing);
expect(screen.getByText('Credited today').closest('.stat')).toHaveAttribute('data-standing', 'missing');
expect(screen.queryByText(/0:00:00/)).toBeNull();
```

Find a state by the operator's word (`'Paused'`), a provenance stamp by its label (`'Provenance: proposed'`), a region by its name (`getByRole('region', { name: 'Sessions you planned' })`).
