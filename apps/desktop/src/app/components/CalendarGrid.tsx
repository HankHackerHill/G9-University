/**
 * In-house accessible week grid (ADR-0008; no FullCalendar).
 *
 * ARIA grid pattern: `role=grid` → rows → columnheader/rowheader/gridcell. One roving tabindex:
 * exactly one cell is tabbable; arrow keys move the active cell, Home/End jump within the row,
 * PageUp/PageDown jump within the column. Each cell's `aria-label` names day, hour and the
 * events it contains, so screen readers get the full picture without entering the cell.
 *
 * No timed event of the week is ever hidden: a timed event is listed in the cell containing its
 * start (an event that began before the week is listed in Monday 00:00). By default the hour rows
 * span 07:00–21:00 and grow to cover the earliest and latest event start of the week. When a
 * caller fixes the range with `startHour`/`endHour`, events starting outside it are listed in a
 * note under the grid instead of being dropped.
 */
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import type { CalendarEvent } from '../../ipc/operations';
import type { IanaZone } from '../../ipc/types';
import { addDays, civilDateOf, civilToUtc, formatIsoDate, toZoned, type CivilDate } from '../../lib/civil-time';
import { formatDateTime, formatTime } from '../../lib/format';

export interface CalendarGridProps {
  weekStart: CivilDate;
  zone: IanaZone;
  events: CalendarEvent[];
  /** Fixes the first hour row. Omitted: 07:00, extended down to the earliest event start of the week. */
  startHour?: number;
  /** Fixes the end of the last hour row (exclusive). Omitted: 21:00, extended up to cover the latest event start. */
  endHour?: number;
  onActivate?: (cell: { date: CivilDate; hour: number | null; events: CalendarEvent[] }) => void;
}

const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const DEFAULT_START_HOUR = 7;
const DEFAULT_END_HOUR = 21;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

interface Cell {
  key: string;
  date: CivilDate;
  hour: number | null;
  start: number;
  end: number;
  events: CalendarEvent[];
}

