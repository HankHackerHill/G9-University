/**
 * Transport selection. `VITE_G9_TRANSPORT=mock|tauri` forces a transport; otherwise the
 * Tauri transport is used when the runtime is present. Without the runtime a development
 * session (`npm run dev`, vitest) uses the deterministic mock, and a production bundle does
 * not: it keeps the Tauri transport, which answers every call RESOURCE_UNAVAILABLE, so a bundle
 * opened outside the desktop shell shows an error instead of synthetic data under the
 * application's name. A review build that wants the mock asks for it by name:
 * `VITE_G9_TRANSPORT=mock npm run build`.
 */
import { createClient, type IpcClient, type IpcTransport } from './client';
import { createMockTransport } from './transports/mock';
import { createTauriTransport, hasTauriRuntime } from './transports/tauri';

export type TransportChoice = 'mock' | 'tauri';

export function chooseTransport(forced: string | undefined, runtimePresent: boolean, production = false): TransportChoice {
  if (forced === 'mock' || forced === 'tauri') {
    return forced;
  }
  return runtimePresent || production ? 'tauri' : 'mock';
}

/**
 * A review aid of the browser build, never reached in the desktop shell: `?mock=fresh` starts the
 * mock as a new install (nothing imported, no program start, nothing planned) so the first-run
 * screens can be looked at and measured.
 */
export function mockOptionsFromSearch(search: string): { fresh: boolean } {
  return { fresh: new URLSearchParams(search).get('mock') === 'fresh' };
}

export function selectTransport(): IpcTransport {
  const choice = chooseTransport(import.meta.env.VITE_G9_TRANSPORT, hasTauriRuntime(), import.meta.env.PROD);
  return choice === 'tauri' ? createTauriTransport() : createMockTransport(mockOptionsFromSearch(typeof window === 'undefined' ? '' : window.location.search));
}

export function createDefaultClient(): IpcClient {
  return createClient(selectTransport());
}

export { createClient, isG9Error, isCommandOutcome } from './client';
export type { IpcClient, IpcTransport, InvokeOptions, InvokeResult } from './client';
export { createMockTransport, createMockClock, MOCK_DEFAULT_NOW } from './transports/mock';
export type { MockTransport, MockClock } from './transports/mock';
