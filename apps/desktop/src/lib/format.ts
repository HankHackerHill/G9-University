/**
 * Display formatting. Durations come from the backend as integer milliseconds; instants as
 * epoch milliseconds plus an IANA zone. Nothing here derives elapsed time from wall clocks.
 */
import type { DurationMs, IanaZone, UtcMillis } from '../ipc/types';
import { isoWeekday, parseIsoDate, toZoned } from './civil-time';

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

const WEEKDAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

/** A civil date with its weekday (`2026-11-02` becomes `Monday 2026-11-02`); text that is not a date is returned as it is. */
export function formatCivilDate(iso: string): string {
  const d = parseIsoDate(iso);
  return d === null ? iso : `${WEEKDAY_NAMES[isoWeekday(d) - 1] ?? ''} ${iso}`;
}

/** Short hash prefix for dense tables; the full hash stays available in `title`. */
export function shortHash(hex: string, length = 12): string {
  return hex.length <= length ? hex : `${hex.slice(0, length)}…`;
}

export function humanizeEnum(value: string): string {
  return value.toLowerCase().replace(/_/g, ' ');
}

/** `GUIDED_LAB` → `Guided lab`: an enum value as a label at the start of a line. */
export function sentenceCase(value: string): string {
  const text = humanizeEnum(value);
  return text.length === 0 ? text : `${text.charAt(0).toUpperCase()}${text.slice(1)}`;
}

/**
 * The name of a planned block on screen. The plan's `label` is what the structure profile of the
 * active version calls the block on that kind of day (ADR-0023: `guided lab` in the Appendix A shape,
 * `Tool practice` in the v2 plan), so it is printed as it is, with its first letter capital. Only a
 * block without a label (an answer that carries none) falls back to the block code as a sentence.
 */
export function blockName(block: { block: string; label?: string | undefined }): string {
  const label = block.label;
  return label === undefined || label === '' ? sentenceCase(block.block) : `${label.charAt(0).toUpperCase()}${label.slice(1)}`;
}

/**
 * The name of the block a session belongs to. A session carries only the block code, so its label
 * comes from the day plan that lists the session; a session the plan does not list (another day,
 * or the plan not read) is named from its code.
 */
export function sessionBlockName(session: { session_id: string; block: string }, plan: { blocks: readonly { session_id: string | null; block: string; label?: string | undefined }[] } | null): string {
  const planned = plan?.blocks.find((b) => b.session_id === session.session_id);
  return planned === undefined ? sentenceCase(session.block) : blockName(planned);
}
