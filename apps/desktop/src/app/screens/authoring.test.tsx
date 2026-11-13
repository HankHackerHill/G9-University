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
    expect(within(panel).getByTitle('UNASSESSED')).toHaveTextContent('Not assessed');
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
    // The form opens from the page head's primary action, with the caret in its first field.
    await user.click(await screen.findByRole('button', { name: 'Define an assessment' }));
    const form = await screen.findByRole('form', { name: 'Define an assessment' });
    expect(document.activeElement).toBe(within(form).getByLabelText('Code'));
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
    // The composer closes and a receipt says what was written: code, title, kind, difficulty, the competency, and the rubric criteria the backend returned.
    const defined = await screen.findByTestId('assessment-defined');
    expect(screen.queryByRole('form', { name: 'Define an assessment' })).toBeNull();
    expect(defined).toHaveTextContent('Assessment defined: WCP-01 “Poison a cache key”, capstone, difficulty 4, for Web cache poisoning.');
    expect(defined).toHaveTextContent('Rubric criteria' + 'independent_execution, root_cause_reasoning');
    expect(defined).toHaveTextContent(comp.competency_id);
    const definitions = screen.getByRole('list', { name: 'Assessment definitions' });
    await waitFor(() => {
      expect(within(definitions).getByText('WCP-01')).toBeInTheDocument();
    });
    expect(within(definitions).getByText('Poison a cache key')).toBeInTheDocument();
    // It can be begun like any other.
    expect(within(definitions).getByRole('button', { name: 'Begin Poison a cache key' })).toBeEnabled();
  });

  it('a typed competency id that does not exist is refused NOT_FOUND by the backend', async () => {
    const user = userEvent.setup();
    renderWithIpc(<Assessments />);
    await user.click(await screen.findByRole('button', { name: 'Define an assessment' }));
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
    // A refused definition keeps the form open with what was typed.
    expect(within(screen.getByRole('form', { name: 'Define an assessment' })).getByLabelText('Competency id')).toHaveValue('comp-missing');
    // Closing the form takes its refusal with it.
    await user.click(within(form).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('form', { name: 'Define an assessment' })).toBeNull();
    expect(screen.queryAllByRole('alert')).toHaveLength(0);
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
    // A line of the attempts list is a button; the attempt id is its title.
    const attempts = await screen.findByRole('region', { name: 'Attempts' });
    await user.click(await within(attempts).findByTitle(attempt.attempt_id));
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
    // No notes were typed, so none are sent.
    expect(payload).not.toHaveProperty('notes');
    expect(await screen.findAllByTitle('SUBMITTED')).not.toHaveLength(0);
  });

  it('sends the notes typed for the submission with it, and starts the next attempt from an empty field', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const attempt = ok(await harness.client.invoke('assessment.begin', { assessment_id: 'mock-assessment-D18' }));
    renderWithIpc(<Assessments />, harness);
    await user.click(await within(await screen.findByRole('region', { name: 'Attempts' })).findByTitle(attempt.attempt_id));
    await user.type(screen.getByLabelText('Notes for this submission (optional)'), '  Used the staging target only.  ');
    // The notes belong to this attempt: another attempt shows no notes field content, and coming back keeps them.
    await user.click(within(screen.getByRole('region', { name: 'Attempts' })).getByTitle('mock-attempt-prev'));
    expect(screen.queryByLabelText('Notes for this submission (optional)')).toBeNull();
    await user.click(within(screen.getByRole('region', { name: 'Attempts' })).getByTitle(attempt.attempt_id));
    expect(screen.getByLabelText('Notes for this submission (optional)')).toHaveValue('  Used the staging target only.  ');
    await user.click(within(screen.getByRole('group', { name: 'Attempt transitions' })).getByRole('button', { name: 'Submit' }));
    const submit = harness.calls.find((c) => c.op === 'assessment.submit');
    expect(submit?.payload).toEqual({ attempt_id: attempt.attempt_id, evidence_refs: [], notes: 'Used the staging target only.' });
    expect(submit?.meta.expected_revision).toBe(attempt.revision);
    expect(await screen.findAllByTitle('SUBMITTED')).not.toHaveLength(0);
  });
});
