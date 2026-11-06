/**
 * Transport selection. `VITE_G9_TRANSPORT=mock|tauri` forces a transport; otherwise the
 * Tauri transport is used when the runtime is present and the deterministic mock otherwise
 * (browser dev server, vitest, storybook-style review).
 */
import { createClient, type IpcClient, type IpcTransport } from './client';
import { createMockTransport } from './transports/mock';
import { createTauriTransport, hasTauriRuntime } from './transports/tauri';

export type TransportChoice = 'mock' | 'tauri';

export function chooseTransport(forced: string | undefined, runtimePresent: boolean): TransportChoice {
  if (forced === 'mock' || forced === 'tauri') {
    return forced;
  }
  return runtimePresent ? 'tauri' : 'mock';
}

export function selectTransport(): IpcTransport {
  const choice = chooseTransport(import.meta.env.VITE_G9_TRANSPORT, hasTauriRuntime());
  return choice === 'tauri' ? createTauriTransport() : createMockTransport();
}

export function createDefaultClient(): IpcClient {
  return createClient(selectTransport());
}

export { createClient, isG9Error, isCommandOutcome } from './client';
export type { IpcClient, IpcTransport, InvokeOptions, InvokeResult } from './client';
export { createMockTransport, createMockClock, MOCK_DEFAULT_NOW } from './transports/mock';
export type { MockTransport, MockClock } from './transports/mock';
