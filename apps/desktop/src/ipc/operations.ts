/**
 * Typed registry of IPC operations. Each entry names the operation, its request/response
 * DTOs, the payload schema version and the policy descriptor the backend will evaluate.
 *
 * The JSON Schemas in `schemas/ipc/<op>.request.schema.json` and
 * `schemas/ipc/<op>.response.schema.json` are the cross-language contract; the types here
 * must stay field-for-field equal to them (checked by `src/ipc/schemas.test.ts`).
 *
 * Nothing in this file is authoritative: `g9-app` validates every payload again (§5 step 1).
 */
import type {
  Action,
  ApprovalState,
  AssessmentAttemptState,
  BackupState,
  Classification,
  CompetencyState,
  DataDomain,
  DurationMs,
  IanaZone,
  Id,
  InteractionMode,
  InterventionState,
  JsonValue,
  OperationDescriptor,
  ResourceKind,
  ResourceRef,
  Revision,
  SchemaVersion,
  SessionState,
  SkillInvocationState,
  SourceState,
  TrustClass,
  UtcMillis,
} from './types';
import { SCHEMA_V1 } from './types';

// ---------------------------------------------------------------------------
// Shared DTO fragments
// ---------------------------------------------------------------------------

/** Empty request body. */
export type EmptyRequest = Record<string, never>;

export type ModelUnavailableReason = 'NO_RUNTIME_CONFIGURED' | 'RUNTIME_ERROR' | 'LOADING' | 'BUDGET_EXHAUSTED';

export interface ModelStatus {
  available: boolean;
  /** Present only when `available` is false. */
  reason?: ModelUnavailableReason;
  /** Registered model id when a runtime is configured; absent otherwise. */
  model_id?: string;
}

export type StorageState = 'READY' | 'MIGRATING' | 'UNAVAILABLE' | 'READ_ONLY';

export interface SystemStatus {
  app_version: string;
  schema_version: SchemaVersion;
  storage: { state: StorageState; migrations_applied: number; wal: boolean };
  model: ModelStatus;
  network: { egress_allowed: boolean };
  clock: { now: UtcMillis; zone: IanaZone; monotonic_ok: boolean };
  transport: 'mock' | 'tauri';
  pending_approvals: number;
  last_backup: { backup_id: Id; state: BackupState; created_at: UtcMillis } | null;
}

export type ManifestProvenance = 'PROPOSED_SEED' | 'RECONCILED_CANDIDATE' | 'APPROVED_CANONICAL';

export interface Violation {
  code: string;
  path: string;
  message: string;
}

export type ObjectiveStatus = 'PLANNED' | 'IN_PROGRESS' | 'VERIFIED';

export interface PlannedObjective {
  objective_id: Id;
  seq: number;
  text: string;
  status: ObjectiveStatus;
}

export interface PlannedBlock {
  seq: number;
  block: string;
  minutes: number;
  session_id: Id | null;
}

export interface DailyPlan {
  date: string;
  zone: IanaZone;
  day_id: Id | null;
  week_number: number | null;
  week_title: string | null;
  unscheduled: boolean;
  is_assessment_day: boolean;
  planned_minutes: number;
  evidence_artifact: string | null;
  objectives: PlannedObjective[];
  blocks: PlannedBlock[];
}

export type CalendarEventKind = 'STUDY_BLOCK' | 'ASSESSMENT' | 'REVIEW' | 'APPOINTMENT' | 'PROTECTED_REST' | 'BUSINESS';
export type DstAdjustment = 'GAP' | 'OVERLAP';

export interface CalendarEvent {
  event_id: Id;
  title: string;
  kind: CalendarEventKind;
  start: UtcMillis;
  end: UtcMillis;
  all_day: boolean;
  immutable: boolean;
  day_id: Id | null;
  block: string | null;
  session_id: Id | null;
  adjusted: DstAdjustment | null;
  provenance: TrustClass;
}

export type ProposalChangeOp = 'CREATE' | 'MOVE' | 'DELETE';

export interface ProposalChange {
  op: ProposalChangeOp;
  event: CalendarEvent;
}

