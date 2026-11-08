/**
 * After a mutation the Assessments screen shows the AttemptView the backend returned and keeps
 * every control disabled until `assessment.list` has caught up, so a second click never carries
 * the pre-mutation revision (NEW-3). Reproduced with a held-back list read, like a slow SQLite
 * round trip; the mock alone resolves in a microtask and cannot show the window.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AttemptView } from '../../ipc/operations';
import { createInterceptHarness, outcome } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { Assessments } from './Assessments';

const T0 = Date.UTC(2026, 10, 17, 9, 0);

const started: AttemptView = {
  attempt_id: 'att_1',
  assessment_id: 'def_1',
  title: 'Week 2 blind practical',
  state: 'STARTED',
  mode: 'EXAM',
  variant: 'A',
  retake_of: null,
  started_at: T0,
  submitted_at: null,
  finalized_at: null,
  grade: null,
  revision: 2,
};
const submitted: AttemptView = { ...started, state: 'SUBMITTED', submitted_at: T0 + 3_600_000, revision: 3 };

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const group = () => screen.getByRole('group', { name: 'Attempt transitions' });

describe('Assessments mutations and the list revalidation', () => {
  it('shows the returned attempt, locks the controls until the list catches up, and sends the new revision next', async () => {
    const user = userEvent.setup();
    let lists = 0;
    const gate = deferred<unknown>();
    const harness = createInterceptHarness(async (op, _payload, next) => {
      if (op === 'assessment.list') {
        lists += 1;
        return lists === 1 ? outcome({ definitions: [], attempts: [started] }) : gate.promise;
      }
      if (op === 'assessment.submit') {
        return outcome(submitted);
      }
      if (op === 'assessment.review_evidence') {
        return outcome({ ...submitted, state: 'EVIDENCE_REVIEW', revision: 4 });
      }
      return next();
    });
    renderWithIpc(<Assessments />, harness);
    await screen.findByRole('group', { name: 'Attempt transitions' });
    expect(screen.getByTestId('attempt-revision')).toHaveTextContent('revision 2');
    expect(within(group()).getByRole('button', { name: 'Submit' })).toBeEnabled();
    await user.click(within(group()).getByRole('button', { name: 'Submit' }));
    // The outcome's AttemptView (SUBMITTED, revision 3) is shown while the list read is still in flight...
    await waitFor(() => {
      expect(screen.getByTestId('attempt-revision')).toHaveTextContent('revision 3');
    });
    expect(screen.getAllByText('SUBMITTED').length).toBeGreaterThan(0);
    // ...and nothing can be clicked with the stale revision meanwhile.
    for (const name of ['Submit', 'Review evidence', 'Grade (provisional)', 'Approve final']) {
      expect(within(group()).getByRole('button', { name })).toBeDisabled();
    }
    gate.resolve(outcome({ definitions: [], attempts: [submitted] }));
    await waitFor(() => {
      expect(within(group()).getByRole('button', { name: 'Review evidence' })).toBeEnabled();
    });
    await user.click(within(group()).getByRole('button', { name: 'Review evidence' }));
    const review = harness.calls.find((c) => c.op === 'assessment.review_evidence');
    expect(review?.meta.expected_revision).toBe(3);
  });
});
