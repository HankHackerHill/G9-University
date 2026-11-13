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
    const receipt = await screen.findByTestId('review-recorded');
    expect(receipt).toHaveTextContent(`Recalled: “${prompt}”. Next review 2026-11-19 09:30, in 1 day.`);
    // The backend's own ladder index stays one step away, in the title.
    expect(receipt).toHaveAttribute('title', 'Ladder stage 1 of the baseline schedule');
    await waitFor(() => {
      expect(screen.queryByRole('group', { name: `Record review: ${prompt}` })).toBeNull();
    });
    // The item beyond the budget moves into the queue and can be recorded now.
    expect(await group('Recall: Test session mitigations')).toBeInTheDocument();
    expect(screen.getByText('The queue fits today’s budget.')).toBeInTheDocument();
  });

  it('"Did not recall" sends an overdue item back to the first step; Skip records the attempt and keeps the due date', async () => {
    const user = userEvent.setup();
    renderWithIpc(<RetentionCenter />);
    const forgotten = 'Recall: Explain HTTP request semantics';
    await user.click(within(await group(forgotten)).getByRole('button', { name: 'Did not recall' }));
    expect(await screen.findByTestId('review-recorded')).toHaveTextContent(
      `Did not recall: “${forgotten}”. It is back on the first step. Next review 2026-11-19 09:30, in 1 day.`,
    );
    const skipped = 'Recall: Analyze cookies, origins and headers';
    await waitFor(() => {
      expect(within(screen.getByRole('group', { name: `Record review: ${skipped}` })).getByRole('button', { name: 'Skip' })).toBeEnabled();
    });
    await user.click(within(screen.getByRole('group', { name: `Record review: ${skipped}` })).getByRole('button', { name: 'Skip' }));
    expect(await screen.findByText(`Skipped: “${skipped}”. The attempt is recorded and the due date stays 2026-11-17 08:30.`)).toBeInTheDocument();
    // Still overdue, in red ink: a skip moves nothing.
    const overdue = screen.getByRole('region', { name: 'Overdue reviews' });
    expect(within(overdue).getByText(skipped)).toBeInTheDocument();
    for (const since of within(overdue).getAllByText('Overdue since 2026-11-17 08:30')) {
      expect(since).toHaveAttribute('data-standing', 'owed');
    }
  });

  it('adds a review item whose first review is due a day later; the answer goes as written', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<RetentionCenter />, harness);
    // The form opens from the page head's primary action.
    await user.click(await screen.findByRole('button', { name: 'Add a review item' }));
    const form = await screen.findByRole('form', { name: 'Add a review item' });
    const add = within(form).getByRole('button', { name: 'Add review item' });
    expect(add).toBeDisabled();
    await user.type(within(form).getByLabelText('Prompt'), 'What does HSTS prevent?');
    await user.type(within(form).getByLabelText('Answer (optional)'), 'Protocol downgrade to plain HTTP.');
    await user.click(add);
    expect(harness.calls.find((c) => c.op === 'retention.create_item')?.payload).toEqual({
      prompt: 'What does HSTS prevent?',
      answer: 'Protocol downgrade to plain HTTP.',
    });
    expect(await screen.findByTestId('review-item-added')).toHaveTextContent('Review item added: “What does HSTS prevent?”. Its first review is due 2026-11-19 09:30.');
    // The composer closes on success and opens empty the next time.
    expect(screen.queryByRole('form', { name: 'Add a review item' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Add a review item' }));
    expect(within(screen.getByRole('form', { name: 'Add a review item' })).getByLabelText('Prompt')).toHaveValue('');
  });
});