export interface ScheduleProposal {
  proposal_id: Id;
  feasible: boolean;
  violated_constraints: { code: string; message: string; event_id: Id | null }[];
  changes: ProposalChange[];
  parameters_sha256: string;
  created_at: UtcMillis;
  expires_at: UtcMillis;
  revision: Revision;
}

export type Continuity = 'VERIFIED' | 'UNCERTAIN';

export interface TimeInterval {
  interval_id: Id;
  start_utc: UtcMillis;
  end_utc: UtcMillis | null;
  continuity: Continuity;
}

export interface TimeCorrection {
  correction_id: Id;
  interval_id: Id;
  credited_ms: DurationMs;
  reason: string;
  corrected_at: UtcMillis;
}

export interface SessionView {
  session_id: Id;
  state: SessionState;
  day_id: Id;
  block: string;
  block_seq: number;
  planned_ms: DurationMs;
  /** Sum of VERIFIED intervals plus accepted corrections. Backend-computed. */
  credited_ms: DurationMs;
  /** Sum of UNCERTAIN intervals that still need an operator correction. */
  uncertain_ms: DurationMs;
  started_at: UtcMillis | null;
  /** Start of the currently open interval while RUNNING; null otherwise. */
  open_interval_started_at: UtcMillis | null;
  requires_correction: boolean;
  intervals: TimeInterval[];
  corrections: TimeCorrection[];
  revision: Revision;
}

export interface NoteView {
  note_id: Id;
  title: string;
  body_markdown: string;
  content_sha256: string;
  classification: Classification;
  domain: DataDomain;
  objective_id: Id | null;
  evidence_refs: Id[];
  version_count: number;
  created_at: UtcMillis;
  updated_at: UtcMillis;
  revision: Revision;
}

/** A note without its body, as `notes.list` shows it; `notes.get` opens one. */
export interface NoteSummary {
  note_id: Id;
  title: string;
  classification: Classification;
  domain: DataDomain;
  objective_id: Id | null;
  version_count: number;
  /** Size of the current body in bytes. */
  size_bytes: number;
  created_at: UtcMillis;
  updated_at: UtcMillis;
  revision: Revision;
}

/** The imported curriculum version the program runs on (`curriculum.program`). */
export interface ProgramCurriculum {
  version_id: Id;
  name: string | null;
  version_label: string;
  provenance: ManifestProvenance;
  weeks: number;
  days: number;
  objectives: number;
  planned_minutes: number;
  imported_at: UtcMillis;
}

/** The Monday of week 1 and the time a planned day starts, as the operator set them. */
export interface ProgramStart {
  /** `YYYY-MM-DD`, a Monday. */
  monday: string;
  /** `HH:MM` in the operator's zone. */
  day_start: string;
  revision: Revision;
}

/** What is set up: the curriculum, where it sits on the calendar, and the competency packs. */
export interface ProgramStatus {
  curriculum: ProgramCurriculum | null;
  program_start: ProgramStart | null;
  competencies: { loaded: number; available: number };
}

export interface PackImported {
  pack_id: string;
  title: string;
  version: string;
  status: string;
  competencies: number;
  created: number;
  existing: number;
  prerequisites_added: number;
}

export interface PackImportResult {
  packs: PackImported[];
  loaded: number;
  available: number;
  prerequisites_added: number;
  prerequisites_pending: number;
}

export interface DayPlanned {
  date: string;
  day_id: Id;
  /** Sessions this call created, one per block that had none. */
  created: SessionView[];
  already_planned: { block: string; seq: number; session_id: Id }[];
}

export type GradeSource = 'DETERMINISTIC_CHECKER' | 'HUMAN_REVIEWER' | 'MODEL_ADVISORY';
/** Anchored rubric level (ADR-0012); the request schema accepts an integer 0..4 only. */
export type AnchoredLevel = 0 | 1 | 2 | 3 | 4;
export type ReliabilityClass = 'DETERMINISTIC' | 'EXTERNAL_REVIEW' | 'SELF_ADJUDICATED';

export interface CriterionResult {
  criterion_code: string;
  weight: number;
  score: number;
  source: GradeSource;
  evidence_refs: Id[];
}

