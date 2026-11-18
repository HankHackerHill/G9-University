import '../theme/screens/curriculum.css';
import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { isG9Error } from '../../ipc/client';
import type { DailyPlan } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { addDays, formatIsoDate, isoWeekday, parseIsoDate, startOfIsoWeek } from '../../lib/civil-time';
import { blockName, formatCivilDate, formatMinutes } from '../../lib/format';
import { DAYS_PER_WEEK, PROGRAM_WEEKS, SETUP_CODES } from '../../lib/program';
import { NON_CANONICAL_WARNING, hasWarning, mergeWarnings, warningsOf } from '../../lib/warnings';
import { Badge, ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin, MarginNote } from '../components/Margin';
import { Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { usePlanDay } from '../hooks/usePlanDay';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

interface DayNode {
  date: string;
  plan: DailyPlan | null;
  error: G9Error | null;
  warnings: readonly string[];
}

const WEEKDAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

/**
 * The curriculum's invariants (ARCHITECTURE §4: 12 × 6 × 3; 420/300 minutes; 2,400 a week;
 * 28,800 in all). They describe the program as designed and are shown as such in the margin,
 * apart from the sums of what this screen read.
 */
const OBJECTIVES_PER_DAY = 3;
const WEEKDAY_MINUTES = 420;
const SATURDAY_MINUTES = 300;
const WEEK_MINUTES = 2_400;
const PROGRAM_MINUTES = 28_800;

function count(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/**
 * The current week of the program as a tree: one line per day (Monday to Saturday), built from
 * `curriculum.daily_plan` for each date. Nothing is created or changed here. The margin says
 * where the plan stands (a version that is not canonical yet gives Proposed plans, never Verified
 * ones) and what was read; a day that could not be read is said so, never counted as empty.
 */
export function CurriculumNavigator() {
  const today = useToday();
  const client = useIpc();
  const [nodes, setNodes] = useState<{ key: string; days: DayNode[] } | null>(null);
  // "Try again" on a failed day reads the week again; the days already shown stay until it answers.
  const [attempt, setAttempt] = useState(0);
  // The Monday of the week on show; null follows today's week.
  const [anchor, setAnchor] = useState<string | null>(null);
  const program = useCommand('curriculum.program', {});
  const planner = usePlanDay(() => {
    setAttempt((n) => n + 1);
  });
  const todayWeek = today.date === null ? null : formatIsoDate(startOfIsoWeek(today.date));
  const weekKey = anchor ?? todayWeek;
  const programStart = program.data?.program_start?.monday ?? null;
  const shift = (days: number) => {
    const from = weekKey === null ? null : parseIsoDate(weekKey);
    if (from !== null) {
      setAnchor(formatIsoDate(addDays(from, days)));
    }
  };

  useEffect(() => {
    // Keyed on the week string only: `today.date` is a new object on every render, and depending
    // on it re-ran this effect after every setNodes (an endless daily_plan request loop).
    const monday = weekKey === null ? null : parseIsoDate(weekKey);
    if (weekKey === null || monday === null) {
      return;
    }
    let active = true;
    const dates = Array.from({ length: DAYS_PER_WEEK }, (_, i) => formatIsoDate(addDays(monday, i)));
    void Promise.all(dates.map((date) => client.invoke('curriculum.daily_plan', { date }))).then((results) => {
      if (!active) {
        return;
      }
      setNodes({
        key: weekKey,
        days: results.map((r, i) => ({ date: dates[i] ?? '', plan: isG9Error(r) ? null : r.result, error: isG9Error(r) ? r : null, warnings: warningsOf(r) })),
      });
    });
    return () => {
      active = false;
    };
  }, [client, weekKey, attempt]);

  const current = nodes !== null && nodes.key === weekKey ? nodes : null;
  const days = current?.days ?? [];
  const plans = days.flatMap((d) => (d.plan === null ? [] : [d.plan]));
  const failed = days.length - plans.length;
  const first = plans[0];
  const weekWarnings = mergeWarnings(...days.map((d) => d.warnings), program.warnings, planner.warnings);
  // Every day answered "not set up": one message, not six errors.
  const notSetUp = days.length > 0 && days.every((d) => d.error !== null && SETUP_CODES.includes(d.error.code));
  // Non-canonical versions (the PROPOSED_SEED today) produce PROPOSED plans, never VERIFIED ones.
  const weekLabel = hasWarning(weekWarnings, NON_CANONICAL_WARNING) ? 'PROPOSED' : 'VERIFIED';
  // Sums of what was read, nothing else: a day that failed adds nothing and is reported as missing.
  const objectives = plans.reduce((sum, p) => sum + p.objectives.length, 0);
  const plannedMinutes = plans.reduce((sum, p) => sum + p.planned_minutes, 0);
  const weekNumber = first?.week_number ?? null;
  const weekName = first?.week_title ?? null;
  const weekTitle = weekNumber === null ? 'This week' : weekName === null ? `Week ${weekNumber}` : `Week ${weekNumber}: ${weekName}`;
  const monday = weekKey === null ? null : parseIsoDate(weekKey);
  const unreadable = current === null && today.error !== null;
  const stamp = plans.length === 0 ? undefined : <ProvenanceBadge label={weekLabel} />;

  return (
    <div className="stack">
      <PageHead
        title="Curriculum Navigator"
        lede="The weeks of the program, day by day: what each day teaches, how its time is split and the evidence it expects."
        actions={
          <>
            <button
              type="button"
              onClick={() => {
                shift(-7);
              }}
              disabled={weekKey === null}
            >
              Earlier week
            </button>
            <button
              type="button"
              onClick={() => {
                shift(7);
              }}
              disabled={weekKey === null}
            >
              Later week
            </button>
            <button
              type="button"
              onClick={() => {
                setAnchor(null);
              }}
              disabled={anchor === null}
            >
              This week
            </button>
            {programStart === null ? null : (
              <button
                type="button"
                onClick={() => {
                  setAnchor(programStart);
                }}
                disabled={anchor === programStart}
              >
                Week 1
              </button>
            )}
          </>
        }
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {program.error !== null ? <ErrorDisplay error={program.error} onRetry={program.reload} /> : null}
      {planner.error !== null ? <ErrorDisplay error={planner.error} /> : null}
      <Warnings warnings={weekWarnings} />

      <div className="with-margin">
        <Panel
          title={weekTitle}
          actions={stamp}
          note={
            monday === null || current === null
              ? undefined
              : `${formatIsoDate(monday)} to ${formatIsoDate(addDays(monday, DAYS_PER_WEEK - 1))}, Monday to Saturday. Open a day for its objectives, its blocks and the evidence it expects.`
          }
        >
          {current === null ? (
            unreadable ? (
              <NoData reason={`The backend clock could not be read (${today.error?.code ?? 'unknown'}), so this week cannot be worked out and no day was requested.`} />
            ) : (
              <Loading what="this week’s plan" />
            )
          ) : notSetUp ? (
            <NoData
              title="Program not set up"
              reason="The weeks come from the curriculum. Import it and choose the Monday week 1 starts on Mission Control."
              action={
                <Link className="button-link" to="/" viewTransition>
                  Open Mission Control
                </Link>
              }
            />
          ) : (
            <ul className="tree" aria-label="Curriculum tree">
              {days.map((d) => {
                const civil = parseIsoDate(d.date);
                const name = civil === null ? '' : (WEEKDAY_NAMES[isoWeekday(civil) - 1] ?? '');
                const isToday = d.date === today.isoDate;
                const plan = d.plan;
                const dayCode = plan?.day_id ?? null;
                return (
                  <li key={d.date} aria-current={isToday ? 'date' : undefined}>
                    {/* Today is open; so is a day that failed, because an error is never folded away. */}
                    <details className="day-group tree-day" data-today={isToday} open={isToday || d.error !== null}>
                      <summary>
                        <strong>{name}</strong> <time dateTime={d.date}>{d.date}</time> {dayCode === null ? null : <code>{dayCode}</code>}{' '}
                        {d.error !== null ? (
                          <span className="row-sub" data-standing="owed">
                            Could not be read ({d.error.code})
                          </span>
                        ) : plan === null ? (
                          <span className="row-sub">No plan returned</span>
                        ) : plan.unscheduled ? (
                          <span className="row-sub">Rest day</span>
                        ) : (
                          <span className="row-sub">
                            {formatMinutes(plan.planned_minutes)} planned, {count(plan.objectives.length, 'objective', 'objectives')}
                          </span>
                        )}{' '}
                        {isToday ? <span className="tree-today">Today</span> : null}{' '}
                        {plan?.is_assessment_day === true ? (
                          <Badge tone="state" title="is_assessment_day">
                            Assessment day
                          </Badge>
                        ) : null}
                      </summary>
                      <div className="stack-sm">
                        {d.error !== null ? (
                          <ErrorDisplay
                            error={d.error}
                            onRetry={() => {
                              setAttempt((n) => n + 1);
                            }}
                          />
                        ) : plan === null ? (
                          <NoData reason="The backend returned no plan for this date." />
                        ) : plan.unscheduled ? (
                          <NoData title="Rest day" reason="No study blocks are scheduled for this date." />
                        ) : (
                          <>
                            <h3 className="visually-hidden">Objectives</h3>
                            <ol className="rows">
                              {plan.objectives.map((o) => (
                                <li key={o.objective_id}>
                                  <div className="row-main">
                                    <span className="row-title">{o.text}</span>
                                    <span className="row-sub">
                                      <code>{o.objective_id}</code>
                                    </span>
                                  </div>
                                  <div className="row-end">
                                    <StateBadge state={o.status} />
                                  </div>
                                </li>
                              ))}
                            </ol>
                            <h3 className="visually-hidden">Blocks, in order</h3>
                            <dl className="facts">
                              {plan.blocks.map((b) => (
                                <div key={b.seq}>
                                  <dt>{blockName(b)}</dt>
                                  <dd>{formatMinutes(b.minutes)}</dd>
                                </div>
                              ))}
                            </dl>
                            <p className="small muted">
                              {plan.evidence_artifact === null ? (
                                'No evidence is named for this day.'
                              ) : (
                                <>
                                  Evidence expected: <strong>{plan.evidence_artifact}</strong>
                                </>
                              )}
                            </p>
                            {plan.blocks.some((b) => b.session_id === null) ? (
                              <div className="toolbar">
                                <button
                                  type="button"
                                  className="sm"
                                  disabled={planner.busy}
                                  onClick={() => {
                                    void planner.plan(d.date);
                                  }}
                                >
                                  {plan.blocks.every((b) => b.session_id === null) ? 'Plan this day’s blocks' : 'Plan the missing blocks'}
                                </button>
                              </div>
                            ) : (
                              <p className="small muted">Every block of this day has a session.</p>
                            )}
                          </>
                        )}
                      </div>
                    </details>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>

        <Margin label="About this curriculum">
          <Panel title="Curriculum version" as="aside" actions={stamp}>
            {current === null ? (
              unreadable ? (
                <p className="muted">Not known: no day of this week was requested.</p>
              ) : (
                <Loading what="the version’s standing" />
              )
            ) : plans.length === 0 ? (
              <p className="muted">No day of this week could be read, so nothing is known about the version it comes from.</p>
            ) : (
              <>
                <p>
                  {weekLabel === 'PROPOSED'
                    ? 'Proposed means this week was read from a curriculum version that has not been approved as canonical yet. Its days, objectives and times can still change.'
                    : 'Verified means this week was read from the curriculum version that is approved as canonical.'}
                </p>
                {program.data?.curriculum == null ? (
                  <p className="small muted">The version is not named here: the plan read says only whether its version is canonical.</p>
                ) : (
                  <dl className="pairs">
                    <dt>Curriculum</dt>
                    <dd>{program.data.curriculum.name ?? program.data.curriculum.version_id}</dd>
                    <dt>Version</dt>
                    <dd>
                      <code>{program.data.curriculum.version_label}</code>
                    </dd>
                    <dt>Content</dt>
                    <dd className="num">
                      {program.data.curriculum.weeks} weeks, {program.data.curriculum.days} days, {program.data.curriculum.objectives} objectives
                    </dd>
                    {programStart === null ? null : (
                      <>
                        <dt>Week 1</dt>
                        <dd>{formatCivilDate(programStart)}</dd>
                      </>
                    )}
                  </dl>
                )}
              </>
            )}
          </Panel>

          <Panel title="This week, as read" as="aside">
            {current === null ? (
              unreadable ? (
                <p className="muted">Nothing was read.</p>
              ) : (
                <Loading what="the week" />
              )
            ) : plans.length === 0 ? (
              <NoData reason={`None of the ${days.length} days could be read, so there is nothing to count.`} />
            ) : (
              <div className="stack-sm">
                <dl className="pairs">
                  <dt>Week</dt>
                  {weekNumber === null ? <dd>Not given by the plan</dd> : <dd className="num">{`${weekNumber} of ${PROGRAM_WEEKS}`}</dd>}
                  <dt>Days loaded</dt>
                  <dd className="num">
                    {plans.length} of {days.length}
                  </dd>
                  <dt>Objectives</dt>
                  <dd className="num">{objectives}</dd>
                  <dt>Planned time</dt>
                  <dd className="num">{formatMinutes(plannedMinutes)}</dd>
                </dl>
                {failed > 0 ? (
                  <p className="small" data-testid="curriculum-partial">
                    <strong>Partial.</strong> {count(failed, 'day', 'days')} could not be read, so the objectives and the planned time count only the {count(plans.length, 'day', 'days')} that
                    loaded.
                  </p>
                ) : (
                  <p className="small muted">Sums of the days listed here, as the backend returned them.</p>
                )}
              </div>
            )}
          </Panel>

          <MarginNote title="The program as designed">
            <p>
              {PROGRAM_WEEKS} weeks of {DAYS_PER_WEEK} days, {OBJECTIVES_PER_DAY} objectives a day. A weekday is planned at {formatMinutes(WEEKDAY_MINUTES)}, a Saturday at{' '}
              {formatMinutes(SATURDAY_MINUTES)}: {formatMinutes(WEEK_MINUTES)} a week, {formatMinutes(PROGRAM_MINUTES)} in all. Sunday is not scheduled.
            </p>
            <p>These are the curriculum’s invariants (ARCHITECTURE, section 4), not sums of what was read.</p>
          </MarginNote>
        </Margin>
      </div>
    </div>
  );
}
