/**
 * Reading notes back and erasing one from the window (owner request 2026-11-14): the list is the
 * backend's `notes.list`, so notes stored in an earlier session are there; a note is opened with
 * `notes.get` (after the usual question about unsaved text); erasing goes through the approval and
 * the native confirmation and leaves nothing of the text behind (ADR-0017).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { LearningJournal } from './LearningJournal';

type Harness = ReturnType<typeof createInterceptHarness>;

async function store(harness: Harness, title: string, body: string): Promise<string> {
  const r = await harness.client.invoke('notes.create', { title, body_markdown: body, classification: 'INTERNAL', domain: 'LEARNING' });
  if (isG9Error(r)) {
    throw new Error(`could not store the note: ${r.code}`);
  }
  harness.transport.clock.advance(60_000);
  return r.result.note_id;
}

describe('Learning Journal: notes stored earlier', () => {
  it('lists them without saving anything and opens one with its body', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    await store(harness, 'Older note', 'written last week');
    await store(harness, 'Newer note', 'written yesterday');
    const seeded = harness.calls.length;
    renderWithIpc(<LearningJournal />, harness);

    const list = await screen.findByRole('list', { name: 'Stored notes' });
    const rows = within(list).getAllByRole('button');
    expect(rows.map((r) => r.textContent)).toEqual([expect.stringContaining('Newer note'), expect.stringContaining('Older note')]);

    await user.click(within(list).getByRole('button', { name: /^Older note/ }));
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLInputElement>('Title').value).toBe('Older note');
    });
    expect((harness.calls.find((c) => c.op === 'notes.get')?.payload as { note_id: string }).note_id).toMatch(/^mock-note-/);
    expect(screen.getByLabelText<HTMLTextAreaElement>('Markdown').value).toBe('written last week');
    expect(screen.getByRole('button', { name: /^Older note/ })).toHaveAttribute('aria-pressed', 'true');
    // Nothing was written by opening a note.
    expect(harness.calls.slice(seeded).filter((c) => c.op === 'notes.create' || c.op === 'notes.revise')).toHaveLength(0);
  });

  it('asks before an opened note replaces unsaved text, and opens it after the answer', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    await store(harness, 'Stored', 'stored body');
    renderWithIpc(<LearningJournal />, harness);
    const list = await screen.findByRole('list', { name: 'Stored notes' });
    await user.type(screen.getByLabelText('Title'), 'Half a thought');
    await user.click(within(list).getByRole('button', { name: /^Stored/ }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Discard the text that is not saved?' });
    expect(dialog).toHaveTextContent('Opening “Stored” replaces what is in the editor');
    expect(harness.calls.filter((c) => c.op === 'notes.get')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: 'Discard the changes' }));
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLInputElement>('Title').value).toBe('Stored');
    });
  });

  it('shows an empty store as "No notes yet" and a failed read as the error it is', async () => {
    const harness = createInterceptHarness((op, _payload, next) =>
      op === 'notes.list' ? Promise.resolve({ category: 'RESOURCE_UNAVAILABLE', code: 'storage.unavailable', message: 'the database is not available' }) : next(),
    );
    renderWithIpc(<LearningJournal />, harness);
    const alerts = await screen.findAllByRole('alert');
    expect(alerts.some((a) => a.getAttribute('data-category') === 'RESOURCE_UNAVAILABLE')).toBe(true);
    expect(screen.queryByText('No notes yet')).toBeNull();
  });
});

describe('Learning Journal: erase a note', () => {
  async function openStored(user: ReturnType<typeof userEvent.setup>, harness: Harness, title: string) {
    renderWithIpc(<LearningJournal />, harness);
    const list = await screen.findByRole('list', { name: 'Stored notes' });
    await user.click(within(list).getByRole('button', { name: new RegExp(`^${title}`) }));
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLInputElement>('Title').value).toBe(title);
    });
  }

  it('asks for a reason, goes through the approval and leaves nothing of the note in the window', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    const keep = await store(harness, 'Keep me', 'stays');
    const gone = await store(harness, 'Door code', 'pin-code-4711');
    await openStored(user, harness, 'Door code');

    await user.click(screen.getByRole('button', { name: 'Erase this note…' }));
    const form = await screen.findByRole('form', { name: 'Erase this note' });
    expect(form).toHaveTextContent('Erasing removes the text of “Door code” and of all 1 version');
    const request = within(form).getByRole('button', { name: 'Request the erasure' });
    // No reason, no request.
    expect(request).toBeDisabled();
    await user.type(within(form).getByLabelText('Reason (1 to 500 characters)'), 'the door code changed');
    expect(request).toBeEnabled();
    await user.click(request);

    const card = await within(form).findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(form).findByRole('button', { name: 'Apply with approval' }));

    expect(await screen.findByTestId('note-erased')).toHaveTextContent('Erased “Door code”.');
    const sent = harness.calls.filter((c) => c.op === 'deletion.request').map((c) => c.payload);
    expect(sent[0]).toEqual({ resource: { kind: 'NOTE', id: gone }, reason: 'the door code changed' });
    // The editor is clean, the list no longer has the note and nothing of its text is on the page.
    await waitFor(() => {
      expect(within(screen.getByRole('list', { name: 'Stored notes' })).queryByRole('button', { name: /^Door code/ })).toBeNull();
    });
    expect(within(screen.getByRole('list', { name: 'Stored notes' })).getByRole('button', { name: /^Keep me/ })).toBeInTheDocument();
    expect(screen.getByLabelText<HTMLInputElement>('Title').value).toBe('');
    expect(document.body.textContent).not.toContain('4711');
    const opened = await harness.client.invoke('notes.get', { note_id: gone });
    expect(isG9Error(opened) && opened.code).toBe('note.erased');
    const kept = await harness.client.invoke('notes.get', { note_id: keep });
    expect(isG9Error(kept)).toBe(false);
  });

  it('cancels without sending anything and refuses a reason that is too long', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    await store(harness, 'Door code', 'pin-code-4711');
    await openStored(user, harness, 'Door code');
    await user.click(screen.getByRole('button', { name: 'Erase this note…' }));
    const form = await screen.findByRole('form', { name: 'Erase this note' });
    const reason = within(form).getByLabelText('Reason (1 to 500 characters)');
    await user.click(reason);
    await user.paste('x'.repeat(501));
    expect(within(form).getByRole('alert')).toHaveTextContent('longer than 500 characters');
    expect(within(form).getByRole('button', { name: 'Request the erasure' })).toBeDisabled();
    await user.click(within(form).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('form', { name: 'Erase this note' })).toBeNull();
    expect(harness.calls.filter((c) => c.op === 'deletion.request')).toHaveLength(0);
    // The note is still there.
    expect(screen.getByLabelText<HTMLInputElement>('Title').value).toBe('Door code');
  });

  it('does not offer to erase a note that is not saved', async () => {
    renderWithIpc(<LearningJournal />);
    await screen.findByRole('region', { name: 'Editor: new note' });
    expect(screen.queryByRole('button', { name: 'Erase this note…' })).toBeNull();
  });
});
