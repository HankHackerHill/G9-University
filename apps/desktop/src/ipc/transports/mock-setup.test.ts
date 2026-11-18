// @vitest-environment node
/**
 * Program setup, notes read-back, the competency packs and the backup list in the mock
 * transport (the same contracts `crates/g9-app/tests/program_setup.rs`, `notes_read_back.rs` and
 * `packs_and_backups.rs` hold the backend to).
 */
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { createClient, isG9Error, type InvokeResult } from '../client';
import type { OperationName } from '../operations';
import type { JsonValue } from '../types';
import { asJson, v2Manifest } from '../../test/v2-manifest';
import { createMockClock, createMockTransport, type MockOptions } from './mock';

const SEED = fileURLToPath(new URL('../../../../../data/curriculum/g9-ct9-480.seed.v0.json', import.meta.url));
const PACK_DIR = fileURLToPath(new URL('../../../../../data/competency-packs/', import.meta.url));

function harness(options: Omit<MockOptions, 'clock'> = {}) {
  const clock = createMockClock();
  const transport = createMockTransport({ ...options, clock });
  let n = 0;
  const client = createClient(transport, {
    now: () => clock.now(),
    uuid: () => {
      n += 1;
      return `u${n}`;
    },
  });
  return { clock, client };
}

function ok<K extends OperationName>(r: InvokeResult<K>) {
  if (isG9Error(r)) {
    throw new Error(`expected success, got ${r.category} ${r.code}`);
  }
  return r;
}

function err<K extends OperationName>(r: InvokeResult<K>) {
  if (!isG9Error(r)) {
    throw new Error('expected a G9Error');
  }
  return r;
}

type Client = ReturnType<typeof harness>['client'];

async function importSeed(client: Client): Promise<void> {
  const manifest = JSON.parse(readFileSync(SEED, 'utf8')) as JsonValue;
  const asked = err(await client.invoke('curriculum.import', { manifest, provenance: 'PROPOSED_SEED' }));
  expect(asked.category).toBe('APPROVAL_REQUIRED');
  const approvalId = asked.approval_id ?? '';
  ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
  ok(await client.invoke('curriculum.import', { manifest, provenance: 'PROPOSED_SEED' }, { approvalId }));
}

describe('mock transport: program setup on a new install', () => {
  it('starts with nothing imported, no start, no sessions and no competencies', async () => {
    const { client } = harness({ fresh: true });
    expect(ok(await client.invoke('curriculum.program', {})).result).toEqual({
      curriculum: null,
      program_start: null,
      competencies: { loaded: 0, available: 64 },
    });
    expect(ok(await client.invoke('session.list', {})).result.sessions).toEqual([]);
    expect(ok(await client.invoke('calendar.list', { from: 0, to: 4_000_000_000_000 })).result.events).toEqual([]);
    expect(ok(await client.invoke('competency.projection', {})).result.projections).toEqual([]);
    expect(ok(await client.invoke('notes.list', {})).result.notes).toEqual([]);
  });

  it('answers the version first, then the start, like the backend', async () => {
    const { client } = harness({ fresh: true });
    expect(err(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).code).toBe('curriculum.not_imported');
    expect(err(await client.invoke('session.plan_day', { date: '2026-11-18' })).code).toBe('curriculum.not_imported');
    expect(err(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' })).code).toBe('curriculum.not_imported');
    await importSeed(client);
    expect(err(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).code).toBe('curriculum.program_start_not_configured');
    expect(err(await client.invoke('session.plan_day', { date: '2026-11-18' })).code).toBe('curriculum.program_start_not_configured');
  });

  it('reports the imported curriculum from the manifest', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    const program = ok(await client.invoke('curriculum.program', {})).result;
    expect(program.curriculum).toMatchObject({ weeks: 12, days: 72, objectives: 216, planned_minutes: 28_800, provenance: 'PROPOSED_SEED' });
    expect(program.curriculum?.name).toMatch(/480-hour/);
    expect(program.program_start).toBeNull();
  });

  it('refuses a start that is not a Monday or a time that is not a clock time, and keeps nothing', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    for (const date of ['2026-11-03', '2026-11-08']) {
      expect(err(await client.invoke('curriculum.set_program_start', { date })).code).toBe('program.start_not_a_monday');
    }
    expect(err(await client.invoke('curriculum.set_program_start', { date: '2026-02-30' })).code).toBe('date.invalid');
    for (const day_start of ['9:00', '24:00', '09:60', 'ab:cd', '09:00:00']) {
      expect(err(await client.invoke('curriculum.set_program_start', { date: '2026-11-02', day_start })).category).toBe('INVALID_INPUT');
    }
    expect(ok(await client.invoke('curriculum.program', {})).result.program_start).toBeNull();
  });

  it('stores the start, numbers its revisions and moves the daily plan with it', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    const set = ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' })).result;
    expect(set).toMatchObject({ curriculum_id: 'g9-ct9-480', monday: '2026-11-02', day_start: '09:00' });
    const week3 = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result;
    expect(week3.week_number).toBe(3);
    expect(week3.blocks.every((b) => b.session_id === null)).toBe(true);
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-09', day_start: '08:30' }));
    const moved = ok(await client.invoke('curriculum.program', {})).result.program_start;
    expect(moved).toEqual({ monday: '2026-11-09', day_start: '08:30', revision: 2 });
    // A start a week later puts 2026-11-18 into week 2, which the one-week fixture does not hold.
    expect(err(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).category).toBe('NOT_FOUND');
  });

  it('refuses a stale revision of the start', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    const stale = err(await client.invoke('curriculum.set_program_start', { date: '2026-11-09' }, { expectedRevision: 7 }));
    expect(stale.category).toBe('STALE_REVISION');
  });

  it('names no week before week 1 and after the last week, like the backend, and keeps the Sunday of a week as a rest day', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-16' }));
    const before = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-14' })).result;
    expect(before).toMatchObject({ unscheduled: true, week_number: null, week_title: null, day_id: null, blocks: [], objectives: [] });
    const after = ok(await client.invoke('curriculum.daily_plan', { date: '2027-02-15' })).result;
    expect(after).toMatchObject({ unscheduled: true, week_number: null });
    // The Sunday of week 1 is inside the program: a rest day with its week named (the fixture holds week 3, so the one
    // week it can answer is week 3, which starts two weeks after this start).
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    const sunday = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-22' })).result;
    expect(sunday).toMatchObject({ unscheduled: true, week_number: 3 });
  });

  it('warns that the plan comes from a proposed seed, as the backend does', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    const plan = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' }));
    expect(plan.warnings?.[0]).toMatch(/^curriculum\.non_canonical/);
  });
});

