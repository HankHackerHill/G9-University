/**
 * app-7: the state badge of a day's objective says what the sessions linked to it did. The
 * backend derives it (`curriculum::objective_status`): PLANNED while no linked session has
 * started, IN_PROGRESS once one has, COMPLETED when every linked session is completed. It used
 * to be the constant PLANNED.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { MissionControl } from './MissionControl';

function badgeOf(objective: string): HTMLElement {
  const row = screen.getByText(objective).closest('li');
  if (row === null) {
    throw new Error(`no row for ${objective}`);
  }
  return within(row).getByText(/Planned|In progress|Completed/);
}

describe('Mission Control: objective status', () => {
  it('shows In progress for an objective whose linked session started, Planned for the others', async () => {
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const created = await harness.client.invoke('session.create', { planned_minutes: 30, objective_id: 'D15-O1' });
    if (isG9Error(created)) {
      throw new Error(created.code);
    }
    const id = created.result.session_id;
    for (const op of ['session.prepare', 'session.start'] as const) {
      const r = await harness.client.invoke(op, { session_id: id });
      if (isG9Error(r)) {
        throw new Error(r.code);
      }
    }
    renderWithIpc(<MissionControl />, harness);
    await waitFor(() => {
      expect(screen.getByText('D15-O1')).toBeInTheDocument();
    });
    expect(badgeOf('D15-O1')).toHaveTextContent('In progress');
    expect(badgeOf('D15-O2')).toHaveTextContent('Planned');
  });
});
