import { Link } from 'react-router-dom';
import { formatDuration, formatMinutes, humanizeEnum } from '../../lib/format';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

export function DailyClassroom() {
  const today = useToday();
  const plan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });
  const sessions = useCommand('session.list', today.isoDate === null ? null : { date: today.isoDate });
  const list = sessions.data?.sessions ?? [];

  return (
    <div>
      <h1>Daily Classroom</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {plan.error !== null ? <ErrorDisplay error={plan.error} onRetry={plan.reload} /> : null}
      {plan.loading ? <Loading what="today's plan" /> : null}
      {plan.data !== null ? (
        plan.data.unscheduled ? (
          <NoData reason={`${plan.data.date} is unscheduled (protected rest)`} />
        ) : (
          <div className="grid-2">
            <Panel title={`${plan.data.day_id} — ${plan.data.week_title}`}>
              <p className="muted">
                Week {plan.data.week_number} · {formatMinutes(plan.data.planned_minutes)} planned · evidence artifact: {plan.data.evidence_artifact}{' '}
                {plan.data.is_assessment_day ? <StateBadge state="ASSESSMENT DAY" /> : null} <ProvenanceBadge label="VERIFIED" />
              </p>
              <h3>Objectives</h3>
              <ol>
                {plan.data.objectives.map((o) => (
                  <li key={o.objective_id}>
                    <code>{o.objective_id}</code> {o.text}
                  </li>
                ))}
              </ol>
              <h3>Blocks</h3>
              <Table
                caption="Study blocks and their session state"
                headers={['#', 'Block', 'Planned', 'Session state', 'Credited', '']}
                rows={plan.data.blocks.map((b) => {
                  const session = list.find((x) => x.session_id === b.session_id);
                  return {
                    key: `${b.seq}`,
                    cells: [
                      b.seq,
                      humanizeEnum(b.block),
                      formatMinutes(b.minutes),
                      session === undefined ? <span className="muted">none</span> : <StateBadge state={session.state} />,
                      session === undefined ? '—' : formatDuration(session.credited_ms),
                      session === undefined ? null : <Link to="/session">Open tracker</Link>,
                    ],
                  };
                })}
              />
            </Panel>
            <div>
              <Panel title="Context-aware teaching">
                {today.status === null ? (
                  <Loading what="model status" />
                ) : today.status.model.available ? (
                  <NoData reason="no lesson proposal was generated for this block" />
                ) : (
                  <ModelUnavailable reason={today.status.model.reason} what="Lesson proposal" />
                )}
                <p className="muted">
                  Teaching output is always a <ProvenanceBadge label="RECOMMENDATION" />; the objective list above is the <ProvenanceBadge label="VERIFIED" /> plan.
                </p>
              </Panel>
              <Panel title="Capture">
                <p>
                  Notes and evidence for this day go to the <Link to="/journal">Learning Journal</Link> (sanitized markdown, content-hashed revisions).
                </p>
              </Panel>
            </div>
          </div>
        )
      ) : null}
    </div>
  );
}
