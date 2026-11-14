/**
 * Deterministic in-memory transport. Seeded from the week-3 fixture slice of the curriculum
 * seed; no randomness, no wall clock (an injectable `MockClock`), no network.
 *
 * It mirrors the `g9-app` command pipeline (ARCHITECTURE §5) closely enough to exercise every
 * screen without Tauri or a model:
 *   1. schema-version check, 2. idempotency lookup, 3. policy/approval, 4. approval consumption
 *   (single-use, hash-bound), 5. expected_revision check, 6. state-machine transition,
 *   7. audit metadata (hash-chained), 9. `CommandOutcome` / closed-category `G9Error`.
 *
 * Addendum A.8: the UI never sets identity. The mock refuses meta that carries an `actor`
 * and assigns the operator actor itself, exactly as the backend does.
 *
 * Everything here is a test double. Its data is fixture data and is labeled as such in the
 * UI through `SystemStatus.transport === 'mock'`.
 */
import catalogJson from '../../fixtures/competency-catalog.json';
import fixtureJson from '../../fixtures/curriculum-week3.json';
import { canonicalJson } from '../../lib/canonical-json';
import { BASELINE_RUBRIC, CRITERION_FLOOR_PERCENT, LEVEL_SCALE, PASS_THRESHOLD } from '../../lib/rubric';
import {
  addDays,
  civilToUtc,
  civilDateOf,
  daysBetween,
  formatIsoDate,
  isoWeekday,
  parseIsoDate,
  startOfDayUtc,
  type CivilDate,
} from '../../lib/civil-time';
import { ATTACHMENT_MEDIA_TYPES, MAX_ATTACHMENT_BYTES } from '../../lib/attachment';
import { sha256Hex, sha256HexBytes } from '../../lib/sha256';
import { firstUnprintableName } from '../../lib/text';
import {
  APPROVAL_MACHINE,
  ASSESSMENT_MACHINE,
  BACKUP_MACHINE,
  SESSION_MACHINE,
  transition,
  type TransitionTable,
} from '../../lib/state-machines';
import type { IpcTransport } from '../client';
import { AUTHORED_CODE_PATTERN, OPERATIONS, isOperationName, type OperationName, type ResponseOf } from '../operations';
import type {
  ApprovalRequest,
  AssessmentDefined,
  AttachmentMediaType,
  AttachmentView,
  AttemptView,
  AuditEntry,
  BackupView,
  CalendarEvent,
  CompetencyCreated,
  CompetencyProjection,
  CriterionResult,
  DailyPlan,
  DayPlanned,
  Difficulty,
  EvidenceItemView,
  ExportFormat,
  ExportResult,
  GradeSource,
  GradeView,
  Intervention,
  LearningDimension,
  ManifestProvenance,
  NoteSummary,
  NoteView,
  OperatorEventKind,
  PackImportResult,
  ProgramCurriculum,
  ProgramStart,
  ProgramStatus,
  ReviewItem,
  ReviewItemAuthoredKind,
  ReviewRecord,
  ScheduleProposal,
  SessionView,
  SkillOutputEnvelope,
  SystemStatus,
  TimeCorrection,
  TimeInterval,
  Violation,
} from '../operations';
import type {
  Actor,
  ApprovalEvent,
  AssessmentEvent,
  BackupEvent,
  Capability,
  Classification,
  ClientCommandMeta,
  CommandMeta,
  CommandOutcome,
  CompetencyState,
  DataDomain,
  ErrorCategory,
  G9Error,
  IanaZone,
  Id,
  InteractionMode,
  JsonValue,
  ResourceKind,
  ResourceRef,
  Revision,
  SessionEvent,
  UtcMillis,
} from '../types';
import { CLASSIFICATIONS, DATA_DOMAINS, INTERACTION_MODES, RESOURCE_KINDS, SCHEMA_V1, schemaAccepts } from '../types';
import { cleanFilename, consistentWith, decodeContent, refusedSignature, rustTrim } from './mock-attachments';

// ---------------------------------------------------------------------------
// Fixture shape
// ---------------------------------------------------------------------------

export interface FixtureBlock {
  seq: number;
  block: string;
  minutes: number;
}

export interface FixtureObjective {
  seq: number;
  objective_id: string;
  text: string;
}

export interface FixtureDay {
  day_id: string;
  day_of_week: number;
  planned_minutes: number;
  objectives: FixtureObjective[];
  evidence_artifact: string;
  blocks: FixtureBlock[];
  is_assessment_day: boolean;
}

export interface CurriculumFixture {
  curriculum_id: string;
  curriculum_version_id: string;
  name: string;
  version: string;
  provenance: string;
  structure: {
    weeks: number;
    days_per_week: number;
    objectives_per_day: number;
    weekday_minutes: number;
    saturday_minutes: number;
    weekly_minutes: number;
    total_minutes: number;
  };
  program_start_date: string;
  zone: string;
  week: { week_number: number; title: string; days: FixtureDay[] };
}

export const WEEK3_FIXTURE: CurriculumFixture = fixtureJson;

// ---------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------

export interface MockClock {
  now(): UtcMillis;
  advance(ms: number): void;
  set(utc: UtcMillis): void;
}

/** Wednesday 2026-11-18 09:30 Europe/Berlin (= 08:30Z): week 3, day D15 of the fixture. */
export const MOCK_DEFAULT_NOW: UtcMillis = Date.UTC(2026, 10, 18, 8, 30, 0);

export function createMockClock(start: UtcMillis = MOCK_DEFAULT_NOW): MockClock {
  let current = start;
  return {
    now: () => current,
    advance: (ms) => {
      current += Math.max(0, Math.floor(ms));
    },
    set: (utc) => {
      current = utc;
    },
  };
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

class CommandFailure extends Error {
  readonly error: G9Error;
  constructor(error: G9Error) {
    super(error.message);
    this.name = 'CommandFailure';
    this.error = error;
  }
}

function fail(category: ErrorCategory, code: string, message: string, details?: JsonValue): CommandFailure {
  const error: G9Error = { category, code, message };
  if (details !== undefined) {
    error.details = details;
  }
  return new CommandFailure(error);
}

/**
 * g9-app pipeline step 1 (`schemas::validate_request`): a payload that does not match its request
 * schema is INVALID_INPUT `ipc.payload_invalid`, with the violations (JSON pointer and message).
 */
function schemaViolation(key: string, message: string): CommandFailure {
  return fail('INVALID_INPUT', 'ipc.payload_invalid', 'payload does not match its schema', { violations: [{ path: `/${key}`, message }] });
}

/**
 * g9-app `dispatch`: a title, file name or label that is blank or holds a hidden character is
 * refused for every operation, before the handler runs (`refuse_unprintable_names`). The backend
 * runs it after the schema check; here the schema checks live inside the handlers, so the order
 * of two simultaneous refusals can differ.
 */
function refuseUnprintableNames(payload: unknown): void {
  const found = firstUnprintableName(payload);
  if (found !== null) {
    throw fail(
      'INVALID_INPUT',
      'input.name_not_printable',
      'a title, file name or label needs visible text and no control, invisible or text-reordering characters',
      { field: found.field, reason: found.reason },
    );
  }
}

// ---------------------------------------------------------------------------
// Payload narrowing helpers (the mock validates like the backend's step 1)
// ---------------------------------------------------------------------------

type Rec = Record<string, unknown>;

function asRecord(payload: unknown, op: string): Rec {
  if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) {
    throw fail('INVALID_INPUT', 'ipc.payload_not_object', `${op}: payload must be a JSON object`);
  }
  return payload as Rec;
}

function str(rec: Rec, key: string, op: string): string {
  const v = rec[key];
  if (typeof v !== 'string' || v.length === 0) {
    throw schemaViolation(key, `${op}: '${key}' must be a non-empty string`);
  }
  return v;
}

function optStr(rec: Rec, key: string, op: string): string | undefined {
  return rec[key] === undefined ? undefined : str(rec, key, op);
}

function int(rec: Rec, key: string, op: string, min = Number.MIN_SAFE_INTEGER): number {
  const v = rec[key];
  if (typeof v !== 'number' || !Number.isInteger(v) || v < min) {
    throw schemaViolation(key, `${op}: '${key}' must be an integer >= ${min}`);
  }
  return v;
}

/** Latest `planned_start` the request schema accepts: 2100-01-01T00:00:00Z in epoch milliseconds. */
const MAX_PLANNED_START_MS = 4_102_444_800_000;

function optInt(rec: Rec, key: string, op: string, min = Number.MIN_SAFE_INTEGER): number | undefined {
  return rec[key] === undefined ? undefined : int(rec, key, op, min);
}

function oneOf<T extends string>(rec: Rec, key: string, values: readonly T[], op: string): T {
  const v = rec[key];
  if (typeof v !== 'string' || !(values as readonly string[]).includes(v)) {
    throw schemaViolation(key, `${op}: '${key}' must be one of ${values.join(', ')}`);
  }
  return v as T;
}

function optOneOf<T extends string>(rec: Rec, key: string, values: readonly T[], op: string): T | undefined {
  return rec[key] === undefined ? undefined : oneOf(rec, key, values, op);
}

function strArray(rec: Rec, key: string, op: string, optional = false): string[] {
  const v = rec[key];
  if (v === undefined && optional) {
    return [];
  }
  if (!Array.isArray(v) || !v.every((x) => typeof x === 'string')) {
    throw schemaViolation(key, `${op}: '${key}' must be an array of strings`);
  }
  return v;
}

function isJsonValue(v: unknown): v is JsonValue {
  if (v === null || typeof v === 'string' || typeof v === 'boolean') {
    return true;
  }
  if (typeof v === 'number') {
    return Number.isFinite(v);
  }
  if (Array.isArray(v)) {
    return v.every(isJsonValue);
  }
  if (typeof v === 'object') {
    return Object.values(v as Rec).every(isJsonValue);
  }
  return false;
}

function json(rec: Rec, key: string, op: string): JsonValue {
  const v = rec[key];
  if (v === undefined || !isJsonValue(v)) {
    throw schemaViolation(key, `${op}: '${key}' must be a JSON value`);
  }
  return v;
}

// ---------------------------------------------------------------------------
// Internal records
// ---------------------------------------------------------------------------

interface SessionRecord extends SessionView {
  date: string;
}

interface IdempotencyRecord {
  request_sha256: string;
  outcome: CommandOutcome<unknown>;
}

interface AssessmentDefinition {
  assessment_id: Id;
  title: string;
  day_id: Id;
  mode: InteractionMode;
}

interface Criterion {
  code: string;
  weight: number;
  critical: boolean;
}

/**
 * Baseline rubric (handoff §9, `g9_domain::assessment::BaselineRubric`): six criteria, weights
 * 25/20/20/15/10/10, `scope_compliance_reporting` is the only critical gate. Mirrored in
 * `src/lib/rubric.ts`, which is parity-tested against the Rust source.
 */
export const BASELINE_CRITERIA: readonly Criterion[] = BASELINE_RUBRIC.map((c) => ({ code: c.code, weight: c.weight, critical: c.critical_gate }));

const GENESIS_HASH = '0'.repeat(64);
const APPROVAL_TTL_MS = 10 * 60_000; // owner decision 2026-11-13 (g9-policy DEFAULT_APPROVAL_TTL)
const REVIEW_OFFSETS_DAYS = [1, 3, 7, 21, 60];
const REVIEW_ITEM_MS = 3 * 60_000;
const REVIEW_BUDGET_MS = 15 * 60_000;
const DAY_START_HOUR = 9;
const EVIDENTIARY_KINDS: readonly ResourceKind[] = [
  'ATTEMPT',
  'GRADE_DECISION',
  'OBSERVATION',
  'AUDIT_METADATA',
  'APPROVAL',
  'POLICY',
  'TIME_INTERVAL',
];

const OPERATOR: Actor = { id: 'operator', kind: { kind: 'OPERATOR' } };

export interface MockOptions {
  clock?: MockClock;
  zone?: IanaZone;
  fixture?: CurriculumFixture;
  /**
   * Assessment definition ids whose version declares no difficulty (the column is nullable in the
   * backend and never defaulted): finalizing an attempt of such a definition derives nothing and
   * warns `competency.observations_not_derived`. Every other fixture definition declares
   * `FIXTURE_DECLARED_DIFFICULTY`.
   */
  assessmentsWithoutDifficulty?: readonly Id[];
  /**
   * The operator's answer in the native confirmation dialog the desktop shell raises for an
   * APPROVE (ADR-0014). Default: confirms. `false` answers `UNAUTHORIZED approval.not_confirmed`
   * and leaves the approval pending, as the backend does when the dialog is cancelled.
   */
  confirmApproval?: (approvalId: Id) => boolean;
  /**
   * `AppConfig::attachment_dir`. Default: the desktop layout under the app data directory.
   * `null` reproduces an install without one: `evidence.attach_file` answers
   * `PRECONDITION_FAILED attachment.store_not_configured`.
   */
  attachmentDir?: string | null;
  /**
   * `AppConfig::export_dir`, the folder `export.request` writes into. Default: the desktop layout
   * under the app data directory. `null` reproduces an install without one: the approved retry
   * answers `PRECONDITION_FAILED export.dir_not_configured` and the approval is not spent.
   */
  exportDir?: string | null;
  /**
   * A new install: nothing imported, no program start, no sessions, calendar events, review
   * items, assessments or notes. The week-3 fixture then only stands in for the structure of the
   * imported curriculum (the mock cannot hold all twelve weeks): `curriculum.daily_plan` and
   * `session.plan_day` answer from it once a curriculum is imported and a start is set, whatever
   * manifest was imported; the counts `curriculum.program` reports come from the manifest.
   */
  fresh?: boolean;
}

/** Tauri `app_data_dir()` of `com.g9.mentor` on Linux, for a user named `operator` (fixture value). */
export const MOCK_DATA_DIR = '/home/operator/.local/share/com.g9.mentor';
export const MOCK_ATTACHMENT_DIR = `${MOCK_DATA_DIR}/attachments`;
export const MOCK_EXPORT_DIR = `${MOCK_DATA_DIR}/exports`;

export interface MockTransport extends IpcTransport {
  readonly kind: 'mock';
  readonly clock: MockClock;
  readonly zone: IanaZone;
  /**
   * Test-only control mirroring `g9_calendar::Ledger::on_process_restart` followed by
   * `SessionEvent::Interrupt` (A03): every open interval is closed as UNCERTAIN at the current
   * clock and its session becomes INTERRUPTED. Returns the affected session ids. This is how a
   * second interruption of the same session is reached in mock mode.
   */
  onProcessRestart(): Id[];
}

interface CatalogPack {
  pack_id: string;
  title: string;
  version: string;
  status: string;
  competencies: { id: string; name: string; prerequisites: string[] }[];
}

/** The competency packs that ship with the application (a copy of what g9-app embeds). */
const PACK_CATALOG: readonly CatalogPack[] = catalogJson.packs;
const PACK_ID_PATTERN = /^[a-z0-9][a-z0-9-]{0,63}$/;
const DAY_START_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;
const DEFAULT_DAY_START = '09:00';

/** The competency a prerequisite names: `id` inside the pack, `pack_id:id` across packs. */
function prerequisiteId(reference: string): string {
  const colon = reference.indexOf(':');
  return colon < 0 ? reference : reference.slice(colon + 1);
}

export function createMockTransport(options: MockOptions = {}): MockTransport {
  const state = new MockState(options);
  return {
    kind: 'mock',
    clock: state.clock,
    zone: state.zone,
    invoke: (op, meta, payload) => Promise.resolve(state.execute(op, meta, payload)),
    onProcessRestart: () => state.onProcessRestart(),
  };
}

class MockState {
  readonly clock: MockClock;
  readonly zone: IanaZone;
  readonly fixture: CurriculumFixture;
  private counter = 0;
  private readonly sessions = new Map<Id, SessionRecord>();
  private readonly notes = new Map<Id, NoteView>();
  private readonly attempts = new Map<Id, AttemptView>();
  private readonly definitions: AssessmentDefinition[] = [];
  /** Difficulty each assessment version's author declared (backend: `assessment_versions.difficulty`). */
  private readonly declaredDifficulty = new Map<Id, number>();
  /** Append-only competency observations derived by `assessment.approve_final` (never edited). */
  private readonly observations: MockObservation[] = [];
  private readonly approvals = new Map<Id, ApprovalRequest>();
  private readonly backups = new Map<Id, BackupView>();
  private readonly proposals = new Map<Id, ScheduleProposal>();
  private readonly idempotency = new Map<string, IdempotencyRecord>();
  private readonly audit: AuditEntry[] = [];
  private events: CalendarEvent[] = [];
  /** Stored revision of each calendar event (the event view carries none; absent = 1). */
  private readonly eventRevisions = new Map<Id, Revision>();
  /** `review_items` rows: fixture items seeded from the week, plus the ones the operator writes. */
  private readonly reviewItems = new Map<Id, ReviewItemRecord>();
  /** Competencies the operator created (`competency.create`); fixture competencies are derived. */
  private readonly authoredCompetencies: AuthoredCompetency[] = [];
  /** Competency each operator-defined assessment assesses (`assessment.define`). */
  private readonly definitionCompetency = new Map<Id, Id>();
  private readonly attachments = new Map<Id, AttachmentRecord>();
  private readonly evidence = new Map<Id, EvidenceRecord>();
  /** Notes erased by an approved `deletion.request` (their domain, for the scope check). */
  private readonly erasedNotes = new Map<Id, DataDomain>();
  private evidenceSeq = 0;
  private readonly fresh: boolean;
  /** `curriculum_versions` and `program_plan`: what is imported and where it sits on the calendar. */
  private program: { curriculum: ProgramCurriculum | null; start: ProgramStart | null };
  /** Competencies of the shipped packs that `competency.import_pack` stored, in load order. */
  private readonly packCompetencies = new Map<Id, { name: string; pack: string }>();
  /** Stored prerequisite edges `from>to` (from = the prerequisite). */
  private readonly packEdges = new Set<string>();
  /**
   * The approval the running command verified. It is consumed only when the command succeeds;
   * a failure afterwards leaves it APPROVED (g9-app: the command transaction rolls back and the
   * approval is not spent).
   */
  private pendingConsume: ApprovalRequest | null = null;
  private readonly undeclared: ReadonlySet<Id>;
  private readonly confirmApproval: (approvalId: Id) => boolean;
  private readonly attachmentDir: string | null;
  private readonly exportDir: string | null;

  constructor(options: MockOptions) {
    this.clock = options.clock ?? createMockClock();
    this.fixture = options.fixture ?? WEEK3_FIXTURE;
    this.zone = options.zone ?? this.fixture.zone;
    this.undeclared = new Set(options.assessmentsWithoutDifficulty ?? []);
    this.confirmApproval = options.confirmApproval ?? (() => true);
    this.attachmentDir = options.attachmentDir === undefined ? MOCK_ATTACHMENT_DIR : options.attachmentDir;
    this.exportDir = options.exportDir === undefined ? MOCK_EXPORT_DIR : options.exportDir;
    this.fresh = options.fresh === true;
    this.program = this.fresh
      ? { curriculum: null, start: null }
      : { curriculum: this.fixtureCurriculum(), start: { monday: this.fixture.program_start_date, day_start: DEFAULT_DAY_START, revision: 1 } };
    this.seed();
  }

