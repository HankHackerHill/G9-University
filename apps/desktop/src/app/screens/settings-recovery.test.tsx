/**
 * Settings/Recovery is where the operator checks the host facts (binding-key fallback, read-only
 * storage). When `system.status` fails, every panel that depends on it must say so (NEW-8);
 * `useToday().status` is null both while loading and after a failure, so "loading" is wrong.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { MOCK_EXPORT_DIR } from '../../ipc/transports/mock';
import { STORAGE_BUSY, createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { SettingsRecovery } from './SettingsRecovery';

describe('Settings/Recovery when system.status fails', () => {
  it('reports the host and storage warnings, runtime and transport panels as unavailable with the code, never as still loading', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => (op === 'system.status' ? STORAGE_BUSY : next()));
    renderWithIpc(<SettingsRecovery />, harness);
    await waitFor(() => {
      expect(screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE')).toHaveLength(1);
    });
    expect(screen.queryByText('Loading status warnings…')).toBeNull();
    expect(screen.queryByText('Loading runtime status…')).toBeNull();
    expect(screen.queryByText('Loading status…')).toBeNull();
    expect(screen.queryByText('Loading transport status…')).toBeNull();
    expect(screen.getByText(/status warnings unavailable: system\.status failed \(storage\.busy\)/)).toBeInTheDocument();
    expect(screen.getByText(/runtime status unavailable: system\.status failed \(storage\.busy\)/)).toBeInTheDocument();
    expect(screen.getByText(/transport status unavailable: system\.status failed \(storage\.busy\)/)).toBeInTheDocument();
    // No claim about the host is made on the failed read.
    expect(screen.queryByTestId('status-warnings-none')).toBeNull();
    expect(screen.queryByTestId('settings-status-warnings')).toBeNull();
  });

  it('shows the loading states only while the read is pending, then the facts', async () => {
    renderWithIpc(<SettingsRecovery />);
    expect(await screen.findByTestId('status-warnings-none')).toBeInTheDocument();
    expect(screen.queryByText(/system\.status failed/)).toBeNull();
    expect(screen.getByTestId('runtime-none')).toBeInTheDocument();
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
    await user.click(await screen.findByRole('button', { name: 'Request LEARNING export (JSON)' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    return card;
  }

  it('shows the written export: folder, rows, files, attachments and the manifest hash', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    await approveExport(user);
    const result = await screen.findByTestId('export-result');
    expect(within(result).getByText('READY')).toBeInTheDocument();
    expect(within(result).getByTestId('export-location').textContent.startsWith(`${MOCK_EXPORT_DIR}/mock-export-`)).toBe(true);
    expect(within(result).getByTestId('export-manifest-sha256')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(result).toHaveTextContent(/Rows\s*\d+/);
    expect(result).toHaveTextContent(/Files\s*2 \(manifest\.json included\)/);
    expect(result).toHaveTextContent(/Attachment files copied\s*0/);
    // What the panel says about the export is what the backend does.
    expect(document.body).not.toHaveTextContent(/no file was produced/);
    expect(document.body).toHaveTextContent('The manifest is not signed.');
    expect(document.body).toHaveTextContent('SECRET rows are never exported.');
    expect(screen.queryByTestId('outcome-warnings')).toBeNull();
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
    expect(await within(card).findByText('APPROVED')).toBeInTheDocument();
    expect(within(card).getByRole('button', { name: 'Apply with approval' })).toBeInTheDocument();
    expect(screen.queryByTestId('export-result')).toBeNull();
  });
});
