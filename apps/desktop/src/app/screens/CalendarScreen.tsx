import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { CalendarEvent, CalendarEventKind, OperatorEventKind } from '../../ipc/operations';
import type { G9Error, Revision } from '../../ipc/types';
import {
  addDays,
  civilToUtc,
  formatIsoDate,
  formatLocalDateTime,
  parseLocalDateTime,
  startOfDayUtc,
  startOfIsoWeek,
  type CivilDate,
} from '../../lib/civil-time';
import { formatDateTime, sentenceCase } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { CalendarGrid } from '../components/CalendarGrid';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

const OPERATOR_KINDS: readonly OperatorEventKind[] = ['STUDY_BLOCK', 'APPOINTMENT', 'REVIEW'];
const WEEK_MS = 7 * 86_400_000;

function isOperatorKind(kind: CalendarEventKind): kind is OperatorEventKind {
  return (OPERATOR_KINDS as readonly string[]).includes(kind);
}

/** What the add/edit form holds: `start`/`end` are `datetime-local` values in the operator's zone. */
interface EventDraft {
  title: string;
  kind: OperatorEventKind;
  start: string;
  end: string;
}

const EMPTY_DRAFT: EventDraft = { title: '', kind: 'STUDY_BLOCK', start: '', end: '' };

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
  // Events the operator adds, changes and deletes (calendar.create_event / update_event / delete_event).
  const [draft, setDraft] = useState<EventDraft>(EMPTY_DRAFT);
  const [editing, setEditing] = useState<CalendarEvent | null>(null);
  // Revisions the backend reported for events changed here. The event view carries none, so an
  // event created elsewhere is sent without `expected_revision` (the backend then checks nothing).
  const [revisions, setRevisions] = useState<Readonly<Record<string, Revision>>>({});
  const [savingEvent, setSavingEvent] = useState(false);
  const [eventError, setEventError] = useState<G9Error | null>(null);
  const [eventWarnings, setEventWarnings] = useState<readonly string[]>([]);
  const [eventStatus, setEventStatus] = useState<string | null>(null);
  const remove = useApprovalFlow<'calendar.delete_event'>();
  const [removing, setRemoving] = useState<CalendarEvent | null>(null);

  const startCivil = parseLocalDateTime(draft.start);
  const endCivil = parseLocalDateTime(draft.end);
  const startAt = startCivil === null ? null : civilToUtc(startCivil, today.zone);
  const endAt = endCivil === null ? null : civilToUtc(endCivil, today.zone);
  const title = draft.title.trim();
  const titleError = draft.title !== '' && title.length === 0 ? 'The title is blank.' : Array.from(title).length > 200 ? 'The title is longer than 200 characters.' : null;
  const rangeError =
    startAt === null || endAt === null ? null : endAt.utc <= startAt.utc ? 'The event must end after it starts.' : endAt.utc - startAt.utc > WEEK_MS ? 'An event lasts at most seven days.' : null;
  const canSave = !savingEvent && today.status !== null && title.length > 0 && titleError === null && startAt !== null && endAt !== null && rangeError === null;
  const ownEvents = (events.data?.events ?? []).filter((e) => e.provenance === 'OPERATOR');
  const deleted = remove.result !== null && !isG9Error(remove.result) ? remove.result.result : null;

  const resetDraft = () => {
    setDraft(EMPTY_DRAFT);
    setEditing(null);
  };

  const saveEvent = async () => {
    if (startAt === null || endAt === null) {
      return;
    }
    setSavingEvent(true);
    const known = editing === null ? undefined : revisions[editing.event_id];
    const r =
      editing === null
        ? await client.invoke('calendar.create_event', { title, kind: draft.kind, start: startAt.utc, end: endAt.utc })
        : await client.invoke(
            'calendar.update_event',
            { event_id: editing.event_id, title, kind: draft.kind, start: startAt.utc, end: endAt.utc },
            known === undefined ? {} : { expectedRevision: known },
          );
    setEventWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setEventError(r);
      setEventStatus(null);
    } else {
      const saved = r.result;
      const revision = r.revision;
      if (revision !== undefined) {
        setRevisions((prev) => ({ ...prev, [saved.event_id]: revision }));
      }
      setEventError(null);
      setEventStatus(`${editing === null ? 'Added' : 'Saved'} “${saved.title}”, ${formatDateTime(saved.start, today.zone)} to ${formatDateTime(saved.end, today.zone)}.`);
      resetDraft();
      events.reload();
    }
    setSavingEvent(false);
  };

  const startEdit = (e: CalendarEvent) => {
    setEditing(e);
    setDraft({
      title: e.title,
      kind: isOperatorKind(e.kind) ? e.kind : 'APPOINTMENT',
      start: formatLocalDateTime(e.start, today.zone),
      end: formatLocalDateTime(e.end, today.zone),
    });
    setEventError(null);
    setEventStatus(null);
  };

  const deleteEvent = async (e: CalendarEvent) => {
    setRemoving(e);
    setEventStatus(null);
    const known = revisions[e.event_id];
    await remove.run((approvalId) =>
      client.invoke(
        'calendar.delete_event',
        { event_id: e.event_id },
        { ...(known === undefined ? {} : { expectedRevision: known }), ...(approvalId === undefined ? {} : { approvalId }) },
      ),
    );
  };

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
      <PageHead title="Calendar" lede="Your week by the hour. Changes to the schedule are proposed first and take effect only when you apply them." />
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
      <Warnings warnings={events.warnings} />
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

      <div className="grid-2">
        <Panel title="Events you added this week">
          <p className="muted">
            Only events you added can be changed or deleted; curriculum blocks, scheduled review slots and protected time stay as they are. Deleting needs an
            approval (DATA_DELETE) bound to the event, confirmed in the application window.
          </p>
          {events.data === null ? null : ownEvents.length === 0 ? (
            <NoData reason="no events you added in this week" />
          ) : (
            <Table
              caption="Events you added"
              headers={['Title', 'Kind', 'Start', 'End', '']}
              rows={ownEvents.map((e) => ({
                key: e.event_id,
                cells: [
                  <span key="title">
                    {e.title} <ProvenanceBadge label="VERIFIED" />
                  </span>,
                  sentenceCase(e.kind),
                  formatDateTime(e.start, today.zone),
                  formatDateTime(e.end, today.zone),
                  <span key="actions" className="row">
                    <button
                      type="button"
                      aria-label={`Edit ${e.title}`}
                      disabled={savingEvent || remove.pending}
                      onClick={() => {
                        startEdit(e);
                      }}
                    >
                      Edit
                    </button>
                    <button
                      type="button"
                      className="danger"
                      aria-label={`Delete ${e.title}`}
                      disabled={savingEvent || remove.pending}
                      onClick={() => {
                        void deleteEvent(e);
                      }}
                    >
                      Delete
                    </button>
                  </span>,
                ],
              }))}
            />
          )}
          {remove.error !== null && remove.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={remove.error} /> : null}
          {remove.cancelled ? <ConfirmationCancelled /> : null}
          {remove.approval !== null && deleted === null ? (
            <ApprovalCard
              approval={remove.approval}
              zone={today.zone}
              now={today.now ?? undefined}
              disabled={remove.pending}
              onDecide={(d) => {
                void remove.decide(d);
              }}
              onApply={() => {
                void remove.applyWithApproval().then(() => {
                  if (editing !== null && editing.event_id === removing?.event_id) {
                    resetDraft();
                  }
                  events.reload();
                });
              }}
            />
          ) : null}
          {deleted !== null ? (
            <p role="status" data-testid="event-deleted">
              Deleted {removing?.title === undefined ? <code>{deleted.event_id}</code> : `“${removing.title}”`}; the approval was used once and is now consumed.
            </p>
          ) : null}
          <Warnings warnings={remove.warnings} />
        </Panel>
        <Panel title={editing === null ? 'Add event' : `Edit “${editing.title}”`}>
          <form
            aria-label={editing === null ? 'Add event' : 'Edit event'}
            onSubmit={(e) => {
              e.preventDefault();
              if (canSave) {
                void saveEvent();
              }
            }}
          >
            <div className="row">
              <label htmlFor="event-title">Title</label>
              <input
                id="event-title"
                type="text"
                required
                maxLength={200}
                value={draft.title}
                aria-invalid={titleError !== null}
                onChange={(e) => {
                  setDraft({ ...draft, title: e.target.value });
                }}
              />
              <label htmlFor="event-kind">Kind</label>
              <select
                id="event-kind"
                value={draft.kind}
                onChange={(e) => {
                  setDraft({ ...draft, kind: e.target.value as OperatorEventKind });
                }}
              >
                {OPERATOR_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {sentenceCase(k)}
                  </option>
                ))}
              </select>
            </div>
            <div className="row">
              <label htmlFor="event-start">Start</label>
              <input
                id="event-start"
                type="datetime-local"
                required
                value={draft.start}
                aria-invalid={draft.start !== '' && startCivil === null}
                onChange={(e) => {
                  setDraft({ ...draft, start: e.target.value });
                }}
              />
              <label htmlFor="event-end">End</label>
              <input
                id="event-end"
                type="datetime-local"
                required
                value={draft.end}
                aria-invalid={rangeError !== null || (draft.end !== '' && endCivil === null)}
                aria-describedby="event-range-hint"
                onChange={(e) => {
                  setDraft({ ...draft, end: e.target.value });
                }}
              />
            </div>
            <p id="event-range-hint" className="field-hint">
              Local times in {today.zone}, the backend&apos;s zone. An event ends after it starts and lasts at most seven days.
              {startAt?.adjusted === 'GAP' || endAt?.adjusted === 'GAP' ? ' A time that does not exist on its day (clocks go forward) is moved past the gap.' : ''}
              {startAt?.adjusted === 'OVERLAP' || endAt?.adjusted === 'OVERLAP' ? ' A time that occurs twice on its day (clocks go back) means the earlier one.' : ''}
            </p>
            {titleError !== null || rangeError !== null ? (
              <p className="muted" role="status" data-testid="event-form-problem">
                {titleError ?? rangeError}
              </p>
            ) : null}
            <div className="row">
              <button type="submit" className="primary" disabled={!canSave}>
                {editing === null ? 'Add event' : 'Save changes'}
              </button>
              {editing !== null ? (
                <button type="button" disabled={savingEvent} onClick={resetDraft}>
                  Cancel editing
                </button>
              ) : null}
            </div>
          </form>
          {eventError !== null ? <ErrorDisplay error={eventError} /> : null}
          {eventStatus !== null ? (
            <p role="status" data-testid="event-saved">
              {eventStatus}
            </p>
          ) : null}
          <Warnings warnings={eventWarnings} />
        </Panel>
      </div>

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
        <Warnings warnings={mergeWarnings(warningsOf(proposalState), apply.warnings)} />
        {apply.error !== null && apply.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={apply.error} /> : null}
        {apply.cancelled ? <ConfirmationCancelled /> : null}
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
