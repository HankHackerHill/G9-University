import type { ReactNode } from 'react';
import type { CompetencyState } from '../../ipc/types';
import { stateLabel } from '../../lib/copy';
import { formatDuration, formatMinutes } from '../../lib/format';
import { dayNumber, programMap } from '../../lib/program';
import { mergeWarnings } from '../../lib/warnings';
import { ProvenanceBadge } from '../components/Badge';
import { Bars, ChartFigure, Legend, Segmented, onlyPart, type Mark, type Part } from '../components/charts';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Stat } from '../components/Stat';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

interface Measure {
  key: string;
  /** The operator's name for the measure. */
  name: string;
  /** The key in docs/evaluations/kpi-definitions.md, when the measure has one. */
  kpi?: string;
  value: ReactNode;
  /** The operation the figure is read from, and the records behind it. */
  source: string | null;
  records: readonly string[];
  definition: string;
}

function noData(reason: string): ReactNode {
  return <span className="muted">No data: {reason}</span>;
}

/** docs/evaluations/kpi-definitions.md: below the minimum n the window says so with n and the minimum, never a value. */
function insufficient(n: number, min: number): ReactNode {
  return (
    <span className="muted">
      Not enough data: n = {n}, minimum {min}
    </span>
  );
}

const DAY_MS = 86_400_000;
/** `first_attempt_pass_rate`: rolling 4 weeks, min n 4. `eventual_pass_rate`: program to date, min n 4. */
const FIRST_ATTEMPT_WINDOW_MS = 28 * DAY_MS;
const PASS_RATE_MIN_N = 4;
/** How many recorded days the study-time chart shows (one program week). */
const DAYS_SHOWN = 6;

/** Competency states in the order a competency moves through them, with the mark each takes. */
const STATE_MARKS: readonly { state: CompetencyState; mark: Mark }[] = [
  { state: 'UNASSESSED', mark: 'quiet' },
  { state: 'INTRODUCED', mark: 'ramp-1' },
  { state: 'GUIDED', mark: 'ramp-2' },
  { state: 'INDEPENDENT', mark: 'ramp-3' },
  { state: 'VALIDATED', mark: 'ramp-4' },
  { state: 'RETEST_DUE', mark: 'owed' },
];

/** The steps of the review ladder take the ordered marks, shortest interval first. */
const LADDER_MARKS: readonly Mark[] = ['quiet', 'ramp-1', 'ramp-2', 'ramp-3', 'ramp-4'];

/**
 * Learning measures. Every figure is computed from backend records returned on this screen; a
 * measure whose records do not exist, or that has fewer than its minimum sample, says so in
 * words and shows no number. Charts repeat what the tables hold and never add a figure.
 */
