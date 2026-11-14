import { describe, expect, it } from 'vitest';
import { chooseTransport, mockOptionsFromSearch } from './index';
import { createTauriTransport, hasTauriRuntime } from './transports/tauri';
import { isG9Error } from './client';

describe('transport selection', () => {
  it('uses the mock without a Tauri runtime and honours an explicit override', () => {
    expect(chooseTransport(undefined, false)).toBe('mock');
    expect(chooseTransport(undefined, true)).toBe('tauri');
    expect(chooseTransport('mock', true)).toBe('mock');
    expect(chooseTransport('tauri', false)).toBe('tauri');
    expect(chooseTransport('garbage', false)).toBe('mock');
  });

  // Security audit 2026-11-13, INFO: a production bundle opened where the Tauri runtime is absent
  // (a browser, a file URL, a port someone else serves) ran on the mock and showed synthetic
  // data under the application's name. It must refuse instead: the Tauri transport, which
  // answers every call RESOURCE_UNAVAILABLE.
  it('does not fall back to the mock in a production bundle', () => {
    expect(chooseTransport(undefined, false, true)).toBe('tauri');
    expect(chooseTransport(undefined, true, true)).toBe('tauri');
    expect(chooseTransport('garbage', false, true)).toBe('tauri');
    // The mock is still available where it is meant to be: the dev server, the tests, and a
    // build that asks for it by name (`VITE_G9_TRANSPORT=mock npm run build`, a review build).
    expect(chooseTransport(undefined, false, false)).toBe('mock');
    expect(chooseTransport('mock', false, true)).toBe('mock');
  });

  it('starts the mock as a new install only when the page asks for it (a review aid of the browser build)', () => {
    expect(mockOptionsFromSearch('?mock=fresh')).toEqual({ fresh: true });
    expect(mockOptionsFromSearch('?x=1&mock=fresh')).toEqual({ fresh: true });
    for (const search of ['', '?mock=other', '?mock=', '?fresh=1', '?mock=FRESH']) {
      expect(mockOptionsFromSearch(search), search).toEqual({ fresh: false });
    }
  });

  it('detects the runtime marker and reports RESOURCE_UNAVAILABLE when it is missing', async () => {
    expect(hasTauriRuntime({})).toBe(false);
    expect(hasTauriRuntime({ __TAURI_INTERNALS__: {} })).toBe(true);
    expect(hasTauriRuntime(null)).toBe(false);
    const transport = createTauriTransport();
    const r = await transport.invoke(
      'system.status',
      {
        request_id: 'r',
        operation: 'system.status',
        idempotency_key: 'k',
        scope: { domain: 'SYSTEM', mode: 'STUDY' },
        payload_schema_version: { major: 1, minor: 0 },
        trace_id: 't',
        issued_at: 0,
      },
      {},
    );
    expect(isG9Error(r) && r.code === 'ipc.tauri_runtime_missing').toBe(true);
  });
});
