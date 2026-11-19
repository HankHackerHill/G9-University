/**
 * The curriculum shapes the mock transport knows (ADR-0023), for the mock only. The backend is
 * authoritative; this file lets the mock answer with the same profile, labels, pauses and codes:
 *  - crates/g9-domain/src/curriculum.rs: `CT9_480`, `MASTERY_V2`, `profile_for`, `label_or_code`,
 *    `pause_after`;
 *  - crates/g9-curriculum/src/manifest.rs and validate.rs: the manifest shape (a closed object at
 *    every level) and every blocking rule `validate` applies, with its code and JSON-pointer path.
 *
 * A manifest that declares `structure.assessment_day_of_week` (format 1.1.0) is checked here against
 * the profile it selects. A manifest without it is the Appendix A shape of format 1.0.0, which the
 * mock keeps checking as it always did (`validateManifest` in mock.ts). Nothing here reads a clock,
 * a file or the network.
 */
import type { StructureProfileId, Violation } from '../operations';
import type { JsonValue } from '../types';
import { hiddenCharacter } from '../../lib/text';

// ---------------------------------------------------------------------------
// Profiles (g9_domain::curriculum)
// ---------------------------------------------------------------------------

/** One block of a profile's day: its storage code, planned minutes and the name it is shown under. */
export interface ProfileBlock {
  readonly code: string;
  readonly minutes: number;
  readonly label: string;
}

/** `minutes` without a block after block `after_block` (1-based). */
export interface DayPause {
  readonly after_block: number;
  readonly minutes: number;
}

export interface StructureProfile {
  readonly id: StructureProfileId;
  readonly weeks: number;
  /** Training days of a week. */
  readonly days_per_week: number;
  readonly objectives_per_day: number;
  /** Minutes of an ordinary day (the manifest field is `weekday_minutes`). */
  readonly weekday_minutes: number;
  /** Minutes of the assessment day (the manifest field is `saturday_minutes`, kept in 1.1.0). */
  readonly saturday_minutes: number;
  readonly weekly_minutes: number;
  readonly total_minutes: number;
  readonly total_days: number;
  readonly total_objectives: number;
  /** 6 (Saturday) or 5 (Friday): the last training day of the week in both profiles. */
  readonly assessment_day_of_week: number;
  readonly ordinary_blocks: readonly ProfileBlock[];
  readonly assessment_blocks: readonly ProfileBlock[];
  readonly pauses: readonly DayPause[];
}

/** Appendix A: a label is the block code with underscores replaced by spaces. */
function appendixBlock(code: string, minutes: number): ProfileBlock {
  return { code, minutes, label: code.replace(/_/g, ' ') };
}

/** The Appendix A shape: 12 weeks of six days, Saturday assessed, no pauses. */
export const CT9_480: StructureProfile = {
  id: 'CT9_480',
  weeks: 12,
  days_per_week: 6,
  objectives_per_day: 3,
  weekday_minutes: 420,
  saturday_minutes: 300,
  weekly_minutes: 2_400,
  total_minutes: 28_800,
  total_days: 72,
  total_objectives: 216,
  assessment_day_of_week: 6,
  ordinary_blocks: [
    appendixBlock('recall', 15),
    appendixBlock('theory', 45),
    appendixBlock('guided_lab', 120),
    appendixBlock('independent_lab', 120),
    appendixBlock('defense_verification', 60),
    appendixBlock('evidence_reporting', 60),
  ],
  assessment_blocks: [appendixBlock('blind_practical', 120), appendixBlock('remediation', 60), appendixBlock('report', 60), appendixBlock('performance_review', 60)],
  pauses: [],
};

/**
 * The owner's v2.0 plan: 60 weekdays of 480 minutes in five blocks (R 30, A 120, B 120, C 150, D 60),
 * a scored Friday, and the plan's 08:00 to 17:15 day (15 minutes after the second block, 60 after the
 * third). The block codes are existing storage codes; the label says what the block is in this plan.
 */
export const MASTERY_V2: StructureProfile = {
  id: 'MASTERY_V2',
  weeks: 12,
  days_per_week: 5,
  objectives_per_day: 3,
  weekday_minutes: 480,
  saturday_minutes: 480,
  weekly_minutes: 2_400,
  total_minutes: 28_800,
  total_days: 60,
  total_objectives: 180,
  assessment_day_of_week: 5,
  ordinary_blocks: [
    { code: 'recall', minutes: 30, label: 'Active recall' },
    { code: 'theory', minutes: 120, label: 'Core lesson' },
    { code: 'guided_lab', minutes: 120, label: 'Tool practice' },
    { code: 'independent_lab', minutes: 150, label: 'Independent challenge' },
    { code: 'defense_verification', minutes: 60, label: 'Defense and documentation' },
  ],
  assessment_blocks: [
    { code: 'recall', minutes: 30, label: 'Active recall' },
    { code: 'theory', minutes: 120, label: 'Knowledge exam' },
    { code: 'performance_review', minutes: 120, label: 'Review and planning' },
    { code: 'blind_practical', minutes: 150, label: 'Scored practical' },
    { code: 'report', minutes: 60, label: 'Scoring and report' },
  ],
  pauses: [
    { after_block: 2, minutes: 15 },
    { after_block: 3, minutes: 60 },
  ],
};