export interface GradeView {
  total: number;
  pass: boolean;
  gate_failures: string[];
  criteria: CriterionResult[];
  reliability_class: ReliabilityClass;
  final: boolean;
}

export interface AttemptView {
  attempt_id: Id;
  assessment_id: Id;
  title: string;
  state: AssessmentAttemptState;
  mode: InteractionMode;
  variant: string;
  retake_of: Id | null;
  started_at: UtcMillis | null;
  submitted_at: UtcMillis | null;
  finalized_at: UtcMillis | null;
  grade: GradeView | null;
  revision: Revision;
}

export type LearningDimension = 'RECALL' | 'UNDERSTANDING' | 'APPLICATION' | 'TRANSFER';

export interface DimensionEstimate {
  /** Null until at least one observation exists. Never invented. */
  score_estimate: number | null;
  n_unaided: number;
  last_observed_at: UtcMillis | null;
  uncertainty: number | null;
}

export interface CompetencyProjection {
  competency_id: Id;
  name: string;
  state: CompetencyState;
  provisional: boolean;
  per_dimension: Record<LearningDimension, DimensionEstimate>;
  contradictions: string[];
  evidence_refs: Id[];
  objective_ids: Id[];
}

export type ReviewItemKind = 'DECLARATIVE' | 'PRACTICAL_TRANSFER';

export interface ReviewItem {
  review_item_id: Id;
  competency_id: Id | null;
  objective_id: Id | null;
  prompt: string;
  kind: ReviewItemKind;
  due_at: UtcMillis;
  last_success_at: UtcMillis | null;
  offset_days: number;
  estimated_ms: DurationMs;
}

export interface Intervention {
  intervention_id: Id;
  state: InterventionState;
  hypothesis_id: Id | null;
  statement: string;
  steps: string[];
  time_budget_ms: DurationMs;
  acceptance: string;
  provenance: TrustClass;
  created_at: UtcMillis;
  revision: Revision;
}

export interface ApprovalRequest {
  approval_id: Id;
  state: ApprovalState;
  operation_name: string;
  operation: OperationDescriptor;
  requested_by: Id;
  requested_at: UtcMillis;
  expires_at: UtcMillis;
  expected_revision: Revision | null;
  decided_at: UtcMillis | null;
  consumed_at: UtcMillis | null;
  summary: string;
  revision: Revision;
}

export type AuditActorKind = 'OPERATOR' | 'SYSTEM' | 'SKILL' | 'MODEL';
export type AuditOutcome = 'OK' | 'ERROR';

export interface AuditEntry {
  id: Id;
  prev_hash: string;
  row_hash: string;
  occurred_at: UtcMillis;
  actor_id: Id;
  actor_kind: AuditActorKind;
  operation: string;
  resource_kind: ResourceKind | null;
  resource_id: Id | null;
  decision: string;
  outcome: AuditOutcome;
  request_id: Id;
  trace_id: Id;
  details_sha256: string;
}

export type IntegrityResult = 'UNKNOWN' | 'OK' | 'FAILED';

export interface BackupView {
  backup_id: Id;
  state: BackupState;
  label: string | null;
  created_at: UtcMillis;
  size_bytes: number | null;
  integrity: IntegrityResult;
  restore_tested_at: UtcMillis | null;
  manifest_sha256: string | null;
  revision: Revision;
}

export type OutputValidation = 'VALID' | 'INVALID' | 'NOT_RUN';

export interface RecommendedAction {
  operation: string;
  payload: JsonValue;
  rationale: string;
}

export interface SkillOutputEnvelope {
  invocation_id: Id;
  skill_code: string;
  state: SkillInvocationState;
  output: JsonValue | null;
  recommended_actions: RecommendedAction[];
  provenance: TrustClass;
  validation: OutputValidation;
  budget: { tokens_used: number; elapsed_ms: DurationMs; tool_calls: number };
}

export type ExportFormat = 'JSON' | 'CSV';
export type ExportState = 'REQUESTED' | 'READY' | 'FAILED';
export type DeletionState = 'REQUESTED' | 'APPLIED' | 'REFUSED';

