import { useCallback } from 'react';
import { Link } from 'react-router-dom';
import type { DailyPlan, SessionView } from '../../ipc/operations';
import { addDays, startOfDayUtc } from '../../lib/civil-time';
import { blockName, formatCivilDate, formatDateTime, formatDuration, formatMinutes, formatTime, sessionBlockName } from '../../lib/format';
import { DAYS_PER_WEEK, PROGRAM_DAYS, dayNumber, programMap } from '../../lib/program';
import { NON_CANONICAL_WARNING, hasWarning, mergeWarnings } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { Bars, ChartFigure, Legend, ProgramMap, type MapDay } from '../components/charts';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin } from '../components/Margin';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { ProgramSetup } from '../components/ProgramSetup';
import { Stat } from '../components/Stat';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { usePlanDay } from '../hooks/usePlanDay';
import { useToday } from '../hooks/useToday';

const WORK_DONE: readonly SessionView['state'][] = ['COMPLETED', 'REVIEWED', 'CLOSED'];

function describeDay(d: MapDay): string {
  if (d.planned_ms === null) {
    return `${d.id}: nothing recorded`;
  }
  const doubt = d.uncertain_ms > 0 ? `, ${formatDuration(d.uncertain_ms)} in doubt` : '';
  return `${d.id}: ${formatDuration(d.credited_ms)} of ${formatDuration(d.planned_ms)} credited${doubt}`;
}

/**
 * The one thing to do next, in solid ink. Every value is the backend's; when today's sessions
 * could not be read the block says so and offers nothing to press.
 */
