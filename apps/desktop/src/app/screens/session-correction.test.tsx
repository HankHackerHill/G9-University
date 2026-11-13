/**
 * Time corrections after more than one interruption, and the Complete confirmation. A
 * correction names its interval; the backend credits the latest correction of an interval
 * (timer.rs `latest_correction_wins`) and keeps accepting corrections in every session state
 * (sessions.rs `correct_time` has no state precondition; policy `operator.correct.allow`), so
 * the UI offers already-corrected intervals with a visible "supersedes" note and keeps the form
 * reachable while the backend reports `requires_correction`.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { RequestOf, SessionView } from '../../ipc/operations';
import { createInterceptHarness, outcome } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
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

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const transitions = () => screen.getByRole('group', { name: 'Session transitions' });

describe('Session Tracker corrections after a second interruption', () => {
  it('offers every closed UNCERTAIN interval, marks the corrected one as superseded, and sends the interval the operator chose', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const form = await screen.findByRole('form', { name: 'Time correction' });
    expect(screen.getByRole('heading', { level: 2, name: 'Correct the time' })).toBeInTheDocument();
    const select = within(form).getByLabelText('Interval in doubt');
    const options = within(select)
      .getAllByRole('option')
      .map((o) => (o as HTMLOptionElement).value)
      .filter((v) => v !== '');
    // NEW-5: the already-corrected ti_1 is offered too (the backend credits the latest correction), labelled as such.
    expect(options).toEqual(['ti_1', 'ti_2']);
    // An interval reads as its time span; the interval id is the option's value (T0 is 09:00 in Europe/Berlin).
    expect(within(select).getByRole('option', { name: /09:00 to 09:40 \(0:40:00\), corrected to 0:30:00/ })).toHaveValue('ti_1');
    expect(within(select).getByRole('option', { name: /10:00 to 10:20 \(0:20:00\), not corrected/ })).toHaveValue('ti_2');
    const submit = within(form).getByRole('button', { name: 'Record correction' });
    await user.clear(within(form).getByLabelText('Minutes to credit'));
    await user.type(within(form).getByLabelText('Minutes to credit'), '15');
    await user.type(within(form).getByLabelText('Reason'), 'second interruption');
    // No interval is pre-selected: the operator chooses it explicitly.
    expect(submit).toBeDisabled();
    expect(screen.queryByTestId('supersedes-note')).toBeNull();
    await user.selectOptions(select, 'ti_2');
    expect(screen.queryByTestId('supersedes-note')).toBeNull();
    await user.click(submit);
    const sent = harness.calls.filter((c) => c.op === 'session.correct_time').map((c) => c.payload as RequestOf<'session.correct_time'>);
    expect(sent).toEqual([{ session_id: 'ss_1', interval_id: 'ti_2', credited_ms: 15 * MIN, reason: 'second interruption' }]);
  });

  it('lets the operator supersede a wrong correction and says which correction is superseded (NEW-5)', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const form = await screen.findByRole('form', { name: 'Time correction' });
    await user.selectOptions(within(form).getByLabelText('Interval in doubt'), 'ti_1');
    const note = screen.getByTestId('supersedes-note');
    expect(note).toHaveTextContent('This replaces correction tc_1 (0:30:00 credited). Both are kept; the latest one counts.');
    await user.clear(within(form).getByLabelText('Minutes to credit'));
    await user.type(within(form).getByLabelText('Minutes to credit'), '35');
    await user.type(within(form).getByLabelText('Reason'), 'typed 30 instead of 35');
    await user.click(within(form).getByRole('button', { name: 'Record correction' }));
    const sent = harness.calls.filter((c) => c.op === 'session.correct_time').map((c) => c.payload as RequestOf<'session.correct_time'>);
    expect(sent).toEqual([{ session_id: 'ss_1', interval_id: 'ti_1', credited_ms: 35 * MIN, reason: 'typed 30 instead of 35' }]);
  });

  it('keeps the correction form available while the backend reports requires_correction outside INTERRUPTED', async () => {
    renderWithIpc(<SessionTracker />, harnessWith(twiceInterrupted({ state: 'PAUSED' })));
    const form = await screen.findByRole('form', { name: 'Time correction' });
    expect(within(form).getByRole('option', { name: /10:00 to 10:20/ })).toHaveValue('ti_2');
  });

  it('a COMPLETED session with an uncorrected interval still offers the form, and the dialog copy agrees (NEW-2)', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    await user.click(within(await screen.findByRole('group', { name: 'Session transitions' })).getByRole('button', { name: 'Complete' }));
    const dialog = await screen.findByTestId('complete-warning');
    // The backend accepts corrections after COMPLETED (session_view keeps requires_correction = true), so the copy must not say the time is lost.
    expect(dialog).not.toHaveTextContent(/never credited/);
    expect(dialog).toHaveTextContent('that time stays in doubt and uncredited until you record a correction');
    expect(dialog).toHaveTextContent('You can still record one after completing');
    expect(dialog).toHaveTextContent('Credit is never assumed');
  });

  it('offers the correction form for a COMPLETED session the backend still flags', async () => {
    renderWithIpc(<SessionTracker />, harnessWith(twiceInterrupted({ state: 'COMPLETED' })));
    const form = await screen.findByRole('form', { name: 'Time correction' });
    expect(screen.getByRole('heading', { level: 2, name: 'Correct the time' })).toBeInTheDocument();
    expect(within(form).getByRole('option', { name: /10:00 to 10:20 .* not corrected/ })).toHaveValue('ti_2');
    expect(screen.queryByTestId('interrupted-banner')).toBeNull();
  });
});

describe('Session Tracker Complete confirmation', () => {
  it('asks before completing a session that still has uncorrected UNCERTAIN time, and only the dialog button sends', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    await user.click(within(group).getByRole('button', { name: 'Complete' }));
    const warning = await screen.findByTestId('complete-warning');
    expect(warning).toHaveTextContent('10:00 to 10:20 (0:20:00)');
    expect(warning).not.toHaveTextContent('09:00 to 09:40');
    expect(harness.calls.some((c) => c.op === 'session.complete')).toBe(false);
    await user.click(within(warning).getByRole('button', { name: 'Keep it open' }));
    expect(screen.queryByTestId('complete-warning')).toBeNull();
    expect(within(group).getByRole('button', { name: 'Complete' })).toBeEnabled();
    await user.click(within(group).getByRole('button', { name: 'Complete' }));
    await user.click(within(await screen.findByTestId('complete-warning')).getByRole('button', { name: 'Complete without crediting' }));
    expect(harness.calls.filter((c) => c.op === 'session.complete')).toHaveLength(1);
  });

  it('a second press on Complete (double-click) cannot bypass the confirmation (NEW-4)', async () => {
    const user = userEvent.setup();
    const harness = harnessWith(twiceInterrupted());
    renderWithIpc(<SessionTracker />, harness);
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    const complete = within(group).getByRole('button', { name: 'Complete' });
    await user.click(complete);
    await screen.findByTestId('complete-warning');
    // The originating button is disabled while the dialog is open; a programmatic second click still sends nothing.
    expect(complete).toBeDisabled();
    await user.click(complete);
    await user.dblClick(complete);
    expect(harness.calls.filter((c) => c.op === 'session.complete')).toHaveLength(0);
    expect(screen.getByTestId('complete-warning')).toBeInTheDocument();
  });

  it('is an alertdialog with an accessible name and moves focus to the button that keeps the session open (NEW-6)', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SessionTracker />, harnessWith(twiceInterrupted()));
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    await user.click(within(group).getByRole('button', { name: 'Complete' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Complete with time still in doubt?' });
    expect(dialog).toHaveAccessibleDescription(/10:00 to 10:20 \(0:20:00\)/);
    const cancel = within(dialog).getByRole('button', { name: 'Keep it open' });
    expect(document.activeElement).toBe(cancel);
    // Enter on the focused element cancels instead of completing.
    await user.keyboard('{Enter}');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(within(group).getByRole('button', { name: 'Complete' })).toBeEnabled();
  });
});

describe('Session Tracker mutations and the list revalidation (NEW-3)', () => {
  it('shows the SessionView carried by the outcome and keeps every control disabled until session.list has caught up, so the next call carries the new revision', async () => {
    const user = userEvent.setup();
    const running = twiceInterrupted({
      state: 'RUNNING',
      intervals: [{ interval_id: 'ti_1', start_utc: T0, end_utc: null, continuity: 'VERIFIED' }],
      corrections: [],
      requires_correction: false,
      uncertain_ms: 0,
      credited_ms: 0,
      open_interval_started_at: T0,
      revision: 5,
    });
    const paused: SessionView = { ...running, state: 'PAUSED', open_interval_started_at: null, revision: 6 };
    let lists = 0;
    const gate = deferred<unknown>();
    const harness = createInterceptHarness(async (op, _payload, next) => {
      if (op === 'session.list') {
        lists += 1;
        // The first read answers at once; the revalidation after Pause is held back like a slow SQLite round trip.
        return lists === 1 ? outcome({ sessions: [running] }) : gate.promise;
      }
      if (op === 'session.pause') {
        return outcome(paused);
      }
      if (op === 'session.resume') {
        return outcome({ ...paused, state: 'RUNNING', revision: 7 });
      }
      return next();
    });
    renderWithIpc(<SessionTracker />, harness);
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    expect(screen.getByTestId('revision')).toHaveTextContent('revision 5');
    await user.click(within(group).getByRole('button', { name: 'Pause' }));
    // The outcome's SessionView (PAUSED, revision 6) is shown while the list is still in flight...
    await waitFor(() => {
      expect(screen.getByTestId('revision')).toHaveTextContent('revision 6');
    });
    expect(screen.getAllByText('Paused').length).toBeGreaterThan(0);
    // ...and no control can fire with the stale revision in the meantime.
    for (const name of ['Pause', 'Resume', 'Complete']) {
      expect(within(transitions()).getByRole('button', { name })).toBeDisabled();
    }
    gate.resolve(outcome({ sessions: [paused] }));
    await waitFor(() => {
      expect(within(transitions()).getByRole('button', { name: 'Resume' })).toBeEnabled();
    });
    await user.click(within(transitions()).getByRole('button', { name: 'Resume' }));
    const resume = harness.calls.find((c) => c.op === 'session.resume');
    expect(resume?.meta.expected_revision).toBe(6);
  });
});

describe('Session Tracker against the mock timer ledger (NEW-1)', () => {
  it('a correction supersedes the uncertain interval; a second interruption reopens the flow with the earlier correction offered for superseding', async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    const first = renderWithIpc(<SessionTracker />, harness);
    expect(await screen.findByTestId('interrupted-banner')).toBeInTheDocument();
    expect(screen.getByTestId('uncertain')).toHaveTextContent('0:40:00');
    const form = screen.getByRole('form', { name: 'Time correction' });
    const select = within(form).getByLabelText('Interval in doubt');
    const intervalId = (within(select).getAllByRole('option')[1] as HTMLOptionElement).value;
    await user.selectOptions(select, intervalId);
    await user.clear(within(form).getByLabelText('Minutes to credit'));
    await user.type(within(form).getByLabelText('Minutes to credit'), '30');
    await user.type(within(form).getByLabelText('Reason'), 'laptop suspended');
    await user.click(within(form).getByRole('button', { name: 'Record correction' }));
    // Backend semantics (Ledger::credited): the corrected interval leaves uncertain_ms.
    await waitFor(() => {
      expect(screen.getByTestId('credited')).toHaveTextContent('0:30:00');
    });
    expect(screen.getByTestId('uncertain')).toHaveTextContent('0:00:00');
    expect(screen.queryByTestId('interrupted-banner')).toBeNull();
    await waitFor(() => {
      expect(within(transitions()).getByRole('button', { name: 'Resume' })).toBeEnabled();
    });
    expect(screen.getByRole('form', { name: 'Time correction' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: 'Change a correction' })).toBeInTheDocument();
    await user.click(within(transitions()).getByRole('button', { name: 'Resume' }));
    await waitFor(() => {
      expect(within(transitions()).getByRole('button', { name: 'Pause' })).toBeEnabled();
    });
    // Suspend/crash while RUNNING: the open interval closes UNCERTAIN and the session is INTERRUPTED again.
    harness.transport.clock.advance(20 * MIN);
    expect(harness.transport.onProcessRestart()).toHaveLength(1);
    first.unmount();
    renderWithIpc(<SessionTracker />, harness);
    expect(await screen.findByTestId('interrupted-banner')).toBeInTheDocument();
    expect(screen.getByTestId('credited')).toHaveTextContent('0:30:00');
    expect(screen.getByTestId('uncertain')).toHaveTextContent('0:20:00');
    const again = screen.getByRole('form', { name: 'Time correction' });
    expect(screen.getByRole('heading', { level: 2, name: 'Correct the time' })).toBeInTheDocument();
    const names = within(within(again).getByLabelText('Interval in doubt'))
      .getAllByRole('option')
      .map((o) => o.textContent)
      .filter((t) => !t.startsWith('Choose'));
    expect(names).toHaveLength(2);
    expect(names[0]).toMatch(/, corrected to 0:30:00$/);
    expect(names[1]).toMatch(/\(0:20:00\), not corrected$/);
  });
});
