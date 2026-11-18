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
import { asJson, v2Manifest } from '../../test/v2-manifest';
import { DayRail } from '../layout/DayRail';
import { CurriculumNavigator } from './CurriculumNavigator';
import { DailyClassroom } from './DailyClassroom';
import { MissionControl } from './MissionControl';
import { SessionTracker } from './SessionTracker';
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


describe('Block names on the screens come from the plan label (ADR-0023)', () => {
  // The mock clock says Wednesday 2026-11-18, week 3 of a program that started on Monday 2026-11-02:
  // D15 of the Appendix A seed, D13 of the v2 plan (five days a week).
  const V2 = ['Active recall', 'Core lesson', 'Tool practice', 'Independent challenge', 'Defense and documentation'];
  const V2_FRIDAY = ['Active recall', 'Knowledge exam', 'Review and planning', 'Scored practical', 'Scoring and report'];
  const APPENDIX_A = ['Recall', 'Theory', 'Guided lab', 'Independent lab', 'Defense verification', 'Evidence reporting'];

  async function ready(shape: 'appendix-a' | 'v2') {
    const harness = fresh();
    await setUpProgram(harness, '2026-11-02', undefined, shape === 'v2' ? asJson(v2Manifest()) : undefined);
    return harness;
  }

  /** The names of the blocks in the Daily Classroom's blocks panel, in order. */
  async function classroomNames(): Promise<string[]> {
    const blocks = await screen.findByRole('region', { name: 'Today’s blocks' });
    const links = await within(blocks).findAllByRole('link', { name: /./ });
    return links.map((l) => l.textContent).filter((t) => t !== '');
  }

  it('Appendix A: the Daily Classroom prints the block code as a sentence, byte for byte as before', async () => {
    renderWithIpc(<DailyClassroom />, await ready('appendix-a'));
    await waitFor(async () => {
      expect(await classroomNames()).toEqual(APPENDIX_A);
    });
  });

  it('v2: the Daily Classroom prints the profile labels of an ordinary day', async () => {
    renderWithIpc(<DailyClassroom />, await ready('v2'));
    await waitFor(async () => {
      expect(await classroomNames()).toEqual(V2);
    });
    const blocks = screen.getByRole('region', { name: 'Today’s blocks' });
    expect(blocks).toHaveTextContent('Block 3 of 5, 2 h planned');
    expect(blocks).not.toHaveTextContent('Guided lab');
    expect(screen.getByTestId('day-facts')).toHaveTextContent('D13');
  });

  it('v2: the same code has another name on the Friday, and the Friday is the assessment day', async () => {
    const harness = await ready('v2');
    harness.transport.clock.set(Date.UTC(2026, 10, 20, 8, 30));
    renderWithIpc(<DailyClassroom />, harness);
    await waitFor(async () => {
      expect(await classroomNames()).toEqual(V2_FRIDAY);
    });
    expect(screen.getByTestId('day-facts')).toHaveTextContent('Today is an assessment day');
  });

  it('v2: a Saturday is a rest day, where the Appendix A shape has its scored Saturday', async () => {
    const v2 = await ready('v2');
    v2.transport.clock.set(Date.UTC(2026, 10, 21, 8, 30));
    renderWithIpc(<DailyClassroom />, v2);
    expect(await screen.findByText('Rest day')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: 'Today’s blocks' })).toBeNull();

    const appendix = await ready('appendix-a');
    appendix.transport.clock.set(Date.UTC(2026, 10, 21, 8, 30));
    renderWithIpc(<DailyClassroom />, appendix);
    await waitFor(async () => {
      expect(await classroomNames()).toEqual(['Blind practical', 'Remediation', 'Report', 'Performance review']);
    });
  });

  it('the day rail names each block of the day with the label, for both shapes', async () => {
    for (const [shape, names] of [
      ['appendix-a', APPENDIX_A],
      ['v2', V2],
    ] as const) {
      const harness = await ready(shape);
      const status = await harness.client.invoke('system.status', {});
      if ('category' in status) {
        throw new Error('system.status failed');
      }
      const { unmount } = renderWithIpc(<DayRail status={status.result} epoch={0} />, harness);
      await waitFor(() => {
        expect(screen.getAllByRole('link', { name: /planned/ }).map((l) => l.getAttribute('aria-label')?.split(',')[0])).toEqual(names);
      });
      unmount();
    }
  });

  it('the Curriculum Navigator lists the blocks of a v2 day by label and shows its Saturday as a rest day', async () => {
    renderWithIpc(<CurriculumNavigator />, await ready('v2'));
    const tree = await screen.findByLabelText('Curriculum tree');
    // Today (Wednesday) is open: its blocks are listed under the labels of the plan.
    await waitFor(() => {
      expect(within(tree).getAllByText('Tool practice').length).toBeGreaterThanOrEqual(1);
    });
    expect(within(tree).queryByText('Guided lab')).toBeNull();
    // The navigator lists six days a week (Monday to Saturday); the backend answers the v2 Saturday as unscheduled.
    await waitFor(() => {
      expect(within(tree).getByText('Saturday').closest('summary')).toHaveTextContent('Rest day');
    });
    expect(within(tree).getByText('Friday').closest('summary')).toHaveTextContent('Assessment day');
  });

  it('Mission Control and the session tracker name a planned v2 session by its label; a session of another day is named from its code', async () => {
    const harness = await ready('v2');
    for (const date of ['2026-11-18', '2026-11-02']) {
      const planned = await harness.client.invoke('session.plan_day', { date });
      expect('category' in planned).toBe(false);
    }
    renderWithIpc(<MissionControl />, harness);
    const now = await screen.findByTestId('now-block');
    await waitFor(() => {
      expect(now).toHaveTextContent('Active recall');
    });
    expect(within(now).getByRole('link', { name: 'Go to active recall' })).toBeInTheDocument();
    const table = await screen.findByRole('table', { name: /Blocks for D13/ });
    expect(within(table).getByText('Tool practice')).toBeInTheDocument();
    expect(within(table).getByText('Defense and documentation')).toBeInTheDocument();
    expect(within(table).queryByText('Guided lab')).toBeNull();

    const tracker = renderWithIpc(<SessionTracker />, harness);
    const todays = await tracker.findByRole('region', { name: 'Today’s sessions' });
    expect(within(todays).getAllByRole('button').map((b) => b.querySelector('.row-title')?.textContent.trim())).toEqual(V2);
    // The session view carries the block code only: D01 is not today's plan, so its sessions are named from their codes.
    const other = await tracker.findByRole('region', { name: 'Sessions of other days' });
    expect(within(other).getByText('Guided lab')).toBeInTheDocument();
    expect(within(other).queryByText('Tool practice')).toBeNull();
  });
});