/** Every structure profile, in the order of the backend's `STRUCTURE_PROFILES`. */
export const STRUCTURE_PROFILES: readonly StructureProfile[] = [CT9_480, MASTERY_V2];

/** The assessment day of a manifest that does not declare one (format 1.0.0). */
export const DEFAULT_ASSESSMENT_DAY_OF_WEEK = 6;

export function profileById(id: StructureProfileId): StructureProfile {
  return id === 'MASTERY_V2' ? MASTERY_V2 : CT9_480;
}

/** `profile_for`: the profile with this number of training days a week and this assessment day; `null` when none has both. */
export function profileFor(daysPerWeek: number, assessmentDayOfWeek: number | undefined): StructureProfile | null {
  const assessment = assessmentDayOfWeek ?? DEFAULT_ASSESSMENT_DAY_OF_WEEK;
  return STRUCTURE_PROFILES.find((p) => p.days_per_week === daysPerWeek && p.assessment_day_of_week === assessment) ?? null;
}

/**
 * `select_profile`: the profile a `structure` declares, or the nearest one when it declares none
 * (the mismatch itself is reported as `structure.mismatch`): without the field the Appendix A shape;
 * with it, the profile whose assessment day it names, else the one with that many days a week.
 */
export function selectProfile(structure: { days_per_week: number; assessment_day_of_week: number | undefined }): StructureProfile {
  const exact = profileFor(structure.days_per_week, structure.assessment_day_of_week);
  if (exact !== null) {
    return exact;
  }
  const declared = structure.assessment_day_of_week;
  if (declared === undefined) {
    return CT9_480;
  }
  return (
    STRUCTURE_PROFILES.find((p) => p.assessment_day_of_week === declared) ?? STRUCTURE_PROFILES.find((p) => p.days_per_week === structure.days_per_week) ?? CT9_480
  );
}

/** Whether `dayOfWeek` (1 = Monday) is a training day of the profile: a Saturday is a rest day in `MASTERY_V2`. */
export function isTrainingDay(profile: StructureProfile, dayOfWeek: number): boolean {
  return dayOfWeek >= 1 && dayOfWeek <= profile.days_per_week;
}

export function isAssessmentDay(profile: StructureProfile, dayOfWeek: number): boolean {
  return dayOfWeek === profile.assessment_day_of_week;
}

/** The canonical block list of a day type. */
export function profileBlocks(profile: StructureProfile, assessmentDay: boolean): readonly ProfileBlock[] {
  return assessmentDay ? profile.assessment_blocks : profile.ordinary_blocks;
}

/**
 * The profile's label of a block code on a day type, or the code with underscores replaced by
 * spaces for a code the day type does not list (`label_or_code`).
 */
export function labelOrCode(profile: StructureProfile, blockCode: string, assessmentDay: boolean): string {
  return profileBlocks(profile, assessmentDay).find((b) => b.code === blockCode)?.label ?? blockCode.replace(/_/g, ' ');
}

/** Minutes of pause after the block at 1-based position `blockSeq` (0 when none). */
export function pauseAfter(profile: StructureProfile, blockSeq: number): number {
  return profile.pauses.filter((p) => p.after_block === blockSeq).reduce((sum, p) => sum + p.minutes, 0);
}

// ---------------------------------------------------------------------------
// The manifest as the backend reads it (manifest.rs: a closed object at every level)
// ---------------------------------------------------------------------------

export interface ParsedBlockSpec {
  block: string;
  minutes: number;
}

export interface ParsedBlock extends ParsedBlockSpec {
  seq: number;
}

export interface ParsedObjective {
  seq: number;
  objective_id: string;
  text: string;
  competency_ids: string[];
}

export interface ParsedResource {
  ref: string;
  kind: 'EXTERNAL' | 'INTERNAL';
  license_note: string | null;
  version: string | null;
}

export interface ParsedDay {
  day_id: string;
  day_of_week: number;
  planned_minutes: number;
  objectives: ParsedObjective[];
  evidence_artifact: string;
  blocks: ParsedBlock[];
  is_assessment_day: boolean;
  resources: ParsedResource[];
}

export interface ParsedWeek {
  week_number: number;
  title: string;
  days: ParsedDay[];
}

interface ParsedStructure {
  weeks: number;
  days_per_week: number;
  objectives_per_day: number;
  weekday_minutes: number;
  saturday_minutes: number;
  weekly_minutes: number;
  total_minutes: number;
  sunday: string;
  assessment_day_of_week: number | undefined;
}

