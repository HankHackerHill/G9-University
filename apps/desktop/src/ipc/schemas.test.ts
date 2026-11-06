// @vitest-environment node
/**
 * Schema house rules and contract parity:
 *  - every file in schemas/ipc is draft 2020-12, has $id/title, is an object with
 *    additionalProperties false and explicit required, and contains no $ref anywhere;
 *  - every operation in the TypeScript registry has a request and a response schema and vice versa;
 *  - the mock transport's responses validate against the response schemas (>= 8 operations);
 *  - the client's wire metadata validates against command-meta; errors against g9-error.
 */
import Ajv2020 from 'ajv/dist/2020.js';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { buildMeta, createClient, isG9Error } from './client';
import { OPERATION_NAMES, type OperationName } from './operations';
import { createMockClock, createMockTransport } from './transports/mock';

const SCHEMA_DIR = fileURLToPath(new URL('../../../../schemas/ipc/', import.meta.url));

interface SchemaDoc {
  $schema?: string;
  $id?: string;
  $async?: false;
  title?: unknown;
  type?: unknown;
  additionalProperties?: unknown;
  required?: unknown;
  [key: string]: unknown;
}

function loadAll(): Map<string, SchemaDoc> {
  const out = new Map<string, SchemaDoc>();
  for (const file of readdirSync(SCHEMA_DIR).filter((f) => f.endsWith('.schema.json')).sort()) {
    out.set(file, JSON.parse(readFileSync(`${SCHEMA_DIR}${file}`, 'utf8')) as SchemaDoc);
  }
  return out;
}

function containsRef(value: unknown): boolean {
  if (Array.isArray(value)) {
    return value.some(containsRef);
  }
  if (typeof value === 'object' && value !== null) {
    return Object.entries(value).some(([k, v]) => k === '$ref' || containsRef(v));
  }
  return false;
}

/** Every nested object schema must also be closed and explicit. */
function everyObjectIsClosed(value: unknown): boolean {
  if (Array.isArray(value)) {
    return value.every(everyObjectIsClosed);
  }
  if (typeof value === 'object' && value !== null) {
    const rec = value as Record<string, unknown>;
    if (rec.type === 'object' && rec.properties !== undefined) {
      if (rec.additionalProperties !== false || !Array.isArray(rec.required)) {
        return false;
      }
    }
    return Object.values(rec).every(everyObjectIsClosed);
  }
  return true;
}

const schemas = loadAll();
// strict mode on; union `type` arrays (nullable fields, serde_json::Value) are valid 2020-12 and explicitly allowed.
const ajv = new Ajv2020({ strict: true, allowUnionTypes: true, allErrors: true });

function validator(file: string) {
  const doc = schemas.get(file);
  if (doc === undefined) {
    throw new Error(`missing schema ${file}`);
  }
  return ajv.compile(doc);
}

describe('schemas/ipc house rules', () => {
  it('has the three envelope schemas and a request+response pair for every registered operation (and nothing else)', () => {
    const files = [...schemas.keys()];
    expect(files).toContain('command-meta.schema.json');
    expect(files).toContain('g9-error.schema.json');
    expect(files).toContain('command-outcome.schema.json');
    const expected = new Set<string>(['command-meta.schema.json', 'g9-error.schema.json', 'command-outcome.schema.json']);
    for (const op of OPERATION_NAMES) {
      expected.add(`${op}.request.schema.json`);
      expected.add(`${op}.response.schema.json`);
    }
    expect(new Set(files)).toEqual(expected);
    expect(OPERATION_NAMES.length).toBeGreaterThanOrEqual(31);
  });

  it('every file is draft 2020-12, has $id/title, is a closed object with explicit required and no $ref', () => {
    expect(schemas.size).toBeGreaterThan(0);
    for (const [file, doc] of schemas) {
      expect(doc.$schema, file).toBe('https://json-schema.org/draft/2020-12/schema');
      expect(doc.$id, file).toBe(`https://g9.local/schemas/ipc/${file.replace(/\.schema\.json$/, '')}`);
      expect(typeof doc.title, file).toBe('string');
      expect(doc.type, file).toBe('object');
      expect(doc.additionalProperties, file).toBe(false);
      expect(Array.isArray(doc.required), file).toBe(true);
      expect(containsRef(doc), file).toBe(false);
      expect(everyObjectIsClosed(doc), file).toBe(true);
      expect(() => ajv.compile(doc), file).not.toThrow();
    }
  });

  it('command-meta forbids an actor field (backend-assigned identity) and accepts the client-built meta', () => {
    const validate = validator('command-meta.schema.json');
    const meta = buildMeta('session.start', { expectedRevision: 3 }, { now: () => 1, uuid: () => 'u' });
    expect(validate(meta), JSON.stringify(validate.errors)).toBe(true);
    expect(validate({ ...meta, actor: { id: 'x', kind: { kind: 'OPERATOR' } } })).toBe(false);
  });

  it('g9-error accepts exactly the 13 categories', () => {
    const validate = validator('g9-error.schema.json');
    expect(validate({ category: 'TIMEOUT', code: 'a.b', message: 'm' })).toBe(true);
    expect(validate({ category: 'APPROVAL_REQUIRED', code: 'a.b', message: 'm', approval_id: 'ap-1', details: { x: 1 } })).toBe(true);
    expect(validate({ category: 'FATAL', code: 'a.b', message: 'm' })).toBe(false);
    expect(validate({ category: 'INTERNAL', code: 'a', message: 'm', backtrace: '...' })).toBe(false);
  });
});

