/**
 * Transition tables mirrored from `crates/g9-domain/src/state.rs`. The backend is the
 * authority; the UI uses these tables only to enable/disable controls before a call and
 * the mock transport uses them to refuse illegal moves exactly like the Rust machines.
 */
import type {
  ApprovalEvent,
  ApprovalState,
  AssessmentAttemptState,
  AssessmentEvent,
  BackupEvent,
  BackupState,
  SessionEvent,
  SessionState,
} from '../ipc/types';

export interface TransitionTable<S extends string, E extends string> {
  readonly machine: string;
  readonly transitions: ReadonlyArray<readonly [S, E, S]>;
  readonly terminal: ReadonlyArray<S>;
}

export interface TransitionRefusal {
  machine: string;
  from: string;
  event: string;
}

export function transition<S extends string, E extends string>(
  table: TransitionTable<S, E>,
  from: S,
  event: E,
): { ok: true; to: S } | { ok: false; refusal: TransitionRefusal } {
  for (const [f, e, to] of table.transitions) {
    if (f === from && e === event) {
      return { ok: true, to };
    }
  }
  return { ok: false, refusal: { machine: table.machine, from, event } };
}

export function can<S extends string, E extends string>(table: TransitionTable<S, E>, from: S, event: E): boolean {
  return transition(table, from, event).ok;
}

export function legalEvents<S extends string, E extends string>(table: TransitionTable<S, E>, from: S): E[] {
  const out: E[] = [];
  for (const [f, e] of table.transitions) {
    if (f === from && !out.includes(e)) {
      out.push(e);
    }
  }
  return out;
}

export function isTerminal<S extends string, E extends string>(table: TransitionTable<S, E>, state: S): boolean {
  return table.terminal.includes(state);
}

export const SESSION_MACHINE: TransitionTable<SessionState, SessionEvent> = {
  machine: 'session',
  terminal: ['CLOSED', 'CANCELLED'],
  transitions: [
    ['PLANNED', 'PREPARE', 'READY'],
    ['PLANNED', 'CANCEL', 'CANCELLED'],
    ['READY', 'START', 'RUNNING'],
    ['READY', 'CANCEL', 'CANCELLED'],
    ['RUNNING', 'PAUSE', 'PAUSED'],
    ['RUNNING', 'COMPLETE', 'COMPLETED'],
    ['RUNNING', 'INTERRUPT', 'INTERRUPTED'],
    ['PAUSED', 'RESUME', 'RUNNING'],
    ['PAUSED', 'COMPLETE', 'COMPLETED'],
    ['PAUSED', 'INTERRUPT', 'INTERRUPTED'],
    ['INTERRUPTED', 'RECOVER', 'PAUSED'],
    ['INTERRUPTED', 'COMPLETE', 'COMPLETED'],
    ['INTERRUPTED', 'CANCEL', 'CANCELLED'],
    ['COMPLETED', 'REVIEW', 'REVIEWED'],
    ['REVIEWED', 'CLOSE', 'CLOSED'],
  ],
};

export const ASSESSMENT_MACHINE: TransitionTable<AssessmentAttemptState, AssessmentEvent> = {
  machine: 'assessment_attempt',
  terminal: ['APPROVED_FINAL', 'VOIDED'],
  transitions: [
    ['AVAILABLE', 'BEGIN', 'STARTED'],
    ['STARTED', 'SUBMIT', 'SUBMITTED'],
    ['STARTED', 'ABANDON', 'VOIDED'],
    ['SUBMITTED', 'REVIEW_EVIDENCE', 'EVIDENCE_REVIEW'],
    ['SUBMITTED', 'VOID', 'VOIDED'],
    ['EVIDENCE_REVIEW', 'GRADE_PROVISIONAL', 'GRADED_PROVISIONAL'],
    ['EVIDENCE_REVIEW', 'VOID', 'VOIDED'],
    ['GRADED_PROVISIONAL', 'REVIEW_EVIDENCE', 'EVIDENCE_REVIEW'],
    ['GRADED_PROVISIONAL', 'APPROVE_FINAL', 'APPROVED_FINAL'],
    ['GRADED_PROVISIONAL', 'VOID', 'VOIDED'],
  ],
};

export const BACKUP_MACHINE: TransitionTable<BackupState, BackupEvent> = {
  machine: 'backup',
  terminal: ['EXPIRED', 'FAILED'],
  transitions: [
    ['REQUESTED', 'SNAPSHOT', 'SNAPSHOT_CREATED'],
    ['REQUESTED', 'FAIL', 'FAILED'],
    ['SNAPSHOT_CREATED', 'CHECK', 'CHECKED'],
    ['SNAPSHOT_CREATED', 'FAIL', 'FAILED'],
    ['CHECKED', 'TEST_RESTORE', 'RESTORE_TESTED'],
    ['CHECKED', 'FAIL', 'FAILED'],
    ['RESTORE_TESTED', 'RETAIN', 'RETAINED'],
    ['RESTORE_TESTED', 'FAIL', 'FAILED'],
    ['RETAINED', 'EXPIRE', 'EXPIRED'],
  ],
};

export const APPROVAL_MACHINE: TransitionTable<ApprovalState, ApprovalEvent> = {
  machine: 'approval',
  terminal: ['REJECTED', 'EXPIRED', 'CONSUMED', 'REVOKED'],
  transitions: [
    ['PENDING', 'APPROVE', 'APPROVED'],
    ['PENDING', 'REJECT', 'REJECTED'],
    ['PENDING', 'EXPIRE', 'EXPIRED'],
    ['APPROVED', 'CONSUME', 'CONSUMED'],
    ['APPROVED', 'EXPIRE', 'EXPIRED'],
    ['APPROVED', 'REVOKE', 'REVOKED'],
  ],
};

/** Maps a session operation name to the machine event it applies. */
export const SESSION_OPERATION_EVENTS = {
  'session.prepare': 'PREPARE',
  'session.start': 'START',
  'session.pause': 'PAUSE',
  'session.resume': 'RESUME',
  'session.complete': 'COMPLETE',
  'session.correct_time': 'RECOVER',
} as const satisfies Record<string, SessionEvent>;