interface ParsedReconciliation {
  original_checksum: string;
  diff_checksum: string;
  approved_by: string;
  approved_at: number;
}

export type ManifestProvenanceName = 'PROPOSED_SEED' | 'RECONCILED_CANDIDATE' | 'APPROVED_CANONICAL';

export interface ParsedManifest {
  manifest_schema_version: string;
  curriculum_id: string;
  name: string;
  version: string;
  provenance: ManifestProvenanceName;
  provenance_note: string | null;
  structure: ParsedStructure;
  weekday_blocks: ParsedBlockSpec[];
  saturday_blocks: ParsedBlockSpec[];
  weeks: ParsedWeek[];
  reconciliation: ParsedReconciliation | null;
}

type Rec = Record<string, unknown>;

/** serde refused the manifest: a missing or unknown field, or a value of the wrong type. */
class ShapeError extends Error {}

function refuse(): never {
  throw new ShapeError('manifest shape');
}

/** A JSON object with exactly the allowed keys (`deny_unknown_fields`), the required ones present. */
function closedObject(value: unknown, required: readonly string[], optional: readonly string[] = []): Rec {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return refuse();
  }
  const rec = value as Rec;
  if (Object.keys(rec).some((k) => !required.includes(k) && !optional.includes(k))) {
    return refuse();
  }
  if (required.some((k) => !Object.hasOwn(rec, k))) {
    return refuse();
  }
  return rec;
}

function unsigned(value: unknown, max: number): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > max) {
    return refuse();
  }
  return value;
}

const u8 = (value: unknown): number => unsigned(value, 255);
const u32 = (value: unknown): number => unsigned(value, 4_294_967_295);

function signed(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    return refuse();
  }
  return value;
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : refuse();
}

/** `Option<String>`: absent or null reads as none. */
function optionalText(value: unknown): string | null {
  return value === undefined || value === null ? null : text(value);
}

function items(value: unknown): unknown[] {
  return Array.isArray(value) ? (value as unknown[]) : refuse();
}

function provenanceOf(value: unknown): ManifestProvenanceName {
  return value === 'PROPOSED_SEED' || value === 'RECONCILED_CANDIDATE' || value === 'APPROVED_CANONICAL' ? value : refuse();
}

function parseBlockSpec(value: unknown): ParsedBlockSpec {
  const rec = closedObject(value, ['block', 'minutes']);
  return { block: text(rec.block), minutes: signed(rec.minutes) };
}

function parseDay(value: unknown): ParsedDay {
  const rec = closedObject(value, ['day_id', 'day_of_week', 'planned_minutes', 'objectives', 'evidence_artifact', 'blocks', 'is_assessment_day'], ['resources']);
  if (typeof rec.is_assessment_day !== 'boolean') {
    return refuse();
  }
  return {
    day_id: text(rec.day_id),
    day_of_week: u8(rec.day_of_week),
    planned_minutes: signed(rec.planned_minutes),
    objectives: items(rec.objectives).map((o) => {
      const obj = closedObject(o, ['seq', 'objective_id', 'text'], ['competency_ids']);
      return {
        seq: u32(obj.seq),
        objective_id: text(obj.objective_id),
        text: text(obj.text),
        competency_ids: obj.competency_ids === undefined ? [] : items(obj.competency_ids).map(text),
      };
    }),
    evidence_artifact: text(rec.evidence_artifact),
    blocks: items(rec.blocks).map((b) => {
      const block = closedObject(b, ['seq', 'block', 'minutes']);
      return { seq: u32(block.seq), block: text(block.block), minutes: signed(block.minutes) };
    }),
    is_assessment_day: rec.is_assessment_day,
    resources:
      rec.resources === undefined
        ? []
        : items(rec.resources).map((r): ParsedResource => {
            const res = closedObject(r, ['ref', 'kind'], ['license_note', 'version']);
            const licenseNote = optionalText(res.license_note);
            if (res.kind !== 'EXTERNAL' && res.kind !== 'INTERNAL') {
              return refuse();
            }
            return { ref: text(res.ref), kind: res.kind, license_note: licenseNote, version: optionalText(res.version) };
          }),
  };
}

