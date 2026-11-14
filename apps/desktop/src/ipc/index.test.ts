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
