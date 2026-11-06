import type { ReactNode } from 'react';
import { formatDuration } from '../../lib/format';
import { ProvenanceBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

interface Metric {
  key: string;
  name: string;
  value: ReactNode;
  evidence: string;
  definition: string;
}

function noData(reason: string): ReactNode {
  return <span className="muted">NO DATA — {reason}</span>;
}

/** Tabular metrics with evidence references. Every value is computed from backend records returned on this screen; nothing is estimated. */
export function PerformanceDashboard() {
  const today = useToday();
  const attempts = useCommand('assessment.list', {});
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const competency = useCommand('competency.projection', {});

  const loading = attempts.loading || sessions.loading || due.loading || competency.loading;
  const finalized = (attempts.data?.attempts ?? []).filter((a) => a.state === 'APPROVED_FINAL' && a.grade !== null);
  const firstAttempts = finalized.filter((a) => a.retake_of === null);
  const todaySessions = sessions.data?.sessions ?? [];
  const credited = todaySessions.reduce((acc, s) => acc + s.credited_ms, 0);
  const uncertain = todaySessions.reduce((acc, s) => acc + s.uncertain_ms, 0);
  const projections = competency.data?.projections ?? [];
  const validated = projections.filter((p) => p.state === 'VALIDATED').length;

  const metrics: Metric[] = [
    {
      key: 'first-pass',
      name: 'Independent exam pass rate (first attempt)',
      value:
        firstAttempts.length === 0
          ? noData('no finalized first attempts')
          : `${firstAttempts.filter((a) => a.grade?.pass === true).length} / ${firstAttempts.length}`,
      evidence: `assessment.list → ${firstAttempts.map((a) => a.attempt_id).join(', ') || '—'}`,
      definition: 'Finalized attempts with retake_of = null and grade.pass; eventual pass rate is reported separately (Addendum A.3).',
    },
    {
      key: 'eventual-pass',
      name: 'Eventual pass rate',
      value: finalized.length === 0 ? noData('no finalized attempts') : `${finalized.filter((a) => a.grade?.pass === true).length} / ${finalized.length}`,
      evidence: `assessment.list → ${finalized.map((a) => a.attempt_id).join(', ') || '—'}`,
      definition: 'All finalized attempts including retakes.',
    },
    {
      key: 'credited-today',
      name: 'Verified study time today',
      value: sessions.data === null ? noData('sessions not loaded') : formatDuration(credited),
      evidence: `session.list(${today.isoDate ?? '?'}) → ${todaySessions.length} sessions`,
      definition: 'Sum of credited_ms (VERIFIED intervals plus accepted corrections) over today’s sessions.',
    },
    {
      key: 'uncertain-today',
      name: 'Uncertain time awaiting correction',
      value: sessions.data === null ? noData('sessions not loaded') : formatDuration(uncertain),
      evidence: `session.list(${today.isoDate ?? '?'})`,
      definition: 'Sum of UNCERTAIN interval lengths; excluded from credit until corrected (A03).',
    },
    {
      key: 'review-backlog',
      name: 'Review backlog (overdue)',
      value: due.data === null ? noData('queue not loaded') : due.data.overdue.length,
      evidence: `retention.due → ${due.data?.overdue.map((r) => r.review_item_id).join(', ') ?? '—'}`,
      definition: 'Items due before today within the budget; overflow is listed in Retention Center.',
    },
    {
      key: 'validated',
      name: 'Competencies validated',
      value: competency.data === null ? noData('projection not loaded') : `${validated} / ${projections.length}`,
      evidence: `competency.projection → ${projections.length} competencies`,
      definition: 'State VALIDATED requires EXAM-mode unaided Application/Transfer evidence.',
    },
    {
      key: 'recall-7-21',
      name: '7/21-day recall (seeded probes)',
      value: noData('probe operation is PROPOSED (LRN-19)'),
      evidence: '—',
      definition: 'Measured on seeded random probes regardless of due date (docs/evaluations/kpi-definitions.md).',
    },
    {
      key: 'calibration',
      name: 'Confidence calibration (Brier)',
      value: noData('no confidence reports recorded'),
      evidence: '—',
      definition: 'Predicted vs observed performance; needs confidence inputs on attempts.',
    },
  ];

  return (
    <div>
      <h1>Performance Dashboard</h1>
      <p className="muted">
        Metrics are reproducible read models with drilldown to evidence (M13). A cell reads NO DATA when the underlying records do not exist; no value is estimated or
        extrapolated. <ProvenanceBadge label="VERIFIED" />
      </p>
      {attempts.error !== null ? <ErrorDisplay error={attempts.error} onRetry={attempts.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {competency.error !== null ? <ErrorDisplay error={competency.error} onRetry={competency.reload} /> : null}
      {loading ? <Loading what="metrics" /> : null}
      <Panel title="Learning KPIs">
        <Table
          caption="Learning KPIs with evidence references"
          headers={['Metric', 'Value', 'Evidence', 'Definition']}
          rows={metrics.map((m) => ({ key: m.key, cells: [m.name, m.value, <span className="mono">{m.evidence}</span>, <span className="muted">{m.definition}</span>] }))}
        />
      </Panel>
      <Panel title="Charts">
        <NoData reason="charts are optional and would duplicate the table above; the table is the accessible source of truth" />
      </Panel>
    </div>
  );
}
