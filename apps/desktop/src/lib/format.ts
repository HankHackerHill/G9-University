/**
 * Display formatting. Durations come from the backend as integer milliseconds; instants as
 * epoch milliseconds plus an IANA zone. Nothing here derives elapsed time from wall clocks.
 */
import type { DurationMs, IanaZone, UtcMillis } from '../ipc/types';
import { toZoned } from './civil-time';

export function formatDuration(ms: DurationMs): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(2, '0');
  const ss = String(s).padStart(2, '0');
  return `${h}:${mm}:${ss}`;
}

export function formatMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) {
    return `${m} min`;
  }
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

export function formatTime(utc: UtcMillis, zone: IanaZone): string {
  const z = toZoned(utc, zone);
  return `${String(z.hour).padStart(2, '0')}:${String(z.minute).padStart(2, '0')}`;
}

export function formatDate(utc: UtcMillis, zone: IanaZone): string {
  const z = toZoned(utc, zone);
  return `${z.year}-${String(z.month).padStart(2, '0')}-${String(z.day).padStart(2, '0')}`;
}

export function formatDateTime(utc: UtcMillis, zone: IanaZone): string {
  return `${formatDate(utc, zone)} ${formatTime(utc, zone)}`;
}

/** Short hash prefix for dense tables; the full hash stays available in `title`. */
export function shortHash(hex: string, length = 12): string {
  return hex.length <= length ? hex : `${hex.slice(0, length)}…`;
}

export function humanizeEnum(value: string): string {
  return value.toLowerCase().replace(/_/g, ' ');
}