describe('mock responses validate against the response schemas', () => {
  const clock = createMockClock();
  const transport = createMockTransport({ clock });
  let n = 0;
  const client = createClient(transport, {
    now: () => clock.now(),
    uuid: () => {
      n += 1;
      return `s${n}`;
    },
  });
  const outcomeValidator = validator('command-outcome.schema.json');

  async function check<K extends OperationName>(op: K, payload: Parameters<typeof client.invoke<K>>[1]) {
    const r = await client.invoke(op, payload);
    if (isG9Error(r)) {
      throw new Error(`${op} failed: ${r.category} ${r.code}`);
    }
    expect(outcomeValidator(r), `${op} envelope ${JSON.stringify(outcomeValidator.errors)}`).toBe(true);
    const validate = validator(`${op}.response.schema.json`);
    expect(validate(r.result), `${op} ${JSON.stringify(validate.errors)}`).toBe(true);
    return r.result;
  }

  it('system.status', async () => {
    await check('system.status', {});
  });
  it('curriculum.daily_plan', async () => {
    await check('curriculum.daily_plan', { date: '2026-11-18' });
  });
  it('calendar.list', async () => {
    const from = Date.UTC(2026, 10, 16);
    await check('calendar.list', { from, to: from + 7 * 86_400_000 });
  });
  it('session.list and session.prepare/start', async () => {
    const list = await check('session.list', { date: '2026-11-18' });
    const planned = list.sessions.find((s) => s.state === 'PLANNED');
    if (planned === undefined) {
      throw new Error('no planned session');
    }
    await check('session.prepare', { session_id: planned.session_id });
    await check('session.start', { session_id: planned.session_id });
  });
  it('retention.due', async () => {
    await check('retention.due', {});
  });
  it('competency.projection', async () => {
    await check('competency.projection', {});
  });
  it('interventions.list', async () => {
    await check('interventions.list', {});
  });
  it('assessment.list', async () => {
    await check('assessment.list', {});
  });
  it('approvals.list after an APPROVAL_REQUIRED', async () => {
    const r = await client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
    expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
    const errValidate = validator('g9-error.schema.json');
    expect(errValidate(r), JSON.stringify(errValidate.errors)).toBe(true);
    const approvals = await check('approvals.list', {});
    expect(approvals.approvals.length).toBeGreaterThan(0);
  });
  it('audit.recent', async () => {
    await check('audit.recent', { limit: 20 });
  });
  it('backup.create / verify', async () => {
    const b = await check('backup.create', { label: 'schema-test' });
    await check('backup.verify', { backup_id: b.backup_id });
  });
  it('notes.create', async () => {
    await check('notes.create', { title: 't', body_markdown: '# hi', classification: 'PRIVATE', domain: 'LEARNING' });
  });
  it('calendar.propose', async () => {
    const from = Date.UTC(2026, 10, 16);
    await check('calendar.propose', { from, to: from + 7 * 86_400_000, intent: 'ADD_REVIEWS' });
  });
  it('curriculum.validate', async () => {
    await check('curriculum.validate', { manifest: { curriculum_id: 'x' } });
  });
});