/**
 * Result of an approved `export.request`: the folder the backend wrote (`<exports>/<export_id>`,
 * an absolute path on this machine), how many files and rows it holds, how many attachment files
 * were copied, and the SHA-256 of `manifest.json` (which lists every file with its SHA-256; it is
 * not signed). `SECRET` rows are never exported; `include_private` adds `PRIVATE` rows only.
 */
export interface ExportResult {
  export_id: Id;
  state: ExportState;
  domains: DataDomain[];
  format: ExportFormat;
  requested_at: UtcMillis;
  location: string;
  /** Files written, `manifest.json` and attachment copies included (>= 1). */
  files: number;
  rows: number;
  /** Attachment files copied into the export. */
  attachments: number;
  manifest_sha256: string;
}

/** Study-session modes the operator may plan (`session.create`; EXAM is refused at the boundary). */
export type PlannedSessionMode = 'STUDY' | 'LAB' | 'REVIEW';

/** Calendar event kinds the operator adds by hand (rest and assessment slots come from the curriculum). */
export type OperatorEventKind = 'STUDY_BLOCK' | 'APPOINTMENT' | 'REVIEW';

/** `review_items.kind` as written by `retention.create_item` (PROCEDURAL is listed as PRACTICAL_TRANSFER). */
export type ReviewItemAuthoredKind = 'DECLARATIVE' | 'PROCEDURAL';

export type ReviewOutcome = 'SUCCESS' | 'FAILURE' | 'SKIPPED';

/** One recorded review and where the baseline ladder put the item (stage reached, next due). */
export interface ReviewRecord {
  review_item_id: Id;
  outcome: ReviewOutcome;
  due_at: UtcMillis;
  stage: number;
  offset_days: number;
}

/** `code` of `competency.create` and `assessment.define` (request schemas): upper case, at most 32 characters. */
export const AUTHORED_CODE_PATTERN = /^[A-Z0-9][A-Z0-9-]{0,31}$/;

/** Assessment kinds the operator may define (the kinds storage can persist). */
export type AuthoredAssessmentKind = 'BLIND_PRACTICAL' | 'CAPSTONE' | 'TRANSFER_CHECK';

/** Declared difficulty band of an assessment version. */
export type Difficulty = 1 | 2 | 3 | 4 | 5;

export interface CompetencyCreated {
  competency_id: Id;
  code: string;
  title: string;
  validity_days: number;
}

export interface AssessmentDefined {
  assessment_id: Id;
  code: string;
  title: string;
  kind: AuthoredAssessmentKind;
  competency_id: Id;
  version_id: Id;
  difficulty: Difficulty;
  /** Criterion codes of the version's rubric (the baseline rubric). */
  criteria: string[];
}

/** The IPC attachment allow-list (g9-evidence `MediaType::ALLOWED`). */
export type AttachmentMediaType =
  | 'text/markdown'
  | 'text/plain'
  | 'application/pdf'
  | 'image/png'
  | 'image/jpeg'
  | 'application/json'
  | 'text/csv';

export interface AttachmentView {
  attachment_id: Id;
  evidence_id: Id;
  note_id: Id | null;
  filename: string;
  media_type: string;
  size_bytes: number;
  sha256: string;
  /** True when identical bytes were already in the content-addressed store (stored once). */
  already_stored: boolean;
}

export type EvidenceKind = 'NOTE' | 'ATTACHMENT' | 'SESSION' | 'ATTEMPT_SUBMISSION' | 'CHECKER_OUTPUT' | 'REVIEWER_COMMENT' | 'EXTERNAL';

export interface EvidenceItemView {
  evidence_id: Id;
  kind: EvidenceKind;
  note_id: Id | null;
  attachment_id: Id | null;
  summary: string;
  sha256: string;
  /** Original file name of an attachment; null for other kinds and for an erased attachment. */
  filename: string | null;
  media_type: string | null;
  size_bytes: number | null;
  created_at: UtcMillis;
  invalidated: boolean;
}

export interface ResearchSource {
  source_id: Id;
  title: string;
  state: SourceState;
  domain_pattern: string;
  license: string | null;
}

// ---------------------------------------------------------------------------
// Operation name → request/response DTO map
// ---------------------------------------------------------------------------

export interface Operations {
  'system.status': { request: EmptyRequest; response: SystemStatus };

