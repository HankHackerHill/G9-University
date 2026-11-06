import { describe, expect, it } from 'vitest';
import { buildMeta, createClient, isCommandOutcome, isG9Error, type IpcTransport } from './client';
import type { ClientCommandMeta } from './types';

const deps = { now: () => 1_700_000_000_000, uuid: () => 'uuid-fixed' };

function transportReturning(value: unknown, capture?: { meta?: ClientCommandMeta; payload?: unknown }): IpcTransport {
  return {
    kind: 'mock',
    invoke: (_op, meta, payload) => {
      if (capture !== undefined) {
        capture.meta = meta;
        capture.payload = payload;
      }
      return Promise.resolve(value);
    },
  };
}

describe('ipc client', () => {
  it('builds wire metadata without an actor and with operation defaults', () => {
    const meta = buildMeta('session.start', {}, deps);
    expect(meta).toEqual({
      request_id: 'uuid-fixed',
      operation: 'session.start',
      idempotency_key: 'uuid-fixed',
      scope: { domain: 'LEARNING', mode: 'STUDY' },
      payload_schema_version: { major: 1, minor: 0 },
      trace_id: 'uuid-fixed',
      issued_at: 1_700_000_000_000,
    });
    expect(Object.hasOwn(meta, 'actor')).toBe(false);
  });

  it('honours per-call overrides (approval, revision, scope) and omits absent optionals', () => {
    const meta = buildMeta('assessment.approve_final', { approvalId: 'ap-1', expectedRevision: 4, domain: 'SYSTEM', mode: 'MAINTENANCE' }, deps);
    expect(meta.approval_id).toBe('ap-1');
    expect(meta.expected_revision).toBe(4);
    expect(meta.scope).toEqual({ domain: 'SYSTEM', mode: 'MAINTENANCE' });
    expect('resource' in meta).toBe(false);
  });

  it('narrows a backend error with the closed category set', async () => {
    const client = createClient(transportReturning({ category: 'NOT_FOUND', code: 'x.missing', message: 'nope' }), deps);
    const r = await client.invoke('session.start', { session_id: 's' });
    expect(isG9Error(r)).toBe(true);
    if (isG9Error(r)) {
      expect(r.category).toBe('NOT_FOUND');
    }
  });

  it('rejects an unknown error category or a malformed envelope as INTERNAL ipc.malformed_response', async () => {
    const bogus = createClient(transportReturning({ category: 'WEIRD', code: 'x', message: 'y' }), deps);
    const r1 = await bogus.invoke('system.status', {});
    expect(isG9Error(r1) && r1.category === 'INTERNAL' && r1.code === 'ipc.malformed_response').toBe(true);
    const text = createClient(transportReturning('not-an-envelope'), deps);
    const r2 = await text.invoke('system.status', {});
    expect(isG9Error(r2) && r2.code === 'ipc.malformed_response').toBe(true);
    expect(isCommandOutcome({ result: 1 })).toBe(false);
    expect(isCommandOutcome({ result: 1, replayed: false })).toBe(true);
  });

  it('passes the payload through untouched and returns the success envelope', async () => {
    const capture: { meta?: ClientCommandMeta; payload?: unknown } = {};
    const client = createClient(transportReturning({ result: { ok: 1 }, replayed: false, revision: 2 }, capture), deps);
    const r = await client.invoke('notes.revise', { note_id: 'n1', body_markdown: '# x' }, { expectedRevision: 1 });
    expect(capture.payload).toEqual({ note_id: 'n1', body_markdown: '# x' });
    expect(capture.meta?.expected_revision).toBe(1);
    expect(isG9Error(r)).toBe(false);
    if (!isG9Error(r)) {
      expect(r.revision).toBe(2);
    }
  });

  it('maps a throwing transport to RESOURCE_UNAVAILABLE without leaking the cause', async () => {
    const client = createClient(
      { kind: 'tauri', invoke: () => Promise.reject(new Error('secret path /home/x')) },
      deps,
    );
    const r = await client.invoke('system.status', {});
    expect(isG9Error(r)).toBe(true);
    if (isG9Error(r)) {
      expect(r.category).toBe('RESOURCE_UNAVAILABLE');
      expect(JSON.stringify(r)).not.toContain('/home/x');
    }
  });
});
