/**
 * Time corrections after more than one interruption. A correction names its interval; the UI
 * must never re-correct (and so replace the credit of) an interval that already has one, and must
 * keep the correction reachable while the backend reports `requires_correction`.
 */
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { RequestOf, SessionView } from '../../ipc/operations';
import { createInterceptHarness, outcome } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { SessionTracker } from './SessionTracker';

const T0 = Date.UTC(2026, 10, 17, 8, 0);
const MIN = 60_000;

function twiceInterrupted(overrides: Partial<SessionView> = {}): SessionView {
  return {
    session_id: 'ss_1',
    state: 'INTERRUPTED',
    day_id: 'D14',
    block: 'GUIDED_LAB',
    block_seq: 3,
    planned_ms: 90 * MIN,
    credited_ms: 30 * MIN,
    uncertain_ms: 20 * MIN,
    started_at: T0,
    open_interval_started_at: null,
    requires_correction: true,
    intervals: [
      { interval_id: 'ti_1', start_utc: T0, end_utc: T0 + 40 * MIN, continuity: 'UNCERTAIN' },
      { interval_id: 'ti_2', start_utc: T0 + 60 * MIN, end_utc: T0 + 80 * MIN, continuity: 'UNCERTAIN' },
    ],
    corrections: [{ correction_id: 'tc_1', interval_id: 'ti_1', credited_ms: 30 * MIN, reason: 'first interruption', corrected_at: T0 + 50 * MIN }],
    revision: 7,
    ...overrides,
  };
}

function harnessWith(session: SessionView) {
  return createInterceptHarness(async (op, _payload, next) => {
    if (op === 'session.list') {
      return outcome({ sessions: [session] });
    }
    if (op === 'session.correct_time' || op === 'session.complete') {
      return outcome({ ...session, revision: session.revision + 1 });
    }
    return next();
  });
}

describe('Session Tracker corrections after a second interruption', () => {
  it('offers only the uncorrected UNCERTAIN interval and sends the interval the operator chose', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const form = await screen.findByRole('form', { name: 'Time correction' });
    const select = within(form).getByLabelText('Uncertain interval to correct');
    const options = within(select)
      .getAllByRole('option')
      .map((o) => (o as HTMLOptionElement).value)
      .filter((v) => v !== '');
    expect(options).toEqual(['ti_2']);
    expect(within(select).getByRole('option', { name: /ti_2 .* 0:20:00/ })).toBeInTheDocument();
    const submit = within(form).getByRole('button', { name: 'Record correction' });
    await user.clear(within(form).getByLabelText('Minutes to credit'));
    await user.type(within(form).getByLabelText('Minutes to credit'), '15');
    await user.type(within(form).getByLabelText('Reason'), 'second interruption');
    // No interval is pre-selected: the operator chooses it explicitly.
    expect(submit).toBeDisabled();
    await user.selectOptions(select, 'ti_2');
    await user.click(submit);
    const sent = harness.calls.filter((c) => c.op === 'session.correct_time').map((c) => c.payload as RequestOf<'session.correct_time'>);
    expect(sent).toEqual([{ session_id: 'ss_1', interval_id: 'ti_2', credited_ms: 15 * MIN, reason: 'second interruption' }]);
  });

  it('keeps the correction form available while the backend reports requires_correction outside INTERRUPTED', async () => {
    renderWithIpc(<SessionTracker />, harnessWith(twiceInterrupted({ state: 'PAUSED' })));
    const form = await screen.findByRole('form', { name: 'Time correction' });
    expect(within(form).getByRole('option', { name: /ti_2/ })).toBeInTheDocument();
  });

  it('asks before completing a session that still has uncorrected UNCERTAIN time', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    await user.click(within(group).getByRole('button', { name: 'Complete' }));
    const warning = await screen.findByTestId('complete-warning');
    expect(warning).toHaveTextContent('ti_2 0:20:00');
    expect(harness.calls.some((c) => c.op === 'session.complete')).toBe(false);
    await user.click(within(warning).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByTestId('complete-warning')).toBeNull();
    await user.click(within(group).getByRole('button', { name: 'Complete' }));
    await user.click(within(await screen.findByTestId('complete-warning')).getByRole('button', { name: 'Complete without crediting' }));
    expect(harness.calls.filter((c) => c.op === 'session.complete')).toHaveLength(1);
  });
});
