import { useCallback } from 'react';
import { Link } from 'react-router-dom';
import { blockName, formatDuration, formatMinutes } from '../../lib/format';
import { findSheet } from '../../lib/lesson-sheets';
import { SETUP_CODES } from '../../lib/program';
import { NON_CANONICAL_WARNING, hasWarning, mergeWarnings } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { LessonMarginPanels, LessonSheetPanel, type LessonState } from '../components/LessonSheet';
import { Margin } from '../components/Margin';
import { PageHead } from '../components/PageHead';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useLessonSheets } from '../hooks/useLessonSheets';
import { usePlanDay } from '../hooks/usePlanDay';
import { useToday } from '../hooks/useToday';

/**
 * Today as the curriculum plans it: the objectives, the blocks in order with where each one
 * stands, what to hand in, and the lesson. Every figure is the backend's; a session that could
 * not be read is said in words and never shown as time not credited.
 */
export function DailyClassroom() {
  const today = useToday();
  const plan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const planReload = plan.reload;
  const sessionsReload = sessions.reload;
  const reloadPlanned = useCallback(() => {
    planReload();
    sessionsReload();
  }, [planReload, sessionsReload]);
  const planner = usePlanDay(reloadPlanned);
  const day = plan.data;
  const notSetUp = plan.error !== null && SETUP_CODES.includes(plan.error.code);
  // Null until today's sessions are read: a block's state is then unknown, not "no session".
  const todaySessions = sessions.data?.sessions ?? null;
  // A plan derived from a curriculum version that is not APPROVED_CANONICAL (the PROPOSED_SEED
  // today, rtm R-004) is a proposal, not a verified record.
  const planLabel = hasWarning(plan.warnings, NON_CANONICAL_WARNING) ? 'PROPOSED' : 'VERIFIED';
  const s = today.status;
  // The lesson sheets of the day on screen: read once a day with blocks is shown, and used only when
  // the plan's objectives are the ones the sheet was written for (see lib/lesson-sheets.ts).
  const sheetDayId = day !== null && !day.unscheduled ? day.day_id : null;
  const sheetsRead = useLessonSheets(sheetDayId !== null);
  const lesson: LessonState = {
    lookup:
      day !== null && sheetDayId !== null && sheetsRead.sheets !== null
        ? findSheet(
            sheetsRead.sheets,
            sheetDayId,
            day.objectives.map((o) => o.text),
          )
        : null,
    failed: sheetsRead.failed,
  };

  return (
    <div className="stack">
      <PageHead
        title="Daily Classroom"
        lede="Today’s objectives and study blocks in the order of the plan, and what to hand in when the day is done."
        actions={
          <Link className="button-link" to="/session" viewTransition>
            Open the session tracker
          </Link>
        }
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {plan.error !== null && !notSetUp ? <ErrorDisplay error={plan.error} onRetry={plan.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {planner.error !== null ? <ErrorDisplay error={planner.error} /> : null}
      <Warnings warnings={mergeWarnings(plan.warnings, sessions.warnings, planner.warnings)} />

      {day !== null && !day.unscheduled ? (
        <dl className="facts" data-testid="day-facts">
          {day.day_id === null ? null : (
            <div>
              <dt>Day</dt>
              <dd>
                <code>{day.day_id}</code>
              </dd>
            </div>
          )}
          {day.week_number === null ? null : (
            <div>
              <dt>Week</dt>
              <dd>
                {day.week_number}
                {day.week_title === null ? '' : `, ${day.week_title}`}
              </dd>
            </div>
          )}
          <div>
            <dt>Planned</dt>
            <dd>
              {formatMinutes(day.planned_minutes)} in {day.blocks.length === 1 ? '1 block' : `${day.blocks.length} blocks`}
            </dd>
          </div>
          {day.is_assessment_day ? (
            <div>
              <dt>Assessment</dt>
              <dd>Today is an assessment day</dd>
            </div>
          ) : null}
        </dl>
      ) : null}

      <div className="with-margin">
        <div className="stack">
          {notSetUp ? (
            <NoData
              title="Program not set up"
              reason="Today’s objectives and blocks come from the curriculum. Import it and choose the Monday week 1 starts on Mission Control."
              action={
                <Link className="button-link" to="/" viewTransition>
                  Open Mission Control
                </Link>
              }
            />
          ) : plan.error !== null ? (
            <NoData reason={`Today’s plan could not be read (${plan.error.code}), so its objectives and blocks cannot be shown.`} />
          ) : day === null ? (
            today.error !== null ? (
              <NoData reason={`Today’s date could not be read (${today.error.code}), so there is no plan to ask for.`} />
            ) : (
              <Loading what="today’s plan" />
            )
          ) : day.unscheduled ? (
            day.week_number === null ? (
              <NoData title="Outside the program weeks" reason={`${day.date} is before week 1 or after the last week of the program, so it has no study blocks. The Curriculum Navigator shows the weeks.`} />
            ) : (
              <NoData title="Rest day" reason={`No study blocks are scheduled for ${day.date}.`} />
            )
          ) : (
            <>
              <Panel title="Today’s objectives" actions={<ProvenanceBadge label={planLabel} />}>
                {day.objectives.length === 0 ? (
                  <p className="muted">The plan names no objective for today.</p>
                ) : (
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
                )}
              </Panel>

              <Panel
                title="Today’s blocks"
                actions={
                  <div className="toolbar">
                    {todaySessions !== null && day.blocks.some((b) => !todaySessions.some((x) => x.session_id === b.session_id)) ? (
                      <button
                        type="button"
                        className="sm primary"
                        disabled={planner.busy}
                        onClick={() => {
                          void planner.plan(day.date);
                        }}
                      >
                        {todaySessions.length === 0 ? 'Plan today’s blocks' : 'Plan the missing blocks'}
                      </button>
                    ) : null}
                    <ProvenanceBadge label={planLabel} />
                  </div>
                }
                note="In the order of the plan. A block is run and timed in the session tracker."
              >
                {day.blocks.length === 0 ? (
                  <p className="muted">The plan names no block for today.</p>
                ) : (
                  <ol className="rows">
                    {day.blocks.map((b) => {
                      const session = todaySessions?.find((x) => x.session_id === b.session_id);
                      return (
                        <li key={b.seq}>
                          <div className="row-main">
                            <Link className="row-title" to="/session" title="Open the session tracker" viewTransition>
                              {blockName(b)}
                            </Link>
                            <span className="row-sub">
                              Block {b.seq} of {day.blocks.length}, {formatMinutes(b.minutes)} planned
                              {session === undefined ? '' : `, ${formatDuration(session.credited_ms)} credited`}
                              {session === undefined || session.uncertain_ms === 0 ? '' : `, ${formatDuration(session.uncertain_ms)} in doubt`}
                            </span>
                          </div>
                          <div className="row-end">
                            {todaySessions === null ? (
                              sessions.error === null ? null : (
                                <span className="muted small" title={`Today’s sessions could not be read (${sessions.error.code}).`}>
                                  No data
                                </span>
                              )
                            ) : session === undefined ? (
                              <span className="muted small" title="No session exists for this block yet.">
                                No session
                              </span>
                            ) : (
                              <StateBadge state={session.state} />
                            )}
                          </div>
                        </li>
                      );
                    })}
                  </ol>
                )}
              </Panel>

              <LessonSheetPanel day={day} state={lesson} sessions={todaySessions} label={planLabel} />
            </>
          )}
        </div>

        <Margin label="Hand-in and lesson">
          <Panel title="What to hand in" as="aside">
            {notSetUp ? (
              <p className="muted">The program is not set up yet.</p>
            ) : plan.error !== null ? (
              <p className="muted">Today’s plan could not be read ({plan.error.code}).</p>
            ) : day === null ? (
              today.error === null ? <Loading what="today’s plan" /> : <p className="muted">Today’s date could not be read ({today.error.code}).</p>
            ) : day.unscheduled ? (
              <p className="muted">Nothing is due on a rest day.</p>
            ) : day.evidence_artifact === null ? (
              <p className="muted">The plan names nothing to hand in today.</p>
            ) : (
              <p>
                <strong>{day.evidence_artifact}</strong>
              </p>
            )}
            <p className="panel-note">
              Notes and evidence go in the journal, where every save is kept as a version under its content hash. <Link to="/journal">Open the Learning Journal</Link>
            </p>
          </Panel>

          <LessonMarginPanels state={lesson} />

          <Panel title="Lesson from the mentor" as="aside" actions={<ProvenanceBadge label="RECOMMENDATION" />} note="A lesson is advice. It never changes the plan, a timer or a grade.">
            {s === null ? (
              today.error === null ? (
                <Loading what="the model status" />
              ) : (
                <p className="muted">The model status could not be read ({today.error.code}).</p>
              )
            ) : s.model.available ? (
              <NoData title="Not built yet" reason="A model is running, but lessons on this page are planned, not built: the window asks the model for nothing here." />
            ) : (
              <ModelUnavailable reason={s.model.reason} what="Lesson" />
            )}
          </Panel>
        </Margin>
      </div>
    </div>
  );
}
