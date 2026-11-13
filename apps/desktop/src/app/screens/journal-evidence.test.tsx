/**
 * Evidence of a note in the Learning Journal: a picked file is read with FileReader, sent as
 * base64 without the `data:` prefix (`evidence.attach_file`), and listed with the note's
 * evidence (`evidence.list`); the note itself becomes evidence (`evidence.from_note`). Files the
 * backend would refuse by type or size are refused before anything is sent.
 *
 * The editor: a saved note is listed and selected, a revision carries the record revision it was
 * read at, unsaved text is never dropped without asking, and a protected evidence action raises
 * its voucher under the control that asked.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { ApprovalRequest, RequestOf } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { decodeBase64Strict } from '../../ipc/transports/mock-attachments';
import { MAX_ATTACHMENT_BYTES } from '../../lib/attachment';
import { sha256Hex } from '../../lib/sha256';
import { createInterceptHarness, outcome } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { LearningJournal } from './LearningJournal';

const FINDINGS = '# Findings\n\nX-Forwarded-Host is unkeyed.\n';

async function saveNote(user: ReturnType<typeof userEvent.setup>, title: string): Promise<HTMLElement> {
  // Nothing attaches to a note the backend does not have yet: the evidence region says so instead of offering controls.
  expect(await screen.findByText(/Save it first: files and evidence attach to a saved note/)).toBeInTheDocument();
  expect(screen.queryByLabelText('File to attach')).toBeNull();
  await user.type(screen.getByLabelText('Title'), title);
  await user.click(screen.getByRole('button', { name: 'Save note' }));
  return screen.findByLabelText('File to attach');
}

describe('Learning Journal: evidence of a note', () => {
  it('attaches a small text file to the saved note and lists it; the note can be made evidence too', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Cache poisoning lab');
    await user.upload(input, new File([FINDINGS], 'findings.md', { type: 'text/markdown' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent(`File attached: findings.md (${new TextEncoder().encode(FINDINGS).length} B, SHA-256 ${sha256Hex(FINDINGS).slice(0, 16)}`);
    const call = harness.calls.find((c) => c.op === 'evidence.attach_file');
    const payload = call?.payload as RequestOf<'evidence.attach_file'>;
    expect(payload).toMatchObject({ filename: 'findings.md', media_type: 'text/markdown' });
    expect(payload.content_base64.startsWith('data:')).toBe(false);
    expect(new TextDecoder().decode(decodeBase64Strict(payload.content_base64) ?? new Uint8Array())).toBe(FINDINGS);
    expect(call?.meta.scope.domain).toBe('LEARNING');
    const list = await screen.findByRole('list', { name: 'Evidence items' });
    expect(within(list).getByText('findings.md')).toBeInTheDocument();
    expect(within(list).getByText('text/markdown')).toBeInTheDocument();
    // The digest is shortened on the line; the whole value is one step away, in its title.
    expect(within(list).getByTitle(sha256Hex(FINDINGS))).toHaveTextContent(`${sha256Hex(FINDINGS).slice(0, 12)}…`);

    await user.click(screen.getByRole('button', { name: 'Use this note as evidence' }));
    expect(await screen.findByTestId('note-evidence')).toHaveTextContent('Note used as evidence: “Cache poisoning lab” is evidence item');
    await waitFor(() => {
      // the attachment + the note
      expect(within(screen.getByRole('list', { name: 'Evidence items' })).getAllByRole('listitem')).toHaveLength(2);
    });
    const items = within(screen.getByRole('list', { name: 'Evidence items' })).getAllByRole('listitem');
    expect(items.some((li) => within(li).queryByText('Cache poisoning lab') !== null && within(li).queryByText(/^Note, added /) !== null)).toBe(true);
    expect(items.some((li) => within(li).queryByText('findings.md') !== null && within(li).queryByText(/^Attached file/) !== null)).toBe(true);
  });

  it('maps a file without a declared type by its extension, and reports identical bytes as stored once', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Data');
    await user.upload(input, new File(['{"ok": true}'], 'result.json', { type: '' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent('File attached: result.json');
    await user.upload(input, new File(['{"ok": true}'], 'copy.json', { type: '' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent('File attached: copy.json');
    expect(screen.getByTestId('attached')).toHaveTextContent('Identical bytes were already in the store and are kept once');
    const types = harness.calls.filter((c) => c.op === 'evidence.attach_file').map((c) => (c.payload as RequestOf<'evidence.attach_file'>).media_type);
    expect(types).toEqual(['application/json', 'application/json']);
  });

  it('refuses an unsupported type and a file over 16 MiB before sending anything', async () => {
    // The chooser's `accept` list would hide these files; a drop or a changed filter would not.
    const user = userEvent.setup({ applyAccept: false });
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Refusals');
    // The refusal is the field's own error: under the file field, announced, and tied to the control.
    const refused = (): HTMLElement => {
      const field = input.closest('.field');
      if (!(field instanceof HTMLElement)) {
        throw new Error('the file field was not found');
      }
      return within(field).getByRole('alert');
    };
    await user.upload(input, new File(['MZ'], 'tool.exe', { type: 'application/x-msdownload' }));
    await waitFor(() => {
      expect(refused()).toHaveTextContent('tool.exe was not attached: only Markdown, plain text, CSV, JSON, PDF, PNG and JPEG files can be attached.');
    });
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription(/tool\.exe was not attached/);
    await user.upload(input, new File([new Uint8Array(MAX_ATTACHMENT_BYTES + 1)], 'big.txt', { type: 'text/plain' }));
    await waitFor(() => {
      expect(refused()).toHaveTextContent(`big.txt is 16 MiB (${MAX_ATTACHMENT_BYTES + 1} bytes); a file attached in the app is at most 16 MiB`);
    });
    await user.upload(input, new File([], 'empty.txt', { type: 'text/plain' }));
    await waitFor(() => {
      expect(refused()).toHaveTextContent('empty.txt is empty');
    });
    expect(harness.calls.some((c) => c.op === 'evidence.attach_file')).toBe(false);
  });

  it('shows the backend refusal when the bytes do not match the declared type', async () => {
    const user = userEvent.setup();
    renderWithIpc(<LearningJournal />);
    const input = await saveNote(user, 'Screenshots');
    await user.upload(input, new File(['not a png at all'], 'shot.png', { type: 'image/png' }));
    const refusal = await screen.findByRole('alert');
    expect(refusal).toHaveAttribute('data-category', 'INVALID_INPUT');
    expect(refusal).toHaveTextContent('attachment.content_mismatch');
    expect(await screen.findByText('No evidence yet')).toBeInTheDocument();
    expect(screen.queryByTestId('attached')).toBeNull();
  });

  it('raises the approval voucher directly under the control that asked, and says so when the confirmation is cancelled', async () => {
    const user = userEvent.setup();
    // The mock asks for approval only for a SECURITY_SENSITIVE note, which this screen cannot create; the backend's answer is reproduced here.
    const pending: ApprovalRequest = {
      approval_id: 'approval-note-evidence',
      state: 'PENDING',
      operation_name: 'evidence.from_note',
      operation: {
        action: 'CREATE',
        resource: { kind: 'EVIDENCE_ITEM', id: 'evidence-1' },
        scope: { domain: 'SECURITY_SENSITIVE', mode: 'STUDY' },
        parameters_sha256: 'a'.repeat(64),
      },
      requested_by: 'operator',
      requested_at: Date.UTC(2026, 10, 18, 8, 30),
      expires_at: Date.UTC(2026, 10, 18, 8, 40),
      expected_revision: null,
      decided_at: null,
      consumed_at: null,
      summary: 'Use a protected note as evidence',
      revision: 1,
    };
    const required: G9Error = { category: 'APPROVAL_REQUIRED', code: 'approval.required', message: 'operator approval required', approval_id: pending.approval_id };
    const notConfirmed: G9Error = { category: 'UNAUTHORIZED', code: 'approval.not_confirmed', message: 'the confirmation was cancelled' };
    const harness = createInterceptHarness((op, _payload, next) => {
      if (op === 'evidence.from_note') {
        return Promise.resolve(required);
      }
      if (op === 'approvals.list') {
        return Promise.resolve(outcome({ approvals: [pending] }));
      }
      return op === 'approvals.decide' ? Promise.resolve(notConfirmed) : next();
    });
    renderWithIpc(<LearningJournal />, harness);
    await saveNote(user, 'Scope letter');
    const use = screen.getByRole('button', { name: 'Use this note as evidence' });
    await user.click(use);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByRole('heading', { name: 'Use a protected note as evidence' })).toBeInTheDocument();
    expect(within(card).getByTestId('approval-hash')).toHaveTextContent('a'.repeat(64));
    expect(screen.getByRole('alert')).toHaveAttribute('data-category', 'APPROVAL_REQUIRED');
    // Under the control that asked: after the button, before the list of evidence items.
    expect(use.compareDocumentPosition(card) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(card.compareDocumentPosition(screen.getByText('No evidence yet')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByTestId('note-evidence')).toBeNull();

    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('flow-confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    expect(screen.getByTestId('approval-card')).toBeInTheDocument();
    expect(harness.calls.filter((c) => c.op === 'evidence.from_note')).toHaveLength(1);
  });
});

describe('Learning Journal: the editor', () => {
  it('lists the saved note, selects it, and saves a revision at the record revision it was read at', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    expect(await screen.findByText('No notes yet')).toBeInTheDocument();
    await user.type(screen.getByLabelText('Title'), 'Session fixation');
    await user.selectOptions(screen.getByLabelText('Classification'), 'INTERNAL');
    // Typed text the backend does not have is stamped as such.
    expect(screen.getByLabelText('Standing: not saved')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Save note' }));
    expect(harness.calls.find((c) => c.op === 'notes.create')?.payload).toMatchObject({ title: 'Session fixation', classification: 'INTERNAL', domain: 'LEARNING' });
    expect(await screen.findByTestId('note-saved')).toHaveTextContent('Note saved: “Session fixation”, version 1. It is selected in the list.');
    const list = screen.getByRole('list', { name: 'Notes saved in this session' });
    const row = within(list).getByRole('button', { name: /^Session fixation/ });
    expect(row).toHaveAttribute('aria-pressed', 'true');
    expect(row).toHaveTextContent('Learning area, internal');
    expect(row).toHaveTextContent('1 version');
    // The saved record: stamped, with the backend's hash of the text and its revision.
    const editor = screen.getByRole('region', { name: 'Editor: Session fixation' });
    expect(within(editor).getByLabelText('Provenance: verified')).toBeInTheDocument();
    expect(within(editor).getByTestId('content-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    expect(within(editor).getByTestId('revision')).toHaveTextContent('revision 1');
    expect(screen.getByLabelText('Classification')).toBeDisabled();

    await user.type(screen.getByLabelText('Markdown'), 'Regenerate the id at login.');
    expect(screen.queryByTestId('note-saved')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Save revision' }));
    const revise = harness.calls.find((c) => c.op === 'notes.revise');
    expect(revise?.meta.expected_revision).toBe(1);
    expect((revise?.payload as RequestOf<'notes.revise'>).body_markdown.endsWith('Regenerate the id at login.')).toBe(true);
    expect(await screen.findByTestId('note-saved')).toHaveTextContent('Revision saved: “Session fixation”, now 2 versions.');
    expect(within(list).getByRole('button', { name: /^Session fixation/ })).toHaveTextContent('2 versions');
    expect(within(editor).getByTestId('revision')).toHaveTextContent('revision 2');
  });

  it('shows the sanitized preview of what is typed on its own tab', async () => {
    const user = userEvent.setup();
    renderWithIpc(<LearningJournal />);
    const editor = await screen.findByRole('region', { name: 'Editor: new note' });
    await user.clear(within(editor).getByLabelText('Markdown'));
    await user.type(within(editor).getByLabelText('Markdown'), '# Cookies<script>alert(1)</script>');
    await user.click(within(editor).getByRole('tab', { name: 'Preview' }));
    const preview = within(editor).getByTestId('markdown-preview');
    expect(within(preview).getByRole('heading', { name: 'Cookies' })).toBeInTheDocument();
    expect(preview.querySelector('script')).toBeNull();
  });

  it('a stale revision surfaces as the error it is and keeps what was typed', async () => {
    const user = userEvent.setup();
    const stale: G9Error = { category: 'STALE_REVISION', code: 'revision.stale', message: 'the record changed since it was read; reload and retry' };
    const harness = createInterceptHarness((op, _payload, next) => (op === 'notes.revise' ? Promise.resolve(stale) : next()));
    renderWithIpc(<LearningJournal />, harness);
    await saveNote(user, 'Race');
    await user.type(screen.getByLabelText('Markdown'), 'second thought');
    await user.click(screen.getByRole('button', { name: 'Save revision' }));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveAttribute('data-category', 'STALE_REVISION');
    expect(alert).toHaveTextContent('This record changed');
    expect(screen.queryByTestId('note-saved')).toBeNull();
    expect(screen.getByLabelText<HTMLTextAreaElement>('Markdown').value.endsWith('second thought')).toBe(true);
    expect(screen.getByTestId('revision')).toHaveTextContent('revision 1');
  });

  it('asks before a new note replaces text that is not saved', async () => {
    const user = userEvent.setup();
    renderWithIpc(<LearningJournal />);
    const newNote = await screen.findByRole('button', { name: 'New note' });
    await user.type(screen.getByLabelText('Title'), 'Half a thought');
    await user.click(newNote);
    const dialog = await screen.findByRole('alertdialog', { name: 'Discard the text that is not saved?' });
    expect(dialog).toHaveTextContent('This new note is not saved yet. Starting a new note replaces what is in the editor');
    // The safe choice has the focus and keeps everything.
    const keep = within(dialog).getByRole('button', { name: 'Keep writing' });
    expect(document.activeElement).toBe(keep);
    await user.click(keep);
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(screen.getByLabelText('Title')).toHaveValue('Half a thought');

    await user.click(newNote);
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Discard the changes' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(screen.getByLabelText('Title')).toHaveValue('');
    expect(document.activeElement).toBe(screen.getByLabelText('Title'));
  });
});
