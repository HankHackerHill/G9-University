/**
 * The lesson sheets that ship with the application: the text of each study block of the G9/CT9
 * curriculum (`data/curriculum/g9-ct9-480.sheets.v1.json`, generated from `docs/curriculum/weeks/`
 * by `scripts/build_curriculum.py`; schema `schemas/curriculum/lesson-sheets.schema.json`).
 *
 * They are reading, not records: nothing here reaches the backend or changes a plan, a timer or a
 * grade. A sheet is shown only for a day whose objective texts equal the ones it was written for,
 * so it is never put next to a curriculum it does not fit (a different version, another import).
 */

/** One block of the day: the sheet text of the block the plan names by `key`. */
export interface SheetSection {
  /** The block name of the manifest (`recall`, `guided_lab`, `blind_practical`, ...). */
  key: string;
  name: string;
  minutes: number;
  markdown: string;
}

export interface DaySheet {
  week: number;
  title: string;
  /** The objective texts of the manifest this sheet was written for, in order. */
  objectives: string[];
  sections: SheetSection[];
  watch_for: string | null;
  extend: string | null;
  /** The external resources as a Markdown list: address as code, version, licence note. */
  resources: string;
}

export interface WeekSheet {
  title: string;
  purpose: string;
  lab_stack: string;
  competencies: string;
  accelerator: string;
}

export interface LessonSheets {
  sheets_schema_version: string;
  curriculum_id: string;
  /** The curriculum version the sheets were written for. */
  built_for_version: string;
  note: string;
  weeks: Record<string, WeekSheet>;
  days: Record<string, DaySheet>;
}

let pending: Promise<LessonSheets> | null = null;

/** Read the sheets once, when a day that has blocks is shown (a separate chunk: about 360 kB). A failed read is tried again next time. */
export function loadLessonSheets(): Promise<LessonSheets> {
  if (pending === null) {
    const attempt = import('../../../../data/curriculum/g9-ct9-480.sheets.v1.json').then((m) => m.default as unknown as LessonSheets);
    pending = attempt;
    void attempt.catch(() => {
      if (pending === attempt) {
        pending = null;
      }
    });
  }
  return pending;
}

export type SheetLookup =
  | { kind: 'found'; sheet: DaySheet; week: WeekSheet | null }
  /** The day exists in the sheets, but the plan's objectives are not the ones it was written for. */
  | { kind: 'other_version'; builtFor: string }
  /** The sheets have no such day (another curriculum). */
  | { kind: 'missing' };

/** The sheet for `dayId`, if its objectives are exactly `objectiveTexts`. */
export function findSheet(sheets: LessonSheets, dayId: string, objectiveTexts: readonly string[]): SheetLookup {
  if (!Object.hasOwn(sheets.days, dayId)) {
    return { kind: 'missing' };
  }
  const sheet = sheets.days[dayId];
  if (sheet === undefined) {
    return { kind: 'missing' };
  }
  const same = sheet.objectives.length === objectiveTexts.length && sheet.objectives.every((text, i) => text === objectiveTexts[i]);
  if (!same) {
    return { kind: 'other_version', builtFor: sheets.built_for_version };
  }
  const weekKey = String(sheet.week);
  return { kind: 'found', sheet, week: Object.hasOwn(sheets.weeks, weekKey) ? (sheets.weeks[weekKey] ?? null) : null };
}

/** Block states in which the block still has work in it: the sheet of the first such block is the one to read now. */
const OPEN_STATES: ReadonlySet<string> = new Set(['PLANNED', 'READY', 'RUNNING', 'PAUSED', 'INTERRUPTED']);

/**
 * The block key to open first: the first block with no session yet or a session that is not
 * finished. Null when the sessions are not known (a read failed or is in flight: unknown is not
 * "no session") or when every block is finished.
 */
export function currentBlockKey(
  blocks: readonly { block: string; session_id: string | null }[],
  sessions: readonly { session_id: string; state: string }[] | null,
): string | null {
  if (sessions === null) {
    return null;
  }
  for (const b of blocks) {
    const s = sessions.find((x) => x.session_id === b.session_id);
    if (s === undefined || OPEN_STATES.has(s.state)) {
      return b.block;
    }
  }
  return null;
}
