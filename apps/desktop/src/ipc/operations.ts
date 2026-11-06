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

export type GradeSource = 'DETERMINISTIC_CHECKER' | 'HUMAN_REVIEWER' | 'MODEL_ADVISORY';
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
    request: { attempt_id: Id; criteria: { criterion_code: string; score: number; source: GradeSource; evidence_refs: Id[] }[] };
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
    response: { export_id: Id; state: ExportState; domains: DataDomain[]; format: ExportFormat; requested_at: UtcMillis };
  };
  'deletion.request': {
    request: { resource: ResourceRef; reason: string };
    response: { deletion_id: Id; state: DeletionState; resource: ResourceRef; tombstone_sha256: string | null; requested_at: UtcMillis };
  };
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
  'curriculum.import': spec('curriculum.import', 'IMPORT', 'CURRICULUM_VERSION', { defaultMode: 'MAINTENANCE' }),
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
};

export const OPERATION_NAMES = Object.keys(OPERATIONS) as OperationName[];

export function isOperationName(value: string): value is OperationName {
  return Object.hasOwn(OPERATIONS, value);
}
