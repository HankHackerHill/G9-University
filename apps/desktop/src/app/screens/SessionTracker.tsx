import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { SessionView, TimeInterval } from '../../ipc/operations';
import type { G9Error, SessionEvent } from '../../ipc/types';
import { formatDateTime, formatDuration, humanizeEnum } from '../../lib/format';
import { SESSION_MACHINE, can, type SESSION_OPERATION_EVENTS } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

type SessionOp = keyof typeof SESSION_OPERATION_EVENTS;

const CONTROLS: readonly { op: Exclude<SessionOp, 'session.correct_time'>; event: SessionEvent; label: string }[] = [
  { op: 'session.prepare', event: 'PREPARE', label: 'Prepare' },
  { op: 'session.start', event: 'START', label: 'Start' },
  { op: 'session.pause', event: 'PAUSE', label: 'Pause' },
  { op: 'session.resume', event: 'RESUME', label: 'Resume' },
  { op: 'session.complete', event: 'COMPLETE', label: 'Complete' },
];

/**
 * Closed UNCERTAIN intervals that no correction references yet. A correction always names its
 * interval; a second correction of an already-corrected interval would supersede (replace) that
 * interval's credit, so already-corrected intervals are never offered.
 */
export function uncorrectedIntervals(session: SessionView): TimeInterval[] {
  const corrected = new Set(session.corrections.map((c) => c.interval_id));
  return session.intervals.filter((i) => i.continuity === 'UNCERTAIN' && i.end_utc !== null && !corrected.has(i.interval_id));
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
  const [creditedMinutes, setCreditedMinutes] = useState('0');
  const [reason, setReason] = useState('');
  // The interval to correct is an explicit operator choice, scoped to one session.
  const [intervalChoice, setIntervalChoice] = useState<{ sessionId: string; intervalId: string } | null>(null);
  const [confirmComplete, setConfirmComplete] = useState<string | null>(null);
  const [actionWarnings, setActionWarnings] = useState<readonly string[]>([]);

  const list = sessions.data?.sessions ?? [];
  const interrupted = list.filter((s) => s.state === 'INTERRUPTED');
  const selected: SessionView | undefined = list.find((s) => s.session_id === selectedId) ?? interrupted[0] ?? list.find((s) => s.state === 'RUNNING' || s.state === 'PAUSED') ?? list[0];
  const candidates = selected === undefined ? [] : uncorrectedIntervals(selected);
  const chosen =
    selected !== undefined && intervalChoice !== null && intervalChoice.sessionId === selected.session_id
      ? candidates.find((i) => i.interval_id === intervalChoice.intervalId)
      : undefined;
  // The form is offered whenever the backend says a correction is outstanding, not only in INTERRUPTED.
  const showCorrection = selected !== undefined && (selected.state === 'INTERRUPTED' || selected.requires_correction || candidates.length > 0);

  const send = async (op: Exclude<SessionOp, 'session.correct_time'>, session: SessionView) => {
    setConfirmComplete(null);
    setBusy(true);
    const r = await client.invoke(op, { session_id: session.session_id }, { expectedRevision: session.revision });
    setError(isG9Error(r) ? r : null);
    setActionWarnings(warningsOf(r));
    setBusy(false);
    sessions.reload();
  };

  const requestTransition = (op: Exclude<SessionOp, 'session.correct_time'>, session: SessionView) => {
    // COMPLETE from INTERRUPTED is a legal edge, but it would leave uncorrected UNCERTAIN time
    // uncredited for good: ask first.
    if (op === 'session.complete' && uncorrectedIntervals(session).length > 0 && confirmComplete !== session.session_id) {
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
    setError(isG9Error(r) ? r : null);
    setActionWarnings(warningsOf(r));
    if (!isG9Error(r)) {
      setIntervalChoice(null);
    }
    setBusy(false);
    sessions.reload();
  };

  return (
    <div>
      <h1>Session Tracker</h1>
      {interrupted.length > 0 ? (
        <div className="banner" role="alert" data-testid="interrupted-banner">
          <strong>INTERRUPTED:</strong> {interrupted.length} session{interrupted.length === 1 ? '' : 's'} closed with UNCERTAIN continuity (suspend, reboot or crash). Credit is
          withheld until you record a correction below. Original intervals are never overwritten; corrections are superseding rows.
        </div>
      ) : null}
      {sessions.error !== null ? <ErrorDisplay error={sessions.error} onRetry={sessions.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(sessions.warnings, actionWarnings)} />
      <div className="grid-2">
        <Panel title="Sessions">
          {sessions.loading && sessions.data === null ? <Loading what="sessions" /> : null}
          {list.length === 0 && !sessions.loading ? <NoData reason="no sessions are planned" /> : null}
          {list.length > 0 ? (
            <Table
              caption="Planned and recorded sessions (backend timer ledger)"
              headers={['Day', 'Block', 'State', 'Credited', 'Uncertain', '']}
              rows={list.map((s) => ({
                key: s.session_id,
                cells: [
                  s.day_id,
                  humanizeEnum(s.block),
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
        <div>
          {selected === undefined ? null : (
            <Panel title={`${selected.day_id} · ${humanizeEnum(selected.block)}`}>
              <p>
                <StateBadge state={selected.state} /> revision {selected.revision} <ProvenanceBadge label="VERIFIED" />
              </p>
              <dl>
                <div className="row">
                  <dt className="muted">Planned</dt>
                  <dd>{formatDuration(selected.planned_ms)}</dd>
                </div>
                <div className="row">
                  <dt className="muted">Credited (verified + corrections)</dt>
                  <dd className="kpi" data-testid="credited">
                    {formatDuration(selected.credited_ms)}
                  </dd>
                </div>
                <div className="row">
                  <dt className="muted">Uncertain</dt>
                  <dd data-testid="uncertain">{formatDuration(selected.uncertain_ms)}</dd>
                </div>
                <div className="row">
                  <dt className="muted">Started</dt>
                  <dd>{selected.started_at === null ? '—' : formatDateTime(selected.started_at, today.zone)}</dd>
                </div>
                {selected.open_interval_started_at !== null ? (
                  <div className="row">
                    <dt className="muted">Open interval since</dt>
                    <dd>{formatDateTime(selected.open_interval_started_at, today.zone)} (credited on pause/complete by the backend)</dd>
                  </div>
                ) : null}
              </dl>
              <div className="row" role="group" aria-label="Session transitions">
                {CONTROLS.map((c) => (
                  <button
                    key={c.op}
                    type="button"
                    disabled={busy || !can(SESSION_MACHINE, selected.state, c.event)}
                    onClick={() => {
                      requestTransition(c.op, selected);
                    }}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
              {confirmComplete === selected.session_id && candidates.length > 0 ? (
                <div className="banner" role="status" data-testid="complete-warning">
                  <strong>Uncorrected UNCERTAIN time:</strong> completing now leaves {candidates.length} interval{candidates.length === 1 ? '' : 's'} (
                  {candidates.map((i) => `${i.interval_id} ${formatDuration(intervalLength(i))}`).join(', ')}) without a correction, so that time is never credited.
                  Record a correction below first to credit it.
                  <div className="row">
                    <button
                      type="button"
                      className="danger"
                      disabled={busy}
                      onClick={() => {
                        void send('session.complete', selected);
                      }}
                    >
                      Complete without crediting
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setConfirmComplete(null);
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : null}
              <p className="muted">
                Buttons are enabled only for edges in the session machine (PLANNED→READY→RUNNING⇄PAUSED→COMPLETED). The backend re-checks every transition; the UI
                mirrors, it does not decide.
              </p>
              {showCorrection ? (
                <form
                  className="stack"
                  aria-label="Time correction"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (chosen !== undefined) {
                      void correct(selected, chosen);
                    }
                  }}
                >
                  <h3>Correction required</h3>
                  {candidates.length === 0 ? (
                    <NoData reason="no closed UNCERTAIN interval without a correction; the backend still reports this session as requiring one" />
                  ) : (
                    <div className="row">
                      <label htmlFor="correction-interval">Uncertain interval to correct</label>
                      <select
                        id="correction-interval"
                        value={chosen?.interval_id ?? ''}
                        onChange={(e) => {
                          setIntervalChoice(e.target.value === '' ? null : { sessionId: selected.session_id, intervalId: e.target.value });
                        }}
                      >
                        <option value="">Choose an interval…</option>
                        {candidates.map((i) => (
                          <option key={i.interval_id} value={i.interval_id}>
                            {`${i.interval_id} · ${formatDateTime(i.start_utc, today.zone)} → ${i.end_utc === null ? 'open' : formatDateTime(i.end_utc, today.zone)} · ${formatDuration(intervalLength(i))}`}
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                  <p className="muted">
                    Only closed UNCERTAIN intervals without a correction are offered; already-corrected intervals keep their recorded credit.
                  </p>
                  <div className="row">
                    <label htmlFor="credited-minutes">Minutes to credit</label>
                    <input
                      id="credited-minutes"
                      type="number"
                      min={0}
                      max={chosen === undefined ? undefined : Math.floor(intervalLength(chosen) / 60_000)}
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
                  <button type="submit" className="primary" disabled={busy || chosen === undefined || reason.trim().length === 0}>
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
