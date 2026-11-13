/**
 * Authoring from the window: a competency (`competency.create`, Competency Graph), an assessment
 * for it (`assessment.define`, Assessments), and the evidence picked in the submit step
 * (`evidence.list` → `assessment.submit` `evidence_refs`).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { OperationName, RequestOf } from '../../ipc/operations';
import { encodeBase64 } from '../../ipc/transports/mock-attachments';
import { createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { Assessments } from './Assessments';
import { CompetencyGraph } from './CompetencyGraph';

function ok<K extends OperationName>(r: InvokeResult<K>) {
  if (isG9Error(r)) {
    throw new Error(`${r.category} ${r.code}`);
  }
  return r.result;
}

describe('Competency Graph: new competency', () => {
  it('creates a competency that is then listed UNASSESSED', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CompetencyGraph />, harness);
    const form = await screen.findByRole('form', { name: 'New competency' });
    await user.type(within(form).getByLabelText('Code'), 'wcp');
    expect(within(form).getByLabelText('Code')).toHaveValue('WCP');
    await user.type(within(form).getByLabelText('Title'), 'Web cache poisoning');
    await user.clear(within(form).getByLabelText('Validity (days)'));
    await user.type(within(form).getByLabelText('Validity (days)'), '120');
    await user.type(within(form).getByLabelText('Description (optional)'), 'Unkeyed inputs change cached responses.');
    await user.click(within(form).getByRole('button', { name: 'Create competency' }));
    expect(harness.calls.find((c) => c.op === 'competency.create')?.payload).toEqual({
      code: 'WCP',
      title: 'Web cache poisoning',
      validity_days: 120,
      description: 'Unkeyed inputs change cached responses.',
    });
    expect(await screen.findByTestId('competency-created')).toHaveTextContent('Created WCP “Web cache poisoning”');
    const tree = screen.getByRole('list', { name: 'Competencies' });
    const panel = await within(tree).findByRole('region', { name: 'Web cache poisoning' });
    expect(within(panel).getByText('UNASSESSED')).toBeInTheDocument();
  });

  it('shows the backend refusal of a duplicate code', async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    ok(await harness.client.invoke('competency.create', { code: 'DUP', title: 'First' }));
    renderWithIpc(<CompetencyGraph />, harness);
    const form = await screen.findByRole('form', { name: 'New competency' });
    await user.type(within(form).getByLabelText('Code'), 'DUP');
    await user.type(within(form).getByLabelText('Title'), 'Second');
    await user.click(within(form).getByRole('button', { name: 'Create competency' }));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveAttribute('data-category', 'CONFLICT');
    expect(alert).toHaveTextContent('storage.unique_violation');
  });
});

describe('Assessments: define an assessment', () => {
  it('defines an assessment for a listed competency; it appears among the definitions under its code', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const comp = ok(await harness.client.invoke('competency.create', { code: 'WCP', title: 'Web cache poisoning' }));
    renderWithIpc(<Assessments />, harness);
    const form = await screen.findByRole('form', { name: 'Define an assessment' });
    await user.type(within(form).getByLabelText('Code'), 'wcp-01');
    await user.type(within(form).getByLabelText('Title'), 'Poison a cache key');
    await user.selectOptions(within(form).getByLabelText('Kind'), 'CAPSTONE');
    await user.selectOptions(within(form).getByLabelText('Difficulty'), '4');
    await within(form).findByRole('option', { name: `Web cache poisoning (${comp.competency_id})` });
    await user.selectOptions(within(form).getByLabelText('Competency'), comp.competency_id);
    expect(within(form).queryByLabelText('Competency id')).toBeNull();
    await user.type(within(form).getByLabelText('Task statement'), 'Find an unkeyed header that changes the cached response.');
    await user.click(within(form).getByRole('button', { name: 'Define assessment' }));
    expect(harness.calls.find((c) => c.op === 'assessment.define')?.payload).toEqual({
      code: 'WCP-01',
      title: 'Poison a cache key',
      kind: 'CAPSTONE',
      competency_id: comp.competency_id,
      task_statement: 'Find an unkeyed header that changes the cached response.',
      difficulty: 4,
    });
    const defined = await screen.findByTestId('assessment-defined');
    expect(defined).toHaveTextContent('Defined WCP-01 “Poison a cache key”');
    expect(defined).toHaveTextContent('difficulty 4');
    expect(defined).toHaveTextContent('Rubric criteria: independent_execution, root_cause_reasoning');
    const definitions = screen.getByRole('table', { name: 'Assessment definitions' });
    await waitFor(() => {
      expect(within(definitions).getByText('WCP-01')).toBeInTheDocument();
    });
    expect(within(definitions).getByText('Poison a cache key')).toBeInTheDocument();
  });

  it('a typed competency id that does not exist is refused NOT_FOUND by the backend', async () => {
    const user = userEvent.setup();
    renderWithIpc(<Assessments />);
    const form = await screen.findByRole('form', { name: 'Define an assessment' });
    await user.type(within(form).getByLabelText('Code'), 'X-1');
    await user.type(within(form).getByLabelText('Title'), 'Nowhere');
    await user.type(within(form).getByLabelText('Competency id'), 'comp-missing');
    await user.type(within(form).getByLabelText('Task statement'), 'x');
    await user.click(within(form).getByRole('button', { name: 'Define assessment' }));
    const refusal = await waitFor(() => {
      const found = screen.queryAllByRole('alert').find((a) => a.getAttribute('data-category') === 'NOT_FOUND');
      expect(found).toBeDefined();
      return found;
    });
    expect(refusal).toHaveTextContent('competency.not_found');
    expect(screen.queryByTestId('assessment-defined')).toBeNull();
  });
});

describe('Assessments: evidence in the submit step', () => {
  it('sends the evidence checked for the attempt as evidence_refs, and only those', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const note = ok(await harness.client.invoke('notes.create', { title: 'Cache poisoning lab', body_markdown: 'Lab write-up.', classification: 'INTERNAL', domain: 'LEARNING' }));
    const file = ok(
      await harness.client.invoke('evidence.attach_file', {
        note_id: note.note_id,
        filename: 'findings.md',
        media_type: 'text/markdown',
        content_base64: encodeBase64(new TextEncoder().encode('# Findings\n')),
      }),
    );
    const fromNote = ok(await harness.client.invoke('evidence.from_note', { note_id: note.note_id }));
    const attempt = ok(await harness.client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' }));
    renderWithIpc(<Assessments />, harness);
    const attempts = await screen.findByRole('table', { name: 'Attempts' });
    const row = (await within(attempts).findByText(attempt.attempt_id)).closest('tr');
    if (row === null) {
      throw new Error('attempt row not found');
    }
    await user.click(within(row).getByRole('button', { name: 'Select' }));
    const picker = await screen.findByRole('group', { name: 'Evidence for this submission' });
    const fileBox = await within(picker).findByRole('checkbox', { name: /findings\.md/ });
    const noteBox = within(picker).getByRole('checkbox', { name: /Cache poisoning lab/ });
    await user.click(fileBox);
    expect(fileBox).toBeChecked();
    expect(noteBox).not.toBeChecked();
    await user.click(within(screen.getByRole('group', { name: 'Attempt transitions' })).getByRole('button', { name: 'Submit' }));
    const submit = harness.calls.find((c) => c.op === 'assessment.submit');
    const payload = submit?.payload as RequestOf<'assessment.submit'> | undefined;
    expect(payload?.evidence_refs).toEqual([file.evidence_id]);
    expect(payload?.evidence_refs).not.toContain(fromNote.evidence_id);
    expect(await screen.findAllByText('SUBMITTED')).not.toHaveLength(0);
  });
});
