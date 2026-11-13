/**
 * Reviewer grading from the Assessments screen: the payload must use the backend rubric codes
 * (g9_domain BaselineRubric) and contain only levels the reviewer actually chose. A criterion's
 * level is a radio group of the five anchored levels; nothing is chosen until the reviewer chooses.
 * Also here: beginning an attempt from the list, and making a grade final through the voucher.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error, type IpcClient } from '../../ipc/client';
import type { RequestOf } from '../../ipc/operations';
import { ANCHORED_LEVELS, BASELINE_RUBRIC } from '../../lib/rubric';
import { createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { Assessments, LEVEL_ANCHORS } from './Assessments';

async function attemptInEvidenceReview(client: IpcClient): Promise<string> {
  const list = await client.invoke('assessment.list', {});
  if (isG9Error(list)) {
    throw new Error(list.code);
  }
  const def = list.result.definitions.find((d) => !list.result.attempts.some((a) => a.assessment_id === d.assessment_id));
  if (def === undefined) {
    throw new Error('no fresh definition');
  }
  const begun = await client.invoke('assessment.begin', { assessment_id: def.assessment_id });
  if (isG9Error(begun)) {
    throw new Error(begun.code);
  }
  for (const step of [
    () => client.invoke('assessment.submit', { attempt_id: begun.result.attempt_id, evidence_refs: [] }),
    () => client.invoke('assessment.review_evidence', { attempt_id: begun.result.attempt_id }),
  ]) {
    const r = await step();
    if (isG9Error(r)) {
      throw new Error(r.code);
    }
  }
  return begun.result.attempt_id;
}

/** The line of an attempt in the list: a button whose title is the attempt id (the exact name, one step away). */
function selectAttempt(attemptId: string): HTMLElement {
  return within(screen.getByRole('region', { name: 'Attempts' })).getByTitle(attemptId);
}

/** The level picker of a criterion: a radio group named by the criterion's title. */
function levelGroup(title: string): HTMLElement {
  return screen.getByRole('radiogroup', { name: title });
}

function level(title: string, n: number): HTMLElement {
  return within(levelGroup(title)).getByRole('radio', { name: new RegExp(`^Level ${n}\\. `) });
}

const transitions = () => screen.getByRole('group', { name: 'Attempt transitions' });

