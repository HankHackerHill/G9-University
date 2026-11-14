/**
 * The program on the screens other than Mission Control (owner request 2026-11-14): an install
 * that is not set up says so once and points to the setup; a set-up install plans a day from the
 * Daily Classroom and the Curriculum Navigator, browses the weeks, and changes the start in
 * Settings.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createInterceptHarness } from '../../test/intercept';
import { setUpProgram } from '../../test/program';
import { renderWithIpc } from '../../test/render';
import { DayRail } from '../layout/DayRail';
import { CurriculumNavigator } from './CurriculumNavigator';
import { DailyClassroom } from './DailyClassroom';
import { MissionControl } from './MissionControl';
import { SettingsRecovery } from './SettingsRecovery';

const fresh = () => createInterceptHarness((_op, _payload, next) => next(), { fresh: true });

describe('Daily Classroom and the program', () => {
  it('says the program is not set up, once, and points to Mission Control', async () => {
    renderWithIpc(<DailyClassroom />, fresh());
    expect(await screen.findByText('Program not set up')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open Mission Control' })).toHaveAttribute('href', '/');
    // Not an error banner: the backend's "not imported" is setup, not a failure.
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });

  it('plans today\'s blocks from the blocks panel and then shows each block with a session', async () => {
    const user = userEvent.setup();
    const harness = fresh();
    await setUpProgram(harness);
    renderWithIpc(<DailyClassroom />, harness);
    const blocks = await screen.findByRole('region', { name: 'Today’s blocks' });
    await user.click(await within(blocks).findByRole('button', { name: 'Plan today’s blocks' }));
    await waitFor(() => {
      expect(harness.calls.find((c) => c.op === 'session.plan_day')?.payload).toEqual({ date: '2026-11-18' });
    });
    await waitFor(() => {
      expect(within(screen.getByRole('region', { name: 'Today’s blocks' })).getAllByText('Planned').length).toBe(6);
    });
    // Nothing is left to plan, so the button is gone.
    expect(within(screen.getByRole('region', { name: 'Today’s blocks' })).queryByRole('button', { name: /Plan/ })).toBeNull();
  });
});

describe('Curriculum Navigator and the program', () => {
  it('says the program is not set up once instead of six errors', async () => {
    renderWithIpc(<CurriculumNavigator />, fresh());
    expect(await screen.findByText('Program not set up')).toBeInTheDocument();
    expect(screen.queryByLabelText('Curriculum tree')).toBeNull();
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });

  it('browses the weeks, jumps to week 1 and plans a day', async () => {
    const user = userEvent.setup();
    const harness = fresh();
    await setUpProgram(harness);
    renderWithIpc(<CurriculumNavigator />, harness);
    // Today is Wednesday 2026-11-18: week 3 of a program that started on 2026-11-02.
    expect(await screen.findByRole('heading', { level: 2, name: /^Week 3/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'This week' })).toBeDisabled();

    await user.click(screen.getByRole('button', { name: 'Earlier week' }));
    // The fixture holds week 3 only: the week before is outside what the mock loaded, and says so per day.
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'This week' })).toBeEnabled();
    });
    await user.click(screen.getByRole('button', { name: 'This week' }));
    expect(await screen.findByRole('heading', { level: 2, name: /^Week 3/ })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Week 1' }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Week 1' })).toBeDisabled();
    });

    await user.click(screen.getByRole('button', { name: 'This week' }));
    const tree = await screen.findByLabelText('Curriculum tree');
    const today = within(tree).getByText('Today').closest('details');
    expect(today).not.toBeNull();
    await user.click(within(today as HTMLElement).getByRole('button', { name: 'Plan this day’s blocks' }));
    await waitFor(() => {
      expect(harness.calls.find((c) => c.op === 'session.plan_day')?.payload).toEqual({ date: '2026-11-18' });
    });
    await waitFor(() => {
      expect(within(screen.getByLabelText('Curriculum tree')).getAllByText('Every block of this day has a session.').length).toBeGreaterThanOrEqual(1);
    });
  });

  it('names the version it reads from once the program is known', async () => {
    const harness = fresh();
    await setUpProgram(harness);
    renderWithIpc(<CurriculumNavigator />, harness);
    const version = await screen.findByRole('complementary', { name: 'Curriculum version' });
    await waitFor(() => {
      expect(version).toHaveTextContent('0.1.0-seed');
    });
    expect(version).toHaveTextContent('Monday 2026-11-02');
  });
});

describe('Settings: the program', () => {
  it('shows what is set up and changes the start with the revision it read, refusing a day that is not a Monday', async () => {
    const user = userEvent.setup();
    const harness = fresh();
    await setUpProgram(harness, '2026-11-02');
    renderWithIpc(<SettingsRecovery />, harness);
    const panel = await screen.findByRole('region', { name: 'Program' });
    await waitFor(() => {
      expect(within(panel).getByTestId('setup-start')).toHaveTextContent('Week 1 starts Monday 2026-11-02');
    });
    expect(within(panel).getByTestId('setup-start')).toHaveTextContent('A planned day starts at 09:00');
    expect(within(panel).getByTestId('setup-curriculum')).toHaveAttribute('data-state', 'done');

    await user.click(within(panel).getByRole('button', { name: 'Change' }));
    const form = await within(panel).findByRole('form', { name: 'Program start' });
    const date = within(form).getByLabelText('Week 1 starts (a Monday)');
    expect(date).toHaveValue('2026-11-02');
    fireEvent.change(date, { target: { value: '2026-11-11' } });
    expect(within(form).getByRole('alert')).toHaveTextContent('is not a Monday');
    expect(within(form).getByRole('button', { name: 'Save the new start' })).toBeDisabled();
    fireEvent.change(date, { target: { value: '2026-11-09' } });
    fireEvent.change(within(form).getByLabelText('A day starts at'), { target: { value: '08:15' } });
    await user.click(within(form).getByRole('button', { name: 'Save the new start' }));

    await waitFor(() => {
      expect(within(screen.getByRole('region', { name: 'Program' })).getByTestId('setup-start')).toHaveTextContent('Week 1 starts Monday 2026-11-09');
    });
    const sent = harness.calls.filter((c) => c.op === 'curriculum.set_program_start').at(-1);
    expect(sent?.payload).toEqual({ date: '2026-11-09', day_start: '08:15' });
    // The change carries the revision it was read at (revision 1 after the first set).
    expect(sent?.meta.expected_revision).toBe(1);
    expect(screen.getByRole('region', { name: 'Program' })).toHaveTextContent('A planned day starts at 08:15');
  });

  it('offers the competency packs from the program panel and stops offering them once loaded', async () => {
    const user = userEvent.setup();
    const harness = fresh();
    await setUpProgram(harness);
    renderWithIpc(<SettingsRecovery />, harness);
    const panel = await screen.findByRole('region', { name: 'Program' });
    await user.click(await within(panel).findByRole('button', { name: 'Load the competency packs' }));
    expect(await within(panel).findByTestId('setup-packs-loaded')).toHaveTextContent('Loaded 64 competencies');
    await waitFor(() => {
      expect(within(screen.getByRole('region', { name: 'Program' })).getByTestId('setup-packs')).toHaveAttribute('data-state', 'done');
    });
    expect(within(screen.getByRole('region', { name: 'Program' })).queryByRole('button', { name: 'Load the competency packs' })).toBeNull();
  });
});

describe('The day rail of the shell', () => {
  it('says the program is not set up instead of an error with a code', async () => {
    const harness = fresh();
    const status = await harness.client.invoke('system.status', {});
    if ('category' in status) {
      throw new Error('system.status failed');
    }
    renderWithIpc(<DayRail status={status.result} epoch={0} />, harness);
    expect(await screen.findByText('Program not set up')).toBeInTheDocument();
    expect(screen.getByText('Set up the program on Mission Control to see the day.')).toBeInTheDocument();
    expect(screen.queryByText(/curriculum\.not_imported/)).toBeNull();
  });
});

describe('Before week 1 starts', () => {
  // The mock clock says Wednesday 2026-11-18; a program that starts on Monday 2026-11-23 has not begun.
  it('Mission Control says when week 1 starts instead of calling today a rest day', async () => {
    const harness = fresh();
    await setUpProgram(harness, '2026-11-23');
    renderWithIpc(<MissionControl />, harness);
    const now = await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(now).toHaveTextContent('Week 1 starts Monday 2026-11-23');
    });
    expect(within(now).getByRole('link', { name: 'Browse the weeks' })).toHaveAttribute('href', '/curriculum');
    expect(screen.queryByText('Rest day')).toBeNull();
    expect((await screen.findAllByText('Outside the program weeks')).length).toBeGreaterThanOrEqual(1);
    expect(now).not.toHaveTextContent('Plan today');
  });

  it('the Daily Classroom and the day rail say it is outside the program weeks, not a rest day', async () => {
    const harness = fresh();
    await setUpProgram(harness, '2026-11-23');
    renderWithIpc(<DailyClassroom />, harness);
    expect(await screen.findByText('Outside the program weeks')).toBeInTheDocument();
    expect(screen.queryByText('Rest day')).toBeNull();
    const status = await harness.client.invoke('system.status', {});
    if ('category' in status) {
      throw new Error('system.status failed');
    }
    renderWithIpc(<DayRail status={status.result} epoch={0} />, harness);
    expect(await screen.findByText(/outside the program weeks$/)).toBeInTheDocument();
  });
});
