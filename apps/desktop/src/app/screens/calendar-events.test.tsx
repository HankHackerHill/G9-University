/**
 * Calendar events the operator adds (`calendar.create_event`), changes (`calendar.update_event`)
 * and deletes (`calendar.delete_event`, approval-gated: DATA_DELETE, confirmed natively). Only
 * OPERATOR events offer Edit and Delete; times are entered in the backend's zone.
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import { createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc } from '../../test/render';
import { CalendarScreen } from './CalendarScreen';

const HOUR = 3_600_000;

describe('Calendar: events you add', () => {
  it('adds an event in the backend zone, then edits it with the revision the backend reported', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CalendarScreen />, harness);
    expect(await screen.findByText('no events you added in this week', { exact: false })).toBeInTheDocument();
    const form = screen.getByRole('form', { name: 'Add event' });
    await user.type(within(form).getByLabelText('Title'), 'Dentist');
    await user.selectOptions(within(form).getByLabelText('Kind'), 'APPOINTMENT');
    fireEvent.change(within(form).getByLabelText('Start'), { target: { value: '2026-11-19T10:00' } });
    fireEvent.change(within(form).getByLabelText('End'), { target: { value: '2026-11-19T11:00' } });
    const add = within(form).getByRole('button', { name: 'Add event' });
    await waitFor(() => {
      expect(add).toBeEnabled();
    });
    await user.click(add);
    // 10:00 and 11:00 in Europe/Berlin are 09:00Z and 10:00Z.
    expect(harness.calls.find((c) => c.op === 'calendar.create_event')?.payload).toEqual({
      title: 'Dentist',
      kind: 'APPOINTMENT',
      start: Date.UTC(2026, 10, 19, 9, 0),
      end: Date.UTC(2026, 10, 19, 10, 0),
    });
    expect(await screen.findByTestId('event-saved')).toHaveTextContent('Added “Dentist”, 2026-11-19 10:00 to 2026-11-19 11:00.');

    const table = await screen.findByRole('table', { name: 'Events you added' });
    await user.click(within(table).getByRole('button', { name: 'Edit Dentist' }));
    const edit = screen.getByRole('form', { name: 'Edit event' });
    expect(within(edit).getByLabelText('Title')).toHaveValue('Dentist');
    expect(within(edit).getByLabelText('Start')).toHaveValue('2026-11-19T10:00');
    await user.clear(within(edit).getByLabelText('Title'));
    await user.type(within(edit).getByLabelText('Title'), 'Dentist (moved)');
    fireEvent.change(within(edit).getByLabelText('End'), { target: { value: '2026-11-19T12:00' } });
    await user.click(within(edit).getByRole('button', { name: 'Save changes' }));
    const update = harness.calls.find((c) => c.op === 'calendar.update_event');
    expect(update?.payload).toMatchObject({ title: 'Dentist (moved)', kind: 'APPOINTMENT', end: Date.UTC(2026, 10, 19, 11, 0) });
    expect(update?.meta.expected_revision).toBe(1);
    expect(await screen.findByTestId('event-saved')).toHaveTextContent('Saved “Dentist (moved)”');
    await waitFor(() => {
      expect(within(screen.getByRole('table', { name: 'Events you added' })).getByRole('button', { name: 'Edit Dentist (moved)' })).toBeInTheDocument();
    });
    // Curriculum blocks are not offered for editing.
    expect(within(screen.getByRole('table', { name: 'Events you added' })).getAllByRole('row')).toHaveLength(2);
  });

  it('will not send an event that ends before it starts', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CalendarScreen />, harness);
    const form = await screen.findByRole('form', { name: 'Add event' });
    await user.type(within(form).getByLabelText('Title'), 'Backwards');
    fireEvent.change(within(form).getByLabelText('Start'), { target: { value: '2026-11-19T11:00' } });
    fireEvent.change(within(form).getByLabelText('End'), { target: { value: '2026-11-19T10:00' } });
    expect(await screen.findByTestId('event-form-problem')).toHaveTextContent('The event must end after it starts.');
    expect(within(form).getByRole('button', { name: 'Add event' })).toBeDisabled();
    expect(harness.calls.some((c) => c.op === 'calendar.create_event')).toBe(false);
  });

  it('deletes an event you added only after an approval; Apply removes it and spends the approval', async () => {
    const user = userEvent.setup();
    const harness = createHarness();
    const start = Date.UTC(2026, 10, 20, 14, 0);
    const created = await harness.client.invoke('calendar.create_event', { title: 'Temporary', kind: 'REVIEW', start, end: start + HOUR });
    expect(isG9Error(created)).toBe(false);
    renderWithIpc(<CalendarScreen />, harness);
    const table = await screen.findByRole('table', { name: 'Events you added' });
    await user.click(within(table).getByRole('button', { name: 'Delete Temporary' }));
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByText('calendar.delete_event')).toBeInTheDocument();
    expect(within(card).getByText('PENDING')).toBeInTheDocument();
    // Nothing is deleted while the approval is pending.
    expect(within(table).getByRole('button', { name: 'Delete Temporary' })).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await screen.findByTestId('event-deleted')).toHaveTextContent('Deleted “Temporary”');
    expect(await screen.findByText('no events you added in this week', { exact: false })).toBeInTheDocument();
    const approvals = await harness.client.invoke('approvals.list', {});
    if (isG9Error(approvals)) {
      throw new Error(approvals.code);
    }
    expect(approvals.result.approvals.map((a) => [a.operation_name, a.state])).toEqual([['calendar.delete_event', 'CONSUMED']]);
  });

  it('a cancelled native confirmation approves nothing: the event stays and the request stays pending', async () => {
    const user = userEvent.setup();
    const harness = createHarness({ confirmApproval: () => false });
    const start = Date.UTC(2026, 10, 20, 9, 0);
    await harness.client.invoke('calendar.create_event', { title: 'Keep me', kind: 'STUDY_BLOCK', start, end: start + HOUR });
    renderWithIpc(<CalendarScreen />, harness);
    const table = await screen.findByRole('table', { name: 'Events you added' });
    await user.click(within(table).getByRole('button', { name: 'Delete Keep me' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('flow-confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    expect(within(card).getByText('PENDING')).toBeInTheDocument();
    expect(screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'UNAUTHORIZED')).toHaveLength(0);
    expect(within(table).getByRole('button', { name: 'Delete Keep me' })).toBeInTheDocument();
  });
});