  /** What `curriculum.program` reports for the preloaded fixture: the one week it holds. */
  private fixtureCurriculum(): ProgramCurriculum {
    const days = this.fixture.week.days;
    return {
      version_id: this.fixture.curriculum_version_id,
      name: this.fixture.name,
      version_label: this.fixture.version,
      provenance: this.fixture.provenance as ManifestProvenance,
      weeks: 1,
      days: days.length,
      objectives: days.reduce((n, d) => n + d.objectives.length, 0),
      planned_minutes: days.reduce((n, d) => n + d.planned_minutes, 0),
      imported_at: this.now(),
    };
  }

  // ----- helpers -----------------------------------------------------------

  private nextId(kind: string): Id {
    this.counter += 1;
    return `mock-${kind}-${this.counter}`;
  }

  private now(): UtcMillis {
    return this.clock.now();
  }

  private programStart(): CivilDate {
    return parseIsoDate(this.program.start?.monday ?? this.fixture.program_start_date) ?? { year: 2026, month: 11, day: 2 };
  }

  /** Same order as the backend: the version first, then the start. */
  private requireProgram(): void {
    if (this.program.curriculum === null) {
      throw fail('NOT_FOUND', 'curriculum.not_imported', 'no curriculum version is imported');
    }
    if (this.program.start === null) {
      throw fail('PRECONDITION_FAILED', 'curriculum.program_start_not_configured', 'the program start date (Monday of week 1) is not configured');
    }
  }

  /** The session of a block of a day (the planner's sessions and the fixture's both carry day and block). */
  private sessionOfBlock(dayId: Id, seq: number, date: string): SessionRecord | undefined {
    return [...this.sessions.values()].find((s) => s.day_id === dayId && s.block_seq === seq && s.date === date);
  }

  private dateOfDay(day: FixtureDay): CivilDate {
    const offset = (this.fixture.week.week_number - 1) * 7 + (day.day_of_week - 1);
    return addDays(this.programStart(), offset);
  }

  private dayForDate(date: CivilDate): { day: FixtureDay | null; week: number; weekday: number } {
    const idx = daysBetween(this.programStart(), date);
    const week = Math.floor(idx / 7) + 1;
    const weekday = isoWeekday(date);
    if (week !== this.fixture.week.week_number || weekday === 7) {
      return { day: null, week, weekday };
    }
    const day = this.fixture.week.days.find((d) => d.day_of_week === weekday) ?? null;
    return { day, week, weekday };
  }

  private blockStart(day: FixtureDay, seq: number): UtcMillis {
    const date = this.dateOfDay(day);
    const [hour, minute] = (this.program.start?.day_start ?? DEFAULT_DAY_START).split(':').map(Number);
    let minutes = (hour ?? DAY_START_HOUR) * 60 + (minute ?? 0);
    for (const b of day.blocks) {
      if (b.seq < seq) {
        minutes += b.minutes;
      }
    }
    return civilToUtc({ ...date, hour: Math.floor(minutes / 60), minute: minutes % 60 }, this.zone).utc;
  }

  // ----- seeding -----------------------------------------------------------

  private seed(): void {
    if (!this.fresh) {
      this.seedWeek();
    }
    this.appendAudit({
      actor: { id: 'system', kind: { kind: 'SYSTEM' } },
      operation: 'mock.seed',
      resource: { kind: 'CURRICULUM_VERSION', id: this.fixture.curriculum_version_id },
      decision: 'ALLOW',
      outcome: 'OK',
      request_id: 'mock-seed',
      trace_id: 'mock-seed',
      details: { fixture: this.fixture.curriculum_version_id, provenance: this.fixture.provenance },
    });
  }

  private seedWeek(): void {
    const today = civilDateOf(this.now(), this.zone);
    for (const day of this.fixture.week.days) {
      const date = this.dateOfDay(day);
      for (const block of day.blocks) {
        const sessionId = `mock-session-${day.day_id}-${block.seq}`;
        const start = this.blockStart(day, block.seq);
        const end = start + block.minutes * 60_000;
        this.sessions.set(sessionId, {
          session_id: sessionId,
          state: 'PLANNED',
          day_id: day.day_id,
          block: block.block,
          block_seq: block.seq,
          planned_ms: block.minutes * 60_000,
          credited_ms: 0,
          uncertain_ms: 0,
          started_at: null,
          open_interval_started_at: null,
          requires_correction: false,
          intervals: [],
          corrections: [],
          revision: 1,
          date: formatIsoDate(date),
        });
        this.events.push({
          event_id: `mock-event-${day.day_id}-${block.seq}`,
          title: `${day.day_id} ${block.block.replace(/_/g, ' ')}`,
          kind: day.is_assessment_day && block.seq === 1 ? 'ASSESSMENT' : 'STUDY_BLOCK',
          start,
          end,
          all_day: false,
          immutable: day.is_assessment_day && block.seq === 1,
          day_id: day.day_id,
          block: block.block,
          session_id: sessionId,
          adjusted: civilToUtc({ ...date, hour: DAY_START_HOUR, minute: 0 }, this.zone).adjusted,
          provenance: 'SYSTEM_DETERMINISTIC',
        });
      }
      if (day.is_assessment_day) {
        this.defineAssessment({ assessment_id: `mock-assessment-${day.day_id}`, title: `${day.day_id} blind practical`, day_id: day.day_id, mode: 'EXAM' }, 'push');
      }
    }
    // Sunday of the fixture week is protected rest (structure.sunday = unscheduled).
    const firstDay = this.fixture.week.days[0];
    if (firstDay !== undefined) {
      const sunday = addDays(this.dateOfDay(firstDay), 6);
      this.events.push({
        event_id: 'mock-event-rest-sunday',
        title: 'Protected rest',
        kind: 'PROTECTED_REST',
        start: startOfDayUtc(sunday, this.zone),
        end: startOfDayUtc(addDays(sunday, 1), this.zone),
        all_day: true,
        immutable: true,
        day_id: null,
        block: null,
        session_id: null,
        adjusted: null,
        provenance: 'SYSTEM_DETERMINISTIC',
      });
    }

    // Fixture history for the day before "today": one completed recall block (verified) and
    // one block interrupted by a suspend (closed UNCERTAIN at process start, A03).
    const yesterday = addDays(today, -1);
    const yesterdayDay = this.dayForDate(yesterday).day;
    if (yesterdayDay !== null) {
      const recall = this.sessions.get(`mock-session-${yesterdayDay.day_id}-1`);
      if (recall !== undefined) {
        const start = this.blockStart(yesterdayDay, 1);
        recall.state = 'COMPLETED';
        recall.started_at = start;
        recall.intervals = [{ interval_id: `${recall.session_id}-i1`, start_utc: start, end_utc: start + recall.planned_ms, continuity: 'VERIFIED' }];
        recall.credited_ms = recall.planned_ms;
        recall.revision = 4;
      }
      const lastSeq = yesterdayDay.blocks[yesterdayDay.blocks.length - 1]?.seq ?? 1;
      const last = this.sessions.get(`mock-session-${yesterdayDay.day_id}-${lastSeq}`);
      if (last !== undefined) {
        const start = this.blockStart(yesterdayDay, lastSeq);
        last.state = 'INTERRUPTED';
        last.started_at = start;
        last.intervals = [{ interval_id: `${last.session_id}-i1`, start_utc: start, end_utc: start + 40 * 60_000, continuity: 'UNCERTAIN' }];
        recomputeLedger(last);
        last.revision = 3;
      }
    }

    // A previous-week attempt in GRADED_PROVISIONAL so the finalization/approval flow is reachable.
    const prevWeek = this.fixture.week.week_number - 1;
    if (prevWeek >= 1) {
      const defId = `mock-assessment-W${prevWeek}`;
      this.defineAssessment({ assessment_id: defId, title: `Week ${prevWeek} blind practical (fixture)`, day_id: `D${prevWeek * 6}`, mode: 'EXAM' }, 'unshift');
      const criteria: CriterionResult[] = BASELINE_CRITERIA.map((c) => ({
        criterion_code: c.code,
        weight: c.weight,
        score: 3,
        source: 'HUMAN_REVIEWER',
        evidence_refs: [`mock-evidence-W${prevWeek}-${c.code.replace(/_/g, '-')}`],
      }));
      const finalizedAt = this.blockStart(this.fixture.week.days[0] ?? { ...emptyDay(), day_of_week: 1 }, 1) - 2 * 86_400_000;
      this.attempts.set('mock-attempt-prev', {
        attempt_id: 'mock-attempt-prev',
        assessment_id: defId,
        title: `Week ${prevWeek} blind practical (fixture)`,
        state: 'GRADED_PROVISIONAL',
        mode: 'EXAM',
        variant: 'A',
        retake_of: null,
        started_at: finalizedAt - 2 * 3_600_000,
        submitted_at: finalizedAt,
        finalized_at: null,
        grade: computeGrade(criteria),
        revision: 4,
      });
    }

    this.seedReviewItems();
  }

  /**
   * Fixture review queue: for every objective of a non-assessment day of the loaded week, one
   * baseline-scheduled item per ladder offset (1, 3, 7, 21, 60 days after the lesson, 08:30
   * local) at the stage of that offset. These are fixture rows, not a measurement; recording a
   * review moves an item along the g9-retention ladder exactly as the backend does.
   */
  private seedReviewItems(): void {
    for (const day of this.fixture.week.days) {
      if (day.is_assessment_day) {
        continue;
      }
      const date = this.dateOfDay(day);
      for (const o of day.objectives) {
        REVIEW_OFFSETS_DAYS.forEach((offset, stage) => {
          const id = `mock-review-${o.objective_id}-${offset}`;
          this.reviewItems.set(id, {
            view: {
              review_item_id: id,
              competency_id: `mock-competency-${day.day_id}`,
              objective_id: o.objective_id,
              prompt: `Recall: ${o.text}`,
              kind: 'DECLARATIVE',
              due_at: civilToUtc({ ...addDays(date, offset), hour: 8, minute: 30 }, this.zone).utc,
              last_success_at: null,
              offset_days: offset,
              estimated_ms: REVIEW_ITEM_MS,
            },
            authoredKind: 'DECLARATIVE',
            stage,
            revision: 1,
            attempts: [],
          });
        });
      }
    }
  }

  // ----- audit chain -------------------------------------------------------

  private appendAudit(input: {
    actor: Actor;
    operation: string;
    resource: ResourceRef | null;
    decision: string;
    outcome: 'OK' | 'ERROR';
    request_id: Id;
    trace_id: Id;
    details: JsonValue;
  }): AuditEntry {
    const prev = this.audit[this.audit.length - 1]?.row_hash ?? GENESIS_HASH;
    const row = {
      id: this.nextId('audit'),
      prev_hash: prev,
      occurred_at: this.now(),
      actor_id: input.actor.id,
      actor_kind: input.actor.kind.kind,
      operation: input.operation,
      resource_kind: input.resource?.kind ?? null,
      resource_id: input.resource?.id ?? null,
      decision: input.decision,
      outcome: input.outcome,
      request_id: input.request_id,
      trace_id: input.trace_id,
      details_sha256: sha256Hex(canonicalJson(input.details)),
    };
    const entry: AuditEntry = { ...row, row_hash: sha256Hex(prev + canonicalJson(row)) };
    this.audit.push(entry);
    return entry;
  }

  private verifyChain(): boolean {
    let prev = GENESIS_HASH;
    for (const e of this.audit) {
      if (e.prev_hash !== prev) {
        return false;
      }
      const { row_hash, ...row } = e;
      if (sha256Hex(prev + canonicalJson(row)) !== row_hash) {
        return false;
      }
      prev = row_hash;
    }
    return true;
  }

  // ----- pipeline ------------------------------------------------------------

  execute(opName: string, clientMeta: ClientCommandMeta, payload: unknown): unknown {
    // Same order as `g9_app::dispatch`: the operation name is checked before the metadata is parsed.
    if (!isOperationName(opName)) {
      return fail('INVALID_INPUT', 'ipc.unknown_operation', 'unknown operation').error;
    }
    // Addendum A.8 — identity is never accepted from the wire.
    if (Object.hasOwn(clientMeta, 'actor')) {
      return fail('INVALID_INPUT', 'meta.actor_not_accepted', 'command metadata must not carry an actor; the backend assigns it').error;
    }
    const spec = OPERATIONS[opName];
    if (clientMeta.operation !== opName) {
      return fail('INVALID_INPUT', 'meta.operation_mismatch', 'meta.operation does not match the invoked operation').error;
    }
    if (!schemaAccepts(spec.schemaVersion, clientMeta.payload_schema_version)) {
      return fail('INVALID_INPUT', 'ipc.schema_version_rejected', `payload schema version not accepted for ${opName}`, {
        accepted: `${spec.schemaVersion.major}.${spec.schemaVersion.minor}`,
      }).error;
    }
    // The boundary refuses a revision that cannot exist (revisions start at 1) before anything else, as g9-app does.
    if (clientMeta.expected_revision !== undefined && (!Number.isSafeInteger(clientMeta.expected_revision) || clientMeta.expected_revision < 1)) {
      return fail('INVALID_INPUT', 'meta.invalid', 'command metadata is malformed').error;
    }
    const meta: CommandMeta = { ...clientMeta, actor: OPERATOR };

    // Step 2: idempotency (mutating commands only).
    const requestHash = sha256Hex(canonicalJson({ op: opName, payload: payload ?? null }));
    const idemKey = `${meta.actor.id}:${meta.idempotency_key}`;
    if (spec.mutating) {
      const hit = this.idempotency.get(idemKey);
      if (hit !== undefined) {
        if (hit.request_sha256 !== requestHash) {
          return fail('CONFLICT', 'idempotency.key_reused', 'idempotency key was used with a different request').error;
        }
        return this.replay(opName, hit.outcome);
      }
    }

    this.pendingConsume = null;
    try {
      const result = this.dispatch(opName, payload, meta);
      this.consumePendingApproval();
      const outcome: CommandOutcome<unknown> = { result: result.result, replayed: false };
      if (result.revision !== undefined) {
        outcome.revision = result.revision;
      }
      // Like the backend, `warnings` is omitted when empty.
      if (result.warnings !== undefined && result.warnings.length > 0) {
        outcome.warnings = [...result.warnings];
      }
      if (spec.mutating) {
        const entry = this.appendAudit({
          actor: meta.actor,
          operation: opName,
          resource: result.resource ?? meta.resource ?? null,
          decision: 'ALLOW',
          outcome: 'OK',
          request_id: meta.request_id,
          trace_id: meta.trace_id,
          details: { request_sha256: requestHash },
        });
        outcome.audit_ref = entry.id;
        this.idempotency.set(idemKey, { request_sha256: requestHash, outcome });
      }
      return outcome;
    } catch (e: unknown) {
      // Rolled back: an approval verified by the failed command is not spent.
      this.pendingConsume = null;
      if (e instanceof CommandFailure) {
        if (spec.mutating) {
          this.appendAudit({
            actor: meta.actor,
            operation: opName,
            resource: meta.resource ?? null,
            decision: e.error.category === 'UNAUTHORIZED' ? 'DENY' : e.error.category,
            outcome: 'ERROR',
            request_id: meta.request_id,
            trace_id: meta.trace_id,
            details: { code: e.error.code },
          });
        }
        return e.error;
      }
      return fail('INTERNAL', 'mock.unexpected', 'internal error; see local operation log').error;
    }
  }

  private dispatch(op: OperationName, payload: unknown, meta: CommandMeta): Handled {
    refuseUnprintableNames(payload);
    switch (op) {
      case 'system.status':
        return { result: this.systemStatus() };
      case 'curriculum.validate':
        return { result: this.curriculumValidate(payload) };
      case 'curriculum.import':
        return this.curriculumImport(payload, meta);
      case 'curriculum.daily_plan':
        return this.dailyPlan(payload);
      case 'curriculum.promote':
        return this.curriculumPromote(payload, meta);
      case 'calendar.list':
        return { result: this.calendarList(payload) };
      case 'calendar.propose':
        return this.calendarPropose(payload);
      case 'calendar.apply':
        return this.calendarApply(payload, meta);
      case 'session.list':
        return { result: this.sessionList(payload) };
      case 'session.prepare':
        return this.sessionEvent(payload, 'PREPARE', meta, op);
      case 'session.start':
        return this.sessionEvent(payload, 'START', meta, op);
      case 'session.pause':
        return this.sessionEvent(payload, 'PAUSE', meta, op);
      case 'session.resume':
        return this.sessionEvent(payload, 'RESUME', meta, op);
      case 'session.complete':
        return this.sessionEvent(payload, 'COMPLETE', meta, op);
      case 'session.correct_time':
        return this.sessionCorrectTime(payload, meta);
      case 'notes.create':
        return this.notesCreate(payload, meta);
      case 'notes.revise':
        return this.notesRevise(payload, meta);
      case 'assessment.list':
        return { result: this.assessmentList() };
      case 'assessment.begin':
        return this.assessmentBegin(payload);
      case 'assessment.submit':
        return this.assessmentSubmit(payload, meta);
      case 'assessment.review_evidence':
        return this.assessmentEvent(payload, 'REVIEW_EVIDENCE', meta, op);
      case 'assessment.grade_provisional':
        return this.assessmentGrade(payload, meta);
      case 'assessment.approve_final':
        return this.assessmentApproveFinal(payload, meta);
      case 'competency.projection':
        return { result: this.competencyProjection(payload) };
      case 'retention.due':
        return this.retentionDue(payload);
      case 'interventions.list':
        return { result: this.interventionsList(payload) };
      case 'approvals.list':
        return { result: this.approvalsList(payload) };
      case 'approvals.decide':
        return this.approvalsDecide(payload, meta);
      case 'audit.recent':
        return { result: this.auditRecent(payload) };
      case 'backup.create':
        return this.backupCreate(payload);
      case 'backup.verify':
        return this.backupEvent(payload, 'CHECK', meta, op);
      case 'backup.restore_test':
        return this.backupEvent(payload, 'TEST_RESTORE', meta, op);
      case 'skills.invoke':
        return this.skillsInvoke(payload, meta);
      case 'export.request':
        return this.exportRequest(payload, meta);
      case 'deletion.request':
        return this.deletionRequest(payload, meta);
      case 'session.create':
        return this.sessionCreate(payload);
      case 'calendar.create_event':
        return this.calendarCreateEvent(payload);
      case 'calendar.update_event':
        return this.calendarUpdateEvent(payload, meta);
      case 'calendar.delete_event':
        return this.calendarDeleteEvent(payload, meta);
      case 'retention.create_item':
        return this.retentionCreateItem(payload);
      case 'retention.record_review':
        return this.retentionRecordReview(payload, meta);
      case 'competency.create':
        return this.competencyCreate(payload);
      case 'assessment.define':
        return this.assessmentDefine(payload);
      case 'evidence.attach_file':
        return this.evidenceAttachFile(payload, meta);
      case 'evidence.from_note':
        return this.evidenceFromNote(payload, meta);
      case 'evidence.list':
        return { result: this.evidenceList(payload, meta) };
      case 'curriculum.program':
        return { result: this.curriculumProgram() };
      case 'curriculum.set_program_start':
        return this.setProgramStart(payload, meta);
      case 'session.plan_day':
        return this.sessionPlanDay(payload);
      case 'notes.list':
        return { result: this.notesList(payload, meta) };
      case 'notes.get':
        return { result: this.notesGet(payload, meta) };
      case 'competency.import_pack':
        return this.competencyImportPack(payload);
      case 'backup.list':
        return { result: this.backupList() };
    }
  }