/** The manifest as `Manifest` deserializes it; `null` when serde would refuse it (`curriculum.manifest_unparseable`). */
export function parseManifest(value: JsonValue): ParsedManifest | null {
  try {
    const rec = closedObject(
      value,
      ['manifest_schema_version', 'curriculum_id', 'name', 'version', 'provenance', 'structure', 'weekday_blocks', 'saturday_blocks', 'weeks'],
      ['provenance_note', 'reconciliation'],
    );
    const provenanceNote = optionalText(rec.provenance_note);
    const s = closedObject(rec.structure, ['weeks', 'days_per_week', 'objectives_per_day', 'weekday_minutes', 'saturday_minutes', 'weekly_minutes', 'total_minutes', 'sunday'], ['assessment_day_of_week']);
    let reconciliation: ParsedReconciliation | null = null;
    if (rec.reconciliation !== undefined && rec.reconciliation !== null) {
      const r = closedObject(rec.reconciliation, ['original_checksum', 'diff_checksum', 'approved_by', 'approved_at']);
      reconciliation = { original_checksum: text(r.original_checksum), diff_checksum: text(r.diff_checksum), approved_by: text(r.approved_by), approved_at: signed(r.approved_at) };
    }
    return {
      manifest_schema_version: text(rec.manifest_schema_version),
      curriculum_id: text(rec.curriculum_id),
      name: text(rec.name),
      version: text(rec.version),
      provenance: provenanceOf(rec.provenance),
      provenance_note: provenanceNote,
      structure: {
        weeks: u32(s.weeks),
        days_per_week: u32(s.days_per_week),
        objectives_per_day: u32(s.objectives_per_day),
        weekday_minutes: u32(s.weekday_minutes),
        saturday_minutes: u32(s.saturday_minutes),
        weekly_minutes: u32(s.weekly_minutes),
        total_minutes: u32(s.total_minutes),
        sunday: text(s.sunday),
        assessment_day_of_week: s.assessment_day_of_week === undefined || s.assessment_day_of_week === null ? undefined : u8(s.assessment_day_of_week),
      },
      weekday_blocks: items(rec.weekday_blocks).map(parseBlockSpec),
      saturday_blocks: items(rec.saturday_blocks).map(parseBlockSpec),
      weeks: items(rec.weeks).map((w): ParsedWeek => {
        const week = closedObject(w, ['week_number', 'title', 'days']);
        return { week_number: u32(week.week_number), title: text(week.title), days: items(week.days).map(parseDay) };
      }),
      reconciliation,
    };
  } catch (e) {
    if (e instanceof ShapeError) {
      return null;
    }
    throw e;
  }
}

/** Whether a manifest declares `structure.assessment_day_of_week` (format 1.1.0): it is then checked here, not by the Appendix A rules of the mock. */
export function declaresAssessmentDay(manifest: JsonValue): boolean {
  if (typeof manifest !== 'object' || manifest === null || Array.isArray(manifest)) {
    return false;
  }
  const structure = manifest.structure;
  if (typeof structure !== 'object' || structure === null || Array.isArray(structure)) {
    return false;
  }
  const declared = structure.assessment_day_of_week;
  return declared !== undefined && declared !== null;
}

// ---------------------------------------------------------------------------
// Validation (g9-curriculum validate.rs; errors block import, warnings are not reported)
// ---------------------------------------------------------------------------

/** Every error code `validate` can emit (validate.rs `codes::ALL_ERRORS`), plus the parse refusal. */
export const MANIFEST_CODES = {
  unparseable: 'curriculum.manifest_unparseable',
  schemaVersionUnsupported: 'manifest.schema_version.unsupported',
  curriculumIdInvalid: 'manifest.curriculum_id.invalid',
  nameEmpty: 'manifest.name.empty',
  versionEmpty: 'manifest.version.empty',
  structureMismatch: 'structure.mismatch',
  structureAssessmentDayFormat: 'structure.assessment_day_of_week.requires_format_1_1',
  structureSunday: 'structure.sunday.not_unscheduled',
  weekdayBlocksCanonical: 'blocks.weekday.canonical_mismatch',
  saturdayBlocksCanonical: 'blocks.saturday.canonical_mismatch',
  weekCount: 'week.count',
  weekNumberSequence: 'week.number.out_of_sequence',
  weekTitleEmpty: 'week.title.empty',
  weekDaysCount: 'week.days.count',
  dayOfWeekRange: 'day.day_of_week.out_of_range',
  daySundayPresent: 'day.sunday_present',
  dayOfWeekOrder: 'day.day_of_week.not_increasing',
  dayIdMismatch: 'day.id.mismatch',
  dayIdDuplicate: 'day.id.duplicate',
  dayPlannedMinutes: 'day.planned_minutes.mismatch',
  dayObjectivesCount: 'day.objectives.count',
  dayEvidenceEmpty: 'day.evidence_artifact.empty',
  dayBlocksSum: 'day.blocks.sum_mismatch',
  dayBlocksCanonical: 'day.blocks.canonical_mismatch',
  dayAssessmentFlag: 'day.assessment_flag.mismatch',
  objectiveSeq: 'objective.seq.mismatch',
  objectiveIdMismatch: 'objective.id.mismatch',
  objectiveIdDuplicate: 'objective.id.duplicate',
  objectiveTextEmpty: 'objective.text.empty',
  textHiddenCharacter: 'manifest.text.hidden_character',
  competencyIdInvalid: 'objective.competency_id.invalid',
  blockSeq: 'block.seq.mismatch',
  durationNonPositive: 'duration.non_positive',
  resourceRefEmpty: 'resource.ref.empty',
  resourceVersionRequired: 'resource.version.required_for_external',
  provenanceReconciliationRequired: 'provenance.reconciliation_required',
  reconciliationIncomplete: 'reconciliation.incomplete',
} as const;

