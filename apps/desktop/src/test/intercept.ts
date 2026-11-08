/**
 * Test-only transport that records every call and lets a test replace selected responses
 * while everything else goes to the deterministic mock. Used to reproduce backend shapes the
 * mock cannot reach (failed reads, warnings, sessions interrupted twice).
 */
import { createClient } from '../ipc/client';
import type { OperationName } from '../ipc/operations';
import type { ClientCommandMeta, G9Error } from '../ipc/types';
import { createMockTransport, type MockTransport } from '../ipc/transports/mock';
import type { Harness } from './render';

export interface RecordedCall {
  op: OperationName;
  meta: ClientCommandMeta;
  payload: unknown;
}

/** Return the replacement response, or call `next()` to get the mock's answer. */
export type Interceptor = (op: OperationName, payload: unknown, next: () => Promise<unknown>) => Promise<unknown>;

export interface InterceptHarness extends Harness {
  calls: RecordedCall[];
}

export function createInterceptHarness(intercept: Interceptor): InterceptHarness {
  const mock = createMockTransport();
  const calls: RecordedCall[] = [];
  const transport: MockTransport = {
    kind: 'mock',
    clock: mock.clock,
    zone: mock.zone,
    invoke: (op, meta, payload) => {
      calls.push({ op, meta, payload });
      return intercept(op, payload, () => mock.invoke(op, meta, payload));
    },
    onProcessRestart: () => mock.onProcessRestart(),
  };
  let n = 0;
  const client = createClient(transport, {
    now: () => mock.clock.now(),
    uuid: () => {
      n += 1;
      return `intercept-uuid-${n}`;
    },
  });
  return { transport, client, calls };
}

/** Appends warnings to a successful `CommandOutcome`; errors pass through unchanged. */
export function addWarnings(response: unknown, warnings: readonly string[]): unknown {
  if (typeof response !== 'object' || response === null || !('result' in response)) {
    return response;
  }
  const outcome = response as { warnings?: string[] };
  return { ...response, warnings: [...(outcome.warnings ?? []), ...warnings] };
}

export function outcome(result: unknown): unknown {
  return { result, replayed: false };
}

export const STORAGE_BUSY: G9Error = { category: 'RETRYABLE_STORAGE', code: 'storage.busy', message: 'database is busy' };