  // ----- approvals (steps 3–4) ------------------------------------------------

  /**
   * Steps 3–4 for an approval-gated command. Without `meta.approval_id` a PENDING request is
   * stored and APPROVAL_REQUIRED thrown. With one, the approval is verified and remembered; it is
   * consumed only when the command succeeds (`consumePendingApproval`), so a later failure leaves
   * it APPROVED, as the backend's rolled-back transaction does.
   *
   * `storedRevision` (an existing resource) binds the approval to the stored revision the way
   * g9-app does (ADR-0003, SEC-02): a request from a stale view gets no approval
   * (STALE_REVISION), and a resource changed between grant and use refuses the approval
   * (`approval.revision_mismatch`). Without it the client's `expected_revision` is bound.
   */
  private requireApproval(
    op: OperationName,
    payload: JsonValue,
    resource: ResourceRef,
    capability: Capability,
    meta: CommandMeta,
    summary: string,
    storedRevision?: Revision,
  ): ApprovalRequest {
    const hash = sha256Hex(canonicalJson({ operation: op, payload }));
    if (meta.approval_id === undefined) {
      if (storedRevision !== undefined) {
        this.checkRevision(meta, storedRevision);
      }
      const approval: ApprovalRequest = {
        approval_id: this.nextId('approval'),
        state: 'PENDING',
        operation_name: op,
        operation: { action: OPERATIONS[op].action, resource, scope: meta.scope, parameters_sha256: hash },
        requested_by: meta.actor.id,
        requested_at: this.now(),
        expires_at: this.now() + APPROVAL_TTL_MS,
        expected_revision: storedRevision ?? meta.expected_revision ?? null,
        decided_at: null,
        consumed_at: null,
        summary,
        revision: 1,
      };
      this.approvals.set(approval.approval_id, approval);
      const error: G9Error = {
        category: 'APPROVAL_REQUIRED',
        code: 'policy.approval_required',
        message: `${op} requires an operator approval (${capability})`,
        details: { parameters_sha256: hash, expires_at: approval.expires_at, required_capability: capability },
        approval_id: approval.approval_id,
      };
      throw new CommandFailure(error);
    }
    const approval = this.approvals.get(meta.approval_id);
    if (approval === undefined) {
      throw fail('NOT_FOUND', 'approval.not_found', 'approval not found');
    }
    this.expireIfDue(approval);
    if (approval.state === 'PENDING') {
      throw fail('PRECONDITION_FAILED', 'approval.not_decided', 'approval is still pending', { approval_id: approval.approval_id });
    }
    if (approval.state !== 'APPROVED') {
      throw fail('PRECONDITION_FAILED', 'approval.not_approved', `approval is ${approval.state}`, { approval_id: approval.approval_id, state: approval.state });
    }
    if (approval.operation_name !== op || approval.operation.parameters_sha256 !== hash || approval.operation.resource.id !== resource.id) {
      throw fail('UNAUTHORIZED', 'approval.binding_mismatch', 'approval is bound to a different operation or parameters', {
        approval_id: approval.approval_id,
      });
    }
    if (storedRevision !== undefined) {
      if (approval.expected_revision !== storedRevision) {
        throw fail(
          'STALE_REVISION',
          'approval.revision_mismatch',
          `approval is bound to expected revision ${approval.expected_revision ?? 'none'}; the operation carries ${storedRevision}`,
          { expected: storedRevision, bound: approval.expected_revision },
        );
      }
    } else if (approval.expected_revision !== null && meta.expected_revision !== undefined && approval.expected_revision !== meta.expected_revision) {
      throw fail('UNAUTHORIZED', 'approval.binding_mismatch', 'approval is bound to a different expected revision');
    }
    this.pendingConsume = approval;
    return approval;
  }

  /**
   * Step 2 replay. g9-app keeps no file name or note title in its idempotency records (erasable
   * content): an `evidence.attach_file` or `evidence.from_note` replay is rebuilt from the rows as
   * they are now (after an erasure the file name and the summary read `[REDACTED]`) and carries a
   * warning saying so. Every other outcome is replayed as stored.
   */
  private replay(op: OperationName, outcome: CommandOutcome<unknown>): CommandOutcome<unknown> {
    const replayed: CommandOutcome<unknown> = { ...outcome, replayed: true };
    const rebuilt = (result: unknown): CommandOutcome<unknown> => ({ ...replayed, result, warnings: [...(outcome.warnings ?? []), REPLAY_REBUILT_WARNING] });
    if (op === 'evidence.attach_file') {
      const stored = outcome.result as AttachmentView;
      const a = this.attachments.get(stored.attachment_id);
      if (a !== undefined) {
        return rebuilt({
          ...stored,
          note_id: a.note_id,
          filename: a.erased ? REDACTED : a.filename,
          media_type: a.media_type,
          size_bytes: a.size_bytes,
          sha256: a.sha256,
        });
      }
    }
    if (op === 'evidence.from_note') {
      const stored = outcome.result as ResponseOf<'evidence.from_note'>;
      const e = this.evidence.get(stored.evidence_id);
      if (e !== undefined && e.note_id !== null) {
        return rebuilt({ evidence_id: e.evidence_id, kind: 'NOTE', note_id: e.note_id, summary: e.summary });
      }
    }
    return replayed;
  }

  /** Single use: the approval the command verified moves to CONSUMED once the command succeeded. */
  private consumePendingApproval(): void {
    const approval = this.pendingConsume;
    this.pendingConsume = null;
    if (approval !== null) {
      this.applyApprovalEvent(approval, 'CONSUME');
      approval.consumed_at = this.now();
    }
  }

  /**
   * g9-app `creation_id`: a retry carrying an approval stored for this action on this resource
   * kind reuses the id the approval was granted for (so the approved resource matches); every
   * other call mints a fresh id. The binding is still verified in full.
   */
  private creationId(meta: CommandMeta, op: OperationName, kind: string): Id {
    if (meta.approval_id !== undefined) {
      const approval = this.approvals.get(meta.approval_id);
      const spec = OPERATIONS[op];
      if (approval?.operation.action === spec.action && approval.operation.resource.kind === spec.resourceKind) {
        return approval.operation.resource.id;
      }
    }
    return this.nextId(kind);
  }

  /** `policy.denied` for UPDATE/DELETE of a finalized (immutable) record: rule `immutable.finalized.deny`. */
  /**
   * Policy rule `operator.security_sensitive.write.approval`: an operator CREATE/UPDATE of a
   * SECURITY_SENSITIVE record needs a PROTECTED_RECORD_WRITE approval (other domains: allowed in
   * the learning modes the screens use; the mode-by-domain matrix is not modelled).
   */
  private requireSensitiveWriteApproval(op: OperationName, domain: DataDomain, payload: JsonValue, resource: ResourceRef, meta: CommandMeta): void {
    if (domain === 'SECURITY_SENSITIVE') {
      this.requireApproval(op, payload, resource, 'PROTECTED_RECORD_WRITE', meta, approvalSummary(op, resource, 'PROTECTED_RECORD_WRITE'));
    }
  }

  private expireIfDue(approval: ApprovalRequest): void {
    if ((approval.state === 'PENDING' || approval.state === 'APPROVED') && this.now() > approval.expires_at) {
      this.applyApprovalEvent(approval, 'EXPIRE');
    }
  }

  private applyApprovalEvent(approval: ApprovalRequest, event: ApprovalEvent): void {
    approval.state = applyMachine(APPROVAL_MACHINE, approval.state, event, 'approval');
    approval.revision += 1;
  }

  private checkRevision(meta: CommandMeta, actual: Revision): void {
    if (meta.expected_revision !== undefined && meta.expected_revision !== actual) {
      throw fail('STALE_REVISION', 'revision.stale', 'the record changed since it was read; reload and retry', {
        expected: meta.expected_revision,
        actual,
      });
    }
  }

  // ----- handlers -----------------------------------------------------------

  private systemStatus(): SystemStatus {
    const backups = [...this.backups.values()].sort((a, b) => b.created_at - a.created_at);
    const last = backups[0];
    for (const a of this.approvals.values()) {
      this.expireIfDue(a);
    }
    return {
      app_version: '0.1.0',
      schema_version: SCHEMA_V1,
      storage: { state: 'READY', migrations_applied: 0, wal: false },
      model: { available: false, reason: 'NO_RUNTIME_CONFIGURED' },
      network: { egress_allowed: false },
      clock: { now: this.now(), zone: this.zone, monotonic_ok: true },
      transport: 'mock',
      pending_approvals: [...this.approvals.values()].filter((a) => a.state === 'PENDING').length,
      last_backup: last === undefined ? null : { backup_id: last.backup_id, state: last.state, created_at: last.created_at },
    };
  }

  private curriculumValidate(payload: unknown): ResponseOf<'curriculum.validate'> {
    const rec = asRecord(payload, 'curriculum.validate');
    const manifest = json(rec, 'manifest', 'curriculum.validate');
    const violations = validateManifest(manifest);
    const provenance =
      typeof manifest === 'object' && manifest !== null && !Array.isArray(manifest) && isProvenance(manifest.provenance) ? manifest.provenance : null;
    return { valid: violations.length === 0, violations, checksum_sha256: sha256Hex(canonicalJson(manifest)), provenance };
  }

