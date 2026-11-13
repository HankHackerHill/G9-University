/**
 * Planning a session from the Session Tracker (`session.create`): the form sends what the request
 * schema allows, resolves the optional start in the backend's zone (never the webview's), and the
 * new session is listed under "Planned by you" (the backend labels it UNLINKED) and selected.
 * The form opens from the page head's primary action.
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
    await user.click(await screen.findByRole('button', { name: 'Plan a session' }));
    const form = await screen.findByRole('form', { name: 'Plan a session' });
    const submit = within(form).getByRole('button', { name: 'Plan session' });
    // Enabled only once the backend clock (and so the zone) is known.
    await waitFor(() => {
      expect(submit).toBeEnabled();
    });
    const minutes = within(form).getByLabelText('Length in minutes');
    // The composer puts the caret in its first field.
    expect(document.activeElement).toBe(minutes);
    await user.clear(minutes);
    await user.type(minutes, '50');
    fireEvent.change(within(form).getByLabelText('Start (optional)'), { target: { value: '2026-11-18T14:00' } });
    await user.selectOptions(within(form).getByLabelText('Kind'), 'LAB');
    await user.type(within(form).getByLabelText('Objective code (optional)'), 'D15-O2');
    await user.click(submit);

    const call = harness.calls.find((c) => c.op === 'session.create');
    // 14:00 in Europe/Berlin (the mock backend's zone) is 13:00Z.
    expect(call?.payload).toEqual({ planned_minutes: 50, mode: 'LAB', planned_start: Date.UTC(2026, 10, 18, 13, 0), objective_id: 'D15-O2' });
    expect(await screen.findByTestId('planned-session')).toHaveTextContent('0:50:00');
    // The composer closes; the new session is the one on the instrument and the pressed line of its group.
    expect(screen.queryByRole('form', { name: 'Plan a session' })).toBeNull();
    expect(await screen.findByRole('heading', { level: 2, name: 'Your session' })).toBeInTheDocument();
    expect(screen.getByText('Planned by you, not tied to a curriculum block')).toBeInTheDocument();
    const own = screen.getByRole('region', { name: 'Sessions you planned' });
    expect(within(own).getByRole('button', { name: /Your session/ })).toHaveAttribute('aria-pressed', 'true');
    const transitions = screen.getByRole('group', { name: 'Session transitions' });
    await waitFor(() => {
      expect(within(transitions).getByRole('button', { name: 'Prepare' })).toBeEnabled();
    });
    expect(within(transitions).getByRole('button', { name: 'Start' })).toBeDisabled();
  });

  it('keeps Plan session disabled for minutes outside 1 to 600 or a malformed objective code, and shows the backend refusal', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SessionTracker />);
    await user.click(await screen.findByRole('button', { name: 'Plan a session' }));
    const form = await screen.findByRole('form', { name: 'Plan a session' });
    const submit = within(form).getByRole('button', { name: 'Plan session' });
    await waitFor(() => {
      expect(submit).toBeEnabled();
    });
    const minutes = within(form).getByLabelText('Length in minutes');
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
    // A refused plan keeps the form open with what was typed.
    expect(screen.getByRole('form', { name: 'Plan a session' })).toBeInTheDocument();
  });
});
