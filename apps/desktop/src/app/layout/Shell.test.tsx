import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { IpcProvider } from '../ipc-context';
import { Shell } from './Shell';
import { createHarness } from '../../test/render';

describe('Shell transport banner', () => {
  it('shows a persistent SYNTHETIC DATA banner when the mock transport is in use', () => {
    const { client } = createHarness();
    expect(client.transportKind).toBe('mock');
    render(
      <IpcProvider client={client}>
        <MemoryRouter>
          <Shell />
        </MemoryRouter>
      </IpcProvider>,
    );
    const banner = screen.getByTestId('mock-banner');
    expect(banner).toHaveTextContent(/SYNTHETIC DATA/);
    expect(banner).toHaveTextContent(/read from, or saved to, the G9 database/);
  });

  it('shows no banner for the Tauri transport', () => {
    const { client } = createHarness();
    const tauriLike = { ...client, transportKind: 'tauri' as const };
    render(
      <IpcProvider client={tauriLike}>
        <MemoryRouter>
          <Shell />
        </MemoryRouter>
      </IpcProvider>,
    );
    expect(screen.queryByTestId('mock-banner')).toBeNull();
  });
});
