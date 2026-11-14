/**
 * `CommandOutcome.warnings` is the backend's disclosure channel: unimplemented (PROPOSED)
 * behaviour behind a successful response, non-canonical provenance, and host facts the
 * Tauri bridge appends to `system.status`. Formats seen on the wire:
 *   `<code>: <message>`                (g9-app handlers)
 *   `WARNING <code>: <message>`        (keyfile / bridge host facts)
 */
import type { InvokeResult } from '../ipc/client';
import { isG9Error } from '../ipc/client';
import type { OperationName } from '../ipc/operations';

/** Backend code emitted by `curriculum.daily_plan` when the plan's version is not APPROVED_CANONICAL. */
export const NON_CANONICAL_WARNING = 'curriculum.non_canonical';

/**
 * Codes that state a standing fact, not an event: the plan comes from a version that is not
 * canonical yet, for as long as the operator has not promoted it. They are still shown (the
 * backend's sentence verbatim) but as a quiet note, not as an alarm, and are not announced.
 */
export const QUIET_WARNING_CODES: readonly string[] = [NON_CANONICAL_WARNING];

/** The warning code: the text before the first ':' with a leading `WARNING ` removed. */
export function warningCode(warning: string): string {
  const body = warning.startsWith('WARNING ') ? warning.slice('WARNING '.length) : warning;
  const colon = body.indexOf(':');
  return (colon === -1 ? body : body.slice(0, colon)).trim();
}

export function hasWarning(warnings: readonly string[], code: string): boolean {
  return warnings.some((w) => warningCode(w) === code);
}

/** Warnings of a successful invoke result; an error or no result has none. */
export function warningsOf<K extends OperationName>(result: InvokeResult<K> | null | undefined): string[] {
  if (result === null || result === undefined || isG9Error(result)) {
    return [];
  }
  return result.warnings ?? [];
}

/** Concatenate warning lists, keeping the first occurrence of each text. */
export function mergeWarnings(...lists: readonly (readonly string[])[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const list of lists) {
    for (const w of list) {
      if (!seen.has(w)) {
        seen.add(w);
        out.push(w);
      }
    }
  }
  return out;
}

/** True for a warning that states a standing fact (see `QUIET_WARNING_CODES`). */
export function isQuietWarning(warning: string): boolean {
  return QUIET_WARNING_CODES.includes(warningCode(warning));
}
