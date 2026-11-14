import '../theme/screens/calendar.css';
import { useState } from 'react';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { CalendarEvent, CalendarEventKind, OperatorEventKind, ProposalChangeOp } from '../../ipc/operations';
import type { G9Error, Revision } from '../../ipc/types';
import {
  addDays,
  civilDateOf,
  civilToUtc,
  formatIsoDate,
  formatLocalDateTime,
  isoWeekday,
  parseIsoDate,
  parseLocalDateTime,
  startOfDayUtc,
  startOfIsoWeek,
  toZoned,
  type CivilDate,
} from '../../lib/civil-time';
import { capabilityLabel } from '../../lib/copy';
import { formatDateTime, humanizeEnum, sentenceCase } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { ProvenanceBadge } from '../components/Badge';
import { CalendarGrid, timeRange } from '../components/CalendarGrid';
import { Composer } from '../components/Composer';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { Instrument, Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

const OPERATOR_KINDS: readonly OperatorEventKind[] = ['STUDY_BLOCK', 'APPOINTMENT', 'REVIEW'];
const WEEK_MS = 7 * 86_400_000;
const WEEKDAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
/** What a proposed change does to the schedule, as the first word of its line. */
const CHANGE_VERB: Readonly<Record<ProposalChangeOp, string>> = { CREATE: 'Add', MOVE: 'Move to', DELETE: 'Remove' };

function isOperatorKind(kind: CalendarEventKind): kind is OperatorEventKind {
  return (OPERATOR_KINDS as readonly string[]).includes(kind);
}

function count(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** What the add/change form holds: `start`/`end` are `datetime-local` values in the operator's zone. */
interface EventDraft {
  title: string;
  kind: OperatorEventKind;
  start: string;
  end: string;
}

const EMPTY_DRAFT: EventDraft = { title: '', kind: 'STUDY_BLOCK', start: '', end: '' };

/** The grid cell the operator activated: its day, its hour (null: the all-day row) and the events it holds. */
interface Picked {
  date: string;
  hour: number | null;
  eventIds: readonly string[];
}

/**
 * The week by the hour. The grid is the record; under it, the events of the day the operator
 * picked in the grid, where the ones they added can be changed or deleted. Adding and changing
 * go through the composer under the page head; deleting an event and applying a schedule
 * proposal are protected operations and raise an approval voucher under the control that asked.
 */
export function CalendarScreen() {
  const today = useToday();
  const client = useIpc();
  const [offsetWeeks, setOffsetWeeks] = useState(0);
  const weekStart: CivilDate | null = today.date === null ? null : addDays(startOfIsoWeek(today.date), offsetWeeks * 7);
  const range = weekStart === null ? null : { from: startOfDayUtc(weekStart, today.zone), to: startOfDayUtc(addDays(weekStart, 7), today.zone) };
  const events = useCommand('calendar.list', range);
  const [picked, setPicked] = useState<Picked | null>(null);
  // The last schedule proposal (calendar.propose) and the week it was asked for.
  const [proposed, setProposed] = useState<{ outcome: InvokeResult<'calendar.propose'>; week: string } | null>(null);
  const [proposing, setProposing] = useState(false);
  const apply = useApprovalFlow<'calendar.apply'>();
  // Events the operator adds, changes and deletes (calendar.create_event / update_event / delete_event).
  const [composing, setComposing] = useState(false);
  const [draft, setDraft] = useState<EventDraft>(EMPTY_DRAFT);
  const [editing, setEditing] = useState<CalendarEvent | null>(null);
  // Revisions the backend reported for events changed here. The event view carries none, so an
  // event created elsewhere is sent without `expected_revision` (the backend then checks nothing).
  const [revisions, setRevisions] = useState<Readonly<Record<string, Revision>>>({});
  const [savingEvent, setSavingEvent] = useState(false);
  const [eventError, setEventError] = useState<G9Error | null>(null);
  const [eventWarnings, setEventWarnings] = useState<readonly string[]>([]);
  const [saved, setSaved] = useState<{ event: CalendarEvent; added: boolean } | null>(null);
  const remove = useApprovalFlow<'calendar.delete_event'>();
  const [removing, setRemoving] = useState<CalendarEvent | null>(null);

  const startCivil = parseLocalDateTime(draft.start);
  const endCivil = parseLocalDateTime(draft.end);
  const startAt = startCivil === null ? null : civilToUtc(startCivil, today.zone);
  const endAt = endCivil === null ? null : civilToUtc(endCivil, today.zone);
  const title = draft.title.trim();
  const titleError = draft.title !== '' && title.length === 0 ? '1 to 200 characters, not only spaces.' : Array.from(title).length > 200 ? 'At most 200 characters.' : null;
  const rangeError =
    startAt === null || endAt === null ? null : endAt.utc <= startAt.utc ? 'The event must end after it starts.' : endAt.utc - startAt.utc > WEEK_MS ? 'An event lasts at most seven days.' : null;
  const canSave = !savingEvent && today.status !== null && title.length > 0 && titleError === null && startAt !== null && endAt !== null && rangeError === null;
  const deleted = remove.result !== null && !isG9Error(remove.result) ? remove.result.result : null;
  const applied = apply.result !== null && !isG9Error(apply.result) ? apply.result.result : null;
  const proposal = proposed !== null && !isG9Error(proposed.outcome) ? proposed.outcome.result : null;
  const canApply = proposal !== null && proposal.feasible && proposal.changes.length > 0 && !apply.pending && applied === null;

  // The week in view and, inside it, the day whose events are listed under the grid: the day of
  // the cell the operator activated, else today, else the Monday of another week (where the
  // grid's roving cell starts too).
  const zone = events.data?.zone ?? today.zone;
  const weekEvents = events.data?.events ?? [];
  const ownCount = weekEvents.filter((e) => e.provenance === 'OPERATOR').length;
  const weekDates = weekStart === null ? [] : Array.from({ length: 7 }, (_, i) => formatIsoDate(addDays(weekStart, i)));
  const weekText = weekStart === null ? null : { from: formatIsoDate(weekStart), to: formatIsoDate(addDays(weekStart, 6)) };
  const pickedHere = picked !== null && weekDates.includes(picked.date) ? picked : null;
  const selectedDate = pickedHere?.date ?? (today.isoDate !== null && weekDates.includes(today.isoDate) ? today.isoDate : (weekDates[0] ?? null));
  const selectedCivil = selectedDate === null ? null : parseIsoDate(selectedDate);
  const dayTitle = selectedCivil === null || selectedDate === null ? '' : `${WEEKDAY_NAMES[isoWeekday(selectedCivil) - 1] ?? ''} ${selectedDate}`;
  const dayFrom = selectedCivil === null ? 0 : startOfDayUtc(selectedCivil, zone);
  const dayTo = selectedCivil === null ? 0 : startOfDayUtc(addDays(selectedCivil, 1), zone);
  const dayEvents = weekEvents.filter((e) => e.start < dayTo && e.end > dayFrom);
  const marked = pickedHere === null ? [] : dayEvents.filter((e) => pickedHere.eventIds.includes(e.event_id));
  const pickedWhere = pickedHere === null ? null : pickedHere.hour === null ? 'All day' : `${String(pickedHere.hour).padStart(2, '0')}:00`;
  const dayStatus = `${dayEvents.length === 0 ? 'Nothing is scheduled on this day.' : `${count(dayEvents.length, 'event', 'events')} on this day.`}${
    pickedWhere === null ? '' : marked.length === 0 ? ` ${pickedWhere}: nothing starts here.` : ` ${pickedWhere}: ${marked.length} marked below.`
  }`;
  // A row's buttons send a revision: they wait while a change is in flight and while the week reloads.
  const rowsLocked = savingEvent || remove.pending || events.loading;
  const removalShown = (remove.error !== null && remove.error.category !== 'APPROVAL_REQUIRED') || remove.cancelled || remove.approval !== null || deleted !== null;
  const savedInWeek = saved !== null && weekDates.includes(formatIsoDate(civilDateOf(saved.event.start, zone)));

  const closeComposer = () => {
    setComposing(false);
    setEditing(null);
    setDraft(EMPTY_DRAFT);
    setEventError(null);
  };

  const openAdd = () => {
    setEditing(null);
    setDraft(EMPTY_DRAFT);
    setEventError(null);
    setComposing(true);
  };

  const saveEvent = async () => {
    if (startAt === null || endAt === null) {
      return;
    }
    setSavingEvent(true);
    const target = editing;
    const known = target === null ? undefined : revisions[target.event_id];
    const r =
      target === null
        ? await client.invoke('calendar.create_event', { title, kind: draft.kind, start: startAt.utc, end: endAt.utc })
        : await client.invoke(
            'calendar.update_event',
            { event_id: target.event_id, title, kind: draft.kind, start: startAt.utc, end: endAt.utc },
            known === undefined ? {} : { expectedRevision: known },
          );
    setEventWarnings(warningsOf(r));
    if (isG9Error(r)) {
      // Refused: the composer stays open with what was typed; the reason is shown above it.
      setEventError(r);
      setSaved(null);
    } else {
      const event = r.result;
      const revision = r.revision;
      if (revision !== undefined) {
        setRevisions((prev) => ({ ...prev, [event.event_id]: revision }));
      }
      setSaved({ event, added: target === null });
      // The saved event is the one marked under the grid, when its day is in the week shown.
      const day = formatIsoDate(civilDateOf(event.start, zone));
      if (weekDates.includes(day)) {
        setPicked({ date: day, hour: toZoned(event.start, zone).hour, eventIds: [event.event_id] });
      }
      closeComposer();
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
    setSaved(null);
    setComposing(true);
  };

  const deleteEvent = async (e: CalendarEvent) => {
    setRemoving(e);
    setSaved(null);
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
    if (range === null || weekText === null) {
      return;
    }
    setProposing(true);
    const r = await client.invoke('calendar.propose', { ...range, intent: 'ADD_REVIEWS' });
    setProposed({ outcome: r, week: `${weekText.from} to ${weekText.to}` });
    apply.reset();
    setProposing(false);
  };

  const runApply = async () => {
    if (proposal === null) {
      return;
    }
    const proposalId = proposal.proposal_id;
    await apply.run((approvalId) => client.invoke('calendar.apply', { proposal_id: proposalId }, approvalId === undefined ? {} : { approvalId }));
  };

  const closeProposal = () => {
    setProposed(null);
    apply.reset();
  };

  const formTitle = editing === null ? 'Add an event' : 'Change the event';

  return (
    <div className="stack">
      <PageHead
        title="Calendar"
        lede="Your week by the hour. A change to the schedule is proposed first and takes effect only when you apply it."
        actions={
          <button type="button" className="primary" disabled={composing && editing === null} onClick={openAdd}>
            Add an event
          </button>
        }
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {events.error !== null ? <ErrorDisplay error={events.error} onRetry={events.reload} /> : null}
      {eventError !== null ? <ErrorDisplay error={eventError} /> : null}
      <Warnings warnings={mergeWarnings(events.warnings, eventWarnings, remove.warnings, warningsOf(proposed?.outcome), apply.warnings)} />
      {saved !== null && !composing ? (
        <p className="receipt" role="status" data-testid="event-saved">
          {saved.added ? 'Event added' : 'Changes saved'}: “{saved.event.title}”, {formatDateTime(saved.event.start, today.zone)} to {formatDateTime(saved.event.end, today.zone)}.{' '}
          {savedInWeek ? 'It is marked in the list under the grid.' : 'It is outside the week shown.'}
        </p>
      ) : null}

      {composing ? (
        <Composer key={editing?.event_id ?? 'new'} title={formTitle} onClose={closeComposer}>
          <form
            aria-label={formTitle}
            onSubmit={(e) => {
              e.preventDefault();
              if (canSave) {
                void saveEvent();
              }
            }}
          >
            <div className="form-grid">
              <Field label="Title" hint="1 to 200 characters." error={titleError}>
                {({ id, describedBy, invalid }) => (
                  <input
                    id={id}
                    type="text"
                    required
                    maxLength={200}
                    value={draft.title}
                    aria-invalid={invalid}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDraft({ ...draft, title: e.target.value });
                    }}
                  />
                )}
              </Field>
              <Field label="Kind">
                {({ id }) => (
                  <select
                    id={id}
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
                )}
              </Field>
              <Field
                label="Start"
                hint={
                  <>
                    Local time in {today.zone}.{startAt?.adjusted === 'GAP' ? ' This time does not exist on that day (clocks go forward); it is moved past the gap.' : ''}
                    {startAt?.adjusted === 'OVERLAP' ? ' This time occurs twice on that day (clocks go back); the earlier one is used.' : ''}
                  </>
                }
              >
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="datetime-local"
                    required
                    value={draft.start}
                    aria-invalid={draft.start !== '' && startCivil === null}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDraft({ ...draft, start: e.target.value });
                    }}
                  />
                )}
              </Field>
              <Field
                label="End"
                hint={
                  <>
                    After the start, at most seven days later.{endAt?.adjusted === 'GAP' ? ' This time does not exist on that day (clocks go forward); it is moved past the gap.' : ''}
                    {endAt?.adjusted === 'OVERLAP' ? ' This time occurs twice on that day (clocks go back); the earlier one is used.' : ''}
                  </>
                }
                error={rangeError}
              >
                {({ id, describedBy, invalid }) => (
                  <input
                    id={id}
                    type="datetime-local"
                    required
                    value={draft.end}
                    aria-invalid={invalid || (draft.end !== '' && endCivil === null)}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setDraft({ ...draft, end: e.target.value });
                    }}
                  />
                )}
              </Field>
            </div>
            <div className="form-actions">
              <button type="submit" className="primary" disabled={!canSave}>
                {editing === null ? 'Add event' : 'Save changes'}
              </button>
              <button type="button" disabled={savingEvent} onClick={closeComposer}>
                Cancel
              </button>
              <span className="form-note">Only events you add can be changed or deleted later. Deleting one needs your approval.</span>
            </div>
          </form>
        </Composer>
      ) : null}

      <div className="stack-sm">
        <div className="toolbar cal-toolbar" role="group" aria-label="Week">
          <div className="seg" role="group" aria-label="Week shown">
            <button
              type="button"
              disabled={weekStart === null}
              onClick={() => {
                setOffsetWeeks((w) => w - 1);
              }}
            >
              Previous week
            </button>
            <button
              type="button"
              aria-pressed={offsetWeeks === 0}
              disabled={weekStart === null}
              onClick={() => {
                setOffsetWeeks(0);
              }}
            >
              This week
            </button>
            <button
              type="button"
              disabled={weekStart === null}
              onClick={() => {
                setOffsetWeeks((w) => w + 1);
              }}
            >
              Next week
            </button>
          </div>
          {weekText !== null ? (
            <p className="muted" data-testid="week-range">
              <time dateTime={weekText.from}>{weekText.from}</time> to <time dateTime={weekText.to}>{weekText.to}</time>, times in {zone}
            </p>
          ) : null}
          <button
            type="button"
            className="cal-toolbar-end"
            title="Ask the scheduler for review slots in this week. Nothing changes until you apply the proposal."
            disabled={range === null || proposing}
            onClick={() => {
              void runPropose();
            }}
          >
            Propose review slots
          </button>
        </div>

        {proposed === null ? null : isG9Error(proposed.outcome) ? (
          <ErrorDisplay error={proposed.outcome} />
        ) : proposal === null ? null : (
          <>
            <Instrument
              title="Proposed review slots"
              label="Schedule proposal"
              actions={
                <div className="row">
                  <ProvenanceBadge label="PROPOSED" />
                  <button type="button" className="quiet sm" onClick={closeProposal}>
                    Close
                  </button>
                </div>
              }
            >
              <p className="small muted">
                For {proposed.week}. Worked out by the scheduler from the reviews that are due, never by a model. Nothing in the calendar changes until you apply it.
              </p>
              {proposal.feasible ? (
                <p data-testid="proposal-feasibility">
                  <strong>Feasible.</strong> No constraint is violated.
                </p>
              ) : (
                <div className="stack-sm">
                  <p data-testid="proposal-feasibility">
                    <strong>Not feasible.</strong> {count(proposal.violated_constraints.length, 'constraint is', 'constraints are')} violated, so this proposal cannot be applied. Change the
                    week, then propose again.
                  </p>
                  <ul className="rows" aria-label="Violated constraints">
                    {proposal.violated_constraints.map((v, i) => (
                      <li key={`${v.code}-${v.event_id ?? ''}-${i}`}>
                        <div className="row-main">
                          <span className="row-sub" data-standing="owed">
                            {v.message}
                          </span>
                        </div>
                        <div className="row-end">
                          <code>{v.code}</code>
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {proposal.changes.length === 0 ? (
                <p className="muted">No change is proposed: nothing is due for review, or every slot already exists.</p>
              ) : (
                <div className="stack-sm">
                  <h3>{proposal.changes.length === 1 ? 'The one change' : `The ${proposal.changes.length} changes`}</h3>
                  <ul className="rows" aria-label="Proposed changes">
                    {proposal.changes.map((c) => (
                      <li key={c.event.event_id}>
                        <div className="row-main">
                          <span className="row-title">{c.event.title}</span>
                          <span className="row-sub">
                            {CHANGE_VERB[c.op]}: {formatDateTime(c.event.start, today.zone)} to {formatDateTime(c.event.end, today.zone)}, {humanizeEnum(c.event.kind)}
                          </span>
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              <dl className="pairs">
                <dt title="Keyed fingerprint (HMAC-SHA-256 under this installation’s key) of the exact changes of this proposal. The approval covers this proposal and no other.">Fingerprint</dt>
                <dd>
                  <span className="hash" data-testid="proposal-hash">
                    {proposal.parameters_sha256}
                  </span>
                </dd>
                <dt>Expires</dt>
                <dd>
                  <time dateTime={new Date(proposal.expires_at).toISOString()}>{formatDateTime(proposal.expires_at, today.zone)}</time>
                </dd>
                <dt>Proposal</dt>
                <dd>
                  <code>{proposal.proposal_id}</code>
                </dd>
              </dl>
              <div className="form-actions">
                <button
                  type="button"
                  className="primary"
                  disabled={!canApply}
                  onClick={() => {
                    void runApply();
                  }}
                >
                  Apply proposal
                </button>
                <span className="form-note">
                  Applying is a protected operation: it needs your approval (to {capabilityLabel('SCHEDULE_APPLY')}), which covers this fingerprint and nothing else.
                </span>
              </div>
            </Instrument>
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
            {apply.cancelled ? <ConfirmationCancelled /> : null}
            {applied !== null ? (
              <p className="receipt" role="status" data-testid="proposal-applied">
                Proposal applied: {count(applied.applied, 'change', 'changes')}. The grid shows the week as it stands now.
              </p>
            ) : null}
          </>
        )}

        {weekStart === null || weekText === null ? (
          today.error !== null ? (
            <NoData reason={`The backend clock could not be read (${today.error.code}), so the week cannot be worked out.`} />
          ) : (
            <Loading what="the week" />
          )
        ) : events.data === null ? (
          events.error !== null ? (
            <NoData reason={`The events of this week could not be read (${events.error.code}).`} />
          ) : (
            <Loading what="the week" />
          )
        ) : weekEvents.length === 0 ? (
          <NoData
            title="No events this week"
            reason={`Nothing is scheduled from ${weekText.from} to ${weekText.to}. A day’s study blocks appear here when you plan that day (Daily Classroom or Mission Control), review slots when you apply a proposal, and your own events when you add them.`}
            action={
              <button type="button" disabled={composing && editing === null} onClick={openAdd}>
                Add an event
              </button>
            }
          />
        ) : (
          <>
            <CalendarGrid
              key={weekText.from}
              weekStart={weekStart}
              zone={events.data.zone}
              events={events.data.events}
              today={today.date}
              onActivate={(cell) => {
                setPicked({ date: formatIsoDate(cell.date), hour: cell.hour, eventIds: cell.events.map((e) => e.event_id) });
              }}
            />
            <p className="small muted" data-testid="week-summary">
              {count(weekEvents.length, 'event', 'events')} this week, {ownCount === 0 ? 'none of them added by you' : `${ownCount} added by you`}. Arrow keys move through the grid; Enter, Space or
              a click lists that day below.
            </p>
          </>
        )}
      </div>

      {events.data !== null && selectedCivil !== null && (weekEvents.length > 0 || removalShown) ? (
        // Calendar events are backend records: stamped Verified unless the read came with a warning.
        <Panel
          title={dayTitle}
          actions={dayEvents.length > 0 && events.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : undefined}
          note={
            <span role="status" data-testid="day-status">
              {dayStatus}
            </span>
          }
        >
          <div className="stack-sm">
            {dayEvents.length > 0 ? (
              <ul className="rows" aria-label={`Events on ${dayTitle}`}>
                {dayEvents.map((e) => {
                  const own = e.provenance === 'OPERATOR';
                  return (
                    <li key={e.event_id} data-selected={marked.includes(e) ? 'true' : undefined}>
                      <div className="row-main">
                        <span className="row-title">{e.title}</span>
                        <span className="row-sub">
                          {e.all_day ? 'All day' : timeRange(e, selectedCivil, zone)}, {humanizeEnum(e.kind)}
                          {own ? ', added by you' : ''}
                        </span>
                      </div>
                      {own ? (
                        <div className="row-end">
                          <button
                            type="button"
                            className="sm"
                            aria-label={`Change ${e.title}`}
                            disabled={rowsLocked}
                            onClick={() => {
                              startEdit(e);
                            }}
                          >
                            Change
                          </button>
                          <button
                            type="button"
                            className="sm danger"
                            aria-label={`Delete ${e.title}`}
                            disabled={rowsLocked}
                            onClick={() => {
                              void deleteEvent(e);
                            }}
                          >
                            Delete
                          </button>
                        </div>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            ) : null}
            {remove.error !== null && remove.error.category !== 'APPROVAL_REQUIRED' ? <ErrorDisplay error={remove.error} /> : null}
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
                      closeComposer();
                    }
                    events.reload();
                  });
                }}
              />
            ) : null}
            {remove.cancelled ? <ConfirmationCancelled /> : null}
            {deleted !== null ? (
              <p className="receipt" role="status" data-testid="event-deleted">
                Event deleted: {removing?.title === undefined ? <code>{deleted.event_id}</code> : `“${removing.title}”`}. The approval was used once and cannot be used again.
              </p>
            ) : null}
            <p className="field-hint">
              Only events you added can be changed or deleted. Curriculum blocks, scheduled review slots and protected time stay as they are. Deleting an event is a protected operation: it
              needs your approval (to {capabilityLabel('DATA_DELETE')}), which covers that one event and is confirmed in a window drawn by the application.
            </p>
          </div>
        </Panel>
      ) : null}
    </div>
  );
}
