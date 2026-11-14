/**
 * Settings/Recovery is where the operator checks the host facts (binding-key fallback, read-only
 * storage). When `system.status` fails, every section that depends on it must say so (NEW-8);
 * `useToday().status` is null both while loading and after a failure, so "loading" is wrong.
 *
 * The rest follows the page: backups driven through the legal steps only, the export request sent
 * with exactly what was chosen, and the facts about the host said in words.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { RequestOf, SystemStatus } from '../../ipc/operations';
import { MOCK_EXPORT_DIR } from '../../ipc/transports/mock';
import { STORAGE_BUSY, addWarnings, createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { SettingsRecovery } from './SettingsRecovery';

/** A harness whose `system.status` answer is changed before the screen sees it. */
function statusHarness(change: (status: SystemStatus) => SystemStatus) {
  return createInterceptHarness(async (op, _payload, next) => {
    const r = await next();
    if (op !== 'system.status' || typeof r !== 'object' || r === null || !('result' in r)) {
      return r;
    }
    return { ...r, result: change(r.result as SystemStatus) };
  });
}

describe('Settings/Recovery when system.status fails', () => {
  it('reports the last backup, the model and the facts about this window as unavailable with the code, never as still loading', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => (op === 'system.status' ? STORAGE_BUSY : next()));
    renderWithIpc(<SettingsRecovery />, harness);
    await waitFor(() => {
      expect(screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE')).toHaveLength(1);
    });
    expect(screen.queryByText('Loading the last backup…')).toBeNull();
    expect(screen.queryByText('Loading the model setting…')).toBeNull();
    expect(screen.queryByText('Loading the facts about this window…')).toBeNull();
    expect(screen.getByText(/The last tested backup could not be read \(storage\.busy\)/)).toBeInTheDocument();
    expect(screen.getByText(/The model setting could not be read \(storage\.busy\)/)).toBeInTheDocument();
    expect(screen.getByText(/The facts about this window could not be read \(storage\.busy\)/)).toBeInTheDocument();
    // No claim about the host is made on the failed read.
    expect(screen.queryByTestId('status-warnings-none')).toBeNull();
    expect(screen.queryByTestId('settings-status-warnings')).toBeNull();
    expect(screen.queryByText('No backup has been verified and restore-tested yet.')).toBeNull();
    expect(screen.queryByTestId('runtime-none')).toBeNull();
    expect(screen.queryByTestId('storage-alert')).toBeNull();
  });

  it('shows the loading states only while the read is pending, then the facts', async () => {
    renderWithIpc(<SettingsRecovery />);
    expect(await screen.findByTestId('status-warnings-none')).toHaveTextContent('None reported');
    expect(screen.queryByText(/could not be read/)).toBeNull();
    expect(screen.getByTestId('runtime-none')).toBeInTheDocument();
    const about = screen.getByRole('region', { name: 'About this window' });
    expect(about).toHaveTextContent('Built-in mock backend: fixture data, nothing is stored.');
    expect(within(about).getByLabelText('State: Ready')).toBeInTheDocument();
    expect(about).toHaveTextContent('Time zoneEurope/Berlin');
    expect(about).toHaveTextContent('2026-11-18 09:30');
  });
});

