/**
 * CommandOutcome.warnings is the backend's disclosure channel (PROPOSED behaviour behind a
 * success, non-canonical provenance, bridge host facts). These tests fail when a screen drops it.
 */
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { IpcProvider } from '../ipc-context';
import { SCREENS, routes } from '../router';
import { addWarnings, createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { CurriculumNavigator } from './CurriculumNavigator';
import { DailyClassroom } from './DailyClassroom';
import { MissionControl } from './MissionControl';
import { SettingsRecovery } from './SettingsRecovery';

const KEY_FALLBACK = 'WARNING binding_key.file_fallback: the approval binding key is stored in the documented fallback file, not the OS keyring';
const MIGRATION_BLOCKED = 'WARNING storage.migration_blocked: pre-migration backup failed; storage is read-only';

/**
 * Screens that issue no read at mount: their disclosure path is a mutation, driven here so the
 * intercepted response actually carries the injected warning. Without this the per-screen test
 * below would assert nothing for them.
 */
const DRIVE: Readonly<Record<string, (user: ReturnType<typeof userEvent.setup>) => Promise<void>>> = {
  'Learning Journal': async (user) => {
    await user.type(screen.getByLabelText('Title'), 'disclosure check');
    await user.click(screen.getByRole('button', { name: 'Save note' }));
  },
  'Settings/Recovery': async (user) => {
    await user.click(screen.getByRole('button', { name: 'Create backup' }));
  },
};

/**
 * Screens with no IPC operation besides `system.status` in this build (PROPOSED surfaces): there
 * is no outcome whose warnings could be dropped. `system.status` warnings are covered by the
 * shell/Settings test above.
 */
const NO_IPC: ReadonlySet<string> = new Set(['Executive Workspace', 'Research Center']);

async function settle(): Promise<void> {
  for (let i = 0; i < 20; i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

describe('CommandOutcome.warnings are rendered', () => {
  it('shows system.status warnings (bridge host facts) in the shell status area and in Settings/Recovery', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      return op === 'system.status' ? addWarnings(r, [KEY_FALLBACK, MIGRATION_BLOCKED]) : r;
    });
    const router = createMemoryRouter(routes, { initialEntries: ['/settings'] });
    render(
      <IpcProvider client={harness.client}>
        <RouterProvider router={router} />
      </IpcProvider>,
    );
    const banner = await screen.findByRole('banner');
    const strip = await within(banner).findByTestId('status-warnings');
    expect(within(strip).getByText(KEY_FALLBACK)).toBeInTheDocument();
    expect(within(strip).getByText(MIGRATION_BLOCKED)).toBeInTheDocument();
    expect(await screen.findByRole('heading', { level: 1, name: 'Settings / Recovery' })).toBeInTheDocument();
    const settings = await screen.findByTestId('settings-status-warnings');
    expect(within(settings).getByText(KEY_FALLBACK)).toBeInTheDocument();
    expect(within(settings).getByText(MIGRATION_BLOCKED)).toBeInTheDocument();
  });

  it('shows export.attachments_missing when an approved export reports attachment files missing from the store', async () => {
    const MISSING = 'export.attachments_missing: 1 attachment file(s) were missing or damaged in the store and are listed in manifest.json';
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      return op === 'export.request' ? addWarnings(r, [MISSING]) : r;
    });
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />, harness);
    expect(await screen.findByTestId('status-warnings-none')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Request LEARNING export (JSON)' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await screen.findByTestId('export-result')).toHaveTextContent('READY');
    const warnings = await screen.findByTestId('outcome-warnings');
    expect(within(warnings).getByText(MISSING)).toBeInTheDocument();
  });

  it.each(SCREENS.map((s) => [s.title, s] as const))('%s renders the warnings of every successful read it issues', async (_title, screenDef) => {
    const succeeded = new Set<string>();
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      if (op === 'system.status' || typeof r !== 'object' || r === null || !('result' in r)) {
        return r;
      }
      succeeded.add(op);
      return addWarnings(r, [`test.injected.${op}: disclosure attached to ${op}`]);
    });
    const { Component } = await screenDef.load();
    renderWithIpc(<Component />, harness);
    expect(await screen.findByRole('heading', { level: 1, name: screenDef.title.replace('/', ' / ') })).toBeInTheDocument();
    await settle();
    const drive = DRIVE[screenDef.title];
    if (drive !== undefined) {
      await drive(userEvent.setup());
      await settle();
    }
    if (NO_IPC.has(screenDef.title)) {
      // Explicitly vacuous: the screen must not have started issuing operations without this list being updated.
      expect(
        harness.calls.filter((c) => c.op !== 'system.status').map((c) => c.op),
        'screen listed as NO_IPC issued an operation',
      ).toHaveLength(0);
      return;
    }
    // NEW-10: a screen whose responses were never intercepted would make the loop below assert nothing.
    expect(succeeded.size, `${screenDef.title} issued no successful intercepted operation; the assertions below would be vacuous`).toBeGreaterThan(0);
    for (const op of succeeded) {
      expect(screen.getAllByText(`test.injected.${op}: disclosure attached to ${op}`).length, op).toBeGreaterThan(0);
    }
  });
});

describe('plans from a curriculum version that is not APPROVED_CANONICAL', () => {
  const panelOf = (heading: RegExp): HTMLElement => {
    const h = screen.getByRole('heading', { level: 2, name: heading });
    const panel = h.closest('section');
    if (panel === null) {
      throw new Error('panel not found');
    }
    return panel;
  };

  it('Daily Classroom labels the PROPOSED_SEED plan PROPOSED, never VERIFIED, and shows the backend warning', async () => {
    renderWithIpc(<DailyClassroom />);
    expect(await screen.findByText(/^curriculum\.non_canonical: /)).toBeInTheDocument();
    expect(screen.queryAllByLabelText('Provenance: verified')).toHaveLength(0);
    expect(screen.getAllByLabelText('Provenance: proposed').length).toBeGreaterThanOrEqual(2);
  });

  it('Mission Control labels the non-canonical objectives PROPOSED', async () => {
    renderWithIpc(<MissionControl />);
    await screen.findByText('Distinguish authentication/authorization');
    const objectives = panelOf(/Today’s objectives/);
    expect(within(objectives).getByLabelText('Provenance: proposed')).toBeInTheDocument();
    expect(within(objectives).queryByLabelText('Provenance: verified')).toBeNull();
    expect(screen.getByText(/^curriculum\.non_canonical: /)).toBeInTheDocument();
  });

  it('a plan without the non-canonical warning (APPROVED_CANONICAL version) keeps the VERIFIED label', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      if (op === 'curriculum.daily_plan' && typeof r === 'object' && r !== null && 'result' in r) {
        const rest: Record<string, unknown> = { ...(r as Record<string, unknown>) };
        delete rest.warnings;
        return rest;
      }
      return r;
    });
    renderWithIpc(<DailyClassroom />, harness);
    await waitFor(() => {
      expect(screen.getAllByLabelText('Provenance: verified').length).toBeGreaterThanOrEqual(2);
    });
    expect(screen.queryByText(/curriculum\.non_canonical/)).toBeNull();
  });
});

describe('Curriculum Navigator', () => {
  it('requests each day of the week once instead of re-requesting after every render', async () => {
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CurriculumNavigator />, harness);
    expect(await screen.findByLabelText('Curriculum tree')).toBeInTheDocument();
    await settle();
    expect(harness.calls.filter((c) => c.op === 'curriculum.daily_plan')).toHaveLength(6);
  });
});
