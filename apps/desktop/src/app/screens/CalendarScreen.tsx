import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import { addDays, formatIsoDate, startOfDayUtc, startOfIsoWeek, type CivilDate } from '../../lib/civil-time';
import { formatDateTime } from '../../lib/format';
import { ApprovalCard } from '../components/ApprovalCard';
import { StateBadge } from '../components/Badge';
import { CalendarGrid } from '../components/CalendarGrid';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

export function CalendarScreen() {
  const today = useToday();
  const client = useIpc();
  const [offsetWeeks, setOffsetWeeks] = useState(0);
  const weekStart: CivilDate | null = today.date === null ? null : addDays(startOfIsoWeek(today.date), offsetWeeks * 7);
  const range = weekStart === null ? null : { from: startOfDayUtc(weekStart, today.zone), to: startOfDayUtc(addDays(weekStart, 7), today.zone) };
  const events = useCommand('calendar.list', range);
  const [selection, setSelection] = useState<string | null>(null);
  const propose = useCommand('calendar.propose', null);
  const [proposalId, setProposalId] = useState<string | null>(null);
  const [proposalState, setProposalState] = useState<Awaited<ReturnType<typeof client.invoke<'calendar.propose'>>> | null>(null);
  const apply = useApprovalFlow<'calendar.apply'>();

  const runPropose = async () => {
    if (range === null) {
      return;
    }
    const r = await client.invoke('calendar.propose', { ...range, intent: 'ADD_REVIEWS' });
    setProposalState(r);
    setProposalId(isG9Error(r) ? null : r.result.proposal_id);
    apply.reset();
  };

  const runApply = async () => {
    if (proposalId === null) {
      return;
    }
    await apply.run((approvalId) => client.invoke('calendar.apply', { proposal_id: proposalId }, approvalId === undefined ? {} : { approvalId }));
  };

  const applied = apply.result !== null && !isG9Error(apply.result);

  return (
    <div>
      <h1>Calendar</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      <div className="row" style={{ marginBottom: 8 }}>
        <button
          type="button"
          onClick={() => {
            setOffsetWeeks((w) => w - 1);
          }}
        >
          Previous week
        </button>
        <button
          type="button"
          onClick={() => {
            setOffsetWeeks(0);
          }}
        >
          This week
        </button>
        <button
          type="button"
          onClick={() => {
            setOffsetWeeks((w) => w + 1);
          }}
        >
          Next week
        </button>
        {weekStart !== null ? (
          <span className="muted">
            {formatIsoDate(weekStart)} to {formatIsoDate(addDays(weekStart, 6))} · {today.zone}
          </span>
        ) : null}
      </div>
      {events.error !== null ? <ErrorDisplay error={events.error} onRetry={events.reload} /> : null}
      {weekStart === null || events.data === null ? (
        <Loading what="calendar" />
      ) : (
        <CalendarGrid
          weekStart={weekStart}
          zone={events.data.zone}
          events={events.data.events}
          onActivate={(cell) => {
            setSelection(cell.events.length === 0 ? `${formatIsoDate(cell.date)} ${cell.hour === null ? 'all day' : `${cell.hour}:00`}: no events` : cell.events.map((e) => e.title).join(', '));
          }}
        />
      )}
      <p className="muted" role="status">
        {selection ?? 'Use arrow keys inside the grid; Enter reads the selected cell.'}
      </p>
      {events.data !== null ? (
        events.data.events.length === 0 ? (
          <NoData reason="no events in this week" />
        ) : (
          <p className="muted">
            {events.data.events.length} event(s) this week <StateBadge state="VERIFIED" />
          </p>
        )
      ) : null}

      <Panel title="Schedule proposal (deterministic)">
        <p className="muted">
          Proposals are computed by the scheduler, never by the model. Applying one is a protected operation (SCHEDULE_APPLY) and needs an approval bound to the
          proposal hash.
        </p>
        <div className="row">
          <button
            type="button"
            disabled={range === null || propose.loading}
            onClick={() => {
              void runPropose();
            }}
          >
            Propose review slots for this week
          </button>
          <button
            type="button"
            className="primary"
            disabled={proposalId === null || apply.pending || applied}
            onClick={() => {
              void runApply();
            }}
          >
            Apply proposal
          </button>
        </div>
        {proposalState === null ? null : isG9Error(proposalState) ? (
          <ErrorDisplay error={proposalState} />
        ) : (
          <div className="stack">
            <div>
              Proposal <code>{proposalState.result.proposal_id}</code> <StateBadge state={proposalState.result.feasible ? 'FEASIBLE' : 'INFEASIBLE'} /> · hash{' '}
              <span className="hash">{proposalState.result.parameters_sha256}</span> · expires {formatDateTime(proposalState.result.expires_at, today.zone)}
            </div>
            {proposalState.result.changes.length === 0 ? (
              <NoData reason="no changes proposed (nothing due or all slots exist)" />
            ) : (
              <ul>
                {proposalState.result.changes.map((c) => (
                  <li key={c.event.event_id}>
                    {c.op} {c.event.title} {formatDateTime(c.event.start, today.zone)}–{formatDateTime(c.event.end, today.zone)}
                  </li>
                ))}
              </ul>
            )}
            {proposalState.result.violated_constraints.length > 0 ? (
              <ul>
                {proposalState.result.violated_constraints.map((v) => (
                  <li key={`${v.code}-${v.event_id ?? ''}`}>
                    <code>{v.code}</code> {v.message}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        )}
        {apply.error !== null && apply.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={apply.error} /> : null}
        {apply.approval !== null ? (
          <ApprovalCard
            approval={apply.approval}
            zone={today.zone}
            now={today.now ?? undefined}
            disabled={apply.pending}
            onDecide={(d) => {
              void apply.decide(d);
            }}
            onApply={() => {
              void apply.applyWithApproval().then(() => {
                events.reload();
              });
            }}
          />
        ) : null}
        {applied ? (
          <p role="status">
            Applied {apply.result !== null && !isG9Error(apply.result) ? apply.result.result.applied : 0} change(s); the grid above was reloaded.
          </p>
        ) : null}
      </Panel>
    </div>
  );
}
