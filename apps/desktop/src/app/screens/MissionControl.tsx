import { Link } from 'react-router-dom';
import type { SessionView } from '../../ipc/operations';
import { addDays, startOfDayUtc } from '../../lib/civil-time';
import { formatDateTime, formatDuration, formatMinutes, formatTime, humanizeEnum } from '../../lib/format';
import { NON_CANONICAL_WARNING, hasWarning, mergeWarnings } from '../../lib/warnings';
import { Badge, ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Tabs } from '../components/Tabs';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

const WORK_DONE: readonly SessionView['state'][] = ['COMPLETED', 'REVIEWED', 'CLOSED'];

export function MissionControl() {
  const today = useToday();
  const plan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const due = useCommand('retention.due', today.now === null ? null : { as_of: today.now });
  const interventions = useCommand('interventions.list', {});
  const approvals = useCommand('approvals.list', { state: 'PENDING' });
  const calendar = useCommand(
    'calendar.list',
    today.date === null ? null : { from: startOfDayUtc(today.date, today.zone), to: startOfDayUtc(addDays(today.date, 2), today.zone) },
  );

  const s = today.status;
  // Derived values exist only when their records loaded. A failed or pending read is shown as an
  // error / loading state, never as a computed zero (Panel.tsx NoData rule, rtm R-014).
  const todaySessions = sessions.data?.sessions ?? null;
  const running = todaySessions?.find((x) => x.state === 'RUNNING' || x.state === 'PAUSED');
  const nextSession = todaySessions?.find((x) => x.state === 'READY' || x.state === 'PLANNED');
  const nextCommitment = today.now === null ? undefined : calendar.data?.events.find((e) => e.start > (today.now ?? 0));
  const credited = todaySessions === null ? null : todaySessions.reduce((acc, x) => acc + x.credited_ms, 0);
  const verifiedDone = todaySessions === null ? null : todaySessions.filter((x) => WORK_DONE.includes(x.state));
  const plannedMs = plan.data === null ? null : plan.data.planned_minutes * 60_000;
  const hypotheses = (interventions.data?.interventions ?? []).filter((i) => i.state === 'HYPOTHESIS' || i.state === 'PROPOSED');
  const planLabel = hasWarning(plan.warnings, NON_CANONICAL_WARNING) ? 'PROPOSED' : 'VERIFIED';

  return (
    <div>
      <h1>Mission Control</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {calendar.error !== null ? <ErrorDisplay error={calendar.error} onRetry={calendar.reload} /> : null}
      <Warnings
        warnings={mergeWarnings(plan.warnings, sessions.warnings, due.warnings, interventions.warnings, approvals.warnings, calendar.warnings)}
      />
      <section className="panel" aria-label="Today header">
        {s === null ? (
          <Loading what="system status" />
        ) : (
          <div className="row" style={{ gap: 24 }}>
            <div>
              <h3>Date / zone</h3>
              <div>
                {today.isoDate} <span className="muted">{s.clock.zone}</span>
              </div>
            </div>
            <div>
              <h3>Model</h3>
              {s.model.available ? <Badge tone="verified">AVAILABLE</Badge> : <Badge tone="off">UNAVAILABLE · {s.model.reason ?? 'UNKNOWN'}</Badge>}
            </div>
            <div>
              <h3>Network</h3>
              {s.network.egress_allowed ? <Badge tone="proposed">EGRESS ALLOWED</Badge> : <Badge tone="off">OFFLINE</Badge>}
            </div>
            <div>
              <h3>Session</h3>
              {todaySessions === null ? (
                <span className="muted">{sessions.error === null ? 'Loading sessions…' : `NO DATA — sessions unavailable (${sessions.error.code})`}</span>
              ) : running !== undefined ? (
                <span>
                  <StateBadge state={running.state} /> {running.day_id} {humanizeEnum(running.block)}
                </span>
              ) : nextSession !== undefined ? (
                <Link to="/session">Start next: {humanizeEnum(nextSession.block)}</Link>
              ) : (
                <span className="muted">No session planned today</span>
              )}
            </div>
            <div>
              <h3>Next commitment</h3>
              {calendar.data === null ? (
                <span className="muted">{calendar.error === null ? 'Loading calendar…' : `NO DATA — calendar unavailable (${calendar.error.code})`}</span>
              ) : nextCommitment === undefined ? (
                <span className="muted">None in the next 48 h</span>
              ) : (
                <span>
                  {formatDateTime(nextCommitment.start, s.clock.zone)} {nextCommitment.title}
                </span>
              )}
            </div>
          </div>
        )}
      </section>

      <div className="grid-3">
        <div>
          <Panel title="Today's objectives">
            {plan.error !== null ? <ErrorDisplay error={plan.error} /> : null}
            {plan.loading ? <Loading what="daily plan" /> : null}
            {plan.data !== null ? (
              plan.data.unscheduled ? (
                <NoData reason="unscheduled day (protected rest)" />
              ) : (
                <>
                  <p className="muted">
                    {plan.data.day_id} · week {plan.data.week_number} · {plan.data.week_title} · evidence: {plan.data.evidence_artifact} <ProvenanceBadge label={planLabel} />
                  </p>
                  <ol>
                    {plan.data.objectives.map((o) => (
                      <li key={o.objective_id}>
                        <code>{o.objective_id}</code> {o.text} <StateBadge state={o.status} />
                      </li>
                    ))}
                  </ol>
                </>
              )
            ) : null}
          </Panel>

          <Panel title="Time budget">
            {plan.data === null || plan.data.unscheduled ? (
              <NoData reason="no study blocks for this date" />
            ) : (
              <Table
                caption={`Blocks for ${plan.data.day_id}: planned ${formatMinutes(plan.data.planned_minutes)}`}
                headers={['#', 'Block', 'Planned', 'Credited', 'Uncertain', 'State']}
                rows={plan.data.blocks.map((b) => {
                  const session = todaySessions?.find((x) => x.session_id === b.session_id);
                  return {
                    key: `${b.seq}`,
                    cells: [
                      b.seq,
                      humanizeEnum(b.block),
                      formatMinutes(b.minutes),
                      session === undefined ? '—' : formatDuration(session.credited_ms),
                      session === undefined ? '—' : formatDuration(session.uncertain_ms),
                      todaySessions === null ? (
                        <span className="muted">NO DATA</span>
                      ) : session === undefined ? (
                        <span className="muted">no session</span>
                      ) : (
                        <StateBadge state={session.state} />
                      ),
                    ],
                  };
                })}
              />
            )}
          </Panel>

          <Panel title="Teaching panel">
            {s === null ? (
              <Loading what="model status" />
            ) : s.model.available ? (
              <NoData reason="no recommendation has been produced for today's objectives" />
            ) : (
              <ModelUnavailable reason={s.model.reason} />
            )}
          </Panel>

          <Panel title="Lab / evidence">
            <Tabs
              label="Lab and evidence"
              tabs={[
                {
                  id: 'lab',
                  label: 'Lab',
                  content: <NoData reason="lab orchestration is not an enabled capability in this build (separately approved adapter, ARCHITECTURE §13)" />,
                },
                {
                  id: 'evidence',
                  label: 'Evidence',
                  content:
                    plan.data === null || plan.data.unscheduled ? (
                      <NoData reason="no evidence artifact expected today" />
                    ) : (
                      <p>
                        Expected artifact: <strong>{plan.data.evidence_artifact}</strong>. Capture it in the <Link to="/journal">Learning Journal</Link>; evidence
                        references attach to the attempt in <Link to="/assessments">Assessments</Link>.
                      </p>
                    ),
                },
              ]}
            />
          </Panel>
        </div>

        <div>
          <Panel title="Overdue reviews" as="aside">
            {due.error !== null ? <ErrorDisplay error={due.error} /> : null}
            {due.data === null ? (
              due.loading ? <Loading what="review queue" /> : null
            ) : due.data.overdue.length === 0 ? (
              <NoData reason="no overdue reviews" />
            ) : (
              <ul>
                {due.data.overdue.map((r) => (
                  <li key={r.review_item_id}>
                    {r.prompt} <span className="muted">(due {formatDateTime(r.due_at, today.zone)})</span>
                  </li>
                ))}
              </ul>
            )}
            {due.data !== null && due.data.due.length > 0 ? (
              <p className="muted">
                {due.data.due.length} due today · {due.data.overflow.length} beyond the {formatMinutes(Math.round(due.data.budget_ms / 60_000))} budget
              </p>
            ) : null}
          </Panel>

          <Panel title="Weak-skill hypotheses" as="aside">
            {interventions.error !== null ? (
              <ErrorDisplay error={interventions.error} onRetry={interventions.reload} />
            ) : interventions.data === null ? (
              <Loading what="hypotheses" />
            ) : hypotheses.length === 0 ? (
              s !== null && !s.model.available ? (
                <ModelUnavailable reason={s.model.reason} what="Hypotheses" />
              ) : (
                <NoData reason="no supported hypotheses" />
              )
            ) : (
              <ul>
                {hypotheses.map((h) => (
                  <li key={h.intervention_id}>
                    <ProvenanceBadge label="PROPOSED" /> {h.statement}
                  </li>
                ))}
              </ul>
            )}
          </Panel>

          <Panel title="Pending approvals" as="aside">
            {approvals.data === null ? (
              approvals.error !== null ? (
                <ErrorDisplay error={approvals.error} onRetry={approvals.reload} />
              ) : (
                <Loading what="approvals" />
              )
            ) : approvals.data.approvals.length === 0 ? (
              <NoData reason="nothing awaits approval" />
            ) : (
              <ul>
                {approvals.data.approvals.map((a) => (
                  <li key={a.approval_id}>
                    <Link to="/approvals">{a.summary}</Link> <span className="muted">expires {formatTime(a.expires_at, today.zone)}</span>
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        </div>
      </div>

      <footer className="panel" aria-label="Day summary">
        <div className="row" style={{ gap: 24 }}>
          <div>
            <h3>Verified work</h3>
            {verifiedDone === null ? (
              <NoData reason={sessions.error === null ? 'sessions loading' : 'sessions could not be read'} />
            ) : (
              <>
                <span className="kpi">{verifiedDone.length}</span> <span className="muted">blocks completed</span> <ProvenanceBadge label="VERIFIED" />
              </>
            )}
          </div>
          <div>
            <h3>Estimated vs actual</h3>
            {plannedMs === null ? (
              <span className="muted">NO DATA — plan not loaded</span>
            ) : (
              <>
                <span className="kpi">{formatDuration(plannedMs)}</span> <span className="muted">planned</span>
              </>
            )}{' '}
            ·{' '}
            {credited === null ? (
              <span className="muted">NO DATA — sessions not loaded</span>
            ) : (
              <>
                <span className="kpi">{formatDuration(credited)}</span> <span className="muted">credited</span>
              </>
            )}
          </div>
          <div>
            <h3>Business milestones</h3>
            <NoData reason="executive operations are PROPOSED (M14)" />
          </div>
          <div>
            <h3>Next step</h3>
            {todaySessions === null ? (
              <span className="muted">NO DATA — sessions not loaded</span>
            ) : nextSession !== undefined ? (
              <span>
                Prepare <strong>{humanizeEnum(nextSession.block)}</strong> ({formatMinutes(Math.round(nextSession.planned_ms / 60_000))})
              </span>
            ) : (
              <span className="muted">No planned block remaining</span>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
}