  /**
   * Mock of `curriculum.import`. Mirrors the backend contract: the manifest is checked first
   * (INVALID_INPUT before any approval), then policy rule `operator.import.curriculum.approval`
   * answers APPROVAL_REQUIRED(CURRICULUM_IMPORT) bound to the manifest identity and checksum.
   */
  private curriculumImport(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'curriculum.import'>> {
    const op = 'curriculum.import';
    const rec = asRecord(payload, op);
    const manifest = json(rec, 'manifest', op);
    const provenance = oneOf(rec, 'provenance', ['PROPOSED_SEED', 'RECONCILED_CANDIDATE', 'APPROVED_CANONICAL'] as const, op);
    if (provenance === 'APPROVED_CANONICAL') {
      throw fail('UNAUTHORIZED', 'curriculum.promote_requires_approval', 'importing directly as APPROVED_CANONICAL is denied; use PROMOTE_CANONICAL');
    }
    const violations = validateManifest(manifest);
    if (violations.length > 0) {
      throw fail('INVALID_INPUT', 'curriculum.invariant_violated', 'manifest violates curriculum invariants', {
        violations: violations.map((x): JsonValue => ({ code: x.code, path: x.path, message: x.message })),
      });
    }
    const m = manifest as Rec;
    const versionId = `${String(m.curriculum_id)}:${String(m.version)}`;
    const checksum = sha256Hex(canonicalJson(manifest));
    const resource: ResourceRef = { kind: 'CURRICULUM_VERSION', id: versionId };
    this.requireApproval(op, { curriculum_version_id: versionId, checksum_sha256: checksum, provenance }, resource, 'CURRICULUM_IMPORT', meta, `Import ${versionId} as ${provenance}`);
    const weeks = Array.isArray(m.weeks) ? m.weeks : [];
    let days = 0;
    let objectives = 0;
    let plannedMinutes = 0;
    for (const w of weeks) {
      const ds = typeof w === 'object' && w !== null && Array.isArray((w as Rec).days) ? ((w as Rec).days as unknown[]) : [];
      days += ds.length;
      for (const d of ds) {
        objectives += typeof d === 'object' && d !== null && Array.isArray((d as Rec).objectives) ? ((d as Rec).objectives as unknown[]).length : 0;
        plannedMinutes += typeof d === 'object' && d !== null && typeof (d as Rec).planned_minutes === 'number' ? ((d as Rec).planned_minutes as number) : 0;
      }
    }
    // The active version is the current canonical one, else the latest import (g9-app `active_version`).
    if (this.program.curriculum?.provenance !== 'APPROVED_CANONICAL') {
      this.program.curriculum = {
        version_id: versionId,
        name: typeof m.name === 'string' ? m.name : null,
        version_label: String(m.version),
        provenance,
        weeks: weeks.length,
        days,
        objectives,
        planned_minutes: plannedMinutes,
        imported_at: this.now(),
      };
    }
    return {
      result: { curriculum_version_id: versionId, checksum_sha256: checksum, provenance, weeks: weeks.length, days, objectives },
      revision: 1,
      resource,
    };
  }

  /**
   * Mock of `curriculum.promote`. Mirrors the backend contract: approval-gated
   * (CURRICULUM_PROMOTE), both manifests must satisfy the invariants, and
   * promotion is allowed only when the day-by-day reconciliation shows no
   * changed, missing or added days. The mock never invents a canonical manifest.
   */
  private curriculumPromote(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'curriculum.promote'>> {
    const op = 'curriculum.promote';
    const rec = asRecord(payload, op);
    const versionId = str(rec, 'curriculum_version_id', op);
    const candidate = json(rec, 'candidate_manifest', op);
    const original = json(rec, 'original_manifest', op);
    for (const [label, m] of [['candidate_manifest', candidate], ['original_manifest', original]] as const) {
      const violations = validateManifest(m);
      if (violations.length > 0) {
        throw fail('INVALID_INPUT', 'curriculum.invariant_violated', `${label} violates curriculum invariants`, {
          field: label,
          violations: violations.map((x): JsonValue => ({ code: x.code, path: x.path, message: x.message })),
        });
      }
    }
    const daysOf = (m: JsonValue): Map<string, string> => {
      const out = new Map<string, string>();
      const weeks = typeof m === 'object' && m !== null && !Array.isArray(m) && Array.isArray(m.weeks) ? m.weeks : [];
      for (const w of weeks) {
        const ds = typeof w === 'object' && w !== null && !Array.isArray(w) && Array.isArray(w.days) ? w.days : [];
        for (const d of ds) {
          if (typeof d === 'object' && d !== null && !Array.isArray(d) && typeof d.day_id === 'string') {
            out.set(d.day_id, canonicalJson(d));
          }
        }
      }
      return out;
    };
    const o = daysOf(original);
    const c = daysOf(candidate);
    let unchanged = 0;
    let changed = 0;
    let missing = 0;
    for (const [id, body] of o) {
      const cb = c.get(id);
      if (cb === undefined) missing += 1;
      else if (cb === body) unchanged += 1;
      else changed += 1;
    }
    const added = [...c.keys()].filter((id) => !o.has(id)).length;
    const summary = { unchanged, changed, missing_in_candidate: missing, added_in_candidate: added, metadata_changes: [] as string[] };
    const promotionAllowed = changed === 0 && missing === 0 && added === 0;
    const resource: ResourceRef = { kind: 'CURRICULUM_VERSION', id: versionId };
    if (!promotionAllowed) {
      throw fail('PRECONDITION_FAILED', 'curriculum.reconciliation_blocks_promotion', 'candidate does not reconcile with the original plan', {
        summary: { unchanged, changed, missing_in_candidate: missing, added_in_candidate: added },
      });
    }
    this.requireApproval(op, { curriculum_version_id: versionId }, resource, 'CURRICULUM_PROMOTE', meta, `Promote ${versionId} to APPROVED_CANONICAL`);
    const originalChecksum = sha256Hex(canonicalJson(original));
    const candidateChecksum = sha256Hex(canonicalJson(candidate));
    return {
      result: {
        curriculum_version_id: versionId,
        provenance: 'APPROVED_CANONICAL',
        promotion_allowed: true,
        original_checksum: originalChecksum,
        candidate_checksum: candidateChecksum,
        diff_checksum: sha256Hex(canonicalJson({ original: originalChecksum, candidate: candidateChecksum, summary })),
        summary,
      },
    };
  }

  private dailyPlan(payload: unknown): Handled<DailyPlan> {
    // Same disclosure as g9-app commands/curriculum.rs: a plan from a non-canonical version says so.
    const result = this.dailyPlanResult(payload);
    const warnings = this.program.curriculum?.provenance === 'APPROVED_CANONICAL' ? [] : [NON_CANONICAL_PLAN_WARNING];
    return { result, warnings };
  }

  private dailyPlanResult(payload: unknown): DailyPlan {
    const rec = asRecord(payload, 'curriculum.daily_plan');
    const dateStr = str(rec, 'date', 'curriculum.daily_plan');
    const date = parseIsoDate(dateStr);
    if (date === null) {
      throw fail('INVALID_INPUT', 'date.invalid', "date must be 'YYYY-MM-DD'", { field: 'date' });
    }
    const zone = optStr(rec, 'zone', 'curriculum.daily_plan') ?? this.zone;
    this.requireProgram();
    const { day, week, weekday } = this.dayForDate(date);
    // Before week 1 or after the last week the backend finds no week row: unscheduled, and no week named.
    if (week < 1 || week > this.fixture.structure.weeks) {
      return {
        date: dateStr,
        zone,
        day_id: null,
        week_number: null,
        week_title: null,
        unscheduled: true,
        is_assessment_day: false,
        planned_minutes: 0,
        evidence_artifact: null,
        objectives: [],
        blocks: [],
      };
    }
    if (weekday === 7 && week === this.fixture.week.week_number) {
      return {
        date: dateStr,
        zone,
        day_id: null,
        week_number: week,
        week_title: this.fixture.week.title,
        unscheduled: true,
        is_assessment_day: false,
        planned_minutes: 0,
        evidence_artifact: null,
        objectives: [],
        blocks: [],
      };
    }
    if (day === null) {
      throw fail('NOT_FOUND', 'curriculum.day_not_in_fixture', `no training day loaded for ${dateStr}`, { week });
    }
    return {
      date: dateStr,
      zone,
      day_id: day.day_id,
      week_number: this.fixture.week.week_number,
      week_title: this.fixture.week.title,
      unscheduled: false,
      is_assessment_day: day.is_assessment_day,
      planned_minutes: day.planned_minutes,
      evidence_artifact: day.evidence_artifact,
      objectives: day.objectives.map((o) => ({ objective_id: o.objective_id, seq: o.seq, text: o.text, status: 'PLANNED' })),
      blocks: day.blocks.map((b) => ({
        seq: b.seq,
        block: b.block,
        minutes: b.minutes,
        session_id: this.sessionOfBlock(day.day_id, b.seq, dateStr)?.session_id ?? null,
      })),
    };
  }

  private calendarList(payload: unknown): ResponseOf<'calendar.list'> {
    const rec = asRecord(payload, 'calendar.list');
    const from = int(rec, 'from', 'calendar.list', 0);
    const to = int(rec, 'to', 'calendar.list', 0);
    if (to < from) {
      throw fail('INVALID_INPUT', 'range.inverted', "'to' must not precede 'from'");
    }
    const zone = optStr(rec, 'zone', 'calendar.list') ?? this.zone;
    return { zone, events: this.events.filter((e) => e.start < to && e.end > from).sort((a, b) => a.start - b.start) };
  }

  private calendarPropose(payload: unknown): Handled<ScheduleProposal> {
    const op = 'calendar.propose';
    const rec = asRecord(payload, op);
    const from = int(rec, 'from', op, 0);
    const to = int(rec, 'to', op, 0);
    const intent = oneOf(rec, 'intent', ['FILL_CURRICULUM', 'ADD_REVIEWS', 'RESCHEDULE'] as const, op);
    if (intent === 'RESCHEDULE') {
      throw fail('PRECONDITION_FAILED', 'calendar.reschedule.not_implemented', 'rescheduling proposals land with g9-calendar::proposal');
    }
    const changes: ScheduleProposal['changes'] = [];
    const violated: ScheduleProposal['violated_constraints'] = [];
    if (intent === 'ADD_REVIEWS') {
      // g9-app `calendar.propose`: every due item sizes the block (no silent truncation).
      const due = this.retentionDue({}).result;
      const items = [...due.overdue, ...due.due, ...due.overflow];
      if (items.length > 0) {
        const tomorrow = addDays(civilDateOf(this.now(), this.zone), 1);
        const start = civilToUtc({ ...tomorrow, hour: 8, minute: 30 }, this.zone);
        const end = start.utc + items.length * REVIEW_ITEM_MS;
        const event: CalendarEvent = {
          event_id: this.nextId('event'),
          title: `Review queue (${items.length} items)`,
          kind: 'REVIEW',
          start: start.utc,
          end,
          all_day: false,
          immutable: false,
          day_id: null,
          block: null,
          session_id: null,
          adjusted: start.adjusted,
          provenance: 'SYSTEM_DETERMINISTIC',
        };
        for (const existing of this.events) {
          if (existing.start < event.end && existing.end > event.start) {
            violated.push({ code: 'OVERLAP', message: `overlaps ${existing.title}`, event_id: existing.event_id });
          }
        }
        changes.push({ op: 'CREATE', event });
      }
    }
    const proposal: ScheduleProposal = {
      proposal_id: this.nextId('proposal'),
      feasible: violated.length === 0,
      violated_constraints: violated,
      changes,
      parameters_sha256: sha256Hex(canonicalJson({ from, to, intent, changes })),
      created_at: this.now(),
      expires_at: this.now() + APPROVAL_TTL_MS,
      revision: 1,
    };
    this.proposals.set(proposal.proposal_id, proposal);
    return { result: proposal, revision: 1, resource: { kind: 'SCHEDULE_PROPOSAL', id: proposal.proposal_id } };
  }

  private calendarApply(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'calendar.apply'>> {
    const op = 'calendar.apply';
    const rec = asRecord(payload, op);
    const proposalId = str(rec, 'proposal_id', op);
    const proposal = this.proposals.get(proposalId);
    if (proposal === undefined) {
      throw fail('NOT_FOUND', 'proposal.not_found', 'schedule proposal not found');
    }
    if (!proposal.feasible) {
      throw fail('PRECONDITION_FAILED', 'proposal.infeasible', 'an infeasible proposal cannot be applied', {
        violated: proposal.violated_constraints.map((v) => v.code),
      });
    }
    if (this.now() > proposal.expires_at) {
      throw fail('PRECONDITION_FAILED', 'proposal.expired', 'proposal expired; propose again');
    }
    const resource: ResourceRef = { kind: 'SCHEDULE_PROPOSAL', id: proposalId };
    this.requireApproval(op, { proposal_id: proposalId }, resource, 'SCHEDULE_APPLY', meta, `Apply schedule proposal (${proposal.changes.length} changes)`);
    const applied: CalendarEvent[] = [];
    // g9-app `calendar.apply` writes the placements with source SCHEDULER: they are listed as
    // SYSTEM_DETERMINISTIC and, like every event the operator did not add, cannot be edited.
    for (const change of proposal.changes) {
      if (change.op === 'CREATE') {
        this.events.push({ ...change.event, provenance: 'SYSTEM_DETERMINISTIC' });
        applied.push(change.event);
      } else if (change.op === 'DELETE') {
        this.events = this.events.filter((e) => e.event_id !== change.event.event_id);
        applied.push(change.event);
      } else {
        this.events = this.events.map((e) => (e.event_id === change.event.event_id ? { ...change.event, provenance: 'SYSTEM_DETERMINISTIC' } : e));
        applied.push(change.event);
      }
    }
    proposal.revision += 1;
    return { result: { proposal_id: proposalId, applied: applied.length, events: applied }, revision: proposal.revision, resource };
  }

  private sessionList(payload: unknown): ResponseOf<'session.list'> {
    const rec = asRecord(payload, 'session.list');
    const date = optStr(rec, 'date', 'session.list');
    if (date !== undefined && parseIsoDate(date) === null) {
      throw fail('INVALID_INPUT', 'date.invalid', "date must be 'YYYY-MM-DD'", { field: 'date' });
    }
    const sessions = [...this.sessions.values()]
      .filter((s) => date === undefined || s.date === date)
      .sort((a, b) => (a.date === b.date ? a.block_seq - b.block_seq : a.date.localeCompare(b.date)))
      .map(stripSession);
    return { sessions };
  }

  private findSession(rec: Rec, op: string): SessionRecord {
    const id = str(rec, 'session_id', op);
    const s = this.sessions.get(id);
    if (s === undefined) {
      throw fail('NOT_FOUND', 'session.not_found', 'session not found', { session_id: id });
    }
    return s;
  }

  private sessionEvent(payload: unknown, event: SessionEvent, meta: CommandMeta, op: string): Handled<SessionView> {
    const rec = asRecord(payload, op);
    const session = this.findSession(rec, op);
    this.checkRevision(meta, session.revision);
    const next = applyMachine(SESSION_MACHINE, session.state, event, 'session');
    const now = this.now();
    if (event === 'START' || event === 'RESUME') {
      session.intervals.push({ interval_id: `${session.session_id}-i${session.intervals.length + 1}`, start_utc: now, end_utc: null, continuity: 'VERIFIED' });
      session.open_interval_started_at = now;
      session.started_at ??= now;
    }
    if (event === 'PAUSE' || event === 'COMPLETE') {
      const open = session.intervals.find((i) => i.end_utc === null);
      if (open !== undefined) {
        open.end_utc = now;
      }
      session.open_interval_started_at = null;
    }
    session.state = next;
    recomputeLedger(session);
    session.revision += 1;
    return { result: stripSession(session), revision: session.revision, resource: { kind: 'STUDY_SESSION', id: session.session_id } };
  }

  /** See `MockTransport.onProcessRestart`. */
  onProcessRestart(): Id[] {
    const now = this.now();
    const affected: Id[] = [];
    for (const session of this.sessions.values()) {
      const open = session.intervals.find((i) => i.end_utc === null);
      if (open === undefined) {
        continue;
      }
      open.end_utc = now;
      open.continuity = 'UNCERTAIN';
      session.open_interval_started_at = null;
      session.state = applyMachine(SESSION_MACHINE, session.state, 'INTERRUPT', 'session');
      recomputeLedger(session);
      session.revision += 1;
      affected.push(session.session_id);
    }
    return affected;
  }

  private sessionCorrectTime(payload: unknown, meta: CommandMeta): Handled<SessionView> {
    const op = 'session.correct_time';
    const rec = asRecord(payload, op);
    const session = this.findSession(rec, op);
    const intervalId = str(rec, 'interval_id', op);
    const credited = int(rec, 'credited_ms', op, 0);
    const reason = str(rec, 'reason', op);
    this.checkRevision(meta, session.revision);
    const interval = session.intervals.find((i) => i.interval_id === intervalId);
    if (interval === undefined) {
      throw fail('NOT_FOUND', 'interval.not_found', 'time interval not found', { interval_id: intervalId });
    }
    if (interval.continuity !== 'UNCERTAIN' || interval.end_utc === null) {
      throw fail('PRECONDITION_FAILED', 'interval.not_correctable', 'only closed UNCERTAIN intervals accept a correction');
    }
    if (credited > interval.end_utc - interval.start_utc) {
      throw fail('INVALID_INPUT', 'correction.exceeds_interval', 'credited time cannot exceed the uncertain interval length', {
        max_ms: interval.end_utc - interval.start_utc,
      });
    }
    // g9-app `correct_time`: a correction is accepted in every session state (policy rule
    // `operator.correct.allow` has no finalized condition; the mode is never EXAM for a study
    // session), supersedes the credit of the named interval, and recovers an INTERRUPTED
    // session only once no closed UNCERTAIN interval is left without a correction.
    session.corrections.push({ correction_id: this.nextId('correction'), interval_id: intervalId, credited_ms: credited, reason, corrected_at: this.now() });
    const remaining = uncorrectedUncertain(session).map((i) => i.interval_id);
    const warnings: string[] = [];
    if (session.state === 'INTERRUPTED') {
      if (remaining.length > 0) {
        warnings.push(`${UNCORRECTED_UNCERTAIN_WARNING} ${remaining.join(', ')}`);
      } else {
        session.state = applyMachine(SESSION_MACHINE, session.state, 'RECOVER', 'session');
      }
    }
    recomputeLedger(session);
    session.revision += 1;
    return { result: stripSession(session), revision: session.revision, resource: { kind: 'TIME_INTERVAL', id: intervalId }, warnings };
  }

  private notesCreate(payload: unknown, meta: CommandMeta): Handled<NoteView> {
    const op = 'notes.create';
    const rec = asRecord(payload, op);
    const title = str(rec, 'title', op);
    const body = typeof rec.body_markdown === 'string' ? rec.body_markdown : null;
    if (body === null) {
      throw schemaViolation('body_markdown', `${op}: 'body_markdown' must be a string`);
    }
    const classification = oneOf(rec, 'classification', CLASSIFICATIONS, op);
    const domain = oneOf(rec, 'domain', DATA_DOMAINS, op);
    const objective = optStr(rec, 'objective_id', op) ?? null;
    const evidence = strArray(rec, 'evidence_refs', op, true);
    const noteId = this.creationId(meta, op, 'note');
    this.requireSensitiveWriteApproval(op, domain, asJson(payload), { kind: 'NOTE', id: noteId }, meta);
    const now = this.now();
    const note: NoteView = {
      note_id: noteId,
      title,
      body_markdown: body,
      content_sha256: sha256Hex(body),
      classification,
      domain,
      objective_id: objective,
      evidence_refs: evidence,
      version_count: 1,
      created_at: now,
      updated_at: now,
      revision: 1,
    };
    this.notes.set(note.note_id, note);
    return { result: note, revision: 1, resource: { kind: 'NOTE', id: note.note_id }, warnings: evidence.length > 0 ? [NOTE_EVIDENCE_NOT_PERSISTED_WARNING] : [] };
  }

  private notesRevise(payload: unknown, meta: CommandMeta): Handled<NoteView> {
    const op = 'notes.revise';
    const rec = asRecord(payload, op);
    const id = str(rec, 'note_id', op);
    const note = this.notes.get(id);
    if (note === undefined) {
      throw fail('NOT_FOUND', 'note.not_found', 'note not found', { note_id: id });
    }
    if (note.domain === 'SECURITY_SENSITIVE') {
      const resource: ResourceRef = { kind: 'NOTE', id };
      this.requireApproval(op, asJson(payload), resource, 'PROTECTED_RECORD_WRITE', meta, approvalSummary(op, resource, 'PROTECTED_RECORD_WRITE'), note.revision);
    }
    this.checkRevision(meta, note.revision);
    const body = typeof rec.body_markdown === 'string' ? rec.body_markdown : null;
    if (body === null) {
      throw schemaViolation('body_markdown', `${op}: 'body_markdown' must be a string`);
    }
    const title = optStr(rec, 'title', op);
    note.body_markdown = body;
    note.content_sha256 = sha256Hex(body);
    if (title !== undefined) {
      note.title = title;
    }
    note.version_count += 1;
    note.updated_at = this.now();
    note.revision += 1;
    return { result: note, revision: note.revision, resource: { kind: 'NOTE', id } };
  }

  private assessmentList(): ResponseOf<'assessment.list'> {
    return {
      definitions: this.definitions.map((d) => ({ ...d })),
      attempts: [...this.attempts.values()].sort((a, b) => (a.started_at ?? 0) - (b.started_at ?? 0)),
    };
  }

  private findAttempt(rec: Rec, op: string): AttemptView {
    const id = str(rec, 'attempt_id', op);
    const a = this.attempts.get(id);
    if (a === undefined) {
      throw fail('NOT_FOUND', 'attempt.not_found', 'attempt not found', { attempt_id: id });
    }
    return a;
  }

  private assessmentBegin(payload: unknown): Handled<AttemptView> {
    const op = 'assessment.begin';
    const rec = asRecord(payload, op);
    const defId = str(rec, 'assessment_id', op);
    const def = this.definitions.find((d) => d.assessment_id === defId);
    if (def === undefined) {
      throw fail('NOT_FOUND', 'assessment.not_found', 'assessment definition not found', { assessment_id: defId });
    }
    const retakeOf = optStr(rec, 'retake_of', op) ?? null;
    if (retakeOf !== null && !this.attempts.has(retakeOf)) {
      throw fail('NOT_FOUND', 'attempt.not_found', 'retake_of attempt not found', { attempt_id: retakeOf });
    }
    const open = [...this.attempts.values()].find((a) => a.assessment_id === defId && !['APPROVED_FINAL', 'VOIDED'].includes(a.state));
    if (open !== undefined) {
      throw fail('CONFLICT', 'attempt.already_open', 'an attempt for this assessment is already in progress', { attempt_id: open.attempt_id });
    }
    const state = applyMachine(ASSESSMENT_MACHINE, 'AVAILABLE', 'BEGIN', 'assessment_attempt');
    const attempt: AttemptView = {
      attempt_id: this.nextId('attempt'),
      assessment_id: defId,
      title: def.title,
      state,
      mode: def.mode,
      // Variant selection is exposure-ledger driven in the backend (Addendum A.3); the mock alternates.
      variant: retakeOf === null ? 'A' : 'B',
      retake_of: retakeOf,
      started_at: this.now(),
      submitted_at: null,
      finalized_at: null,
      grade: null,
      revision: 1,
    };
    this.attempts.set(attempt.attempt_id, attempt);
    return { result: attempt, revision: 1, resource: { kind: 'ATTEMPT', id: attempt.attempt_id } };
  }

  private assessmentSubmit(payload: unknown, meta: CommandMeta): Handled<AttemptView> {
    const op = 'assessment.submit';
    const rec = asRecord(payload, op);
    const attempt = this.findAttempt(rec, op);
    const refs = strArray(rec, 'evidence_refs', op);
    this.checkRevision(meta, attempt.revision);
    const next = applyMachine(ASSESSMENT_MACHINE, attempt.state, 'SUBMIT', 'assessment_attempt');
    // g9-app links every item as SUBMISSION evidence (`attempt_evidence` → `evidence_items`
    // FOREIGN KEY): an id that names no evidence item fails the whole command.
    if (refs.some((r) => !this.evidence.has(r))) {
      throw fail('PRECONDITION_FAILED', 'storage.foreign_key', 'foreign key constraint failed: FOREIGN KEY constraint failed');
    }
    attempt.state = next;
    attempt.submitted_at = this.now();
    attempt.revision += 1;
    return { result: attempt, revision: attempt.revision, resource: { kind: 'ATTEMPT', id: attempt.attempt_id } };
  }

  private assessmentEvent(payload: unknown, event: AssessmentEvent, meta: CommandMeta, op: string): Handled<AttemptView> {
    const rec = asRecord(payload, op);
    const attempt = this.findAttempt(rec, op);
    this.checkRevision(meta, attempt.revision);
    attempt.state = applyMachine(ASSESSMENT_MACHINE, attempt.state, event, 'assessment_attempt');
    attempt.revision += 1;
    return { result: attempt, revision: attempt.revision, resource: { kind: 'ATTEMPT', id: attempt.attempt_id } };
  }

  private assessmentGrade(payload: unknown, meta: CommandMeta): Handled<AttemptView> {
    const op = 'assessment.grade_provisional';
    const rec = asRecord(payload, op);
    const attempt = this.findAttempt(rec, op);
    this.checkRevision(meta, attempt.revision);
    const rawCriteria: unknown = rec.criteria;
    if (!Array.isArray(rawCriteria)) {
      throw schemaViolation('criteria', `${op}: 'criteria' must be an array`);
    }
    const raw: Rec[] = rawCriteria.map((r): Rec => {
      if (typeof r !== 'object' || r === null || Array.isArray(r)) {
        throw schemaViolation('criteria', `${op}: 'criteria[]' must be objects`);
      }
      return r as Rec;
    });
    // Same checks and order as g9-app commands/assessment.rs grade_provisional:
    // unknown code → level range → every rubric criterion present → source/actor provenance.
    for (const e of raw) {
      const code = str(e, 'criterion_code', op);
      if (!BASELINE_CRITERIA.some((c) => c.code === code)) {
        throw fail('INVALID_INPUT', 'grade.criterion_unknown', 'criterion is not part of this rubric', { criterion_code: code });
      }
      const level = int(e, 'score', op, 0);
      if (level > 4) {
        throw fail('INVALID_INPUT', 'grade.level_out_of_range', 'criterion score is an anchored level 0..=4', { criterion_code: code, level });
      }
    }
    const results: CriterionResult[] = [];
    for (const c of BASELINE_CRITERIA) {
      const e = raw.find((r) => r.criterion_code === c.code);
      if (e === undefined) {
        throw fail('INVALID_INPUT', 'grade.criterion_missing', 'every rubric criterion needs a result', { criterion_code: c.code });
      }
      const source = oneOf(e, 'source', ['DETERMINISTIC_CHECKER', 'HUMAN_REVIEWER', 'MODEL_ADVISORY'] as const satisfies readonly GradeSource[], op);
      // The operator may record HUMAN_REVIEWER (or advisory) results; a DETERMINISTIC_CHECKER
      // result can only come from a System checker (g9-assessment CriterionResult::new).
      if (source === 'DETERMINISTIC_CHECKER') {
        throw fail('UNAUTHORIZED', 'assessment.grade.source_actor_mismatch', 'grade source DETERMINISTIC_CHECKER cannot be recorded by an OPERATOR actor');
      }
      results.push({ criterion_code: c.code, weight: c.weight, score: int(e, 'score', op, 0), source, evidence_refs: strArray(e, 'evidence_refs', op, true) });
    }
    attempt.state = applyMachine(ASSESSMENT_MACHINE, attempt.state, 'GRADE_PROVISIONAL', 'assessment_attempt');
    attempt.grade = computeGrade(results);
    attempt.revision += 1;
    return { result: attempt, revision: attempt.revision, resource: { kind: 'GRADE_DECISION', id: attempt.attempt_id } };
  }

  private assessmentApproveFinal(payload: unknown, meta: CommandMeta): Handled<AttemptView> {
    const op = 'assessment.approve_final';
    const rec = asRecord(payload, op);
    const attempt = this.findAttempt(rec, op);
    this.checkRevision(meta, attempt.revision);
    if (attempt.state !== 'GRADED_PROVISIONAL' || attempt.grade === null) {
      throw fail('PRECONDITION_FAILED', 'assessment.illegal_transition', `cannot finalize from ${attempt.state}`, {
        from: attempt.state,
        event: 'APPROVE_FINAL',
      });
    }
    if (attempt.grade.criteria.some((c) => c.source === 'MODEL_ADVISORY')) {
      throw fail('PRECONDITION_FAILED', 'grade.advisory_cannot_finalize', 'every criterion needs a non-advisory result before finalization');
    }
    const resource: ResourceRef = { kind: 'GRADE_DECISION', id: attempt.attempt_id };
    this.requireApproval(op, { attempt_id: attempt.attempt_id }, resource, 'GRADE_FINALIZE', meta, `Finalize grade for ${attempt.title}`);
    attempt.state = applyMachine(ASSESSMENT_MACHINE, attempt.state, 'APPROVE_FINAL', 'assessment_attempt');
    attempt.grade = { ...attempt.grade, final: true };
    attempt.finalized_at = this.now();
    attempt.revision += 1;
    // Same step as the backend finalization transaction (g9-app `finalize_attempt`).
    const notDerived = this.deriveObservations(attempt, attempt.grade);
    return { result: attempt, revision: attempt.revision, resource, warnings: notDerived === null ? [] : [observationsNotDerivedWarning(notDerived)] };
  }

  private defineAssessment(def: AssessmentDefinition, at: 'push' | 'unshift'): void {
    if (at === 'push') {
      this.definitions.push(def);
    } else {
      this.definitions.unshift(def);
    }
    if (!this.undeclared.has(def.assessment_id)) {
      this.declaredDifficulty.set(def.assessment_id, FIXTURE_DECLARED_DIFFICULTY);
    }
  }

  /**
   * Port of g9-app `derive_observations` (over g9-assessment `observations_from_decision`): one
   * observation per dimension that has criteria, scored by the weighted mean of that dimension's
   * criteria (rounded half up), carrying the difficulty the version declared. The attempt is EXAM
   * mode and unaided; the operator finalized their own grade, so the decision is SELF_ADJUDICATED and
   * no observation is validation-eligible (see `projectObservations`). Returns why nothing was
   * derived, or `null` when observations were appended.
   */
  private deriveObservations(attempt: AttemptView, grade: GradeView): string | null {
    const difficulty = this.declaredDifficulty.get(attempt.assessment_id);
    if (difficulty === undefined) {
      return 'the assessment version declares no difficulty';
    }
    const groups = new Map<LearningDimension, { num: number; den: number }>();
    for (const c of grade.criteria) {
      const dimension = BASELINE_DIMENSION[c.criterion_code];
      if (dimension === undefined) {
        continue;
      }
      const acc = groups.get(dimension) ?? { num: 0, den: 0 };
      acc.num += c.weight * (LEVEL_SCALE[c.score] ?? 0);
      acc.den += c.weight;
      groups.set(dimension, acc);
    }
    if (groups.size === 0) {
      return 'the decision maps to no competency dimension';
    }
    const observedAt = this.now();
    for (const dimension of LEARNING_DIMENSIONS) {
      const acc = groups.get(dimension);
      if (acc === undefined) {
        continue;
      }
      this.observations.push({
        observation_id: this.nextId('observation'),
        competency_id: this.competencyOf(attempt.assessment_id),
        dimension,
        attempt_id: attempt.attempt_id,
        difficulty,
        score: acc.den === 0 ? 0 : Math.floor((2 * acc.num + acc.den) / (2 * acc.den)),
        observed_at: observedAt,
      });
    }
    return null;
  }

  private competencyProjection(payload: unknown): ResponseOf<'competency.projection'> {
    const rec = asRecord(payload, 'competency.projection');
    const only = optStr(rec, 'competency_id', 'competency.projection');
    const projections: CompetencyProjection[] = this.knownCompetencies().map((c) => ({
      competency_id: c.id,
      name: c.name,
      // The fixture seeds no observations: only `assessment.approve_final` appends any, so a
      // competency nothing was finalized for is honestly UNASSESSED with empty estimates.
      ...projectObservations(this.observations.filter((o) => o.competency_id === c.id)),
      contradictions: [],
      objective_ids: c.objective_ids,
    }));
    const filtered = only === undefined ? projections : projections.filter((p) => p.competency_id === only);
    if (only !== undefined && filtered.length === 0) {
      throw fail('NOT_FOUND', 'competency.not_found', 'competency not found', { competency_id: only });
    }
    return { derived_at: this.now(), projections: filtered };
  }

  /**
   * Port of g9-app `retention.due`: every item due at `as_of` (`due_at <= as_of`, no row limit),
   * planned like g9-retention `plan_reviews` (most overdue first, then the lower stage, then the
   * id) into the budget; what does not fit is `overflow`, never dropped. Overdue = due before
   * local midnight of `as_of`.
   */
  private retentionDue(payload: unknown): Handled<ResponseOf<'retention.due'>> {
    const rec = asRecord(payload, 'retention.due');
    const asOf = optInt(rec, 'as_of', 'retention.due', 0) ?? this.now();
    const budget = optInt(rec, 'budget_ms', 'retention.due', 0) ?? REVIEW_BUDGET_MS;
    const todayStart = startOfDayUtc(civilDateOf(asOf, this.zone), this.zone);
    const due = [...this.reviewItems.values()]
      .filter((r) => r.view.due_at <= asOf)
      .sort((a, b) => a.view.due_at - b.view.due_at || a.stage - b.stage || compareIds(a.view.review_item_id, b.view.review_item_id));
    const capacity = Math.floor(budget / REVIEW_ITEM_MS);
    const inBudget = due.slice(0, capacity).map((r) => ({ ...r.view }));
    const overflow = due.slice(capacity).map((r) => ({ ...r.view }));
    const procedural = due.filter((r) => r.authoredKind === 'PROCEDURAL').length;
    return {
      result: {
        as_of: asOf,
        budget_ms: budget,
        overdue: inBudget.filter((i) => i.due_at < todayStart),
        due: inBudget.filter((i) => i.due_at >= todayStart),
        overflow,
      },
      warnings: procedural > 0 ? [proceduralLabelWarning(procedural)] : [],
    };
  }

  private interventionsList(payload: unknown): ResponseOf<'interventions.list'> {
    const rec = asRecord(payload, 'interventions.list');
    optStr(rec, 'state', 'interventions.list');
    // No model runtime and no operator-created interventions in the fixture: an empty list is the truth.
    const interventions: Intervention[] = [];
    return { interventions };
  }

  private approvalsList(payload: unknown): ResponseOf<'approvals.list'> {
    const rec = asRecord(payload, 'approvals.list');
    const only = optStr(rec, 'state', 'approvals.list');
    for (const a of this.approvals.values()) {
      this.expireIfDue(a);
    }
    return {
      approvals: [...this.approvals.values()]
        .filter((a) => only === undefined || a.state === only)
        .sort((a, b) => b.requested_at - a.requested_at)
        .map((a) => ({ ...a })),
    };
  }

  private approvalsDecide(payload: unknown, meta: CommandMeta): Handled<ApprovalRequest> {
    const op = 'approvals.decide';
    const rec = asRecord(payload, op);
    const id = str(rec, 'approval_id', op);
    const decision = oneOf(rec, 'decision', ['APPROVE', 'REJECT'] as const, op);
    const approval = this.approvals.get(id);
    if (approval === undefined) {
      throw fail('NOT_FOUND', 'approval.not_found', 'approval not found', { approval_id: id });
    }
    if (decision === 'APPROVE' && !this.confirmApproval(id)) {
      throw fail('UNAUTHORIZED', 'approval.not_confirmed', 'the approval was not confirmed');
    }
    this.checkRevision(meta, approval.revision);
    this.expireIfDue(approval);
    this.applyApprovalEvent(approval, decision);
    approval.decided_at = this.now();
    return { result: { ...approval }, revision: approval.revision, resource: { kind: 'APPROVAL', id } };
  }

  private auditRecent(payload: unknown): ResponseOf<'audit.recent'> {
    const rec = asRecord(payload, 'audit.recent');
    const limit = optInt(rec, 'limit', 'audit.recent', 1) ?? 50;
    const before = optInt(rec, 'before', 'audit.recent', 0);
    const entries = this.audit
      .filter((e) => before === undefined || e.occurred_at < before)
      .slice(-limit)
      .reverse();
    return { entries, chain_ok: this.verifyChain() };
  }

  private backupCreate(payload: unknown): Handled<BackupView> {
    const op = 'backup.create';
    const rec = asRecord(payload, op);
    const label = optStr(rec, 'label', op) ?? null;
    // Requested → SnapshotCreated inside one command (Online Backup API run to completion).
    let state = applyMachine(BACKUP_MACHINE, 'REQUESTED', 'SNAPSHOT', 'backup');
    state = state === 'SNAPSHOT_CREATED' ? state : 'FAILED';
    const backup: BackupView = {
      backup_id: this.nextId('backup'),
      state,
      label,
      created_at: this.now(),
      size_bytes: null,
      integrity: 'UNKNOWN',
      restore_tested_at: null,
      manifest_sha256: sha256Hex(canonicalJson({ fixture: this.fixture.curriculum_version_id, at: this.now() })),
      revision: 2,
    };
    this.backups.set(backup.backup_id, backup);
    return { result: { ...backup }, revision: backup.revision, resource: { kind: 'BACKUP', id: backup.backup_id } };
  }

  private backupEvent(payload: unknown, event: BackupEvent, meta: CommandMeta, op: string): Handled<BackupView> {
    const rec = asRecord(payload, op);
    const id = str(rec, 'backup_id', op);
    const backup = this.backups.get(id);
    if (backup === undefined) {
      throw fail('NOT_FOUND', 'backup.not_found', 'backup not found', { backup_id: id });
    }
    this.checkRevision(meta, backup.revision);
    backup.state = applyMachine(BACKUP_MACHINE, backup.state, event, 'backup');
    if (event === 'CHECK') {
      backup.integrity = 'OK';
    }
    if (event === 'TEST_RESTORE') {
      backup.restore_tested_at = this.now();
    }
    backup.revision += 1;
    return { result: { ...backup }, revision: backup.revision, resource: { kind: 'BACKUP', id } };
  }

  private skillsInvoke(payload: unknown, meta: CommandMeta): Handled<SkillOutputEnvelope> {
    const op = 'skills.invoke';
    const rec = asRecord(payload, op);
    const code = str(rec, 'skill_code', op);
    json(rec, 'input', op);
    const mode = optOneOf(rec, 'mode', INTERACTION_MODES, op) ?? meta.scope.mode;
    if (!/^(?:[ETLPK]\d{2}|g9-[a-z0-9-]+)$/.test(code)) {
      throw fail('INVALID_INPUT', 'skill.code_invalid', 'skill code must be E01–K03 or an Agent Skills name', { field: 'skill_code' });
    }
    const invocationId = this.nextId('invocation');
    // Preflight: policy denies teaching/hint skills in EXAM mode regardless of runtime.
    if (mode === 'EXAM' && (code.startsWith('T') || code.startsWith('L'))) {
      throw fail('UNAUTHORIZED', 'policy.hints_denied_in_exam', 'teaching skills are denied in EXAM mode', { invocation_id: invocationId, state: 'DENIED' });
    }
    // No inference runtime is configured: NullRuntime behaviour (ARCHITECTURE §10).
    throw fail('RESOURCE_UNAVAILABLE', 'inference.runtime_unavailable', 'no model runtime is configured; skill invocation cannot run', {
      invocation_id: invocationId,
      state: 'ERROR',
      skill_code: code,
    });
  }

  /**
   * Mock of g9-app `export.request` (privacy.rs): approval-gated (DATA_EXPORT); the export
   * directory is checked after the approval, so an install without one refuses the approved
   * retry with `export.dir_not_configured` and the approval is not spent (rolled back). The
   * approved command "writes" `<exports>/<export_id>/` from the mock's own rows (see
   * `writeExport`) and answers READY with the folder, counts and manifest hash.
   */
  private exportRequest(payload: unknown, meta: CommandMeta): Handled<ExportResult> {
    const op = 'export.request';
    const rec = asRecord(payload, op);
    const domainsRaw = rec.domains;
    if (!Array.isArray(domainsRaw) || domainsRaw.length === 0 || !domainsRaw.every((d) => (DATA_DOMAINS as readonly string[]).includes(String(d)))) {
      throw schemaViolation('domains', `${op}: 'domains' must be a non-empty array of data domains`);
    }
    const requested = domainsRaw as DataDomain[];
    const format = oneOf(rec, 'format', ['JSON', 'CSV'] as const, op);
    const includePrivate = rec.include_private;
    if (typeof includePrivate !== 'boolean') {
      throw schemaViolation('include_private', `${op}: 'include_private' must be a boolean`);
    }
    // privacy.rs: the selection is a set, labelled in `DataDomain` order.
    const domains = DATA_DOMAINS.filter((d) => requested.includes(d));
    // The resource names the selection, the privacy ceiling and the format (shown in the native confirmation).
    const resource: ResourceRef = { kind: 'EXPORT', id: `export:${domains.join('.')}:${includePrivate ? 'PRIVATE' : 'INTERNAL'}:${format}` };
    this.requireApproval(op, asJson(payload), resource, 'DATA_EXPORT', meta, `Export ${domains.join(', ')} as ${format}`);
    if (this.exportDir === null) {
      throw fail('PRECONDITION_FAILED', 'export.dir_not_configured', 'no export directory is configured');
    }
    const exportId = this.nextId('export');
    const written = this.writeExport(exportId, domains, format, includePrivate);
    return {
      result: {
        export_id: exportId,
        state: 'READY',
        domains,
        format,
        requested_at: this.now(),
        location: `${this.exportDir}/${exportId}`,
        files: written.files,
        rows: written.rows,
        attachments: written.attachments,
        manifest_sha256: written.manifestSha256,
      },
      revision: 1,
      resource: { kind: 'EXPORT', id: exportId },
    };
  }

  /**
   * Stand-in for g9-storage `export::collect` + `export::write` over the mock's in-memory rows:
   * every table of the model is listed (empty or not); a row is exported when its domain is
   * selected and its classification is PUBLIC or INTERNAL, or PRIVATE with `include_private`
   * (SECRET never); erased rows are left out. JSON is one `data.json`, CSV one file per table;
   * every exported attachment's bytes are copied (all present in the in-memory store); the
   * manifest (selection, row counts, every file with its SHA-256, not signed) is written last.
   * Nothing touches a disk: the counts and the manifest hash are computed from this model.
   */
  private writeExport(
    exportId: Id,
    domains: readonly DataDomain[],
    format: ExportFormat,
    includePrivate: boolean,
  ): { files: number; rows: number; attachments: number; manifestSha256: string } {
    const exported = (domain: DataDomain, classification: Classification): boolean =>
      domains.includes(domain) && (classification === 'PUBLIC' || classification === 'INTERNAL' || (includePrivate && classification === 'PRIVATE'));
    const learning = exported('LEARNING', 'INTERNAL');
    const sessions = learning ? [...this.sessions.values()] : [];
    const items = learning ? [...this.reviewItems.values()] : [];
    const attachments = [...this.attachments.values()].filter((a) => !a.erased && exported(a.domain, a.classification));
    const tables: { name: string; rows: JsonValue[] }[] = [
      {
        name: 'notes',
        rows: [...this.notes.values()]
          .filter((n) => exported(n.domain, n.classification))
          .map((n) => ({ id: n.note_id, title: n.title, body_sha256: n.content_sha256, data_domain: n.domain, classification: n.classification })),
      },
      {
        name: 'attachments',
        rows: attachments.map((a) => ({ id: a.attachment_id, note_id: a.note_id, media_type: a.media_type, size_bytes: a.size_bytes, content_sha256: a.sha256 })),
      },
      {
        name: 'evidence_items',
        rows: [...this.evidence.values()]
          .filter((e) => exported(e.domain, e.classification))
          .map((e) => ({ id: e.evidence_id, kind: e.kind, note_id: e.note_id, attachment_id: e.attachment_id, content_sha256: e.sha256 })),
      },
      { name: 'study_sessions', rows: sessions.map((s) => ({ id: s.session_id, state: s.state, planned_ms: s.planned_ms, credited_ms: s.credited_ms })) },
      { name: 'time_intervals', rows: sessions.flatMap((s) => s.intervals.map((i) => ({ id: i.interval_id, session_id: s.session_id, continuity: i.continuity }))) },
      { name: 'manual_time_corrections', rows: sessions.flatMap((s) => s.corrections.map((c) => ({ id: c.correction_id, interval_id: c.interval_id, credited_ms: c.credited_ms }))) },
      { name: 'calendar_events', rows: learning ? this.events.map((e) => ({ id: e.event_id, kind: e.kind, start_utc: e.start, end_utc: e.end })) : [] },
      { name: 'review_items', rows: items.map((r) => ({ id: r.view.review_item_id, due_at: r.view.due_at, stage: r.stage })) },
      { name: 'review_attempts', rows: items.flatMap((r) => r.attempts.map((a) => ({ review_item_id: r.view.review_item_id, outcome: a.outcome, attempted_at: a.at }))) },
      { name: 'competencies', rows: learning ? this.knownCompetencies().map((c) => ({ id: c.id, title: c.name })) : [] },
      { name: 'assessment_definitions', rows: learning ? this.definitions.map((d) => ({ id: d.assessment_id, title: d.title })) : [] },
      // Attempts are PRIVATE rows (g9-app `assessment.begin`).
      { name: 'attempts', rows: exported('LEARNING', 'PRIVATE') ? [...this.attempts.values()].map((a) => ({ id: a.attempt_id, state: a.state })) : [] },
    ];
    if (domains.includes('SYSTEM')) {
      tables.push(
        { name: 'audit_metadata', rows: this.audit.map((e) => ({ id: e.id, row_hash: e.row_hash })) },
        { name: 'approval_requests', rows: [...this.approvals.values()].map((a) => ({ id: a.approval_id, state: a.state })) },
        { name: 'backup_manifests', rows: [...this.backups.values()].map((b) => ({ id: b.backup_id, state: b.state })) },
      );
    }
    const digests = [...new Set(attachments.map((a) => a.sha256))].sort();
    const files: { path: string; sha256: string }[] =
      format === 'JSON'
        ? [{ path: 'data.json', sha256: sha256Hex(canonicalJson({ tables: Object.fromEntries(tables.map((t) => [t.name, t.rows])) })) }]
        : tables.map((t) => ({ path: `${t.name}.csv`, sha256: sha256Hex(canonicalJson(t.rows)) }));
    for (const sha of digests) {
      files.push({ path: `attachments/${sha.slice(0, 2)}/${sha.slice(2, 4)}/${sha}`, sha256: sha });
    }
    const manifest = {
      format: 'g9-export',
      version: 1,
      export_id: exportId,
      created_at: this.now(),
      domains: [...domains],
      include_private: includePrivate,
      data_format: format,
      csv_formula_guard: format === 'CSV',
      tables: tables.map((t) => ({ name: t.name, rows: t.rows.length })),
      files,
      attachments: { copied: digests.length, missing: [], damaged: [] },
    };
    return {
      files: files.length + 1,
      rows: tables.reduce((n, t) => n + t.rows.length, 0),
      attachments: digests.length,
      manifestSha256: sha256Hex(canonicalJson(manifest)),
    };
  }

  private deletionRequest(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'deletion.request'>> {
    const op = 'deletion.request';
    const rec = asRecord(payload, op);
    const resourceRaw = rec.resource;
    if (typeof resourceRaw !== 'object' || resourceRaw === null) {
      throw schemaViolation('resource', `${op}: 'resource' must be a resource reference`);
    }
    const r = resourceRaw as Rec;
    const resource: ResourceRef = { kind: oneOf(r, 'kind', RESOURCE_KINDS, op), id: str(r, 'id', op) };
    const reason = str(rec, 'reason', op);
    if (EVIDENTIARY_KINDS.includes(resource.kind)) {
      throw fail('PRECONDITION_FAILED', 'deletion.evidentiary_restricted', `${resource.kind} rows are ON DELETE RESTRICT; corrections supersede, never erase`);
    }
    this.requireApproval(op, { resource: { kind: resource.kind, id: resource.id }, reason }, resource, 'DATA_DELETE', meta, `Erase ${resource.kind} ${resource.id}`);
    const deletionId = this.nextId('deletion');
    if (resource.kind === 'NOTE') {
      const note = this.notes.get(resource.id);
      if (note !== undefined) {
        const requestedAt = this.now();
        // g9-storage `tombstone_digest`: identifies the tombstone (request, table, row, time) and is
        // never derived from the erased content (ADR-0017: the digest of a short text is the text).
        const tombstone = sha256Hex(`g9-tombstone-v1\n${deletionId}\nnotes\n${resource.id}\n${requestedAt}`);
        this.notes.delete(resource.id);
        const filesLeft = this.eraseNote(note);
        return {
          result: { deletion_id: deletionId, state: 'APPLIED', resource, tombstone_sha256: tombstone, requested_at: requestedAt },
          revision: 1,
          resource,
          // The files leave the folder inside the erasure; only a missing folder leaves them.
          warnings: filesLeft && this.attachmentDir === null ? [ATTACHMENT_BYTES_NOT_PURGED_WARNING] : [],
        };
      }
    }
    return { result: { deletion_id: deletionId, state: 'REQUESTED', resource, tombstone_sha256: null, requested_at: this.now() }, revision: 1, resource };
  }

  /**
   * g9-storage deletion `erase_note`, reduced to what the mock keeps: the note is remembered as
   * erased (attaching to it answers `note.erased`), its attachments are erased (their file name
   * is no longer listed), and the evidence items of the note and of those attachments are
   * invalidated. Returns true when the erased attachments held bytes that no remaining attachment
   * uses: the backend removes those files from the attachment folder inside the erasure (the
   * approval covers the record and its files, ADR-0017), so the caller warns only when no folder
   * is configured.
   */
  private eraseNote(note: NoteView): boolean {
    this.erasedNotes.set(note.note_id, note.domain);
    let bytesLeft = false;
    for (const a of this.attachments.values()) {
      if (a.note_id !== note.note_id || a.erased) {
        continue;
      }
      a.erased = true;
      const shared = [...this.attachments.values()].some((o) => !o.erased && o.attachment_id !== a.attachment_id && o.sha256 === a.sha256);
      bytesLeft ||= !shared;
      this.invalidateEvidence((e) => e.attachment_id === a.attachment_id);
    }
    this.invalidateEvidence((e) => e.note_id === note.note_id);
    return bytesLeft;
  }

  /** g9-storage deletion `invalidate_evidence`: the summary repeats erased content, so it is redacted with the item. */
  private invalidateEvidence(match: (e: EvidenceRecord) => boolean): void {
    for (const e of this.evidence.values()) {
      if (!e.invalidated && match(e)) {
        e.invalidated = true;
        e.summary = REDACTED;
      }
    }
  }

  // ----- wave-2 operations: authoring, scheduling, evidence ---------------------

  /**
   * Port of g9-app `session.create` (sessions.rs): a PLANNED session the operator schedules,
   * optionally checked against an objective code. It is not linked to a curriculum block, so
   * its view is labelled `UNLINKED` (as `session_view` labels it); it then runs like any session.
   * The mock knows the objectives of the loaded fixture week only.
   */
  private sessionCreate(payload: unknown): Handled<SessionView> {
    const op = 'session.create';
    const rec = asRecord(payload, op);
    const minutes = boundedInt(rec, 'planned_minutes', op, 1, 600);
    const plannedStart = rec['planned_start'] === undefined ? undefined : boundedInt(rec, 'planned_start', op, 0, MAX_PLANNED_START_MS);
    // The request schema allows STUDY, LAB and REVIEW: EXAM is refused at the boundary.
    optOneOf(rec, 'mode', ['STUDY', 'LAB', 'REVIEW'] as const, op);
    const objective = optId(rec, 'objective_id', op);
    if (objective !== undefined && !this.fixture.week.days.some((d) => d.objectives.some((o) => o.objective_id === objective))) {
      throw fail('NOT_FOUND', 'objective.not_found', 'objective not found');
    }
    const start = plannedStart ?? this.now();
    const session: SessionRecord = {
      session_id: this.nextId('session'),
      state: 'PLANNED',
      day_id: 'UNLINKED',
      block: 'UNLINKED',
      block_seq: 1,
      planned_ms: minutes * 60_000,
      credited_ms: 0,
      uncertain_ms: 0,
      started_at: null,
      open_interval_started_at: null,
      requires_correction: false,
      intervals: [],
      corrections: [],
      revision: 1,
      date: formatIsoDate(civilDateOf(start, this.zone)),
    };
    this.sessions.set(session.session_id, session);
    return { result: stripSession(session), revision: 1, resource: { kind: 'STUDY_SESSION', id: session.session_id } };
  }

  // ----- program setup, notes read-back, packs, backup list -------------------

  /** `curriculum.program`: what is imported, where it sits on the calendar, the packs. */
  private curriculumProgram(): ProgramStatus {
    return {
      curriculum: this.program.curriculum === null ? null : { ...this.program.curriculum },
      program_start: this.program.start === null ? null : { ...this.program.start },
      competencies: { loaded: this.packCompetencies.size, available: PACK_CATALOG.reduce((n, p) => n + p.competencies.length, 0) },
    };
  }

  /** `curriculum.set_program_start`: the Monday of week 1 and the time a planned day starts. */
  private setProgramStart(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'curriculum.set_program_start'>> {
    const op = 'curriculum.set_program_start';
    const rec = asRecord(payload, op);
    const dateStr = patterned(rec, 'date', op, /^\d{4}-\d{2}-\d{2}$/);
    const dayStart = rec.day_start === undefined ? DEFAULT_DAY_START : patterned(rec, 'day_start', op, DAY_START_PATTERN);
    const date = parseIsoDate(dateStr);
    if (date === null) {
      throw fail('INVALID_INPUT', 'date.invalid', "date must be 'YYYY-MM-DD'", { field: 'date' });
    }
    if (isoWeekday(date) !== 1) {
      throw fail('INVALID_INPUT', 'program.start_not_a_monday', 'the program starts on a Monday: week 1 runs Monday to Saturday');
    }
    if (this.program.start !== null) {
      this.checkRevision(meta, this.program.start.revision);
    }
    if (this.program.curriculum === null) {
      throw fail('NOT_FOUND', 'curriculum.not_imported', 'no curriculum version is imported: import the curriculum first');
    }
    const revision = (this.program.start?.revision ?? 0) + 1;
    this.program.start = { monday: dateStr, day_start: dayStart, revision };
    const resource: ResourceRef = { kind: 'CURRICULUM', id: this.fixture.curriculum_id };
    return { result: { curriculum_id: this.fixture.curriculum_id, monday: dateStr, day_start: dayStart }, revision, resource };
  }

  /**
   * `session.plan_day`: one PLANNED session for each block of the program day on `date` that has
   * none, linked to its day and block. The mode of the block is not part of the session view.
   */
  private sessionPlanDay(payload: unknown): Handled<DayPlanned> {
    const op = 'session.plan_day';
    const rec = asRecord(payload, op);
    const dateStr = patterned(rec, 'date', op, /^\d{4}-\d{2}-\d{2}$/);
    const date = parseIsoDate(dateStr);
    if (date === null) {
      throw fail('INVALID_INPUT', 'date.invalid', "date must be 'YYYY-MM-DD'", { field: 'date' });
    }
    this.requireProgram();
    const { day } = this.dayForDate(date);
    if (day === null || day.blocks.length === 0) {
      throw fail('NOT_FOUND', 'curriculum.no_training_day', 'the program has no training day on this date');
    }
    const created: SessionView[] = [];
    const already: DayPlanned['already_planned'] = [];
    for (const block of day.blocks) {
      const linked = this.sessionOfBlock(day.day_id, block.seq, dateStr);
      if (linked !== undefined) {
        already.push({ block: block.block, seq: block.seq, session_id: linked.session_id });
        continue;
      }
      const session: SessionRecord = {
        session_id: this.nextId('session'),
        state: 'PLANNED',
        day_id: day.day_id,
        block: block.block,
        block_seq: block.seq,
        planned_ms: block.minutes * 60_000,
        credited_ms: 0,
        uncertain_ms: 0,
        started_at: null,
        open_interval_started_at: null,
        requires_correction: false,
        intervals: [],
        corrections: [],
        revision: 1,
        date: dateStr,
      };
      this.sessions.set(session.session_id, session);
      created.push(stripSession(session));
      // Like g9-app: each block is also placed on the calendar, by the scheduler (the operator cannot edit it);
      // the blind practical is an assessment slot the scheduler may not move. `block` and `session_id` stay null.
      const isAssessment = block.block === 'blind_practical';
      const start = this.blockStart(day, block.seq);
      const [hour, minute] = (this.program.start?.day_start ?? DEFAULT_DAY_START).split(':').map(Number);
      this.events.push({
        event_id: this.nextId('event'),
        title: `${day.day_id} ${block.block.replace(/_/g, ' ')}`,
        kind: isAssessment ? 'ASSESSMENT' : 'STUDY_BLOCK',
        start,
        end: start + block.minutes * 60_000,
        all_day: false,
        immutable: isAssessment,
        day_id: day.day_id,
        block: null,
        session_id: null,
        adjusted: block.seq === day.blocks[0]?.seq ? civilToUtc({ ...date, hour: hour ?? DAY_START_HOUR, minute: minute ?? 0 }, this.zone).adjusted : null,
        provenance: 'SYSTEM_DETERMINISTIC',
      });
    }
    const resource: ResourceRef = { kind: 'STUDY_SESSION', id: `day-${dateStr}` };
    return { result: { date: dateStr, day_id: day.day_id, created, already_planned: already }, revision: 1, resource };
  }

  /** `notes.list`: the newest notes of the scope's domain, without bodies; erased notes are gone. */
  private notesList(payload: unknown, meta: CommandMeta): { notes: NoteSummary[] } {
    const op = 'notes.list';
    const rec = asRecord(payload, op);
    const limit = rec.limit === undefined ? 100 : boundedInt(rec, 'limit', op, 1, 200);
    const encoder = new TextEncoder();
    const notes = [...this.notes.values()]
      .filter((n) => n.domain === meta.scope.domain)
      .sort((a, b) => b.updated_at - a.updated_at || compareIds(b.note_id, a.note_id))
      .slice(0, limit)
      .map(
        (n): NoteSummary => ({
          note_id: n.note_id,
          title: n.title,
          classification: n.classification,
          domain: n.domain,
          objective_id: n.objective_id,
          version_count: n.version_count,
          size_bytes: encoder.encode(n.body_markdown).length,
          created_at: n.created_at,
          updated_at: n.updated_at,
          revision: n.revision,
        }),
      );
    return { notes };
  }

  /** `notes.get`: one note with its body; an unknown id is not found, an erased one says so. */
  private notesGet(payload: unknown, meta: CommandMeta): NoteView {
    const op = 'notes.get';
    const rec = asRecord(payload, op);
    const id = idField(rec, 'note_id', op);
    const note = this.notes.get(id);
    const domain = note?.domain ?? this.erasedNotes.get(id);
    if (domain === undefined) {
      throw fail('NOT_FOUND', 'note.not_found', 'note not found');
    }
    if (domain !== meta.scope.domain) {
      throw fail('UNAUTHORIZED', 'storage.cross_domain_read', 'cross-domain read refused; Capability::CrossDomainRead not granted', {
        table: 'notes',
        requested_domain: meta.scope.domain,
        row_domain: domain,
      });
    }
    if (note === undefined) {
      throw fail('NOT_FOUND', 'note.erased', 'the note was erased; nothing of it is kept');
    }
    return { ...note };
  }

  /**
   * `competency.import_pack`: load the named packs (all when none is named). Idempotent; the
   * prerequisite edges of every pack are completed on each run, so a pack loaded later adds the
   * edges of packs loaded before it.
   */
  private competencyImportPack(payload: unknown): Handled<PackImportResult> {
    const op = 'competency.import_pack';
    const rec = asRecord(payload, op);
    let wanted: string[] = PACK_CATALOG.map((p) => p.pack_id);
    if (rec.pack_ids !== undefined) {
      const ids = strArray(rec, 'pack_ids', op);
      if (ids.length < 1 || ids.length > 8 || new Set(ids).size !== ids.length || ids.some((i) => !PACK_ID_PATTERN.test(i))) {
        throw schemaViolation('pack_ids', `${op}: 'pack_ids' must be 1 to 8 distinct pack ids`);
      }
      const unknown = ids.find((i) => !PACK_CATALOG.some((p) => p.pack_id === i));
      if (unknown !== undefined) {
        throw fail('NOT_FOUND', 'competency.pack_unknown', 'no competency pack of that id ships with the application');
      }
      wanted = [...new Set(ids)].sort();
    }
    const results: PackImportResult['packs'] = [];
    for (const pack of PACK_CATALOG.filter((p) => wanted.includes(p.pack_id))) {
      let created = 0;
      let existing = 0;
      for (const c of pack.competencies) {
        if (this.packCompetencies.has(c.id)) {
          existing += 1;
        } else {
          this.packCompetencies.set(c.id, { name: c.name, pack: pack.pack_id });
          created += 1;
        }
      }
      results.push({ pack_id: pack.pack_id, title: pack.title, version: pack.version, status: pack.status, competencies: pack.competencies.length, created, existing, prerequisites_added: 0 });
    }
    let added = 0;
    let pending = 0;
    for (const pack of PACK_CATALOG) {
      for (const c of pack.competencies) {
        if (!this.packCompetencies.has(c.id)) {
          continue;
        }
        for (const reference of c.prerequisites) {
          const from = prerequisiteId(reference);
          if (!this.packCompetencies.has(from)) {
            pending += 1;
          } else if (!this.packEdges.has(`${from}>${c.id}`)) {
            this.packEdges.add(`${from}>${c.id}`);
            added += 1;
            const own = results.find((r) => r.pack_id === pack.pack_id);
            if (own !== undefined) {
              own.prerequisites_added += 1;
            }
          }
        }
      }
    }
    const available = PACK_CATALOG.reduce((n, p) => n + p.competencies.length, 0);
    return {
      result: { packs: results, loaded: this.packCompetencies.size, available, prerequisites_added: added, prerequisites_pending: pending },
      revision: 1,
      resource: { kind: 'COMPETENCY', id: 'packs' },
    };
  }

  /** `backup.list`: the recorded backups, newest first. */
  private backupList(): ResponseOf<'backup.list'> {
    const backups = [...this.backups.values()]
      .sort((a, b) => b.created_at - a.created_at || compareIds(b.backup_id, a.backup_id))
      .map((b) => ({ ...b }));
    return { backups, unlisted: 0 };
  }

  private findEvent(id: Id): CalendarEvent {
    const event = this.events.find((e) => e.event_id === id);
    if (event === undefined) {
      throw fail('NOT_FOUND', 'calendar.event_not_found', 'event not found');
    }
    return event;
  }

  /** Port of g9-app `calendar.create_event`: an event the operator adds (source OPERATOR). */
  private calendarCreateEvent(payload: unknown): Handled<CalendarEvent> {
    const op = 'calendar.create_event';
    const rec = asRecord(payload, op);
    const title = boundedStr(rec, 'title', op, 200);
    const kind = oneOf(rec, 'kind', OPERATOR_EVENT_KINDS, op);
    const start = int(rec, 'start', op, 0);
    const end = int(rec, 'end', op, 0);
    const allDay = optBool(rec, 'all_day', op) ?? false;
    checkEventRange(start, end);
    const event: CalendarEvent = {
      event_id: this.nextId('event'),
      title: rustTrim(title),
      kind,
      start,
      end,
      all_day: allDay,
      immutable: false,
      day_id: null,
      block: null,
      session_id: null,
      adjusted: null,
      provenance: 'OPERATOR',
    };
    this.events.push(event);
    this.eventRevisions.set(event.event_id, 1);
    return { result: { ...event }, revision: 1, resource: { kind: 'CALENDAR_EVENT', id: event.event_id } };
  }

  /**
   * Port of g9-app `calendar.update_event`, in the backend's order: unknown event (NOT_FOUND),
   * immutable event (policy `immutable.finalized.deny`), `expected_revision`, then only events
   * the operator added (`calendar.event_not_editable`), then the new range.
   */
  private calendarUpdateEvent(payload: unknown, meta: CommandMeta): Handled<CalendarEvent> {
    const op = 'calendar.update_event';
    const rec = asRecord(payload, op);
    const id = idField(rec, 'event_id', op);
    const title = rec.title === undefined ? undefined : boundedStr(rec, 'title', op, 200);
    const kind = optOneOf(rec, 'kind', OPERATOR_EVENT_KINDS, op);
    const start = optInt(rec, 'start', op, 0);
    const end = optInt(rec, 'end', op, 0);
    const event = this.findEvent(id);
    // g9-app checks this before policy: an event that cannot be changed is refused, and never put to an approval.
    assertEditable(event);
    const revision = this.eventRevisions.get(id) ?? 1;
    this.checkRevision(meta, revision);
    assertEditable(event);
    const nextStart = start ?? event.start;
    const nextEnd = end ?? event.end;
    checkEventRange(nextStart, nextEnd);
    const updated: CalendarEvent = {
      ...event,
      title: title === undefined ? event.title : rustTrim(title),
      kind: kind ?? event.kind,
      start: nextStart,
      end: nextEnd,
    };
    this.events = this.events.map((e) => (e.event_id === id ? updated : e));
    this.eventRevisions.set(id, revision + 1);
    return { result: { ...updated }, revision: revision + 1, resource: { kind: 'CALENDAR_EVENT', id } };
  }

  /**
   * Port of g9-app `calendar.delete_event`: policy `operator.delete.approval` asks for a
   * DATA_DELETE approval bound to the event's stored revision (immutable events are denied
   * first); with the approval, only events the operator added are deleted. A refusal after the
   * approval rolls back and leaves the approval APPROVED.
   */
  private calendarDeleteEvent(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'calendar.delete_event'>> {
    const op = 'calendar.delete_event';
    const rec = asRecord(payload, op);
    const id = idField(rec, 'event_id', op);
    const event = this.findEvent(id);
    // g9-app checks this before policy: the operator is never asked to approve a deletion that cannot happen.
    assertEditable(event);
    const revision = this.eventRevisions.get(id) ?? 1;
    const resource: ResourceRef = { kind: 'CALENDAR_EVENT', id };
    this.requireApproval(op, asJson(payload), resource, 'DATA_DELETE', meta, approvalSummary(op, resource, 'DATA_DELETE'), revision);
    this.checkRevision(meta, revision);
    this.events = this.events.filter((e) => e.event_id !== id);
    this.eventRevisions.delete(id);
    return { result: { event_id: id, deleted: true }, resource };
  }

  /**
   * Port of g9-app `retention.create_item`: a baseline-scheduled item due one day later at
   * stage 0; a non-blank answer is kept as an erasable note ("Review answer"), never on the item.
   */
  private retentionCreateItem(payload: unknown): Handled<ReviewItem> {
    const op = 'retention.create_item';
    const rec = asRecord(payload, op);
    const prompt = boundedStr(rec, 'prompt', op, 2000);
    const answer = optText(rec, 'answer', op, 20_000);
    const kind = optOneOf(rec, 'kind', ['DECLARATIVE', 'PROCEDURAL'] as const satisfies readonly ReviewItemAuthoredKind[], op) ?? 'DECLARATIVE';
    const competencyId = optId(rec, 'competency_id', op);
    if (competencyId !== undefined && !this.knownCompetencies().some((c) => c.id === competencyId)) {
      throw fail('NOT_FOUND', 'competency.not_found', 'competency not found');
    }
    const now = this.now();
    const answerText = answer === undefined ? '' : rustTrim(answer);
    if (answerText.length > 0) {
      const note: NoteView = {
        note_id: this.nextId('note'),
        title: 'Review answer',
        body_markdown: answerText,
        content_sha256: sha256Hex(answerText),
        classification: 'INTERNAL',
        domain: 'LEARNING',
        objective_id: null,
        evidence_refs: [],
        version_count: 1,
        created_at: now,
        updated_at: now,
        revision: 1,
      };
      this.notes.set(note.note_id, note);
    }
    const view: ReviewItem = {
      review_item_id: this.nextId('review'),
      competency_id: competencyId ?? null,
      objective_id: null,
      prompt: rustTrim(prompt),
      // retention.rs `review_item_view`: the v1.0 vocabulary has no PROCEDURAL kind.
      kind: kind === 'PROCEDURAL' ? 'PRACTICAL_TRANSFER' : 'DECLARATIVE',
      due_at: now + DAY_MS,
      last_success_at: null,
      offset_days: 1,
      estimated_ms: REVIEW_ITEM_MS,
    };
    this.reviewItems.set(view.review_item_id, { view, authoredKind: kind, stage: 0, revision: 1, attempts: [] });
    return { result: { ...view }, revision: 1, resource: { kind: 'REVIEW_ITEM', id: view.review_item_id } };
  }

  /**
   * Port of g9-app `retention.record_review`: the attempt is appended (append-only); SUCCESS and
   * FAILURE move the item along g9-retention's `BaselineSchedule` (`baselineNext`); SKIPPED is
   * recorded and leaves due date, stage and revision as they were.
   */
  private retentionRecordReview(payload: unknown, meta: CommandMeta): Handled<ReviewRecord> {
    const op = 'retention.record_review';
    const rec = asRecord(payload, op);
    const id = idField(rec, 'review_item_id', op);
    const outcome = oneOf(rec, 'outcome', ['SUCCESS', 'FAILURE', 'SKIPPED'] as const, op);
    optInt(rec, 'response_ms', op, 0);
    if (rec.rating !== undefined) {
      boundedInt(rec, 'rating', op, 1, 4);
    }
    optId(rec, 'session_id', op);
    const item = this.reviewItems.get(id);
    if (item === undefined) {
      throw fail('NOT_FOUND', 'retention.item_not_found', 'review item not found');
    }
    this.checkRevision(meta, item.revision);
    const now = this.now();
    item.attempts.push({ outcome, at: now });
    if (outcome !== 'SKIPPED') {
      const next = baselineNext(item.stage, outcome === 'SUCCESS', now);
      item.stage = next.stage;
      item.view = {
        ...item.view,
        due_at: next.dueAt,
        offset_days: next.offsetDays,
        last_success_at: outcome === 'SUCCESS' ? now : item.view.last_success_at,
      };
      item.revision += 1;
    }
    return {
      result: { review_item_id: id, outcome, due_at: item.view.due_at, stage: item.stage, offset_days: item.view.offset_days },
      revision: item.revision,
      resource: { kind: 'REVIEW_ITEM', id },
    };
  }

  /** Every competency the mock knows: one per fixture study day, one per fixture assessment, and the authored ones. */
  private knownCompetencies(): { id: Id; name: string; objective_ids: Id[] }[] {
    const days = (this.fresh ? [] : this.fixture.week.days)
      .filter((d) => !d.is_assessment_day)
      .map((d) => ({ id: `mock-competency-${d.day_id}`, name: d.evidence_artifact, objective_ids: d.objectives.map((o) => o.objective_id) }));
    // One competency per fixture assessment definition (the backend definition links one competency).
    const assessed = this.definitions
      .filter((d) => !this.definitionCompetency.has(d.assessment_id))
      .map((d) => ({ id: assessedCompetencyId(d.assessment_id), name: d.title, objective_ids: [] as Id[] }));
    const authored = this.authoredCompetencies.map((c) => ({ id: c.competency_id, name: c.name, objective_ids: [] as Id[] }));
    const packs = [...this.packCompetencies].map(([id, c]) => ({ id, name: c.name, objective_ids: [] as Id[] }));
    return [...days, ...assessed, ...authored, ...packs];
  }

  /** The competency an assessment definition assesses. */
  private competencyOf(assessmentId: Id): Id {
    return this.definitionCompetency.get(assessmentId) ?? assessedCompetencyId(assessmentId);
  }

  /**
   * Port of g9-app `competency.create` (authoring.rs): validated like `g9_competency::Competency`
   * (a blank title is refused; validity default 180 days); the code is unique.
   */
  private competencyCreate(payload: unknown): Handled<CompetencyCreated> {
    const op = 'competency.create';
    const rec = asRecord(payload, op);
    const code = patterned(rec, 'code', op, AUTHORED_CODE_PATTERN);
    const title = boundedStr(rec, 'title', op, 200);
    const description = optText(rec, 'description', op, 4000) ?? '';
    const validity = rec.validity_days === undefined ? 180 : boundedInt(rec, 'validity_days', op, 1, 3650);
    const name = rustTrim(title);
    if (name.length === 0) {
      throw fail('INVALID_INPUT', 'competency.invalid', 'invalid competency definition: name must not be empty');
    }
    if (this.authoredCompetencies.some((c) => c.code === code)) {
      throw fail('CONFLICT', 'storage.unique_violation', 'unique constraint failed: UNIQUE constraint failed: competencies.code');
    }
    const id = this.nextId('competency');
    this.authoredCompetencies.push({ competency_id: id, code, name, description, validity_days: validity });
    return { result: { competency_id: id, code, title: name, validity_days: validity }, revision: 1, resource: { kind: 'COMPETENCY', id } };
  }

  /**
   * Port of g9-app `assessment.define`: the competency must exist, the definition (EXAM mode,
   * listed under its code), version 1 with the declared difficulty and the baseline rubric are
   * written together; the code is unique.
   */
  private assessmentDefine(payload: unknown): Handled<AssessmentDefined> {
    const op = 'assessment.define';
    const rec = asRecord(payload, op);
    const code = patterned(rec, 'code', op, AUTHORED_CODE_PATTERN);
    const title = boundedStr(rec, 'title', op, 200);
    const kind = oneOf(rec, 'kind', ['BLIND_PRACTICAL', 'CAPSTONE', 'TRANSFER_CHECK'] as const, op);
    const competencyId = idField(rec, 'competency_id', op);
    boundedStr(rec, 'task_statement', op, 20_000);
    const difficulty = boundedInt(rec, 'difficulty', op, 1, 5) as Difficulty;
    if (!this.knownCompetencies().some((c) => c.id === competencyId)) {
      throw fail('NOT_FOUND', 'competency.not_found', 'competency not found');
    }
    const trimmed = rustTrim(title);
    if (trimmed.length === 0) {
      throw fail('INVALID_INPUT', 'assessment.definition_invalid', 'assessment title is empty');
    }
    if ([...this.definitionCompetency.keys()].some((defId) => this.definitions.find((d) => d.assessment_id === defId)?.day_id === code)) {
      throw fail('CONFLICT', 'storage.unique_violation', 'unique constraint failed: UNIQUE constraint failed: assessment_definitions.code');
    }
    const id = this.nextId('assessment');
    // assessment.rs `list`: a definition without a training day is listed under its code.
    this.definitions.push({ assessment_id: id, title: trimmed, day_id: code, mode: 'EXAM' });
    this.declaredDifficulty.set(id, difficulty);
    this.definitionCompetency.set(id, competencyId);
    return {
      result: {
        assessment_id: id,
        code,
        title: trimmed,
        kind,
        competency_id: competencyId,
        version_id: `${id}:v1`,
        difficulty,
        criteria: BASELINE_CRITERIA.map((c) => c.code),
      },
      revision: 1,
      resource: { kind: 'ASSESSMENT_DEFINITION', id },
    };
  }

  /**
   * evidence.rs `owner` / the `from_note` load: the note read in the command's scope domain
   * (another domain is a refused cross-domain read), present, and not erased.
   */
  private noteInScope(noteId: Id, meta: CommandMeta, erasedMessage: string): NoteView {
    const note = this.notes.get(noteId);
    const domain = note?.domain ?? this.erasedNotes.get(noteId);
    if (domain === undefined) {
      throw fail('NOT_FOUND', 'note.not_found', 'note not found');
    }
    if (domain !== meta.scope.domain) {
      throw fail('UNAUTHORIZED', 'storage.cross_domain_read', 'cross-domain read refused; Capability::CrossDomainRead not granted', {
        table: 'notes',
        requested_domain: meta.scope.domain,
        row_domain: domain,
      });
    }
    if (note === undefined) {
      throw fail('CONFLICT', 'note.erased', erasedMessage);
    }
    return note;
  }

  private addEvidence(record: Omit<EvidenceRecord, 'seq' | 'invalidated'>): void {
    this.evidenceSeq += 1;
    this.evidence.set(record.evidence_id, { ...record, seq: this.evidenceSeq, invalidated: false });
  }

  /**
   * Port of g9-app `evidence.attach_file`, in the backend's order: the request schema; the file
   * name and the base64 (before the pipeline); the attachment store and the owning note (facts);
   * the policy (a SECURITY_SENSITIVE note needs a PROTECTED_RECORD_WRITE approval); then the bytes
   * (g9-evidence `AttachmentRef::new`: empty, refused signature, declared type mismatch). The
   * bytes are content-addressed: identical bytes are stored once (`already_stored`).
   */
  private evidenceAttachFile(payload: unknown, meta: CommandMeta): Handled<AttachmentView> {
    const op = 'evidence.attach_file';
    const rec = asRecord(payload, op);
    const noteId = optId(rec, 'note_id', op);
    const rawName = boundedStr(rec, 'filename', op, 255);
    const mediaType: AttachmentMediaType = oneOf(rec, 'media_type', ATTACHMENT_MEDIA_TYPES, op);
    const content = str(rec, 'content_base64', op);
    if (content.length < 4 || content.length > MAX_BASE64_CHARS) {
      throw schemaViolation('content_base64', `${op}: 'content_base64' must be 4 to ${MAX_BASE64_CHARS} characters`);
    }
    const classification = optOneOf(rec, 'classification', CLASSIFICATIONS, op);
    const filename = cleanFilename(rawName);
    if (filename === null) {
      throw fail('INVALID_INPUT', 'attachment.filename_invalid', 'the file name must be 1 to 255 bytes with no path separators or control characters');
    }
    const decoded = decodeContent(content);
    if (!decoded.ok) {
      throw decoded.code === 'attachment.too_large'
        ? fail('INVALID_INPUT', 'attachment.too_large', `a file attached through the app is at most ${MAX_UPLOAD_BYTES} bytes`)
        : fail('INVALID_INPUT', 'attachment.content_invalid', 'content_base64 is not valid base64 (standard alphabet, padded)');
    }
    const bytes = decoded.bytes;
    const attachmentId = this.creationId(meta, op, 'attachment');
    const resource: ResourceRef = { kind: 'ATTACHMENT', id: attachmentId };
    if (this.attachmentDir === null) {
      throw fail('PRECONDITION_FAILED', 'attachment.store_not_configured', 'no attachment directory is configured');
    }
    const note = noteId === undefined ? null : this.noteInScope(noteId, meta, 'the note is erased; nothing can be attached to it');
    const domain: DataDomain = note?.domain ?? 'LEARNING';
    // The stricter of the note's classification and the one asked for: a file can be more
    // sensitive than its note, never less (security audit 2026-11-13).
    const ownerClassification: Classification =
      note === null
        ? classification ?? 'INTERNAL'
        : CLASSIFICATIONS[Math.max(CLASSIFICATIONS.indexOf(note.classification), classification === undefined ? 0 : CLASSIFICATIONS.indexOf(classification))] ?? note.classification;
    this.requireSensitiveWriteApproval(op, domain, asJson(payload), resource, meta);
    if (bytes.length === 0) {
      throw fail('INVALID_INPUT', 'attachment.empty', 'the file is empty');
    }
    const refused = refusedSignature(bytes);
    if (refused !== null) {
      throw fail('INVALID_INPUT', 'attachment.refused', `${refused}: executables, archives and packet captures are not stored; attach a written report instead`);
    }
    if (!consistentWith(mediaType, bytes)) {
      throw fail(
        'INVALID_INPUT',
        'attachment.content_mismatch',
        `the file's bytes do not match ${mediaType}; quarantine is not stored yet (PROPOSED), so it was not attached`,
      );
    }
    const sha256 = sha256HexBytes(bytes);
    // evidence.rs: true only when a non-erased attachment with these bytes exists in the caller's
    // scope domain, so the flag never reveals content the caller cannot read.
    const alreadyStored = [...this.attachments.values()].some((a) => a.sha256 === sha256 && !a.erased && a.domain === meta.scope.domain);
    const now = this.now();
    this.attachments.set(attachmentId, {
      attachment_id: attachmentId,
      note_id: note?.note_id ?? null,
      filename,
      media_type: mediaType,
      size_bytes: bytes.length,
      sha256,
      domain,
      classification: ownerClassification,
      erased: false,
    });
    const evidenceId = this.nextId('evidence');
    this.addEvidence({
      evidence_id: evidenceId,
      kind: 'ATTACHMENT',
      note_id: null,
      attachment_id: attachmentId,
      summary: filename,
      sha256,
      domain,
      classification: ownerClassification,
      created_at: now,
    });
    return {
      result: {
        attachment_id: attachmentId,
        evidence_id: evidenceId,
        note_id: note?.note_id ?? null,
        filename,
        media_type: mediaType,
        size_bytes: bytes.length,
        sha256,
        already_stored: alreadyStored,
      },
      revision: 1,
      resource,
    };
  }

  /** Port of g9-app `evidence.from_note`: a typed note becomes a NOTE evidence item (summary = its title). */
  private evidenceFromNote(payload: unknown, meta: CommandMeta): Handled<ResponseOf<'evidence.from_note'>> {
    const op = 'evidence.from_note';
    const rec = asRecord(payload, op);
    const noteId = idField(rec, 'note_id', op);
    const evidenceId = this.creationId(meta, op, 'evidence');
    const note = this.noteInScope(noteId, meta, 'the note is erased');
    const resource: ResourceRef = { kind: 'EVIDENCE_ITEM', id: evidenceId };
    this.requireSensitiveWriteApproval(op, note.domain, asJson(payload), resource, meta);
    const summary = evidenceSummary(note.title);
    this.addEvidence({
      evidence_id: evidenceId,
      kind: 'NOTE',
      note_id: note.note_id,
      attachment_id: null,
      summary,
      sha256: sha256Hex(note.body_markdown),
      domain: note.domain,
      classification: note.classification,
      created_at: this.now(),
    });
    return { result: { evidence_id: evidenceId, kind: 'NOTE', note_id: note.note_id, summary }, revision: 1, resource };
  }

  /**
   * Port of g9-app `evidence.list` (read-only): with `note_id`, the note's NOTE items and its
   * attachments' ATTACHMENT items, oldest first; without, the newest items. Only items of the
   * command's scope domain are listed; `limit` defaults to 100.
   */
  private evidenceList(payload: unknown, meta: CommandMeta): ResponseOf<'evidence.list'> {
    const op = 'evidence.list';
    const rec = asRecord(payload, op);
    const noteId = optId(rec, 'note_id', op);
    const limit = rec.limit === undefined ? 100 : boundedInt(rec, 'limit', op, 1, 500);
    const inScope = [...this.evidence.values()].filter((e) => e.domain === meta.scope.domain);
    const rows =
      noteId === undefined
        ? inScope.sort((a, b) => b.created_at - a.created_at || b.seq - a.seq)
        : inScope
            .filter((e) => e.note_id === noteId || (e.attachment_id !== null && this.attachments.get(e.attachment_id)?.note_id === noteId))
            .sort((a, b) => a.created_at - b.created_at || a.seq - b.seq);
    return { items: rows.slice(0, limit).map((e) => this.evidenceView(e)) };
  }

  private evidenceView(e: EvidenceRecord): EvidenceItemView {
    const attachment = e.attachment_id === null ? undefined : this.attachments.get(e.attachment_id);
    return {
      evidence_id: e.evidence_id,
      kind: e.kind,
      note_id: e.note_id,
      attachment_id: e.attachment_id,
      summary: e.summary,
      sha256: e.sha256,
      filename: attachment === undefined || attachment.erased ? null : attachment.filename,
      media_type: attachment?.media_type ?? null,
      size_bytes: attachment?.size_bytes ?? null,
      created_at: e.created_at,
      invalidated: e.invalidated,
    };
  }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

interface Handled<T = unknown> {
  result: T;
  revision?: Revision;
  resource?: ResourceRef;
  /** Becomes `CommandOutcome.warnings` (omitted when empty), as in g9-app. */
  warnings?: readonly string[];
}

/** A `review_items` row with the scheduler columns the view does not show. */
interface ReviewItemRecord {
  view: ReviewItem;
  authoredKind: ReviewItemAuthoredKind;
  /** Baseline ladder stage reached (g9-retention `ItemState::stage_index`). */
  stage: number;
  revision: Revision;
  /** `review_attempts` rows (append-only). */
  attempts: { outcome: ReviewRecord['outcome']; at: UtcMillis }[];
}

interface AuthoredCompetency {
  competency_id: Id;
  code: string;
  name: string;
  description: string;
  validity_days: number;
}

interface AttachmentRecord {
  attachment_id: Id;
  note_id: Id | null;
  filename: string;
  media_type: AttachmentMediaType;
  size_bytes: number;
  sha256: string;
  /** The owning note's domain and classification (LEARNING and the requested classification without a note). */
  domain: DataDomain;
  classification: Classification;
  erased: boolean;
}

interface EvidenceRecord {
  evidence_id: Id;
  kind: 'NOTE' | 'ATTACHMENT';
  note_id: Id | null;
  attachment_id: Id | null;
  summary: string;
  sha256: string;
  domain: DataDomain;
  classification: Classification;
  created_at: UtcMillis;
  /** Insertion order: the backend's UUIDv7 ids order items created in the same millisecond. */
  seq: number;
  invalidated: boolean;
}

const DAY_MS = 86_400_000;
const OPERATOR_EVENT_KINDS: readonly OperatorEventKind[] = ['STUDY_BLOCK', 'APPOINTMENT', 'REVIEW'];
const ID_PATTERN = /^[A-Za-z0-9._:-]{1,128}$/;
/** evidence.rs `MAX_UPLOAD_BYTES`. */
const MAX_UPLOAD_BYTES = MAX_ATTACHMENT_BYTES;
/** `content_base64` maxLength in the request schema. */
const MAX_BASE64_CHARS = 22_369_624;

/** The payload as JSON (what an approval's parameter hash covers); anything else hashes as null. */
function asJson(payload: unknown): JsonValue {
  return isJsonValue(payload) ? payload : null;
}

function idField(rec: Rec, key: string, op: string): Id {
  const v = str(rec, key, op);
  if (!ID_PATTERN.test(v)) {
    throw schemaViolation(key, `${op}: '${key}' must be an identifier [A-Za-z0-9._:-]{1,128}`);
  }
  return v;
}

function optId(rec: Rec, key: string, op: string): Id | undefined {
  return rec[key] === undefined ? undefined : idField(rec, key, op);
}

/** A string of 1..`max` characters (JSON Schema counts code points). */
function boundedStr(rec: Rec, key: string, op: string, max: number): string {
  const v = str(rec, key, op);
  if (Array.from(v).length > max) {
    throw schemaViolation(key, `${op}: '${key}' must be at most ${max} characters`);
  }
  return v;
}

/** An optional string of 0..`max` characters. */
function optText(rec: Rec, key: string, op: string, max: number): string | undefined {
  const v = rec[key];
  if (v === undefined) {
    return undefined;
  }
  if (typeof v !== 'string' || Array.from(v).length > max) {
    throw schemaViolation(key, `${op}: '${key}' must be a string of at most ${max} characters`);
  }
  return v;
}

function boundedInt(rec: Rec, key: string, op: string, min: number, max: number): number {
  const v = int(rec, key, op, min);
  if (v > max) {
    throw schemaViolation(key, `${op}: '${key}' must be an integer from ${min} to ${max}`);
  }
  return v;
}

function optBool(rec: Rec, key: string, op: string): boolean | undefined {
  const v = rec[key];
  if (v === undefined) {
    return undefined;
  }
  if (typeof v !== 'boolean') {
    throw schemaViolation(key, `${op}: '${key}' must be a boolean`);
  }
  return v;
}

function patterned(rec: Rec, key: string, op: string, pattern: RegExp): string {
  const v = rec[key];
  if (typeof v !== 'string' || !pattern.test(v)) {
    throw schemaViolation(key, `${op}: '${key}' must match ${pattern.source}`);
  }
  return v;
}

/** The summary g9-app shows for an approval request (approvals.rs `view`). */
function approvalSummary(op: OperationName, resource: ResourceRef, capability: Capability): string {
  return `${op}: ${resource.kind} ${resource.id} (requires ${capability})`;
}

/** calendar.rs `placed`: an event ends after it starts and lasts at most seven days. */
function checkEventRange(start: UtcMillis, end: UtcMillis): void {
  if (end <= start) {
    throw fail('INVALID_INPUT', 'calendar.event_range_invalid', 'an event ends after it starts');
  }
  if (end - start > 7 * DAY_MS) {
    throw fail('INVALID_INPUT', 'calendar.event_too_long', 'an event lasts at most seven days');
  }
}

/** calendar.rs `editable`: only events the operator added (curriculum, scheduler and imported events cannot change). */
function assertEditable(event: CalendarEvent): void {
  if (event.provenance !== 'OPERATOR' || event.immutable) {
    throw fail('PRECONDITION_FAILED', 'calendar.event_not_editable', 'only events you added can be changed or deleted');
  }
}

/**
 * g9-retention `BaselineSchedule::next_after` with the default ladder (offsets 1, 3, 7, 21, 60
 * days; failure resets to stage 0). Success: the interval is the offset of the stage just
 * completed and the stage advances (capped at the last); failure: stage 0 and its offset.
 */
export function baselineNext(stage: number, success: boolean, now: UtcMillis): { stage: number; offsetDays: number; dueAt: UtcMillis } {
  const last = REVIEW_OFFSETS_DAYS.length - 1;
  const current = success ? Math.min(stage, last) : 0;
  const offsetDays = REVIEW_OFFSETS_DAYS[current] ?? 1;
  return { stage: success ? Math.min(current + 1, last) : 0, offsetDays, dueAt: now + offsetDays * DAY_MS };
}

/** Byte-wise id order (Rust `String` ordering) for ASCII ids. */
function compareIds(a: Id, b: Id): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

// Warning texts copied from g9-app (commands/curriculum.rs, privacy.rs, assessment.rs, notes.rs, retention.rs).
const NON_CANONICAL_PLAN_WARNING = 'curriculum.non_canonical: plan derived from a version that is not APPROVED_CANONICAL';
/** privacy.rs `deletion.request`: erased attachments whose bytes no remaining attachment uses. */
const ATTACHMENT_BYTES_NOT_PURGED_WARNING =
  'deletion.attachment_bytes_not_purged: the record is erased, but no attachment folder inside an approved root is configured, so its file was not removed from disk';
/** evidence.rs: an `evidence.attach_file` / `evidence.from_note` replay rebuilt from current rows. */
const REPLAY_REBUILT_WARNING =
  'idempotency.replay_rebuilt: names and titles are not retained in idempotency records; the result shows the evidence as currently stored';
/** g9-storage deletion `REDACTED`: what an erased attachment's file name reads. */
const REDACTED = '[REDACTED]';

/**
 * evidence.rs `summary_of`: one line (control characters become spaces), at most 500 characters, never empty.
 * Exported for its own test: a title with control characters can no longer be created (`input.name_not_printable`),
 * but a row stored before that rule can still hold one.
 */
export function evidenceSummary(title: string): string {
  const line = rustTrim(
    Array.from(title, (c) => {
      const code = c.codePointAt(0) ?? 0;
      return code <= 0x1f || (code >= 0x7f && code <= 0x9f) ? ' ' : c;
    })
      .slice(0, 500)
      .join(''),
  );
  return line.length === 0 ? 'Note' : line;
}

/** retention.rs `retention.due`: PROCEDURAL items are listed with the PRACTICAL_TRANSFER label. */
function proceduralLabelWarning(count: number): string {
  return `retention.due.procedural_label: ${count} PROCEDURAL review item(s) are memory-scheduled recall items labelled PRACTICAL_TRANSFER because the v1.0 response has no PROCEDURAL kind; they are not practical transfer checks`;
}
/** g9-app commands/assessment.rs: names why nothing was derived; the difficulty is never guessed. */
function observationsNotDerivedWarning(reason: string): string {
  return `competency.observations_not_derived: ${reason}; no competency observation was recorded from this decision and the difficulty is never guessed`;
}
const NOTE_EVIDENCE_NOT_PERSISTED_WARNING = 'notes.evidence_refs_not_persisted: note-level evidence links are PROPOSED (wave-1 storage gap)';
/** g9-app commands/sessions.rs `correct_time`; the remaining interval ids follow after a space. */
export const UNCORRECTED_UNCERTAIN_WARNING =
  'session.uncorrected_uncertain_time: the session stays INTERRUPTED until every UNCERTAIN interval has a correction; remaining:';

function applyMachine<S extends string, E extends string>(table: TransitionTable<S, E>, from: S, event: E, area: string): S {
  const r = transition(table, from, event);
  if (!r.ok) {
    throw fail('PRECONDITION_FAILED', `${area}.illegal_transition`, `${r.refusal.machine}: event ${event} is not allowed in state ${from}`, {
      machine: r.refusal.machine,
      from,
      event,
    });
  }
  return r.to;
}

/** The latest correction of each interval (`Ledger::latest_correction`: corrections are recorded in order). */
function latestCorrections(session: SessionRecord): Map<Id, TimeCorrection> {
  const latest = new Map<Id, TimeCorrection>();
  for (const c of session.corrections) {
    latest.set(c.interval_id, c);
  }
  return latest;
}

/** Closed UNCERTAIN intervals without any correction (`Ledger::has_uncorrected_uncertain`). */
function uncorrectedUncertain(session: SessionRecord): TimeInterval[] {
  const latest = latestCorrections(session);
  return session.intervals.filter((i) => i.continuity === 'UNCERTAIN' && i.end_utc !== null && !latest.has(i.interval_id));
}

/**
 * `Ledger::credited` + `session_view`: a closed interval with a correction is credited with its
 * latest correction and leaves `uncertain_ms`; otherwise VERIFIED length is credited and
 * UNCERTAIN length is uncertain. `requires_correction` is INTERRUPTED or any uncorrected
 * closed UNCERTAIN interval, whatever the state (so it stays true after COMPLETE).
 */
function recomputeLedger(session: SessionRecord): void {
  const latest = latestCorrections(session);
  let credited = 0;
  let uncertain = 0;
  for (const i of session.intervals) {
    if (i.end_utc === null) {
      continue;
    }
    const correction = latest.get(i.interval_id);
    const len = Math.max(0, i.end_utc - i.start_utc);
    if (correction !== undefined) {
      credited += correction.credited_ms;
    } else if (i.continuity === 'VERIFIED') {
      credited += len;
    } else {
      uncertain += len;
    }
  }
  session.credited_ms = credited;
  session.uncertain_ms = uncertain;
  session.requires_correction = session.state === 'INTERRUPTED' || uncorrectedUncertain(session).length > 0;
}

function stripSession(s: SessionRecord): SessionView {
  return {
    session_id: s.session_id,
    state: s.state,
    day_id: s.day_id,
    block: s.block,
    block_seq: s.block_seq,
    planned_ms: s.planned_ms,
    credited_ms: s.credited_ms,
    uncertain_ms: s.uncertain_ms,
    started_at: s.started_at,
    open_interval_started_at: s.open_interval_started_at,
    requires_correction: s.requires_correction,
    intervals: s.intervals.map((i): TimeInterval => ({ ...i })),
    corrections: s.corrections.map((c) => ({ ...c })),
    revision: s.revision,
  };
}

/**
 * Difficulty (1..=5) declared by the fixture for every mock assessment version, like the value the
 * CLI seed and the evaluation fixtures declare: a fixture value, not a measurement.
 */
export const FIXTURE_DECLARED_DIFFICULTY = 3;

const LEARNING_DIMENSIONS: readonly LearningDimension[] = ['RECALL', 'UNDERSTANDING', 'APPLICATION', 'TRANSFER'];

/** `g9_assessment::Rubric::baseline_dimension` (crates/g9-assessment/src/rubric.rs). */
const BASELINE_DIMENSION: Readonly<Record<string, LearningDimension>> = {
  independent_execution: 'APPLICATION',
  root_cause_reasoning: 'UNDERSTANDING',
  remediation_verification: 'APPLICATION',
  detection_visibility: 'TRANSFER',
  evidence_documentation: 'UNDERSTANDING',
  scope_compliance_reporting: 'APPLICATION',
};

/** A competency observation derived from a final grade (EXAM mode, unaided, SELF_ADJUDICATED decision). */
interface MockObservation {
  observation_id: Id;
  competency_id: Id;
  dimension: LearningDimension;
  attempt_id: Id;
  difficulty: number;
  score: number;
  observed_at: UtcMillis;
}

/** `mock-assessment-W2` assesses `mock-competency-W2`. */
function assessedCompetencyId(assessmentId: Id): Id {
  return `mock-competency-${assessmentId.replace(/^mock-assessment-/, '')}`;
}

/**
 * Reduced port of g9-competency `derive_state_excluding_validation` over the observations the mock
 * can hold (all EXAM mode, unaided, from SELF_ADJUDICATED decisions: the MVP operator is the
 * learner, so no mock observation is validation-eligible, ADR-0005/0012, Addendum A.2):
 *  - UNASSESSED without observations, else INTRODUCED;
 *  - GUIDED on an Application observation >= 60;
 *  - INDEPENDENT on >= 2 Application observations >= 70 at difficulty >= 3 (the 60-day window is not
 *    modelled: the mock clock does not move between attempts);
 *  - never VALIDATED; `provisional` when an observation would have been validation-level evidence
 *    (Application/Transfer >= 80) but is excluded;
 *  - per dimension: mean score (every mock observation is unaided, independence weight 1), unaided
 *    count, last instant, uncertainty 0 (the backend records 0 = certain, no error model exists).
 * RETEST_DUE and Regress need a second observation round the mock does not produce; not modelled.
 */
function projectObservations(
  observations: readonly MockObservation[],
): Pick<CompetencyProjection, 'state' | 'provisional' | 'per_dimension' | 'evidence_refs'> {
  const per_dimension: CompetencyProjection['per_dimension'] = {
    RECALL: emptyDimension(),
    UNDERSTANDING: emptyDimension(),
    APPLICATION: emptyDimension(),
    TRANSFER: emptyDimension(),
  };
  for (const dimension of LEARNING_DIMENSIONS) {
    const mine = observations.filter((o) => o.dimension === dimension);
    if (mine.length > 0) {
      per_dimension[dimension] = {
        score_estimate: mine.reduce((sum, o) => sum + o.score, 0) / mine.length,
        n_unaided: mine.length,
        last_observed_at: Math.max(...mine.map((o) => o.observed_at)),
        uncertainty: 0,
      };
    }
  }
  const application = observations.filter((o) => o.dimension === 'APPLICATION');
  let state: CompetencyState = observations.length === 0 ? 'UNASSESSED' : 'INTRODUCED';
  if (application.some((o) => o.score >= 60)) {
    state = 'GUIDED';
  }
  if (application.filter((o) => o.score >= 70 && o.difficulty >= 3).length >= 2) {
    state = 'INDEPENDENT';
  }
  const provisional = observations.some((o) => (o.dimension === 'APPLICATION' || o.dimension === 'TRANSFER') && o.score >= 80);
  return { state, provisional, per_dimension, evidence_refs: observations.map((o) => o.observation_id) };
}

function emptyDimension(): CompetencyProjection['per_dimension']['RECALL'] {
  return { score_estimate: null, n_unaided: 0, last_observed_at: null, uncertainty: null };
}

function emptyDay(): FixtureDay {
  return { day_id: 'D00', day_of_week: 1, planned_minutes: 0, objectives: [], evidence_artifact: '', blocks: [], is_assessment_day: false };
}

/**
 * Port of the backend provisional grade view (g9-app commands/assessment.rs `grade_view` over
 * g9-assessment `score_breakdown`, ADR-0012, LRN-10 option a):
 *  - a level maps through the anchored scale 0/25/50/80/100 (level 3 everywhere = 80 = PASS);
 *  - per criterion the best HUMAN_REVIEWER result counts, else the best DETERMINISTIC_CHECKER;
 *    MODEL_ADVISORY results never count;
 *  - PASS iff every criterion has a counted result, each meets the 50 % floor and the weighted
 *    total is >= 80; a criterion without a counted result makes the total 0 (no estimate);
 *  - `gate_failures` lists critical criteria below the floor (or without a counted result);
 *  - the reliability class of an operator-entered provisional grade is SELF_ADJUDICATED
 *    (the MVP operator is the learner, LRN-11).
 */
export function computeGrade(criteria: CriterionResult[]): GradeView {
  let weightedSum: number | null = 0;
  let totalWeight = 0;
  let floorsMet = true;
  let knownFailure = false;
  const gateFailures: string[] = [];
  for (const def of BASELINE_CRITERIA) {
    totalWeight += def.weight;
    const counted = (source: GradeSource): number | null => {
      const levels = criteria.filter((c) => c.criterion_code === def.code && c.source === source).map((c) => c.score);
      return levels.length === 0 ? null : Math.max(...levels);
    };
    const level = counted('HUMAN_REVIEWER') ?? counted('DETERMINISTIC_CHECKER');
    const score = level === null ? null : (LEVEL_SCALE[level] ?? 0);
    const meetsFloor = score === null ? null : score >= CRITERION_FLOOR_PERCENT;
    if (meetsFloor !== true) {
      floorsMet = false;
    }
    if (meetsFloor === false) {
      knownFailure = true;
    }
    if (def.critical && meetsFloor !== true) {
      gateFailures.push(def.code);
    }
    weightedSum = weightedSum === null || score === null ? null : weightedSum + def.weight * score;
  }
  const total = weightedSum === null || totalWeight === 0 ? 0 : Math.round((weightedSum / totalWeight) * 100) / 100;
  const pass = !knownFailure && weightedSum !== null && totalWeight > 0 && weightedSum >= PASS_THRESHOLD * totalWeight && floorsMet;
  return { total, pass, gate_failures: gateFailures, criteria, reliability_class: 'SELF_ADJUDICATED', final: false };
}

function isProvenance(v: unknown): v is ManifestProvenance {
  return v === 'PROPOSED_SEED' || v === 'RECONCILED_CANDIDATE' || v === 'APPROVED_CANONICAL';
}

/** Curriculum invariants (ARCHITECTURE §4): 12 × 6 × 3; 420/300 minutes; 2,400/week; 28,800 total. */
export function validateManifest(manifest: JsonValue): Violation[] {
  const v: Violation[] = [];
  if (typeof manifest !== 'object' || manifest === null || Array.isArray(manifest)) {
    return [{ code: 'MANIFEST_NOT_OBJECT', path: '$', message: 'manifest must be an object' }];
  }
  const m = manifest as Rec;
  if (typeof m.curriculum_id !== 'string') {
    v.push({ code: 'MISSING_FIELD', path: '$.curriculum_id', message: 'curriculum_id is required' });
  }
  if (typeof m.version !== 'string') {
    v.push({ code: 'MISSING_FIELD', path: '$.version', message: 'version is required' });
  }
  if (!isProvenance(m.provenance)) {
    v.push({ code: 'PROVENANCE_INVALID', path: '$.provenance', message: 'provenance must be PROPOSED_SEED | RECONCILED_CANDIDATE | APPROVED_CANONICAL' });
  }
  const s = typeof m.structure === 'object' && m.structure !== null ? (m.structure as Rec) : null;
  if (s === null) {
    v.push({ code: 'MISSING_FIELD', path: '$.structure', message: 'structure is required' });
    return v;
  }
  const expect = (key: string, value: number): void => {
    if (s[key] !== value) {
      v.push({ code: 'INVARIANT', path: `$.structure.${key}`, message: `${key} must be ${value}` });
    }
  };
  expect('weeks', 12);
  expect('days_per_week', 6);
  expect('objectives_per_day', 3);
  expect('weekday_minutes', 420);
  expect('saturday_minutes', 300);
  expect('weekly_minutes', 2400);
  expect('total_minutes', 28800);
  return v;
}

