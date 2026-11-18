/**
 * Test helper: a valid manifest of the second curriculum shape (ADR-0023, structure profile
 * `MASTERY_V2`, format 1.1.0), built in code so that no file of the repository is read. It follows
 * the manifest contract the backend validates (`crates/g9-curriculum/src/validate.rs`): 12 weeks of
 * five days, three objectives a day, 480 minutes a day, the block lists of an ordinary day and of the
 * Friday, `D01` to `D60`, and the structure constants. It carries no syllabus: the texts are
 * placeholders that say which day they belong to.
 */
import type { JsonValue } from '../ipc/types';

export interface V2Block {
  seq: number;
  block: string;
  minutes: number;
}

export interface V2Objective {
  seq: number;
  objective_id: string;
  text: string;
  competency_ids?: string[];
}

export interface V2Resource {
  ref: string;
  kind: 'EXTERNAL' | 'INTERNAL';
  version?: string;
}

export interface V2Day {
  day_id: string;
  day_of_week: number;
  planned_minutes: number;
  objectives: V2Objective[];
  evidence_artifact: string;
  blocks: V2Block[];
  is_assessment_day: boolean;
  resources?: V2Resource[];
}

export interface V2Week {
  week_number: number;
  title: string;
  days: V2Day[];
}

export interface V2Structure {
  weeks: number;
  days_per_week: number;
  objectives_per_day: number;
  weekday_minutes: number;
  saturday_minutes: number;
  weekly_minutes: number;
  total_minutes: number;
  sunday: string;
  assessment_day_of_week?: number;
}

export interface V2Manifest {
  manifest_schema_version: string;
  curriculum_id: string;
  name: string;
  version: string;
  provenance: string;
  structure: V2Structure;
  weekday_blocks: { block: string; minutes: number }[];
  saturday_blocks: { block: string; minutes: number }[];
  weeks: V2Week[];
  reconciliation?: { original_checksum: string; diff_checksum: string; approved_by: string; approved_at: number };
}

/** The blocks of an ordinary day of the v2 plan: R 30, A 120, B 120, C 150, D 60. */
export const V2_ORDINARY_BLOCKS: readonly [string, number][] = [
  ['recall', 30],
  ['theory', 120],
  ['guided_lab', 120],
  ['independent_lab', 150],
  ['defense_verification', 60],
];

/** The blocks of the scored Friday. */
export const V2_FRIDAY_BLOCKS: readonly [string, number][] = [
  ['recall', 30],
  ['theory', 120],
  ['performance_review', 120],
  ['blind_practical', 150],
  ['report', 60],
];

export const V2_CURRICULUM_ID = 'g9-cyber-mastery-480';
export const V2_VERSION = '2.0.0-test';

const specs = (blocks: readonly [string, number][]) => blocks.map(([block, minutes]) => ({ block, minutes }));
const placed = (blocks: readonly [string, number][]): V2Block[] => blocks.map(([block, minutes], i) => ({ seq: i + 1, block, minutes }));

/** A valid v2 manifest: every call returns a fresh object a test may mutate. */
export function v2Manifest(): V2Manifest {
  const weeks: V2Week[] = [];
  for (let w = 0; w < 12; w += 1) {
    const days: V2Day[] = [];
    for (let d = 0; d < 5; d += 1) {
      const dayId = `D${String(w * 5 + d + 1).padStart(2, '0')}`;
      const friday = d === 4;
      days.push({
        day_id: dayId,
        day_of_week: d + 1,
        planned_minutes: 480,
        objectives: [1, 2, 3].map((seq) => ({ seq, objective_id: `${dayId}-O${seq}`, text: `Objective ${seq} of ${dayId}` })),
        evidence_artifact: `Evidence of ${dayId}`,
        blocks: placed(friday ? V2_FRIDAY_BLOCKS : V2_ORDINARY_BLOCKS),
        is_assessment_day: friday,
      });
    }
    weeks.push({ week_number: w + 1, title: `Week ${w + 1} of the v2 test plan`, days });
  }
  return {
    manifest_schema_version: '1.1.0',
    curriculum_id: V2_CURRICULUM_ID,
    name: 'G9 cybersecurity mastery (test manifest)',
    version: V2_VERSION,
    provenance: 'PROPOSED_SEED',
    structure: {
      weeks: 12,
      days_per_week: 5,
      objectives_per_day: 3,
      weekday_minutes: 480,
      saturday_minutes: 480,
      weekly_minutes: 2_400,
      total_minutes: 28_800,
      sunday: 'unscheduled',
      assessment_day_of_week: 5,
    },
    weekday_blocks: specs(V2_ORDINARY_BLOCKS),
    saturday_blocks: specs(V2_FRIDAY_BLOCKS),
    weeks,
  };
}

/** The manifest as the JSON a request carries (a copy: later edits of `m` do not reach it). */
export function asJson(m: V2Manifest): JsonValue {
  return JSON.parse(JSON.stringify(m)) as JsonValue;
}

/** `list[index]`, or a failure that names what was missing (no non-null assertions in tests). */
export function at<T>(list: readonly T[], index: number): T {
  const item = list[index];
  if (item === undefined) {
    throw new Error(`no item at index ${index}`);
  }
  return item;
}

/** Day `n` (1-based position in the program) of the manifest. */
export function dayOf(m: V2Manifest, n: number): V2Day {
  return at(at(m.weeks, Math.floor((n - 1) / 5)).days, (n - 1) % 5);
}