const SLUG_REASON_ALPHABET = /^[A-Za-z0-9._:-]*$/;
const CHECKSUM = /^[0-9a-f]{64}$/;

/** `validate_id` of g9-domain: why `s` is not an identifier slug, or `null` when it is. */
function slugProblem(s: string): string | null {
  if (s.length === 0) {
    return 'empty';
  }
  if (new TextEncoder().encode(s).length > 128) {
    return 'longer than 128 bytes';
  }
  return SLUG_REASON_ALPHABET.test(s) ? null : 'contains characters outside [A-Za-z0-9._:-]';
}

/** `parse_schema_version`: `MAJOR.MINOR.PATCH` of unsigned 16-bit numbers; the patch is ignored. */
function schemaVersion(s: string): { major: number; minor: number } | null {
  const parts = s.split('.');
  if (parts.length !== 3 || parts.some((p) => !/^\d+$/.test(p) || Number(p) > 65_535)) {
    return null;
  }
  return { major: Number(parts[0]), minor: Number(parts[1]) };
}

class Report {
  readonly errors: Violation[] = [];
  error(code: string, path: string, message: string): void {
    this.errors.push({ code, path, message });
  }
}

function canonicalDescription(canonical: readonly ProfileBlock[]): string {
  return canonical.map((b) => `${b.code} ${b.minutes}`).join(', ');
}

/** The list equals the canonical one: the same codes and minutes in the same order. */
function blocksMatch(list: readonly ParsedBlockSpec[], canonical: readonly ProfileBlock[]): boolean {
  return (
    list.length === canonical.length &&
    list.every((b, i) => {
      const c = canonical[i];
      return b.block === c?.code && b.minutes === c.minutes;
    })
  );
}

/** Hidden-character reports a manifest can cost (`MAX_HIDDEN_TEXT_ERRORS` in `crates/g9-curriculum/src/validate.rs`). */
const MAX_HIDDEN_TEXT_ERRORS = 20;

/**
 * Every free-text field is shown to the operator as text: a bidirectional override or a zero-width
 * character in one of them makes the screen show something other than what was stored (audit
 * 2026-11-18, V2-01). TAB and LF are the layout characters free text may hold. The message names
 * the code point, never the text. Twin of `check_hidden_text` in the backend.
 */
function checkHiddenText(m: ParsedManifest, r: Report): void {
  let found = 0;
  const check = (path: string, text: string | null): void => {
    if (text === null || found >= MAX_HIDDEN_TEXT_ERRORS) {
      return;
    }
    const c = hiddenCharacter(text);
    if (c !== null) {
      found += 1;
      r.error(MANIFEST_CODES.textHiddenCharacter, path, `holds a character that cannot be seen or that changes the order of the text (U+${c})`);
    }
  };
  check('/name', m.name);
  check('/version', m.version);
  check('/provenance_note', m.provenance_note);
  m.weeks.forEach((week, wi) => {
    check(`/weeks/${wi}/title`, week.title);
    week.days.forEach((day, di) => {
      const dpath = `/weeks/${wi}/days/${di}`;
      check(`${dpath}/evidence_artifact`, day.evidence_artifact);
      day.objectives.forEach((obj, oi) => {
        check(`${dpath}/objectives/${oi}/text`, obj.text);
      });
      day.resources.forEach((res, ri) => {
        check(`${dpath}/resources/${ri}/ref`, res.ref);
        check(`${dpath}/resources/${ri}/license_note`, res.license_note);
        check(`${dpath}/resources/${ri}/version`, res.version);
      });
    });
  });
}

function checkHeader(m: ParsedManifest, r: Report): void {
  const v = schemaVersion(m.manifest_schema_version);
  // The newest format read is 1.1: format 1.0.x and 1.1.x manifests are accepted.
  if (v !== null && v.major === 1 && v.minor <= 1) {
    // The field that declares the second profile exists from 1.1.0 on: a 1.0.x manifest that carries it is refused.
    if (m.structure.assessment_day_of_week !== undefined && v.minor < 1) {
      r.error(
        MANIFEST_CODES.structureAssessmentDayFormat,
        '/structure/assessment_day_of_week',
        `assessment_day_of_week needs manifest format 1.1; this manifest declares ${v.major}.${v.minor}`,
      );
    }
  } else {
    r.error(MANIFEST_CODES.schemaVersionUnsupported, '/manifest_schema_version', 'expected a MAJOR.MINOR.PATCH version accepted by 1.1');
  }
  const idProblem = slugProblem(m.curriculum_id);
  if (idProblem !== null) {
    r.error(MANIFEST_CODES.curriculumIdInvalid, '/curriculum_id', idProblem);
  }
  if (m.name.trim() === '') {
    r.error(MANIFEST_CODES.nameEmpty, '/name', 'name must not be empty');
  }
  if (m.version.trim() === '') {
    r.error(MANIFEST_CODES.versionEmpty, '/version', 'version must not be empty');
  }
}

