// @vitest-environment node
/**
 * The lesson sheets that ship with the application (`data/curriculum/g9-ct9-480.sheets.v1.json`):
 * the generated file is valid against its schema and fits the draft manifest it was generated with
 * (checked here in TypeScript, independently of the Python builder), a sheet is found only for the
 * objectives it was written for, and the block to open first follows the sessions.
 */
import Ajv2020 from 'ajv/dist/2020.js';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { currentBlockKey, findSheet, type DaySheet, type LessonSheets } from './lesson-sheets';

const root = (path: string) => fileURLToPath(new URL(`../../../../${path}`, import.meta.url));
const read = (path: string): unknown => JSON.parse(readFileSync(root(path), 'utf8'));

interface Manifest {
  curriculum_id: string;
  version: string;
  weekday_blocks: { block: string; minutes: number }[];
  saturday_blocks: { block: string; minutes: number }[];
  weeks: { week_number: number; title: string; days: { day_id: string; is_assessment_day: boolean; objectives: { text: string }[] }[] }[];
}

const schema = read('schemas/curriculum/lesson-sheets.schema.json') as Record<string, unknown>;
const sheets = read('data/curriculum/g9-ct9-480.sheets.v1.json') as LessonSheets;
const manifest = read('data/curriculum/g9-ct9-480.draft.v1.json') as Manifest;

/** The sheet of a day that must exist. */
function dayOf(file: LessonSheets, id: string): DaySheet {
  const day = file.days[id];
  if (day === undefined) {
    throw new Error(`no sheet for ${id}`);
  }
  return day;
}

/** The objective texts of a day of a manifest that must contain it. */
function objectiveTexts(m: Manifest, id: string): string[] {
  const day = m.weeks.flatMap((w) => w.days).find((d) => d.day_id === id);
  if (day === undefined) {
    throw new Error(`no day ${id}`);
  }
  return day.objectives.map((o) => o.text);
}

describe('the lesson sheets schema', () => {
  it('follows the house rules of the schemas', () => {
    expect(schema.$schema).toBe('https://json-schema.org/draft/2020-12/schema');
    expect(typeof schema.$id).toBe('string');
    expect(typeof schema.title).toBe('string');
    expect(schema.type).toBe('object');
    expect(schema.additionalProperties).toBe(false);
    expect(Array.isArray(schema.required)).toBe(true);
    expect(JSON.stringify(schema)).not.toContain('$ref');
  });

  it('accepts the generated file and refuses a file that is not what the application reads', () => {
    const validate = new Ajv2020({ allErrors: true, strict: true }).compile(schema);
    expect(validate(sheets), JSON.stringify(validate.errors)).toBe(true);

    const clone = (): LessonSheets => JSON.parse(JSON.stringify(sheets)) as LessonSheets;
    const noDay = clone();
    Reflect.deleteProperty(noDay.days, 'D72');
    expect(validate(noDay)).toBe(false);
    const extra = clone() as unknown as Record<string, unknown>;
    extra.html = '<script>1</script>';
    expect(validate(extra)).toBe(false);
    const badKey = clone();
    const firstSection = dayOf(badKey, 'D01').sections[0];
    if (firstSection === undefined) {
      throw new Error('D01 has no section');
    }
    firstSection.key = 'free_time';
    expect(validate(badKey)).toBe(false);
    const fourObjectives = clone();
    dayOf(fourObjectives, 'D01').objectives.push('one more');
    expect(validate(fourObjectives)).toBe(false);
    const stray = clone();
    stray.days.D73 = dayOf(stray, 'D01');
    expect(validate(stray)).toBe(false);
  });
});

