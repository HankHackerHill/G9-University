import { describe, expect, it } from 'vitest';
import { createClient, isG9Error, type InvokeResult } from '../client';
import type { OperationName } from '../operations';
import type { ClientCommandMeta } from '../types';
import { MOCK_DEFAULT_NOW, createMockClock, createMockTransport } from './mock';

function harness() {
  const clock = createMockClock();
  const transport = createMockTransport({ clock });
  let n = 0;
  const client = createClient(transport, {
    now: () => clock.now(),
    uuid: () => {
      n += 1;
      return `u${n}`;
    },
  });
  return { clock, transport, client };
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

describe('mock transport: pipeline', () => {
  it('refuses metadata that carries an actor (Addendum A.8) and assigns the operator itself', async () => {
    const { transport } = harness();
    const meta: ClientCommandMeta & { actor: unknown } = {
      request_id: 'r',
      operation: 'system.status',
      idempotency_key: 'k',
      scope: { domain: 'SYSTEM', mode: 'STUDY' },
      payload_schema_version: { major: 1, minor: 0 },
      trace_id: 't',
      issued_at: MOCK_DEFAULT_NOW,
      actor: { id: 'attacker', kind: { kind: 'OPERATOR' } },
    };
    const r = await transport.invoke('system.status', meta, {});
    expect(isG9Error(r) && r.category === 'INVALID_INPUT' && r.code === 'meta.actor_not_accepted').toBe(true);

    const { client } = harness();
    const audit = ok(await client.invoke('audit.recent', { limit: 5 }));
    expect(audit.result.entries.every((e) => e.actor_kind === 'OPERATOR' || e.actor_kind === 'SYSTEM')).toBe(true);
  });

  it('reports the model as unavailable with NO_RUNTIME_CONFIGURED and network egress off', async () => {
    const { client } = harness();
    const s = ok(await client.invoke('system.status', {})).result;
    expect(s.model).toEqual({ available: false, reason: 'NO_RUNTIME_CONFIGURED' });
    expect(s.network.egress_allowed).toBe(false);
    expect(s.transport).toBe('mock');
    expect(s.clock.zone).toBe('Europe/Berlin');
  });

  it('rejects an unaccepted payload schema version', async () => {
    const { transport } = harness();
    const r = await transport.invoke(
      'system.status',
      {
        request_id: 'r',
        operation: 'system.status',
        idempotency_key: 'k',
        scope: { domain: 'SYSTEM', mode: 'STUDY' },
        payload_schema_version: { major: 2, minor: 0 },
        trace_id: 't',
        issued_at: MOCK_DEFAULT_NOW,
      },
      {},
    );
    expect(isG9Error(r) && r.code === 'ipc.schema_version_rejected').toBe(true);
  });

  it('replays an idempotent command and answers CONFLICT when the key is reused with a different request', async () => {
    const { client } = harness();
    const today = ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions;
    const target = today.find((s) => s.state === 'PLANNED');
    expect(target).toBeDefined();
    if (target === undefined) {
      return;
    }
    const first = ok(await client.invoke('session.prepare', { session_id: target.session_id }, { idempotencyKey: 'same' }));
    const replay = ok(await client.invoke('session.prepare', { session_id: target.session_id }, { idempotencyKey: 'same' }));
    expect(first.replayed).toBe(false);
    expect(replay.replayed).toBe(true);
    expect(replay.result.state).toBe('READY');
    const conflict = err(await client.invoke('session.start', { session_id: target.session_id }, { idempotencyKey: 'same' }));
    expect(conflict.category).toBe('CONFLICT');
  });
});

describe('mock transport: session machine and timer ledger', () => {
  it('walks PLANNED → READY → RUNNING → PAUSED → RUNNING → COMPLETED and credits only closed VERIFIED intervals', async () => {
    const { client, clock } = harness();
    const planned = ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions.find((s) => s.state === 'PLANNED');
    if (planned === undefined) {
      throw new Error('fixture has no planned session today');
    }
    const id = planned.session_id;
    expect(ok(await client.invoke('session.prepare', { session_id: id })).result.state).toBe('READY');
    const running = ok(await client.invoke('session.start', { session_id: id })).result;
    expect(running.state).toBe('RUNNING');
    expect(running.open_interval_started_at).toBe(MOCK_DEFAULT_NOW);
    expect(running.credited_ms).toBe(0);
    clock.advance(10 * 60_000);
    const paused = ok(await client.invoke('session.pause', { session_id: id })).result;
    expect(paused.state).toBe('PAUSED');
    expect(paused.credited_ms).toBe(10 * 60_000);
    expect(paused.open_interval_started_at).toBeNull();
    ok(await client.invoke('session.resume', { session_id: id }));
    clock.advance(5 * 60_000);
    const done = ok(await client.invoke('session.complete', { session_id: id })).result;
    expect(done.state).toBe('COMPLETED');
    expect(done.credited_ms).toBe(15 * 60_000);
    expect(done.uncertain_ms).toBe(0);
    expect(done.intervals).toHaveLength(2);
  });

  it('rejects illegal moves with PRECONDITION_FAILED session.illegal_transition', async () => {
    const { client } = harness();
    const planned = ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions.find((s) => s.state === 'PLANNED');
    if (planned === undefined) {
      throw new Error('no planned session');
    }
    const r1 = err(await client.invoke('session.start', { session_id: planned.session_id }));
    expect(r1.category).toBe('PRECONDITION_FAILED');
    expect(r1.code).toBe('session.illegal_transition');
    expect(r1.details).toEqual({ machine: 'session', from: 'PLANNED', event: 'START' });
    const r2 = err(await client.invoke('session.pause', { session_id: planned.session_id }));
    expect(r2.code).toBe('session.illegal_transition');
    const interrupted = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.state === 'INTERRUPTED');
    if (interrupted === undefined) {
      throw new Error('fixture has no interrupted session');
    }
    expect(err(await client.invoke('session.resume', { session_id: interrupted.session_id })).code).toBe('session.illegal_transition');
  });

  it('answers STALE_REVISION when expected_revision does not match', async () => {
    const { client } = harness();
    const planned = ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions.find((s) => s.state === 'PLANNED');
    if (planned === undefined) {
      throw new Error('no planned session');
    }
    const r = err(await client.invoke('session.prepare', { session_id: planned.session_id }, { expectedRevision: planned.revision + 5 }));
    expect(r.category).toBe('STALE_REVISION');
    expect(r.details).toEqual({ expected: planned.revision + 5, actual: planned.revision });
  });

  it('recovers an INTERRUPTED session only through a correction that references the uncertain interval', async () => {
    const { client } = harness();
    const interrupted = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.state === 'INTERRUPTED');
    if (interrupted === undefined) {
      throw new Error('fixture has no interrupted session');
    }
    expect(interrupted.requires_correction).toBe(true);
    expect(interrupted.uncertain_ms).toBe(40 * 60_000);
    expect(interrupted.credited_ms).toBe(0);
    const interval = interrupted.intervals[0];
    if (interval === undefined) {
      throw new Error('no interval');
    }
    const tooMuch = err(
      await client.invoke('session.correct_time', { session_id: interrupted.session_id, interval_id: interval.interval_id, credited_ms: 60 * 60_000, reason: 'x' }),
    );
    expect(tooMuch.code).toBe('correction.exceeds_interval');
    const fixed = ok(
      await client.invoke('session.correct_time', { session_id: interrupted.session_id, interval_id: interval.interval_id, credited_ms: 30 * 60_000, reason: 'laptop suspended' }),
    ).result;
    expect(fixed.state).toBe('PAUSED');
    expect(fixed.requires_correction).toBe(false);
    expect(fixed.credited_ms).toBe(30 * 60_000);
    expect(fixed.uncertain_ms).toBe(40 * 60_000);
    expect(fixed.intervals[0]?.continuity).toBe('UNCERTAIN');
    expect(fixed.corrections).toHaveLength(1);
  });
});

