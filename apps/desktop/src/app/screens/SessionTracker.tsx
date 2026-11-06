import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { SessionView } from '../../ipc/operations';
import type { G9Error, SessionEvent } from '../../ipc/types';
import { formatDateTime, formatDuration, humanizeEnum } from '../../lib/format';
import { SESSION_MACHINE, can, type SESSION_OPERATION_EVENTS } from '../../lib/state-machines';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
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

export function SessionTracker() {
  const today = useToday();
  const client = useIpc();
  const sessions = useCommand('session.list', {});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [creditedMinutes, setCreditedMinutes] = useState('0');
  const [reason, setReason] = useState('');

  const list = sessions.data?.sessions ?? [];
  const interrupted = list.filter((s) => s.state === 'INTERRUPTED');
  const selected: SessionView | undefined = list.find((s) => s.session_id === selectedId) ?? interrupted[0] ?? list.find((s) => s.state === 'RUNNING' || s.state === 'PAUSED') ?? list[0];

  const send = async (op: Exclude<SessionOp, 'session.correct_time'>, session: SessionView) => {
    setBusy(true);
    const r = await client.invoke(op, { session_id: session.session_id }, { expectedRevision: session.revision });
    setError(isG9Error(r) ? r : null);
    setBusy(false);
    sessions.reload();
  };

  const correct = async (session: SessionView) => {
    const interval = session.intervals.find((i) => i.continuity === 'UNCERTAIN');
    if (interval === undefined) {
      return;
    }
    setBusy(true);
    const r = await client.invoke(
      'session.correct_time',
      { session_id: session.session_id, interval_id: interval.interval_id, credited_ms: Math.max(0, Math.round(Number(creditedMinutes) * 60_000)), reason },
      { expectedRevision: session.revision },
    );
    setError(isG9Error(r) ? r : null);
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
                      void send(c.op, selected);
                    }}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
              <p className="muted">
                Buttons are enabled only for edges in the session machine (PLANNED→READY→RUNNING⇄PAUSED→COMPLETED). The backend re-checks every transition; the UI
                mirrors, it does not decide.
              </p>
              {selected.state === 'INTERRUPTED' ? (
                <form
                  className="stack"
                  aria-label="Time correction"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void correct(selected);
                  }}
                >
                  <h3>Correction required</h3>
                  <p>
                    Uncertain interval:{' '}
                    {selected.intervals
                      .filter((i) => i.continuity === 'UNCERTAIN')
                      .map((i) => `${i.interval_id} (${formatDateTime(i.start_utc, today.zone)} → ${i.end_utc === null ? 'open' : formatDateTime(i.end_utc, today.zone)})`)
                      .join(', ')}
                  </p>
                  <div className="row">
                    <label htmlFor="credited-minutes">Minutes to credit</label>
                    <input
                      id="credited-minutes"
                      type="number"
                      min={0}
                      max={Math.round(selected.uncertain_ms / 60_000)}
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
                  <button type="submit" className="primary" disabled={busy || reason.trim().length === 0}>
                    Record correction and recover
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
