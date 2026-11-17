/**
 * TypeScript mirrors of the serde shapes in `crates/g9-domain` (envelope.rs, policy.rs,
 * state.rs, mode.rs, privacy.rs, actor.rs, ids.rs, revision.rs, time.rs).
 *
 * Rules of the mirror:
 * - enums serialize as SCREAMING_SNAKE_CASE strings;
 * - `ActorKind` is internally tagged by `kind`; `Decision` is tagged by `decision`;
 * - typed ids are plain strings; instants are integer epoch milliseconds; durations are
 *   non-negative integer milliseconds; `Revision` is an unsigned integer.
 *
 * Nothing here is authoritative on its own: the backend validates every payload again.
 */

// ---------------------------------------------------------------------------
// Primitive aliases (ids.rs, time.rs, revision.rs)
// ---------------------------------------------------------------------------

/** Any typed id (`SessionId`, `AttemptId`, ...). `[A-Za-z0-9._:-]{1,128}`. */
export type Id = string;
/** UTC instant as integer milliseconds since the Unix epoch (`UtcMillis`). */
export type UtcMillis = number;
/** Non-negative elapsed duration in integer milliseconds (`DurationMs`). */
export type DurationMs = number;
/** Validated IANA zone identifier, e.g. `Europe/Berlin`. */
export type IanaZone = string;
/** Optimistic-concurrency counter (`Revision`, transparent u64). */
export type Revision = number;

/** Plain JSON value (`serde_json::Value`). */
export type JsonValue = string | number | boolean | null | JsonValue[] | JsonObject;
export interface JsonObject {
  [key: string]: JsonValue;
}

// ---------------------------------------------------------------------------
// Privacy (privacy.rs) and mode (mode.rs)
// ---------------------------------------------------------------------------

export const DATA_DOMAINS = ['LEARNING', 'BUSINESS', 'PERSONAL', 'SECURITY_SENSITIVE', 'SYSTEM'] as const;
export type DataDomain = (typeof DATA_DOMAINS)[number];

export const CLASSIFICATIONS = ['PUBLIC', 'INTERNAL', 'PRIVATE', 'SECRET'] as const;
export type Classification = (typeof CLASSIFICATIONS)[number];

export const INTERACTION_MODES = ['STUDY', 'LAB', 'EXAM', 'REVIEW', 'EXECUTIVE', 'RESEARCH', 'MAINTENANCE'] as const;
export type InteractionMode = (typeof INTERACTION_MODES)[number];

// ---------------------------------------------------------------------------
// Actor (actor.rs)
// ---------------------------------------------------------------------------

export type ActorKind =
  | { kind: 'OPERATOR' }
  | { kind: 'SYSTEM' }
  | { kind: 'SKILL'; skill_id: Id }
  | { kind: 'MODEL' };

export interface Actor {
  id: Id;
  kind: ActorKind;
}

// ---------------------------------------------------------------------------
// Envelope (envelope.rs)
// ---------------------------------------------------------------------------

/** The closed 13-category error set. Adding a category requires a schema version bump. */
export const ERROR_CATEGORIES = [
  'INVALID_INPUT',
  'UNAUTHORIZED',
  'APPROVAL_REQUIRED',
  'STALE_REVISION',
  'PRECONDITION_FAILED',
  'NOT_FOUND',
  'CONFLICT',
  'RESOURCE_UNAVAILABLE',
  'TIMEOUT',
  'CANCELLED',
  'RETRYABLE_STORAGE',
  'DATA_CORRUPTION',
  'INTERNAL',
] as const;
export type ErrorCategory = (typeof ERROR_CATEGORIES)[number];

export function isRetryableCategory(category: ErrorCategory): boolean {
  return category === 'RETRYABLE_STORAGE' || category === 'TIMEOUT' || category === 'RESOURCE_UNAVAILABLE';
}

/** The only error type that crosses the IPC boundary. Never carries secrets or backtraces. */
export interface G9Error {
  category: ErrorCategory;
  /** Machine-readable code, e.g. `curriculum.invariant_violated`. */
  code: string;
  /** Human-readable, secret-free message. */
  message: string;
  /** Optional structured, secret-free details. */
  details?: JsonValue;
  /** Set when `category === 'APPROVAL_REQUIRED'` and an approval request was created. */
  approval_id?: Id;
}

export interface ResourceRef {
  kind: ResourceKind;
  id: Id;
}

export interface Scope {
  domain: DataDomain;
  mode: InteractionMode;
}

export interface SchemaVersion {
  major: number;
  minor: number;
}

