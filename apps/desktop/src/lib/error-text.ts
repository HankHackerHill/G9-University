/**
 * Operator-facing text for the closed 13-category error set. Exhaustive: adding a category
 * in `g9-domain` fails the typecheck here until the mapping is extended.
 */
import type { ErrorCategory, G9Error } from '../ipc/types';
import { ERROR_CATEGORIES } from '../ipc/types';

export interface ErrorPresentation {
  title: string;
  guidance: string;
  /** Operator may retry without changing anything. */
  retryable: boolean;
  /** Tone used by the error display. */
  severity: 'info' | 'warning' | 'error';
}

export const ERROR_PRESENTATION: Readonly<Record<ErrorCategory, ErrorPresentation>> = {
  INVALID_INPUT: {
    title: 'Input rejected',
    guidance: 'The request did not pass validation. Correct the highlighted fields and try again.',
    retryable: false,
    severity: 'warning',
  },
  UNAUTHORIZED: {
    title: 'Not permitted',
    guidance: 'Policy denies this operation for the current actor, mode or domain. Nothing was changed.',
    retryable: false,
    severity: 'error',
  },
  APPROVAL_REQUIRED: {
    title: 'Approval required',
    guidance: 'This operation needs an explicit operator approval bound to its exact parameters before it can run.',
    retryable: false,
    severity: 'info',
  },
  STALE_REVISION: {
    title: 'Record changed',
    guidance: 'The record was modified since it was loaded. Reload to see the current version, then retry.',
    retryable: false,
    severity: 'warning',
  },
  PRECONDITION_FAILED: {
    title: 'Not allowed in the current state',
    guidance: 'The operation is not legal from the current state or the feature is not implemented in this build.',
    retryable: false,
    severity: 'warning',
  },
  NOT_FOUND: {
    title: 'Not found',
    guidance: 'The referenced record does not exist or is outside the loaded data.',
    retryable: false,
    severity: 'warning',
  },
  CONFLICT: {
    title: 'Conflict',
    guidance: 'A different request already used this idempotency key, or two changes collided. Review and resubmit.',
    retryable: false,
    severity: 'warning',
  },
  RESOURCE_UNAVAILABLE: {
    title: 'Resource unavailable',
    guidance: 'A required resource (model runtime, storage, transport) is not available. Core planning keeps working.',
    retryable: true,
    severity: 'warning',
  },
  TIMEOUT: {
    title: 'Timed out',
    guidance: 'The operation exceeded its time budget and was stopped. You may retry.',
    retryable: true,
    severity: 'warning',
  },
  CANCELLED: {
    title: 'Cancelled',
    guidance: 'The operation was cancelled before completion. No partial state was kept.',
    retryable: false,
    severity: 'info',
  },
  RETRYABLE_STORAGE: {
    title: 'Storage busy',
    guidance: 'The local database was busy or locked. Retry in a moment.',
    retryable: true,
    severity: 'warning',
  },
  DATA_CORRUPTION: {
    title: 'Integrity check failed',
    guidance: 'Stored data failed an integrity check. Stop and use Settings/Recovery to verify a backup before continuing.',
    retryable: false,
    severity: 'error',
  },
  INTERNAL: {
    title: 'Internal error',
    guidance: 'An internal error occurred. The cause is in the local operation log; nothing sensitive is shown here.',
    retryable: false,
    severity: 'error',
  },
};

export function presentError(error: G9Error): ErrorPresentation {
  return ERROR_PRESENTATION[error.category];
}

/** Sanity check used by tests and the error display: every category has text. */
export function allCategoriesPresented(): boolean {
  return ERROR_CATEGORIES.every((c) => ERROR_PRESENTATION[c].title.length > 0 && ERROR_PRESENTATION[c].guidance.length > 0);
}
