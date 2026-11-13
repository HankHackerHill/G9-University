/**
 * Dashboard KPIs follow docs/evaluations/kpi-definitions.md (UI-11): minimum n, mode, window,
 * denominator, flags and the backlog breakdown. Fixtures are fed through the intercept harness;
 * `today.now` is the mock clock (MOCK_DEFAULT_NOW, 2026-11-18 08:30Z).
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { AttemptView, GradeView, ReliabilityClass, ReviewItem } from '../../ipc/operations';
import { MOCK_DEFAULT_NOW } from '../../ipc/transports/mock';
import { STORAGE_BUSY, createInterceptHarness, outcome } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { PerformanceDashboard } from './PerformanceDashboard';

const DAY = 86_400_000;
const NOW = MOCK_DEFAULT_NOW;

function grade(pass: boolean, reliability: ReliabilityClass = 'EXTERNAL_REVIEW'): GradeView {
  return { total: pass ? 80 : 40, pass, gate_failures: pass ? [] : ['scope_compliance_reporting'], criteria: [], reliability_class: reliability, final: true };
}

function attempt(id: string, definition: string, overrides: Partial<AttemptView> = {}): AttemptView {
  return {
    attempt_id: id,
    assessment_id: definition,
    title: definition,
    state: 'APPROVED_FINAL',
    mode: 'EXAM',
    variant: 'A',
    retake_of: null,
    started_at: NOW - 3 * DAY,
    submitted_at: NOW - 3 * DAY + 3_600_000,
    finalized_at: NOW - 2 * DAY,
    grade: grade(true),
    revision: 5,
    ...overrides,
  };
}

function review(id: string, dueAt: number): ReviewItem {
  return { review_item_id: id, competency_id: null, objective_id: null, prompt: id, kind: 'DECLARATIVE', due_at: dueAt, last_success_at: null, offset_days: 1, estimated_ms: 180_000 };
}

function harness(attempts: AttemptView[], due?: { overdue: ReviewItem[]; due: ReviewItem[]; overflow: ReviewItem[] }) {
  return createInterceptHarness(async (op, _payload, next) => {
    if (op === 'assessment.list') {
      return outcome({ definitions: [], attempts });
    }
    if (op === 'retention.due' && due !== undefined) {
      return outcome({ as_of: NOW, budget_ms: 900_000, ...due });
    }
    return next();
  });
}

/** The Value and Records cells of the row of the measures table with the given name. */
async function kpiRow(name: string): Promise<{ value: HTMLElement; evidence: HTMLElement }> {
  const table = await screen.findByRole('table', { name: /Learning measures/ });
  const label = await within(table).findByText(name);
  const row = label.closest('tr');
  if (row === null) {
    throw new Error(`row for ${name} not found`);
  }
  const cells = within(row).getAllByRole('cell');
  const value = cells[1];
  const evidence = cells[2];
  if (value === undefined || evidence === undefined) {
    throw new Error('row has fewer than 3 cells');
  }
  return { value, evidence };
}

const FIRST = 'First-attempt pass rate';
const EVENTUAL = 'Eventual pass rate';
const BACKLOG = 'Review backlog';

/** Four EXAM first attempts finalized within the rolling 4 weeks, one of them SELF_ADJUDICATED. */
const fourFirstAttempts = [
  attempt('att-1', 'def-1'),
  attempt('att-2', 'def-2', { grade: grade(true, 'SELF_ADJUDICATED') }),
  attempt('att-3', 'def-3', { grade: grade(false) }),
  attempt('att-4', 'def-4'),
];