export const SCHEMA_V1: SchemaVersion = { major: 1, minor: 0 };

/** A reader at `reader` accepts `payload` if the major matches and the payload minor is not newer. */
export function schemaAccepts(reader: SchemaVersion, payload: SchemaVersion): boolean {
  return reader.major === payload.major && payload.minor <= reader.minor;
}

/**
 * Size limits the backend enforces at the IPC boundary (app-9): the `maxLength` (characters) and
 * `maxItems` of `schemas/ipc/*.request.schema.json`, and the serialized-payload byte caps of
 * `crates/g9-app/src/dispatch.rs` (`payload_cap`, checked before the schema).
 */
export const IPC_LIMITS = {
  /** `notes.create` / `notes.revise` `title`. */
  noteTitleChars: 200,
  /** `session.correct_time` `reason`. */
  correctionReasonChars: 500,
  /** `backup.create` `label`. */
  backupLabelChars: 200,
  /** `skills.invoke` `skill_code`. */
  skillCodeChars: 128,
  /** `assessment.grade_provisional` `criteria[].criterion_code`. */
  criterionCodeChars: 64,
  /** `evidence_refs` of `notes.create`, `assessment.submit` and each graded criterion. */
  evidenceRefs: 100,
  /** `assessment.grade_provisional` `criteria`. */
  gradeCriteria: 64,
  /** Serialized payload of any operation not listed below (`MAX_PAYLOAD_BYTES`). */
  payloadBytes: 1024 * 1024,
  /** `evidence.attach_file` (`MAX_ATTACHMENT_PAYLOAD_BYTES`): a 16 MiB file in base64. */
  attachmentPayloadBytes: 23 * 1024 * 1024,
  /** `notes.create`, `notes.revise`, `assessment.submit` (`MAX_NOTE_PAYLOAD_BYTES`). */
  notePayloadBytes: 6 * 4 * 1024 * 1024 + 1024 * 1024,
  /** `curriculum.import` (`MAX_MANIFEST_PAYLOAD_BYTES`). */
  manifestPayloadBytes: 8 * 1024 * 1024,
} as const;

/** The largest serialized payload, in bytes, the backend accepts for `op` (`dispatch::payload_cap`). */
export function payloadCap(op: string): number {
  switch (op) {
    case 'evidence.attach_file':
      return IPC_LIMITS.attachmentPayloadBytes;
    case 'notes.create':
    case 'notes.revise':
    case 'assessment.submit':
      return IPC_LIMITS.notePayloadBytes;
    case 'curriculum.import':
    case 'curriculum.validate':
      return IPC_LIMITS.manifestPayloadBytes;
    case 'curriculum.promote':
      // Two manifests: the candidate and the original.
      return 2 * IPC_LIMITS.manifestPayloadBytes;
    default:
      return IPC_LIMITS.payloadBytes;
  }
}

/**
 * Metadata the UI sends with every command (handoff §6, ADR-0013, ARCHITECTURE Addendum A.8).
 *
 * The wire shape deliberately has NO `actor`: incoming IPC meta never sets identity. The
 * backend (`g9-app`) assigns the operator actor from the session and refuses payloads that
 * try to carry one. `schemas/ipc/command-meta.schema.json` describes exactly this shape.
 */
export interface ClientCommandMeta {
  request_id: Id;
  /** Operation name, e.g. `session.start`. */
  operation: string;
  resource?: ResourceRef;
  expected_revision?: Revision;
  idempotency_key: Id;
  scope: Scope;
  payload_schema_version: SchemaVersion;
  approval_id?: Id;
  trace_id: Id;
  issued_at: UtcMillis;
}

/**
 * Full command metadata as the backend sees it after assigning the actor
 * (`g9_domain::envelope::CommandMeta`). The UI never constructs this type for sending;
 * the mock transport builds it internally to mirror `g9-app`.
 */
export interface CommandMeta extends ClientCommandMeta {
  actor: Actor;
}

/** Success envelope. `warnings` is omitted by the backend when empty. */
export interface CommandOutcome<T> {
  result: T;
  revision?: Revision;
  audit_ref?: Id;
  /** True when this response was replayed from the idempotency store. */
  replayed: boolean;
  warnings?: string[];
}

// ---------------------------------------------------------------------------
// Policy vocabulary (policy.rs)
// ---------------------------------------------------------------------------

