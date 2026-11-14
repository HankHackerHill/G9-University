/**
 * The lesson sheets in the Daily Classroom (ADR-0022): the text of each block folded under its name,
 * the first unfinished block open, what to watch for and the sources in the margin. A sheet is
 * shown only for the objectives it was written for; any other day says so in words and shows
 * nothing of the sheet. It is reading: no call is made for it and nothing is written.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DailyPlan, SessionView } from '../../ipc/operations';
import * as lessonSheets from '../../lib/lesson-sheets';
import { createInterceptHarness, outcome } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { DailyClassroom } from './DailyClassroom';

afterEach(() => {
  vi.restoreAllMocks();
});

async function sheetOf(dayId: string) {
  const sheets = await lessonSheets.loadLessonSheets();
  const sheet = sheets.days[dayId];
  if (sheet === undefined) {
    throw new Error(`no sheet for ${dayId}`);
  }
  return sheet;
}

/** The mock's plan for today, made to be `dayId` with the given objective texts and blocks' sessions. */
function planAs(dayId: string, objectives: readonly string[], sessionIds: readonly (string | null)[] = []) {
  return async (_op: unknown, next: () => Promise<unknown>) => {
    const answer = (await next()) as { result: DailyPlan };
    return {
      ...answer,
      result: {
        ...answer.result,
        day_id: dayId,
        objectives: answer.result.objectives.map((o, i) => ({ ...o, objective_id: `${dayId}-O${i + 1}`, text: objectives[i] ?? o.text })),
        blocks: answer.result.blocks.map((b, i) => ({ ...b, session_id: sessionIds[i] ?? null })),
      },
    };
  };
}

function session(session_id: string, state: SessionView['state']): SessionView {
  return { session_id, state, day_id: 'D01', block: 'recall', block_seq: 1, planned_ms: 900_000, credited_ms: 0, uncertain_ms: 0 } as unknown as SessionView;
}

function harnessFor(dayId: string, objectives: readonly string[], sessions: SessionView[] | null, sessionIds: readonly (string | null)[] = []) {
  const plan = planAs(dayId, objectives, sessionIds);
  return createInterceptHarness((op, _payload, next) => {
    if (op === 'curriculum.daily_plan') {
      return plan(op, next);
    }
    if (op === 'session.list') {
      return sessions === null ? Promise.resolve({ category: 'RETRYABLE_STORAGE', code: 'storage.busy', message: 'database is busy' }) : Promise.resolve(outcome({ sessions }));
    }
    return next();
  });
}

