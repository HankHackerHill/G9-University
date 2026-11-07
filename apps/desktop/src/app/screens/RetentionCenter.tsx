import type { ReviewItem } from '../../ipc/operations';
import { formatDateTime, formatMinutes } from '../../lib/format';
import { ProvenanceBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

export function RetentionCenter() {
  const today = useToday();
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });

  const rows = (items: ReviewItem[]) =>
    items.map((r) => ({
      key: r.review_item_id,
      cells: [r.prompt, r.kind, `+${r.offset_days} d`, formatDateTime(r.due_at, today.zone), r.objective_id ?? '—', formatMinutes(Math.round(r.estimated_ms / 60_000))],
    }));
  const headers = ['Prompt', 'Kind', 'Offset', 'Due', 'Objective', 'Est.'];

  return (
    <div>
      <h1>Retention Center</h1>
      <p className="muted">
        Baseline schedule: offsets 1, 3, 7, 21, 60 days after the last successful recall; a failed recall resets to day 1. The daily review budget caps the queue; overflow
        is reported, never dropped (ARCHITECTURE §9). FSRS is an optional, item-level feature that is OFF.
      </p>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {due.loading ? <Loading what="review queue" /> : null}
      <Warnings warnings={due.warnings} />
      {due.data !== null ? (
        <>
          <p>
            As of {formatDateTime(due.data.as_of, today.zone)} · budget {formatMinutes(Math.round(due.data.budget_ms / 60_000))} <ProvenanceBadge label="VERIFIED" />
          </p>
          <Panel title="Overdue">{due.data.overdue.length === 0 ? <NoData reason="nothing overdue" /> : <Table caption="Overdue reviews" headers={headers} rows={rows(due.data.overdue)} />}</Panel>
          <Panel title="Due today">{due.data.due.length === 0 ? <NoData reason="nothing due today" /> : <Table caption="Due today" headers={headers} rows={rows(due.data.due)} />}</Panel>
          <Panel title="Beyond budget (overflow)">
            {due.data.overflow.length === 0 ? <NoData reason="queue fits the budget" /> : <Table caption="Overflow" headers={headers} rows={rows(due.data.overflow)} />}
          </Panel>
        </>
      ) : null}
    </div>
  );
}
