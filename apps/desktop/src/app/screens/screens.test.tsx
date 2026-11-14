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
    // Every screen labels what it shows: a provenance stamp (Verified, Proposed, Advice) on its records, or an explicit
    // empty state that says in words why nothing is there (`NoData`, "no model", a "No data" tile). Never a blank.
    await waitFor(() => {
      expect(document.querySelectorAll('[aria-label^="Provenance:"], .nodata, .stat[data-standing="missing"]').length).toBeGreaterThan(0);
    });
    for (const empty of document.querySelectorAll('.nodata')) {
      // An empty state has a title and a reason: two pieces of text at least.
      expect(empty.textContent.trim().length).toBeGreaterThan(20);
    }
    // No error alerts (ErrorDisplay carries data-category); the INTERRUPTED banner is a legitimate alert.
    expect(screen.queryAllByRole('alert').filter((a) => a.hasAttribute('data-category'))).toHaveLength(0);
    unmount();
  });

  const downTransport: IpcTransport = {
    kind: 'tauri',
    invoke: () => Promise.resolve({ category: 'RESOURCE_UNAVAILABLE', code: 'storage.down', message: 'backend not reachable' }),
  };

  it.each(SCREENS.map((s) => [s.title, s] as const))('%s degrades to "No data" / error states when the backend is unavailable', async (_title, screenDef) => {
    const { Component } = await screenDef.load();
    const client = createClient(downTransport, { now: () => 0, uuid: () => 'u' });
    const { unmount } = renderWithIpc(<Component />, { client, transport: createHarness().transport });
    expect(await screen.findByRole('heading', { level: 1, name: screenDef.title.replace('/', ' / ') })).toBeInTheDocument();
    expect(await screen.findAllByText(/No data|Not available|Proposed/)).not.toHaveLength(0);
    unmount();
  });

  it('the shell routes to every screen lazily and shows the host facts (no model, network off) in the status group', async () => {
    const { client } = createHarness();
    const router = createMemoryRouter(routes, { initialEntries: ['/'] });
    render(
      <IpcProvider client={client}>
        <RouterProvider router={router} />
      </IpcProvider>,
    );
    const nav = await screen.findByRole('navigation', { name: 'Screens' });
    expect(within(nav).getAllByRole('link')).toHaveLength(16);
    // Host facts are the operator's settings, not faults: they sit in the sidebar in plain words, with the exact state in the title.
    const status = screen.getByRole('group', { name: 'System status' });
    expect(await within(status).findByTitle(/^Model: UNAVAILABLE \(.*NO_RUNTIME_CONFIGURED\)$/)).toHaveTextContent('Modelnone');
    expect(within(status).getByTitle('Network: OFF')).toHaveTextContent('Networkoff');
    expect(within(status).getByTitle('Storage: READY')).toHaveTextContent('Storageready');
    expect(screen.getByTestId('mock-banner')).toBeInTheDocument();
    // The header is the day: its code and the blocks drawn to scale.
    expect(await within(screen.getByRole('banner')).findByText('D15')).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 1, name: 'Mission Control' })).toBeInTheDocument();
    await userEvent.setup().click(within(nav).getByRole('link', { name: 'Research Center' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Research Center' })).toBeInTheDocument();
    expect(await screen.findByTestId('network-indicator')).toHaveTextContent(/off/i);
  });
});

describe('Mission Control', () => {
  it('renders the next thing to do, the figures, today\'s blocks and objectives, the program map and what waits in the margin', async () => {
    renderWithIpc(<MissionControl />);
    const now = await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(now).toHaveTextContent('NextRecall15 min planned, block 1 of 6');
    });
    expect(within(now).getByRole('link', { name: 'Go to recall' })).toHaveAttribute('href', '/session');
    expect(await screen.findByText('Distinguish authentication/authorization')).toBeInTheDocument();
    // The fixture curriculum is not canonical yet, so the objectives are stamped Proposed.
    expect(await screen.findByLabelText('Provenance: proposed')).toBeInTheDocument();
    // Time drawn to scale, each with its table view.
    expect(await screen.findByRole('list', { name: /Today’s blocks/ })).toBeInTheDocument();
    expect(screen.getByRole('table', { name: /Blocks for D15/ })).toBeInTheDocument();
    expect(await screen.findByRole('img', { name: /The program, day by day: \d+ of 72 days have recorded sessions/ })).toBeInTheDocument();
    expect(screen.getByRole('table', { name: 'Recorded days of the program' })).toBeInTheDocument();
    // The margin: reviews (overdue in red ink), approvals, calendar, and the mentor with no model.
    const margin = screen.getByRole('complementary', { name: 'What waits for you' });
    expect(await within(margin).findByText(/Recall: Explain HTTP request semantics/)).toBeInTheDocument();
    expect(within(margin).getAllByText(/^Overdue since /).length).toBeGreaterThan(0);
    expect(await within(margin).findByText('Nothing waits for your approval.')).toBeInTheDocument();
    expect(await within(margin).findByText('Advice: no model')).toBeInTheDocument();
    expect(within(margin).getByTitle('NO_RUNTIME_CONFIGURED')).toBeInTheDocument();
  });
});

