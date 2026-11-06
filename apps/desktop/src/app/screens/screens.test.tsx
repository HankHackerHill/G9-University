import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { createClient, isG9Error, type IpcTransport } from '../../ipc/client';
import { IpcProvider } from '../ipc-context';
import { SCREENS, routes } from '../router';
import { createHarness, renderWithIpc } from '../../test/render';
import { ApprovalsAudit } from './ApprovalsAudit';
import { Assessments } from './Assessments';
import { MissionControl } from './MissionControl';
import { SessionTracker } from './SessionTracker';
import { SettingsRecovery } from './SettingsRecovery';

describe('all 16 screens render against the mock transport', () => {
  it('registers exactly the 16 screens of ARCHITECTURE §12', () => {
    expect(SCREENS.map((s) => s.title)).toEqual([
      'Mission Control',
      'Daily Classroom',
      'Calendar',
      'Curriculum Navigator',
      'Session Tracker',
      'Competency Graph',
      'Assessments',
      'Retention Center',
      'Intervention Center',
      'Learning Journal',
      'Performance Dashboard',
      'Executive Workspace',
      'Mentor Memory Explorer',
      'Research Center',
      'Approvals/Audit',
      'Settings/Recovery',
    ]);
  });

  it.each(SCREENS.map((s) => [s.title, s] as const))('%s renders against fixture data with provenance or explicit empty states', async (_title, screenDef) => {
    const { Component } = await screenDef.load();
    const { unmount } = renderWithIpc(<Component />);
    expect(await screen.findByRole('heading', { level: 1, name: screenDef.title.replace('/', ' / ') })).toBeInTheDocument();
    // Every screen labels what it shows: VERIFIED fixture data, PROPOSED items, or an explicit NO DATA / model-unavailable state. Never a blank.
    expect(await screen.findAllByText(/NO DATA|model unavailable|UNAVAILABLE|NONE CONFIGURED|OFF|VERIFIED|PROPOSED|PROVISIONAL/)).not.toHaveLength(0);
    // No error alerts (ErrorDisplay carries data-category); the INTERRUPTED banner is a legitimate alert.
    expect(screen.queryAllByRole('alert').filter((a) => a.hasAttribute('data-category'))).toHaveLength(0);
    unmount();
  });

  const downTransport: IpcTransport = {
    kind: 'tauri',
    invoke: () => Promise.resolve({ category: 'RESOURCE_UNAVAILABLE', code: 'storage.down', message: 'backend not reachable' }),
  };

  it.each(SCREENS.map((s) => [s.title, s] as const))('%s degrades to NO DATA / error states when the backend is unavailable', async (_title, screenDef) => {
    const { Component } = await screenDef.load();
    const client = createClient(downTransport, { now: () => 0, uuid: () => 'u' });
    const { unmount } = renderWithIpc(<Component />, { client, transport: createHarness().transport });
    expect(await screen.findByRole('heading', { level: 1, name: screenDef.title.replace('/', ' / ') })).toBeInTheDocument();
    expect(await screen.findAllByText(/NO DATA|Resource unavailable|PROPOSED/)).not.toHaveLength(0);
    unmount();
  });

  it('the shell routes to every screen lazily and shows model UNAVAILABLE in the status strip', async () => {
    const { client } = createHarness();
    const router = createMemoryRouter(routes, { initialEntries: ['/'] });
    render(
      <IpcProvider client={client}>
        <RouterProvider router={router} />
      </IpcProvider>,
    );
    const nav = await screen.findByRole('navigation', { name: 'Screens' });
    expect(within(nav).getAllByRole('link')).toHaveLength(16);
    const banner = screen.getByRole('banner');
    expect(await within(banner).findByText(/UNAVAILABLE · NO_RUNTIME_CONFIGURED/)).toBeInTheDocument();
    expect(within(banner).getByText('MOCK')).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 1, name: 'Mission Control' })).toBeInTheDocument();
    await userEvent.setup().click(within(nav).getByRole('link', { name: 'Research Center' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Research Center' })).toBeInTheDocument();
    expect(await screen.findByTestId('network-indicator')).toHaveTextContent('OFF');
  });
});

