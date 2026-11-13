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

/**
 * Each entry says what happened and what to do about it, in the operator's words
 * (docs/ui/ux-copy.md §5). The backend's own message and code are shown under it by
 * `ErrorDisplay`, so nothing the backend said is replaced.
 */
export const ERROR_PRESENTATION: Readonly<Record<ErrorCategory, ErrorPresentation>> = {
  INVALID_INPUT: {
    title: 'Check what you entered',
    guidance: 'The backend refused a value, so nothing was saved. Correct it and send it again.',
    retryable: false,
    severity: 'warning',
  },
  UNAUTHORIZED: {
    title: 'Not permitted',
    guidance: 'Policy does not allow this action here. Nothing was changed.',
    retryable: false,
    severity: 'error',
  },
  APPROVAL_REQUIRED: {
    title: 'Needs your approval',
    guidance: 'This action changes a protected record, so it waits for you. Approve the request, then apply it. The approval covers exactly these values and nothing else.',
    retryable: false,
    severity: 'info',
  },
  STALE_REVISION: {
    title: 'This record changed',
    guidance: 'It was changed after this window loaded it, so your change was not applied. Reload to see the current version, then make your change again.',
    retryable: false,
    severity: 'warning',
  },
  PRECONDITION_FAILED: {
    title: 'Not possible right now',
    guidance: 'The record is not in a state that allows this, or this part is not built yet. Nothing was changed.',
    retryable: false,
    severity: 'warning',
  },
  NOT_FOUND: {
    title: 'Not found',
    guidance: 'That record does not exist or is not in the loaded data. Check the name or the code.',
    retryable: false,
    severity: 'warning',
  },
  CONFLICT: {
    title: 'Two changes collided',
    guidance: 'Another request used the same key, or the record was changed at the same moment. Look at the current record, then send your change again.',
    retryable: false,
    severity: 'warning',
  },
  RESOURCE_UNAVAILABLE: {
    title: 'Not available',
    guidance: 'Something this needs is not available: storage, the model, or the link to the backend. Planning, notes and timers do not need a model.',
    retryable: true,
    severity: 'warning',
  },
  TIMEOUT: {
    title: 'Took too long',
    guidance: 'The action was stopped at its time limit and nothing partial was kept.',
    retryable: true,
    severity: 'warning',
  },
  CANCELLED: {
    title: 'Cancelled',
    guidance: 'The action was cancelled before it finished. Nothing partial was kept.',
    retryable: false,
    severity: 'info',
  },
  RETRYABLE_STORAGE: {
    title: 'Database busy',
    guidance: 'The local database was busy for a moment. Nothing was lost.',
    retryable: true,
    severity: 'warning',
  },
  DATA_CORRUPTION: {
    title: 'Integrity check failed',
    guidance: 'Stored data did not pass an integrity check. Stop here and verify a backup in Settings/Recovery before you continue.',
    retryable: false,
    severity: 'error',
  },
  INTERNAL: {
    title: 'Something failed inside the application',
    guidance: 'The cause is written to the local log. Nothing sensitive is shown here.',
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
