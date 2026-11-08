import { describe, expect, it } from 'vitest';
import { createClient, isG9Error, type InvokeResult } from '../client';
import { OPERATIONS, type CriterionResult, type GradeSource, type OperationName } from '../operations';
import type { ClientCommandMeta } from '../types';
import { BASELINE_CRITERIA, MOCK_DEFAULT_NOW, computeGrade, createMockClock, createMockTransport } from './mock';

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

  it('emits the backend disclosure warnings: non-canonical daily plan, export not implemented', async () => {
    const { client } = harness();
    const plan = ok(await client.invoke('curriculum.daily_plan', { date: '2026-11-18' }));
    expect(plan.warnings).toEqual(['curriculum.non_canonical: plan derived from a version that is not APPROVED_CANONICAL']);
    const need = err(await client.invoke('export.request', { domains: ['LEARNING'], format: 'JSON', include_private: false }));
    ok(await client.invoke('approvals.decide', { approval_id: need.approval_id ?? '', decision: 'APPROVE' }));
    const exported = ok(await client.invoke('export.request', { domains: ['LEARNING'], format: 'JSON', include_private: false }, { approvalId: need.approval_id ?? '' }));
    expect(exported.warnings?.[0]).toMatch(/^export\.execution_not_implemented: .*no file was produced$/);
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