export function PerformanceDashboard() {
  const today = useToday();
  const attempts = useCommand('assessment.list', {});
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const allSessions = useCommand('session.list', {});
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const competency = useCommand('competency.projection', {});
  // Only to know which program day today is, so the chart never draws a day that has not come yet.
  const plan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });

  const loading = attempts.loading || sessions.loading || due.loading || competency.loading;
  const warnings = mergeWarnings(attempts.warnings, sessions.warnings, allSessions.warnings, due.warnings, competency.warnings, plan.warnings);
  // "Verified" is claimed for the measures only when every read behind them succeeded without a
  // warning. The plan and the all-sessions reads feed the day chart, not a measure: their
  // warnings are shown above like any other, and do not bear on this stamp.
  const measureWarnings = mergeWarnings(attempts.warnings, sessions.warnings, due.warnings, competency.warnings);
  const verified = [attempts, sessions, due, competency].every((q) => q.data !== null && q.error === null) && measureWarnings.length === 0;
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
  const firstSelfGraded = firstAttempts.filter((a) => a.grade?.reliability_class === 'SELF_ADJUDICATED').length;
  const firstKnown = attempts.data !== null && now !== null;
  const firstEnough = firstKnown && firstAttempts.length >= PASS_RATE_MIN_N;
  // eventual_pass_rate: definitions with >= 1 passing (finalized) attempt ÷ definitions attempted.
  const attemptedDefinitions = [...new Set(allAttempts.map((a) => a.assessment_id))];
  const passedDefinitions = new Set(finalized.filter((a) => a.grade?.pass === true).map((a) => a.assessment_id));
  const eventualPassed = attemptedDefinitions.filter((d) => passedDefinitions.has(d)).length;
  const eventualEnough = attempts.data !== null && attemptedDefinitions.length >= PASS_RATE_MIN_N;
  const todaySessions = sessions.data?.sessions ?? [];
  const credited = todaySessions.reduce((acc, s) => acc + s.credited_ms, 0);
  const uncertain = todaySessions.reduce((acc, s) => acc + s.uncertain_ms, 0);
  // review_backlog: every due item not completed — in budget (overdue + due today) and beyond budget (overflow).
  const backlog = due.data === null ? null : [...due.data.overdue, ...due.data.due, ...due.data.overflow];
  const projections = competency.data?.projections ?? [];
  // A VALIDATED projection flagged provisional rests on SELF_ADJUDICATED evidence (Addendum A.2): counted separately.
  const validated = projections.filter((p) => p.state === 'VALIDATED' && !p.provisional).length;
  const validatedProvisional = projections.filter((p) => p.state === 'VALIDATED' && p.provisional).length;

  // Charts. Each is drawn only from records already listed above.
  const todayNumber = plan.data?.day_id === null || plan.data?.day_id === undefined ? null : dayNumber(plan.data.day_id);
  const recordedDays =
    allSessions.data === null
      ? null
      : programMap(allSessions.data.sessions, null).filter((d) => d.planned_ms !== null && (todayNumber === null || (dayNumber(d.id) ?? 0) <= todayNumber));
  const shownDays = recordedDays === null ? null : recordedDays.slice(-DAYS_SHOWN);
  const stateParts: Part[] = STATE_MARKS.map(({ state, mark }) => ({
    key: state,
    name: stateLabel(state),
    n: projections.filter((p) => p.state === state).length,
    mark,
  }));
  const ladderSteps = backlog === null ? [] : [...new Set(backlog.map((r) => r.offset_days))].sort((a, b) => a - b);
  const ladderParts: Part[] = ladderSteps.map((days, i) => ({
    key: `${days}`,
    name: `${days}-day step`,
    n: backlog === null ? 0 : backlog.filter((r) => r.offset_days === days).length,
    mark: LADDER_MARKS[Math.min(i, LADDER_MARKS.length - 1)] ?? 'ramp-4',
  }));
  const oneState = onlyPart(stateParts);
  const oneStep = onlyPart(ladderParts);

  const measures: Measure[] = [
    {
      key: 'first-pass',
      name: 'First-attempt pass rate',
      kpi: 'first_attempt_pass_rate',
      value:
        attempts.data === null
          ? noData('attempts not loaded')
          : now === null
            ? noData('backend clock not loaded')
            : !firstEnough
              ? insufficient(firstAttempts.length, PASS_RATE_MIN_N)
              : `${firstPassed} of ${firstAttempts.length}${firstSelfGraded > 0 ? ` (${firstSelfGraded} self-graded)` : ''}`,
      source: 'assessment.list',
      records: firstAttempts.map((a) => `${a.attempt_id}${a.grade?.reliability_class === 'SELF_ADJUDICATED' ? ' [self-graded]' : ''}`),
      definition:
        'Final first attempts (not retakes) in exam mode that passed, out of those first attempts. Finalized in the last 4 weeks; needs at least 4. Self-graded decisions are flagged (kpi-definitions.md, Addendum A.3).',
    },
    {
      key: 'eventual-pass',
      name: 'Eventual pass rate',
      kpi: 'eventual_pass_rate',
      value:
        attempts.data === null
          ? noData('attempts not loaded')
          : !eventualEnough
            ? insufficient(attemptedDefinitions.length, PASS_RATE_MIN_N)
            : `${eventualPassed} of ${attemptedDefinitions.length}`,
      source: 'assessment.list',
      records: attemptedDefinitions,
      definition:
        'Assessments with at least one passing final attempt, out of assessments attempted at all. Whole program to date; needs at least 4. Retakes raise it by construction.',
    },
    {
      key: 'credited-today',
      name: 'Credited study time today',
      value: sessions.data === null ? noData('sessions not loaded') : formatDuration(credited),
      source: `session.list (${today.isoDate ?? '?'})`,
      records: todaySessions.map((s) => s.session_id),
      definition: 'Time the backend has vouched for in today’s sessions: verified intervals plus corrections you made.',
    },
    {
      key: 'uncertain-today',
      name: 'Time in doubt today',
      value: sessions.data === null ? noData('sessions not loaded') : formatDuration(uncertain),
      source: `session.list (${today.isoDate ?? '?'})`,
      records: todaySessions.filter((s) => s.uncertain_ms > 0).map((s) => s.session_id),
      definition: 'Uncertain intervals of today’s sessions. Not credited until you correct them (A03).',
    },
    {
      key: 'review-backlog',
      name: 'Review backlog',
      kpi: 'review_backlog',
      value:
        due.data === null || backlog === null
          ? noData('queue not loaded')
          : `${backlog.length} (${due.data.overdue.length} overdue, ${due.data.due.length} due today, ${due.data.overflow.length} beyond budget)`,
      source: 'retention.due',
      records: backlog === null ? [] : backlog.map((r) => r.review_item_id),
      definition:
        'Reviews that are due and not done. The headline counts items beyond today’s budget too, because they are still due; the breakdown lists them separately, as the definition requires.',
    },
    {
      key: 'validated',
      name: 'Competencies validated',
      value:
        competency.data === null
          ? noData('projection not loaded')
          : `${validated} of ${projections.length}${validatedProvisional > 0 ? ` (+${validatedProvisional} provisional, not counted)` : ''}`,
      source: 'competency.projection',
      records: projections.filter((p) => p.state === 'VALIDATED').map((p) => `${p.competency_id}${p.provisional ? ' [provisional]' : ''}`),
      definition:
        'Validated needs unaided exam-mode evidence of application or transfer. Evidence from a self-graded decision reaches Independent (provisional) at most, so provisional ones are not counted (Addendum A.2).',
    },
    {
      key: 'recall-7-21',
      name: 'Recall after 7 and 21 days',
      value: noData('the probe operation is proposed, not built (LRN-19)'),
      source: null,
      records: [],
      definition: 'Measured on seeded random probes, whatever their due date (kpi-definitions.md).',
    },
    {
      key: 'calibration',
      name: 'Confidence calibration',
      kpi: 'calibration_brier',
      value: noData('no confidence reports recorded'),
      source: null,
      records: [],
      definition:
        'Mean Brier score over the probabilities you state, with reliability bins. Rolling 4 weeks; min n 20; discrete questions only. Attempts do not record confidence yet.',
    },
  ];

  return (
    <div className="stack">
      <PageHead title="Performance Dashboard" lede="Learning measures, with their definitions and the records behind each figure." />
      {attempts.error !== null ? <ErrorDisplay error={attempts.error} onRetry={attempts.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {due.error !== null ? <ErrorDisplay error={due.error} onRetry={due.reload} /> : null}
      {competency.error !== null ? <ErrorDisplay error={competency.error} onRetry={competency.reload} /> : null}
      {loading ? <Loading what="measures" /> : null}
      <Warnings warnings={warnings} />

      <section className="bento" aria-label="Headline measures">
        <div data-span="3">
          {!firstKnown ? (
            <Stat label="First-attempt pass rate" value="No data" standing="missing" foot={attempts.data === null ? 'Attempts are not loaded.' : 'The backend clock is not loaded.'} />
          ) : !firstEnough ? (
            <Stat label="First-attempt pass rate" value="Not enough data" standing="missing" foot={`n = ${firstAttempts.length}, minimum ${PASS_RATE_MIN_N}. Exam mode, last 4 weeks.`} />
          ) : (
            <Stat
              label="First-attempt pass rate"
              value={firstPassed}
              unit={`of ${firstAttempts.length}`}
              standing={firstSelfGraded > 0 ? 'doubt' : 'posted'}
              foot={`Exam mode, last 4 weeks.${firstSelfGraded > 0 ? ` ${firstSelfGraded} self-graded.` : ''}`}
            />
          )}
        </div>
        <div data-span="3">
          {attempts.data === null ? (
            <Stat label="Eventual pass rate" value="No data" standing="missing" foot="Attempts are not loaded." />
          ) : !eventualEnough ? (
            <Stat label="Eventual pass rate" value="Not enough data" standing="missing" foot={`n = ${attemptedDefinitions.length}, minimum ${PASS_RATE_MIN_N}. Whole program.`} />
          ) : (
            <Stat label="Eventual pass rate" value={eventualPassed} unit={`of ${attemptedDefinitions.length}`} foot="Assessments passed at least once, of those attempted." />
          )}
        </div>
        <div data-span="3">
          {due.data === null || backlog === null ? (
            <Stat label="Review backlog" value="No data" standing="missing" foot="The review queue is not loaded." />
          ) : (
            <Stat
              label="Review backlog"
              value={backlog.length}
              standing={due.data.overdue.length > 0 ? 'owed' : 'posted'}
              foot={`${due.data.overdue.length} overdue, ${due.data.due.length} due today, ${due.data.overflow.length} beyond budget`}
            />
          )}
        </div>
        <div data-span="3">
          {competency.data === null ? (
            <Stat label="Competencies validated" value="No data" standing="missing" foot="The competency projection is not loaded." />
          ) : (
            <Stat
              label="Competencies validated"
              value={validated}
              unit={`of ${projections.length}`}
              foot={validatedProvisional > 0 ? `+${validatedProvisional} provisional, not counted` : 'Unaided exam-mode evidence only.'}
            />
          )}
        </div>
      </section>

      <div className="grid-2">
        <Panel title="Study time by day">
          {allSessions.error !== null ? (
            <ErrorDisplay error={allSessions.error} onRetry={allSessions.reload} />
          ) : shownDays === null ? (
            <Loading what="recorded sessions" />
          ) : shownDays.length === 0 ? (
            <NoData reason="No session is recorded yet. Days appear here once a session exists for them." />
          ) : (
            <ChartFigure
              caption={`${shownDays.length === 1 ? 'The one day' : `The last ${shownDays.length} days`} with sessions${todayNumber === null ? '' : ', up to today'}. Each bar is as long as that day’s plan; the solid part is credited.`}
              table={
                <Table
                  caption="Study time by recorded day"
                  headers={['Day', 'Planned', 'Credited', 'In doubt']}
                  numeric={[1, 2, 3]}
                  rows={shownDays.map((d) => ({ key: d.id, cells: [d.id, formatDuration(d.planned_ms ?? 0), formatDuration(d.credited_ms), formatDuration(d.uncertain_ms)] }))}
                />
              }
            >
              <Bars
                label="Credited and in-doubt time against the plan, by recorded day"
                rows={shownDays.map((d) => ({
                  key: d.id,
                  name: `Day ${dayNumber(d.id) ?? d.id}`,
                  of: d.planned_ms ?? 0,
                  posted: d.credited_ms,
                  doubted: d.uncertain_ms,
                  value: `${formatDuration(d.credited_ms)} of ${formatMinutes(Math.round((d.planned_ms ?? 0) / 60_000))}`,
                }))}
              />
              <Legend
                items={[
                  { mark: 'posted', label: 'Credited' },
                  { mark: 'doubt', label: 'In doubt' },
                  { mark: 'pencil', label: 'Planned' },
                ]}
              />
            </ChartFigure>
          )}
        </Panel>

        <div className="stack">
          <Panel title="Competencies by state">
            {competency.data === null ? (
              competency.error === null ? (
                <Loading what="the competency projection" />
              ) : (
                <NoData reason={`The competency projection could not be read (${competency.error.code}).`} />
              )
            ) : projections.length === 0 ? (
              <NoData reason="No competency exists yet. Create one in the Competency Graph." />
            ) : oneState !== null ? (
              <p>
                All {oneState.n} competencies are <strong>{oneState.name.toLowerCase()}</strong>. A competency changes state when a final grade gives evidence for it.
              </p>
            ) : (
              <ChartFigure
                caption={`${projections.length} competencies, in the order a competency moves through the states.`}
                table={
                  <Table
                    caption="Competencies by state"
                    headers={['State', 'Competencies']}
                    numeric={[1]}
                    rows={stateParts.map((p) => ({ key: p.key, cells: [p.name, p.n] }))}
                  />
                }
              >
                <Segmented parts={stateParts} label="Competencies by state" />
                <Legend items={stateParts.map((p) => ({ mark: p.mark, label: `${p.name} ${p.n}` }))} />
              </ChartFigure>
            )}
          </Panel>

          <Panel title="Reviews due, by ladder step">
            {backlog === null ? (
              due.error === null ? (
                <Loading what="the review queue" />
              ) : (
                <NoData reason={`The review queue could not be read (${due.error.code}).`} />
              )
            ) : backlog.length === 0 ? (
              <NoData title="Nothing due" reason="No review is due, so there is nothing to draw." />
            ) : oneStep !== null ? (
              <p>
                {backlog.length === 1 ? 'The one review due is' : `All ${backlog.length} reviews due are`} on the <strong>{oneStep.name}</strong>. A review moves up a step each time you recall it
                and back to the first step when you do not.
              </p>
            ) : (
              <ChartFigure
                caption="A review moves up a step each time you recall it and back to the first step when you do not. Longer steps hold older knowledge."
                table={
                  <Table
                    caption="Reviews due by ladder step"
                    headers={['Step', 'Reviews due']}
                    numeric={[1]}
                    rows={ladderParts.map((p) => ({ key: p.key, cells: [p.name, p.n] }))}
                  />
                }
              >
                <Segmented parts={ladderParts} label="Reviews due by ladder step" />
                <Legend items={ladderParts.map((p) => ({ mark: p.mark, label: `${p.name} ${p.n}` }))} />
              </ChartFigure>
            )}
          </Panel>
        </div>
      </div>

      <Panel
        title="Measures"
        note="Every measure with its figure, the records it was computed from, and its definition."
        actions={
          verified ? (
            <ProvenanceBadge label="VERIFIED" />
          ) : (
            <span className="badge badge-proposed" data-testid="dashboard-provenance-partial" title="A read behind these measures failed, is still pending, or came with a backend warning.">
              Partial
            </span>
          )
        }
      >
        <Table
          caption="Learning measures with the records behind them"
          captionVisible={false}
          widths={['15rem', '16rem', '13rem', undefined]}
          headers={['Measure', 'Value', 'Records', 'Definition']}
          rows={measures.map((m) => ({
            key: m.key,
            cells: [
              <span className="row-main">
                <span className="row-title">{m.name}</span>
                {m.kpi === undefined ? null : <code className="row-sub">{m.kpi}</code>}
              </span>,
              m.value,
              m.source === null ? (
                <span className="muted">None</span>
              ) : m.records.length === 0 ? (
                <span className="muted">
                  <code>{m.source}</code>: none
                </span>
              ) : (
                <details>
                  <summary>
                    <code>{m.source}</code>: {m.records.length}
                  </summary>
                  <span className="hash">{m.records.join(', ')}</span>
                </details>
              ),
              <span className="muted">{m.definition}</span>,
            ],
          }))}
        />
      </Panel>
    </div>
  );
}
