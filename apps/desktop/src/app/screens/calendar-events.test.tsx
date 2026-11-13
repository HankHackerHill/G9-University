/**
 * Calendar events the operator adds (`calendar.create_event`), changes (`calendar.update_event`)
 * and deletes (`calendar.delete_event`, approval-gated: DATA_DELETE, confirmed natively). Only
 * OPERATOR events offer Change and Delete; times are entered in the backend's zone. Adding and
 * changing go through the composer the page head opens; the events of a day are listed under
 * the grid once a cell of that day is activated.
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
    // No event of this week was added by the operator, so nothing offers Change or Delete.
    expect(await screen.findByTestId('week-summary')).toHaveTextContent('none of them added by you');
    expect(screen.queryByRole('button', { name: /^(Change|Delete) / })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Add an event' }));
    const form = screen.getByRole('form', { name: 'Add an event' });
    // The composer puts the caret in its first field.
    expect(document.activeElement).toBe(within(form).getByLabelText('Title'));
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
    expect(await screen.findByTestId('event-saved')).toHaveTextContent('Event added: “Dentist”, 2026-11-19 10:00 to 2026-11-19 11:00.');
    // The composer closes; the day of the new event is listed under the grid, with the event marked.
    expect(screen.queryByRole('form', { name: 'Add an event' })).toBeNull();
    const day = await screen.findByRole('region', { name: 'Thursday 2026-11-19' });
    const change = await within(day).findByRole('button', { name: 'Change Dentist' });
    expect(change.closest('li')).toHaveAttribute('data-selected', 'true');
    await waitFor(() => {
      expect(change).toBeEnabled();
    });

    await user.click(change);
    const edit = screen.getByRole('form', { name: 'Change the event' });
    expect(within(edit).getByLabelText('Title')).toHaveValue('Dentist');
    expect(within(edit).getByLabelText('Start')).toHaveValue('2026-11-19T10:00');
    await user.clear(within(edit).getByLabelText('Title'));
    await user.type(within(edit).getByLabelText('Title'), 'Dentist (moved)');
    fireEvent.change(within(edit).getByLabelText('End'), { target: { value: '2026-11-19T12:00' } });
    await user.click(within(edit).getByRole('button', { name: 'Save changes' }));
    const update = harness.calls.find((c) => c.op === 'calendar.update_event');
    expect(update?.payload).toMatchObject({ title: 'Dentist (moved)', kind: 'APPOINTMENT', end: Date.UTC(2026, 10, 19, 11, 0) });
    expect(update?.meta.expected_revision).toBe(1);
    expect(await screen.findByTestId('event-saved')).toHaveTextContent('Changes saved: “Dentist (moved)”');
    await waitFor(() => {
      expect(within(screen.getByRole('region', { name: 'Thursday 2026-11-19' })).getByRole('button', { name: 'Change Dentist (moved)' })).toBeInTheDocument();
    });
    // Curriculum blocks are listed on that day too, but they are not offered for editing: only the one event you added has buttons.
    const list = screen.getByRole('list', { name: 'Events on Thursday 2026-11-19' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(7);
    expect(within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(['Change Dentist (moved)', 'Delete Dentist (moved)']);
    expect(screen.getByTestId('week-summary')).toHaveTextContent('36 events this week, 1 added by you');
  });

  it('will not send an event that ends before it starts', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CalendarScreen />, harness);
    await user.click(await screen.findByRole('button', { name: 'Add an event' }));
    const form = await screen.findByRole('form', { name: 'Add an event' });
    await user.type(within(form).getByLabelText('Title'), 'Backwards');
    fireEvent.change(within(form).getByLabelText('Start'), { target: { value: '2026-11-19T11:00' } });
    fireEvent.change(within(form).getByLabelText('End'), { target: { value: '2026-11-19T10:00' } });
    // The problem is said under the field it concerns, and the field is marked.
    expect(await within(form).findByRole('alert')).toHaveTextContent('The event must end after it starts.');
    expect(within(form).getByLabelText('End')).toHaveAttribute('aria-invalid', 'true');
    expect(within(form).getByLabelText('End')).toHaveAccessibleDescription(expect.stringContaining('The event must end after it starts.'));
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
    // 14:00Z is Friday 15:00 in Europe/Berlin: activating that cell lists Friday under the grid.
    await user.click(await screen.findByTestId('cell-2026-11-20:15'));
    const day = await screen.findByRole('region', { name: 'Friday 2026-11-20' });
    expect(screen.getByTestId('week-summary')).toHaveTextContent('1 added by you');
    await user.click(within(day).getByRole('button', { name: 'Delete Temporary' }));
    const card = await screen.findByTestId('approval-card');
    // The voucher sits directly under the list of that day.
    expect(day).toContainElement(card);
    expect(within(card).getByText('calendar.delete_event')).toBeInTheDocument();
    expect(within(card).getByTitle('PENDING')).toHaveTextContent('Waiting for you');
    // Nothing is deleted while the approval is pending.
    expect(within(day).getByRole('button', { name: 'Delete Temporary' })).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await screen.findByTestId('event-deleted')).toHaveTextContent('Event deleted: “Temporary”');
    await waitFor(() => {
      expect(screen.getByTestId('week-summary')).toHaveTextContent('none of them added by you');
    });
    expect(screen.queryByRole('button', { name: 'Delete Temporary' })).toBeNull();
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
    // 09:00Z is Friday 10:00 in Europe/Berlin.
    await user.click(await screen.findByTestId('cell-2026-11-20:10'));
    const day = await screen.findByRole('region', { name: 'Friday 2026-11-20' });
    await user.click(within(day).getByRole('button', { name: 'Delete Keep me' }));
    const card = await screen.findByTestId('approval-card');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    expect(await screen.findByTestId('flow-confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    expect(within(card).getByTitle('PENDING')).toHaveTextContent('Waiting for you');
    expect(screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'UNAUTHORIZED')).toHaveLength(0);
    expect(within(day).getByRole('button', { name: 'Delete Keep me' })).toBeInTheDocument();
  });
});

describe('Calendar: the week, the day under the grid, and schedule proposals', () => {
  it('lists today under the grid, then the day of the cell you activate, with the events of that cell marked', async () => {
    const user = userEvent.setup();
    renderWithIpc(<CalendarScreen />);
    // Today is Wednesday 2026-11-18 by the backend clock: its column is marked and its events are listed first.
    const todayPanel = await screen.findByRole('region', { name: 'Wednesday 2026-11-18' });
    expect(within(todayPanel).getByTestId('day-status')).toHaveTextContent('6 events on this day.');
    expect(within(todayPanel).getByLabelText('Provenance: verified')).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: /Wed 18/ })).toHaveAttribute('data-today', 'true');
    expect(screen.getAllByRole('columnheader').filter((h) => h.hasAttribute('data-today'))).toHaveLength(1);
    expect(screen.getByTestId('week-range')).toHaveTextContent('2026-11-16 to 2026-11-22, times in Europe/Berlin');
    expect(screen.getByRole('button', { name: 'This week' })).toHaveAttribute('aria-pressed', 'true');

    await user.click(screen.getByTestId('cell-2026-11-17:9'));
    const tuesday = await screen.findByRole('region', { name: 'Tuesday 2026-11-17' });
    expect(within(tuesday).getByTestId('day-status')).toHaveTextContent('6 events on this day. 09:00: 2 marked below.');
    const rows = within(tuesday).getAllByRole('listitem');
    expect(rows).toHaveLength(6);
    expect(rows[0]).toHaveTextContent('D14 recall09:00 to 09:15, study block');
    expect(rows.filter((r) => r.getAttribute('data-selected') === 'true').map((r) => r.querySelector('.row-title')?.textContent)).toEqual(['D14 recall', 'D14 theory']);
    // Curriculum blocks cannot be changed from here.
    expect(within(tuesday).queryAllByRole('button')).toHaveLength(0);
  });

  it('says so when a week has no events and offers to add one', async () => {
    const user = userEvent.setup();
    renderWithIpc(<CalendarScreen />);
    await screen.findByRole('grid');
    await user.click(screen.getByRole('button', { name: 'Next week' }));
    const title = await screen.findByText('No events this week');
    expect(screen.getByText(/Nothing is scheduled from 2026-11-23 to 2026-11-29\./)).toBeInTheDocument();
    expect(screen.queryByRole('grid')).toBeNull();
    expect(screen.getByRole('button', { name: 'This week' })).toHaveAttribute('aria-pressed', 'false');
    // The empty state's own action opens the composer.
    const empty = title.closest<HTMLElement>('[role="status"]');
    if (empty === null) {
      throw new Error('empty state not found');
    }
    await user.click(within(empty).getByRole('button', { name: 'Add an event' }));
    expect(screen.getByRole('form', { name: 'Add an event' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'This week' }));
    expect(await screen.findByRole('grid')).toBeInTheDocument();
  });

  it('shows a schedule proposal with its changes, fingerprint and expiry; applying it needs an approval and reloads the week', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness((_op, _payload, next) => next());
    renderWithIpc(<CalendarScreen />, harness);
    await screen.findByRole('grid');
    await user.click(screen.getByRole('button', { name: 'Propose review slots' }));
    const region = await screen.findByRole('region', { name: 'Schedule proposal' });
    // Asked for the week in view, in the backend zone: Monday 00:00 Europe/Berlin is Sunday 23:00Z.
    expect(harness.calls.find((c) => c.op === 'calendar.propose')?.payload).toEqual({
      from: Date.UTC(2026, 10, 15, 23, 0),
      to: Date.UTC(2026, 10, 22, 23, 0),
      intent: 'ADD_REVIEWS',
    });
    expect(within(region).getByLabelText('Provenance: proposed')).toBeInTheDocument();
    expect(within(region).getByTestId('proposal-feasibility')).toHaveTextContent('Feasible. No constraint is violated.');
    expect(within(region).getByTestId('proposal-hash')).toHaveTextContent(/^[0-9a-f]{64}$/);
    const changes = within(region).getByRole('list', { name: 'Proposed changes' });
    expect(within(changes).getAllByRole('listitem')).toHaveLength(1);
    expect(changes).toHaveTextContent(/Review queue \(\d+ items\)Add: 2026-11-19 08:30 to 2026-11-19 \d\d:\d\d, review/);
    // Proposals expire with the approval window: ten minutes after 09:30.
    expect(region).toHaveTextContent('Expires2026-11-18 09:40');

    await user.click(within(region).getByRole('button', { name: 'Apply proposal' }));
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByText('calendar.apply')).toBeInTheDocument();
    expect(within(card).getByTitle('PENDING')).toHaveTextContent('Waiting for you');
    // Asking is not applying: the week is unchanged while the approval waits.
    expect(screen.getByTestId('cell-2026-11-19:8')).toHaveAttribute('aria-label', 'Thu 2026-11-19 08:00, no events');
    await user.click(within(card).getByRole('button', { name: 'Approve' }));
    await user.click(await within(card).findByRole('button', { name: 'Apply with approval' }));
    expect(await screen.findByTestId('proposal-applied')).toHaveTextContent('Proposal applied: 1 change.');
    await waitFor(() => {
      expect(screen.getByTestId('cell-2026-11-19:8')).toHaveAttribute('aria-label', expect.stringContaining('Review queue'));
    });
    expect(within(region).getByRole('button', { name: 'Apply proposal' })).toBeDisabled();
  });

  it('an infeasible proposal names the violated constraints and cannot be applied', async () => {
    const user = userEvent.setup();
    const harness = createInterceptHarness(async (op, _payload, next) => {
      const r = await next();
      if (op === 'calendar.propose' && typeof r === 'object' && r !== null && 'result' in r) {
        const outcome = r as { result: Record<string, unknown> };
        return {
          ...outcome,
          result: { ...outcome.result, feasible: false, violated_constraints: [{ code: 'OVERLAP', message: 'overlaps D16 recall', event_id: 'mock-event-D16-1' }] },
        };
      }
      return r;
    });
    renderWithIpc(<CalendarScreen />, harness);
    await screen.findByRole('grid');
    await user.click(screen.getByRole('button', { name: 'Propose review slots' }));
    const region = await screen.findByRole('region', { name: 'Schedule proposal' });
    expect(within(region).getByTestId('proposal-feasibility')).toHaveTextContent('Not feasible. 1 constraint is violated, so this proposal cannot be applied.');
    const violated = within(region).getByRole('list', { name: 'Violated constraints' });
    // Red ink, and the words say it too.
    expect(within(violated).getByText('overlaps D16 recall')).toHaveAttribute('data-standing', 'owed');
    expect(within(violated).getByText('OVERLAP')).toBeInTheDocument();
    expect(within(region).getByRole('button', { name: 'Apply proposal' })).toBeDisabled();
    expect(harness.calls.some((c) => c.op === 'calendar.apply')).toBe(false);
    // Close puts the proposal away.
    await user.click(within(region).getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('region', { name: 'Schedule proposal' })).toBeNull();
  });
});