describe('Settings/Recovery host facts', () => {
  it('storage that is read-only or unavailable is an alert; ready storage is a plain fact', async () => {
    const readOnly = renderWithIpc(<SettingsRecovery />, statusHarness((s) => ({ ...s, storage: { ...s.storage, state: 'READ_ONLY' } })));
    const alert = await screen.findByTestId('storage-alert');
    expect(alert).toHaveAttribute('role', 'alert');
    expect(alert).toHaveTextContent('Storage is read-only. Changes are refused until it is writable again.');
    expect(within(screen.getByRole('region', { name: 'About this window' })).getByLabelText('State: Read-only')).toBeInTheDocument();
    readOnly.unmount();

    const unavailable = renderWithIpc(<SettingsRecovery />, statusHarness((s) => ({ ...s, storage: { ...s.storage, state: 'UNAVAILABLE' } })));
    expect(await screen.findByTestId('storage-alert')).toHaveTextContent('Storage is unavailable. Records cannot be read or saved until it is back.');
    unavailable.unmount();

    renderWithIpc(<SettingsRecovery />);
    expect(await screen.findByTestId('status-warnings-none')).toBeInTheDocument();
    expect(screen.queryByTestId('storage-alert')).toBeNull();
  });

  it('no model is a setting: "None configured" with what works without one; other reasons and a configured model are said as they are', async () => {
    const none = renderWithIpc(<SettingsRecovery />);
    const runtime = await screen.findByTestId('runtime-none');
    expect(runtime).toHaveTextContent('None configured');
    expect(runtime).toHaveTextContent('This is a setting, not a fault.');
    expect(runtime).toHaveTextContent('Planning, notes, timers and reviews work without a model');
    expect(within(runtime).getByTitle('NO_RUNTIME_CONFIGURED')).toBeInTheDocument();
    none.unmount();

    const failing = renderWithIpc(<SettingsRecovery />, statusHarness((s) => ({ ...s, model: { available: false, reason: 'RUNTIME_ERROR' } })));
    const broken = await screen.findByTestId('runtime-none');
    expect(broken).toHaveTextContent('Not available');
    expect(broken).toHaveTextContent('The backend gives the reason as runtime error.');
    expect(broken).not.toHaveTextContent('None configured');
    expect(broken).not.toHaveTextContent('not a fault');
    failing.unmount();

    renderWithIpc(<SettingsRecovery />, statusHarness((s) => ({ ...s, model: { available: true, model_id: 'fixture-model-q4' } })));
    const model = await screen.findByRole('region', { name: 'Model' });
    expect(await within(model).findByText('fixture-model-q4')).toBeInTheDocument();
    expect(model).toHaveTextContent('Available');
    expect(screen.queryByTestId('runtime-none')).toBeNull();
  });
});

