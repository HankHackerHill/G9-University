/**
 * Settings/Recovery is where the operator checks the host facts (binding-key fallback, read-only
 * storage). When `system.status` fails, every panel that depends on it must say so (NEW-8);
 * `useToday().status` is null both while loading and after a failure, so "loading" is wrong.
 */
import { screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { STORAGE_BUSY, createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
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