describe('mock transport: approvals', () => {
  it('runs the APPROVAL_REQUIRED flow for assessment.approve_final with a hash-bound, single-use approval', async () => {
    const { client } = harness();
    const attemptId = 'mock-attempt-prev';
    const first = err(await client.invoke('assessment.approve_final', { attempt_id: attemptId }));
    expect(first.category).toBe('APPROVAL_REQUIRED');
    expect(first.approval_id).toBeDefined();
    const approvalId = first.approval_id ?? '';

    const listed = ok(await client.invoke('approvals.list', { state: 'PENDING' })).result.approvals;
    const approval = listed.find((a) => a.approval_id === approvalId);
    expect(approval?.operation_name).toBe('assessment.approve_final');
    expect(approval?.operation.parameters_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(approval?.expires_at).toBe(MOCK_DEFAULT_NOW + 15 * 60_000);

    // Using a pending approval is refused.
    expect(err(await client.invoke('assessment.approve_final', { attempt_id: attemptId }, { approvalId })).code).toBe('approval.not_decided');

    const approved = ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' })).result;
    expect(approved.state).toBe('APPROVED');

    // Different parameters → binding mismatch, UNAUTHORIZED.
    const other = err(await client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-other' }, { approvalId }));
    expect(['UNAUTHORIZED', 'NOT_FOUND']).toContain(other.category);

    const finalized = ok(await client.invoke('assessment.approve_final', { attempt_id: attemptId }, { approvalId })).result;
    expect(finalized.state).toBe('APPROVED_FINAL');
    expect(finalized.grade?.final).toBe(true);

    const consumed = ok(await client.invoke('approvals.list', {})).result.approvals.find((a) => a.approval_id === approvalId);
    expect(consumed?.state).toBe('CONSUMED');
    // Finalized attempts are immutable: no outgoing edge.
    expect(err(await client.invoke('assessment.approve_final', { attempt_id: attemptId }, { approvalId })).category).toBe('PRECONDITION_FAILED');
  });

  it('expires approvals after 15 minutes and refuses to consume them', async () => {
    const { client, clock } = harness();
    const first = err(await client.invoke('calendar.propose', { from: 0, to: 1, intent: 'RESCHEDULE' }));
    expect(first.code).toBe('calendar.reschedule.not_implemented');
    const proposal = ok(await client.invoke('calendar.propose', { from: MOCK_DEFAULT_NOW, to: MOCK_DEFAULT_NOW + 7 * 86_400_000, intent: 'ADD_REVIEWS' })).result;
    const need = err(await client.invoke('calendar.apply', { proposal_id: proposal.proposal_id }));
    expect(need.category).toBe('APPROVAL_REQUIRED');
    const approvalId = need.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    clock.advance(16 * 60_000);
    const late = err(await client.invoke('calendar.apply', { proposal_id: proposal.proposal_id }, { approvalId }));
    expect(['approval.expired', 'approval.not_approved', 'proposal.expired']).toContain(late.code);
  });
});

describe('mock transport: model-down, audit and retention', () => {
  it('answers RESOURCE_UNAVAILABLE for skills.invoke and UNAUTHORIZED for teaching skills in EXAM mode', async () => {
    const { client } = harness();
    const r = err(await client.invoke('skills.invoke', { skill_code: 'T01', input: {} }));
    expect(r.category).toBe('RESOURCE_UNAVAILABLE');
    expect(r.code).toBe('inference.runtime_unavailable');
    const exam = err(await client.invoke('skills.invoke', { skill_code: 'T01', input: {}, mode: 'EXAM' }));
    expect(exam.category).toBe('UNAUTHORIZED');
    expect(exam.code).toBe('policy.hints_denied_in_exam');
  });

  it('keeps a verifiable hash chain over audit metadata for mutating commands', async () => {
    const { client } = harness();
    ok(await client.invoke('notes.create', { title: 'a', body_markdown: 'b', classification: 'PRIVATE', domain: 'LEARNING' }));
    ok(await client.invoke('backup.create', {}));
    const audit = ok(await client.invoke('audit.recent', { limit: 10 })).result;
    expect(audit.chain_ok).toBe(true);
    expect(audit.entries.length).toBeGreaterThanOrEqual(3);
    const ops = audit.entries.map((e) => e.operation);
    expect(ops).toContain('notes.create');
    expect(ops).toContain('backup.create');
    for (const e of audit.entries) {
      expect(e.row_hash).toMatch(/^[0-9a-f]{64}$/);
      expect(e.details_sha256).toMatch(/^[0-9a-f]{64}$/);
    }
  });

  it('derives the review queue deterministically from the fixture (baseline offsets, budget overflow)', async () => {
    const { client } = harness();
    const due = ok(await client.invoke('retention.due', {})).result;
    expect(due.overdue.map((r) => r.objective_id)).toEqual(['D13-O1', 'D13-O2', 'D13-O3']);
    expect(due.due.map((r) => r.objective_id)).toEqual(['D14-O1', 'D14-O2']);
    expect(due.overflow.map((r) => r.objective_id)).toEqual(['D14-O3']);
    expect(due.budget_ms).toBe(15 * 60_000);
  });

  it('drives the backup machine and refuses a restore test before a check', async () => {
    const { client } = harness();
    const created = ok(await client.invoke('backup.create', { label: 'nightly' })).result;
    expect(created.state).toBe('SNAPSHOT_CREATED');
    const early = err(await client.invoke('backup.restore_test', { backup_id: created.backup_id }));
    expect(early.code).toBe('backup.illegal_transition');
    const checked = ok(await client.invoke('backup.verify', { backup_id: created.backup_id })).result;
    expect(checked.state).toBe('CHECKED');
    expect(checked.integrity).toBe('OK');
    const tested = ok(await client.invoke('backup.restore_test', { backup_id: created.backup_id })).result;
    expect(tested.state).toBe('RESTORE_TESTED');
    expect(tested.restore_tested_at).toBe(MOCK_DEFAULT_NOW);
  });

  it('serves the fixture daily plan and refuses dates outside the loaded week', async () => {
    const { client } = harness();
    const plan = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result;
    expect(plan.day_id).toBe('D15');
    expect(plan.objectives.map((o) => o.objective_id)).toEqual(['D15-O1', 'D15-O2', 'D15-O3']);
    expect(plan.blocks).toHaveLength(6);
    const sunday = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-22' })).result;
    expect(sunday.unscheduled).toBe(true);
    expect(err(await client.invoke('curriculum.daily_plan', { date: '2026-12-25' })).category).toBe('NOT_FOUND');
    expect(err(await client.invoke('curriculum.daily_plan', { date: 'yesterday' })).category).toBe('INVALID_INPUT');
  });
});

describe('mock transport: curriculum promotion', () => {
  function manifest(provenance: string, overrideDay?: { id: string; text: string }) {
    const weeks = Array.from({ length: 12 }, (_, w) => ({
      week_number: w + 1,
      days: Array.from({ length: 6 }, (_, d) => {
        const n = w * 6 + d + 1;
        const id = `D${String(n).padStart(2, '0')}`;
        const text = overrideDay?.id === id ? overrideDay.text : `objective for ${id}`;
        return { day_id: id, objectives: [1, 2, 3].map((seq) => ({ seq, objective_id: `${id}-O${String(seq)}`, text })) };
      }),
    }));
    return {
      curriculum_id: 'g9-ct9-480',
      version: '0.2.0',
      provenance,
      structure: { weeks: 12, days_per_week: 6, objectives_per_day: 3, weekday_minutes: 420, saturday_minutes: 300, weekly_minutes: 2400, total_minutes: 28800 },
      weeks,
    };
  }

  it('refuses promotion when the candidate does not reconcile with the original plan, before any approval is created', async () => {
    const { client } = harness();
    const r = err(
      await client.invoke('curriculum.promote', {
        curriculum_version_id: 'g9-ct9-480:0.2.0',
        candidate_manifest: manifest('RECONCILED_CANDIDATE', { id: 'D15', text: 'silently changed objective' }),
        original_manifest: manifest('APPROVED_CANONICAL'),
      }),
    );
    expect(r.category).toBe('PRECONDITION_FAILED');
    expect(r.code).toBe('curriculum.reconciliation_blocks_promotion');
    const pending = ok(await client.invoke('approvals.list', { state: 'PENDING' })).result.approvals;
    expect(pending.some((a) => a.operation_name === 'curriculum.promote')).toBe(false);
  });

  it('requires a hash-bound CURRICULUM_PROMOTE approval and returns APPROVED_CANONICAL with checksums once approved', async () => {
    const { client } = harness();
    const payload = {
      curriculum_version_id: 'g9-ct9-480:0.2.0',
      candidate_manifest: manifest('RECONCILED_CANDIDATE'),
      original_manifest: manifest('RECONCILED_CANDIDATE'),
    };
    const first = err(await client.invoke('curriculum.promote', payload));
    expect(first.category).toBe('APPROVAL_REQUIRED');
    const approvalId = first.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    const other = err(await client.invoke('curriculum.promote', { ...payload, curriculum_version_id: 'g9-ct9-480:9.9.9' }, { approvalId }));
    expect(other.category).toBe('UNAUTHORIZED');
    const done = ok(await client.invoke('curriculum.promote', payload, { approvalId })).result;
    expect(done.provenance).toBe('APPROVED_CANONICAL');
    expect(done.promotion_allowed).toBe(true);
    expect(done.summary).toEqual({ unchanged: 72, changed: 0, missing_in_candidate: 0, added_in_candidate: 0, metadata_changes: [] });
    expect(done.candidate_checksum).toMatch(/^[0-9a-f]{64}$/);
  });
});