function checkStructure(s: ParsedStructure, profile: StructureProfile, r: Report): void {
  const fields: [string, number, number][] = [
    ['weeks', profile.weeks, s.weeks],
    ['days_per_week', profile.days_per_week, s.days_per_week],
    ['objectives_per_day', profile.objectives_per_day, s.objectives_per_day],
    ['weekday_minutes', profile.weekday_minutes, s.weekday_minutes],
    ['saturday_minutes', profile.saturday_minutes, s.saturday_minutes],
    ['weekly_minutes', profile.weekly_minutes, s.weekly_minutes],
    ['total_minutes', profile.total_minutes, s.total_minutes],
  ];
  for (const [name, expected, actual] of fields) {
    if (expected !== actual) {
      r.error(MANIFEST_CODES.structureMismatch, `/structure/${name}`, `expected ${expected}, declared ${actual}`);
    }
  }
  if (s.assessment_day_of_week !== undefined && s.assessment_day_of_week !== profile.assessment_day_of_week) {
    r.error(MANIFEST_CODES.structureMismatch, '/structure/assessment_day_of_week', `expected ${profile.assessment_day_of_week}, declared ${s.assessment_day_of_week}`);
  }
  if (s.sunday !== 'unscheduled') {
    r.error(MANIFEST_CODES.structureSunday, '/structure/sunday', 'Sunday must be declared `unscheduled`');
  }
}

function checkBlockList(path: string, list: readonly ParsedBlockSpec[], canonical: readonly ProfileBlock[], code: string, r: Report): void {
  list.forEach((b, i) => {
    if (b.minutes <= 0) {
      r.error(MANIFEST_CODES.durationNonPositive, `${path}/${i}/minutes`, `block \`${b.block}\` has non-positive minutes ${b.minutes}`);
    }
  });
  if (!blocksMatch(list, canonical)) {
    r.error(code, path, `expected exactly [${canonicalDescription(canonical)}]`);
  }
}

interface Counts {
  weeks: number;
  days: number;
  objectives: number;
  total_minutes: number;
  weekday_days: number;
  saturday_days: number;
}

interface Seen {
  dayIds: Map<string, string>;
  objectiveIds: Map<string, string>;
}