describe('Settings/Recovery backups', () => {
  it('says no backup is recorded when the database holds none', async () => {
    renderWithIpc(<SettingsRecovery />);
    const backups = screen.getByRole('region', { name: 'Backups' });
    expect(await within(backups).findByText('No backups yet')).toBeInTheDocument();
    expect(backups).toHaveTextContent('No backup is recorded. Create one before you change anything you cannot redo.');
    expect(await within(backups).findByText('No backup has been verified and restore-tested yet.')).toBeInTheDocument();
    expect(screen.queryByRole('table', { name: 'Backups' })).toBeNull();
  });

  it('lists the backups the database records, made in an earlier session, newest first, and steps them', async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    const first = await harness.client.invoke('backup.create', { label: 'before the import' });
    harness.transport.clock.advance(3_600_000);
    const second = await harness.client.invoke('backup.create', {});
    if ('category' in first || 'category' in second) {
      throw new Error('could not seed the backups');
    }
    renderWithIpc(<SettingsRecovery />, harness);
    const table = await screen.findByRole('table', { name: 'Backups' });
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent(second.result.backup_id);
    expect(rows[1]).toHaveTextContent(first.result.backup_id);
    // A backup of an earlier session is verified from the list like any other.
    const earlier = rows[1];
    expect(earlier).toBeDefined();
    if (earlier === undefined) {
      return;
    }
    await user.click(within(earlier).getByRole('button', { name: 'Verify' }));
    await waitFor(() => {
      expect(within(screen.getByRole('table', { name: 'Backups' })).getAllByText('Verified').length).toBe(1);
    });
  });

  it('shows the backend refusal when the stored backups cannot be read and keeps the page usable', async () => {
    const harness = createInterceptHarness((op, _payload, next) => (op === 'backup.list' ? Promise.resolve(STORAGE_BUSY) : next()));
    renderWithIpc(<SettingsRecovery />, harness);
    expect(await screen.findByText(/The stored backups could not be read \(storage\.busy\)/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create backup' })).toBeEnabled();
  });

  it('drives the backup through its legal steps only, with a receipt for each and the record revision on every step', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<SettingsRecovery />, harness);
    await user.click(screen.getByRole('button', { name: 'Create backup' }));
    expect(await screen.findByTestId('backup-receipt')).toHaveTextContent(/^Backup created: mock-backup-\d+, Snapshot taken\.$/);
    const table = screen.getByRole('table', { name: 'Backups' });
    expect(within(table).getByLabelText('State: Snapshot taken')).toBeInTheDocument();
    expect(within(table).getByText('Not checked')).toBeInTheDocument();
    expect(within(table).getByText('Restore not tested')).toBeInTheDocument();
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeEnabled();
    expect(within(table).getByRole('button', { name: 'Test restore' })).toBeDisabled();
    // Each step button is tied to its backup for assistive technology.
    expect(within(table).getByRole('button', { name: 'Verify' })).toHaveAccessibleDescription(/^mock-backup-\d+$/);

    await user.click(within(table).getByRole('button', { name: 'Verify' }));
    expect(await within(table).findByLabelText('State: Verified')).toBeInTheDocument();
    // The state is said once in the table, in the operator's word; the integrity result has its own.
    expect(within(table).getByText('Verified')).toBeInTheDocument();
    expect(within(table).getByText('Passed')).toBeInTheDocument();
    expect(screen.getByTestId('backup-receipt')).toHaveTextContent(/^Backup verified: mock-backup-\d+, Verified\.$/);
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeDisabled();
    expect(within(table).getByRole('button', { name: 'Test restore' })).toBeEnabled();

    await user.click(within(table).getByRole('button', { name: 'Test restore' }));
    expect(await within(table).findByLabelText('State: Restore tested')).toBeInTheDocument();
    expect(within(table).getByText('Restore tested 2026-11-18 09:30')).toBeInTheDocument();
    expect(screen.getByTestId('backup-receipt')).toHaveTextContent(/^Restore tested: mock-backup-\d+, Restore tested\.$/);
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeDisabled();
    expect(within(table).getByRole('button', { name: 'Test restore' })).toBeDisabled();

    // The steps carried the revision the backend last returned, so a stale view cannot move a backup.
    expect(harness.calls.find((c) => c.op === 'backup.create')?.payload).toEqual({ label: 'manual' });
    expect(harness.calls.find((c) => c.op === 'backup.verify')?.meta.expected_revision).toBe(2);
    expect(harness.calls.find((c) => c.op === 'backup.restore_test')?.meta.expected_revision).toBe(3);
    // The status is read again after each step, so the last backup on record follows.
    expect(await screen.findByTestId('last-backup')).toHaveTextContent(/Last verified and restore-tested backup: mock-backup-\d+ Restore tested 2026-11-18 09:30/);
  });

  it('a refused step shows the backend error and posts no receipt', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness(async (op, _payload, next) => (op === 'backup.verify' ? STORAGE_BUSY : next()));
    renderWithIpc(<SettingsRecovery />, harness);
    await user.click(screen.getByRole('button', { name: 'Create backup' }));
    const table = await screen.findByRole('table', { name: 'Backups' });
    await user.click(within(table).getByRole('button', { name: 'Verify' }));
    const refusal = await waitFor(() => {
      const found = screen.queryAllByRole('alert').find((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE');
      expect(found).toBeDefined();
      return found;
    });
    expect(refusal).toHaveTextContent('storage.busy');
    expect(screen.queryByTestId('backup-receipt')).toBeNull();
    // The backup is where the backend left it.
    expect(within(table).getByLabelText('State: Snapshot taken')).toBeInTheDocument();
    expect(within(table).getByRole('button', { name: 'Verify' })).toBeEnabled();
  });
});

describe('Settings/Recovery restore guidance', () => {
  it('says that in-app restore is not available and names the offline command and its safeguards', async () => {
    renderWithIpc(<SettingsRecovery />);
    const guidance = await screen.findByTestId('restore-guidance');
    expect(guidance).toHaveTextContent(/not available in this window/);
    expect(guidance).toHaveTextContent(/g9 restore/);
    expect(guidance).toHaveTextContent(/--confirm-replace-live-database/);
    expect(guidance).toHaveTextContent(/never deleted/);
  });
});