  'curriculum.validate': {
    request: { manifest: JsonValue };
    response: { valid: boolean; violations: Violation[]; checksum_sha256: string; provenance: ManifestProvenance | null };
  };
  'curriculum.import': {
    request: { manifest: JsonValue; provenance: Exclude<ManifestProvenance, 'APPROVED_CANONICAL'> };
    response: {
      curriculum_version_id: Id;
      checksum_sha256: string;
      provenance: ManifestProvenance;
      weeks: number;
      days: number;
      objectives: number;
    };
  };
  'curriculum.daily_plan': { request: { date: string; zone?: IanaZone }; response: DailyPlan };
  'curriculum.promote': {
    request: { curriculum_version_id: Id; candidate_manifest: JsonValue; original_manifest: JsonValue };
    response: {
      curriculum_version_id: Id;
      provenance: ManifestProvenance;
      promotion_allowed: boolean;
      original_checksum: string;
      candidate_checksum: string;
      diff_checksum: string;
      summary: {
        unchanged: number;
        changed: number;
        missing_in_candidate: number;
        added_in_candidate: number;
        metadata_changes: string[];
      };
    };
  };

  'calendar.list': { request: { from: UtcMillis; to: UtcMillis; zone?: IanaZone }; response: { zone: IanaZone; events: CalendarEvent[] } };
  'calendar.propose': {
    request: { from: UtcMillis; to: UtcMillis; intent: 'FILL_CURRICULUM' | 'ADD_REVIEWS' | 'RESCHEDULE' };
    response: ScheduleProposal;
  };
  'calendar.apply': { request: { proposal_id: Id }; response: { proposal_id: Id; applied: number; events: CalendarEvent[] } };

  'session.list': { request: { date?: string }; response: { sessions: SessionView[] } };
  'session.prepare': { request: { session_id: Id }; response: SessionView };
  'session.start': { request: { session_id: Id }; response: SessionView };
  'session.pause': { request: { session_id: Id }; response: SessionView };
  'session.resume': { request: { session_id: Id }; response: SessionView };
  'session.complete': { request: { session_id: Id }; response: SessionView };
  'session.correct_time': {
    request: { session_id: Id; interval_id: Id; credited_ms: DurationMs; reason: string };
    response: SessionView;
  };

  'notes.create': {
    request: {
      title: string;
      body_markdown: string;
      classification: Classification;
      domain: DataDomain;
      objective_id?: Id;
      evidence_refs?: Id[];
    };
    response: NoteView;
  };
  'notes.revise': { request: { note_id: Id; title?: string; body_markdown: string }; response: NoteView };

  'assessment.list': {
    request: EmptyRequest;
    response: { definitions: { assessment_id: Id; title: string; day_id: Id; mode: InteractionMode }[]; attempts: AttemptView[] };
  };
  'assessment.begin': { request: { assessment_id: Id; retake_of?: Id }; response: AttemptView };
  'assessment.submit': { request: { attempt_id: Id; evidence_refs: Id[]; notes?: string }; response: AttemptView };
  'assessment.review_evidence': { request: { attempt_id: Id }; response: AttemptView };
  'assessment.grade_provisional': {
    request: { attempt_id: Id; criteria: { criterion_code: string; score: AnchoredLevel; source: GradeSource; evidence_refs: Id[] }[] };
    response: AttemptView;
  };
  'assessment.approve_final': { request: { attempt_id: Id }; response: AttemptView };

  'competency.projection': { request: { competency_id?: Id }; response: { derived_at: UtcMillis; projections: CompetencyProjection[] } };

  'retention.due': {
    request: { as_of?: UtcMillis; budget_ms?: DurationMs };
    response: { as_of: UtcMillis; budget_ms: DurationMs; due: ReviewItem[]; overdue: ReviewItem[]; overflow: ReviewItem[] };
  };

  'interventions.list': { request: { state?: InterventionState }; response: { interventions: Intervention[] } };

  'approvals.list': { request: { state?: ApprovalState }; response: { approvals: ApprovalRequest[] } };
  'approvals.decide': { request: { approval_id: Id; decision: 'APPROVE' | 'REJECT' }; response: ApprovalRequest };