function checkDay(day: ParsedDay, profile: StructureProfile, wi: number, di: number, dpath: string, prevDow: number | null, seen: Seen, counts: Counts, r: Report): void {
  // Day of week: a training day of the profile (null otherwise; the checks that depend on the
  // day type are then skipped and the error is reported once).
  let dow: number | null = null;
  if (day.day_of_week === 7) {
    r.error(MANIFEST_CODES.daySundayPresent, `${dpath}/day_of_week`, 'Sunday (7) is unscheduled and must not appear');
  } else if (isTrainingDay(profile, day.day_of_week)) {
    dow = day.day_of_week;
  } else {
    r.error(
      MANIFEST_CODES.dayOfWeekRange,
      `${dpath}/day_of_week`,
      day.day_of_week >= 1 && day.day_of_week <= 6
        ? `day_of_week ${day.day_of_week} is not a training day of profile ${profile.id}: expected 1..=${profile.days_per_week}`
        : `day_of_week.out_of_range: expected 1..=6 (Mon..Sat), actual ${day.day_of_week}`,
    );
  }
  if (prevDow !== null && day.day_of_week <= prevDow) {
    r.error(MANIFEST_CODES.dayOfWeekOrder, `${dpath}/day_of_week`, `day_of_week ${day.day_of_week} does not follow ${prevDow}`);
  }
  // Day id: position-derived, so a wrong week_number does not cascade.
  const expectedId = `D${String(wi * profile.days_per_week + di + 1).padStart(2, '0')}`;
  if (day.day_id !== expectedId) {
    r.error(MANIFEST_CODES.dayIdMismatch, `${dpath}/day_id`, `expected \`${expectedId}\`, found \`${day.day_id}\``);
  }
  const firstDay = seen.dayIds.get(day.day_id);
  if (firstDay !== undefined) {
    r.error(MANIFEST_CODES.dayIdDuplicate, `${dpath}/day_id`, `day id \`${day.day_id}\` already used at ${firstDay}`);
  }
  seen.dayIds.set(day.day_id, dpath);
  if (day.planned_minutes <= 0) {
    r.error(MANIFEST_CODES.durationNonPositive, `${dpath}/planned_minutes`, `planned_minutes ${day.planned_minutes} is not positive`);
  }
  if (dow !== null) {
    const expected = isAssessmentDay(profile, dow) ? profile.saturday_minutes : profile.weekday_minutes;
    if (day.planned_minutes !== expected) {
      r.error(MANIFEST_CODES.dayPlannedMinutes, `${dpath}/planned_minutes`, `expected ${expected} for day_of_week ${dow}, found ${day.planned_minutes}`);
    }
    const assessed = isAssessmentDay(profile, dow);
    if (assessed) {
      counts.saturday_days += 1;
    } else {
      counts.weekday_days += 1;
    }
    if (day.is_assessment_day !== assessed) {
      r.error(MANIFEST_CODES.dayAssessmentFlag, `${dpath}/is_assessment_day`, `is_assessment_day must be ${assessed} on day_of_week ${dow}`);
    }
  }
  counts.total_minutes += Math.min(Math.max(day.planned_minutes, 0), 4_294_967_295);

  if (day.objectives.length !== profile.objectives_per_day) {
    r.error(MANIFEST_CODES.dayObjectivesCount, `${dpath}/objectives`, `expected ${profile.objectives_per_day} objectives, found ${day.objectives.length}`);
  }
  day.objectives.forEach((obj, oi) => {
    const opath = `${dpath}/objectives/${oi}`;
    counts.objectives += 1;
    const expectedSeq = oi + 1;
    if (obj.seq !== expectedSeq) {
      r.error(MANIFEST_CODES.objectiveSeq, `${opath}/seq`, `expected seq ${expectedSeq}, found ${obj.seq}`);
    }
    const expectedObjectiveId = `${day.day_id}-O${expectedSeq}`;
    if (obj.objective_id !== expectedObjectiveId) {
      r.error(MANIFEST_CODES.objectiveIdMismatch, `${opath}/objective_id`, `expected \`${expectedObjectiveId}\`, found \`${obj.objective_id}\``);
    }
    const firstObjective = seen.objectiveIds.get(obj.objective_id);
    if (firstObjective !== undefined) {
      r.error(MANIFEST_CODES.objectiveIdDuplicate, `${opath}/objective_id`, `objective id \`${obj.objective_id}\` already used at ${firstObjective}`);
    }
    seen.objectiveIds.set(obj.objective_id, opath);
    if (obj.text.trim() === '') {
      r.error(MANIFEST_CODES.objectiveTextEmpty, `${opath}/text`, 'objective text must not be empty');
    }
    obj.competency_ids.forEach((cid, ci) => {
      const problem = slugProblem(cid);
      if (problem !== null) {
        r.error(MANIFEST_CODES.competencyIdInvalid, `${opath}/competency_ids/${ci}`, problem);
      }
    });
  });

  if (day.evidence_artifact.trim() === '') {
    r.error(MANIFEST_CODES.dayEvidenceEmpty, `${dpath}/evidence_artifact`, 'every day needs a deliverable');
  }

  checkBlocks(day, profile, dow, dpath, r);

  day.resources.forEach((res, ri) => {
    const rpath = `${dpath}/resources/${ri}`;
    if (res.ref.trim() === '') {
      r.error(MANIFEST_CODES.resourceRefEmpty, `${rpath}/ref`, 'resource ref must not be empty');
    }
    if (res.kind === 'EXTERNAL' && (res.version === null || res.version.trim() === '')) {
      r.error(MANIFEST_CODES.resourceVersionRequired, `${rpath}/version`, 'external resources must carry a version');
    }
  });
}

function checkBlocks(day: ParsedDay, profile: StructureProfile, dow: number | null, dpath: string, r: Report): void {
  let sum = 0;
  day.blocks.forEach((b, bi) => {
    const bpath = `${dpath}/blocks/${bi}`;
    const expectedSeq = bi + 1;
    if (b.seq !== expectedSeq) {
      r.error(MANIFEST_CODES.blockSeq, `${bpath}/seq`, `expected seq ${expectedSeq}, found ${b.seq}`);
    }
    if (b.minutes <= 0) {
      r.error(MANIFEST_CODES.durationNonPositive, `${bpath}/minutes`, `block \`${b.block}\` has non-positive minutes ${b.minutes}`);
    }
    sum += b.minutes;
  });
  if (sum !== day.planned_minutes) {
    r.error(MANIFEST_CODES.dayBlocksSum, `${dpath}/blocks`, `blocks sum to ${sum}, planned_minutes is ${day.planned_minutes}`);
  }
  if (dow !== null) {
    const canonical = profileBlocks(profile, isAssessmentDay(profile, dow));
    if (!blocksMatch(day.blocks, canonical)) {
      r.error(MANIFEST_CODES.dayBlocksCanonical, `${dpath}/blocks`, `expected exactly [${canonicalDescription(canonical)}]`);
    }
  }
}

