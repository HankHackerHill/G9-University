import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { ReviewItem, ReviewOutcome, ReviewRecord } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatDateTime, formatMinutes } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

const OUTCOMES: readonly { outcome: ReviewOutcome; label: string }[] = [
  { outcome: 'SUCCESS', label: 'Recalled' },
  { outcome: 'FAILURE', label: 'Forgot' },
  { outcome: 'SKIPPED', label: 'Skip' },
];

export function RetentionCenter() {
  const today = useToday();
  const client = useIpc();
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const [prompt, setPrompt] = useState('');
  const [answer, setAnswer] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<G9Error | null>(null);
  const [actionWarnings, setActionWarnings] = useState<readonly string[]>([]);
  const [added, setAdded] = useState<ReviewItem | null>(null);
  const [recorded, setRecorded] = useState<{ prompt: string; record: ReviewRecord } | null>(null);
  // While a review is recorded or the queue re-reads, no second click can record the same item twice.
  const locked = busy || due.loading;

  const addItem = async () => {
    setBusy(true);
    const trimmedAnswer = answer.trim();
    const r = await client.invoke('retention.create_item', { prompt: prompt.trim(), ...(trimmedAnswer === '' ? {} : { answer: trimmedAnswer }) });
    setActionWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
      setAdded(r.result);
      setPrompt('');
      setAnswer('');
    }
    setBusy(false);
    due.reload();
  };

  const record = async (item: ReviewItem, outcome: ReviewOutcome) => {
    setBusy(true);
    const r = await client.invoke('retention.record_review', { review_item_id: item.review_item_id, outcome });
    setActionWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
      setRecorded(null);
    } else {
      setError(null);
      setRecorded({ prompt: item.prompt, record: r.result });
    }
    setBusy(false);
    due.reload();
  };

  const rows = (items: ReviewItem[], actions: boolean) =>
    items.map((r) => ({
      key: r.review_item_id,
      cells: [
        r.prompt,
        r.kind,
        `+${r.offset_days} d`,
        formatDateTime(r.due_at, today.zone),
        r.objective_id ?? '—',
        formatMinutes(Math.round(r.estimated_ms / 60_000)),
        ...(actions
          ? [
              <div key="record" className="row" role="group" aria-label={`Record review: ${r.prompt}`}>
                {OUTCOMES.map((o) => (
                  <button
                    key={o.outcome}
                    type="button"
                    disabled={locked}
                    onClick={() => {
                      void record(r, o.outcome);
                    }}
                  >
                    {o.label}
                  </button>
                ))}
              </div>,
            ]
          : []),
      ],
    }));
  const headers = ['Prompt', 'Kind', 'Offset', 'Due', 'Objective', 'Est.'];
  const withActions = [...headers, 'Record'];

  return (
    <div>
      <PageHead title="Retention Center" lede="Reviews that are overdue, due today, and beyond today's review budget." />
      <p className="muted">
        Baseline schedule: offsets 1, 3, 7, 21, 60 days after the last successful recall; a failed recall resets to day 1. The daily review budget caps the queue; overflow
        is reported, never dropped (ARCHITECTURE §9). FSRS is an optional, item-level feature that is OFF.
      </p>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      {due.loading ? <Loading what="review queue" /> : null}
      <Warnings warnings={mergeWarnings(due.warnings, actionWarnings)} />
      {recorded !== null ? <RecordedReview prompt={recorded.prompt} record={recorded.record} zone={today.zone} /> : null}
      {due.data !== null ? (
        <>
          <p>
            As of {formatDateTime(due.data.as_of, today.zone)} · budget {formatMinutes(Math.round(due.data.budget_ms / 60_000))} <ProvenanceBadge label="VERIFIED" />
          </p>
          <p className="muted">
            Record each review as you do it: Recalled moves the item to the next offset, Forgot sends it back to one day, Skip records the attempt and keeps the due
            date. The backend computes the next date; this window only shows it.
          </p>
          <Panel title="Overdue">
            {due.data.overdue.length === 0 ? <NoData reason="nothing overdue" /> : <Table caption="Overdue reviews" headers={withActions} rows={rows(due.data.overdue, true)} />}
          </Panel>
          <Panel title="Due today">
            {due.data.due.length === 0 ? <NoData reason="nothing due today" /> : <Table caption="Due today" headers={withActions} rows={rows(due.data.due, true)} />}
          </Panel>
          <Panel title="Beyond budget (overflow)">
            {due.data.overflow.length === 0 ? <NoData reason="queue fits the budget" /> : <Table caption="Overflow" headers={headers} rows={rows(due.data.overflow, false)} />}
          </Panel>
        </>
      ) : null}
      <Panel title="Add review item">
        <form
          aria-label="Add review item"
          onSubmit={(e) => {
            e.preventDefault();
            if (prompt.trim() !== '') {
              void addItem();
            }
          }}
        >
          <div className="field">
            <label htmlFor="review-prompt">Prompt</label>
            <textarea
              id="review-prompt"
              required
              maxLength={2000}
              rows={2}
              value={prompt}
              onChange={(e) => {
                setPrompt(e.target.value);
              }}
            />
          </div>
          <div className="field">
            <label htmlFor="review-answer">Answer (optional)</label>
            <textarea
              id="review-answer"
              maxLength={20000}
              rows={3}
              aria-describedby="review-answer-hint"
              value={answer}
              onChange={(e) => {
                setAnswer(e.target.value);
              }}
            />
            <span id="review-answer-hint" className="field-hint">
              Kept as a note you can erase, never on the item itself. The first review is due one day after you add the item.
            </span>
          </div>
          <button type="submit" className="primary" disabled={busy || prompt.trim() === ''}>
            Add review item
          </button>
        </form>
        {added !== null ? (
          <p role="status" data-testid="review-item-added">
            Added “{added.prompt}”: first review {formatDateTime(added.due_at, today.zone)} <ProvenanceBadge label="VERIFIED" />
          </p>
        ) : null}
      </Panel>
    </div>
  );
}

/** What the backend did with the last recorded review (its next due date, stage and offset). */
function RecordedReview({ prompt, record, zone }: { prompt: string; record: ReviewRecord; zone: string }) {
  const next = formatDateTime(record.due_at, zone);
  const text =
    record.outcome === 'SUCCESS'
      ? `Recalled “${prompt}”: next review ${next} (stage ${record.stage}, ${record.offset_days} day${record.offset_days === 1 ? '' : 's'} later).`
      : record.outcome === 'FAILURE'
        ? `Forgot “${prompt}”: back to the first stage; next review ${next}.`
        : `Skipped “${prompt}”: recorded; the due date stays ${next}.`;
  return (
    <p role="status" data-testid="review-recorded">
      {text}
    </p>
  );
}
