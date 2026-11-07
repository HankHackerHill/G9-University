/**
 * A failed read on Mission Control must surface as an error, never as computed zeros or empty
 * states (Panel.tsx: never substitute a number or a placeholder for missing data; rtm R-014).
 */
import { screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { OperationName } from '../../ipc/operations';
import { STORAGE_BUSY, createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { MissionControl } from './MissionControl';

function failing(...ops: OperationName[]) {
  return createInterceptHarness(async (op, _payload, next) => (ops.includes(op) ? STORAGE_BUSY : next()));
}

const busyAlerts = () => screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE');

describe('Mission Control with failing reads', () => {
  it('session.list failing shows the error and no VERIFIED zero count, credited zero or "no session" claim', async () => {
    renderWithIpc(<MissionControl />, failing('session.list'));
    await screen.findByText('Distinguish authentication/authorization');
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(1);
    });
    expect(screen.getAllByText(/storage\.busy/).length).toBeGreaterThan(0);
    expect(screen.queryByText('blocks completed')).toBeNull();
    expect(screen.queryByText('credited')).toBeNull();
    expect(screen.queryByText('No session planned today')).toBeNull();
    expect(screen.queryByText('No planned block remaining')).toBeNull();
    expect(screen.queryByText('0:00:00')).toBeNull();
  });

  it('interventions.list, approvals.list and calendar.list failing show errors instead of empty or model-unavailable states', async () => {
    renderWithIpc(<MissionControl />, failing('interventions.list', 'approvals.list', 'calendar.list'));
    await screen.findByText('Distinguish authentication/authorization');
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(3);
    });
    expect(screen.queryByText('Hypotheses (model unavailable)')).toBeNull();
    expect(screen.queryByText(/no supported hypotheses/)).toBeNull();
    expect(screen.queryByText(/nothing awaits approval/)).toBeNull();
    expect(screen.queryByText('None in the next 48 h')).toBeNull();
  });

  it('with every read succeeding the counts are shown with their VERIFIED label', async () => {
    renderWithIpc(<MissionControl />, failing());
    expect(await screen.findByText('blocks completed')).toBeInTheDocument();
    expect(busyAlerts()).toHaveLength(0);
  });
});