function checkReconciliation(rec: ParsedReconciliation, r: Report): void {
  if (!CHECKSUM.test(rec.original_checksum)) {
    r.error(MANIFEST_CODES.reconciliationIncomplete, '/reconciliation/original_checksum', 'original_checksum must be a 64-character lowercase hex SHA-256');
  }
  if (!CHECKSUM.test(rec.diff_checksum)) {
    r.error(MANIFEST_CODES.reconciliationIncomplete, '/reconciliation/diff_checksum', 'diff_checksum must be a 64-character lowercase hex SHA-256');
  }
  const actor = slugProblem(rec.approved_by);
  if (actor !== null) {
    r.error(MANIFEST_CODES.reconciliationIncomplete, '/reconciliation/approved_by', `approved_by must be an actor id: ${actor}`);
  }
  if (rec.approved_at <= 0) {
    r.error(MANIFEST_CODES.reconciliationIncomplete, '/reconciliation/approved_at', 'approved_at must be a positive UTC epoch millisecond instant');
  }
}

export interface ManifestCheck {
  /** What blocks the import, in the order of the backend report: errors first, then the arithmetic invariants. */
  violations: Violation[];
  /** The manifest as read; `null` when it does not match the manifest format. */
  manifest: ParsedManifest | null;
  /** The profile the checks used; `null` when the manifest could not be read. */
  profile: StructureProfile | null;
}

/**
 * `curriculum_validate`: parse the manifest, select its profile and apply every blocking rule of
 * `g9_curriculum::validate` against that profile. The arithmetic invariants are those of
 * `StructureProfile::violations`, reported on `$.structure`.
 */
export function checkManifest(value: JsonValue): ManifestCheck {
  const m = parseManifest(value);
  if (m === null) {
    return {
      violations: [
        {
          code: MANIFEST_CODES.unparseable,
          path: '$',
          message: 'manifest does not match the curriculum manifest format (v1.0.0 or v1.1.0) (shape)',
        },
      ],
      manifest: null,
      profile: null,
    };
  }
  const profile = selectProfile(m.structure);
  const r = new Report();
  checkHeader(m, r);
  checkStructure(m.structure, profile, r);
  checkBlockList('/weekday_blocks', m.weekday_blocks, profile.ordinary_blocks, MANIFEST_CODES.weekdayBlocksCanonical, r);
  checkBlockList('/saturday_blocks', m.saturday_blocks, profile.assessment_blocks, MANIFEST_CODES.saturdayBlocksCanonical, r);

  const counts: Counts = { weeks: 0, days: 0, objectives: 0, total_minutes: 0, weekday_days: 0, saturday_days: 0 };
  const seen: Seen = { dayIds: new Map(), objectiveIds: new Map() };
  if (m.weeks.length !== profile.weeks) {
    r.error(MANIFEST_CODES.weekCount, '/weeks', `expected ${profile.weeks} weeks, found ${m.weeks.length}`);
  }
  m.weeks.forEach((week, wi) => {
    const wpath = `/weeks/${wi}`;
    counts.weeks += 1;
    if (week.week_number !== wi + 1) {
      r.error(MANIFEST_CODES.weekNumberSequence, `${wpath}/week_number`, `expected week_number ${wi + 1}, found ${week.week_number}`);
    }
    if (week.title.trim() === '') {
      r.error(MANIFEST_CODES.weekTitleEmpty, `${wpath}/title`, 'week title must not be empty');
    }
    if (week.days.length !== profile.days_per_week) {
      r.error(MANIFEST_CODES.weekDaysCount, `${wpath}/days`, `expected ${profile.days_per_week} days, found ${week.days.length}`);
    }
    let prevDow: number | null = null;
    for (const [di, day] of week.days.entries()) {
      counts.days += 1;
      checkDay(day, profile, wi, di, `${wpath}/days/${di}`, prevDow, seen, counts, r);
      prevDow = day.day_of_week;
    }
  });

  if (m.provenance === 'PROPOSED_SEED') {
    if (m.reconciliation !== null) {
      checkReconciliation(m.reconciliation, r);
    }
  } else if (m.reconciliation === null) {
    r.error(MANIFEST_CODES.provenanceReconciliationRequired, '/provenance', `provenance ${m.provenance} requires a complete reconciliation block`);
  } else {
    checkReconciliation(m.reconciliation, r);
  }

  checkHiddenText(m, r);

  const arithmetic: [string, number, number][] = [
    ['curriculum.weeks', profile.weeks, counts.weeks],
    ['curriculum.days', profile.total_days, counts.days],
    ['curriculum.objectives', profile.total_objectives, counts.objectives],
    ['curriculum.total_minutes', profile.total_minutes, counts.total_minutes],
    ['curriculum.weekday_days', profile.weeks * (profile.days_per_week - 1), counts.weekday_days],
    ['curriculum.saturday_days', profile.weeks, counts.saturday_days],
  ];
  const violations = [...r.errors];
  for (const [code, expected, actual] of arithmetic) {
    if (expected !== actual) {
      violations.push({ code, path: '$.structure', message: `expected ${expected}, found ${actual}` });
    }
  }
  return { violations, manifest: m, profile };
}