describe('Mission Control', () => {
  it('renders the model-unavailable teaching panel, today\'s fixture objectives and NO DATA for hypotheses', async () => {
    renderWithIpc(<MissionControl />);
    expect(await screen.findByText('Recommendation (model unavailable)')).toBeInTheDocument();
    expect(await screen.findByText('Distinguish authentication/authorization')).toBeInTheDocument();
    expect(screen.getByText(/UNAVAILABLE · NO_RUNTIME_CONFIGURED/)).toBeInTheDocument();
    expect(screen.getByText('OFFLINE')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Lab' })).toBeInTheDocument();
    expect(await screen.findByText('Hypotheses (model unavailable)')).toBeInTheDocument();
    expect(await screen.findByText(/Recall: Explain HTTP request semantics/)).toBeInTheDocument();
    expect(screen.getByText(/executive operations are PROPOSED/)).toBeInTheDocument();
  });
});

describe('Session Tracker', () => {
  it('shows the INTERRUPTED banner, disables illegal transitions and requires a correction', async () => {
    renderWithIpc(<SessionTracker />);
    expect(await screen.findByTestId('interrupted-banner')).toHaveTextContent('INTERRUPTED');
    const group = await screen.findByRole('group', { name: 'Session transitions' });
    expect(within(group).getByRole('button', { name: 'Start' })).toBeDisabled();
    expect(within(group).getByRole('button', { name: 'Resume' })).toBeDisabled();
    expect(within(group).getByRole('button', { name: 'Pause' })).toBeDisabled();
    expect(within(group).getByRole('button', { name: 'Complete' })).toBeEnabled();
    expect(screen.getByRole('form', { name: 'Time correction' })).toBeInTheDocument();
    expect(screen.getByTestId('uncertain')).toHaveTextContent('0:40:00');
    expect(screen.getByTestId('credited')).toHaveTextContent('0:00:00');
  });

  it('enables only legal buttons as the backend state advances', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SessionTracker />);
    const rows = await screen.findAllByRole('button', { name: 'Select' });
    // Pick a PLANNED session (row 0 is D13 recall in the fixture, which is PLANNED).
    const first = rows[0];
    if (first === undefined) {
      throw new Error('no session rows');
    }
    await user.click(first);
    const group = screen.getByRole('group', { name: 'Session transitions' });
    expect(within(group).getByRole('button', { name: 'Prepare' })).toBeEnabled();
    expect(within(group).getByRole('button', { name: 'Start' })).toBeDisabled();
    await user.click(within(group).getByRole('button', { name: 'Prepare' }));
    await waitFor(() => {
      expect(within(screen.getByRole('group', { name: 'Session transitions' })).getByRole('button', { name: 'Start' })).toBeEnabled();
    });
    expect(within(screen.getByRole('group', { name: 'Session transitions' })).getByRole('button', { name: 'Prepare' })).toBeDisabled();
    expect(screen.getAllByText('READY')).not.toHaveLength(0);
  });
});

describe('Assessments and Approvals/Audit', () => {
  it('approve_final triggers APPROVAL_REQUIRED and shows the approval card with the operation hash and expiry', async () => {
    const user = userEvent.setup();
    renderWithIpc(<Assessments />);
    const approve = await screen.findByRole('button', { name: 'Approve final' });
    expect(approve).toBeEnabled();
    await user.click(approve);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(within(card).getByTestId('approval-expiry')).toHaveTextContent('2026-11-18 09:45');
    expect(within(card).getByText('PENDING')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveAttribute('data-category', 'APPROVAL_REQUIRED');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await screen.findByText('CONSUMED')).toBeInTheDocument();
    expect(await screen.findAllByText('APPROVED_FINAL')).not.toHaveLength(0);
  });

  it('lists approval requests with hash and expiry and shows an intact audit chain', async () => {
    const harness = createHarness();
    const r = await harness.client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
    expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
    renderWithIpc(<ApprovalsAudit />, harness);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(within(card).getByTestId('approval-expiry')).toHaveTextContent('2026-11-18 09:45');
    expect(within(card).getByText('assessment.approve_final')).toBeInTheDocument();
    expect(await screen.findByText('INTACT')).toBeInTheDocument();
    expect(screen.getByText('mock.seed')).toBeInTheDocument();
  });
});

describe('Settings/Recovery', () => {
  it('shows no model runtime, drives the backup machine with legal buttons only and persists the contrast toggle', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    expect(await screen.findByTestId('runtime-none')).toHaveTextContent('NONE CONFIGURED');
    await user.click(screen.getByRole('button', { name: 'Create backup' }));
    const verify = await screen.findByRole('button', { name: 'Verify' });
    const restore = screen.getByRole('button', { name: 'Restore test' });
    expect(verify).toBeEnabled();
    expect(restore).toBeDisabled();
    await user.click(verify);
    const table = screen.getByRole('table', { name: 'Backups in this session' });
    expect(await within(table).findByText('CHECKED')).toBeInTheDocument();
    expect(within(table).getByRole('button', { name: 'Restore test' })).toBeEnabled();
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeDisabled();
    const toggle = screen.getByRole('button', { name: /High contrast/ });
    await user.click(toggle);
    expect(localStorage.getItem('g9.contrast')).toBe('high');
    expect(document.documentElement.dataset.contrast).toBe('high');
  });
});
