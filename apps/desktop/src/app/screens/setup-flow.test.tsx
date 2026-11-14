/**
 * First run on a new install (owner request 2026-11-14): Mission Control offers the three setup
 * steps, the import goes through the approval and the native confirmation, the start must be a
 * Monday, and once the program is set up today's blocks are planned in one press. The mock runs
 * in its fresh-install mode (nothing imported, no start, no sessions).
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { MissionControl } from './MissionControl';

async function setupPanel() {
  return screen.findByRole('region', { name: 'Set up the program' });
}

async function importCurriculum(user: ReturnType<typeof userEvent.setup>) {
  const panel = await setupPanel();
  await user.click(within(panel).getByRole('button', { name: 'Import the curriculum' }));
  const card = await within(panel).findByTestId('approval-card');
  await user.click(within(card).getByRole('button', { name: 'Approve' }));
  await user.click(await within(panel).findByRole('button', { name: 'Apply with approval' }));
}

describe('Mission Control on a new install', () => {
  it('says the program is not set up, offers the three steps and runs the approval for the import', async () => {
    const user = userEvent.setup();
    const harness = createHarness({ fresh: true });
    renderWithIpc(<MissionControl />, harness);

    const now = await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(now).toHaveTextContent('Set up your program');
    });
    const panel = await setupPanel();
    const steps = within(panel).getByRole('list', { name: 'Setup steps' });
    expect(within(steps).getAllByRole('listitem')).toHaveLength(3);
    // Nothing is claimed about a plan that does not exist.
    expect(screen.getAllByText('Program not set up').length).toBeGreaterThan(0);

    // The start cannot be chosen before there is a curriculum.
    expect(within(panel).getByTestId('setup-start')).toHaveAttribute('data-state', 'waiting');
    expect(within(panel).queryByRole('form', { name: 'Program start' })).toBeNull();

    await user.click(within(panel).getByRole('button', { name: 'Import the curriculum' }));
    const card = await within(panel).findByTestId('approval-card');
    expect(card).toHaveTextContent('Import a curriculum');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(panel).findByRole('button', { name: 'Apply with approval' }));

    expect(await within(panel).findByTestId('setup-imported')).toHaveTextContent('Imported 12 weeks, 72 days and 216 objectives as PROPOSED_SEED.');
    const curriculum = within(panel).getByTestId('setup-curriculum');
    expect(curriculum).toHaveAttribute('data-state', 'done');
    expect(curriculum).toHaveTextContent('12 weeks, 72 days, 216 objectives, 480 h');
    expect(curriculum).toHaveTextContent('Proposed');
  });

  it('suggests the next Monday, refuses any other day and saves the start with the time of day', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next(), { fresh: true });
    renderWithIpc(<MissionControl />, harness);
    await importCurriculum(user);

    const form = await screen.findByRole('form', { name: 'Program start' });
    // The mock clock says Wednesday 2026-11-18; the next Monday is 2026-11-23.
    const date = within(form).getByLabelText('Week 1 starts (a Monday)');
    expect(date).toHaveValue('2026-11-23');
    expect(within(form).getByLabelText('A day starts at')).toHaveValue('09:00');
    const submit = within(form).getByRole('button', { name: 'Set the start' });
    expect(submit).toBeEnabled();

    fireEvent.change(date, { target: { value: '2026-11-03' } });
    expect(within(form).getByRole('alert')).toHaveTextContent('Tuesday 2026-11-03 is not a Monday');
    expect(submit).toBeDisabled();

    fireEvent.change(date, { target: { value: '2026-11-02' } });
    fireEvent.change(within(form).getByLabelText('A day starts at'), { target: { value: '08:30' } });
    expect(submit).toBeEnabled();
    await user.click(submit);

    await waitFor(() => {
      expect(harness.calls.some((c) => c.op === 'curriculum.set_program_start')).toBe(true);
    });
    expect(harness.calls.find((c) => c.op === 'curriculum.set_program_start')?.payload).toEqual({ date: '2026-11-02', day_start: '08:30' });
    // The program is set up: the panel is gone and today's plan is the page.
    await waitFor(() => {
      expect(screen.queryByRole('region', { name: 'Set up the program' })).toBeNull();
    });
  });

  it('plans today\'s blocks in one press and then offers the first block', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next(), { fresh: true });
    renderWithIpc(<MissionControl />, harness);
    await importCurriculum(user);
    const form = await screen.findByRole('form', { name: 'Program start' });
    fireEvent.change(within(form).getByLabelText('Week 1 starts (a Monday)'), { target: { value: '2026-11-02' } });
    await user.click(within(form).getByRole('button', { name: 'Set the start' }));

    const now = await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(now).toHaveTextContent('Today’s blocks are not planned yet');
    });
    expect(now).toHaveTextContent('6 blocks, 7 h from the curriculum');
    await user.click(within(now).getByRole('button', { name: 'Plan today’s blocks' }));

    await waitFor(() => {
      expect(harness.calls.find((c) => c.op === 'session.plan_day')?.payload).toEqual({ date: '2026-11-18' });
    });
    await waitFor(() => {
      expect(screen.getByTestId('now-block')).toHaveTextContent('Recall');
    });
    expect(screen.getByTestId('now-block')).toHaveTextContent('block 1 of 6');
    // The blocks table now names a state for each of the six blocks.
    const blocks = screen.getByRole('region', { name: 'Today’s blocks' });
    await waitFor(() => {
      expect(within(blocks).getAllByText('Planned').length).toBeGreaterThanOrEqual(6);
    });
  });

  it('keeps every other screen of an install that is already set up exactly as it was (no setup panel)', async () => {
    renderWithIpc(<MissionControl />);
    await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(screen.getByTestId('now-block')).not.toHaveTextContent('Set up your program');
    });
    expect(screen.queryByRole('region', { name: 'Set up the program' })).toBeNull();
  });
});
