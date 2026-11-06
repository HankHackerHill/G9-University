/**
 * Tauri transport. The only place `@tauri-apps/api` is referenced, and only through a
 * dynamic import so that the bundle, tests and the browser dev server never need the Tauri
 * runtime. The bridge exposes exactly one command, `g9_command`, which forwards
 * `{ op, meta, payload }` to `g9-app` (ARCHITECTURE §2 rule 6).
 */
import type { IpcTransport } from '../client';
import type { OperationName } from '../operations';
import type { ClientCommandMeta } from '../types';

interface TauriInternalsWindow {
  __TAURI_INTERNALS__?: unknown;
}

/** True when running inside a Tauri webview (`window.__TAURI_INTERNALS__` is injected by the runtime). */
export function hasTauriRuntime(target: unknown = globalThis): boolean {
  if (typeof target !== 'object' || target === null) {
    return false;
  }
  return (target as TauriInternalsWindow).__TAURI_INTERNALS__ !== undefined;
}

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

let invokeCache: Promise<InvokeFn> | null = null;

async function loadInvoke(): Promise<InvokeFn> {
  invokeCache ??= import('@tauri-apps/api/core').then((mod) => mod.invoke as InvokeFn);
  return invokeCache;
}

export function createTauriTransport(): IpcTransport {
  return {
    kind: 'tauri',
    async invoke(op: OperationName, meta: ClientCommandMeta, payload: unknown): Promise<unknown> {
      if (!hasTauriRuntime()) {
        // Returned (not thrown) as a G9Error so the client narrows it like any backend error.
        return {
          category: 'RESOURCE_UNAVAILABLE',
          code: 'ipc.tauri_runtime_missing',
          message: 'the Tauri runtime is not present in this window',
        };
      }
      const invoke = await loadInvoke();
      // Tauri rejects the promise with the command's `Err` value: a G9Error-shaped JSON object.
      return invoke('g9_command', { op, meta, payload }).catch((err: unknown) => err);
    },
  };
}
