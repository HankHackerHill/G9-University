import type { CSSProperties, ReactNode } from 'react';
import { Link } from 'react-router-dom';
import type { SessionView, SystemStatus } from '../../ipc/operations';
import { civilDateOf, formatIsoDate } from '../../lib/civil-time';
import { formatDuration, formatMinutes, humanizeEnum, sentenceCase } from '../../lib/format';
import { useCommand } from '../hooks/useCommand';

function share(part: number, whole: number): number {
  if (whole <= 0) {
    return 0;
  }
  return Math.min(1, Math.max(0, part / whole));
}

function describe(name: string, minutes: number, session: SessionView | undefined, sessionsKnown: boolean): string {
  const planned = `${name}, ${formatMinutes(minutes)} planned`;
  if (!sessionsKnown) {
    return `${planned}, session state not loaded`;
  }
  if (session === undefined) {
    return `${planned}, no session`;
  }
  const doubt = session.uncertain_ms > 0 ? `, ${formatDuration(session.uncertain_ms)} in doubt` : '';
  return `${planned}, ${formatDuration(session.credited_ms)} credited${doubt}, ${humanizeEnum(session.state)}`;
}

/**
 * The day drawn to scale. One segment per study block of today's plan, as wide as its planned
 * minutes; inside, solid ink is time the backend has credited and amber hatch is time in
 * doubt (UNCERTAIN intervals awaiting a correction). Every value is the backend's: the window
 * keeps no clock and estimates nothing, so a running block shows a lamp, not a moving number.
 *
 * `epoch` is the provider's mutation count; the rail re-reads when a command is issued anywhere.
 */
export function DayRail({ status, epoch, clock }: { status: SystemStatus | null; epoch: number; clock?: ReactNode }) {
  const zone = status?.clock.zone ?? 'UTC';
  const isoDate = status === null ? null : formatIsoDate(civilDateOf(status.clock.now, zone));
  const plan = useCommand('curriculum.daily_plan', isoDate === null ? null : { date: isoDate }, undefined, epoch);
  const sessions = useCommand('session.list', isoDate === null ? null : { date: isoDate }, undefined, epoch);
  const day = plan.data;
  const list = sessions.data?.sessions ?? null;

  let code = '';
  let note: string;
  if (status === null) {
    note = '';
  } else if (plan.error !== null) {
    note = `Day plan unavailable (${plan.error.code})`;
  } else if (day === null) {
    note = 'Reading the day plan…';
  } else if (day.unscheduled || day.day_id === null) {
    code = 'Rest';
    note = `${day.date}: no study blocks`;
  } else {
    code = day.day_id;
    note = day.week_number === null ? day.date : `Week ${day.week_number} of 12, ${day.week_title ?? ''}`;
  }

  const blocks = day !== null && !day.unscheduled ? day.blocks : [];
  const credited = list === null ? null : list.reduce((sum, s) => sum + s.credited_ms, 0);
  const doubted = list === null ? null : list.reduce((sum, s) => sum + s.uncertain_ms, 0);
  const summary =
    day === null
      ? ''
      : `Today's study blocks, drawn to scale: ${formatMinutes(day.planned_minutes)} planned` +
        (credited === null || doubted === null ? '' : `, ${formatDuration(credited)} credited, ${formatDuration(doubted)} in doubt`);

  return (
    <div className="day">
      <p className="day-id">
        <span className="day-code">{code}</span>
        <span className="day-note" title={note}>
          {note}
        </span>
      </p>

      {blocks.length > 0 ? (
        <ol className="rail" aria-label={summary} data-drawn="true">
          {blocks.map((b) => {
            const session = list?.find((x) => x.session_id === b.session_id);
            const name = sentenceCase(b.block);
            const postedShare = session === undefined ? 0 : share(session.credited_ms, session.planned_ms);
            const doubtShare = session === undefined ? 0 : Math.min(share(session.uncertain_ms, session.planned_ms), 1 - postedShare);
            const attention = session !== undefined && (session.requires_correction || session.state === 'INTERRUPTED');
            return (
              <li key={b.seq} style={{ '--w': b.minutes } as CSSProperties} data-state={session?.state} data-attention={attention}>
                <Link to="/session" viewTransition aria-label={describe(name, b.minutes, session, list !== null)}>
                  <span className="rail-label" aria-hidden="true">
                    <span className="rail-name">{name}</span>
                    <span className="rail-min">{formatMinutes(b.minutes)}</span>
                  </span>
                  <span className="rail-bar" style={{ '--credited': postedShare, '--uncertain': doubtShare } as CSSProperties} />
                </Link>
              </li>
            );
          })}
        </ol>
      ) : (
        <p className="rail-empty">
          {status === null
            ? ''
            : plan.error !== null
              ? `Today's plan could not be read (${plan.error.code}), so its blocks cannot be drawn.`
              : day === null
                ? 'Reading today’s blocks…'
                : 'No study blocks today: a rest day.'}
        </p>
      )}

      <div className="day-end">
        {day !== null && !day.unscheduled && credited !== null ? (
          <span className="day-total">
            <span className="num">{formatDuration(credited)}</span> of {formatMinutes(day.planned_minutes)} credited
            {doubted !== null && doubted > 0 ? <>, {formatDuration(doubted)} in doubt</> : null}
          </span>
        ) : null}
        {clock}
      </div>
    </div>
  );
}
