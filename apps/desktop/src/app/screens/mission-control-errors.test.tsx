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

const stat = (label: string) => screen.getByText(label).closest('.stat');

describe('Mission Control with failing reads', () => {
  it('session.list failing shows the error once, and "No data" tiles instead of a zero count, a credited zero or a "nothing planned" claim', async () => {
    renderWithIpc(<MissionControl />, failing('session.list'));
    await screen.findByText('Distinguish authentication/authorization');
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(1);
    });
    expect(screen.getAllByText(/storage\.busy/).length).toBeGreaterThan(0);
    for (const label of ['Credited today', 'Blocks completed']) {
      expect(stat(label)).toHaveAttribute('data-standing', 'missing');
      expect(stat(label)).toHaveTextContent('No data');
    }
    expect(screen.getByText('Today’s sessions could not be read')).toBeInTheDocument();
    expect(screen.queryByText('Nothing is planned yet')).toBeNull();
    expect(screen.queryByText('Every block is done')).toBeNull();
    expect(screen.queryByText(/0:00:00/)).toBeNull();
    // The program map needs the same records: it says why it is missing and draws nothing.
    expect(screen.getByText(/the program map cannot be drawn/)).toBeInTheDocument();
  });

  it('interventions.list, approvals.list and calendar.list failing show errors instead of empty or no-model states', async () => {
    renderWithIpc(<MissionControl />, failing('interventions.list', 'approvals.list', 'calendar.list'));
    await screen.findByText('Distinguish authentication/authorization');
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(3);
    });
    expect(screen.queryByText('Advice: no model')).toBeNull();
    expect(screen.queryByText(/No hypothesis is supported/)).toBeNull();
    expect(screen.queryByText(/Nothing waits for your approval/)).toBeNull();
    expect(screen.queryByText(/Nothing in the next 48 hours/)).toBeNull();
    expect(stat('Waiting for you')).toHaveAttribute('data-standing', 'missing');
  });

  it('curriculum.daily_plan failing shows the error, not a rest day or an empty objective list (NEW-11)', async () => {
    renderWithIpc(<MissionControl />, failing('curriculum.daily_plan'));
    await screen.findByText('Blocks completed');
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(1);
    });
    expect(screen.queryByText('Rest day')).toBeNull();
    expect(screen.queryByText(/No study blocks are scheduled/)).toBeNull();
    expect(screen.queryByText(/No objectives are scheduled/)).toBeNull();
    // The blocks panel says the plan is unavailable, with the code; it does not draw an empty chart.
    expect(screen.getByText(/Today’s plan could not be read \(storage\.busy\), so its blocks cannot be shown/)).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /Today’s blocks/ })).toBeNull();
  });

  it('with every read succeeding the figures are shown as posted values', async () => {
    renderWithIpc(<MissionControl />, failing());
    expect(await screen.findByText('Blocks completed')).toBeInTheDocument();
    await waitFor(() => {
      expect(stat('Blocks completed')).toHaveAttribute('data-standing', 'posted');
    });
    expect(stat('Credited today')).toHaveAttribute('data-standing', 'posted');
    expect(busyAlerts()).toHaveLength(0);
  });
});
