import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { ReviewItem, ReviewOutcome, ReviewRecord } from '../../ipc/operations';
import type { G9Error, IanaZone } from '../../ipc/types';
import { formatDateTime, formatMinutes, sentenceCase } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge } from '../components/Badge';
import { Composer } from '../components/Composer';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { Margin, MarginNote } from '../components/Margin';
import { PageHead } from '../components/PageHead';
import { Loading, NoData, Panel } from '../components/Panel';
import { Stat } from '../components/Stat';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

const OUTCOMES: readonly { outcome: ReviewOutcome; label: string }[] = [
  { outcome: 'SUCCESS', label: 'Recalled' },
  { outcome: 'FAILURE', label: 'Did not recall' },
  { outcome: 'SKIPPED', label: 'Skip' },
];

/** The baseline ladder (ARCHITECTURE §9): the wait before the next review, in days, shortest first. */
const LADDER_DAYS: readonly number[] = [1, 3, 7, 21, 60];

/** The kind of a review item in the operator's words; the backend's name stays in the `title`. */
const KIND_WORDS: Readonly<Record<string, string>> = {
  DECLARATIVE: 'Knowledge',
  PRACTICAL_TRANSFER: 'Practical task',
};

function days(n: number): string {
  return n === 1 ? '1 day' : `${n} days`;
}

type Group = 'overdue' | 'due' | 'overflow';