  'audit.recent': { request: { limit?: number; before?: UtcMillis }; response: { entries: AuditEntry[]; chain_ok: boolean | null } };

  'backup.create': { request: { label?: string }; response: BackupView };
  'backup.verify': { request: { backup_id: Id }; response: BackupView };
  'backup.restore_test': { request: { backup_id: Id }; response: BackupView };

  'skills.invoke': { request: { skill_code: string; input: JsonValue; mode?: InteractionMode }; response: SkillOutputEnvelope };

  'export.request': {
    request: { domains: DataDomain[]; format: ExportFormat; include_private: boolean };
    response: ExportResult;
  };
  'deletion.request': {
    request: { resource: ResourceRef; reason: string };
    response: { deletion_id: Id; state: DeletionState; resource: ResourceRef; tombstone_sha256: string | null; requested_at: UtcMillis };
  };

  'session.create': {
    /** `planned_minutes` 1..600; `planned_start` defaults to now; `objective_id` is an objective code of the imported curriculum. */
    request: { planned_minutes: number; planned_start?: UtcMillis; mode?: PlannedSessionMode; objective_id?: Id };
    response: SessionView;
  };

  'calendar.create_event': {
    request: { title: string; kind: OperatorEventKind; start: UtcMillis; end: UtcMillis; all_day?: boolean };
    response: CalendarEvent;
  };
  'calendar.update_event': {
    request: { event_id: Id; title?: string; kind?: OperatorEventKind; start?: UtcMillis; end?: UtcMillis };
    response: CalendarEvent;
  };
  'calendar.delete_event': { request: { event_id: Id }; response: { event_id: Id; deleted: boolean } };

  'retention.create_item': {
    request: { prompt: string; answer?: string; kind?: ReviewItemAuthoredKind; competency_id?: Id };
    response: ReviewItem;
  };
  'retention.record_review': {
    request: { review_item_id: Id; outcome: ReviewOutcome; response_ms?: DurationMs; rating?: 1 | 2 | 3 | 4; session_id?: Id };
    response: ReviewRecord;
  };

  'competency.create': {
    /** `code` matches `^[A-Z0-9][A-Z0-9-]{0,31}$`; `validity_days` 1..3650 (default 180). */
    request: { code: string; title: string; description?: string; validity_days?: number };
    response: CompetencyCreated;
  };
  'assessment.define': {
    request: { code: string; title: string; kind: AuthoredAssessmentKind; competency_id: Id; task_statement: string; difficulty: Difficulty };
    response: AssessmentDefined;
  };

  'evidence.attach_file': {
    /** `content_base64`: standard padded base64 of at most 16 MiB of bytes. Without `note_id` the file is LEARNING data with `classification` (default INTERNAL). */
    request: { note_id?: Id; filename: string; media_type: AttachmentMediaType; content_base64: string; classification?: Classification };
    response: AttachmentView;
  };
  'evidence.from_note': { request: { note_id: Id }; response: { evidence_id: Id; kind: 'NOTE'; note_id: Id; summary: string } };
  'evidence.list': { request: { note_id?: Id; limit?: number }; response: { items: EvidenceItemView[] } };

  'curriculum.program': { request: EmptyRequest; response: ProgramStatus };
  'curriculum.set_program_start': {
    /** `date` must be a Monday; `day_start` is `HH:MM` (default 09:00). */
    request: { date: string; day_start?: string };
    response: { curriculum_id: Id; monday: string; day_start: string };
  };
  'session.plan_day': { request: { date: string }; response: DayPlanned };
  'notes.list': { request: { limit?: number }; response: { notes: NoteSummary[] } };
  'notes.get': { request: { note_id: Id }; response: NoteView };
  'competency.import_pack': { request: { pack_ids?: string[] }; response: PackImportResult };
  'backup.list': { request: EmptyRequest; response: { backups: BackupView[]; unlisted: number } };
}

export type OperationName = keyof Operations;
export type RequestOf<K extends OperationName> = Operations[K]['request'];
export type ResponseOf<K extends OperationName> = Operations[K]['response'];

