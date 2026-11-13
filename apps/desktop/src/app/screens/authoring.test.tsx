/**
 * Authoring from the window: a competency (`competency.create`, Competency Graph), an assessment
 * for it (`assessment.define`, Assessments), and the evidence picked and the notes typed in the
 * submit step (`evidence.list` → `assessment.submit` `evidence_refs`, `notes`). Both forms open
 * from the page head's primary action. Also here: what the Competency Graph shows of a projection.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error, type InvokeResult } from '../../ipc/client';
import type { CompetencyProjection, OperationName, RequestOf } from '../../ipc/operations';
import { encodeBase64 } from '../../ipc/transports/mock-attachments';
import { STORAGE_BUSY, createInterceptHarness, outcome } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { Assessments } from './Assessments';
import { CompetencyGraph } from './CompetencyGraph';

function ok<K extends OperationName>(r: InvokeResult<K>) {
  if (isG9Error(r)) {
    throw new Error(`${r.category} ${r.code}`);
  }
  return r.result;
}

describe('Competency Graph: add a competency', () => {
  it('adds a competency that is then listed, selected and not assessed, with no figure where no observation exists', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CompetencyGraph />, harness);
    // The form opens from the page head's primary action, with the caret in its first field.
    await user.click(await screen.findByRole('button', { name: 'Add a competency' }));
    const form = await screen.findByRole('form', { name: 'Add a competency' });
    expect(document.activeElement).toBe(within(form).getByLabelText('Code'));
    await user.type(within(form).getByLabelText('Code'), 'wcp');
    expect(within(form).getByLabelText('Code')).toHaveValue('WCP');
    await user.type(within(form).getByLabelText('Title'), 'Web cache poisoning');
    await user.clear(within(form).getByLabelText('Validity in days'));
    await user.type(within(form).getByLabelText('Validity in days'), '120');
    await user.type(within(form).getByLabelText('Description (optional)'), 'Unkeyed inputs change cached responses.');
    await user.click(within(form).getByRole('button', { name: 'Add competency' }));
    expect(harness.calls.find((c) => c.op === 'competency.create')?.payload).toEqual({
      code: 'WCP',
      title: 'Web cache poisoning',
      validity_days: 120,
      description: 'Unkeyed inputs change cached responses.',
    });
    expect(await screen.findByTestId('competency-created')).toHaveTextContent('Competency added: WCP “Web cache poisoning”, valid for 120 days.');
    expect(screen.queryByRole('form', { name: 'Add a competency' })).toBeNull();
    // Listed as the pressed line, stamped Not assessed (exact name UNASSESSED in the stamp's title)...
    const list = screen.getByRole('list', { name: 'Competencies' });
    const line = await within(list).findByRole('button', { name: /^Web cache poisoning/ });
    expect(line).toHaveAttribute('aria-pressed', 'true');
    expect(within(line).getByTitle('UNASSESSED')).toHaveTextContent('Not assessed');
    // ...and shown in the detail region named by it, with the same stamp.
    const panel = await screen.findByRole('region', { name: 'Web cache poisoning' });
    expect(within(panel).getByTitle('UNASSESSED')).toHaveTextContent('Not assessed');
    // Nothing is observed in any dimension yet: the table says so once, in words, under its five columns. Never a dash, never a zero.
    const table = within(panel).getByRole('table', { name: 'Estimates by dimension' });
    expect(within(table).getAllByRole('columnheader').map((th) => th.textContent)).toEqual(['Dimension', 'Estimate', 'Unaided observations', 'Last observed', 'Uncertainty']);
    expect(within(table).getAllByRole('row').slice(1).map((row) => row.textContent)).toEqual([
      'No observation yet in any of the four dimensions: recall, understanding, application and transfer.',
    ]);
    expect(panel).not.toHaveTextContent('—');
    expect(within(panel).queryByText('0')).toBeNull();
  });

  it('shows the backend refusal of a duplicate code and keeps the form open with what was typed', async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    ok(await harness.client.invoke('competency.create', { code: 'DUP', title: 'First' }));
    renderWithIpc(<CompetencyGraph />, harness);
    await user.click(await screen.findByRole('button', { name: 'Add a competency' }));
    const form = await screen.findByRole('form', { name: 'Add a competency' });
    await user.type(within(form).getByLabelText('Code'), 'DUP');
    await user.type(within(form).getByLabelText('Title'), 'Second');
    await user.click(within(form).getByRole('button', { name: 'Add competency' }));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveAttribute('data-category', 'CONFLICT');
    expect(alert).toHaveTextContent('storage.unique_violation');
    expect(screen.queryByTestId('competency-created')).toBeNull();
    expect(within(screen.getByRole('form', { name: 'Add a competency' })).getByLabelText('Title')).toHaveValue('Second');
  });

  it('keeps Add competency disabled for a malformed code or a validity outside 1 to 3650', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CompetencyGraph />, harness);
    await user.click(await screen.findByRole('button', { name: 'Add a competency' }));
    const form = await screen.findByRole('form', { name: 'Add a competency' });
    const submit = within(form).getByRole('button', { name: 'Add competency' });
    expect(submit).toBeDisabled();
    await user.type(within(form).getByLabelText('Title'), 'Web cache poisoning');
    await user.type(within(form).getByLabelText('Code'), '-x');
    expect(within(form).getByLabelText('Code')).toHaveAttribute('aria-invalid', 'true');
    expect(submit).toBeDisabled();
    await user.clear(within(form).getByLabelText('Code'));
    await user.type(within(form).getByLabelText('Code'), 'wcp');
    expect(submit).toBeEnabled();
    await user.clear(within(form).getByLabelText('Validity in days'));
    await user.type(within(form).getByLabelText('Validity in days'), '3651');
    expect(submit).toBeDisabled();
    expect(harness.calls.some((c) => c.op === 'competency.create')).toBe(false);
  });
});

describe('Competency Graph: the projection', () => {
  const T = Date.UTC(2026, 10, 17, 9, 0);
  const none = { score_estimate: null, n_unaided: 0, last_observed_at: null, uncertainty: null };
  const projected: CompetencyProjection[] = [
    {
      competency_id: 'comp-http',
      name: 'HTTP analysis',
      state: 'INDEPENDENT',
      provisional: true,
      per_dimension: {
        RECALL: none,
        UNDERSTANDING: { score_estimate: 76.66666, n_unaided: 0, last_observed_at: T, uncertainty: 0.25 },
        APPLICATION: { score_estimate: 82, n_unaided: 2, last_observed_at: T, uncertainty: 0 },
        TRANSFER: none,
      },
      contradictions: ['Recall failed on 2026-11-16 after a passing practical.'],
      evidence_refs: ['obs-1', 'obs-2'],
      objective_ids: ['D13-O1'],
    },
    { competency_id: 'comp-acl', name: 'Access-control matrix', state: 'RETEST_DUE', provisional: false, per_dimension: { RECALL: none, UNDERSTANDING: none, APPLICATION: none, TRANSFER: none }, contradictions: [], evidence_refs: [], objective_ids: [] },
    { competency_id: 'comp-new', name: 'Session review', state: 'UNASSESSED', provisional: false, per_dimension: { RECALL: none, UNDERSTANDING: none, APPLICATION: none, TRANSFER: none }, contradictions: [], evidence_refs: [], objective_ids: [] },
  ];

  it('draws the distribution over the states with its table view, and the selected competency dimension by dimension', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((op, _payload, next) => (op === 'competency.projection' ? Promise.resolve(outcome({ derived_at: T, projections: projected })) : next()));
    renderWithIpc(<CompetencyGraph />, harness);
    // Several states hold something, so the distribution is a chart with a key and a table of the same figures.
    expect(
      await screen.findByRole('img', { name: 'Competencies by state: Not assessed 1, Introduced 0, Guided 0, Independent 1, Validated 0, Retest due 1' }),
    ).toBeInTheDocument();
    const byState = screen.getByRole('table', { name: 'Competencies by state' });
    expect(within(byState).getAllByRole('row').slice(1).map((row) => row.textContent)).toEqual(['Not assessed1', 'Introduced0', 'Guided0', 'Independent1', 'Validated0', 'Retest due1']);
    expect(screen.queryByText(/competencies are/)).toBeNull();
    // The first line is selected: a provisional competency says what provisional means, and shows the backend's figures.
    const list = screen.getByRole('list', { name: 'Competencies' });
    const first = within(list).getByRole('button', { pressed: true });
    expect(first).toHaveTextContent('HTTP analysis');
    expect(first).toHaveTextContent('Observed in 2 of 4 dimensions');
    expect(within(first).getByTitle('PROVISIONAL')).toHaveTextContent('Provisional');
    const panel = screen.getByRole('region', { name: 'HTTP analysis' });
    expect(within(panel).getByTitle('INDEPENDENT')).toHaveTextContent('Independent');
    expect(within(panel).getByText(/^Provisional: this state rests on self-graded evidence\./)).toBeInTheDocument();
    const rows = within(within(panel).getByRole('table', { name: 'Estimates by dimension' })).getAllByRole('row').slice(1);
    expect(rows.map((row) => within(row).getAllByRole('cell').map((cell) => cell.textContent))).toEqual([
      ['Recall', 'No observation yet', 'None', 'No observation yet', 'No observation yet'],
      // Observed, but never unaided: the count is said in words; the estimate is shown to two decimals with the exact value in its title.
      ['Understanding', '76.67', 'None', '2026-11-17 10:00', '0.25'],
      // An uncertainty of 0 is the backend's value, not a missing one.
      ['Application', '82', '2', '2026-11-17 10:00', '0'],
      ['Transfer', 'No observation yet', 'None', 'No observation yet', 'No observation yet'],
    ]);
    expect(within(panel).getByText('76.67')).toHaveAttribute('title', '76.66666');
    expect(panel).toHaveTextContent('Recall failed on 2026-11-16 after a passing practical.');
    expect(panel).toHaveTextContent('obs-1, obs-2');
    expect(panel).toHaveTextContent('D13-O1');
    // Another line shows another competency; one without a flag has no provisional note.
    await user.click(within(list).getByRole('button', { name: /^Access-control matrix/ }));
    const other = screen.getByRole('region', { name: 'Access-control matrix' });
    expect(within(other).getByTitle('RETEST_DUE')).toHaveTextContent('Retest due');
    expect(within(other).queryByText(/^Provisional:/)).toBeNull();
    expect(other).toHaveTextContent('None linked');
    expect(within(other).getByRole('table', { name: 'Estimates by dimension' })).toHaveTextContent(/No observation yet in any of the four dimensions/);
    expect(screen.queryByRole('region', { name: 'HTTP analysis' })).toBeNull();
  });

  it('says in one sentence that every competency is in one state instead of drawing a one-part chart', async () => {
    renderWithIpc(<CompetencyGraph />);
    expect(await screen.findByText(/^All 7 competencies are/)).toHaveTextContent('All 7 competencies are not assessed.');
    expect(screen.queryByRole('img', { name: /Competencies by state/ })).toBeNull();
    // Backend records read without a warning are stamped Verified.
    expect(within(screen.getByRole('region', { name: 'Competencies' })).getByLabelText('Provenance: verified')).toBeInTheDocument();
  });

  it('a failed read shows the error once and "No data" with the code, never an empty list or a chart', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => (op === 'competency.projection' ? STORAGE_BUSY : next()));
    renderWithIpc(<CompetencyGraph />, harness);
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveAttribute('data-category', 'RETRYABLE_STORAGE');
    expect(screen.getAllByText('No data')).toHaveLength(2);
    expect(screen.getAllByText(/The competency projection could not be read \(storage\.busy\)/)).toHaveLength(2);
    expect(screen.queryByRole('list', { name: 'Competencies' })).toBeNull();
    expect(screen.queryByText(/No competencies yet/)).toBeNull();
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
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
    // Copied now: the mock hands out its own record, whose revision moves when the attempt is submitted below.
    const revisionAtBegin = attempt.revision;
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
    // The submission carries the revision the attempt had before it, and the backend moves it on by one.
    expect(submit?.meta.expected_revision).toBe(revisionAtBegin);
    expect(await screen.findAllByTitle('SUBMITTED')).not.toHaveLength(0);
    await waitFor(() => {
      expect(screen.getByTestId('attempt-revision')).toHaveTextContent(`revision ${revisionAtBegin + 1}`);
    });
  });
});