describe('mock transport: session.plan_day', () => {
  async function ready() {
    const h = harness({ fresh: true });
    await importSeed(h.client);
    ok(await h.client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    return h;
  }

  it('plans one session per block, linked to the day and the block, and the plan then names them', async () => {
    const { client } = await ready();
    const before = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result;
    const planned = ok(await client.invoke('session.plan_day', { date: '2026-11-18' })).result;
    expect(planned.created).toHaveLength(before.blocks.length);
    expect(planned.already_planned).toEqual([]);
    planned.created.forEach((s, i) => {
      expect(s).toMatchObject({ state: 'PLANNED', day_id: before.day_id, block: before.blocks[i]?.block, block_seq: i + 1, credited_ms: 0 });
      expect(s.planned_ms).toBe((before.blocks[i]?.minutes ?? 0) * 60_000);
    });
    const after = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result;
    expect(after.blocks.map((b) => b.session_id)).toEqual(planned.created.map((s) => s.session_id));
    expect(ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions).toHaveLength(before.blocks.length);
  });

  it('plans only what is missing when called again and creates nothing the second time', async () => {
    const { client } = await ready();
    ok(await client.invoke('session.plan_day', { date: '2026-11-18' }));
    const again = ok(await client.invoke('session.plan_day', { date: '2026-11-18' })).result;
    expect(again.created).toEqual([]);
    expect(again.already_planned.length).toBeGreaterThan(0);
    expect(ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions).toHaveLength(again.already_planned.length);
  });

  it('refuses a Sunday, a day outside the weeks the program holds and a malformed date', async () => {
    const { client } = await ready();
    for (const date of ['2026-11-22', '2026-10-27', '2027-03-01']) {
      const e = err(await client.invoke('session.plan_day', { date }));
      expect(e.category).toBe('NOT_FOUND');
      expect(e.code).toBe('curriculum.no_training_day');
    }
    expect(err(await client.invoke('session.plan_day', { date: '2026-13-01' })).code).toBe('date.invalid');
    expect(ok(await client.invoke('session.list', {})).result.sessions).toEqual([]);
  });

  it('a planned session runs like any other (prepare, start, complete)', async () => {
    const { client, clock } = await ready();
    const first = ok(await client.invoke('session.plan_day', { date: '2026-11-18' })).result.created[0];
    if (first === undefined) {
      throw new Error('nothing planned');
    }
    ok(await client.invoke('session.prepare', { session_id: first.session_id }));
    ok(await client.invoke('session.start', { session_id: first.session_id }));
    clock.advance(10 * 60_000);
    const done = ok(await client.invoke('session.complete', { session_id: first.session_id })).result;
    expect(done.state).toBe('COMPLETED');
    expect(done.credited_ms).toBe(10 * 60_000);
  });
});

describe('mock transport: notes read-back', () => {
  async function note(client: Client, title: string, body: string, classification: 'INTERNAL' | 'PRIVATE' = 'INTERNAL') {
    return ok(await client.invoke('notes.create', { title, body_markdown: body, classification, domain: 'LEARNING' })).result;
  }

  it('lists the newest change first, without bodies, and opens one with its body', async () => {
    const { client, clock } = harness();
    const first = await note(client, 'First', 'alpha');
    clock.advance(1000);
    const second = await note(client, 'Second', 'bravo bravo');
    clock.advance(1000);
    ok(await client.invoke('notes.revise', { note_id: first.note_id, title: 'First, revised', body_markdown: 'alpha alpha alpha' }, { expectedRevision: 1 }));
    const listed = ok(await client.invoke('notes.list', {})).result.notes;
    expect(listed.map((n) => n.note_id)).toEqual([first.note_id, second.note_id]);
    expect(listed[0]).toMatchObject({ title: 'First, revised', version_count: 2, revision: 2, size_bytes: 'alpha alpha alpha'.length });
    expect(listed.every((n) => !('body_markdown' in n))).toBe(true);
    const opened = ok(await client.invoke('notes.get', { note_id: second.note_id })).result;
    expect(opened.body_markdown).toBe('bravo bravo');
  });

  it('bounds the list, counts bytes not characters and refuses a limit of zero', async () => {
    const { client, clock } = harness();
    for (const title of ['a', 'b', 'c']) {
      await note(client, title, 'x');
      clock.advance(1000);
    }
    expect(ok(await client.invoke('notes.list', { limit: 2 })).result.notes).toHaveLength(2);
    expect(err(await client.invoke('notes.list', { limit: 0 })).category).toBe('INVALID_INPUT');
    const wide = await note(client, 'wide', 'äöü');
    const summary = ok(await client.invoke('notes.list', {})).result.notes.find((n) => n.note_id === wide.note_id);
    expect(summary?.size_bytes).toBe(6);
  });

  it('does not list or open a note of another domain from this scope', async () => {
    const { client } = harness();
    await note(client, 'Learning note', 'visible');
    const sensitive = err(
      await client.invoke(
        'notes.create',
        { title: 'Scope letter', body_markdown: 'authorized targets', classification: 'SECRET', domain: 'SECURITY_SENSITIVE' },
        { domain: 'SECURITY_SENSITIVE' },
      ),
    );
    expect(sensitive.category).toBe('APPROVAL_REQUIRED');
    const approvalId = sensitive.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    const created = ok(
      await client.invoke(
        'notes.create',
        { title: 'Scope letter', body_markdown: 'authorized targets', classification: 'SECRET', domain: 'SECURITY_SENSITIVE' },
        { domain: 'SECURITY_SENSITIVE', approvalId },
      ),
    ).result;
    expect(ok(await client.invoke('notes.list', {})).result.notes.map((n) => n.title)).toEqual(['Learning note']);
    expect(ok(await client.invoke('notes.list', {}, { domain: 'SECURITY_SENSITIVE' })).result.notes.map((n) => n.title)).toEqual(['Scope letter']);
    expect(err(await client.invoke('notes.get', { note_id: created.note_id })).category).toBe('UNAUTHORIZED');
    expect(ok(await client.invoke('notes.get', { note_id: created.note_id }, { domain: 'SECURITY_SENSITIVE' })).result.body_markdown).toBe('authorized targets');
  });

  it('an erased note leaves the list and cannot be opened; an unknown id is not found', async () => {
    const { client } = harness();
    const keep = await note(client, 'Keep', 'stays');
    const gone = await note(client, 'Door code', 'pin-code-4711');
    const asked = err(await client.invoke('deletion.request', { resource: { kind: 'NOTE', id: gone.note_id }, reason: 'the door code changed' }));
    const approvalId = asked.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    ok(await client.invoke('deletion.request', { resource: { kind: 'NOTE', id: gone.note_id }, reason: 'the door code changed' }, { approvalId }));
    const listed = ok(await client.invoke('notes.list', {})).result.notes;
    expect(listed.map((n) => n.note_id)).toEqual([keep.note_id]);
    const opened = err(await client.invoke('notes.get', { note_id: gone.note_id }));
    expect(opened).toMatchObject({ category: 'NOT_FOUND', code: 'note.erased' });
    expect(JSON.stringify(ok(await client.invoke('notes.list', {})))).not.toContain('4711');
    expect(err(await client.invoke('notes.get', { note_id: 'note-unknown' })).code).toBe('note.not_found');
  });
});

describe('mock transport: competency packs', () => {
  const edgesOnDisk = (): Set<string> => {
    const out = new Set<string>();
    for (const dir of readdirSync(PACK_DIR)) {
      const pack = JSON.parse(readFileSync(`${PACK_DIR}${dir}/pack.json`, 'utf8')) as { competencies: { id: string; prerequisites: string[] }[] };
      for (const c of pack.competencies) {
        for (const pre of c.prerequisites) {
          out.add(`${pre.slice(pre.indexOf(':') + 1)}>${c.id}`);
        }
      }
    }
    return out;
  };

  it('loads all 64 competencies and every prerequisite edge, and the graph lists them unassessed', async () => {
    const { client } = harness({ fresh: true });
    const done = ok(await client.invoke('competency.import_pack', {})).result;
    expect(done).toMatchObject({ loaded: 64, available: 64, prerequisites_pending: 0 });
    expect(done.prerequisites_added).toBe(edgesOnDisk().size);
    expect(done.packs).toHaveLength(8);
    expect(done.packs.every((p) => p.created === 8 && p.existing === 0 && p.status === 'PROPOSED')).toBe(true);
    const rows = ok(await client.invoke('competency.projection', {})).result.projections;
    expect(rows).toHaveLength(64);
    expect(rows.every((r) => r.state === 'UNASSESSED')).toBe(true);
    expect(ok(await client.invoke('curriculum.program', {})).result.competencies).toEqual({ loaded: 64, available: 64 });
  });

  it('stores nothing new the second time', async () => {
    const { client } = harness({ fresh: true });
    ok(await client.invoke('competency.import_pack', {}));
    const again = ok(await client.invoke('competency.import_pack', {})).result;
    expect(again.prerequisites_added).toBe(0);
    expect(again.packs.every((p) => p.created === 0 && p.existing === 8)).toBe(true);
  });

  it('completes the edges of a pack loaded earlier when the pack it needs is loaded later', async () => {
    const { client } = harness({ fresh: true });
    const first = ok(await client.invoke('competency.import_pack', { pack_ids: ['agentic-mcp-supplychain'] })).result;
    expect(first.loaded).toBe(8);
    expect(first.prerequisites_pending).toBeGreaterThanOrEqual(1);
    const second = ok(await client.invoke('competency.import_pack', { pack_ids: ['llm-rag-security'] })).result;
    expect(second.loaded).toBe(16);
    expect(second.prerequisites_added).toBeGreaterThanOrEqual(1);
  });

  it('refuses an unknown or malformed pack id and stores nothing', async () => {
    const { client } = harness({ fresh: true });
    expect(err(await client.invoke('competency.import_pack', { pack_ids: ['no-such-pack'] })).code).toBe('competency.pack_unknown');
    for (const pack_ids of [['../web-api-appsec'], [], ['web-api-appsec', 'web-api-appsec'], ['Web']]) {
      expect(err(await client.invoke('competency.import_pack', { pack_ids })).category).toBe('INVALID_INPUT');
    }
    expect(ok(await client.invoke('curriculum.program', {})).result.competencies.loaded).toBe(0);
  });

  it('does not count a competency the operator wrote as a pack competency', async () => {
    const { client } = harness({ fresh: true });
    ok(await client.invoke('competency.create', { code: 'MINE-1', title: 'My own competency' }));
    expect(ok(await client.invoke('curriculum.program', {})).result.competencies).toEqual({ loaded: 0, available: 64 });
    ok(await client.invoke('competency.import_pack', { pack_ids: ['web-api-appsec'] }));
    expect(ok(await client.invoke('curriculum.program', {})).result.competencies).toEqual({ loaded: 8, available: 64 });
    expect(ok(await client.invoke('competency.projection', {})).result.projections).toHaveLength(9);
  });
});

describe('mock transport: backup.list', () => {
  it('is empty before a backup and lists them newest first with their state afterwards', async () => {
    const { client, clock } = harness();
    expect(ok(await client.invoke('backup.list', {})).result).toEqual({ backups: [], unlisted: 0 });
    const first = ok(await client.invoke('backup.create', { label: 'before the import' })).result;
    clock.advance(60_000);
    const second = ok(await client.invoke('backup.create', {})).result;
    const listed = ok(await client.invoke('backup.list', {})).result.backups;
    expect(listed.map((b) => b.backup_id)).toEqual([second.backup_id, first.backup_id]);
    expect(listed[1]?.label).toBe('before the import');
    ok(await client.invoke('backup.verify', { backup_id: first.backup_id }));
    const verified = ok(await client.invoke('backup.list', {})).result.backups.find((b) => b.backup_id === first.backup_id);
    expect(verified).toMatchObject({ state: 'CHECKED', integrity: 'OK' });
  });
});


describe('mock transport: the imported Appendix A seed keeps its shape next to the v2 plan (ADR-0023)', () => {
  async function ready() {
    const h = harness({ fresh: true });
    await importSeed(h.client);
    ok(await h.client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    return h;
  }

  it('reports profile CT9_480 and six days a week for the imported seed', async () => {
    const { client } = harness({ fresh: true });
    await importSeed(client);
    expect(ok(await client.invoke('curriculum.program', {})).result.curriculum).toMatchObject({
      profile: 'CT9_480',
      days_per_week: 6,
      weeks: 12,
      days: 72,
      objectives: 216,
      planned_minutes: 28_800,
    });
  });

  it('labels each block of a weekday with its code and spaces, and the Saturday as before', async () => {
    const { client } = await ready();
    const wednesday = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result;
    expect(wednesday.blocks.map((b) => b.label)).toEqual(['recall', 'theory', 'guided lab', 'independent lab', 'defense verification', 'evidence reporting']);
    expect(wednesday.blocks.map((b) => b.label)).toEqual(wednesday.blocks.map((b) => b.block.replace(/_/g, ' ')));
    const saturday = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-21' })).result;
    expect(saturday.is_assessment_day).toBe(true);
    expect(saturday.blocks.map((b) => b.label)).toEqual(['blind practical', 'remediation', 'report', 'performance review']);
  });

  it('plans a weekday back to back with no pause, titled with the code and spaces', async () => {
    const { client } = await ready();
    const planned = ok(await client.invoke('session.plan_day', { date: '2026-11-18' })).result;
    expect(planned.created.map((s) => s.planned_ms / 60_000)).toEqual([15, 45, 120, 120, 60, 60]);
    const events = ok(await client.invoke('calendar.list', { from: Date.UTC(2026, 10, 18), to: Date.UTC(2026, 10, 19) })).result.events.filter((e) => e.day_id === 'D15');
    expect(events.map((e) => e.title)).toEqual(['D15 recall', 'D15 theory', 'D15 guided lab', 'D15 independent lab', 'D15 defense verification', 'D15 evidence reporting']);
    // 09:00 in Europe/Berlin (UTC+1) is 08:00Z; each block starts when the one before it ends.
    expect(events[0]?.start).toBe(Date.UTC(2026, 10, 18, 8, 0));
    for (let i = 1; i < events.length; i += 1) {
      expect(events[i]?.start).toBe(events[i - 1]?.end);
    }
    expect(events[events.length - 1]?.end).toBe(Date.UTC(2026, 10, 18, 8, 0) + 420 * 60_000);
  });

  it('importing the seed after a v2 manifest brings back the Appendix A profile', async () => {
    const { client } = harness({ fresh: true });
    const v2 = asJson(v2Manifest());
    const asked = err(await client.invoke('curriculum.import', { manifest: v2, provenance: 'PROPOSED_SEED' }));
    ok(await client.invoke('approvals.decide', { approval_id: asked.approval_id ?? '', decision: 'APPROVE' }));
    ok(await client.invoke('curriculum.import', { manifest: v2, provenance: 'PROPOSED_SEED' }, { approvalId: asked.approval_id ?? '' }));
    expect(ok(await client.invoke('curriculum.program', {})).result.curriculum).toMatchObject({ profile: 'MASTERY_V2', days_per_week: 5 });
    await importSeed(client);
    expect(ok(await client.invoke('curriculum.program', {})).result.curriculum).toMatchObject({ profile: 'CT9_480', days_per_week: 6, days: 72 });
    ok(await client.invoke('curriculum.set_program_start', { date: '2026-11-02' }));
    // The Saturday is a training day of the Appendix A shape again; the one-week fixture answers week 3.
    expect(ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-21' })).result).toMatchObject({ day_id: 'D18', is_assessment_day: true });
  });
});