describe('Settings/Recovery export', () => {
  async function approveExport(user: ReturnType<typeof userEvent.setup>): Promise<HTMLElement> {
    await user.click(await screen.findByRole('button', { name: 'Request export' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    return card;
  }

  const sentExports = (harness: ReturnType<typeof createInterceptHarness>) => harness.calls.filter((c) => c.op === 'export.request');

  it('shows the written export: folder, rows, files, attachments and the manifest hash', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<SettingsRecovery />, harness);
    await approveExport(user);
    const result = await screen.findByTestId('export-result');
    expect(within(result).getByText('Ready')).toBeInTheDocument();
    expect(result).toHaveTextContent('Export written: Learning as JSON');
    expect(within(result).getByTestId('export-location').textContent.startsWith(`${MOCK_EXPORT_DIR}/mock-export-`)).toBe(true);
    expect(within(result).getByTestId('export-manifest-sha256')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(result).toHaveTextContent(/Rows\s*\d+/);
    expect(result).toHaveTextContent(/Files\s*2 \(manifest\.json included\)/);
    expect(result).toHaveTextContent(/Attached files copied\s*0/);
    // What the panel says about the export is what the backend does.
    expect(document.body).not.toHaveTextContent(/no file was produced/);
    expect(document.body).toHaveTextContent('The manifest is not signed.');
    expect(document.body).toHaveTextContent('Secret records are never exported.');
    expect(screen.queryByTestId('outcome-warnings')).toBeNull();
    // The form's defaults are the narrowest request, and the approved retry carries exactly the same values.
    const sent = sentExports(harness);
    expect(sent.map((c) => c.payload)).toEqual([
      { domains: ['LEARNING'], format: 'JSON', include_private: false },
      { domains: ['LEARNING'], format: 'JSON', include_private: false },
    ]);
    expect(sent[0]?.meta.approval_id).toBeUndefined();
    expect(sent[1]?.meta.approval_id).toMatch(/^mock-approval-\d+$/);
  });

  it('shows the backend disclosure of a written export beside the result', async () => {
    const MISSING = 'export.attachments_missing: 1 attachment file(s) were missing or damaged in the store and are listed in manifest.json';
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      return op === 'export.request' ? addWarnings(r, [MISSING]) : r;
    });
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />, harness);
    await approveExport(user);
    expect(await screen.findByTestId('export-result')).toHaveTextContent('Ready');
    const warnings = await screen.findByTestId('outcome-warnings');
    expect(within(warnings).getByText(MISSING)).toBeInTheDocument();
    // The plain sentence the window has for this code is said first; the backend's own sentence stays.
    expect(warnings).toHaveTextContent('The export was written, but some attached files were missing or damaged.');
    expect(within(screen.getByRole('region', { name: 'Export your data' })).getByTestId('outcome-warnings')).toBe(warnings);
  });

  it('without an export directory the approved request is refused and the approval stays approved', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />, createHarness({ exportDir: null }));
    const card = await approveExport(user);
    const refusal = await waitFor(() => {
      const found = screen.queryAllByRole('alert').find((a) => a.getAttribute('data-category') === 'PRECONDITION_FAILED');
      expect(found).toBeDefined();
      return found;
    });
    expect(refusal).toHaveTextContent('export.dir_not_configured');
    expect(await within(card).findByText('Approved')).toBeInTheDocument();
    expect(within(card).getByRole('button', { name: 'Apply with approval' })).toBeInTheDocument();
    expect(screen.queryByTestId('export-result')).toBeNull();
  });

  it('sends exactly the areas, the format and the private choice that were chosen, and cannot be sent with no area', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<SettingsRecovery />, harness);
    const form = screen.getByRole('form', { name: 'Export your data' });
    const request = within(form).getByRole('button', { name: 'Request export' });
    const areas = within(form).getByRole('group', { name: 'Areas' });
    // Every area of the backend is offered, in the operator's words; only Learning is ticked.
    expect(within(areas).getAllByRole('checkbox').map((c) => (c as HTMLInputElement).checked)).toEqual([true, false, false, false, false]);
    expect(within(form).getByRole('checkbox', { name: 'Include private records' })).not.toBeChecked();
    expect(within(form).getByRole('checkbox', { name: 'Include private records' })).toHaveAccessibleDescription(/Secret records are never exported\./);
    expect(within(form).getByRole('button', { name: 'JSON' })).toHaveAttribute('aria-pressed', 'true');

    await user.click(within(areas).getByRole('checkbox', { name: 'Learning' }));
    expect(request).toBeDisabled();
    expect(within(form).getByRole('alert')).toHaveTextContent('Tick at least one area.');
    // Ticked out of order; the request lists them in the backend's order.
    await user.click(within(areas).getByRole('checkbox', { name: 'System' }));
    await user.click(within(areas).getByRole('checkbox', { name: 'Business' }));
    await user.click(within(form).getByRole('checkbox', { name: 'Include private records' }));
    await user.click(within(form).getByRole('button', { name: 'CSV' }));
    expect(request).toBeEnabled();
    await user.click(request);

    const card = await screen.findByTestId('approval-card');
    const payload = sentExports(harness)[0]?.payload as RequestOf<'export.request'>;
    expect(payload).toEqual({ domains: ['BUSINESS', 'SYSTEM'], format: 'CSV', include_private: true });
    // The voucher names what the approval covers.
    expect(within(card).getByText('export:BUSINESS.SYSTEM:PRIVATE:CSV')).toBeInTheDocument();
    expect(screen.queryByTestId('export-result')).toBeNull();
  });

  it('a cancelled confirmation writes nothing and the request keeps waiting', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />, createHarness({ confirmApproval: () => false }));
    await user.click(await screen.findByRole('button', { name: 'Request export' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('flow-confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    expect(within(card).getByRole('button', { name: 'Approve' })).toBeEnabled();
    expect(screen.queryByTestId('export-result')).toBeNull();
  });
});

