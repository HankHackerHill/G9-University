import type { ReactNode } from 'react';
import { formatDuration } from '../../lib/format';
import { ProvenanceBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { mergeWarnings } from '../../lib/warnings';
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

/** docs/evaluations/kpi-definitions.md: below the minimum n the UI shows INSUFFICIENT DATA, never a value. */
function insufficient(n: number, min: number): ReactNode {
  return (
    <span className="muted">
      INSUFFICIENT DATA — n = {n}, minimum {min}
    </span>
  );
}

const DAY_MS = 86_400_000;
/** `first_attempt_pass_rate`: rolling 4 weeks, min n 4. `eventual_pass_rate`: program to date, min n 4. */
const FIRST_ATTEMPT_WINDOW_MS = 28 * DAY_MS;
const PASS_RATE_MIN_N = 4;

/** Tabular metrics with evidence references. Every value is computed from backend records returned on this screen; nothing is estimated. */
export function PerformanceDashboard() {
  const today = useToday();
  const attempts = useCommand('assessment.list', {});
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const competency = useCommand('competency.projection', {});

  const loading = attempts.loading || sessions.loading || due.loading || competency.loading;
  const warnings = mergeWarnings(attempts.warnings, sessions.warnings, due.warnings, competency.warnings);
  // VERIFIED is claimed only when every read behind the table succeeded without a warning.
  const verified = [attempts, sessions, due, competency].every((q) => q.data !== null && q.error === null) && warnings.length === 0;
  const allAttempts = attempts.data?.attempts ?? [];
  const finalized = allAttempts.filter((a) => a.state === 'APPROVED_FINAL' && a.grade !== null);
  // first_attempt_pass_rate: APPROVED_FINAL first attempts, EXAM mode, finalized in the rolling 4-week window (backend clock).
  const now = today.now;
  const firstAttempts =
    now === null
      ? []
      : finalized.filter(
          (a) => a.retake_of === null && a.mode === 'EXAM' && a.finalized_at !== null && a.finalized_at > now - FIRST_ATTEMPT_WINDOW_MS && a.finalized_at <= now,
        );
  const firstPassed = firstAttempts.filter((a) => a.grade?.pass === true).length;
  const firstSelfAdjudicated = firstAttempts.filter((a) => a.grade?.reliability_class === 'SELF_ADJUDICATED').length;
  // eventual_pass_rate: definitions with >= 1 passing (finalized) attempt ÷ definitions attempted.
  const attemptedDefinitions = [...new Set(allAttempts.map((a) => a.assessment_id))];
  const passedDefinitions = new Set(finalized.filter((a) => a.grade?.pass === true).map((a) => a.assessment_id));
  const todaySessions = sessions.data?.sessions ?? [];
  const credited = todaySessions.reduce((acc, s) => acc + s.credited_ms, 0);
  const uncertain = todaySessions.reduce((acc, s) => acc + s.uncertain_ms, 0);
  // review_backlog: every due item not completed — in budget (overdue + due today) and beyond budget (overflow).
  const backlog = due.data === null ? null : [...due.data.overdue, ...due.data.due, ...due.data.overflow];
  const projections = competency.data?.projections ?? [];
  // A VALIDATED projection flagged provisional rests on SELF_ADJUDICATED evidence (Addendum A.2): counted separately.
  const validated = projections.filter((p) => p.state === 'VALIDATED' && !p.provisional).length;
  const validatedProvisional = projections.filter((p) => p.state === 'VALIDATED' && p.provisional).length;

  const metrics: Metric[] = [
    {
      key: 'first-pass',
      name: 'First-attempt pass rate (first_attempt_pass_rate)',
      value:
        attempts.data === null
          ? noData('attempts not loaded')
          : now === null
            ? noData('backend clock not loaded')
            : firstAttempts.length < PASS_RATE_MIN_N
              ? insufficient(firstAttempts.length, PASS_RATE_MIN_N)
              : `${firstPassed} / ${firstAttempts.length}${firstSelfAdjudicated > 0 ? ` (${firstSelfAdjudicated} SELF_ADJUDICATED)` : ''}`,
      evidence: `assessment.list → ${firstAttempts.map((a) => `${a.attempt_id}${a.grade?.reliability_class === 'SELF_ADJUDICATED' ? ' [SELF_ADJUDICATED]' : ''}`).join(', ') || '—'}`,
      definition:
        'APPROVED_FINAL first attempts (retake_of = null) in EXAM mode with pass = true ÷ those first attempts; finalized in the rolling 4 weeks; min n 4; SELF_ADJUDICATED decisions flagged (kpi-definitions.md, Addendum A.3).',
    },
    {
      key: 'eventual-pass',
      name: 'Eventual pass rate (eventual_pass_rate)',
      value:
        attempts.data === null
          ? noData('attempts not loaded')
          : attemptedDefinitions.length < PASS_RATE_MIN_N
            ? insufficient(attemptedDefinitions.length, PASS_RATE_MIN_N)
            : `${attemptedDefinitions.filter((d) => passedDefinitions.has(d)).length} / ${attemptedDefinitions.length}`,
      evidence: `assessment.list → definitions ${attemptedDefinitions.join(', ') || '—'}`,
      definition:
        'Assessment definitions with at least one passing APPROVED_FINAL attempt ÷ definitions attempted (any attempt recorded); program to date; min n 4; inflated by retakes by construction.',
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
      name: 'Review backlog (review_backlog)',
      value:
        due.data === null || backlog === null
          ? noData('queue not loaded')
          : `${backlog.length} (${due.data.overdue.length} overdue · ${due.data.due.length} due today · ${due.data.overflow.length} beyond budget)`,
      evidence: `retention.due → ${backlog === null || backlog.length === 0 ? '—' : backlog.map((r) => r.review_item_id).join(', ')}`,
      definition:
        'Due review items not completed (kpi-definitions.md review_backlog, daily, no minimum n). UI decision: the headline counts overflow too, because an item beyond the budget is still due and not completed; the breakdown lists overflow separately as the definition requires.',
    },
    {
      key: 'validated',
      name: 'Competencies validated',
      value:
        competency.data === null
          ? noData('projection not loaded')
          : `${validated} / ${projections.length}${validatedProvisional > 0 ? ` (+${validatedProvisional} provisional, not counted)` : ''}`,
      evidence: `competency.projection → ${projections.length} competencies`,
      definition:
        'State VALIDATED requires EXAM-mode unaided Application/Transfer evidence; evidence from a SELF_ADJUDICATED decision yields at most Independent (provisional), so provisional projections are not counted (Addendum A.2).',
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
      definition: 'Mean Brier score over stated probabilities with reliability bins; rolling 4 weeks; min n 20; discrete questions only (kpi-definitions.md calibration_brier). Needs confidence inputs on attempts, which do not exist yet.',
    },
  ];

  return (
    <div>
      <h1>Performance Dashboard</h1>
      <p className="muted">
        Metrics are reproducible read models with drilldown to evidence (M13). A cell reads NO DATA when the underlying records do not exist; no value is estimated or
        extrapolated.{' '}
        {verified ? (
          <ProvenanceBadge label="VERIFIED" />
        ) : (
          <span className="muted" data-testid="dashboard-provenance-partial">
            (provenance: partial — a read failed, is pending or carries a backend warning; see above)
          </span>
        )}
      </p>
      {attempts.error !== null ? <ErrorDisplay error={attempts.error} onRetry={attempts.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {competency.error !== null ? <ErrorDisplay error={competency.error} onRetry={competency.reload} /> : null}
      {loading ? <Loading what="metrics" /> : null}
      <Warnings warnings={warnings} />
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