describe('Session Tracker', () => {
  it('shows the interrupted banner, disables illegal transitions and requires a correction', async () => {
    renderWithIpc(<SessionTracker />);
    expect(await screen.findByTestId('interrupted-banner')).toHaveTextContent('One session was interrupted.');
    // The interrupted session is selected and listed first, under what needs the operator.
    const needs = screen.getByRole('region', { name: 'Sessions that need a correction' });
    expect(within(needs).getByRole('button', { pressed: true })).toHaveTextContent('Evidence reporting');
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
    // Today's blocks are listed as lines to pick; Recall is PLANNED in the fixture.
    const todays = await screen.findByRole('region', { name: 'Today’s sessions' });
    await user.click(within(todays).getByRole('button', { name: /Recall/ }));
    const group = screen.getByRole('group', { name: 'Session transitions' });
    expect(within(group).getByRole('button', { name: 'Prepare' })).toBeEnabled();
    expect(within(group).getByRole('button', { name: 'Start' })).toBeDisabled();
    await user.click(within(group).getByRole('button', { name: 'Prepare' }));
    await waitFor(() => {
      expect(within(screen.getByRole('group', { name: 'Session transitions' })).getByRole('button', { name: 'Start' })).toBeEnabled();
    });
    expect(within(screen.getByRole('group', { name: 'Session transitions' })).getByRole('button', { name: 'Prepare' })).toBeDisabled();
    expect(screen.getAllByText('Ready')).not.toHaveLength(0);
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
    expect(within(card).getByTestId('approval-expiry')).toHaveTextContent('2026-11-18 09:40');
    // The request waits for the operator; the backend's own state name stays in the stamp's title.
    expect(within(card).getByText('Waiting for you')).toHaveAttribute('title', 'PENDING');
    expect(screen.getByRole('alert')).toHaveAttribute('data-category', 'APPROVAL_REQUIRED');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    // The approval is spent (CONSUMED) and the attempt is final (APPROVED_FINAL), each in the operator's words.
    expect(await screen.findByTitle('CONSUMED')).toHaveTextContent('Used');
    expect(await screen.findAllByTitle('APPROVED_FINAL')).not.toHaveLength(0);
    expect(screen.getAllByTitle('APPROVED_FINAL')[0]).toHaveTextContent('Final');
  });

  it('lists approval requests with hash and expiry and shows an intact audit chain', async () => {
    const harness = createHarness();
    const r = await harness.client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
    expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
    renderWithIpc(<ApprovalsAudit />, harness);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(within(card).getByTestId('approval-expiry')).toHaveTextContent('2026-11-18 09:40');
    expect(within(card).getByText('assessment.approve_final')).toBeInTheDocument();
    expect((await screen.findByText('Audit chain')).closest('.stat')).toHaveTextContent('Intact');
    expect(screen.getByText('Audit chain').closest('.stat')).toHaveAttribute('data-standing', 'posted');
    expect(screen.getByText('mock.seed')).toBeInTheDocument();
  });
});

describe('Settings/Recovery', () => {
  it('shows no model runtime, drives the backup machine with legal buttons only and persists the contrast toggle', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    expect(await screen.findByTestId('runtime-none')).toHaveTextContent('None configured');
    await user.click(screen.getByRole('button', { name: 'Create backup' }));
    const verify = await screen.findByRole('button', { name: 'Verify' });
    const restore = screen.getByRole('button', { name: 'Test restore' });
    expect(verify).toBeEnabled();
    expect(restore).toBeDisabled();
    await user.click(verify);
    const table = screen.getByRole('table', { name: 'Backups' });
    // CHECKED in the backend's words; the stamp says Verified and keeps the exact name in its title.
    expect(await within(table).findByText('Verified')).toHaveAttribute('title', 'CHECKED');
    expect(within(table).getByRole('button', { name: 'Test restore' })).toBeEnabled();
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeDisabled();
    const toggle = screen.getByRole('button', { name: /High contrast/ });
    await user.click(toggle);
    expect(localStorage.getItem('g9.contrast')).toBe('high');
    expect(document.documentElement.dataset.contrast).toBe('high');
  });
});