// ---------------------------------------------------------------------------
// Registry metadata
// ---------------------------------------------------------------------------

export interface OperationSpec {
  readonly name: OperationName;
  readonly schemaVersion: SchemaVersion;
  /** False for read-only queries (pipeline steps 4–9 are skipped). */
  readonly mutating: boolean;
  readonly action: Action;
  readonly resourceKind: ResourceKind;
  readonly defaultDomain: DataDomain;
  readonly defaultMode: InteractionMode;
  /** True when the backend policy table is expected to answer APPROVAL_REQUIRED. */
  readonly approvalExpected: boolean;
}

function spec(
  name: OperationName,
  action: Action,
  resourceKind: ResourceKind,
  options: Partial<Pick<OperationSpec, 'mutating' | 'defaultDomain' | 'defaultMode' | 'approvalExpected' | 'schemaVersion'>> = {},
): OperationSpec {
  return {
    name,
    action,
    resourceKind,
    schemaVersion: options.schemaVersion ?? SCHEMA_V1,
    mutating: options.mutating ?? true,
    defaultDomain: options.defaultDomain ?? 'LEARNING',
    defaultMode: options.defaultMode ?? 'STUDY',
    approvalExpected: options.approvalExpected ?? false,
  };
}

export const OPERATIONS: Readonly<Record<OperationName, OperationSpec>> = {
  'system.status': spec('system.status', 'READ', 'APP_SETTING', { mutating: false, defaultDomain: 'SYSTEM' }),

  'curriculum.validate': spec('curriculum.validate', 'READ', 'CURRICULUM', { mutating: false }),
  // Policy rule `operator.import.curriculum.approval`: ApprovalRequired(CURRICULUM_IMPORT).
  'curriculum.import': spec('curriculum.import', 'IMPORT', 'CURRICULUM_VERSION', { defaultMode: 'MAINTENANCE', approvalExpected: true }),
  'curriculum.daily_plan': spec('curriculum.daily_plan', 'READ', 'CURRICULUM_VERSION', { mutating: false }),
  'curriculum.promote': spec('curriculum.promote', 'PROMOTE_CANONICAL', 'CURRICULUM_VERSION', { defaultMode: 'MAINTENANCE', approvalExpected: true }),

  'calendar.list': spec('calendar.list', 'READ', 'CALENDAR_EVENT', { mutating: false }),
  'calendar.propose': spec('calendar.propose', 'PROPOSE', 'SCHEDULE_PROPOSAL'),
  'calendar.apply': spec('calendar.apply', 'APPLY', 'SCHEDULE_PROPOSAL', { approvalExpected: true }),

  'session.list': spec('session.list', 'READ', 'STUDY_SESSION', { mutating: false }),
  'session.prepare': spec('session.prepare', 'UPDATE', 'STUDY_SESSION'),
  'session.start': spec('session.start', 'UPDATE', 'STUDY_SESSION'),
  'session.pause': spec('session.pause', 'UPDATE', 'STUDY_SESSION'),
  'session.resume': spec('session.resume', 'UPDATE', 'STUDY_SESSION'),
  'session.complete': spec('session.complete', 'UPDATE', 'STUDY_SESSION'),
  'session.correct_time': spec('session.correct_time', 'CORRECT', 'TIME_INTERVAL'),

  'notes.create': spec('notes.create', 'CREATE', 'NOTE'),
  'notes.revise': spec('notes.revise', 'UPDATE', 'NOTE'),

  'assessment.list': spec('assessment.list', 'READ', 'ATTEMPT', { mutating: false, defaultMode: 'REVIEW' }),
  'assessment.begin': spec('assessment.begin', 'UPDATE', 'ATTEMPT', { defaultMode: 'EXAM' }),
  'assessment.submit': spec('assessment.submit', 'UPDATE', 'ATTEMPT', { defaultMode: 'EXAM' }),
  'assessment.review_evidence': spec('assessment.review_evidence', 'UPDATE', 'ATTEMPT', { defaultMode: 'REVIEW' }),
  'assessment.grade_provisional': spec('assessment.grade_provisional', 'UPDATE', 'GRADE_DECISION', { defaultMode: 'REVIEW' }),
  'assessment.approve_final': spec('assessment.approve_final', 'FINALIZE', 'GRADE_DECISION', {
    defaultMode: 'REVIEW',
    approvalExpected: true,
  }),

  'competency.projection': spec('competency.projection', 'READ', 'COMPETENCY', { mutating: false }),
  'retention.due': spec('retention.due', 'READ', 'REVIEW_ITEM', { mutating: false }),
  'interventions.list': spec('interventions.list', 'READ', 'INTERVENTION', { mutating: false }),

  'approvals.list': spec('approvals.list', 'READ', 'APPROVAL', { mutating: false, defaultDomain: 'SYSTEM' }),
  'approvals.decide': spec('approvals.decide', 'APPROVE', 'APPROVAL', { defaultDomain: 'SYSTEM' }),
  'audit.recent': spec('audit.recent', 'READ', 'AUDIT_METADATA', { mutating: false, defaultDomain: 'SYSTEM' }),

  'backup.create': spec('backup.create', 'CREATE', 'BACKUP', { defaultDomain: 'SYSTEM', defaultMode: 'MAINTENANCE' }),
  'backup.verify': spec('backup.verify', 'UPDATE', 'BACKUP', { defaultDomain: 'SYSTEM', defaultMode: 'MAINTENANCE' }),
  'backup.restore_test': spec('backup.restore_test', 'UPDATE', 'BACKUP', { defaultDomain: 'SYSTEM', defaultMode: 'MAINTENANCE' }),

  'skills.invoke': spec('skills.invoke', 'INVOKE', 'SKILL'),

  'export.request': spec('export.request', 'EXPORT', 'EXPORT', { defaultDomain: 'SYSTEM', approvalExpected: true }),
  'deletion.request': spec('deletion.request', 'DELETE', 'NOTE', { approvalExpected: true }),

  'session.create': spec('session.create', 'CREATE', 'STUDY_SESSION'),

  'calendar.create_event': spec('calendar.create_event', 'CREATE', 'CALENDAR_EVENT'),
  'calendar.update_event': spec('calendar.update_event', 'UPDATE', 'CALENDAR_EVENT'),
  // Policy rule `operator.delete.approval`: ApprovalRequired(DATA_DELETE), confirmed natively.
  'calendar.delete_event': spec('calendar.delete_event', 'DELETE', 'CALENDAR_EVENT', { approvalExpected: true }),

  'retention.create_item': spec('retention.create_item', 'CREATE', 'REVIEW_ITEM'),
  'retention.record_review': spec('retention.record_review', 'UPDATE', 'REVIEW_ITEM'),

  'competency.create': spec('competency.create', 'CREATE', 'COMPETENCY'),
  'assessment.define': spec('assessment.define', 'CREATE', 'ASSESSMENT_DEFINITION'),

  // Approval only when the owning note is SECURITY_SENSITIVE (`operator.security_sensitive.write.approval`).
  'evidence.attach_file': spec('evidence.attach_file', 'CREATE', 'ATTACHMENT'),
  'evidence.from_note': spec('evidence.from_note', 'CREATE', 'EVIDENCE_ITEM'),
  'evidence.list': spec('evidence.list', 'READ', 'EVIDENCE_ITEM', { mutating: false }),

  'curriculum.program': spec('curriculum.program', 'READ', 'CURRICULUM', { mutating: false }),
  'curriculum.set_program_start': spec('curriculum.set_program_start', 'UPDATE', 'CURRICULUM'),
  'session.plan_day': spec('session.plan_day', 'CREATE', 'STUDY_SESSION'),
  'notes.list': spec('notes.list', 'READ', 'NOTE', { mutating: false }),
  'notes.get': spec('notes.get', 'READ', 'NOTE', { mutating: false }),
  'competency.import_pack': spec('competency.import_pack', 'CREATE', 'COMPETENCY'),
  'backup.list': spec('backup.list', 'READ', 'BACKUP', { mutating: false, defaultDomain: 'SYSTEM' }),
};

export const OPERATION_NAMES = Object.keys(OPERATIONS) as OperationName[];

export function isOperationName(value: string): value is OperationName {
  return Object.hasOwn(OPERATIONS, value);
}