export const RESOURCE_KINDS = [
  'CURRICULUM',
  'CURRICULUM_VERSION',
  'CALENDAR_EVENT',
  'SCHEDULE_PROPOSAL',
  'STUDY_SESSION',
  'TIME_INTERVAL',
  'NOTE',
  'ATTACHMENT',
  'EVIDENCE_ITEM',
  'ASSESSMENT_DEFINITION',
  'ATTEMPT',
  'GRADE_DECISION',
  'COMPETENCY',
  'OBSERVATION',
  'LEARNER_HYPOTHESIS',
  'REVIEW_ITEM',
  'INTERVENTION',
  'EXPERIMENT',
  'GOAL',
  'PROJECT',
  'MILESTONE',
  'DECISION',
  'RESEARCH_SOURCE',
  'KNOWLEDGE_DOCUMENT',
  'MODEL',
  'SKILL',
  'SKILL_INVOCATION',
  'APPROVAL',
  'BACKUP',
  'POLICY',
  'EXPORT',
  'FILESYSTEM_PATH',
  'NETWORK_HOST',
  'PROCESS',
  'APP_SETTING',
  'AUDIT_METADATA',
] as const;
export type ResourceKind = (typeof RESOURCE_KINDS)[number];

export const ACTIONS = [
  'READ',
  'CREATE',
  'UPDATE',
  'DELETE',
  'EXPORT',
  'IMPORT',
  'PROMOTE_CANONICAL',
  'PROPOSE',
  'APPROVE',
  'APPLY',
  'FINALIZE',
  'INVOKE',
  'EXECUTE',
  'CONNECT',
  'LOAD',
  'CORRECT',
] as const;
export type Action = (typeof ACTIONS)[number];

export const CAPABILITIES = [
  'PROTECTED_RECORD_WRITE',
  'GRADE_FINALIZE',
  'SCHEDULE_APPLY',
  'CURRICULUM_IMPORT',
  'CURRICULUM_PROMOTE',
  'SKILL_INVOKE',
  'MODEL_INFERENCE',
  'FILESYSTEM_READ',
  'FILESYSTEM_WRITE',
  'NETWORK_EGRESS',
  'SHELL_EXEC',
  'DATA_EXPORT',
  'DATA_DELETE',
  'POLICY_MODIFY',
  'MODEL_LOAD',
  'LAB_ORCHESTRATE',
  'CROSS_DOMAIN_READ',
] as const;
export type Capability = (typeof CAPABILITIES)[number];

/** Mirrors `Capability::operator_only()`. */
export function isOperatorOnlyCapability(capability: Capability): boolean {
  switch (capability) {
    case 'SHELL_EXEC':
    case 'NETWORK_EGRESS':
    case 'FILESYSTEM_WRITE':
    case 'POLICY_MODIFY':
    case 'GRADE_FINALIZE':
    case 'CURRICULUM_PROMOTE':
    case 'DATA_DELETE':
    case 'MODEL_LOAD':
    case 'LAB_ORCHESTRATE':
      return true;
    case 'PROTECTED_RECORD_WRITE':
    case 'SCHEDULE_APPLY':
    case 'CURRICULUM_IMPORT':
    case 'SKILL_INVOKE':
    case 'MODEL_INFERENCE':
    case 'FILESYSTEM_READ':
    case 'DATA_EXPORT':
    case 'CROSS_DOMAIN_READ':
      return false;
  }
}

/** Exact description of an operation for policy evaluation and approval binding. */
export interface OperationDescriptor {
  action: Action;
  resource: ResourceRef;
  scope: Scope;
  /** Hex SHA-256 of the canonical JSON of the full operation parameters. */
  parameters_sha256: string;
}

/** Outcome of a policy evaluation; internally tagged by `decision`. */
export type Decision =
  | { decision: 'ALLOW'; rule_id: string }
  | { decision: 'DENY'; rule_id: string; reason: string }
  | { decision: 'APPROVAL_REQUIRED'; rule_id: string; reason: string; required_capability: Capability };

// ---------------------------------------------------------------------------
// State machines (state.rs) — states and events
// ---------------------------------------------------------------------------

export const SESSION_STATES = [
  'PLANNED',
  'READY',
  'RUNNING',
  'PAUSED',
  'COMPLETED',
  'REVIEWED',
  'CLOSED',
  'INTERRUPTED',
  'CANCELLED',
] as const;
export type SessionState = (typeof SESSION_STATES)[number];
export const SESSION_EVENTS = [
  'PREPARE',
  'START',
  'PAUSE',
  'RESUME',
  'COMPLETE',
  'REVIEW',
  'CLOSE',
  'INTERRUPT',
  'CANCEL',
  'RECOVER',
] as const;
export type SessionEvent = (typeof SESSION_EVENTS)[number];