export function CalendarGrid({ weekStart, zone, events, startHour, endHour, onActivate }: CalendarGridProps) {
  const days = useMemo(() => Array.from({ length: 7 }, (_, i) => addDays(weekStart, i)), [weekStart]);
  const weekFrom = useMemo(() => civilToUtc({ ...weekStart, hour: 0, minute: 0 }, zone).utc, [weekStart, zone]);
  const weekTo = useMemo(() => civilToUtc({ ...addDays(weekStart, 7), hour: 0, minute: 0 }, zone).utc, [weekStart, zone]);
  // Timed events of this week, each anchored at the instant it is listed: its start, or the
  // start of the week for an event that began earlier and is still running.
  const timed = useMemo(
    () =>
      events
        .filter((e) => !e.all_day && e.end > weekFrom && e.start < weekTo)
        .map((e) => ({ event: e, anchor: Math.max(e.start, weekFrom) })),
    [events, weekFrom, weekTo],
  );
  const [firstHour, lastHour] = useMemo(() => {
    let first = startHour ?? DEFAULT_START_HOUR;
    let last = endHour ?? DEFAULT_END_HOUR;
    for (const { anchor } of timed) {
      const hour = toZoned(anchor, zone).hour;
      if (startHour === undefined) {
        first = Math.min(first, hour);
      }
      if (endHour === undefined) {
        last = Math.max(last, hour + 1);
      }
    }
    return [Math.max(0, first), Math.min(24, Math.max(last, first + 1))];
  }, [timed, zone, startHour, endHour]);
  const hours = useMemo(() => Array.from({ length: lastHour - firstHour }, (_, i) => firstHour + i), [firstHour, lastHour]);

  const rows = useMemo<Cell[][]>(() => {
    const allDay: Cell[] = days.map((date) => {
      const start = civilToUtc({ ...date, hour: 0, minute: 0 }, zone).utc;
      const end = civilToUtc({ ...addDays(date, 1), hour: 0, minute: 0 }, zone).utc;
      return {
        key: `${formatIsoDate(date)}:allday`,
        date,
        hour: null,
        start,
        end,
        events: events.filter((e) => e.all_day && e.start < end && e.end > start),
      };
    });
    const hourly = hours.map((hour) =>
      days.map((date) => {
        const start = civilToUtc({ ...date, hour, minute: 0 }, zone).utc;
        const end = civilToUtc({ ...date, hour: hour + 1, minute: 0 }, zone).utc;
        return {
          key: `${formatIsoDate(date)}:${hour}`,
          date,
          hour,
          start,
          end,
          // An event is listed in the cell containing its anchor (its start within this week).
          events: timed.filter((t) => t.anchor >= start && t.anchor < end).map((t) => t.event),
        };
      }),
    );
    return [allDay, ...hourly];
  }, [days, hours, events, timed, zone]);

  // Only possible with a caller-fixed range: events whose anchor lies outside every hour row.
  const outside = useMemo(() => {
    const listed = new Set(rows.slice(1).flatMap((cells) => cells.flatMap((c) => c.events.map((e) => e.event_id))));
    return timed.filter((t) => !listed.has(t.event.event_id)).map((t) => t.event);
  }, [rows, timed]);

  const [active, setActive] = useState<{ row: number; col: number }>({ row: 1, col: 0 });
  const pendingFocus = useRef(false);
  const cellRefs = useRef(new Map<string, HTMLDivElement>());

  useEffect(() => {
    if (!pendingFocus.current) {
      return;
    }
    pendingFocus.current = false;
    const cell = rows[active.row]?.[active.col];
    if (cell !== undefined) {
      cellRefs.current.get(cell.key)?.focus();
    }
  }, [active, rows]);

  const move = (dRow: number, dCol: number, absolute?: Partial<{ row: number; col: number }>) => {
    setActive((a) => {
      const row = Math.min(rows.length - 1, Math.max(0, absolute?.row ?? a.row + dRow));
      const col = Math.min(6, Math.max(0, absolute?.col ?? a.col + dCol));
      return { row, col };
    });
    pendingFocus.current = true;
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case 'ArrowRight':
        move(0, 1);
        break;
      case 'ArrowLeft':
        move(0, -1);
        break;
      case 'ArrowDown':
        move(1, 0);
        break;
      case 'ArrowUp':
        move(-1, 0);
        break;
      case 'Home':
        move(0, 0, { col: 0 });
        break;
      case 'End':
        move(0, 0, { col: 6 });
        break;
      case 'PageUp':
        move(0, 0, { row: 0 });
        break;
      case 'PageDown':
        move(0, 0, { row: rows.length - 1 });
        break;
      case 'Enter':
      case ' ': {
        const cell = rows[active.row]?.[active.col];
        if (cell !== undefined && onActivate !== undefined) {
          onActivate({ date: cell.date, hour: cell.hour, events: cell.events });
        }
        break;
      }
      default:
        return;
    }
    e.preventDefault();
  };

  const weekLabel = `Week of ${DAY_NAMES[0]} ${weekStart.day} ${MONTHS[weekStart.month - 1] ?? ''} ${weekStart.year}`;

  return (
    <>
      <div
        className="cal"
        role="grid"
        aria-label={weekLabel}
        aria-rowcount={rows.length + 1}
        aria-colcount={8}
        tabIndex={-1}
        data-testid="calendar-grid"
      >
        <div role="row" aria-rowindex={1}>
          <div role="columnheader" aria-colindex={1} className="muted">
            {zone}
          </div>
          {days.map((d, i) => (
            <div role="columnheader" aria-colindex={i + 2} key={formatIsoDate(d)}>
              <abbr title={formatIsoDate(d)}>{DAY_NAMES[i]}</abbr> {d.day}
            </div>
          ))}
        </div>
        {rows.map((cells, rowIndex) => (
          <div role="row" aria-rowindex={rowIndex + 2} key={rowIndex === 0 ? 'allday' : `h${hours[rowIndex - 1] ?? rowIndex}`}>
            <div role="rowheader" aria-colindex={1}>
              {rowIndex === 0 ? 'all day' : `${String(hours[rowIndex - 1] ?? 0).padStart(2, '0')}:00`}
            </div>
            {cells.map((cell, colIndex) => {
              const isActive = active.row === rowIndex && active.col === colIndex;
              const label = cellLabel(cell, zone);
              return (
                <div
                  key={cell.key}
                  role="gridcell"
                  aria-colindex={colIndex + 2}
                  aria-label={label}
                  aria-selected={isActive}
                  tabIndex={isActive ? 0 : -1}
                  onKeyDown={onKeyDown}
                  data-testid={`cell-${cell.key}`}
                  ref={(el) => {
                    if (el === null) {
                      cellRefs.current.delete(cell.key);
                    } else {
                      cellRefs.current.set(cell.key, el);
                    }
                  }}
                  onFocus={() => {
                    if (!isActive) {
                      setActive({ row: rowIndex, col: colIndex });
                    }
                  }}
                  onClick={() => {
                    setActive({ row: rowIndex, col: colIndex });
                    pendingFocus.current = true;
                    onActivate?.({ date: cell.date, hour: cell.hour, events: cell.events });
                  }}
                >
                  {cell.events.map((e) => (
                    <span className="cal-event" data-kind={e.kind} key={e.event_id} title={`${e.title} ${formatTime(e.start, zone)}–${formatTime(e.end, zone)}`}>
                      {e.all_day ? e.title : `${formatTime(e.start, zone)} ${e.title}`}
                    </span>
                  ))}
                </div>
              );
            })}
          </div>
        ))}
      </div>
      {outside.length > 0 ? (
        <p className="banner" role="note" data-testid="calendar-outside-hours">
          {outside.length} event{outside.length === 1 ? '' : 's'} outside the shown hours ({String(firstHour).padStart(2, '0')}:00–{String(lastHour).padStart(2, '0')}:00):{' '}
          {outside.map((e) => `${e.title} ${formatDateTime(e.start, zone)} to ${formatDateTime(e.end, zone)}`).join('; ')}
        </p>
      ) : null}
    </>
  );
}

function cellLabel(cell: Cell, zone: IanaZone): string {
  const dayName = DAY_NAMES[(new Date(Date.UTC(cell.date.year, cell.date.month - 1, cell.date.day)).getUTCDay() + 6) % 7] ?? '';
  const when = cell.hour === null ? 'all day' : `${String(cell.hour).padStart(2, '0')}:00`;
  const base = `${dayName} ${formatIsoDate(cell.date)} ${when}`;
  if (cell.events.length === 0) {
    return `${base}, no events`;
  }
  const list = cell.events.map((e) => (e.all_day ? e.title : `${e.title} ${timeRange(e, cell.date, zone)}`)).join('; ');
  return `${base}, ${cell.events.length} ${cell.events.length === 1 ? 'event' : 'events'}: ${list}`;
}

/** `HH:MM to HH:MM`; a start or end on another civil day than the cell carries its date. */
function timeRange(e: CalendarEvent, date: CivilDate, zone: IanaZone): string {
  const sameDay = (utc: number): boolean => formatIsoDate(civilDateOf(utc, zone)) === formatIsoDate(date);
  const start = sameDay(e.start) ? formatTime(e.start, zone) : formatDateTime(e.start, zone);
  const end = sameDay(e.end) ? formatTime(e.end, zone) : formatDateTime(e.end, zone);
  return `${start} to ${end}`;
}
