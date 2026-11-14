/**
 * Loading the competency packs from the Competency Graph (owner request 2026-11-14): an empty
 * graph offers the packs that ship with the application; loading them lists 64 competencies, each
 * not assessed, and stops offering them.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { CompetencyGraph } from './CompetencyGraph';

describe('Competency Graph: the competency packs', () => {
  it('offers the packs when the graph is empty, loads them and lists every competency as not assessed', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next(), { fresh: true });
    renderWithIpc(<CompetencyGraph />, harness);

    expect(await screen.findByText('No competencies yet')).toBeInTheDocument();
    const empty = screen.getByText('No competencies yet').closest('[role="status"]');
    expect(empty).not.toBeNull();
    await user.click(within(empty as HTMLElement).getByRole('button', { name: 'Load the competency packs' }));

    const receipt = await screen.findByTestId('packs-loaded');
    expect(receipt).toHaveTextContent('Loaded 64 competencies');
    expect(receipt).toHaveTextContent('64 of 64 are stored');
    expect(harness.calls.find((c) => c.op === 'competency.import_pack')?.payload).toEqual({});

    await waitFor(() => {
      expect(screen.queryByText('No competencies yet')).toBeNull();
    });
    // Every competency is listed and none is graded: the state shown is the backend's "not assessed".
    const list = await screen.findByRole('list', { name: /Competencies/ });
    expect(within(list).getAllByRole('listitem').length).toBe(64);
    // Nothing is left to load, so the page head stops offering it.
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Load the competency packs' })).toBeNull();
    });
  });

  it('shows the backend refusal and keeps offering the packs', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness(
      (op, _payload, next) => (op === 'competency.import_pack' ? Promise.resolve({ category: 'RETRYABLE_STORAGE', code: 'storage.busy', message: 'database is busy' }) : next()),
      { fresh: true },
    );
    renderWithIpc(<CompetencyGraph />, harness);
    await user.click(await screen.findByRole('button', { name: 'Load the competency packs' }));
    const alerts = await screen.findAllByRole('alert');
    expect(alerts.some((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE')).toBe(true);
    expect(screen.queryByTestId('packs-loaded')).toBeNull();
    expect(screen.getAllByRole('button', { name: 'Load the competency packs' }).length).toBeGreaterThan(0);
  });

  it('does not offer the packs on a fixture that already has them all', async () => {
    const harness = createInterceptHarness((_op, _payload, next) => next(), { fresh: true });
    await harness.client.invoke('competency.import_pack', {});
    renderWithIpc(<CompetencyGraph />, harness);
    await screen.findByRole('list', { name: /Competencies/ });
    expect(screen.queryByRole('button', { name: 'Load the competency packs' })).toBeNull();
  });
});
