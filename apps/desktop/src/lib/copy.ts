/**
 * The operator's vocabulary (docs/ui/ux-copy.md). The backend speaks in exact names
 * (`GRADED_PROVISIONAL`, `calendar.delete_event`, `DATA_DELETE`); the window says the same
 * thing in plain words and keeps the exact name one step away (a `title`, the audit screen, an
 * error's code). Every function here falls back to the backend's own name, lower-cased, so a
 * state the window has never heard of is still shown and never hidden.
 */
import { sentenceCase } from './format';

const STATE_LABELS: Readonly<Record<string, string>> = {
  // Sessions
  PLANNED: 'Planned',
  READY: 'Ready',
  RUNNING: 'Running',
  PAUSED: 'Paused',
  COMPLETED: 'Completed',
  REVIEWED: 'Reviewed',
  CLOSED: 'Closed',
  INTERRUPTED: 'Interrupted',
  CANCELLED: 'Cancelled',
  // Assessment attempts
  AVAILABLE: 'Available',
  STARTED: 'In progress',
  SUBMITTED: 'Submitted',
  EVIDENCE_REVIEW: 'Evidence in review',
  GRADED_PROVISIONAL: 'Graded, not final',
  APPROVED_FINAL: 'Final',
  VOIDED: 'Voided',
  // Approvals
  PENDING: 'Waiting for you',
  APPROVED: 'Approved',
  REJECTED: 'Rejected',
  EXPIRED: 'Expired',
  CONSUMED: 'Used',
  REVOKED: 'Revoked',
  // Backups
  REQUESTED: 'Requested',
  SNAPSHOT_CREATED: 'Snapshot taken',
  CHECKED: 'Verified',
  RESTORE_TESTED: 'Restore tested',
  RETAINED: 'Kept',
  FAILED: 'Failed',
  // Competencies
  UNASSESSED: 'Not assessed',
  INTRODUCED: 'Introduced',
  GUIDED: 'Guided',
  INDEPENDENT: 'Independent',
  VALIDATED: 'Validated',
  RETEST_DUE: 'Retest due',
  // Provenance and standing
  VERIFIED: 'Verified',
  PROPOSED: 'Proposed',
  RECOMMENDATION: 'Advice',
  PROVISIONAL: 'Provisional',
  UNCERTAIN: 'Uncertain',
  SELF_ADJUDICATED: 'Self-graded',
  // Exports and erasures
  APPLIED: 'Done',
  REFUSED: 'Refused',
  // Host
  OFFLINE: 'Offline',
  UNAVAILABLE: 'Unavailable',
  READ_ONLY: 'Read-only',
  DENIED: 'Denied',
  ERROR: 'Error',
  TIMED_OUT: 'Timed out',
};

/** A backend state in the operator's words; unknown states are shown lower-cased, never dropped. */
export function stateLabel(state: string): string {
  return STATE_LABELS[state] ?? sentenceCase(state);
}

const CAPABILITY_LABELS: Readonly<Record<string, string>> = {
  PROTECTED_RECORD_WRITE: 'write a protected record',
  GRADE_FINALIZE: 'make a grade final',
  SCHEDULE_APPLY: 'apply a schedule',
  CURRICULUM_IMPORT: 'import a curriculum',
  CURRICULUM_PROMOTE: 'promote a curriculum version',
  SKILL_INVOKE: 'run a skill',
  MODEL_INFERENCE: 'ask the model',
  FILESYSTEM_READ: 'read a file on this machine',
  FILESYSTEM_WRITE: 'write a file on this machine',
  NETWORK_EGRESS: 'reach the network',
  SHELL_EXEC: 'run a program',
  DATA_EXPORT: 'export data',
  DATA_DELETE: 'delete data',
  POLICY_MODIFY: 'change policy',
  MODEL_LOAD: 'load a model',
  LAB_ORCHESTRATE: 'drive a lab',
  CROSS_DOMAIN_READ: 'read across data areas',
};

/** What a capability lets a command do, as a verb phrase ("delete data"). */
export function capabilityLabel(capability: string): string {
  return CAPABILITY_LABELS[capability] ?? sentenceCase(capability).toLowerCase();
}