export function RetentionCenter() {
  const today = useToday();
  const client = useIpc();
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const [composing, setComposing] = useState(false);
  const [prompt, setPrompt] = useState('');
  const [answer, setAnswer] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<G9Error | null>(null);
  const [actionWarnings, setActionWarnings] = useState<readonly string[]>([]);
  const [added, setAdded] = useState<ReviewItem | null>(null);
  const [recorded, setRecorded] = useState<{ prompt: string; record: ReviewRecord } | null>(null);
  // While a review is recorded or the queue re-reads, no second click can record the same item twice.
  const locked = busy || due.loading;
  const queue = due.data;
  // Why there is no figure, when there is none: said on every tile instead of a zero.
  const missing =
    due.error !== null
      ? `The review queue could not be read (${due.error.code}).`
      : today.error !== null
        ? `The backend clock could not be read (${today.error.code}), so the queue cannot be asked for.`
        : 'The review queue is still being read.';

  const addItem = async () => {
    setBusy(true);
    const trimmedAnswer = answer.trim();
    const r = await client.invoke('retention.create_item', { prompt: prompt.trim(), ...(trimmedAnswer === '' ? {} : { answer: trimmedAnswer }) });
    setActionWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
      setAdded(null);
    } else {
      setError(null);
      setAdded(r.result);
      setPrompt('');
      setAnswer('');
      setComposing(false);
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

  const line = (r: ReviewItem, group: Group) => (
    <li key={r.review_item_id}>
      <div className="row-main">
        <span className="row-title">{r.prompt}</span>
        {group === 'overdue' ? (
          <span className="row-sub" data-standing="owed">
            Overdue since {formatDateTime(r.due_at, today.zone)}
          </span>
        ) : (
          <span className="row-sub">Due {formatDateTime(r.due_at, today.zone)}</span>
        )}
        <span className="row-sub">
          <span title={r.kind}>{KIND_WORDS[r.kind] ?? sentenceCase(r.kind)}</span>, {r.offset_days}-day step, about {formatMinutes(Math.round(r.estimated_ms / 60_000))}
          {r.objective_id === null ? null : (
            <>
              , objective <code>{r.objective_id}</code>
            </>
          )}
        </span>
      </div>
      {/* What is beyond the budget is listed, never dropped; it is recorded once it has moved into today's queue. */}
      {group === 'overflow' ? null : (
        <div className="row-end" role="group" aria-label={`Record review: ${r.prompt}`}>
          {OUTCOMES.map((o) => (
            <button
              key={o.outcome}
              type="button"
              className="sm"
              disabled={locked}
              onClick={() => {
                void record(r, o.outcome);
              }}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </li>
  );

  return (
    <div className="stack">
      <PageHead
        title="Retention Center"
        lede="Recall what is due and record how it went. The backend sets the date of every next review."
        actions={
          <button
            type="button"
            className="primary"
            disabled={composing}
            onClick={() => {
              setComposing(true);
              setAdded(null);
            }}
          >
            Add a review item
          </button>
        }
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(due.warnings, actionWarnings)} />
      {added !== null && !composing ? (
        <p className="receipt" role="status" data-testid="review-item-added">
          Review item added: “{added.prompt}”. Its first review is due {formatDateTime(added.due_at, today.zone)}. <ProvenanceBadge label="VERIFIED" />
        </p>
      ) : null}

      {composing ? (
        <Composer
          title="Add a review item"
          onClose={() => {
            setComposing(false);
          }}
        >
          <form
            aria-label="Add a review item"
            onSubmit={(e) => {
              e.preventDefault();
              if (!busy && prompt.trim() !== '') {
                void addItem();
              }
            }}
          >
            <div className="form-grid">
              <Field wide label="Prompt" hint="What you will ask yourself. At most 2000 characters.">
                {({ id, describedBy }) => (
                  <textarea
                    id={id}
                    required
                    maxLength={2000}
                    rows={2}
                    value={prompt}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setPrompt(e.target.value);
                    }}
                  />
                )}
              </Field>
              <Field wide label="Answer (optional)" hint="Kept as a note you can erase, never on the item itself.">
                {({ id, describedBy }) => (
                  <textarea
                    id={id}
                    maxLength={20000}
                    rows={3}
                    value={answer}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setAnswer(e.target.value);
                    }}
                  />
                )}
              </Field>
            </div>
            <div className="form-actions">
              <button type="submit" className="primary" disabled={busy || prompt.trim() === ''}>
                Add review item
              </button>
              <button
                type="button"
                onClick={() => {
                  setComposing(false);
                }}
              >
                Cancel
              </button>
              <span className="form-note">The first review is due one day after you add the item.</span>
            </div>
          </form>
        </Composer>
      ) : null}

      <section className="bento" aria-label="The queue in numbers">
        <div data-span="3">
          {queue === null ? (
            <Stat label="Overdue" value="No data" standing="missing" foot={missing} />
          ) : (
            <Stat
              label="Overdue"
              value={queue.overdue.length}
              standing={queue.overdue.length > 0 ? 'owed' : 'posted'}
              foot={queue.overdue.length > 0 ? 'Due before today and still open.' : 'Nothing is left from earlier days.'}
            />
          )}
        </div>
        <div data-span="3">
          {queue === null ? <Stat label="Due today" value="No data" standing="missing" foot={missing} /> : <Stat label="Due today" value={queue.due.length} foot="Due today, within the budget." />}
        </div>
        <div data-span="3">
          {queue === null ? (
            <Stat label="Beyond today’s budget" value="No data" standing="missing" foot={missing} />
          ) : (
            <Stat label="Beyond today’s budget" value={queue.overflow.length} foot="Due as well. They wait until there is room." />
          )}
        </div>
        <div data-span="3">
          {queue === null ? (
            <Stat label="Today’s budget" value="No data" standing="missing" foot={missing} />
          ) : (
            <Stat label="Today’s budget" value={Math.round(queue.budget_ms / 60_000)} unit="min" foot="What does not fit is listed, never dropped." />
          )}
        </div>
      </section>

      <div className="with-margin">
        {/* The queue is a backend record: stamped Verified unless the read came with a warning. */}
        <Panel
          title="Review queue"
          actions={queue !== null && due.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : undefined}
          note={queue === null ? undefined : `The queue as of ${formatDateTime(queue.as_of, today.zone)}, most overdue first.`}
        >
          {recorded !== null ? <RecordedReview prompt={recorded.prompt} record={recorded.record} zone={today.zone} /> : null}
          {queue === null ? (
            due.error !== null || today.error !== null ? (
              <NoData reason={missing} />
            ) : (
              <Loading what="the review queue" />
            )
          ) : (
            <>
              <section className="stack-sm" aria-label="Overdue reviews">
                <h3>Overdue</h3>
                {queue.overdue.length === 0 ? <p className="muted">Nothing is overdue.</p> : <ul className="rows">{queue.overdue.map((r) => line(r, 'overdue'))}</ul>}
              </section>
              <section className="stack-sm" aria-label="Reviews due today">
                <h3>Due today</h3>
                {queue.due.length === 0 ? <p className="muted">Nothing is due today.</p> : <ul className="rows">{queue.due.map((r) => line(r, 'due'))}</ul>}
              </section>
              <section className="stack-sm" aria-label="Reviews beyond today’s budget">
                <h3>Beyond today’s budget</h3>
                {queue.overflow.length === 0 ? (
                  <p className="muted">The queue fits today’s budget.</p>
                ) : (
                  <>
                    <p className="muted small">Due as well, but today’s budget is used up. A line moves into the queue above as you record the ones before it.</p>
                    <ul className="rows">{queue.overflow.map((r) => line(r, 'overflow'))}</ul>
                  </>
                )}
              </section>
            </>
          )}
        </Panel>

        <Margin label="How the ladder works">
          <Panel title="The ladder" as="aside" note="Five steps. Each is the wait before the next review.">
            <ol className="steps" data-layout="list" aria-label="The review ladder, shortest wait first">
              {LADDER_DAYS.map((d) => (
                <li key={d}>{days(d)}</li>
              ))}
            </ol>
          </Panel>
          <MarginNote title="Recalled">
            <p>The item moves one step up the ladder. The backend sets the date of the next review and the receipt shows it.</p>
          </MarginNote>
          <MarginNote title="Did not recall">
            <p>The item goes back to the first step. Its next review is one day later.</p>
          </MarginNote>
          <MarginNote title="Skip">
            <p>The attempt is recorded. The step and the due date stay as they are.</p>
          </MarginNote>
          <MarginNote title="The budget">
            <p>Today’s budget caps the queue. A per-item schedule (FSRS) is optional and switched off.</p>
          </MarginNote>
        </Margin>
      </div>
    </div>
  );
}

/** What the backend did with the last recorded review: its next due date and its place on the ladder. */
function RecordedReview({ prompt, record, zone }: { prompt: string; record: ReviewRecord; zone: IanaZone }) {
  const next = formatDateTime(record.due_at, zone);
  // The backend's own ladder index stays one step away, in the title.
  const stage = `Ladder stage ${record.stage} of the baseline schedule`;
  return (
    <p className="receipt" role="status" data-testid="review-recorded" title={stage}>
      {record.outcome === 'SUCCESS'
        ? `Recalled: “${prompt}”. Next review ${next}, in ${days(record.offset_days)}.`
        : record.outcome === 'FAILURE'
          ? `Did not recall: “${prompt}”. It is back on the first step. Next review ${next}, in ${days(record.offset_days)}.`
          : `Skipped: “${prompt}”. The attempt is recorded and the due date stays ${next}.`}
    </p>
  );
}
