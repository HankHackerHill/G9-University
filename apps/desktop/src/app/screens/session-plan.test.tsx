/**
 * Planning a session from the Session Tracker (`session.create`): the form sends what the request
 * schema allows, resolves the optional start in the backend's zone (never the webview's), and the
 * new session is listed (labelled UNLINKED, as `session_view` labels it) and selected.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { SessionTracker } from './SessionTracker';

describe('Session Tracker: plan a session', () => {
  it('sends minutes, mode, objective and the start in the backend zone; the new session is listed and selected', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<SessionTracker />, harness);
    const form = await screen.findByRole('form', { name: 'Plan a session' });
    const submit = within(form).getByRole('button', { name: 'Plan session' });
    // Enabled only once the backend clock (and so the zone) is known.
    await waitFor(() => {
      expect(submit).toBeEnabled();
    });
    const minutes = within(form).getByLabelText('Planned minutes');
    await user.clear(minutes);
    await user.type(minutes, '50');
    fireEvent.change(within(form).getByLabelText('Start (optional)'), { target: { value: '2026-11-18T14:00' } });
    await user.selectOptions(within(form).getByLabelText('Mode'), 'LAB');
    await user.type(within(form).getByLabelText('Objective code (optional)'), 'D15-O2');
    await user.click(submit);

    const call = harness.calls.find((c) => c.op === 'session.create');
    // 14:00 in Europe/Berlin (the mock backend's zone) is 13:00Z.
    expect(call?.payload).toEqual({ planned_minutes: 50, mode: 'LAB', planned_start: Date.UTC(2026, 10, 18, 13, 0), objective_id: 'D15-O2' });
    expect(await screen.findByTestId('planned-session')).toHaveTextContent('0:50:00');
    expect(await screen.findByRole('heading', { level: 2, name: 'UNLINKED: planned by you, no curriculum block' })).toBeInTheDocument();
    const table = screen.getByRole('table', { name: /Planned and recorded sessions/ });
    expect(within(table).getByText('planned by you, no curriculum block')).toBeInTheDocument();
    const transitions = screen.getByRole('group', { name: 'Session transitions' });
    await waitFor(() => {
      expect(within(transitions).getByRole('button', { name: 'Prepare' })).toBeEnabled();
    });
    expect(within(transitions).getByRole('button', { name: 'Start' })).toBeDisabled();
  });

  it('keeps Plan session disabled for minutes outside 1 to 600 or a malformed objective code, and shows the backend refusal', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SessionTracker />);
    const form = await screen.findByRole('form', { name: 'Plan a session' });
    const submit = within(form).getByRole('button', { name: 'Plan session' });
    await waitFor(() => {
      expect(submit).toBeEnabled();
    });
    const minutes = within(form).getByLabelText('Planned minutes');
    await user.clear(minutes);
    await user.type(minutes, '601');
    expect(submit).toBeDisabled();
    await user.clear(minutes);
    await user.type(minutes, '30');
    const objective = within(form).getByLabelText('Objective code (optional)');
    await user.type(objective, 'NO SUCH');
    expect(submit).toBeDisabled();
    await user.clear(objective);
    await user.type(objective, 'NO-SUCH-OBJECTIVE');
    await user.click(submit);
    // The fixture's INTERRUPTED banner is an alert too; the refusal is the one with a category.
    const refusal = await waitFor(() => {
      const found = screen.queryAllByRole('alert').find((a) => a.hasAttribute('data-category'));
      expect(found).toBeDefined();
      return found;
    });
    expect(refusal).toHaveAttribute('data-category', 'NOT_FOUND');
    expect(refusal).toHaveTextContent('objective.not_found');
    expect(screen.queryByTestId('planned-session')).toBeNull();
  });
});