const OPERATION_LABELS: Readonly<Record<string, string>> = {
  'curriculum.import': 'Import a curriculum',
  'curriculum.promote': 'Promote a curriculum version',
  'calendar.apply': 'Apply a schedule proposal',
  'calendar.delete_event': 'Delete a calendar event',
  'calendar.create_event': 'Add a calendar event',
  'calendar.update_event': 'Change a calendar event',
  'assessment.approve_final': 'Make a grade final',
  'assessment.begin': 'Begin an assessment',
  'export.request': 'Export data',
  'deletion.request': 'Erase a note or a file',
  'notes.create': 'Save a protected note',
  'notes.revise': 'Change a protected note',
  'evidence.attach_file': 'Attach a file to a protected note',
  'evidence.from_note': 'Use a protected note as evidence',
  'session.correct_time': 'Correct session time',
  'backup.create': 'Create a backup',
  'backup.verify': 'Verify a backup',
  'backup.restore_test': 'Test a restore',
  'skills.invoke': 'Run a skill',
  'approvals.decide': 'Decide an approval',
};

/** An operation in the operator's words; the wire name is the fallback and stays in a `title`. */
export function operationLabel(operation: string): string {
  return OPERATION_LABELS[operation] ?? operation;
}

const DOMAIN_LABELS: Readonly<Record<string, string>> = {
  LEARNING: 'Learning',
  BUSINESS: 'Business',
  PERSONAL: 'Personal',
  SECURITY_SENSITIVE: 'Security-sensitive',
  SYSTEM: 'System',
};

export function domainLabel(domain: string): string {
  return DOMAIN_LABELS[domain] ?? sentenceCase(domain);
}

const CLASSIFICATION_LABELS: Readonly<Record<string, string>> = {
  PUBLIC: 'Public',
  INTERNAL: 'Internal',
  PRIVATE: 'Private',
  SECRET: 'Secret',
};

export function classificationLabel(classification: string): string {
  return CLASSIFICATION_LABELS[classification] ?? sentenceCase(classification);
}

/**
 * What a backend warning means, by its code. The backend's own sentence is always shown as
 * well (a warning is a disclosure; the window never replaces it), so an unknown code loses
 * nothing: it simply has no plain-language line above it.
 */
const WARNING_MEANINGS: Readonly<Record<string, string>> = {
  'curriculum.non_canonical': 'Today’s plan comes from a curriculum version that has not been approved as canonical yet.',
  'session.uncorrected_uncertain_time': 'Some time in this session could not be vouched for. It is not credited until you correct it.',
  'export.attachments_missing': 'The export was written, but some attached files were missing or damaged. They are listed in manifest.json.',
  'backup.attachments_incomplete': 'The backup was taken, but some attached files were missing or damaged and were not copied with it.',
  'backup.attachments_not_copied': 'The database snapshot was taken, but the attached files could not be copied beside it.',
  'backup.attachments_not_checked': 'This backup holds attached files that could not be checked.',
  'deletion.attachment_bytes_not_purged': 'The record is erased, but its file was not removed from disk because no attachment folder is configured.',
  'idempotency.replay_rebuilt': 'You sent this before. The result shows what is stored now, not what was sent then.',
  'competency.observations_not_derived': 'The grade is final, but it did not update any competency.',
  'notes.evidence_refs_not_persisted': 'The note is saved, but its links to evidence were not kept.',
  'audit.recent.process_scoped': 'The list shows what happened since the application started; the whole chain was still checked.',
  'clock.zone_fallback': 'The time zone of this machine could not be read, so dates are shown in UTC.',
  'data_dir.permissions': 'A folder of the application is readable by other users of this machine.',
  'approvals.reason_not_persisted': 'The reason you gave is not stored.',
  'keyfile.fallback': 'The approval key is kept in a file because no keyring is available.',
};

/** The plain-language meaning of a warning code, or `null` when the window has none. */
export function warningMeaning(code: string): string | null {
  return WARNING_MEANINGS[code] ?? null;
}