describe('Settings/Recovery page', () => {
  it('"On this page" moves to a section by scrolling the page and focusing its heading, never by a link that would change the route', async () => {
    const user = userEvent.setup();
    // In the application the shell's main region scrolls; here a stand-in with the same overflow.
    renderWithIpc(
      <div style={{ overflowY: 'auto' }} data-testid="scroller">
        <SettingsRecovery />
      </div>,
    );
    const scroller = screen.getByTestId('scroller');
    const contents = screen.getByRole('navigation', { name: 'On this page' });
    expect(within(contents).queryAllByRole('link')).toHaveLength(0);
    expect(within(contents).getAllByRole('button').map((b) => b.textContent)).toEqual(['Backups', 'Export your data', 'Program', 'Model', 'Appearance', 'About this window']);
    // jsdom has no layout: place the last section 900 px below the top of the scroller.
    const about = screen.getByRole('region', { name: 'About this window' });
    vi.spyOn(about, 'getBoundingClientRect').mockReturnValue({ x: 0, y: 900, top: 900, bottom: 1100, left: 0, right: 600, width: 600, height: 200, toJSON: () => ({}) });
    await user.click(within(contents).getByRole('button', { name: 'About this window' }));
    expect(document.activeElement).toBe(screen.getByRole('heading', { level: 2, name: 'About this window' }));
    expect(scroller.scrollTop).toBe(900);
    await user.click(within(contents).getByRole('button', { name: 'Backups' }));
    expect(document.activeElement).toBe(screen.getByRole('heading', { level: 2, name: 'Backups' }));
  });

  it('offers the colour scheme and the contrast as exclusive choices; "High contrast" is a pressed button when on', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    const contrast = screen.getByRole('group', { name: 'Contrast' });
    const high = within(contrast).getByRole('button', { name: 'High contrast' });
    const colours = within(contrast).getByRole('button', { name: 'Theme colours' });
    expect(high).toHaveAttribute('aria-pressed', 'false');
    expect(colours).toHaveAttribute('aria-pressed', 'true');
    await user.click(high);
    expect(high).toHaveAttribute('aria-pressed', 'true');
    expect(colours).toHaveAttribute('aria-pressed', 'false');
    expect(document.documentElement.dataset.contrast).toBe('high');
    expect(localStorage.getItem('g9.contrast')).toBe('high');
    await user.click(colours);
    expect(high).toHaveAttribute('aria-pressed', 'false');
    expect(document.documentElement.dataset.contrast).toBe('default');
    const scheme = screen.getByRole('group', { name: 'Colour scheme' });
    await user.click(within(scheme).getByRole('button', { name: 'Dark' }));
    expect(within(scheme).getByRole('button', { name: 'Dark' })).toHaveAttribute('aria-pressed', 'true');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});