describe('Daily Classroom: the lesson sheet of the day', () => {
  it('shows each block of the day under its name, the first block open, and the sources and cautions in the margin', async () => {
    const sheet = await sheetOf('D01');
    const harness = harnessFor('D01', sheet.objectives, []);
    renderWithIpc(<DailyClassroom />, harness);

    const panel = await screen.findByRole('region', { name: 'Lesson sheet' });
    const body = await within(panel).findByTestId('lesson-sheet');
    const names = ['Recall, 15 min', 'Theory, 45 min', 'Guided lab, 2 h', 'Independent lab, 2 h', 'Defence and verification, 1 h', 'Evidence and report, 1 h'];
    for (const name of names) {
      expect(within(body).getByText(name)).toBeInTheDocument();
    }
    expect(within(body).getByText(/^About week 1, /)).toBeInTheDocument();
    // The sheet is a proposed curriculum's text: it carries the stamp of the plan.
    expect(within(panel).getByLabelText('Provenance: proposed')).toBeInTheDocument();

    // No block has a session yet, so the first block is the one to read now.
    const open = (block: string) => body.querySelector(`details[data-block="${block}"]`);
    expect(open('recall')).toHaveAttribute('open');
    expect(open('theory')).not.toHaveAttribute('open');
    expect(within(open('recall') as HTMLElement).getByText(/Nothing to recall yet/)).toBeVisible();
    expect(within(open('theory') as HTMLElement).getByText(/Authorization is permission/)).not.toBeVisible();

    const watch = screen.getByRole('complementary', { name: 'Watch for' });
    expect(watch).toHaveTextContent('Calling a network "isolated"');
    const sources = screen.getByRole('complementary', { name: 'Sources for the day' });
    expect(within(sources).getByText('https://csrc.nist.gov/pubs/sp/800/115/final').tagName).toBe('CODE');
    expect(sources).toHaveTextContent('ASSUMED');
    // The window opens no link: an address is text to copy.
    expect(within(panel).queryAllByRole('link')).toHaveLength(0);
    expect(within(sources).queryAllByRole('link')).toHaveLength(0);
    // Reading is not a command: nothing was sent for it.
    expect(harness.calls.filter((c) => c.op !== 'curriculum.daily_plan' && c.op !== 'session.list' && c.op !== 'system.status').map((c) => c.op)).toEqual([]);
  });

  it('opens the first block that is not finished, following the day’s sessions', async () => {
    const sheet = await sheetOf('D01');
    const sessions = [session('s1', 'COMPLETED'), session('s2', 'RUNNING'), session('s3', 'PLANNED'), session('s4', 'PLANNED'), session('s5', 'PLANNED'), session('s6', 'PLANNED')];
    renderWithIpc(<DailyClassroom />, harnessFor('D01', sheet.objectives, sessions, ['s1', 's2', 's3', 's4', 's5', 's6']));
    const body = await screen.findByTestId('lesson-sheet');
    await waitFor(() => {
      expect(body.querySelector('details[data-block="theory"]')).toHaveAttribute('open');
    });
    expect(body.querySelector('details[data-block="recall"]')).not.toHaveAttribute('open');
  });

  it('opens no block when the sessions could not be read, and says nothing is known about them', async () => {
    const sheet = await sheetOf('D01');
    renderWithIpc(<DailyClassroom />, harnessFor('D01', sheet.objectives, null));
    const body = await screen.findByTestId('lesson-sheet');
    expect(body.querySelectorAll('details[open]')).toHaveLength(0);
    // The failed read is shown once, as the screen always does; the sheet does not pretend.
    expect(await screen.findAllByRole('alert')).not.toHaveLength(0);
  });

  it('shows a Saturday’s blocks without its task statement', async () => {
    const sheet = await sheetOf('D06');
    renderWithIpc(<DailyClassroom />, harnessFor('D06', sheet.objectives, []));
    const body = await screen.findByTestId('lesson-sheet');
    for (const name of ['Blind practical, 2 h', 'Remediation, 1 h', 'Report, 1 h', 'Performance review, 1 h']) {
      expect(within(body).getByText(name)).toBeInTheDocument();
    }
    expect(body.querySelectorAll('details[data-block]')).toHaveLength(5); // four blocks and the week
    expect(body).not.toHaveTextContent('Task statement');
  });

  it('says in words that the sheet was written for another version, and shows none of it', async () => {
    // The mock's own plan is the Appendix A wording of week 3: not the text the sheets were written for.
    renderWithIpc(<DailyClassroom />);
    const panel = await screen.findByRole('region', { name: 'Lesson sheet' });
    await waitFor(() => {
      expect(within(panel).getByText('Lesson sheet not shown')).toBeInTheDocument();
    });
    expect(panel).toHaveTextContent('were written for curriculum version 1.0.0-draft1');
    expect(screen.queryByTestId('lesson-sheet')).toBeNull();
    expect(screen.queryByRole('complementary', { name: 'Watch for' })).toBeNull();
    expect(screen.queryByRole('complementary', { name: 'Sources for the day' })).toBeNull();
  });

  it('says there is no sheet for a day the sheets do not have', async () => {
    renderWithIpc(<DailyClassroom />, harnessFor('D99', ['a', 'b', 'c'], []));
    const panel = await screen.findByRole('region', { name: 'Lesson sheet' });
    await waitFor(() => {
      expect(within(panel).getByText('No lesson sheet for this day')).toBeInTheDocument();
    });
    expect(panel).toHaveTextContent('have no sheet for D99');
  });

  it('says so when the sheets that ship with the application cannot be loaded', async () => {
    vi.spyOn(lessonSheets, 'loadLessonSheets').mockRejectedValue(new Error('chunk failed'));
    renderWithIpc(<DailyClassroom />);
    const panel = await screen.findByRole('region', { name: 'Lesson sheet' });
    await waitFor(() => {
      expect(within(panel).getByText('No lesson sheet')).toBeInTheDocument();
    });
    expect(panel).toHaveTextContent('could not be loaded, so none is shown');
    // The rest of the page is the plan, untouched.
    expect(screen.getByRole('region', { name: 'Today’s objectives' })).toBeInTheDocument();
  });

  it('shows no lesson sheet on a day without blocks, and reads no sheet for it', async () => {
    const load = vi.spyOn(lessonSheets, 'loadLessonSheets');
    const harness = createInterceptHarness((op, _payload, next) =>
      op === 'curriculum.daily_plan' ? Promise.resolve(outcome({ date: '2026-11-22', zone: 'Europe/Berlin', day_id: null, week_number: 3, week_title: 'Rest', unscheduled: true, is_assessment_day: false, planned_minutes: 0, evidence_artifact: null, objectives: [], blocks: [] })) : next(),
    );
    renderWithIpc(<DailyClassroom />, harness);
    expect(await screen.findByText('Rest day')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: 'Lesson sheet' })).toBeNull();
    expect(load).not.toHaveBeenCalled();
  });
});
