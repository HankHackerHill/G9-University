/**
 * The shape of the program (ARCHITECTURE §4: 12 weeks × 6 days × 3 objectives) and the day map
 * drawn from recorded sessions. Nothing here is estimated: a day with no recorded session has
 * `planned_ms: null` and is drawn as "nothing recorded", never as zero.
 */
import type { SessionView } from '../ipc/operations';
import type { MapDay } from '../app/components/charts';

export const PROGRAM_WEEKS = 12;
export const DAYS_PER_WEEK = 6;
export const PROGRAM_DAYS = PROGRAM_WEEKS * DAYS_PER_WEEK;

/** `D01` … `D72`, the day ids of the curriculum manifest. */
export function dayId(n: number): string {
  return `D${String(n).padStart(2, '0')}`;
}

/** The ordinal of a day id (`D15` → 15), or null when it is not a program day (e.g. `UNLINKED`). */
export function dayNumber(id: string): number | null {
  const m = /^D(\d{2})$/.exec(id);
  if (m === null) {
    return null;
  }
  const n = Number(m[1]);
  return n >= 1 && n <= PROGRAM_DAYS ? n : null;
}

/** One cell per program day, summed from the sessions recorded for it. */
export function programMap(sessions: readonly SessionView[], todayId: string | null): MapDay[] {
  const byDay = new Map<string, { planned: number; credited: number; uncertain: number }>();
  for (const s of sessions) {
    if (dayNumber(s.day_id) === null) {
      continue;
    }
    const sum = byDay.get(s.day_id) ?? { planned: 0, credited: 0, uncertain: 0 };
    sum.planned += s.planned_ms;
    sum.credited += s.credited_ms;
    sum.uncertain += s.uncertain_ms;
    byDay.set(s.day_id, sum);
  }
  return Array.from({ length: PROGRAM_DAYS }, (_, i) => {
    const id = dayId(i + 1);
    const sum = byDay.get(id);
    return {
      id,
      planned_ms: sum === undefined ? null : sum.planned,
      credited_ms: sum?.credited ?? 0,
      uncertain_ms: sum?.uncertain ?? 0,
      today: id === todayId,
    };
  });
}