describe('Assessments reviewer grading', () => {
  it('sends the backend rubric criterion codes and reaches GRADED_PROVISIONAL (level 3 everywhere = 80 PASS)', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const attemptId = await attemptInEvidenceReview(harness.client);
    renderWithIpc(<Assessments />, harness);
    await user.click(await waitFor(() => selectAttempt(attemptId)));
    for (const c of BASELINE_RUBRIC) {
      await user.click(level(c.title, 3));
    }
    // The chosen level is said in words under the criterion, with the anchor the backend rubric gives it.
    expect(screen.getAllByText('Level 3. Correct and complete. Counts 80 % of the weight.')).toHaveLength(BASELINE_RUBRIC.length);
    await user.click(screen.getByRole('button', { name: 'Grade' }));
    const call = harness.calls.find((c) => c.op === 'assessment.grade_provisional');
    const payload = call?.payload as RequestOf<'assessment.grade_provisional'> | undefined;
    expect(payload?.criteria.map((c) => c.criterion_code)).toEqual([
      'independent_execution',
      'root_cause_reasoning',
      'remediation_verification',
      'detection_visibility',
      'evidence_documentation',
      'scope_compliance_reporting',
    ]);
    expect(payload?.criteria.every((c) => c.score === 3 && c.source === 'HUMAN_REVIEWER')).toBe(true);
    // The grade region shows the backend's total and result: 80, pass, and not final yet.
    const grade = screen.getByRole('region', { name: 'Grade' });
    await waitFor(() => {
      expect(within(grade).getByTestId('grade-total')).toHaveTextContent(/^80$/);
    });
    expect(within(grade).getByLabelText('Result: pass')).toHaveTextContent('Pass');
    expect(within(grade).getByLabelText('State: Provisional')).toBeInTheDocument();
    expect(within(grade).getByText('Self-graded')).toHaveAttribute('title', 'SELF_ADJUDICATED');
    // One line per criterion: its title, weight, the level entered and who graded it.
    const firstLine = within(within(grade).getByRole('table', { name: 'Criterion results' })).getByText('Independent execution').closest('tr');
    expect(within(firstLine ?? grade).getAllByRole('cell').map((cell) => cell.textContent)).toEqual(['Independent execution', '25', '3', 'Reviewer', 'None']);
    expect(screen.getAllByTitle('GRADED_PROVISIONAL').length).toBeGreaterThan(0);
    expect(screen.queryAllByRole('alert').filter((a) => a.hasAttribute('data-category'))).toHaveLength(0);
  });

  it('starts with no level chosen, keeps Grade disabled until every criterion holds a chosen level 0–4, and resets per attempt', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const first = await attemptInEvidenceReview(harness.client);
    // The seeded GRADED_PROVISIONAL attempt goes back to EVIDENCE_REVIEW for a regrade.
    const second = 'mock-attempt-prev';
    const back = await harness.client.invoke('assessment.review_evidence', { attempt_id: second });
    expect(isG9Error(back)).toBe(false);
    renderWithIpc(<Assessments />, harness);
    await user.click(await waitFor(() => selectAttempt(first)));
    const grade = within(transitions()).getByRole('button', { name: 'Grade' });
    const titles = BASELINE_RUBRIC.map((c) => c.title);
    for (const title of titles) {
      // Only an anchored level 0–4 can be entered at all: the group offers exactly those five, none of them chosen.
      const radios = within(levelGroup(title)).getAllByRole('radio');
      expect(radios.map((r) => r.textContent)).toEqual(['0', '1', '2', '3', '4']);
      for (const radio of radios) {
        expect(radio).not.toBeChecked();
      }
    }
    expect(screen.getAllByText('No level chosen.')).toHaveLength(titles.length);
    expect(grade).toBeDisabled();
    for (const title of titles.slice(0, -1)) {
      await user.click(level(title, 2));
    }
    expect(grade).toBeDisabled();
    expect(screen.getByText(/^5 of 6 criteria have a level\./)).toBeInTheDocument();
    // A criterion without a chosen level is never sent as a level 0 the reviewer did not enter.
    await user.click(grade);
    expect(harness.calls.some((c) => c.op === 'assessment.grade_provisional')).toBe(false);
    const last = titles.at(-1);
    const firstTitle = titles[0];
    if (last === undefined || firstTitle === undefined) {
      throw new Error('no criteria');
    }
    await user.click(level(last, 4));
    expect(grade).toBeEnabled();
    // Choosing again replaces the level: one level per criterion, the latest choice.
    await user.click(level(firstTitle, 0));
    expect(level(firstTitle, 0)).toBeChecked();
    expect(level(firstTitle, 2)).not.toBeChecked();
    expect(within(levelGroup(firstTitle)).getAllByRole('radio', { checked: true })).toHaveLength(1);
    expect(grade).toBeEnabled();
    // Another attempt starts with no level chosen.
    await user.click(selectAttempt(second));
    for (const title of titles) {
      expect(within(levelGroup(title)).queryAllByRole('radio', { checked: true })).toHaveLength(0);
    }
    expect(within(transitions()).getByRole('button', { name: 'Grade' })).toBeDisabled();
    expect(harness.calls.some((c) => c.op === 'assessment.grade_provisional')).toBe(false);
  });

  it('a level group works from the keyboard like a radio group: one tab stop, arrows move and choose', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const attemptId = await attemptInEvidenceReview(harness.client);
    renderWithIpc(<Assessments />, harness);
    await user.click(await waitFor(() => selectAttempt(attemptId)));
    const radios = within(levelGroup('Independent execution')).getAllByRole('radio');
    expect(radios.map((r) => r.tabIndex)).toEqual([0, -1, -1, -1, -1]);
    radios[0]?.focus();
    // Focus alone chooses nothing.
    expect(radios[0]).not.toBeChecked();
    await user.keyboard('{ArrowRight}');
    expect(radios[1]).toBeChecked();
    expect(document.activeElement).toBe(radios[1]);
    await user.keyboard('{End}');
    expect(radios[4]).toBeChecked();
    await user.keyboard('{ArrowDown}');
    expect(radios[0]).toBeChecked();
    await user.keyboard('{ArrowLeft}');
    expect(radios[4]).toBeChecked();
    expect(document.activeElement).toBe(radios[4]);
    expect(radios.filter((r) => r.getAttribute('aria-checked') === 'true')).toHaveLength(1);
    expect(radios.map((r) => r.tabIndex)).toEqual([-1, -1, -1, -1, 0]);
    // The other criteria are untouched.
    expect(within(levelGroup('Root-cause reasoning')).queryAllByRole('radio', { checked: true })).toHaveLength(0);
  });

  it('shows the anchors of the backend rubric (g9-assessment STANDARD_LEVEL_DESCRIPTORS), level for level', () => {
    // A path, not a URL object: under jsdom the global `URL` is not the one `node:url` accepts.
    const src = readFileSync(join(import.meta.dirname, '../../../../../crates/g9-assessment/src/rubric.rs'), 'utf8');
    const block = /pub const STANDARD_LEVEL_DESCRIPTORS: \[&str; LEVEL_COUNT\] = \[([\s\S]*?)\];/.exec(src)?.[1];
    if (block === undefined) {
      throw new Error('STANDARD_LEVEL_DESCRIPTORS not found in g9-assessment/src/rubric.rs');
    }
    const rust = [...block.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect(rust).toHaveLength(5);
    expect(ANCHORED_LEVELS.map((l) => LEVEL_ANCHORS[l])).toEqual(rust);
  });
});

