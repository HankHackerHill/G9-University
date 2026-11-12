/**
 * Read-model hook over the IPC client. `payload === null` skips the call (dependent queries).
 * Loading is derived (the stored result's key differs from the current key), so no state is
 * written synchronously inside effects. A `reload()` keeps the previous result visible while
 * the new one is in flight (stale-while-revalidate) so screens do not flicker to empty.
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { InvokeOptions, InvokeResult } from '../../ipc/client';
import { isG9Error } from '../../ipc/client';
import type { OperationName, RequestOf, ResponseOf } from '../../ipc/operations';
import type { CommandOutcome, G9Error } from '../../ipc/types';
import { canonicalJson } from '../../lib/canonical-json';
import { useIpc } from '../ipc-context';

export type CommandState<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ok'; outcome: CommandOutcome<T> }
  | { status: 'error'; error: G9Error };

export interface UseCommandResult<T> {
  state: CommandState<T>;
  data: T | null;
  error: G9Error | null;
  loading: boolean;
  /**
   * `CommandOutcome.warnings` of the result in `data` (empty when there is none). The backend
   * uses warnings to disclose partial or PROPOSED behaviour; screens must render them.
   */
  warnings: readonly string[];
  reload: () => void;
}

const NO_WARNINGS: readonly string[] = [];

export function useCommand<K extends OperationName>(
  op: K,
  payload: RequestOf<K> | null,
  options?: InvokeOptions,
  /**
   * Any change of this value re-reads the query while the last result stays visible (the same
   * stale-while-revalidate path as `reload()`). The shell passes the mutation epoch so the day
   * rail and the lamps follow commands issued from any screen.
   */
  revalidate: number | string = '',
): UseCommandResult<ResponseOf<K>> {
  const client = useIpc();
  const [tick, setTick] = useState(0);
  const payloadKey = payload === null ? null : canonicalJson(payload);
  const optionsKey = options === undefined ? '' : canonicalJson(options);
  const baseKey = payloadKey === null ? null : `${op}|${payloadKey}|${optionsKey}`;
  const key = baseKey === null ? null : `${baseKey}|${tick}|${revalidate}`;
  const [stored, setStored] = useState<{ key: string; baseKey: string; result: InvokeResult<K> } | null>(null);

  useEffect(() => {
    if (key === null || payloadKey === null) {
      return;
    }
    let active = true;
    const parsed = JSON.parse(payloadKey) as RequestOf<K>;
    const parsedOptions = optionsKey === '' ? undefined : (JSON.parse(optionsKey) as InvokeOptions);
    void client.invoke(op, parsed, parsedOptions).then((result) => {
      if (active && baseKey !== null) {
        setStored({ key, baseKey, result });
      }
    });
    return () => {
      active = false;
    };
  }, [client, op, key, baseKey, payloadKey, optionsKey]);

  const reload = useCallback(() => {
    setTick((t) => t + 1);
  }, []);

  return useMemo(() => {
    if (key === null) {
      return { state: { status: 'idle' }, data: null, error: null, loading: false, warnings: NO_WARNINGS, reload };
    }
    if (stored?.key !== key) {
      // Revalidating the same query: keep the last good result visible.
      if (stored !== null && stored.baseKey === baseKey && !isG9Error(stored.result)) {
        return {
          state: { status: 'loading' },
          data: stored.result.result,
          error: null,
          loading: true,
          warnings: stored.result.warnings ?? NO_WARNINGS,
          reload,
        };
      }
      return { state: { status: 'loading' }, data: null, error: null, loading: true, warnings: NO_WARNINGS, reload };
    }
    if (isG9Error(stored.result)) {
      return { state: { status: 'error', error: stored.result }, data: null, error: stored.result, loading: false, warnings: NO_WARNINGS, reload };
    }
    return {
      state: { status: 'ok', outcome: stored.result },
      data: stored.result.result,
      error: null,
      loading: false,
      warnings: stored.result.warnings ?? NO_WARNINGS,
      reload,
    };
  }, [key, baseKey, stored, reload]);
}
