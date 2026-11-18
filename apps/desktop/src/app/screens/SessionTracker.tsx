import { useEffect, useId, useRef, useState } from 'react';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { DailyPlan, PlannedSessionMode, RequestOf, SessionView, TimeCorrection, TimeInterval } from '../../ipc/operations';
import type { G9Error, IanaZone, SessionEvent } from '../../ipc/types';
import { civilToUtc, parseLocalDateTime } from '../../lib/civil-time';
import { formatDateTime, formatDuration, formatMinutes, formatTime, sessionBlockName } from '../../lib/format';
import { dayNumber } from '../../lib/program';
import { SESSION_MACHINE, can, type SESSION_OPERATION_EVENTS } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { IntervalStrip, Legend } from '../components/charts';
import { Composer } from '../components/Composer';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { Meter } from '../components/Meter';
import { Instrument, Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

type SessionOp = keyof typeof SESSION_OPERATION_EVENTS;
type TransitionOp = Exclude<SessionOp, 'session.correct_time'>;

const CONTROLS: readonly { op: TransitionOp; event: SessionEvent; label: string }[] = [
  { op: 'session.prepare', event: 'PREPARE', label: 'Prepare' },
  { op: 'session.start', event: 'START', label: 'Start' },
  { op: 'session.pause', event: 'PAUSE', label: 'Pause' },
  { op: 'session.resume', event: 'RESUME', label: 'Resume' },
  { op: 'session.complete', event: 'COMPLETE', label: 'Complete' },
];

/** The step a session most likely takes next, by state: the one button drawn as the primary action. */
const NEXT_STEP: Readonly<Partial<Record<SessionView['state'], SessionEvent>>> = {
  PLANNED: 'PREPARE',
  READY: 'START',
  RUNNING: 'COMPLETE',
  PAUSED: 'RESUME',
};

/** The latest correction per interval: corrections arrive in recording order and the backend credits the last one (timer.rs `latest_correction`). */
function latestCorrections(session: SessionView): Map<string, TimeCorrection> {
  const latest = new Map<string, TimeCorrection>();
  for (const c of session.corrections) {
    latest.set(c.interval_id, c);
  }
  return latest;
}

/**
 * Closed UNCERTAIN intervals that no correction references yet. These are the intervals whose
 * time is still uncredited; the backend reports `requires_correction` while any exists.
 */
export function uncorrectedIntervals(session: SessionView): TimeInterval[] {
  const corrected = latestCorrections(session);
  return session.intervals.filter((i) => i.continuity === 'UNCERTAIN' && i.end_utc !== null && !corrected.has(i.interval_id));
}

export interface Correctable {
  interval: TimeInterval;
  /** The correction currently credited for this interval, or null when it has none. */
  latest: TimeCorrection | null;
}

/**
 * Every closed UNCERTAIN interval, each with the correction currently in force. The backend
 * accepts a further correction for an already-corrected interval and credits the latest one
 * (`Ledger::correct`, test `latest_correction_wins`); the earlier rows are kept, so a typo
 * can be superseded from here instead of being made permanent.
 */
export function correctableIntervals(session: SessionView): Correctable[] {
  const corrected = latestCorrections(session);
  return session.intervals
    .filter((i) => i.continuity === 'UNCERTAIN' && i.end_utc !== null)
    .map((interval) => ({ interval, latest: corrected.get(interval.interval_id) ?? null }));
}

function intervalLength(i: TimeInterval): number {
  return i.end_utc === null ? 0 : Math.max(0, i.end_utc - i.start_utc);
}

/** `09:41 to 10:01 (0:20:00)`: an interval as the operator reads it; its id stays in a `title`. */
function intervalSpan(i: TimeInterval, zone: IanaZone): string {
  return `${formatDateTime(i.start_utc, zone)} to ${i.end_utc === null ? 'now' : formatTime(i.end_utc, zone)} (${formatDuration(intervalLength(i))})`;
}

const PLAN_MODES: readonly { mode: PlannedSessionMode; label: string }[] = [
  { mode: 'STUDY', label: 'Study' },
  { mode: 'LAB', label: 'Lab' },
  { mode: 'REVIEW', label: 'Review' },
];
const OBJECTIVE_CODE = /^[A-Za-z0-9._:-]{1,128}$/;

/**
 * A session's name: its curriculum block, or "Your session" for one you planned yourself. The
 * backend labels those `UNLINKED` (`session_view`); they run like any other session. A session
 * carries only the block code: the name the curriculum's profile gives the block comes from the
 * day plan that lists the session (`plan`), and a session of another day is named from its code.
 */
export function blockLabel(session: SessionView, plan: DailyPlan | null = null): string {
  return session.block === 'UNLINKED' ? 'Your session' : sessionBlockName(session, plan);
}

function ownSession(session: SessionView): boolean {
  return dayNumber(session.day_id) === null;
}

/** Where a session belongs: its program day, or that you planned it yourself. */
function dayLine(session: SessionView): string {
  const n = dayNumber(session.day_id);
  return n === null ? 'Planned by you, not tied to a curriculum block' : `Day ${n} of the program, block ${session.block_seq}`;
}

function rowSub(session: SessionView): string {
  const parts = [`${formatMinutes(Math.round(session.planned_ms / 60_000))} planned`];
  if (session.credited_ms > 0) {
    parts.push(`${formatDuration(session.credited_ms)} credited`);
  }
  if (session.uncertain_ms > 0) {
    parts.push(`${formatDuration(session.uncertain_ms)} in doubt`);
  }
  return parts.join(', ');
}

export function SessionTracker() {
  const today = useToday();
  const client = useIpc();
  const sessions = useCommand('session.list', {});
  // Only to know which program day today is, so today's blocks are listed first.
  const dayPlan = useCommand('curriculum.daily_plan', today.isoDate === null ? null : { date: today.isoDate });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  // The SessionView returned by the last mutation. `session.list` is revalidated after every
  // mutation; until it resolves this is the newest state and revision the backend has told us.
  const [fresh, setFresh] = useState<SessionView | null>(null);
  const [creditedMinutes, setCreditedMinutes] = useState('0');
  const [reason, setReason] = useState('');
  // The interval to correct is an explicit operator choice, scoped to one session.
  const [intervalChoice, setIntervalChoice] = useState<{ sessionId: string; intervalId: string } | null>(null);
  const [confirmComplete, setConfirmComplete] = useState<string | null>(null);
  const [actionWarnings, setActionWarnings] = useState<readonly string[]>([]);
  const keepOpenRef = useRef<HTMLButtonElement>(null);
  const dialogId = useId();
  // "Plan a session" (session.create), opened from the page head.
  const [composing, setComposing] = useState(false);
  const [planMinutes, setPlanMinutes] = useState('45');
  const [planStart, setPlanStart] = useState('');
  const [planMode, setPlanMode] = useState<PlannedSessionMode>('STUDY');
  const [planObjective, setPlanObjective] = useState('');
  const [planned, setPlanned] = useState<SessionView | null>(null);

  const listed = sessions.data?.sessions ?? [];
  const list = fresh === null ? listed : listed.map((s) => (s.session_id === fresh.session_id && fresh.revision > s.revision ? fresh : s));
  const interrupted = list.filter((s) => s.state === 'INTERRUPTED');
  const todayId = dayPlan.data?.day_id ?? null;

  // The list, in the order the operator needs it: what needs a correction, today, own sessions, other days.
  const needsYou = list.filter((s) => s.state === 'INTERRUPTED' || s.requires_correction);
  const needsIds = new Set(needsYou.map((s) => s.session_id));
  const rest = list.filter((s) => !needsIds.has(s.session_id));
  const todays = todayId === null ? [] : rest.filter((s) => s.day_id === todayId);
  const own = rest.filter(ownSession);
  const others = rest.filter((s) => !ownSession(s) && s.day_id !== todayId);
  const otherDays = [...new Set(others.map((s) => s.day_id))];

  const selected: SessionView | undefined =
    list.find((s) => s.session_id === selectedId) ??
    interrupted[0] ??
    list.find((s) => s.state === 'RUNNING' || s.state === 'PAUSED') ??
    todays.find((s) => s.state === 'READY' || s.state === 'PLANNED') ??
    todays[0] ??
    list[0];
  const uncorrected = selected === undefined ? [] : uncorrectedIntervals(selected);
  const correctable = selected === undefined ? [] : correctableIntervals(selected);
  const chosen =
    selected !== undefined && intervalChoice !== null && intervalChoice.sessionId === selected.session_id
      ? correctable.find((c) => c.interval.interval_id === intervalChoice.intervalId)
      : undefined;
  // The backend says a correction is outstanding (INTERRUPTED or an uncorrected closed UNCERTAIN interval, in any state).
  const outstanding = selected !== undefined && (selected.state === 'INTERRUPTED' || selected.requires_correction || uncorrected.length > 0);
  // Superseding an earlier correction is offered whenever a closed UNCERTAIN interval exists.
  const showCorrection = outstanding || correctable.length > 0;
  const confirming = confirmComplete !== null && confirmComplete === selected?.session_id && uncorrected.length > 0;
  // Every control stays disabled while a mutation is in flight AND while the list revalidates:
  // a click in that window would carry the pre-mutation revision (STALE_REVISION).
  const locked = busy || sessions.loading;
  // The plan form checks what the request schema checks; the backend decides the rest.
  const minutes = /^\d{1,3}$/.test(planMinutes.trim()) ? Number(planMinutes.trim()) : Number.NaN;
  const minutesValid = minutes >= 1 && minutes <= 600;
  const startCivil = planStart === '' ? null : parseLocalDateTime(planStart);
  const startResolved = startCivil === null ? null : civilToUtc(startCivil, today.zone);
  const objective = planObjective.trim();
  const objectiveValid = objective === '' || OBJECTIVE_CODE.test(objective);
  const canPlan = !busy && today.status !== null && minutesValid && (planStart === '' || startCivil !== null) && objectiveValid;

  useEffect(() => {
    if (confirming) {
      keepOpenRef.current?.focus();
    }
  }, [confirming]);

  const applyOutcome = (r: InvokeResult<SessionOp>) => {
    setError(isG9Error(r) ? r : null);
    setActionWarnings(warningsOf(r));
    if (!isG9Error(r)) {
      setFresh(r.result);
    }
    setBusy(false);
    sessions.reload();
  };

  const send = async (op: TransitionOp, session: SessionView) => {
    setConfirmComplete(null);
    setBusy(true);
    applyOutcome(await client.invoke(op, { session_id: session.session_id }, { expectedRevision: session.revision }));
  };

  const requestTransition = (op: TransitionOp, session: SessionView) => {
    // COMPLETE is a legal edge from INTERRUPTED, but it leaves uncorrected UNCERTAIN time
    // uncredited until someone comes back to correct it: ask first. Only the dialog's own
    // button sends; pressing Complete again only re-opens the dialog.
    if (op === 'session.complete' && uncorrectedIntervals(session).length > 0) {
      setConfirmComplete(session.session_id);
      return;
    }
    void send(op, session);
  };

  const plan = async () => {
    const payload: RequestOf<'session.create'> = {
      planned_minutes: minutes,
      mode: planMode,
      ...(startResolved === null ? {} : { planned_start: startResolved.utc }),
      ...(objective === '' ? {} : { objective_id: objective }),
    };
    setBusy(true);
    const r = await client.invoke('session.create', payload);
    setError(isG9Error(r) ? r : null);
    setActionWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setPlanned(null);
    } else {
      setPlanned(r.result);
      setSelectedId(r.result.session_id);
      setConfirmComplete(null);
      setComposing(false);
    }
    setBusy(false);
    sessions.reload();
  };

  const correct = async (session: SessionView, interval: TimeInterval) => {
    setBusy(true);
    const r = await client.invoke(
      'session.correct_time',
      { session_id: session.session_id, interval_id: interval.interval_id, credited_ms: Math.max(0, Math.round(Number(creditedMinutes) * 60_000)), reason },
      { expectedRevision: session.revision },
    );
    if (!isG9Error(r)) {
      setIntervalChoice(null);
    }
    applyOutcome(r);
  };

  const pick = (s: SessionView) => {
    setSelectedId(s.session_id);
    setError(null);
    setConfirmComplete(null);
    setActionWarnings([]);
  };

  const row = (s: SessionView, withDay: boolean) => {
    const isSelected = selected?.session_id === s.session_id;
    return (
      <li key={s.session_id} data-selected={isSelected}>
        <button
          type="button"
          className="row-pick"
          aria-pressed={isSelected}
          onClick={() => {
            pick(s);
          }}
        >
          <span className="row-main">
            <span className="row-title">
              {withDay && !ownSession(s) ? <code>{s.day_id}</code> : null} {blockLabel(s, dayPlan.data)}
            </span>
            <span className="row-sub">{rowSub(s)}</span>
          </span>
          <span className="row-end">
            <StateBadge state={s.state} />
          </span>
        </button>
      </li>
    );
  };

  const nextStep = selected === undefined ? undefined : NEXT_STEP[selected.state];

  return (
    <div className="stack">
      <PageHead
        title="Session Tracker"
        lede="Run your study blocks. The backend keeps the time; time it cannot vouch for waits for your correction."
        actions={
          <button
            type="button"
            className="primary"
            disabled={composing}
            onClick={() => {
              setComposing(true);
            }}
          >
            Plan a session
          </button>
        }
      />
      {interrupted.length > 0 ? (
        <div className="banner" role="alert" data-testid="interrupted-banner">
          <strong>{interrupted.length === 1 ? 'One session was interrupted.' : `${interrupted.length} sessions were interrupted.`}</strong> The machine slept, restarted, or the
          application closed while the timer ran. That time is not credited until you correct it below. The original record is kept; a correction is added beside it.
        </div>
      ) : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(sessions.warnings, dayPlan.warnings, actionWarnings)} />
      {planned !== null && !composing ? (
        <p className="receipt" role="status" data-testid="planned-session">
          Session planned: {formatDuration(planned.planned_ms)}, <StateBadge state={planned.state} />. It is selected below.
        </p>
      ) : null}

      {composing ? (
        <Composer
          title="Plan a session"
          onClose={() => {
            setComposing(false);
          }}
        >
          <form
            aria-label="Plan a session"
            onSubmit={(e) => {
              e.preventDefault();
              if (canPlan) {
                void plan();
              }
            }}
          >
            <div className="form-grid">
              <Field label="Length in minutes" hint="1 to 600.">
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="number"
                    inputMode="numeric"
                    min={1}
                    max={600}
                    step={1}
                    required
                    value={planMinutes}
                    aria-invalid={planMinutes !== '' && !minutesValid}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setPlanMinutes(e.target.value);
                    }}
                  />
                )}
              </Field>
              <Field
                label="Start (optional)"
                hint={
                  <>
                    Local time in {today.zone}. Leave it empty to plan from now.
                    {startResolved?.adjusted === 'GAP' ? ' This time does not exist on that day (clocks go forward); it was moved past the gap.' : ''}
                    {startResolved?.adjusted === 'OVERLAP' ? ' This time occurs twice on that day (clocks go back); the earlier one is used.' : ''}
                  </>
                }
              >
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="datetime-local"
                    value={planStart}
                    aria-invalid={planStart !== '' && startCivil === null}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setPlanStart(e.target.value);
                    }}
                  />
                )}
              </Field>
              <Field label="Kind">
                {({ id }) => (
                  <select
                    id={id}
                    value={planMode}
                    onChange={(e) => {
                      setPlanMode(e.target.value as PlannedSessionMode);
                    }}
                  >
                    {PLAN_MODES.map((m) => (
                      <option key={m.mode} value={m.mode}>
                        {m.label}
                      </option>
                    ))}
                  </select>
                )}
              </Field>
              <Field label="Objective code (optional)" hint="Checked against the imported curriculum, for example D15-O1.">
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="text"
                    value={planObjective}
                    aria-invalid={!objectiveValid}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setPlanObjective(e.target.value);
                    }}
                  />
                )}
              </Field>
            </div>
            <div className="form-actions">
              <button type="submit" className="primary" disabled={!canPlan}>
                Plan session
              </button>
              <button
                type="button"
                onClick={() => {
                  setComposing(false);
                }}
              >
                Cancel
              </button>
              <span className="form-note">A session you plan yourself is not tied to a curriculum block. It runs like any other.</span>
            </div>
          </form>
        </Composer>
      ) : null}

      <div className="split" data-lead="end">
        {/* Sessions are backend records: stamped Verified unless the read came with a warning. */}
        <Panel title="Sessions" actions={sessions.data !== null && sessions.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : undefined}>
          {sessions.loading && sessions.data === null ? <Loading what="sessions" /> : null}
          {list.length === 0 && !sessions.loading ? (
            <NoData title="No sessions yet" reason="Sessions appear here when a curriculum day is planned or when you plan one yourself." />
          ) : null}
          {needsYou.length > 0 ? (
            <section className="stack-sm" aria-label="Sessions that need a correction">
              <h3>Needs your correction</h3>
              <ul className="rows">{needsYou.map((s) => row(s, true))}</ul>
            </section>
          ) : null}
          {todays.length > 0 ? (
            <section className="stack-sm" aria-label="Today’s sessions">
              <h3>
                Today, <code>{todayId}</code>
              </h3>
              <ul className="rows">{todays.map((s) => row(s, false))}</ul>
            </section>
          ) : null}
          {own.length > 0 ? (
            <section className="stack-sm" aria-label="Sessions you planned">
              <h3>Planned by you</h3>
              <ul className="rows">{own.map((s) => row(s, false))}</ul>
            </section>
          ) : null}
          {otherDays.length > 0 ? (
            <section className="stack-sm" aria-label="Sessions of other days">
              <h3>{todays.length > 0 ? 'Other days' : 'By day'}</h3>
              {otherDays.map((day) => {
                const ofDay = others.filter((s) => s.day_id === day);
                const creditedOfDay = ofDay.reduce((sum, s) => sum + s.credited_ms, 0);
                return (
                  <details key={day} className="day-group" open={ofDay.some((s) => s.session_id === selected?.session_id)}>
                    <summary>
                      <code>{day}</code>{' '}
                      <span className="muted">
                        {ofDay.length} {ofDay.length === 1 ? 'session' : 'sessions'}, {formatDuration(creditedOfDay)} credited
                      </span>
                    </summary>
                    <ul className="rows">{ofDay.map((s) => row(s, false))}</ul>
                  </details>
                );
              })}
            </section>
          ) : null}
        </Panel>

        <div className="sticky stack">
          {selected === undefined ? (
            <Panel title="Session">
              <NoData title="No session selected" reason="Choose one in the list, or plan one." />
            </Panel>
          ) : (
            <>
              <Instrument title={blockLabel(selected, dayPlan.data)} actions={<StateBadge state={selected.state} />} label={`Session: ${blockLabel(selected, dayPlan.data)}`}>
                <p className="muted small">{dayLine(selected)}</p>
                <div className="timesheet">
                  <p className="timesheet-figure">
                    <span className="kpi-xl" data-testid="credited">
                      {formatDuration(selected.credited_ms)}
                    </span>
                    <span className="muted">credited of {formatMinutes(Math.round(selected.planned_ms / 60_000))} planned</span>
                  </p>
                  <Meter
                    posted={selected.credited_ms}
                    doubted={selected.uncertain_ms}
                    of={selected.planned_ms}
                    owed={selected.requires_correction}
                    label={`${formatDuration(selected.credited_ms)} credited and ${formatDuration(selected.uncertain_ms)} in doubt of ${formatDuration(selected.planned_ms)} planned`}
                  />
                </div>
                <div className="toolbar transitions" role="group" aria-label="Session transitions">
                  {CONTROLS.map((c) => (
                    <button
                      key={c.op}
                      type="button"
                      className={c.event === nextStep ? 'primary' : undefined}
                      disabled={locked || (c.op === 'session.complete' && confirming) || !can(SESSION_MACHINE, selected.state, c.event)}
                      onClick={() => {
                        requestTransition(c.op, selected);
                      }}
                    >
                      {c.label}
                    </button>
                  ))}
                </div>
                {confirming ? (
                  <div className="banner" role="alertdialog" aria-labelledby={`${dialogId}-title`} aria-describedby={`${dialogId}-desc`} data-testid="complete-warning">
                    <strong id={`${dialogId}-title`}>Complete with time still in doubt?</strong>
                    <p id={`${dialogId}-desc`}>
                      {uncorrected.length === 1 ? 'One interval has' : `${uncorrected.length} intervals have`} no correction:{' '}
                      {uncorrected.map((i) => intervalSpan(i, today.zone)).join('; ')}. If you complete now, that time stays in doubt and uncredited until you record a
                      correction. You can still record one after completing. Credit is never assumed.
                    </p>
                    <div className="row">
                      <button
                        type="button"
                        className="danger"
                        disabled={locked}
                        onClick={() => {
                          void send('session.complete', selected);
                        }}
                      >
                        Complete without crediting
                      </button>
                      <button
                        type="button"
                        ref={keepOpenRef}
                        onClick={() => {
                          setConfirmComplete(null);
                        }}
                      >
                        Keep it open
                      </button>
                    </div>
                  </div>
                ) : null}
                <p className="field-hint">Only the steps this session can take now are enabled. The backend checks every step again.</p>
                {/* One interval is already drawn by the meter above; the strip earns its place from the second on. */}
                {selected.intervals.length > 1 ? (
                  <div className="stack-sm">
                    <h3>Intervals, in order</h3>
                    <IntervalStrip
                      label={`The intervals of this session in order: ${selected.intervals.length} recorded`}
                      intervals={selected.intervals.map((i) => ({
                        key: i.interval_id,
                        ms: intervalLength(i),
                        continuity: i.end_utc === null ? 'OPEN' : i.continuity,
                        title: `${i.interval_id}: ${intervalSpan(i, today.zone)}, ${i.end_utc === null ? 'open' : i.continuity === 'VERIFIED' ? 'verified' : 'in doubt'}`,
                      }))}
                    />
                    <Legend
                      items={[
                        { mark: 'posted', label: 'Verified' },
                        { mark: 'doubt', label: 'In doubt' },
                        { mark: 'live', label: 'Open now' },
                      ]}
                    />
                  </div>
                ) : null}
                <dl className="pairs">
                  <dt>Planned</dt>
                  <dd className="num">{formatDuration(selected.planned_ms)}</dd>
                  <dt>In doubt</dt>
                  <dd className="num" data-testid="uncertain">
                    {formatDuration(selected.uncertain_ms)}
                  </dd>
                  <dt>Started</dt>
                  <dd>{selected.started_at === null ? 'Not started' : formatDateTime(selected.started_at, today.zone)}</dd>
                  {selected.open_interval_started_at !== null ? (
                    <>
                      <dt>Open since</dt>
                      <dd>{formatDateTime(selected.open_interval_started_at, today.zone)}. The backend credits it when you pause or complete.</dd>
                    </>
                  ) : null}
                  <dt>Record</dt>
                  <dd>
                    <span data-testid="revision">revision {selected.revision}</span>, <code>{selected.session_id}</code>
                  </dd>
                </dl>
              </Instrument>

              {showCorrection ? (
                <Panel title={outstanding ? 'Correct the time' : 'Change a correction'}>
                  <form
                    className="stack"
                    aria-label="Time correction"
                    onSubmit={(e) => {
                      e.preventDefault();
                      if (chosen !== undefined) {
                        void correct(selected, chosen.interval);
                      }
                    }}
                  >
                    {correctable.length === 0 ? (
                      <NoData reason="The backend says this session needs a correction, but it has no closed interval in doubt to correct yet." />
                    ) : (
                      <div className="form-grid">
                        <Field
                          wide
                          label="Interval in doubt"
                          hint="Every closed interval the backend could not vouch for. One that already has a correction can be corrected again: every correction is kept and the latest counts."
                        >
                          {({ id, describedBy }) => (
                            <select
                              id={id}
                              aria-describedby={describedBy}
                              value={chosen?.interval.interval_id ?? ''}
                              onChange={(e) => {
                                setIntervalChoice(e.target.value === '' ? null : { sessionId: selected.session_id, intervalId: e.target.value });
                              }}
                            >
                              <option value="">Choose an interval…</option>
                              {correctable.map(({ interval: i, latest }) => (
                                <option key={i.interval_id} value={i.interval_id}>
                                  {`${intervalSpan(i, today.zone)}, ${latest === null ? 'not corrected' : `corrected to ${formatDuration(latest.credited_ms)}`}`}
                                </option>
                              ))}
                            </select>
                          )}
                        </Field>
                        {chosen !== undefined && chosen.latest !== null ? (
                          <p className="note wide" data-testid="supersedes-note">
                            This replaces correction <code>{chosen.latest.correction_id}</code> ({formatDuration(chosen.latest.credited_ms)} credited). Both are kept; the latest one
                            counts.
                          </p>
                        ) : null}
                        <Field
                          label="Minutes to credit"
                          hint={chosen === undefined ? undefined : `At most ${Math.floor(intervalLength(chosen.interval) / 60_000)}, the length of the interval.`}
                        >
                          {({ id, describedBy }) => (
                            <input
                              id={id}
                              type="number"
                              min={0}
                              max={chosen === undefined ? undefined : Math.floor(intervalLength(chosen.interval) / 60_000)}
                              value={creditedMinutes}
                              aria-describedby={describedBy}
                              onChange={(e) => {
                                setCreditedMinutes(e.target.value);
                              }}
                            />
                          )}
                        </Field>
                        <Field wide label="Reason" hint="Kept with the correction.">
                          {({ id, describedBy }) => (
                            <input
                              id={id}
                              type="text"
                              value={reason}
                              required
                              aria-describedby={describedBy}
                              onChange={(e) => {
                                setReason(e.target.value);
                              }}
                            />
                          )}
                        </Field>
                      </div>
                    )}
                    <div className="form-actions">
                      <button type="submit" className="primary" disabled={locked || chosen === undefined || reason.trim().length === 0}>
                        Record correction
                      </button>
                    </div>
                  </form>
                </Panel>
              ) : null}

              {selected.corrections.length > 0 ? (
                <Panel title="Corrections">
                  <Table
                    caption="Corrections recorded for this session, oldest first"
                    headers={['Interval', 'Credited', 'Reason', 'Recorded']}
                    numeric={[1]}
                    rows={selected.corrections.map((c) => {
                      const interval = selected.intervals.find((i) => i.interval_id === c.interval_id);
                      return {
                        key: c.correction_id,
                        cells: [
                          <span title={`${c.correction_id} for ${c.interval_id}`}>{interval === undefined ? <code>{c.interval_id}</code> : intervalSpan(interval, today.zone)}</span>,
                          formatDuration(c.credited_ms),
                          c.reason,
                          formatDateTime(c.corrected_at, today.zone),
                        ],
                      };
                    })}
                  />
                </Panel>
              ) : null}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
