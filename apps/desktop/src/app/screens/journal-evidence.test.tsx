/**
 * Evidence of a note in the Learning Journal: a picked file is read with FileReader, sent as
 * base64 without the `data:` prefix (`evidence.attach_file`), and listed with the note's
 * evidence (`evidence.list`); the note itself becomes evidence (`evidence.from_note`). Files the
 * backend would refuse by type or size are refused before anything is sent.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { RequestOf } from '../../ipc/operations';
import { decodeBase64Strict } from '../../ipc/transports/mock-attachments';
import { MAX_ATTACHMENT_BYTES } from '../../lib/attachment';
import { sha256Hex } from '../../lib/sha256';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { LearningJournal } from './LearningJournal';

const FINDINGS = '# Findings\n\nX-Forwarded-Host is unkeyed.\n';

async function saveNote(user: ReturnType<typeof userEvent.setup>, title: string): Promise<HTMLElement> {
  expect(await screen.findByText(/save the note first/)).toBeInTheDocument();
  await user.type(screen.getByLabelText('Title'), title);
  await user.click(screen.getByRole('button', { name: 'Save note' }));
  return screen.findByLabelText('Attach a file to this note');
}

describe('Learning Journal: evidence of a note', () => {
  it('attaches a small text file to the saved note and lists it; the note can be made evidence too', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Cache poisoning lab');
    await user.upload(input, new File([FINDINGS], 'findings.md', { type: 'text/markdown' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent(`Attached findings.md (${new TextEncoder().encode(FINDINGS).length} B, SHA-256 ${sha256Hex(FINDINGS).slice(0, 16)}`);
    const call = harness.calls.find((c) => c.op === 'evidence.attach_file');
    const payload = call?.payload as RequestOf<'evidence.attach_file'>;
    expect(payload).toMatchObject({ filename: 'findings.md', media_type: 'text/markdown' });
    expect(payload.content_base64.startsWith('data:')).toBe(false);
    expect(new TextDecoder().decode(decodeBase64Strict(payload.content_base64) ?? new Uint8Array())).toBe(FINDINGS);
    expect(call?.meta.scope.domain).toBe('LEARNING');
    const table = await screen.findByRole('table', { name: 'Evidence of this note' });
    expect(within(table).getByText('findings.md')).toBeInTheDocument();
    expect(within(table).getByText('text/markdown')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Use as evidence' }));
    expect(await screen.findByTestId('note-evidence')).toHaveTextContent('(“Cache poisoning lab”)');
    await waitFor(() => {
      // header row + the attachment + the note
      expect(within(screen.getByRole('table', { name: 'Evidence of this note' })).getAllByRole('row')).toHaveLength(3);
    });
    expect(within(screen.getByRole('table', { name: 'Evidence of this note' })).getByText('Cache poisoning lab')).toBeInTheDocument();
  });

  it('maps a file without a declared type by its extension, and reports identical bytes as stored once', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Data');
    await user.upload(input, new File(['{"ok": true}'], 'result.json', { type: '' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent('Attached result.json');
    await user.upload(input, new File(['{"ok": true}'], 'copy.json', { type: '' }));
    expect(await screen.findByTestId('attached')).toHaveTextContent('Attached copy.json');
    expect(screen.getByTestId('attached')).toHaveTextContent('identical bytes were already in the store and are kept once');
    const types = harness.calls.filter((c) => c.op === 'evidence.attach_file').map((c) => (c.payload as RequestOf<'evidence.attach_file'>).media_type);
    expect(types).toEqual(['application/json', 'application/json']);
  });

  it('refuses an unsupported type and a file over 16 MiB before sending anything', async () => {
    // The chooser's `accept` list would hide these files; a drop or a changed filter would not.
    const user = userEvent.setup({ applyAccept: false });
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<LearningJournal />, harness);
    const input = await saveNote(user, 'Refusals');
    await user.upload(input, new File(['MZ'], 'tool.exe', { type: 'application/x-msdownload' }));
    expect(await screen.findByTestId('attach-refused')).toHaveTextContent(
      'tool.exe was not attached: only Markdown, plain text, CSV, JSON, PDF, PNG and JPEG files can be attached.',
    );
    await user.upload(input, new File([new Uint8Array(MAX_ATTACHMENT_BYTES + 1)], 'big.txt', { type: 'text/plain' }));
    expect(await screen.findByTestId('attach-refused')).toHaveTextContent(`big.txt is 16 MiB (${MAX_ATTACHMENT_BYTES + 1} bytes); a file attached in the app is at most 16 MiB`);
    await user.upload(input, new File([], 'empty.txt', { type: 'text/plain' }));
    expect(await screen.findByTestId('attach-refused')).toHaveTextContent('empty.txt is empty');
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
    expect(await screen.findByText('no evidence for this note yet', { exact: false })).toBeInTheDocument();
  });
});
