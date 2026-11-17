import { describe, expect, it } from 'vitest';
import { MAX_ATTACHMENT_BYTES } from '../../lib/attachment';
import { sha256HexBytes } from '../../lib/sha256';
import { createClient, isG9Error, type InvokeResult } from '../client';
import { OPERATIONS, type CriterionResult, type GradeSource, type OperationName, type RequestOf } from '../operations';
import type { ClientCommandMeta } from '../types';
import { BASELINE_CRITERIA, MOCK_DEFAULT_NOW, MOCK_EXPORT_DIR, computeGrade, createMockClock, createMockTransport, evidenceSummary, type MockOptions } from './mock';
import { encodeBase64 } from './mock-attachments';

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

  it('refuses a revision of 0 at the boundary, before any approval is asked, as g9-app does', async () => {
    const { client } = harness();
    const before = ok(await client.invoke('approvals.list', {})).result.approvals.length;
    const refused = err(await client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' }, { expectedRevision: 0 }));
    expect(refused).toMatchObject({ category: 'INVALID_INPUT', code: 'meta.invalid' });
    expect(ok(await client.invoke('approvals.list', {})).result.approvals).toHaveLength(before);
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
    const outcome = ok(
      await client.invoke('session.correct_time', { session_id: interrupted.session_id, interval_id: interval.interval_id, credited_ms: 30 * 60_000, reason: 'laptop suspended' }),
    );
    const fixed = outcome.result;
    expect(fixed.state).toBe('PAUSED');
    expect(fixed.requires_correction).toBe(false);
    expect(fixed.credited_ms).toBe(30 * 60_000);
    // `Ledger::credited`: the correction supersedes the interval, which leaves uncertain_ms.
    expect(fixed.uncertain_ms).toBe(0);
    expect(fixed.intervals[0]?.continuity).toBe('UNCERTAIN');
    expect(fixed.corrections).toHaveLength(1);
    expect(outcome.warnings).toBeUndefined();
  });

  it('a second interruption (process restart) reopens the correction flow; the latest correction of an interval wins', async () => {
    const { client, clock, transport } = harness();
    const interrupted = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.state === 'INTERRUPTED');
    if (interrupted === undefined) {
      throw new Error('fixture has no interrupted session');
    }
    const id = interrupted.session_id;
    const i1 = interrupted.intervals[0]?.interval_id ?? '';
    ok(await client.invoke('session.correct_time', { session_id: id, interval_id: i1, credited_ms: 30 * 60_000, reason: 'first suspend' }));
    ok(await client.invoke('session.resume', { session_id: id }));
    clock.advance(20 * 60_000);
    expect(transport.onProcessRestart()).toEqual([id]);
    const again = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.session_id === id);
    expect(again?.state).toBe('INTERRUPTED');
    expect(again?.requires_correction).toBe(true);
    expect(again?.credited_ms).toBe(30 * 60_000);
    expect(again?.uncertain_ms).toBe(20 * 60_000);
    expect(again?.intervals.map((i) => i.continuity)).toEqual(['UNCERTAIN', 'UNCERTAIN']);
    const i2 = again?.intervals[1]?.interval_id ?? '';
    const recovered = ok(await client.invoke('session.correct_time', { session_id: id, interval_id: i2, credited_ms: 10 * 60_000, reason: 'second suspend' }));
    expect(recovered.result.state).toBe('PAUSED');
    expect(recovered.result.credited_ms).toBe(40 * 60_000);
    expect(recovered.result.uncertain_ms).toBe(0);
    expect(recovered.warnings).toBeUndefined();
    // Superseding correction in a non-INTERRUPTED state: recorded, no transition, latest wins (timer.rs latest_correction_wins).
    const superseded = ok(await client.invoke('session.correct_time', { session_id: id, interval_id: i1, credited_ms: 25 * 60_000, reason: 'typo in first correction' })).result;
    expect(superseded.state).toBe('PAUSED');
    expect(superseded.corrections).toHaveLength(3);
    expect(superseded.credited_ms).toBe(35 * 60_000);
    expect(superseded.requires_correction).toBe(false);
  });

  it('completing an INTERRUPTED session keeps requires_correction true until the uncertain interval is corrected (session_view)', async () => {
    const { client } = harness();
    const interrupted = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.state === 'INTERRUPTED');
    if (interrupted === undefined) {
      throw new Error('fixture has no interrupted session');
    }
    const id = interrupted.session_id;
    const done = ok(await client.invoke('session.complete', { session_id: id })).result;
    expect(done.state).toBe('COMPLETED');
    expect(done.requires_correction).toBe(true);
    expect(done.uncertain_ms).toBe(40 * 60_000);
    const i1 = done.intervals[0]?.interval_id ?? '';
    const corrected = ok(await client.invoke('session.correct_time', { session_id: id, interval_id: i1, credited_ms: 40 * 60_000, reason: 'after completion' })).result;
    expect(corrected.state).toBe('COMPLETED');
    expect(corrected.requires_correction).toBe(false);
    expect(corrected.credited_ms).toBe(40 * 60_000);
    expect(corrected.uncertain_ms).toBe(0);
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
    expect(approval?.expires_at).toBe(MOCK_DEFAULT_NOW + 10 * 60_000);

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

  it('expires approvals after 10 minutes and refuses to consume them', async () => {
    const { client, clock } = harness();
    const first = err(await client.invoke('calendar.propose', { from: 0, to: 1, intent: 'RESCHEDULE' }));
    expect(first.code).toBe('calendar.reschedule.not_implemented');
    const proposal = ok(await client.invoke('calendar.propose', { from: MOCK_DEFAULT_NOW, to: MOCK_DEFAULT_NOW + 7 * 86_400_000, intent: 'ADD_REVIEWS' })).result;
    const need = err(await client.invoke('calendar.apply', { proposal_id: proposal.proposal_id }));
    expect(need.category).toBe('APPROVAL_REQUIRED');
    const approvalId = need.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    clock.advance(10 * 60_000 + 1);
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

describe('mock transport: fidelity with g9-app (review findings)', () => {
  const levels = (level: number, overrides: Record<string, { score?: number; source?: GradeSource }> = {}): CriterionResult[] =>
    BASELINE_CRITERIA.map((c) => ({
      criterion_code: c.code,
      weight: c.weight,
      score: overrides[c.code]?.score ?? level,
      source: overrides[c.code]?.source ?? 'HUMAN_REVIEWER',
      evidence_refs: [],
    }));

  it('grades on the anchored scale: level 3 everywhere is 80 and PASS, SELF_ADJUDICATED (LRN-10, ADR-0012)', () => {
    const g = computeGrade(levels(3));
    expect(g.total).toBe(80);
    expect(g.pass).toBe(true);
    expect(g.reliability_class).toBe('SELF_ADJUDICATED');
    expect(g.gate_failures).toEqual([]);
  });

  it('applies the 50 % floor and reports the critical criterion as the gate failure', () => {
    const atTwo = computeGrade(levels(2));
    expect(atTwo.total).toBe(50);
    expect(atTwo.pass).toBe(false);
    const gate = computeGrade(levels(4, { scope_compliance_reporting: { score: 1 } }));
    expect(gate.total).toBe(92.5);
    expect(gate.pass).toBe(false);
    expect(gate.gate_failures).toEqual(['scope_compliance_reporting']);
    const floor = computeGrade(levels(4, { root_cause_reasoning: { score: 1 } }));
    expect(floor.total).toBe(85);
    expect(floor.pass).toBe(false);
    expect(floor.gate_failures).toEqual([]);
  });

  it('never counts MODEL_ADVISORY results; a criterion with only advisory results leaves no total', () => {
    const withAdvisory = computeGrade([...levels(3), { criterion_code: 'independent_execution', weight: 25, score: 0, source: 'MODEL_ADVISORY', evidence_refs: [] }]);
    expect(withAdvisory.total).toBe(80);
    expect(withAdvisory.pass).toBe(true);
    const advisoryOnly = computeGrade(levels(4, { scope_compliance_reporting: { source: 'MODEL_ADVISORY' } }));
    expect(advisoryOnly.total).toBe(0);
    expect(advisoryOnly.pass).toBe(false);
    expect(advisoryOnly.gate_failures).toEqual(['scope_compliance_reporting']);
  });

  it('shows the seeded level-3 attempt as 80 PASS, as the backend would', async () => {
    const { client } = harness();
    const prev = ok(await client.invoke('assessment.list', {})).result.attempts.find((a) => a.attempt_id === 'mock-attempt-prev');
    expect(prev?.grade?.total).toBe(80);
    expect(prev?.grade?.pass).toBe(true);
  });

  it('rejects criterion codes outside the rubric, out-of-range levels and operator-recorded DETERMINISTIC_CHECKER results', async () => {
    const { client } = harness();
    const def = ok(await client.invoke('assessment.list', {})).result.definitions.find((d) => d.assessment_id !== 'mock-assessment-W2');
    if (def === undefined) {
      throw new Error('no definition');
    }
    const begun = ok(await client.invoke('assessment.begin', { assessment_id: def.assessment_id })).result;
    ok(await client.invoke('assessment.submit', { attempt_id: begun.attempt_id, evidence_refs: [] }));
    const inReview = ok(await client.invoke('assessment.review_evidence', { attempt_id: begun.attempt_id })).result;
    expect(inReview.state).toBe('EVIDENCE_REVIEW');
    const legacy = ['SCOPE', 'METHOD', 'EVIDENCE', 'REMEDIATION', 'VERIFICATION', 'REPORT'].map((code) => ({
      criterion_code: code,
      score: 3 as const,
      source: 'HUMAN_REVIEWER' as const,
      evidence_refs: [],
    }));
    const unknown = err(await client.invoke('assessment.grade_provisional', { attempt_id: begun.attempt_id, criteria: legacy }));
    expect(unknown.category).toBe('INVALID_INPUT');
    expect(unknown.code).toBe('grade.criterion_unknown');
    expect(unknown.details).toEqual({ criterion_code: 'SCOPE' });
    const baseline = BASELINE_CRITERIA.map((c) => ({ criterion_code: c.code, score: 3 as const, source: 'HUMAN_REVIEWER' as const, evidence_refs: [] }));
    const checker = baseline.map((c) => ({ ...c, source: 'DETERMINISTIC_CHECKER' as const }));
    expect(err(await client.invoke('assessment.grade_provisional', { attempt_id: begun.attempt_id, criteria: checker })).category).toBe('UNAUTHORIZED');
    const graded = ok(await client.invoke('assessment.grade_provisional', { attempt_id: begun.attempt_id, criteria: baseline })).result;
    expect(graded.state).toBe('GRADED_PROVISIONAL');
    expect(graded.grade?.total).toBe(80);
    expect(graded.grade?.pass).toBe(true);
  });

  it('answers INVALID_INPUT ipc.unknown_operation for an unknown operation, checked before the metadata (g9-app dispatch order)', async () => {
    const { transport } = harness();
    const meta = {
      request_id: 'r',
      operation: 'nope.operation',
      idempotency_key: 'k',
      scope: { domain: 'SYSTEM', mode: 'STUDY' },
      payload_schema_version: { major: 1, minor: 0 },
      trace_id: 't',
      issued_at: MOCK_DEFAULT_NOW,
    } satisfies ClientCommandMeta;
    const plain = await transport.invoke('nope.operation' as OperationName, meta, {});
    expect(isG9Error(plain) && plain.category === 'INVALID_INPUT' && plain.code === 'ipc.unknown_operation').toBe(true);
    const withActor = await transport.invoke('nope.operation' as OperationName, { ...meta, actor: { id: 'x', kind: { kind: 'OPERATOR' } } } as ClientCommandMeta, {});
    expect(isG9Error(withActor) && withActor.code === 'ipc.unknown_operation').toBe(true);
  });

  it('gates curriculum.import behind a CURRICULUM_IMPORT approval, like policy rule operator.import.curriculum.approval', async () => {
    expect(OPERATIONS['curriculum.import'].approvalExpected).toBe(true);
    const { client } = harness();
    const weeks = Array.from({ length: 12 }, (_, w) => ({
      week_number: w + 1,
      days: Array.from({ length: 6 }, (_, d) => {
        const id = `D${String(w * 6 + d + 1).padStart(2, '0')}`;
        return { day_id: id, objectives: [1, 2, 3].map((seq) => ({ seq, objective_id: `${id}-O${String(seq)}`, text: `objective ${id}` })) };
      }),
    }));
    const manifest = {
      curriculum_id: 'g9-ct9-480',
      version: '0.3.0',
      provenance: 'PROPOSED_SEED',
      structure: { weeks: 12, days_per_week: 6, objectives_per_day: 3, weekday_minutes: 420, saturday_minutes: 300, weekly_minutes: 2400, total_minutes: 28800 },
      weeks,
    };
    const payload = { manifest, provenance: 'PROPOSED_SEED' as const };
    const first = err(await client.invoke('curriculum.import', payload));
    expect(first.category).toBe('APPROVAL_REQUIRED');
    expect(first.details).toMatchObject({ required_capability: 'CURRICULUM_IMPORT' });
    const approvalId = first.approval_id ?? '';
    ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
    const done = ok(await client.invoke('curriculum.import', payload, { approvalId })).result;
    expect(done.curriculum_version_id).toBe('g9-ct9-480:0.3.0');
    expect(done.days).toBe(72);
    const consumed = ok(await client.invoke('approvals.list', {})).result.approvals.find((a) => a.approval_id === approvalId);
    expect(consumed?.state).toBe('CONSUMED');
  });

  it('emits the backend disclosure warnings: non-canonical daily plan; an approved export is READY and carries none', async () => {
    const { client } = harness();
    const plan = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' }));
    expect(plan.warnings).toEqual(['curriculum.non_canonical: plan derived from a version that is not APPROVED_CANONICAL']);
    const need = err(await client.invoke('export.request', { domains: ['LEARNING'], format: 'JSON', include_private: false }));
    ok(await client.invoke('approvals.decide', { approval_id: need.approval_id ?? '', decision: 'APPROVE' }));
    const exported = ok(await client.invoke('export.request', { domains: ['LEARNING'], format: 'JSON', include_private: false }, { approvalId: need.approval_id ?? '' }));
    expect(exported.result.state).toBe('READY');
    // `export.attachments_missing` is the only export warning; every attachment is in the mock store.
    expect(exported.warnings).toBeUndefined();
    const status = ok(await client.invoke('system.status', {}));
    expect(status.warnings).toBeUndefined();
  });
});

/** Approval-gated finalization of the fixture attempt `mock-attempt-prev` (GRADED_PROVISIONAL, level 3 everywhere). */
async function finalizePrev(client: ReturnType<typeof harness>['client']) {
  const fin = err(await client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' }));
  ok(await client.invoke('approvals.decide', { approval_id: fin.approval_id ?? '', decision: 'APPROVE' }));
  return ok(await client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' }, { approvalId: fin.approval_id ?? '' }));
}

const PREV_COMPETENCY = 'mock-competency-W2';

describe('mock transport: grade decision -> competency observations (mirrors g9-app approve_final)', () => {
  it('derives observations for a version that declares a difficulty: no warning, the projection changes', async () => {
    const { client } = harness();
    const before = ok(await client.invoke('competency.projection', { competency_id: PREV_COMPETENCY })).result.projections[0];
    expect(before).toMatchObject({ state: 'UNASSESSED', provisional: false, evidence_refs: [] });
    expect(before?.per_dimension.APPLICATION).toEqual({ score_estimate: null, n_unaided: 0, last_observed_at: null, uncertainty: null });

    const finalized = await finalizePrev(client);
    expect(finalized.result.state).toBe('APPROVED_FINAL');
    expect(finalized.result.grade?.reliability_class).toBe('SELF_ADJUDICATED');
    // no competency.observations_not_derived warning: the observations were derived
    expect(finalized.warnings).toBeUndefined();

    const after = ok(await client.invoke('competency.projection', { competency_id: PREV_COMPETENCY })).result.projections[0];
    // level 3 everywhere = 80 per criterion = 80 per dimension (understanding, application, transfer)
    expect(after?.evidence_refs).toHaveLength(3);
    expect(after?.per_dimension.RECALL.score_estimate).toBeNull();
    for (const d of ['UNDERSTANDING', 'APPLICATION', 'TRANSFER'] as const) {
      expect(after?.per_dimension[d]).toMatchObject({ score_estimate: 80, n_unaided: 1, uncertainty: 0 });
    }
    // One Application observation >= 60 reaches GUIDED; the operator graded their own attempt
    // (SELF_ADJUDICATED), so the 80 is validation-level evidence that is excluded: provisional.
    expect(after).toMatchObject({ state: 'GUIDED', provisional: true });
    // other competencies are untouched
    const all = ok(await client.invoke('competency.projection', {})).result.projections;
    expect(all.filter((p) => p.evidence_refs.length > 0).map((p) => p.competency_id)).toEqual([PREV_COMPETENCY]);
  });

  it('a failing decision derives observations that never validate or inflate the state', async () => {
    const { client } = harness();
    ok(await client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' }));
    const list = ok(await client.invoke('assessment.list', {})).result.attempts;
    const attempt = list.find((a) => a.assessment_id === 'mock-assessment-D18');
    expect(attempt).toBeDefined();
    const id = attempt?.attempt_id ?? '';
    let rev = attempt?.revision ?? 0;
    for (const op of ['assessment.submit', 'assessment.review_evidence'] as const) {
      const r = ok(await client.invoke(op, op === 'assessment.submit' ? { attempt_id: id, evidence_refs: [] } : { attempt_id: id }, { expectedRevision: rev }));
      rev = r.revision ?? rev;
    }
    const criteria = BASELINE_CRITERIA.map((c) => ({ criterion_code: c.code, score: 1 as const, source: 'HUMAN_REVIEWER' as const, evidence_refs: [] }));
    const graded = ok(await client.invoke('assessment.grade_provisional', { attempt_id: id, criteria }, { expectedRevision: rev }));
    expect(graded.result.grade?.pass).toBe(false);
    const gradedRevision = graded.revision ?? rev;
    const fin = err(await client.invoke('assessment.approve_final', { attempt_id: id }, { expectedRevision: gradedRevision }));
    ok(await client.invoke('approvals.decide', { approval_id: fin.approval_id ?? '', decision: 'APPROVE' }));
    const done = ok(await client.invoke('assessment.approve_final', { attempt_id: id }, { approvalId: fin.approval_id ?? '', expectedRevision: gradedRevision }));
    expect(done.warnings).toBeUndefined();
    const projected = ok(await client.invoke('competency.projection', { competency_id: 'mock-competency-D18' })).result.projections[0];
    // level 1 = 25 per criterion: introduced only, no estimate above the evidence, nothing validation-level
    expect(projected).toMatchObject({ state: 'INTRODUCED', provisional: false });
    expect(projected?.per_dimension.APPLICATION.score_estimate).toBe(25);
  });

  it('derives nothing and warns when the version declares no difficulty (never guessed)', async () => {
    const clock = createMockClock();
    const transport = createMockTransport({ clock, assessmentsWithoutDifficulty: ['mock-assessment-W2'] });
    let n = 0;
    const client = createClient(transport, {
      now: () => clock.now(),
      uuid: () => {
        n += 1;
        return `u${n}`;
      },
    });
    const finalized = await finalizePrev(client);
    expect(finalized.result.state).toBe('APPROVED_FINAL');
    expect(finalized.warnings).toHaveLength(1);
    expect(finalized.warnings?.[0]).toMatch(/^competency\.observations_not_derived: the assessment version declares no difficulty; /);
    expect(finalized.warnings?.[0]).not.toMatch(/wave-1/);
    const after = ok(await client.invoke('competency.projection', { competency_id: PREV_COMPETENCY })).result.projections[0];
    expect(after).toMatchObject({ state: 'UNASSESSED', provisional: false, evidence_refs: [] });
  });

  it('a competency nobody finalized an attempt for is unchanged: UNASSESSED, empty estimates', async () => {
    const { client } = harness();
    await finalizePrev(client);
    const untouched = ok(await client.invoke('competency.projection', { competency_id: 'mock-competency-D13' })).result.projections[0];
    expect(untouched).toMatchObject({ state: 'UNASSESSED', provisional: false, evidence_refs: [], contradictions: [] });
    for (const d of ['RECALL', 'UNDERSTANDING', 'APPLICATION', 'TRANSFER'] as const) {
      expect(untouched?.per_dimension[d]).toEqual({ score_estimate: null, n_unaided: 0, last_observed_at: null, uncertainty: null });
    }
  });
});

// ---------------------------------------------------------------------------
// Wave-2 operations: parity with g9-app (sessions.rs, calendar.rs, retention.rs, authoring.rs,
// evidence.rs, privacy.rs) and their feature tests in crates/g9-app/tests/feature_*.rs.
// ---------------------------------------------------------------------------

const HOUR = 3_600_000;
const DAY = 86_400_000;
const WEEK = { from: Date.UTC(2026, 10, 16), to: Date.UTC(2026, 10, 23) };
const utf8 = (text: string) => new TextEncoder().encode(text);

type Client = ReturnType<typeof harness>['client'];

/** Request, approve (native confirmation answered yes) and return the approval id. */
async function approvalFor(client: Client, refused: InvokeResult<OperationName>): Promise<string> {
  const need = err(refused);
  expect(need.category).toBe('APPROVAL_REQUIRED');
  const approvalId = need.approval_id ?? '';
  ok(await client.invoke('approvals.decide', { approval_id: approvalId, decision: 'APPROVE' }));
  return approvalId;
}

async function approvalState(client: Client, approvalId: string) {
  return ok(await client.invoke('approvals.list', {})).result.approvals.find((a) => a.approval_id === approvalId);
}

describe('mock transport: session.create', () => {
  it('plans an UNLINKED session that runs like any other and is listed on the date of its planned start', async () => {
    const { client, clock } = harness();
    const created = ok(await client.invoke('session.create', { planned_minutes: 45 }));
    expect(created.result).toMatchObject({ state: 'PLANNED', day_id: 'UNLINKED', block: 'UNLINKED', block_seq: 1, planned_ms: 45 * 60_000, credited_ms: 0, revision: 1 });
    expect(created.revision).toBe(1);
    const id = created.result.session_id;
    ok(await client.invoke('session.prepare', { session_id: id }));
    ok(await client.invoke('session.start', { session_id: id }));
    clock.advance(20 * 60_000);
    ok(await client.invoke('session.complete', { session_id: id }));
    const listed = ok(await client.invoke('session.list', {})).result.sessions.find((s) => s.session_id === id);
    expect(listed).toMatchObject({ state: 'COMPLETED', credited_ms: 20 * 60_000 });

    const tomorrow = ok(
      await client.invoke('session.create', { planned_minutes: 30, planned_start: Date.UTC(2026, 10, 19, 9, 0), mode: 'REVIEW', objective_id: 'D15-O1' }),
    ).result;
    expect(ok(await client.invoke('session.list', { date: '2026-11-19' })).result.sessions.map((s) => s.session_id)).toContain(tomorrow.session_id);
    expect(ok(await client.invoke('session.list', { date: '2026-11-18' })).result.sessions.map((s) => s.session_id)).not.toContain(tomorrow.session_id);
  });

  it('refuses EXAM mode and out-of-range minutes at the boundary and an unknown objective as NOT_FOUND', async () => {
    const { client } = harness();
    const exam = { planned_minutes: 30, mode: 'EXAM' } as unknown as RequestOf<'session.create'>;
    expect(err(await client.invoke('session.create', exam)).category).toBe('INVALID_INPUT');
    // `planned_start` may be at most 2100-01-01T00:00:00Z (4102444800000), as the request schema says.
    expect(ok(await client.invoke('session.create', { planned_minutes: 30, planned_start: 4_102_444_800_000 })).result.state).toBe('PLANNED');
    const late = { planned_minutes: 30, planned_start: 4_102_444_800_001 };
    expect(err(await client.invoke('session.create', late)).category).toBe('INVALID_INPUT');
    expect(err(await client.invoke('session.create', { planned_minutes: 601 })).category).toBe('INVALID_INPUT');
    expect(err(await client.invoke('session.create', { planned_minutes: 0 })).category).toBe('INVALID_INPUT');
    expect(err(await client.invoke('session.create', { planned_minutes: 30, objective_id: 'NO-SUCH-OBJECTIVE' }))).toMatchObject({
      category: 'NOT_FOUND',
      code: 'objective.not_found',
    });
  });
});

describe('mock transport: calendar events the operator adds', () => {
  it('creates, moves and lists an OPERATOR event; a stale expected revision is refused', async () => {
    const { client } = harness();
    const start = Date.UTC(2026, 10, 19, 9, 0);
    const created = ok(await client.invoke('calendar.create_event', { title: '  Dentist ', kind: 'APPOINTMENT', start, end: start + HOUR }));
    expect(created.result).toMatchObject({ title: 'Dentist', kind: 'APPOINTMENT', provenance: 'OPERATOR', immutable: false, all_day: false, adjusted: null, day_id: null });
    expect(created.revision).toBe(1);
    const id = created.result.event_id;
    expect(ok(await client.invoke('calendar.list', WEEK)).result.events.some((e) => e.event_id === id)).toBe(true);
    const moved = ok(await client.invoke('calendar.update_event', { event_id: id, title: 'Dentist (moved)', start: start + 2 * HOUR, end: start + 3 * HOUR }, { expectedRevision: 1 }));
    expect(moved.result).toMatchObject({ title: 'Dentist (moved)', kind: 'APPOINTMENT', start: start + 2 * HOUR, end: start + 3 * HOUR });
    expect(moved.revision).toBe(2);
    expect(err(await client.invoke('calendar.update_event', { event_id: id, title: 'again' }, { expectedRevision: 1 })).category).toBe('STALE_REVISION');
    expect(err(await client.invoke('calendar.update_event', { event_id: id, start: start + 4 * HOUR })).code).toBe('calendar.event_range_invalid');
  });

  it('deletes an added event only with a DATA_DELETE approval bound to its stored revision, consumed once', async () => {
    const { client } = harness();
    const start = Date.UTC(2026, 10, 20, 14, 0);
    const id = ok(await client.invoke('calendar.create_event', { title: 'Temporary', kind: 'REVIEW', start, end: start + HOUR })).result.event_id;
    // A request from a stale view gets no approval at all.
    expect(err(await client.invoke('calendar.delete_event', { event_id: id }, { expectedRevision: 3 })).category).toBe('STALE_REVISION');
    expect(ok(await client.invoke('approvals.list', { state: 'PENDING' })).result.approvals).toHaveLength(0);
    const first = await client.invoke('calendar.delete_event', { event_id: id }, { expectedRevision: 1 });
    expect(err(first).details).toMatchObject({ required_capability: 'DATA_DELETE' });
    const approvalId = await approvalFor(client, first);
    expect(await approvalState(client, approvalId)).toMatchObject({
      operation_name: 'calendar.delete_event',
      expected_revision: 1,
      operation: { action: 'DELETE', resource: { kind: 'CALENDAR_EVENT', id } },
      summary: `calendar.delete_event: CALENDAR_EVENT ${id} (requires DATA_DELETE)`,
    });
    const deleted = ok(await client.invoke('calendar.delete_event', { event_id: id }, { approvalId, expectedRevision: 1 }));
    expect(deleted.result).toEqual({ event_id: id, deleted: true });
    expect(deleted.revision).toBeUndefined();
    expect(ok(await client.invoke('calendar.list', WEEK)).result.events.some((e) => e.event_id === id)).toBe(false);
    expect((await approvalState(client, approvalId))?.state).toBe('CONSUMED');
  });

  it('refuses an approval when the event changed between grant and use, and leaves it unspent', async () => {
    const { client } = harness();
    const start = Date.UTC(2026, 10, 20, 16, 0);
    const id = ok(await client.invoke('calendar.create_event', { title: 'Moving target', kind: 'STUDY_BLOCK', start, end: start + HOUR })).result.event_id;
    const approvalId = await approvalFor(client, await client.invoke('calendar.delete_event', { event_id: id }));
    ok(await client.invoke('calendar.update_event', { event_id: id, title: 'Moved after the grant' }));
    const refused = err(await client.invoke('calendar.delete_event', { event_id: id }, { approvalId }));
    expect(refused).toMatchObject({ category: 'STALE_REVISION', code: 'approval.revision_mismatch', details: { expected: 2, bound: 1 } });
    expect((await approvalState(client, approvalId))?.state).toBe('APPROVED');
  });

  it('refuses invalid ranges and leaves curriculum, scheduler and immutable events alone, as calendar.rs does', async () => {
    const { client } = harness();
    const start = Date.UTC(2026, 10, 19, 9, 0);
    expect(err(await client.invoke('calendar.create_event', { title: 'x', kind: 'REVIEW', start, end: start - 1 }))).toMatchObject({
      category: 'INVALID_INPUT',
      code: 'calendar.event_range_invalid',
    });
    expect(err(await client.invoke('calendar.create_event', { title: 'x', kind: 'REVIEW', start, end: start + 8 * DAY })).code).toBe('calendar.event_too_long');
    expect(err(await client.invoke('calendar.update_event', { event_id: 'mock-event-missing', title: 'x' })).code).toBe('calendar.event_not_found');
    // A curriculum study block (SYSTEM_DETERMINISTIC) is not editable: g9-app refuses it before policy,
    // so a deletion is never put to an approval that could not be used.
    const block = 'mock-event-D15-2';
    expect(err(await client.invoke('calendar.update_event', { event_id: block, title: 'mine now' })).code).toBe('calendar.event_not_editable');
    expect(err(await client.invoke('calendar.delete_event', { event_id: block }))).toMatchObject({ category: 'PRECONDITION_FAILED', code: 'calendar.event_not_editable' });
    expect(ok(await client.invoke('approvals.list', { state: 'PENDING' })).result.approvals.some((a) => a.operation_name === 'calendar.delete_event')).toBe(false);
    // Immutable slots (protected rest, the assessment block) are refused the same way.
    expect(err(await client.invoke('calendar.delete_event', { event_id: 'mock-event-rest-sunday' }))).toMatchObject({ category: 'PRECONDITION_FAILED', code: 'calendar.event_not_editable' });
    expect(err(await client.invoke('calendar.update_event', { event_id: 'mock-event-D18-1', title: 'x' })).code).toBe('calendar.event_not_editable');
    // Events a schedule proposal created come from the scheduler: listed SYSTEM_DETERMINISTIC.
    const proposal = ok(await client.invoke('calendar.propose', { from: MOCK_DEFAULT_NOW, to: MOCK_DEFAULT_NOW + 7 * DAY, intent: 'ADD_REVIEWS' })).result;
    const applyId = await approvalFor(client, await client.invoke('calendar.apply', { proposal_id: proposal.proposal_id }));
    const applied = ok(await client.invoke('calendar.apply', { proposal_id: proposal.proposal_id }, { approvalId: applyId })).result.events;
    const listed = ok(await client.invoke('calendar.list', { from: MOCK_DEFAULT_NOW, to: MOCK_DEFAULT_NOW + 7 * DAY })).result.events;
    const created = listed.filter((e) => applied.some((a) => a.event_id === e.event_id));
    expect(created.length).toBeGreaterThan(0);
    expect(created.every((e) => e.provenance === 'SYSTEM_DETERMINISTIC')).toBe(true);
  });
});

describe('mock transport: review items and the baseline ladder (g9-retention BaselineSchedule)', () => {
  it('a new item is due a day later at stage 0; successes climb 1, 3, 7 days; a failure resets; a skip moves nothing', async () => {
    const { client, clock } = harness();
    const created = ok(await client.invoke('retention.create_item', { prompt: ' What does HSTS prevent? ', answer: 'Protocol downgrade to plain HTTP.' }));
    expect(created.result).toMatchObject({
      prompt: 'What does HSTS prevent?',
      kind: 'DECLARATIVE',
      due_at: MOCK_DEFAULT_NOW + DAY,
      offset_days: 1,
      last_success_at: null,
      competency_id: null,
      objective_id: null,
      estimated_ms: 180_000,
    });
    const id = created.result.review_item_id;
    const before = ok(await client.invoke('retention.due', {})).result;
    expect([...before.overdue, ...before.due, ...before.overflow].some((i) => i.review_item_id === id)).toBe(false);
    clock.advance(DAY);
    const due = ok(await client.invoke('retention.due', {})).result;
    expect([...due.overdue, ...due.due, ...due.overflow].some((i) => i.review_item_id === id)).toBe(true);

    const record = async (outcome: 'SUCCESS' | 'FAILURE' | 'SKIPPED') => ok(await client.invoke('retention.record_review', { review_item_id: id, outcome }));
    const first = await record('SUCCESS');
    expect(first.result).toEqual({ review_item_id: id, outcome: 'SUCCESS', stage: 1, offset_days: 1, due_at: clock.now() + DAY });
    clock.advance(DAY);
    expect((await record('SUCCESS')).result).toMatchObject({ stage: 2, offset_days: 3, due_at: clock.now() + 3 * DAY });
    clock.advance(3 * DAY);
    expect((await record('SUCCESS')).result).toMatchObject({ stage: 3, offset_days: 7, due_at: clock.now() + 7 * DAY });
    const failed = await record('FAILURE');
    expect(failed.result).toMatchObject({ stage: 0, offset_days: 1, due_at: clock.now() + DAY });
    clock.advance(HOUR);
    const skipped = await record('SKIPPED');
    expect(skipped.result).toEqual({ review_item_id: id, outcome: 'SKIPPED', stage: 0, offset_days: 1, due_at: failed.result.due_at });
    expect(skipped.revision).toBe(failed.revision);
  });

  it('records a fixture item from the due queue and moves it out of the queue', async () => {
    const { client } = harness();
    const before = ok(await client.invoke('retention.due', {})).result;
    const item = before.due[0];
    if (item === undefined) {
      throw new Error('fixture has nothing due today');
    }
    const recorded = ok(await client.invoke('retention.record_review', { review_item_id: item.review_item_id, outcome: 'SUCCESS' })).result;
    expect(recorded).toMatchObject({ stage: 1, offset_days: 1, due_at: MOCK_DEFAULT_NOW + DAY });
    const after = ok(await client.invoke('retention.due', {})).result;
    expect([...after.overdue, ...after.due, ...after.overflow].some((i) => i.review_item_id === item.review_item_id)).toBe(false);
    // The overflow item moves into the budget.
    expect(after.overflow).toHaveLength(0);
  });

  it('lists a PROCEDURAL item as PRACTICAL_TRANSFER with the backend warning; unknown items and competencies are NOT_FOUND', async () => {
    const { client, clock } = harness();
    const procedural = ok(await client.invoke('retention.create_item', { prompt: 'Rotate the TLS key', kind: 'PROCEDURAL', competency_id: 'mock-competency-D13' })).result;
    expect(procedural).toMatchObject({ kind: 'PRACTICAL_TRANSFER', competency_id: 'mock-competency-D13' });
    clock.advance(DAY);
    const due = ok(await client.invoke('retention.due', {}));
    expect(due.warnings).toEqual([expect.stringMatching(/^retention\.due\.procedural_label: 1 PROCEDURAL review item\(s\) /)]);
    expect(err(await client.invoke('retention.record_review', { review_item_id: 'mock-review-missing', outcome: 'SUCCESS' })).code).toBe('retention.item_not_found');
    expect(err(await client.invoke('retention.create_item', { prompt: 'x', competency_id: 'comp-missing' })).code).toBe('competency.not_found');
  });
});

describe('mock transport: authoring (competency.create, assessment.define)', () => {
  it('authors a competency and an assessment usable end to end; codes are unique; an unknown competency is NOT_FOUND', async () => {
    const { client } = harness();
    const comp = ok(await client.invoke('competency.create', { code: 'WCP', title: ' Web cache poisoning ', validity_days: 120 })).result;
    expect(comp).toMatchObject({ code: 'WCP', title: 'Web cache poisoning', validity_days: 120 });
    // A typed conflict, answered before storage: no table or constraint text (app-6).
    const duplicate = err(await client.invoke('competency.create', { code: 'WCP', title: 'again' }));
    expect(duplicate).toMatchObject({ category: 'CONFLICT', code: 'competency.code_exists' });
    expect(JSON.stringify(duplicate)).not.toMatch(/constraint|UNIQUE|competencies/);
    // A blank title is a name that is not storable: refused at the boundary, before the handler.
    expect(err(await client.invoke('competency.create', { code: 'BLANK', title: '   ' }))).toMatchObject({ code: 'input.name_not_printable', details: { field: 'title' } });
    expect(err(await client.invoke('competency.create', { code: 'lower', title: 'x' })).category).toBe('INVALID_INPUT');
    expect(ok(await client.invoke('competency.create', { code: 'DEF', title: 'Default validity' })).result.validity_days).toBe(180);
    const listedComp = ok(await client.invoke('competency.projection', { competency_id: comp.competency_id })).result.projections[0];
    expect(listedComp).toMatchObject({ name: 'Web cache poisoning', state: 'UNASSESSED', evidence_refs: [] });

    const definition = {
      code: 'WCP-01',
      title: 'Poison a cache key',
      kind: 'BLIND_PRACTICAL' as const,
      competency_id: comp.competency_id,
      task_statement: 'Find an unkeyed header that changes the cached response.',
      difficulty: 4 as const,
    };
    expect(err(await client.invoke('assessment.define', { ...definition, competency_id: 'comp-missing' }))).toMatchObject({
      category: 'NOT_FOUND',
      code: 'competency.not_found',
    });
    const defined = ok(await client.invoke('assessment.define', definition)).result;
    expect(defined).toMatchObject({ code: 'WCP-01', title: 'Poison a cache key', kind: 'BLIND_PRACTICAL', competency_id: comp.competency_id, difficulty: 4 });
    expect(defined.version_id).toBe(`${defined.assessment_id}:v1`);
    expect(defined.criteria).toEqual(BASELINE_CRITERIA.map((c) => c.code));
    expect(err(await client.invoke('assessment.define', definition))).toMatchObject({ category: 'CONFLICT', code: 'assessment.code_exists' });
    const listed = ok(await client.invoke('assessment.list', {})).result.definitions.find((d) => d.assessment_id === defined.assessment_id);
    expect(listed).toEqual({ assessment_id: defined.assessment_id, title: 'Poison a cache key', day_id: 'WCP-01', mode: 'EXAM' });

    // The finalized attempt feeds the authored competency with the declared difficulty (no warning).
    const attempt = ok(await client.invoke('assessment.begin', { assessment_id: defined.assessment_id })).result.attempt_id;
    ok(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: [] }));
    ok(await client.invoke('assessment.review_evidence', { attempt_id: attempt }));
    const criteria = BASELINE_CRITERIA.map((c) => ({ criterion_code: c.code, score: 4 as const, source: 'HUMAN_REVIEWER' as const, evidence_refs: [] }));
    ok(await client.invoke('assessment.grade_provisional', { attempt_id: attempt, criteria }));
    const finalId = await approvalFor(client, await client.invoke('assessment.approve_final', { attempt_id: attempt }));
    const final = ok(await client.invoke('assessment.approve_final', { attempt_id: attempt }, { approvalId: finalId }));
    expect(final.warnings).toBeUndefined();
    const projected = ok(await client.invoke('competency.projection', { competency_id: comp.competency_id })).result.projections[0];
    expect(projected).toMatchObject({ state: 'GUIDED', provisional: true });
    expect(projected?.per_dimension.APPLICATION).toMatchObject({ score_estimate: 100, n_unaided: 1 });
  });
});

describe('mock transport: evidence (attach_file, from_note, list)', () => {
  it('stores bytes by content (identical bytes once), lists a note\'s evidence oldest first and the newest items first', async () => {
    const { client } = harness();
    const note = ok(await client.invoke('notes.create', { title: 'Cache poisoning lab', body_markdown: 'Lab write-up.', classification: 'INTERNAL', domain: 'LEARNING' })).result;
    const bytes = utf8('# Findings\n\nX-Forwarded-Host is unkeyed.\n');
    const payload = { note_id: note.note_id, filename: 'findings.md', media_type: 'text/markdown' as const, content_base64: encodeBase64(bytes) };
    const first = ok(await client.invoke('evidence.attach_file', payload)).result;
    expect(first).toMatchObject({ note_id: note.note_id, filename: 'findings.md', media_type: 'text/markdown', size_bytes: bytes.length, sha256: sha256HexBytes(bytes), already_stored: false });
    const again = ok(await client.invoke('evidence.attach_file', { ...payload, filename: ' copy.md ' })).result;
    expect(again).toMatchObject({ filename: 'copy.md', sha256: first.sha256, already_stored: true });
    expect(again.attachment_id).not.toBe(first.attachment_id);
    const fromNote = ok(await client.invoke('evidence.from_note', { note_id: note.note_id })).result;
    expect(fromNote).toMatchObject({ kind: 'NOTE', note_id: note.note_id, summary: 'Cache poisoning lab' });

    const listed = ok(await client.invoke('evidence.list', { note_id: note.note_id })).result.items;
    expect(listed.map((i) => [i.kind, i.filename])).toEqual([
      ['ATTACHMENT', 'findings.md'],
      ['ATTACHMENT', 'copy.md'],
      ['NOTE', null],
    ]);
    expect(listed[0]).toMatchObject({ evidence_id: first.evidence_id, attachment_id: first.attachment_id, note_id: null, media_type: 'text/markdown', size_bytes: bytes.length, invalidated: false });
    expect(listed[2]).toMatchObject({ evidence_id: fromNote.evidence_id, note_id: note.note_id, attachment_id: null, sha256: sha256HexBytes(utf8('Lab write-up.')) });
    const recent = ok(await client.invoke('evidence.list', { limit: 2 })).result.items;
    expect(recent.map((i) => i.evidence_id)).toEqual([fromNote.evidence_id, again.evidence_id]);
    // An item of another domain's scope is not listed; an unknown note lists nothing.
    expect(ok(await client.invoke('evidence.list', {}, { domain: 'BUSINESS' })).result.items).toHaveLength(0);
    expect(ok(await client.invoke('evidence.list', { note_id: 'mock-note-missing' })).result.items).toHaveLength(0);

    // The evidence goes into a submission; an id that names no evidence item is NOT_FOUND (app-6).
    const attempt = ok(await client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' })).result.attempt_id;
    expect(err(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: ['mock-evidence-missing'] }))).toMatchObject({
      category: 'NOT_FOUND',
      code: 'evidence.not_found',
      details: { evidence_id: 'mock-evidence-missing' },
    });
    expect(ok(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: [first.evidence_id, fromNote.evidence_id] })).result.state).toBe('SUBMITTED');
  });

  it('refuses executables, archives, mismatched types, bad base64 and file names, and oversized payloads; nothing is stored', async () => {
    const { client } = harness();
    const attach = async (filename: string, mediaType: RequestOf<'evidence.attach_file'>['media_type'], content: string) =>
      err(await client.invoke('evidence.attach_file', { filename, media_type: mediaType, content_base64: content }));
    const b64 = (bytes: number[] | string) => encodeBase64(typeof bytes === 'string' ? utf8(bytes) : new Uint8Array(bytes));
    expect(await attach('tool', 'text/plain', b64([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1, 0, 0x70]))).toMatchObject({
      category: 'INVALID_INPUT',
      code: 'attachment.refused',
      message: 'ELF executable: executables, archives and packet captures are not stored; attach a written report instead',
    });
    expect((await attach('a.csv', 'text/csv', b64('PK\u0003\u0004rest'))).code).toBe('attachment.refused');
    expect((await attach('a.json', 'application/json', b64([0x1f, 0x8b, 0x08]))).code).toBe('attachment.refused');
    expect((await attach('cap.png', 'image/png', b64([0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0]))).code).toBe('attachment.refused');
    expect((await attach('report.md', 'text/markdown', b64('MZ is not a heading'))).code).toBe('attachment.refused');
    expect(await attach('shot.png', 'image/png', b64('not a png at all'))).toMatchObject({
      code: 'attachment.content_mismatch',
      message: "the file's bytes do not match image/png; quarantine is not stored yet (PROPOSED), so it was not attached",
    });
    expect((await attach('scan.pdf', 'application/pdf', b64([0xff, 0xd8, 0xff, 0xe0]))).code).toBe('attachment.content_mismatch');
    expect((await attach('a.json', 'application/json', b64('{not json'))).code).toBe('attachment.content_mismatch');
    expect((await attach('bom.json', 'application/json', b64('﻿{}'))).code).toBe('attachment.content_mismatch');
    expect((await attach('nul.txt', 'text/plain', b64('abc\u0000def'))).code).toBe('attachment.content_mismatch');
    expect((await attach('latin1.txt', 'text/plain', b64([0x63, 0x61, 0x66, 0xe9]))).code).toBe('attachment.content_mismatch');
    for (const bad of ['%%%%', 'aGkx=', 'aGl=', 'aG k=', 'aGk=aGk=']) {
      expect((await attach('a.txt', 'text/plain', bad)).code, bad).toBe('attachment.content_invalid');
    }
    // Shorter than the request schema's minimum (4): refused at step 1.
    expect((await attach('a.txt', 'text/plain', 'aGk')).code).toBe('ipc.payload_invalid');
    for (const name of ['../../etc/passwd', 'a\\b', '..', '   ', 'tab\there', 'x'.repeat(256)]) {
      // Blank names and names with a control character are refused by the name rule (before the handler).
      const code = name === 'x'.repeat(256) ? 'ipc.payload_invalid' : name === '   ' || name === 'tab\there' ? 'input.name_not_printable' : 'attachment.filename_invalid';
      expect((await attach(name, 'text/plain', b64('x'))).code, name).toBe(code);
    }
    // 255 characters but more than 255 UTF-8 bytes.
    expect((await attach('é'.repeat(200), 'text/plain', b64('x'))).code).toBe('attachment.filename_invalid');
    // The schema allows ceil(16 MiB / 3) * 4 characters: unpadded, that decodes to 16 MiB + 2 bytes.
    const atSchemaLimit = 'A'.repeat(Math.ceil(MAX_ATTACHMENT_BYTES / 3) * 4);
    expect(await attach('big.txt', 'text/plain', atSchemaLimit)).toMatchObject({
      code: 'attachment.too_large',
      message: `a file attached through the app is at most ${MAX_ATTACHMENT_BYTES} bytes`,
    });
    expect((await attach('big.txt', 'text/plain', `${atSchemaLimit}AAAA`)).code).toBe('ipc.payload_invalid');
    expect(ok(await client.invoke('evidence.list', {})).result.items).toHaveLength(0);
    // The same bytes with a matching type are accepted and were not "already stored".
    expect(ok(await client.invoke('evidence.attach_file', { filename: 'ok.txt', media_type: 'text/plain', content_base64: b64('plain bytes') })).result.already_stored).toBe(false);
  });

  it('a file on a SECURITY_SENSITIVE note needs a PROTECTED_RECORD_WRITE approval in that domain, granted for the attachment id', async () => {
    const { client } = harness();
    const scope = { domain: 'SECURITY_SENSITIVE' as const };
    const notePayload = { title: 'Scope letter', body_markdown: 'Authorized targets.', classification: 'SECRET' as const, domain: 'SECURITY_SENSITIVE' as const };
    const needNote = await client.invoke('notes.create', notePayload, scope);
    expect(err(needNote).details).toMatchObject({ required_capability: 'PROTECTED_RECORD_WRITE' });
    const noteApproval = await approvalFor(client, needNote);
    const note = ok(await client.invoke('notes.create', notePayload, { ...scope, approvalId: noteApproval })).result;
    expect(note.note_id).toBe((await approvalState(client, noteApproval))?.operation.resource.id);

    const payload = { note_id: note.note_id, filename: 'letter.pdf', media_type: 'application/pdf' as const, content_base64: encodeBase64(utf8('%PDF-1.7\n...')) };
    // The same bytes attached in the LEARNING scope are not "already stored" for another domain's scope.
    expect(ok(await client.invoke('evidence.attach_file', { filename: 'copy.pdf', media_type: 'application/pdf', content_base64: payload.content_base64 })).result.already_stored).toBe(false);
    // Read in the LEARNING scope, the note is a refused cross-domain read.
    expect(err(await client.invoke('evidence.attach_file', payload))).toMatchObject({ category: 'UNAUTHORIZED', code: 'storage.cross_domain_read' });
    const first = await client.invoke('evidence.attach_file', payload, scope);
    expect(err(first).details).toMatchObject({ required_capability: 'PROTECTED_RECORD_WRITE' });
    const approvalId = await approvalFor(client, first);
    const attached = ok(await client.invoke('evidence.attach_file', payload, { ...scope, approvalId })).result;
    expect(attached).toMatchObject({ note_id: note.note_id, already_stored: false });
    const approval = await approvalState(client, approvalId);
    expect(approval?.operation.resource).toEqual({ kind: 'ATTACHMENT', id: attached.attachment_id });
    expect(approval?.state).toBe('CONSUMED');
    expect(ok(await client.invoke('evidence.list', { note_id: note.note_id }, scope)).result.items.map((i) => i.attachment_id)).toEqual([attached.attachment_id]);
    // Making the note evidence needs the same approval in that domain.
    expect(err(await client.invoke('evidence.from_note', { note_id: note.note_id }, scope)).category).toBe('APPROVAL_REQUIRED');
  });

  it('a title, file name or label that is blank or holds a hidden character is refused before anything is stored', async () => {
    const { client } = harness();
    const hostile = ['   ', 'pin\u202Etxt.exe', 'first line\nApprove: nothing', 'zero\u200Bwidth'];
    for (const title of hostile) {
      const refused = err(await client.invoke('notes.create', { title, body_markdown: 'b', classification: 'INTERNAL', domain: 'LEARNING' }));
      expect(refused, JSON.stringify(title)).toMatchObject({ category: 'INVALID_INPUT', code: 'input.name_not_printable', details: { field: 'title' } });
      // The refusal names the field and the code point, never the text.
      expect(JSON.stringify(refused)).not.toContain('Approve: nothing');
      expect(err(await client.invoke('evidence.attach_file', { filename: title, media_type: 'text/plain', content_base64: encodeBase64(utf8('x')) }))).toMatchObject({
        code: 'input.name_not_printable',
        details: { field: 'filename' },
      });
    }
    expect(ok(await client.invoke('evidence.list', {})).result.items).toEqual([]);
    // Free text keeps its line breaks.
    expect(ok(await client.invoke('notes.create', { title: 'Lab notes', body_markdown: 'one\n\n\ttwo', classification: 'INTERNAL', domain: 'LEARNING' })).result.note_id).toBeTruthy();
  });

  it('a file takes the stricter of its own and its note\'s classification, which an export then honours', async () => {
    const { client } = harness();
    const note = ok(await client.invoke('notes.create', { title: 'Plain', body_markdown: 'b', classification: 'INTERNAL', domain: 'LEARNING' })).result;
    const attach = async (name: string, classification?: 'PUBLIC' | 'PRIVATE' | 'SECRET') => {
      const payload = { note_id: note.note_id, filename: name, media_type: 'text/plain' as const, content_base64: encodeBase64(utf8(name)), ...(classification === undefined ? {} : { classification }) };
      return ok(await client.invoke('evidence.attach_file', payload)).result.attachment_id;
    };
    await attach('secret.txt', 'SECRET'); // stricter than the note: kept
    await attach('private.txt', 'PRIVATE'); // stricter than the note: kept
    await attach('public.txt', 'PUBLIC'); // weaker than the note: the note's INTERNAL
    await attach('plain.txt'); // not given: the note's INTERNAL
    const exportOf = async (include_private: boolean) => {
      const payload: RequestOf<'export.request'> = { domains: ['LEARNING'], format: 'JSON', include_private };
      const approvalId = await approvalFor(client, await client.invoke('export.request', payload));
      return ok(await client.invoke('export.request', payload, { approvalId })).result;
    };
    // Two INTERNAL files leave in a plain export; the PRIVATE one only on request; the SECRET one never.
    expect((await exportOf(false)).attachments).toBe(2);
    expect((await exportOf(true)).attachments).toBe(3);
  });

  it('an erased note takes no attachment; its files and evidence items are erased and invalidated', async () => {
    const { client } = harness();
    const note = ok(await client.invoke('notes.create', { title: 'Diary', body_markdown: 'private', classification: 'PRIVATE', domain: 'LEARNING' })).result;
    const payload = { note_id: note.note_id, filename: 'day.txt', media_type: 'text/plain' as const, content_base64: encodeBase64(utf8('dear diary')) };
    ok(await client.invoke('evidence.attach_file', payload));
    ok(await client.invoke('evidence.from_note', { note_id: note.note_id }));
    const erase = { resource: { kind: 'NOTE' as const, id: note.note_id }, reason: 'no longer needed' };
    const deletion = ok(await client.invoke('deletion.request', erase, { approvalId: await approvalFor(client, await client.invoke('deletion.request', erase)) }));
    expect(deletion.result.state).toBe('APPLIED');
    // The file leaves the attachment folder inside the erasure, so there is nothing to warn about,
    // and the tombstone's digest is not the digest of the erased body (ADR-0017).
    expect(deletion.warnings ?? []).toEqual([]);
    expect(deletion.result.tombstone_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(deletion.result.tombstone_sha256).not.toBe(sha256HexBytes(utf8('private')));
    expect(err(await client.invoke('evidence.attach_file', payload))).toMatchObject({ category: 'CONFLICT', code: 'note.erased' });
    expect(err(await client.invoke('evidence.from_note', { note_id: note.note_id }))).toMatchObject({ category: 'CONFLICT', code: 'note.erased' });
    const items = ok(await client.invoke('evidence.list', { note_id: note.note_id })).result.items;
    expect(items).toHaveLength(2);
    expect(items.every((i) => i.invalidated)).toBe(true);
    expect(items.map((i) => i.filename)).toEqual([null, null]);
    // The summaries repeated the file name and the note title: redacted with the items.
    expect(items.map((i) => i.summary)).toEqual(['[REDACTED]', '[REDACTED]']);
    expect(err(await client.invoke('evidence.attach_file', { ...payload, note_id: 'mock-note-missing' }))).toMatchObject({ category: 'NOT_FOUND', code: 'note.not_found' });
    // `already_stored` consults non-erased attachments only: it never reveals erased content.
    const unowned = { filename: payload.filename, media_type: payload.media_type, content_base64: payload.content_base64 };
    expect(ok(await client.invoke('evidence.attach_file', unowned)).result.already_stored).toBe(false);
  });

  it('rebuilds an attach_file or from_note replay from the current rows (names and titles are not retained) and says so', async () => {
    const { client } = harness();
    const note = ok(await client.invoke('notes.create', { title: '  Lab log  ', body_markdown: 'b', classification: 'INTERNAL', domain: 'LEARNING' })).result;
    const payload = { note_id: note.note_id, filename: 'log.txt', media_type: 'text/plain' as const, content_base64: encodeBase64(utf8('log line')) };
    const first = ok(await client.invoke('evidence.attach_file', payload, { idempotencyKey: 'attach-once' }));
    expect(first.warnings).toBeUndefined();
    const again = ok(await client.invoke('evidence.attach_file', payload, { idempotencyKey: 'attach-once' }));
    expect(again).toMatchObject({ replayed: true, result: first.result });
    expect(again.warnings).toEqual([expect.stringMatching(/^idempotency\.replay_rebuilt: names and titles are not retained/)]);
    // evidence.rs `summary_of`: the line is trimmed.
    const made = ok(await client.invoke('evidence.from_note', { note_id: note.note_id }, { idempotencyKey: 'note-once' }));
    expect(made.result.summary).toBe('Lab log');
    const madeAgain = ok(await client.invoke('evidence.from_note', { note_id: note.note_id }, { idempotencyKey: 'note-once' }));
    expect(madeAgain).toMatchObject({ replayed: true, result: made.result });
    expect(madeAgain.warnings).toHaveLength(1);
    // A blank title can no longer be created (input.name_not_printable); a row stored before that
    // rule can still hold one, or control characters, and the summary stays one printable line.
    expect(err(await client.invoke('notes.create', { title: ' ', body_markdown: 'b', classification: 'INTERNAL', domain: 'LEARNING' })).code).toBe('input.name_not_printable');
    expect(evidenceSummary(' ')).toBe('Note');
    expect(evidenceSummary('  Lab\u0007 log\n')).toBe('Lab  log');
    // After the note is erased, the rebuilt replay shows the redacted file name.
    const erase = { resource: { kind: 'NOTE' as const, id: note.note_id }, reason: 'erase' };
    ok(await client.invoke('deletion.request', erase, { approvalId: await approvalFor(client, await client.invoke('deletion.request', erase)) }));
    expect(ok(await client.invoke('evidence.attach_file', payload, { idempotencyKey: 'attach-once' })).result.filename).toBe('[REDACTED]');
    expect(ok(await client.invoke('evidence.from_note', { note_id: note.note_id }, { idempotencyKey: 'note-once' })).result.summary).toBe('[REDACTED]');
  });

  it('without an attachment directory nothing is attached', async () => {
    const { client } = harness({ attachmentDir: null });
    const refused = err(await client.invoke('evidence.attach_file', { filename: 'a.txt', media_type: 'text/plain', content_base64: encodeBase64(utf8('x')) }));
    expect(refused).toMatchObject({ category: 'PRECONDITION_FAILED', code: 'attachment.store_not_configured' });
  });
});

describe('mock transport: export.request writes the export after approval', () => {
  async function exported(client: Client, payload: RequestOf<'export.request'>) {
    const approvalId = await approvalFor(client, await client.invoke('export.request', payload));
    return ok(await client.invoke('export.request', payload, { approvalId }));
  }

  it('answers READY with the folder, counts and manifest hash; SECRET rows never, PRIVATE rows only with include_private', async () => {
    const { client } = harness();
    const shared = ok(await client.invoke('notes.create', { title: 'Shared lab', body_markdown: 'b', classification: 'INTERNAL', domain: 'LEARNING' })).result;
    ok(await client.invoke('notes.create', { title: 'Private diary', body_markdown: 'b', classification: 'PRIVATE', domain: 'LEARNING' }));
    ok(await client.invoke('notes.create', { title: 'Key material', body_markdown: 'b', classification: 'SECRET', domain: 'LEARNING' }));
    ok(await client.invoke('evidence.attach_file', { note_id: shared.note_id, filename: 'summary.txt', media_type: 'text/plain', content_base64: encodeBase64(utf8('capture summary')) }));

    const plain = await exported(client, { domains: ['LEARNING'], format: 'JSON', include_private: false });
    expect(plain.result).toMatchObject({ state: 'READY', domains: ['LEARNING'], format: 'JSON', attachments: 1, files: 3, requested_at: MOCK_DEFAULT_NOW });
    expect(plain.result.location).toBe(`${MOCK_EXPORT_DIR}/${plain.result.export_id}`);
    expect(plain.result.manifest_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(plain.warnings).toBeUndefined();
    const withPrivate = await exported(client, { domains: ['LEARNING'], format: 'JSON', include_private: true });
    // The PRIVATE note and the PRIVATE fixture attempt are added; the SECRET note never is.
    expect(withPrivate.result.rows - plain.result.rows).toBe(2);
    const csv = await exported(client, { domains: ['LEARNING'], format: 'CSV', include_private: false });
    expect(csv.result.rows).toBe(plain.result.rows);
    expect(csv.result.files).toBeGreaterThan(plain.result.files);
    expect(csv.result.manifest_sha256).not.toBe(plain.result.manifest_sha256);
  });

  it('labels the selection in domain order and binds the approval to it', async () => {
    const { client } = harness();
    const payload: RequestOf<'export.request'> = { domains: ['SYSTEM', 'LEARNING', 'LEARNING'], format: 'JSON', include_private: false };
    const first = await client.invoke('export.request', payload);
    const approvalId = await approvalFor(client, first);
    // The resource names the selection, the privacy ceiling and the format (privacy.rs).
    expect((await approvalState(client, approvalId))?.operation.resource).toEqual({ kind: 'EXPORT', id: 'export:LEARNING.SYSTEM:INTERNAL:JSON' });
    const done = ok(await client.invoke('export.request', payload, { approvalId })).result;
    expect(done.domains).toEqual(['LEARNING', 'SYSTEM']);
  });

  it('without an export directory the approval comes first; the approved retry is refused and the approval stays unspent', async () => {
    const { client } = harness({ exportDir: null });
    const payload: RequestOf<'export.request'> = { domains: ['LEARNING'], format: 'JSON', include_private: false };
    const approvalId = await approvalFor(client, await client.invoke('export.request', payload));
    expect(err(await client.invoke('export.request', payload, { approvalId }))).toMatchObject({ category: 'PRECONDITION_FAILED', code: 'export.dir_not_configured' });
    expect((await approvalState(client, approvalId))?.state).toBe('APPROVED');
  });
});

describe('mock transport: boundary limits, evidence references, objective status, deleted events (mirrors g9-app)', () => {
  const invalid = (r: InvokeResult<OperationName>) => {
    expect(err(r)).toMatchObject({ category: 'INVALID_INPUT', code: 'ipc.payload_invalid' });
  };

  it('refuses titles, reasons, labels, codes and lists over their limits (app-9)', async () => {
    const { client } = harness();
    const note = { body_markdown: 'Body.', classification: 'INTERNAL' as const, domain: 'LEARNING' as const };
    invalid(await client.invoke('notes.create', { ...note, title: 't'.repeat(201) }));
    const kept = ok(await client.invoke('notes.create', { ...note, title: 'é'.repeat(200) })).result;
    invalid(await client.invoke('notes.revise', { note_id: kept.note_id, title: 't'.repeat(201), body_markdown: 'x' }));
    invalid(await client.invoke('notes.create', { ...note, title: 'T', evidence_refs: Array.from({ length: 101 }, (_, i) => `e-${i}`) }));
    invalid(await client.invoke('backup.create', { label: 'l'.repeat(201) }));
    invalid(await client.invoke('skills.invoke', { skill_code: 's'.repeat(129), input: {} }));
    const session = ok(await client.invoke('session.create', { planned_minutes: 30 })).result.session_id;
    invalid(await client.invoke('session.correct_time', { session_id: session, interval_id: 'i-1', credited_ms: 0, reason: 'r'.repeat(501) }));
    const attempt = ok(await client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' })).result.attempt_id;
    invalid(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: Array.from({ length: 101 }, (_, i) => `e-${i}`) }));
    invalid(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: ['e-1', 'e-1'] }));
    const criteria = Array.from({ length: 65 }, (_, i) => ({ criterion_code: `c${i}`, score: 1 as const, source: 'HUMAN_REVIEWER' as const, evidence_refs: [] }));
    invalid(await client.invoke('assessment.grade_provisional', { attempt_id: attempt, criteria }));
    // The byte cap is checked before anything else reads the payload.
    expect(err(await client.invoke('calendar.list', { from: 0, to: 1, zone: 'z'.repeat(2 * 1024 * 1024) }))).toMatchObject({
      category: 'INVALID_INPUT',
      code: 'ipc.payload_too_large',
      details: { limit_bytes: 1024 * 1024 },
    });
  });

  it('grades only with evidence submitted for the attempt (app-8)', async () => {
    const { client } = harness();
    const note = ok(await client.invoke('notes.create', { title: 'Write-up', body_markdown: 'Body.', classification: 'INTERNAL', domain: 'LEARNING' })).result;
    const own = ok(await client.invoke('evidence.from_note', { note_id: note.note_id })).result.evidence_id;
    const attempt = ok(await client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' })).result.attempt_id;
    ok(await client.invoke('assessment.submit', { attempt_id: attempt, evidence_refs: [own] }));
    ok(await client.invoke('assessment.review_evidence', { attempt_id: attempt }));
    const grade = (refs: string[]) =>
      client.invoke('assessment.grade_provisional', {
        attempt_id: attempt,
        criteria: BASELINE_CRITERIA.map((c, i) => ({ criterion_code: c.code, score: 3 as const, source: 'HUMAN_REVIEWER' as const, evidence_refs: i === 0 ? refs : [] })),
      });
    expect(err(await grade(['evi-does-not-exist']))).toMatchObject({ category: 'INVALID_INPUT', code: 'grade.evidence_unknown', details: { evidence_id: 'evi-does-not-exist' } });
    expect(ok(await grade([own])).result.state).toBe('GRADED_PROVISIONAL');
  });

  it('derives an objective status from the sessions linked to it (app-7)', async () => {
    const { client } = harness();
    const status = async (objective: string) =>
      ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' })).result.objectives.find((o) => o.objective_id === objective)?.status;
    expect(await status('D15-O1')).toBe('PLANNED');
    const session = ok(await client.invoke('session.create', { planned_minutes: 30, objective_id: 'D15-O1' })).result.session_id;
    expect(await status('D15-O1')).toBe('PLANNED');
    ok(await client.invoke('session.prepare', { session_id: session }));
    ok(await client.invoke('session.start', { session_id: session }));
    expect(await status('D15-O1')).toBe('IN_PROGRESS');
    expect(await status('D15-O2')).toBe('PLANNED');
    ok(await client.invoke('session.complete', { session_id: session }));
    expect(await status('D15-O1')).toBe('COMPLETED');
  });

  it('a deleted event leaves no title in a stored result; a replay says it was deleted (G1)', async () => {
    const { client } = harness();
    const start = Date.UTC(2026, 10, 20, 9, 0);
    const payload = { title: 'Dentist with Dr Example', kind: 'APPOINTMENT' as const, start, end: start + 3_600_000 };
    const id = ok(await client.invoke('calendar.create_event', payload, { idempotencyKey: 'ev-create' })).result.event_id;
    expect(ok(await client.invoke('calendar.create_event', payload, { idempotencyKey: 'ev-create' })).replayed).toBe(true);
    const approvalId = await approvalFor(client, await client.invoke('calendar.delete_event', { event_id: id }));
    ok(await client.invoke('calendar.delete_event', { event_id: id }, { approvalId }));
    expect(err(await client.invoke('calendar.create_event', payload, { idempotencyKey: 'ev-create' }))).toMatchObject({
      category: 'NOT_FOUND',
      code: 'calendar.event_deleted',
      details: { event_id: id },
    });
  });
});
