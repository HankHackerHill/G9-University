import { useEffect, useId, useRef, useState } from 'react';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { SessionView, TimeCorrection, TimeInterval } from '../../ipc/operations';
import type { G9Error, SessionEvent } from '../../ipc/types';
import { formatDateTime, formatDuration, humanizeEnum } from '../../lib/format';
import { SESSION_MACHINE, can, type SESSION_OPERATION_EVENTS } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Meter } from '../components/Meter';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

type SessionOp = keyof typeof SESSION_OPERATION_EVENTS;
type TransitionOp = Exclude<SessionOp, 'session.correct_time'>;

const CONTROLS: readonly { op: TransitionOp; event: SessionEvent; label: string }[] = [
  { op: 'session.prepare', event: 'PREPARE', label: 'Prepare' },
  { op: 'session.start', event: 'START', label: 'Start' },
  { op: 'session.pause', event: 'PAUSE', label: 'Pause' },
  { op: 'session.resume', event: 'RESUME', label: 'Resume' },
  { op: 'session.complete', event: 'COMPLETE', label: 'Complete' },
];

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

export function SessionTracker() {
  const today = useToday();
  const client = useIpc();
  const sessions = useCommand('session.list', {});
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
  const cancelRef = useRef<HTMLButtonElement>(null);
  const dialogId = useId();

  const listed = sessions.data?.sessions ?? [];
  const list = fresh === null ? listed : listed.map((s) => (s.session_id === fresh.session_id && fresh.revision > s.revision ? fresh : s));
  const interrupted = list.filter((s) => s.state === 'INTERRUPTED');
  const selected: SessionView | undefined = list.find((s) => s.session_id === selectedId) ?? interrupted[0] ?? list.find((s) => s.state === 'RUNNING' || s.state === 'PAUSED') ?? list[0];
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

  useEffect(() => {
    if (confirming) {
      cancelRef.current?.focus();
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

  return (
    <div>
      <PageHead title="Session Tracker" lede="Prepare, start, pause and complete study blocks. The backend credits the time; time it cannot vouch for waits for your correction." />
      {interrupted.length > 0 ? (
        <div className="banner" role="alert" data-testid="interrupted-banner">
          <strong>INTERRUPTED:</strong> {interrupted.length} session{interrupted.length === 1 ? '' : 's'} closed with UNCERTAIN continuity (suspend, reboot or crash). Credit is
          withheld until you record a correction below. Original intervals are never overwritten; corrections are superseding rows.
        </div>
      ) : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(sessions.warnings, actionWarnings)} />
      <div className="split">
        <Panel title="Sessions">
          {sessions.loading && sessions.data === null ? <Loading what="sessions" /> : null}
          {list.length === 0 && !sessions.loading ? <NoData reason="no sessions are planned" /> : null}
          {list.length > 0 ? (
            <Table
              caption="Planned and recorded sessions (backend timer ledger)"
              headers={['Block', 'State', 'Credited', 'Uncertain', '']}
              numeric={[2, 3]}
              rows={list.map((s) => ({
                key: s.session_id,
                cells: [
                  <span key="block">
                    <code>{s.day_id}</code> {humanizeEnum(s.block)}
                  </span>,
                  <StateBadge state={s.state} />,
                  formatDuration(s.credited_ms),
                  formatDuration(s.uncertain_ms),
                  <button
                    type="button"
                    aria-pressed={selected?.session_id === s.session_id}
                    onClick={() => {
                      setSelectedId(s.session_id);
                      setError(null);
                      setConfirmComplete(null);
                      setActionWarnings([]);
                    }}
                  >
                    Select
                  </button>,
                ],
              }))}
            />
          ) : null}
        </Panel>
        <div className="sticky">
          {selected === undefined ? (
            <Panel title="Session">
              <NoData reason="no session selected; choose one in the list" />
            </Panel>
          ) : (
            <Panel title={`${selected.day_id}: ${humanizeEnum(selected.block)}`}>
              <p className="row">
                <StateBadge state={selected.state} /> <span data-testid="revision">revision {selected.revision}</span>{' '}
                {sessions.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : <span className="muted">(read carries backend warnings, see above)</span>}
              </p>
              <div className="timesheet">
                <p className="timesheet-figure">
                  <span className="kpi-xl" data-testid="credited">
                    {formatDuration(selected.credited_ms)}
                  </span>
                  <span className="muted">credited (verified time plus corrections) of {formatDuration(selected.planned_ms)} planned</span>
                </p>
                <Meter
                  posted={selected.credited_ms}
                  doubted={selected.uncertain_ms}
                  of={selected.planned_ms}
                  owed={selected.requires_correction}
                  label={`${formatDuration(selected.credited_ms)} credited and ${formatDuration(selected.uncertain_ms)} uncertain of ${formatDuration(selected.planned_ms)} planned`}
                />
              </div>
              <dl className="pairs">
                <dt>Planned</dt>
                <dd className="num">{formatDuration(selected.planned_ms)}</dd>
                <dt>Uncertain</dt>
                <dd className="num" data-testid="uncertain">
                  {formatDuration(selected.uncertain_ms)}
                </dd>
                <dt>Started</dt>
                <dd>{selected.started_at === null ? '—' : formatDateTime(selected.started_at, today.zone)}</dd>
                {selected.open_interval_started_at !== null ? (
                  <>
                    <dt>Open interval since</dt>
                    <dd>{formatDateTime(selected.open_interval_started_at, today.zone)} (credited on pause or complete by the backend)</dd>
                  </>
                ) : null}
              </dl>
              <div className="toolbar transitions" role="group" aria-label="Session transitions">
                {CONTROLS.map((c) => (
                  <button
                    key={c.op}
                    type="button"
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
                  <strong id={`${dialogId}-title`}>Uncorrected UNCERTAIN time</strong>
                  <p id={`${dialogId}-desc`}>
                    Completing now leaves {uncorrected.length} interval{uncorrected.length === 1 ? '' : 's'} (
                    {uncorrected.map((i) => `${i.interval_id} ${formatDuration(intervalLength(i))}`).join(', ')}) without a correction. That time stays uncertain and
                    uncredited until a correction is recorded; the backend keeps accepting corrections after completion and the correction form stays available while the
                    session reports one outstanding. Credit is never assumed. To credit it now, cancel and record the correction first.
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
                      ref={cancelRef}
                      onClick={() => {
                        setConfirmComplete(null);
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : null}
              <p className="field-hint">
                Buttons are enabled only for edges in the session machine (PLANNED→READY→RUNNING⇄PAUSED→COMPLETED) and only once the list has caught up with the last
                change. The backend re-checks every transition; the window mirrors it and decides nothing.
              </p>
              {showCorrection ? (
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
                  <h3>{outstanding ? 'Correction required' : 'Time corrections (superseding)'}</h3>
                  {correctable.length === 0 ? (
                    <NoData reason="no closed UNCERTAIN interval; the backend still reports this session as requiring a correction" />
                  ) : (
                    <div className="row">
                      <label htmlFor="correction-interval">Uncertain interval to correct</label>
                      <select
                        id="correction-interval"
                        value={chosen?.interval.interval_id ?? ''}
                        onChange={(e) => {
                          setIntervalChoice(e.target.value === '' ? null : { sessionId: selected.session_id, intervalId: e.target.value });
                        }}
                      >
                        <option value="">Choose an interval…</option>
                        {correctable.map(({ interval: i, latest }) => (
                          <option key={i.interval_id} value={i.interval_id}>
                            {`${i.interval_id} · ${formatDateTime(i.start_utc, today.zone)} → ${i.end_utc === null ? 'open' : formatDateTime(i.end_utc, today.zone)} · ${formatDuration(intervalLength(i))}${
                              latest === null ? ' · uncorrected' : ` · corrected ${formatDuration(latest.credited_ms)} by ${latest.correction_id} (a new correction supersedes it)`
                            }`}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                  {chosen !== undefined && chosen.latest !== null ? (
                    <p className="muted" data-testid="supersedes-note">
                      Supersedes <code>{chosen.latest.correction_id}</code> (currently credited {formatDuration(chosen.latest.credited_ms)}): the backend keeps both rows and credits
                      the latest one.
                    </p>
                  ) : null}
                  <p className="muted">
                    Every closed UNCERTAIN interval is offered. An interval that already has a correction can be corrected again: the backend keeps every correction row and
                    credits the latest one; the original interval is never edited.
                  </p>
                  <div className="row">
                    <label htmlFor="credited-minutes">Minutes to credit</label>
                    <input
                      id="credited-minutes"
                      type="number"
                      min={0}
                      max={chosen === undefined ? undefined : Math.floor(intervalLength(chosen.interval) / 60_000)}
                      value={creditedMinutes}
                      onChange={(e) => {
                        setCreditedMinutes(e.target.value);
                      }}
                    />
                  </div>
                  <div className="row">
                    <label htmlFor="correction-reason">Reason</label>
                    <input
                      id="correction-reason"
                      type="text"
                      value={reason}
                      required
                      onChange={(e) => {
                        setReason(e.target.value);
                      }}
                    />
                  </div>
                  <button type="submit" className="primary" disabled={locked || chosen === undefined || reason.trim().length === 0}>
                    Record correction
                  </button>
                </form>
              ) : null}
              {selected.corrections.length > 0 ? (
                <Table
                  caption="Manual time corrections (superseding rows)"
                  headers={['Correction', 'Interval', 'Credited', 'Reason']}
                  rows={selected.corrections.map((c) => ({
                    key: c.correction_id,
                    cells: [<code>{c.correction_id}</code>, <code>{c.interval_id}</code>, formatDuration(c.credited_ms), c.reason],
                  }))}
                />
              ) : null}
            </Panel>
          )}
        </div>
      </div>
    </div>
  );
}