describe('Assessments: begin', () => {
  it('Begin starts an attempt, selects it and lists it in progress; an assessment with an open attempt takes no second one', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<Assessments />, harness);
    const definitions = await screen.findByRole('list', { name: 'Assessment definitions' });
    // The fixture's graded attempt is not final yet, so its assessment cannot be begun again.
    expect(within(definitions).getByRole('button', { name: 'Begin Week 2 blind practical (fixture)' })).toBeDisabled();
    expect(within(definitions).getByText(/An attempt is in progress\./)).toBeInTheDocument();
    await user.click(within(definitions).getByRole('button', { name: 'Begin D18 blind practical' }));
    expect(harness.calls.find((c) => c.op === 'assessment.begin')?.payload).toEqual({ assessment_id: 'mock-assessment-D18' });
    const inProgress = await screen.findByRole('region', { name: 'Attempts in progress' });
    expect(within(inProgress).getByRole('button', { pressed: true })).toHaveTextContent('D18 blind practical');
    expect(screen.getByRole('region', { name: 'Attempt: D18 blind practical' })).toBeInTheDocument();
    // Begin is behind it, Submit is the current step and the one primary button.
    const steps = screen.getByRole('list', { name: 'Steps of this attempt' });
    expect(within(steps).getByRole('listitem', { current: 'step' })).toHaveTextContent(/^Submit$/);
    expect(within(steps).getAllByRole('listitem').map((li) => li.getAttribute('data-step'))).toEqual(['done', 'current', null, null, null]);
    await waitFor(() => {
      expect(within(transitions()).getByRole('button', { name: 'Submit' })).toBeEnabled();
    });
    expect(within(transitions()).getByRole('button', { name: 'Submit' })).toHaveClass('primary');
    for (const name of ['Review evidence', 'Grade', 'Approve final']) {
      expect(within(transitions()).getByRole('button', { name })).toBeDisabled();
      expect(within(transitions()).getByRole('button', { name })).not.toHaveClass('primary');
    }
    expect(within(definitions).getByRole('button', { name: 'Begin D18 blind practical' })).toBeDisabled();
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });
});

describe('Assessments: making a grade final', () => {
  it('Approve final raises the voucher directly under the buttons; the applied approval makes the grade final, which cannot be changed', async () => {
    const user = userEvent.setup();
    renderWithIpc(<Assessments />);
    const approve = await screen.findByRole('button', { name: 'Approve final' });
    expect(approve).toBeEnabled();
    expect(approve).toHaveClass('primary');
    // Back to the evidence review is legal too, as a default button.
    expect(within(transitions()).getByRole('button', { name: 'Review evidence' })).toBeEnabled();
    expect(within(transitions()).getByRole('button', { name: 'Review evidence' })).not.toHaveClass('primary');
    expect(screen.getByText('This grade is not final. It counts once you approve it as final; a final grade cannot be changed.')).toBeInTheDocument();
    await user.click(approve);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(within(card).getByTestId('approval-expiry')).toHaveTextContent('2026-11-18 09:40');
    expect(within(card).getByTitle('PENDING')).toHaveTextContent('Waiting for you');
    expect(screen.getByRole('alert')).toHaveAttribute('data-category', 'APPROVAL_REQUIRED');
    // The voucher follows the instrument that asked, in the same group, before the grade.
    const instrument = screen.getByRole('region', { name: 'Attempt: Week 2 blind practical (fixture)' });
    expect(card.parentElement).toBe(instrument.parentElement);
    expect(instrument.compareDocumentPosition(card) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(card.compareDocumentPosition(screen.getByRole('region', { name: 'Grade' })) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await within(card).findByTitle('CONSUMED')).toHaveTextContent('Used');
    expect(await screen.findAllByTitle('APPROVED_FINAL')).not.toHaveLength(0);
    expect(within(await screen.findByRole('region', { name: 'Final attempts' })).getByRole('button', { pressed: true })).toHaveTextContent('Week 2 blind practical (fixture)');
    const grade = screen.getByRole('region', { name: 'Grade' });
    expect(within(grade).getByText('This grade is final. A final grade cannot be changed.')).toBeInTheDocument();
    expect(within(grade).getByLabelText('Provenance: verified')).toBeInTheDocument();
    // A final attempt takes no further step.
    await waitFor(() => {
      expect(within(screen.getByRole('list', { name: 'Steps of this attempt' })).queryAllByRole('listitem', { current: 'step' })).toHaveLength(0);
    });
    for (const name of ['Submit', 'Review evidence', 'Grade', 'Approve final']) {
      expect(within(transitions()).getByRole('button', { name })).toBeDisabled();
    }
  });

  it('a cancelled confirmation approves nothing and says so under the buttons', async () => {
    const user = userEvent.setup();
    renderWithIpc(<Assessments />, createHarness({ confirmApproval: () => false }));
    await user.click(await screen.findByRole('button', { name: 'Approve final' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('flow-confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    expect(within(card).getByTitle('PENDING')).toBeInTheDocument();
    expect(screen.getAllByTitle('GRADED_PROVISIONAL').length).toBeGreaterThan(0);
    expect(screen.queryAllByTitle('APPROVED_FINAL')).toHaveLength(0);
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
  });
});