describe('Performance Dashboard KPIs (kpi-definitions.md)', () => {
  it('(a) fewer than 4 EXAM first attempts in the window says "Not enough data" with n and the minimum, never a rate', async () => {
    renderWithIpc(<PerformanceDashboard />, harness(fourFirstAttempts.slice(0, 3)));
    const { value } = await kpiRow(FIRST);
    await waitFor(() => {
      expect(value).toHaveTextContent('Not enough data: n = 3, minimum 4');
    });
    expect(value).not.toHaveTextContent(/\d+ of \d+/);
    // The headline tile says the same and shows no figure.
    const tile = screen.getAllByText(FIRST).map((el) => el.closest('.stat')).find((el) => el !== null);
    expect(tile).toHaveAttribute('data-standing', 'missing');
    expect(tile).toHaveTextContent('n = 3, minimum 4');
  });

  it('(b) STUDY-mode attempts, retakes and attempts finalized more than 28 days before today.now are excluded from first_attempt_pass_rate', async () => {
    const attempts = [
      ...fourFirstAttempts,
      attempt('att-study', 'def-5', { mode: 'STUDY' }),
      attempt('att-retake', 'def-1', { retake_of: 'att-1', grade: grade(false) }),
      attempt('att-old', 'def-6', { finalized_at: NOW - 30 * DAY }),
      attempt('att-edge', 'def-7', { finalized_at: NOW - 28 * DAY }),
      attempt('att-provisional', 'def-8', { state: 'GRADED_PROVISIONAL', finalized_at: null, grade: { ...grade(true), final: false } }),
    ];
    renderWithIpc(<PerformanceDashboard />, harness(attempts));
    const { value, evidence } = await kpiRow(FIRST);
    await waitFor(() => {
      expect(value).toHaveTextContent(/^3 of 4/);
    });
    expect(evidence).toHaveTextContent('assessment.list: 4');
    expect(evidence).toHaveTextContent('att-1, att-2 [self-graded], att-3, att-4');
    for (const excluded of ['att-study', 'att-retake', 'att-old', 'att-edge', 'att-provisional']) {
      expect(evidence).not.toHaveTextContent(excluded);
    }
  });

  it('(c) eventual_pass_rate counts distinct definitions attempted: 3 failed attempts and 1 pass of one definition count 1/1', async () => {
    const attempts = [
      attempt('a-fail-1', 'def-A', { grade: grade(false) }),
      attempt('a-fail-2', 'def-A', { retake_of: 'a-fail-1', grade: grade(false) }),
      attempt('a-fail-3', 'def-A', { retake_of: 'a-fail-2', grade: grade(false) }),
      attempt('a-pass', 'def-A', { retake_of: 'a-fail-3' }),
      attempt('b-pass', 'def-B'),
      attempt('c-fail', 'def-C', { grade: grade(false) }),
      attempt('d-started', 'def-D', { state: 'STARTED', finalized_at: null, grade: null }),
    ];
    renderWithIpc(<PerformanceDashboard />, harness(attempts));
    const { value, evidence } = await kpiRow(EVENTUAL);
    await waitFor(() => {
      // Per attempt it would read 2 of 7; the denominator is the 4 definitions attempted.
      expect(value).toHaveTextContent('2 of 4');
    });
    expect(evidence).toHaveTextContent('def-A, def-B, def-C, def-D');
  });

  it('(d) SELF_ADJUDICATED first attempts are flagged as self-graded in the value and in the records', async () => {
    renderWithIpc(<PerformanceDashboard />, harness(fourFirstAttempts));
    const { value, evidence } = await kpiRow(FIRST);
    await waitFor(() => {
      expect(value).toHaveTextContent('3 of 4 (1 self-graded)');
    });
    expect(evidence).toHaveTextContent('att-2 [self-graded]');
    expect(evidence).not.toHaveTextContent('att-1 [self-graded]');
  });

  it('(e) review_backlog headline equals overdue + due + overflow and shows the breakdown', async () => {
    const due = {
      overdue: [review('r-1', NOW - 2 * DAY), review('r-2', NOW - DAY)],
      due: [review('r-3', NOW), review('r-4', NOW + 3_600_000), review('r-5', NOW + 7_200_000)],
      overflow: [review('r-6', NOW + 10_800_000)],
    };
    renderWithIpc(<PerformanceDashboard />, harness(fourFirstAttempts, due));
    const { value, evidence } = await kpiRow(BACKLOG);
    await waitFor(() => {
      expect(value).toHaveTextContent('6 (2 overdue, 3 due today, 1 beyond budget)');
    });
    expect(evidence).toHaveTextContent('retention.due: 6');
    expect(evidence).toHaveTextContent('r-1, r-2, r-3, r-4, r-5, r-6');
  });

  it('states the calibration definition from kpi-definitions.md (rolling 4 weeks; min n 20) with no data', async () => {
    renderWithIpc(<PerformanceDashboard />, harness(fourFirstAttempts));
    const label = await screen.findByText('Confidence calibration');
    const row = label.closest('tr');
    expect(row).not.toBeNull();
    expect(row).toHaveTextContent('No data: no confidence reports recorded');
    expect(row).toHaveTextContent('Rolling 4 weeks; min n 20');
    expect(row).toHaveTextContent('calibration_brier');
  });

  it('does not claim Verified when a read behind the table failed; the affected cells say "No data" with the reason', async () => {
    const failing = createInterceptHarness(async (op, _payload, next) => (op === 'assessment.list' ? STORAGE_BUSY : next()));
    renderWithIpc(<PerformanceDashboard />, failing);
    await waitFor(() => {
      expect(screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE')).toHaveLength(1);
    });
    const { value } = await kpiRow(FIRST);
    expect(value).toHaveTextContent('No data: attempts not loaded');
    await waitFor(() => {
      expect(screen.getByTestId('dashboard-provenance-partial')).toBeInTheDocument();
    });
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
  });

  it('claims VERIFIED only once every read succeeded without a warning', async () => {
    renderWithIpc(<PerformanceDashboard />, harness(fourFirstAttempts));
    expect(await screen.findByLabelText('Provenance: verified')).toBeInTheDocument();
    expect(screen.queryByTestId('dashboard-provenance-partial')).toBeNull();
  });
});
