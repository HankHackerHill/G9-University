/**
 * Recording reviews (`retention.record_review`) and adding review items (`retention.create_item`)
 * from the Retention Center. The next due date is the backend's (baseline ladder); the window
 * only shows it. Mock clock: 2026-11-18 09:30 Europe/Berlin.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { createInterceptHarness } from '../../test/intercept';
import { renderWithIpc } from '../../test/render';
import { RetentionCenter } from './RetentionCenter';

const group = (prompt: string) => screen.findByRole('group', { name: `Record review: ${prompt}` });

describe('Retention Center: recording reviews', () => {
  it('Recalled moves a due item to its next offset, shows the next review and takes it out of the queue', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<RetentionCenter />, harness);
    const prompt = 'Recall: Model authentication/session state';
    await user.click(within(await group(prompt)).getByRole('button', { name: 'Recalled' }));
    expect(harness.calls.find((c) => c.op === 'retention.record_review')?.payload).toEqual({ review_item_id: 'mock-review-D14-O1-1', outcome: 'SUCCESS' });
    expect(await screen.findByTestId('review-recorded')).toHaveTextContent(`Recalled “${prompt}”: next review 2026-11-19 09:30 (stage 1, 1 day later).`);
    await waitFor(() => {
      expect(screen.queryByRole('group', { name: `Record review: ${prompt}` })).toBeNull();
    });
    // The item beyond the budget moves into the queue and can be recorded now.
    expect(await group('Recall: Test session mitigations')).toBeInTheDocument();
    expect(screen.getByText('queue fits the budget', { exact: false })).toBeInTheDocument();
  });

  it('Forgot sends an overdue item back to the first stage; Skip records the attempt and keeps the due date', async () => {
    const user = userEvent.setup();
    renderWithIpc(<RetentionCenter />);
    const forgotten = 'Recall: Explain HTTP request semantics';
    await user.click(within(await group(forgotten)).getByRole('button', { name: 'Forgot' }));
    expect(await screen.findByTestId('review-recorded')).toHaveTextContent(`Forgot “${forgotten}”: back to the first stage; next review 2026-11-19 09:30.`);
    const skipped = 'Recall: Analyze cookies, origins and headers';
    await waitFor(() => {
      expect(within(screen.getByRole('group', { name: `Record review: ${skipped}` })).getByRole('button', { name: 'Skip' })).toBeEnabled();
    });
    await user.click(within(screen.getByRole('group', { name: `Record review: ${skipped}` })).getByRole('button', { name: 'Skip' }));
    expect(await screen.findByText(`Skipped “${skipped}”: recorded; the due date stays 2026-11-17 08:30.`)).toBeInTheDocument();
    // Still overdue: a skip moves nothing.
    expect(within(screen.getByRole('table', { name: 'Overdue reviews' })).getByText(skipped)).toBeInTheDocument();
  });

  it('adds a review item whose first review is due a day later; the answer goes as written', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<RetentionCenter />, harness);
    const form = await screen.findByRole('form', { name: 'Add review item' });
    const add = within(form).getByRole('button', { name: 'Add review item' });
    expect(add).toBeDisabled();
    await user.type(within(form).getByLabelText('Prompt'), 'What does HSTS prevent?');
    await user.type(within(form).getByLabelText('Answer (optional)'), 'Protocol downgrade to plain HTTP.');
    await user.click(add);
    expect(harness.calls.find((c) => c.op === 'retention.create_item')?.payload).toEqual({
      prompt: 'What does HSTS prevent?',
      answer: 'Protocol downgrade to plain HTTP.',
    });
    expect(await screen.findByTestId('review-item-added')).toHaveTextContent('Added “What does HSTS prevent?”: first review 2026-11-19 09:30');
    expect(within(form).getByLabelText('Prompt')).toHaveValue('');
  });
});
