/**
 * The theme setting end to end in the DOM: choosing a theme in Settings, in the shell's picker
 * or from the command palette applies it to <html> and keeps it; high contrast keeps the
 * theme's type but removes its colours; Ledger clears every inline property.
 */
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';
import { IpcProvider } from '../ipc-context';
import { routes } from '../router';
import { SettingsRecovery } from '../screens/SettingsRecovery';
import { createHarness, renderWithIpc } from '../../test/render';
import { applySkin, SKINS, skinById, SKIN_STORAGE_KEY } from './skins';
import { readStoredSkin } from './useSkin';

const html = () => document.documentElement;

beforeEach(() => {
  applySkin(skinById('ledger'), 'default');
  delete html().dataset.contrast;
  delete html().dataset.theme;
});

describe('theme setting', () => {
  it('lists every theme as one radio in the Settings gallery and applies the chosen one to <html>', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    const gallery = await screen.findByRole('group', { name: 'Theme' });
    const radios = within(gallery).getAllByRole('radio');
    expect(radios).toHaveLength(SKINS.length);
    expect(within(gallery).getByRole('radio', { name: 'Ledger' })).toBeChecked();

    await user.click(within(gallery).getByRole('radio', { name: 'Hacker' }));
    expect(within(gallery).getByRole('radio', { name: 'Hacker' })).toBeChecked();
    expect(html().dataset.skin).toBe('hacker');
    expect(html().dataset.effect).toBe('glow');
    expect(html().style.getPropertyValue('--paper')).toMatch(/^oklch\(/);
    expect(html().style.getPropertyValue('--mono')).toBe('1');
    expect(localStorage.getItem(SKIN_STORAGE_KEY)).toBe('hacker');
    // A single-scheme theme hides the light/dark control and says why.
    expect(screen.queryByRole('group', { name: 'Colour scheme' })).toBeNull();
    expect(screen.getByText(/Hacker is a dark theme/)).toBeInTheDocument();
  });

  it('keeps the type but removes the colours of a theme under high contrast, and restores them after', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    const gallery = await screen.findByRole('group', { name: 'Theme' });
    await user.click(within(gallery).getByRole('radio', { name: 'Typewriter' }));
    expect(html().style.getPropertyValue('--ink')).not.toBe('');

    await user.click(screen.getByRole('button', { name: /High contrast/ }));
    expect(html().dataset.contrast).toBe('high');
    expect(html().style.getPropertyValue('--ink')).toBe('');
    expect(html().style.getPropertyValue('--paper')).toBe('');
    expect(html().style.getPropertyValue('--mono')).toBe('1');

    await user.click(screen.getByRole('button', { name: /High contrast/ }));
    expect(html().style.getPropertyValue('--ink')).toMatch(/^oklch\(/);
  });

  it('returns to Ledger with no inline properties left behind', async () => {
    const user = userEvent.setup();
    renderWithIpc(<SettingsRecovery />);
    const gallery = await screen.findByRole('group', { name: 'Theme' });
    await user.click(within(gallery).getByRole('radio', { name: 'Archive' }));
    expect(html().style.getPropertyValue('--font')).toBe('var(--font-serif)');
    await user.click(within(gallery).getByRole('radio', { name: 'Ledger' }));
    expect(html().dataset.skin).toBe('ledger');
    expect(html().style.getPropertyValue('--font')).toBe('');
    expect(html().style.getPropertyValue('--paper')).toBe('');
    expect(screen.getByRole('group', { name: 'Colour scheme' })).toBeInTheDocument();
  });

  it('falls back to Ledger for a stored value that is not a theme', () => {
    expect(readStoredSkin({ getItem: () => 'not-a-theme' })).toBe('ledger');
    expect(readStoredSkin({ getItem: () => null })).toBe('ledger');
    expect(
      readStoredSkin({
        getItem: () => {
          throw new Error('storage disabled');
        },
      }),
    ).toBe('ledger');
    expect(readStoredSkin({ getItem: () => 'blueprint' })).toBe('blueprint');
  });

  it('switches theme from the shell picker and from the command palette', async () => {
    const user = userEvent.setup();
    const { client } = createHarness();
    const router = createMemoryRouter(routes, { initialEntries: ['/'] });
    render(
      <IpcProvider client={client}>
        <RouterProvider router={router} />
      </IpcProvider>,
    );
    const picker = await screen.findByRole('combobox', { name: 'Theme' });
    expect(within(picker).getAllByRole('option')).toHaveLength(SKINS.length);
    await user.selectOptions(picker, 'Clean Crisp');
    expect(html().dataset.skin).toBe('clean');

    await act(async () => {
      await user.keyboard('{Control>}k{/Control}');
    });
    const search = await screen.findByRole('combobox', { name: 'Search screens and commands' });
    await user.type(search, 'theme: amber');
    await user.keyboard('{Enter}');
    expect(html().dataset.skin).toBe('amber-crt');
    expect(html().dataset.effect).toBe('scanlines');
  });
});