function NowBlock({
  plan,
  sessions,
  sessionsFailed,
  setupIncomplete,
  startsOn,
  onPlan,
  planning,
}: {
  plan: DailyPlan | null;
  sessions: readonly SessionView[] | null;
  sessionsFailed: boolean;
  /** The curriculum is not imported or the start is not set: there is no plan to run yet. */
  setupIncomplete: boolean;
  /** The Monday of week 1 when today is before it; null otherwise. */
  startsOn: string | null;
  /** Plan today's blocks (one session per block); offered when the plan has blocks and no session exists. */
  onPlan: () => void;
  planning: boolean;
}) {
  const running = sessions?.find((x) => x.state === 'RUNNING');
  const paused = sessions?.find((x) => x.state === 'PAUSED');
  const interrupted = sessions?.find((x) => x.state === 'INTERRUPTED' || x.requires_correction);
  const next = sessions?.find((x) => x.state === 'READY' || x.state === 'PLANNED');
  const active = running ?? paused;
  const blockCount = plan === null || plan.unscheduled ? null : plan.blocks.length;

  let kicker: string;
  let title: string;
  let meta: string;
  let action: { to: string; label: string } | { onClick: () => void; label: string } | null = null;

  if (setupIncomplete) {
    kicker = 'Setup';
    title = 'Set up your program';
    meta = 'Import the curriculum and choose the Monday week 1 starts. Until then there is no plan to run.';
  } else if (startsOn !== null) {
    kicker = 'Today';
    title = `Week 1 starts ${formatCivilDate(startsOn)}`;
    meta = 'There is nothing to run before then. The Curriculum Navigator shows every week in advance.';
    action = { to: '/curriculum', label: 'Browse the weeks' };
  } else if (plan?.unscheduled === true && plan.week_number === null) {
    kicker = 'Today';
    title = 'Outside the program weeks';
    meta = 'Today is after the last week of the program, so there are no blocks to run.';
  } else if (sessions === null) {
    kicker = 'Today';
    title = sessionsFailed ? 'Today’s sessions could not be read' : 'Reading today’s sessions…';
    meta = sessionsFailed ? 'The reason is shown above. Nothing is assumed about your time.' : '';
  } else if (active !== undefined) {
    kicker = active.state === 'RUNNING' ? 'Running now' : 'Paused';
    title = sessionBlockName(active, plan);
    meta = `${formatDuration(active.credited_ms)} credited of ${formatDuration(active.planned_ms)} planned`;
    action = { to: '/session', label: active.state === 'RUNNING' ? 'Open the timer' : 'Resume in the session tracker' };
  } else if (interrupted !== undefined) {
    kicker = 'Needs you';
    title = `${sessionBlockName(interrupted, plan)} was interrupted`;
    meta = `${formatDuration(interrupted.uncertain_ms)} could not be vouched for and is not credited until you correct it.`;
    action = { to: '/session', label: 'Correct the time' };
  } else if (next !== undefined) {
    kicker = 'Next';
    title = sessionBlockName(next, plan);
    meta = `${formatMinutes(Math.round(next.planned_ms / 60_000))} planned${blockCount === null ? '' : `, block ${next.block_seq} of ${blockCount}`}`;
    action = { to: '/session', label: `Go to ${sessionBlockName(next, plan).toLowerCase()}` };
  } else if (plan?.unscheduled === true) {
    kicker = 'Today';
    title = 'Rest day';
    meta = 'No study blocks are scheduled for this date.';
  } else if (sessions.length === 0 && plan !== null && blockCount !== null && blockCount > 0) {
    kicker = 'Today';
    title = 'Today’s blocks are not planned yet';
    meta = `${blockCount === 1 ? '1 block' : `${blockCount} blocks`}, ${formatMinutes(plan.planned_minutes)} from the curriculum. Planning creates one session per block; no clock runs until you start one.`;
    action = { onClick: onPlan, label: planning ? 'Planning…' : 'Plan today’s blocks' };
  } else if (sessions.length === 0) {
    kicker = 'Today';
    title = 'Nothing is planned yet';
    meta = 'No session exists for today. Plan one to start the clock.';
    action = { to: '/session', label: 'Plan a session' };
  } else {
    kicker = 'Today';
    title = 'Every block is done';
    meta = `${formatDuration(sessions.reduce((sum, x) => sum + x.credited_ms, 0))} credited today.`;
  }

  return (
    <section className="now" aria-label="What to do next" data-testid="now-block">
      <div>
        <p className="now-kicker">{kicker}</p>
        <p className="now-title">{title}</p>
        {meta === '' ? null : <p className="now-meta">{meta}</p>}
      </div>
      {action === null ? null : 'to' in action ? (
        <Link className="button-link" to={action.to} viewTransition>
          {action.label}
        </Link>
      ) : (
        <button type="button" className="primary" disabled={planning} onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </section>
  );
}

export function MissionControl() {
  const today = useToday();
  const plan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const allSessions = useCommand('session.list', {});
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const interventions = useCommand('interventions.list', {});
  const approvals = useCommand('approvals.list', { state: 'PENDING' });
  const calendar = useCommand(
    'calendar.list',
    today.date === null ? null : { from: startOfDayUtc(today.date, today.zone), to: startOfDayUtc(addDays(today.date, 2), today.zone) },
  );
  const program = useCommand('curriculum.program', {});
  const planReload = plan.reload;
  const sessionsReload = sessions.reload;
  const allSessionsReload = allSessions.reload;
  const reloadPlanned = useCallback(() => {
    planReload();
    sessionsReload();
    allSessionsReload();
  }, [planReload, sessionsReload, allSessionsReload]);
  const planner = usePlanDay(reloadPlanned);
  // Until `curriculum.program` answers (or when it cannot be read) nothing is claimed about the setup.
  const setupIncomplete = program.data !== null && (program.data.curriculum === null || program.data.program_start === null);
  const mondayOne = program.data?.program_start?.monday ?? null;
  const startsOn = mondayOne !== null && today.isoDate !== null && today.isoDate < mondayOne ? mondayOne : null;

  const s = today.status;
  // Derived values exist only when their records loaded. A failed or pending read is shown as an
  // error or a "no data" stat, never as a computed zero (Panel.tsx NoData rule, rtm R-014).
  const todaySessions = sessions.data?.sessions ?? null;
  const day = plan.data;
  const nextCommitment = today.now === null ? undefined : calendar.data?.events.find((e) => e.start > (today.now ?? 0));
  const credited = todaySessions === null ? null : todaySessions.reduce((acc, x) => acc + x.credited_ms, 0);
  const doneCount = todaySessions === null ? null : todaySessions.filter((x) => WORK_DONE.includes(x.state)).length;
  const blockSessions = day === null || todaySessions === null ? null : day.blocks.flatMap((b) => todaySessions.filter((x) => x.session_id === b.session_id));
  const blockTotals =
    blockSessions === null
      ? null
      : {
          credited: blockSessions.reduce((sum, x) => sum + x.credited_ms, 0),
          uncertain: blockSessions.reduce((sum, x) => sum + x.uncertain_ms, 0),
        };
  const hypotheses = (interventions.data?.interventions ?? []).filter((i) => i.state === 'HYPOTHESIS' || i.state === 'PROPOSED');
  const planLabel = hasWarning(plan.warnings, NON_CANONICAL_WARNING) ? 'PROPOSED' : 'VERIFIED';
  const map = allSessions.data === null ? null : programMap(allSessions.data.sessions, day?.day_id ?? null);
  const todayNumber = day?.day_id === null || day?.day_id === undefined ? null : dayNumber(day.day_id);
  const sessionReason = sessions.error === null ? 'Today’s sessions are still being read.' : `Today’s sessions could not be read (${sessions.error.code}).`;

  return (
    <div className="stack">
      <PageHead title="Mission Control" lede="What is planned today, what has been credited, and what waits for you." />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {calendar.error !== null ? <ErrorDisplay error={calendar.error} onRetry={calendar.reload} /> : null}
      <Warnings
        warnings={mergeWarnings(plan.warnings, sessions.warnings, allSessions.warnings, due.warnings, interventions.warnings, approvals.warnings, calendar.warnings, program.warnings)}
      />

      {program.error !== null ? <ErrorDisplay error={program.error} onRetry={program.reload} /> : null}
      {planner.error !== null ? <ErrorDisplay error={planner.error} /> : null}
      <Warnings warnings={planner.warnings} label="Planning warnings" testId="plan-warnings" />

      <NowBlock
        plan={day}
        sessions={todaySessions}
        sessionsFailed={sessions.error !== null}
        setupIncomplete={setupIncomplete}
        startsOn={startsOn}
        planning={planner.busy}
        onPlan={() => {
          if (today.isoDate !== null) {
            void planner.plan(today.isoDate);
          }
        }}
      />

      {setupIncomplete ? (
        <ProgramSetup
          program={program}
          zone={today.zone}
          now={today.now}
          todayIso={today.isoDate}
          onChanged={() => {
            reloadPlanned();
            today.reload();
          }}
        />
      ) : null}

      <section className="bento" aria-label="Today in numbers">
        <div data-span="3">
          {credited === null ? (
            <Stat label="Credited today" value="No data" standing="missing" foot={sessionReason} />
          ) : (
            <Stat
              label="Credited today"
              value={formatDuration(credited)}
              foot={day === null || day.unscheduled ? 'Time the backend has vouched for.' : `of ${formatMinutes(day.planned_minutes)} planned`}
            />
          )}
        </div>
        <div data-span="3">
          {doneCount === null ? (
            <Stat label="Blocks completed" value="No data" standing="missing" foot={sessionReason} />
          ) : (
            <Stat label="Blocks completed" value={doneCount} unit={day === null || day.unscheduled ? undefined : `of ${day.blocks.length}`} foot="Completed, reviewed or closed today." />
          )}
        </div>
        <div data-span="3">
          {due.data === null ? (
            <Stat label="Reviews due" value="No data" standing="missing" foot={due.error === null ? 'The review queue is still being read.' : `The review queue could not be read (${due.error.code}).`} />
          ) : (
            <Stat
              label="Reviews due"
              value={due.data.due.length + due.data.overdue.length + due.data.overflow.length}
              standing={due.data.overdue.length > 0 ? 'owed' : 'posted'}
              foot={`${due.data.overdue.length} overdue, ${due.data.overflow.length} beyond today’s ${formatMinutes(Math.round(due.data.budget_ms / 60_000))} budget`}
            />
          )}
        </div>
        <div data-span="3">
          {approvals.data === null ? (
            <Stat label="Waiting for you" value="No data" standing="missing" foot={approvals.error === null ? 'Approvals are still being read.' : `Approvals could not be read (${approvals.error.code}).`} />
          ) : (
            <Stat
              label="Waiting for you"
              value={approvals.data.approvals.length}
              standing={approvals.data.approvals.length > 0 ? 'doubt' : 'posted'}
              foot={approvals.data.approvals.length === 1 ? 'approval to decide' : 'approvals to decide'}
            />
          )}
        </div>
      </section>

      <div className="with-margin">
        <div className="stack">
          <Panel title="Today’s blocks">
            {setupIncomplete ? (
              <NoData title="Program not set up" reason="Today’s blocks come from the curriculum. Import it and choose the start above." />
            ) : plan.error !== null ? (
              <NoData reason={`Today’s plan could not be read (${plan.error.code}), so its blocks cannot be shown.`} />
            ) : day === null ? (
              <Loading what="today’s blocks" />
            ) : day.unscheduled ? (
              day.week_number === null ? (
                <NoData title="Outside the program weeks" reason={startsOn === null ? 'Today is after the last week of the program.' : `Week 1 starts ${formatCivilDate(startsOn)}.`} />
              ) : (
                <NoData title="Rest day" reason="No study blocks are scheduled for this date." />
              )
            ) : (
              <ChartFigure
                caption={`${day.day_id}, ${formatMinutes(day.planned_minutes)} planned. Each bar is as long as its block; the solid part is credited.`}
                table={
                  <Table
                    caption={`Blocks for ${day.day_id}: planned ${formatMinutes(day.planned_minutes)}`}
                    headers={['#', 'Block', 'Planned', 'Credited', 'In doubt', 'State']}
                    numeric={[0, 2, 3, 4]}
                    footer={
                      blockTotals === null ? undefined : ['', 'Total', formatMinutes(day.planned_minutes), formatDuration(blockTotals.credited), formatDuration(blockTotals.uncertain), '']
                    }
                    rows={day.blocks.map((b) => {
                      const session = todaySessions?.find((x) => x.session_id === b.session_id);
                      return {
                        key: `${b.seq}`,
                        cells: [
                          b.seq,
                          blockName(b),
                          formatMinutes(b.minutes),
                          session === undefined ? '—' : formatDuration(session.credited_ms),
                          session === undefined ? '—' : formatDuration(session.uncertain_ms),
                          todaySessions === null ? <span className="muted">No data</span> : session === undefined ? <span className="muted">No session</span> : <StateBadge state={session.state} />,
                        ],
                      };
                    })}
                  />
                }
              >
                <Bars
                  label="Today’s blocks: credited and in-doubt time against the plan"
                  rows={day.blocks.map((b) => {
                    const session = todaySessions?.find((x) => x.session_id === b.session_id);
                    return {
                      key: `${b.seq}`,
                      name: blockName(b),
                      of: b.minutes * 60_000,
                      posted: session?.credited_ms ?? 0,
                      doubted: session?.uncertain_ms ?? 0,
                      value: session === undefined ? `${formatMinutes(b.minutes)} planned` : `${formatDuration(session.credited_ms)} of ${formatMinutes(b.minutes)}`,
                    };
                  })}
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

          <Panel title="Today’s objectives" actions={day === null || day.unscheduled ? undefined : <ProvenanceBadge label={planLabel} />}>
            {plan.error !== null && !setupIncomplete ? <ErrorDisplay error={plan.error} /> : null}
            {setupIncomplete ? <NoData title="Program not set up" reason="Today’s objectives come from the curriculum. Import it and choose the start above." /> : null}
            {plan.loading && !setupIncomplete ? <Loading what="today’s plan" /> : null}
            {day !== null ? (
              day.unscheduled ? (
                day.week_number === null ? (
                  <NoData title="Outside the program weeks" reason={startsOn === null ? 'Today is after the last week of the program.' : `Week 1 starts ${formatCivilDate(startsOn)}.`} />
                ) : (
                  <NoData title="Rest day" reason="No objectives are scheduled for this date." />
                )
              ) : (
                <>
                  <ol className="rows">
                    {day.objectives.map((o) => (
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
                  <p className="panel-note">
                    Evidence expected today: <strong>{day.evidence_artifact}</strong>. Write it up in the <Link to="/journal">Learning Journal</Link>.
                  </p>
                </>
              )
            ) : null}
          </Panel>

          <Panel title="The program">
            {allSessions.error !== null ? (
              // The failed read is already reported once at the top of the page when it is today's; here it is the reason there is no map.
              <NoData
                reason={`Sessions could not be read (${allSessions.error.code}), so the program map cannot be drawn.`}
                action={
                  <button type="button" className="sm" onClick={allSessions.reload}>
                    Try again
                  </button>
                }
              />
            ) : map === null ? (
              <Loading what="the program map" />
            ) : (
              <ChartFigure
                caption={`${todayNumber === null ? '' : `Day ${todayNumber} of ${PROGRAM_DAYS}. `}One cell is one day, one row is one week. A cell fills as its planned time is credited.`}
                table={
                  <Table
                    caption="Recorded days of the program"
                    headers={['Day', 'Planned', 'Credited', 'In doubt']}
                    numeric={[1, 2, 3]}
                    emptyText="No day has a recorded session yet."
                    rows={map
                      .filter((d) => d.planned_ms !== null)
                      .map((d) => ({ key: d.id, cells: [d.id, formatDuration(d.planned_ms ?? 0), formatDuration(d.credited_ms), formatDuration(d.uncertain_ms)] }))}
                  />
                }
              >
                <ProgramMap
                  days={map}
                  daysPerWeek={DAYS_PER_WEEK}
                  describe={describeDay}
                  label={`The program, day by day: ${map.filter((d) => d.planned_ms !== null).length} of ${PROGRAM_DAYS} days have recorded sessions`}
                />
                <Legend
                  items={[
                    { mark: 'posted', label: 'Credited' },
                    { mark: 'doubt', label: 'In doubt' },
                    { mark: 'pencil', label: 'Planned' },
                    { mark: 'rest', label: 'Nothing recorded' },
                    { mark: 'today', label: 'Today' },
                  ]}
                />
              </ChartFigure>
            )}
          </Panel>
        </div>

        <Margin label="What waits for you">
          <Panel title="Reviews" as="aside">
            {due.error !== null ? <ErrorDisplay error={due.error} /> : null}
            {due.data === null ? (
              due.loading ? <Loading what="the review queue" /> : null
            ) : due.data.overdue.length + due.data.due.length + due.data.overflow.length === 0 ? (
              <p className="muted">Nothing is due.</p>
            ) : (
              <>
                {due.data.overdue.length > 0 ? (
                  <ul className="rows">
                    {due.data.overdue.slice(0, 3).map((r) => (
                      <li key={r.review_item_id}>
                        <div className="row-main">
                          <span className="row-title">{r.prompt}</span>
                          <span className="row-sub" data-standing="owed">
                            Overdue since {formatDateTime(r.due_at, today.zone)}
                          </span>
                        </div>
                      </li>
                    ))}
                  </ul>
                ) : null}
                <p className="panel-note">
                  {due.data.overdue.length} overdue, {due.data.due.length} due today, {due.data.overflow.length} beyond today’s{' '}
                  {formatMinutes(Math.round(due.data.budget_ms / 60_000))} budget. <Link to="/retention">Open the review queue</Link>
                </p>
              </>
            )}
          </Panel>

          <Panel title="Waiting for your approval" as="aside">
            {approvals.data === null ? (
              approvals.error !== null ? (
                <ErrorDisplay error={approvals.error} onRetry={approvals.reload} />
              ) : (
                <Loading what="approvals" />
              )
            ) : approvals.data.approvals.length === 0 ? (
              <p className="muted">Nothing waits for your approval.</p>
            ) : (
              <ul className="rows">
                {approvals.data.approvals.map((a) => (
                  <li key={a.approval_id}>
                    <div className="row-main">
                      <Link className="row-title" to="/approvals">
                        {a.summary}
                      </Link>
                      <span className="row-sub">Expires {formatTime(a.expires_at, today.zone)}</span>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Panel>

          <Panel title="Next on the calendar" as="aside">
            {calendar.data === null || s === null ? (
              calendar.error === null ? (
                <Loading what="the calendar" />
              ) : (
                <p className="muted">The calendar could not be read ({calendar.error.code}).</p>
              )
            ) : nextCommitment === undefined ? (
              <p className="muted">Nothing in the next 48 hours.</p>
            ) : (
              <p>
                <strong>{nextCommitment.title}</strong>
                <br />
                <span className="muted">{formatDateTime(nextCommitment.start, s.clock.zone)}</span>
              </p>
            )}
          </Panel>

          <Panel title="From the mentor" as="aside">
            {interventions.error !== null ? (
              <ErrorDisplay error={interventions.error} onRetry={interventions.reload} />
            ) : interventions.data === null ? (
              <Loading what="hypotheses" />
            ) : hypotheses.length === 0 ? (
              s !== null && !s.model.available ? (
                <ModelUnavailable reason={s.model.reason} what="Advice" />
              ) : (
                <p className="muted">No hypothesis is supported by the evidence yet.</p>
              )
            ) : (
              <ul className="rows">
                {hypotheses.map((h) => (
                  <li key={h.intervention_id}>
                    <div className="row-main">
                      <span className="row-title">{h.statement}</span>
                    </div>
                    <div className="row-end">
                      <ProvenanceBadge label="PROPOSED" />
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        </Margin>
      </div>
    </div>
  );
}
