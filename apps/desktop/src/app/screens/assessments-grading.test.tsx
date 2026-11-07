/**
 * Reviewer grading from the Assessments screen: the payload must use the backend rubric codes
 * (g9_domain BaselineRubric) and contain only levels the reviewer actually entered.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error, type IpcClient } from '../../ipc/client';
import type { RequestOf } from '../../ipc/operations';
import { BASELINE_RUBRIC } from '../../lib/rubric';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { Assessments } from './Assessments';

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

function selectAttempt(attemptId: string): HTMLElement {
  const table = screen.getByRole('table', { name: 'Attempts' });
  const row = within(table).getByText(attemptId).closest('tr');
  if (row === null) {
    throw new Error('attempt row not found');
  }
  return within(row).getByRole('button', { name: 'Select' });
}

describe('Assessments reviewer grading', () => {
  it('sends the backend rubric criterion codes and reaches GRADED_PROVISIONAL (level 3 everywhere = 80 PASS)', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const attemptId = await attemptInEvidenceReview(harness.client);
    renderWithIpc(<Assessments />, harness);
    await user.click(await waitFor(() => selectAttempt(attemptId)));
    for (const c of BASELINE_RUBRIC) {
      await user.type(screen.getByLabelText(new RegExp(c.code)), '3');
    }
    await user.click(screen.getByRole('button', { name: 'Grade (provisional)' }));
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
    expect(await screen.findByText(/Total 80 · PASS/)).toBeInTheDocument();
    expect(screen.queryAllByRole('alert').filter((a) => a.hasAttribute('data-category'))).toHaveLength(0);
  });

  it('starts with empty inputs, keeps Grade disabled until every criterion holds an entered level 0–4, and resets per attempt', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const first = await attemptInEvidenceReview(harness.client);
    // The seeded GRADED_PROVISIONAL attempt goes back to EVIDENCE_REVIEW for a regrade.
    const second = 'mock-attempt-prev';
    const back = await harness.client.invoke('assessment.review_evidence', { attempt_id: second });
    expect(isG9Error(back)).toBe(false);
    renderWithIpc(<Assessments />, harness);
    await user.click(await waitFor(() => selectAttempt(first)));
    const grade = screen.getByRole('button', { name: 'Grade (provisional)' });
    const inputs = BASELINE_RUBRIC.map((c) => screen.getByLabelText(new RegExp(c.code)));
    for (const input of inputs) {
      expect(input).toHaveValue(null);
    }
    expect(grade).toBeDisabled();
    for (const input of inputs.slice(0, -1)) {
      await user.type(input, '2');
    }
    expect(grade).toBeDisabled();
    const last = inputs.at(-1);
    if (last === undefined) {
      throw new Error('no inputs');
    }
    await user.type(last, '7');
    expect(grade).toBeDisabled();
    await user.clear(last);
    await user.type(last, '4');
    expect(grade).toBeEnabled();
    // Clearing a criterion to retype it must not send a 0 the reviewer never entered.
    const firstInput = inputs[0];
    if (firstInput === undefined) {
      throw new Error('no inputs');
    }
    await user.clear(firstInput);
    expect(grade).toBeDisabled();
    await user.click(grade);
    expect(harness.calls.some((c) => c.op === 'assessment.grade_provisional')).toBe(false);
    // Another attempt starts from empty inputs.
    await user.click(selectAttempt(second));
    for (const c of BASELINE_RUBRIC) {
      expect(screen.getByLabelText(new RegExp(c.code))).toHaveValue(null);
    }
    expect(screen.getByRole('button', { name: 'Grade (provisional)' })).toBeDisabled();
  });
});