export const ASSESSMENT_ATTEMPT_STATES = [
  'AVAILABLE',
  'STARTED',
  'SUBMITTED',
  'EVIDENCE_REVIEW',
  'GRADED_PROVISIONAL',
  'APPROVED_FINAL',
  'VOIDED',
] as const;
export type AssessmentAttemptState = (typeof ASSESSMENT_ATTEMPT_STATES)[number];
export const ASSESSMENT_EVENTS = [
  'BEGIN',
  'SUBMIT',
  'REVIEW_EVIDENCE',
  'GRADE_PROVISIONAL',
  'APPROVE_FINAL',
  'VOID',
  'ABANDON',
] as const;
export type AssessmentEvent = (typeof ASSESSMENT_EVENTS)[number];

export const INTERVENTION_STATES = [
  'HYPOTHESIS',
  'PROPOSED',
  'REVIEWED',
  'APPROVED',
  'REJECTED',
  'SCHEDULED',
  'APPLIED',
  'RETESTED',
  'EVALUATED',
  'RETAINED',
  'REVISED',
  'RETIRED',
] as const;
export type InterventionState = (typeof INTERVENTION_STATES)[number];

export const SKILL_INVOCATION_STATES = [
  'REQUESTED',
  'PREFLIGHT',
  'AUTHORIZED',
  'RUNNING',
  'VALIDATING',
  'COMPLETED',
  'ERROR',
  'DENIED',
  'TIMED_OUT',
  'CANCELLED',
] as const;
export type SkillInvocationState = (typeof SKILL_INVOCATION_STATES)[number];

export const SOURCE_STATES = [
  'UNTRUSTED',
  'QUARANTINED',
  'EVALUATED',
  'APPROVED_REFERENCE',
  'SUPERSEDED',
  'REVOKED',
] as const;
export type SourceState = (typeof SOURCE_STATES)[number];

export const BACKUP_STATES = [
  'REQUESTED',
  'SNAPSHOT_CREATED',
  'CHECKED',
  'RESTORE_TESTED',
  'RETAINED',
  'EXPIRED',
  'FAILED',
] as const;
export type BackupState = (typeof BACKUP_STATES)[number];
export const BACKUP_EVENTS = ['SNAPSHOT', 'CHECK', 'TEST_RESTORE', 'RETAIN', 'EXPIRE', 'FAIL'] as const;
export type BackupEvent = (typeof BACKUP_EVENTS)[number];

export const COMPETENCY_STATES = [
  'UNASSESSED',
  'INTRODUCED',
  'GUIDED',
  'INDEPENDENT',
  'VALIDATED',
  'RETEST_DUE',
] as const;
export type CompetencyState = (typeof COMPETENCY_STATES)[number];

export const APPROVAL_STATES = ['PENDING', 'APPROVED', 'REJECTED', 'EXPIRED', 'CONSUMED', 'REVOKED'] as const;
export type ApprovalState = (typeof APPROVAL_STATES)[number];
export const APPROVAL_EVENTS = ['APPROVE', 'REJECT', 'EXPIRE', 'CONSUME', 'REVOKE'] as const;
export type ApprovalEvent = (typeof APPROVAL_EVENTS)[number];

// ---------------------------------------------------------------------------
// Trust / provenance (trust.rs subset used by the UI labels)
// ---------------------------------------------------------------------------

export const TRUST_CLASSES = [
  'OPERATOR',
  'SYSTEM_DETERMINISTIC',
  'APPROVED_REFERENCE',
  'IMPORTED_DOCUMENT',
  'SKILL_PACKAGE',
  'MODEL_OUTPUT',
  'UNKNOWN',
] as const;
export type TrustClass = (typeof TRUST_CLASSES)[number];

/**
 * UI-level provenance label. Every piece of text shown to the operator carries one:
 * `VERIFIED` = operator/system-deterministic record; `RECOMMENDATION` = model or skill
 * output that can only propose; `PROPOSED` = a hypothesis/plan awaiting an operator decision.
 */
export type ProvenanceLabel = 'VERIFIED' | 'RECOMMENDATION' | 'PROPOSED';

export function provenanceForTrust(trust: TrustClass): ProvenanceLabel {
  switch (trust) {
    case 'OPERATOR':
    case 'SYSTEM_DETERMINISTIC':
    case 'APPROVED_REFERENCE':
      return 'VERIFIED';
    case 'MODEL_OUTPUT':
    case 'SKILL_PACKAGE':
      return 'RECOMMENDATION';
    case 'IMPORTED_DOCUMENT':
    case 'UNKNOWN':
      return 'PROPOSED';
  }
}
