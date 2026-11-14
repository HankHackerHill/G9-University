/**
 * Civil (wall-clock) time in an IANA zone <-> UTC epoch milliseconds, using the platform
 * `Intl` database. Mirrors the DST policy of ARCHITECTURE §7: a nonexistent local time
 * (spring-forward gap) resolves forward and is flagged `GAP`; an ambiguous local time
 * (fall-back overlap) resolves to the earlier, pre-transition instant and is flagged
 * `OVERLAP`. Used by the mock transport and by the calendar grid for display only; the
 * backend (`g9-calendar`) is authoritative for stored occurrences.
 */
import type { IanaZone, UtcMillis } from '../ipc/types';
import type { DstAdjustment } from '../ipc/operations';

export interface CivilDate {
  year: number;
  month: number; // 1-12
  day: number; // 1-31
}

export interface CivilDateTime extends CivilDate {
  hour: number;
  minute: number;
}

const DAY_MS = 86_400_000;
const formatterCache = new Map<string, Intl.DateTimeFormat>();

function formatter(zone: IanaZone): Intl.DateTimeFormat {
  let f = formatterCache.get(zone);
  if (f === undefined) {
    f = new Intl.DateTimeFormat('en-US', {
      timeZone: zone,
      hourCycle: 'h23',
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      weekday: 'short',
    });
    formatterCache.set(zone, f);
  }
  return f;
}

export function isValidZone(zone: string): boolean {
  try {
    formatter(zone);
    return true;
  } catch {
    return false;
  }
}

export interface ZonedParts extends CivilDateTime {
  second: number;
  /** 0 = Sunday ... 6 = Saturday, in the zone. */
  weekday: number;
}

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** Civil fields of an instant in a zone. */
export function toZoned(utc: UtcMillis, zone: IanaZone): ZonedParts {
  const parts = formatter(zone).formatToParts(new Date(utc));
  const get = (type: Intl.DateTimeFormatPartTypes): string => parts.find((p) => p.type === type)?.value ?? '0';
  return {
    year: Number(get('year')),
    month: Number(get('month')),
    day: Number(get('day')),
    hour: Number(get('hour')),
    minute: Number(get('minute')),
    second: Number(get('second')),
    weekday: Math.max(0, WEEKDAYS.indexOf(get('weekday'))),
  };
}

/** Zone offset (ms east of UTC) in effect at `utc`. */
export function offsetAt(utc: UtcMillis, zone: IanaZone): number {
  const z = toZoned(utc, zone);
  const asUtc = Date.UTC(z.year, z.month - 1, z.day, z.hour, z.minute, z.second);
  // Drop sub-second noise: the formatter has second precision.
  return asUtc - (utc - (((utc % 1000) + 1000) % 1000));
}

export interface Resolved {
  utc: UtcMillis;
  adjusted: DstAdjustment | null;
}

/** Resolve a civil date-time in a zone to an instant, applying the DST policy. */
export function civilToUtc(civil: CivilDateTime, zone: IanaZone): Resolved {
  const guess = Date.UTC(civil.year, civil.month - 1, civil.day, civil.hour, civil.minute);
  const before = offsetAt(guess - DAY_MS / 2, zone);
  const after = offsetAt(guess + DAY_MS / 2, zone);
  const candidates = [guess - before, guess - after].filter((c, i, arr) => arr.indexOf(c) === i);
  const valid = candidates.filter((c) => offsetAt(c, zone) === guess - c);
  if (valid.length === 1) {
    const only = valid[0];
    return { utc: only ?? guess, adjusted: null };
  }
  if (valid.length > 1) {
    // Overlap: the earlier instant carries the pre-transition offset.
    return { utc: Math.min(...valid), adjusted: 'OVERLAP' };
  }
  // Gap: shift forward by the gap length (first valid instant after the gap).
  return { utc: guess - before, adjusted: 'GAP' };
}

export function parseIsoDate(date: string): CivilDate | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (m === null) {
    return null;
  }
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  // The backend's `CivilDate::new` refuses a day the month does not have (2026-02-30), and so does this.
  if (month < 1 || month > 12 || day < 1 || day > new Date(Date.UTC(year, month, 0)).getUTCDate()) {
    return null;
  }
  return { year, month, day };
}

export function formatIsoDate(d: CivilDate): string {
  const mm = String(d.month).padStart(2, '0');
  const dd = String(d.day).padStart(2, '0');
  return `${d.year}-${mm}-${dd}`;
}

/** Add whole days to a civil date (calendar arithmetic, no zone involved). */
export function addDays(d: CivilDate, days: number): CivilDate {
  const t = Date.UTC(d.year, d.month - 1, d.day) + days * DAY_MS;
  const x = new Date(t);
  return { year: x.getUTCFullYear(), month: x.getUTCMonth() + 1, day: x.getUTCDate() };
}

/** Whole days from `a` to `b` (b - a). */
export function daysBetween(a: CivilDate, b: CivilDate): number {
  return Math.round((Date.UTC(b.year, b.month - 1, b.day) - Date.UTC(a.year, a.month - 1, a.day)) / DAY_MS);
}

/** ISO weekday: 1 = Monday ... 7 = Sunday. */
export function isoWeekday(d: CivilDate): number {
  const js = new Date(Date.UTC(d.year, d.month - 1, d.day)).getUTCDay();
  return js === 0 ? 7 : js;
}

/** The Monday on or before `d`. */
export function startOfIsoWeek(d: CivilDate): CivilDate {
  return addDays(d, 1 - isoWeekday(d));
}

/** Local midnight of a civil date in a zone. */
export function startOfDayUtc(d: CivilDate, zone: IanaZone): UtcMillis {
  return civilToUtc({ ...d, hour: 0, minute: 0 }, zone).utc;
}

export function civilDateOf(utc: UtcMillis, zone: IanaZone): CivilDate {
  const z = toZoned(utc, zone);
  return { year: z.year, month: z.month, day: z.day };
}

/**
 * The value of an `<input type="datetime-local">` (`YYYY-MM-DDTHH:mm`, optional seconds) as a
 * civil date-time; null when it is empty or not a real calendar date and time. The input has no
 * zone: forms resolve it with `civilToUtc` in the operator's zone from `system.status`, never in
 * the webview's.
 */
export function parseLocalDateTime(value: string): CivilDateTime | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::\d{2}(?:\.\d{1,3})?)?$/.exec(value);
  if (m === null) {
    return null;
  }
  const [year, month, day, hour, minute] = [m[1], m[2], m[3], m[4], m[5]].map(Number) as [number, number, number, number, number];
  const probe = new Date(Date.UTC(year, month - 1, day));
  if (probe.getUTCFullYear() !== year || probe.getUTCMonth() !== month - 1 || probe.getUTCDate() !== day || hour > 23 || minute > 59) {
    return null;
  }
  return { year, month, day, hour, minute };
}

/** An instant as a `datetime-local` value in `zone` (to prefill a form). */
export function formatLocalDateTime(utc: UtcMillis, zone: IanaZone): string {
  const z = toZoned(utc, zone);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${formatIsoDate(z)}T${pad(z.hour)}:${pad(z.minute)}`;
}