describe('the lesson sheets fit the draft manifest', () => {
  it('were written for the draft: same curriculum, same version, same objectives, same blocks in the same order', () => {
    expect(sheets.curriculum_id).toBe(manifest.curriculum_id);
    expect(sheets.built_for_version).toBe(manifest.version);
    const days = manifest.weeks.flatMap((w) => w.days.map((d) => ({ week: w, day: d })));
    expect(days).toHaveLength(72);
    expect(Object.keys(sheets.days)).toEqual(days.map((x) => x.day.day_id));
    for (const { week, day } of days) {
      const sheet = sheets.days[day.day_id];
      expect(sheet, day.day_id).toBeDefined();
      expect(sheet?.week, day.day_id).toBe(week.week_number);
      expect(sheet?.objectives, day.day_id).toEqual(day.objectives.map((o) => o.text));
      const blocks = day.is_assessment_day ? manifest.saturday_blocks : manifest.weekday_blocks;
      expect(
        sheet?.sections.map((s) => [s.key, s.minutes]),
        day.day_id,
      ).toEqual(blocks.map((b) => [b.block, b.minutes]));
      // A Saturday keeps its task statement with the assessment definition, not in the sheet.
      for (const s of sheet?.sections ?? []) {
        expect(s.markdown.split(/\s+/).length, `${day.day_id} ${s.key}`).toBeGreaterThanOrEqual(25);
      }
      expect(sheet?.resources, day.day_id).toContain('`https://');
      expect(sheet?.resources, day.day_id).not.toContain('docs/curriculum');
    }
    expect(Object.keys(sheets.weeks)).toEqual(manifest.weeks.map((w) => String(w.week_number)));
    for (const w of manifest.weeks) {
      expect(sheets.weeks[String(w.week_number)]?.title).toBe(w.title);
    }
  });
});

describe('findSheet', () => {
  const texts = (id: string) => objectiveTexts(manifest, id);

  it('finds the sheet of a day whose objectives are the ones it was written for, with its week', () => {
    const found = findSheet(sheets, 'D15', texts('D15'));
    expect(found.kind).toBe('found');
    if (found.kind === 'found') {
      expect(found.sheet.week).toBe(3);
      expect(found.week?.title).toBe(manifest.weeks[2]?.title);
    }
  });

  it('says another version when one objective differs, in any of the three places, or the count does', () => {
    for (let i = 0; i < 3; i += 1) {
      const changed = texts('D15').map((t, k) => (k === i ? `${t} (reworded)` : t));
      expect(findSheet(sheets, 'D15', changed)).toEqual({ kind: 'other_version', builtFor: manifest.version });
    }
    expect(findSheet(sheets, 'D15', texts('D15').slice(0, 2)).kind).toBe('other_version');
    expect(findSheet(sheets, 'D15', [...texts('D15'), 'extra']).kind).toBe('other_version');
    // The Appendix A wording alone is not the text the sheet was written for.
    const seed = read('data/curriculum/g9-ct9-480.seed.v0.json') as Manifest;
    expect(findSheet(sheets, 'D15', objectiveTexts(seed, 'D15')).kind).toBe('other_version');
  });

  it('has no sheet for a day it does not know, and never reads a property of the object itself', () => {
    for (const id of ['D00', 'D73', 'UNLINKED', '', '__proto__', 'constructor', 'toString', 'hasOwnProperty']) {
      expect(findSheet(sheets, id, texts('D15')), id).toEqual({ kind: 'missing' });
    }
  });
});

describe('currentBlockKey', () => {
  const blocks = [
    { block: 'recall', session_id: 's1' },
    { block: 'theory', session_id: 's2' },
    { block: 'guided_lab', session_id: null },
  ];
  const s = (session_id: string, state: string) => ({ session_id, state });

  it('opens the first block that still has work in it', () => {
    expect(currentBlockKey(blocks, [s('s1', 'PLANNED'), s('s2', 'PLANNED')])).toBe('recall');
    expect(currentBlockKey(blocks, [s('s1', 'COMPLETED'), s('s2', 'PLANNED')])).toBe('theory');
    expect(currentBlockKey(blocks, [s('s1', 'REVIEWED'), s('s2', 'RUNNING')])).toBe('theory');
    expect(currentBlockKey(blocks, [s('s1', 'CLOSED'), s('s2', 'CANCELLED')])).toBe('guided_lab');
    for (const state of ['READY', 'RUNNING', 'PAUSED', 'INTERRUPTED']) {
      expect(currentBlockKey(blocks, [s('s1', state), s('s2', 'PLANNED')]), state).toBe('recall');
    }
  });

  it('treats a block without a session as not started, and opens nothing when every block is finished', () => {
    expect(currentBlockKey(blocks, [])).toBe('recall');
    expect(currentBlockKey(blocks, [s('s1', 'COMPLETED'), s('s2', 'COMPLETED')])).toBe('guided_lab');
    expect(currentBlockKey(blocks.slice(0, 2), [s('s1', 'COMPLETED'), s('s2', 'REVIEWED')])).toBeNull();
  });

  it('opens nothing when the sessions are not known: unknown is not "no session"', () => {
    expect(currentBlockKey(blocks, null)).toBeNull();
  });
});
