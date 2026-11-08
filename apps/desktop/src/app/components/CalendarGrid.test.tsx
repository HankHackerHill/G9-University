import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import type { CalendarEvent } from '../../ipc/operations';
import { createHarness } from '../../test/render';
import { CalendarGrid } from './CalendarGrid';

async function weekEvents(): Promise<CalendarEvent[]> {
  const { client } = createHarness();
  const from = Date.UTC(2026, 10, 15, 23, 0); // Mon 2026-11-16 00:00 Europe/Berlin
  const r = await client.invoke('calendar.list', { from, to: from + 7 * 86_400_000 });
  if (isG9Error(r)) {
    throw new Error(r.code);
  }
  return r.result.events;
}

describe('CalendarGrid', () => {
  it('renders an ARIA grid with one tabbable cell (roving tabindex) and labelled cells', async () => {
    const events = await weekEvents();
    render(<CalendarGrid weekStart={{ year: 2026, month: 11, day: 16 }} zone="Europe/Berlin" events={events} />);
    const grid = screen.getByRole('grid');
    expect(grid).toHaveAttribute('aria-label', expect.stringContaining('Week of Mon 16 Nov 2026'));
    const cells = screen.getAllByRole('gridcell');
    expect(cells.filter((c) => c.tabIndex === 0)).toHaveLength(1);
    expect(screen.getAllByRole('columnheader')).toHaveLength(8);
    const wed9 = screen.getByTestId('cell-2026-11-18:9');
    expect(wed9).toHaveAttribute('aria-label', 'Wed 2026-11-18 09:00, 2 events: D15 recall 09:00 to 09:15; D15 theory 09:15 to 10:00');
    expect(wed9).toHaveTextContent('09:00 D15 recall');
    expect(screen.getByTestId('cell-2026-11-22:allday')).toHaveAttribute('aria-label', expect.stringContaining('Protected rest'));
  });

  it('moves focus with arrow keys, Home/End and PageUp/PageDown', async () => {
    const user = userEvent.setup();
    const events = await weekEvents();
    render(<CalendarGrid weekStart={{ year: 2026, month: 11, day: 16 }} zone="Europe/Berlin" events={events} startHour={7} endHour={10} />);
    const start = screen.getByTestId('cell-2026-11-16:7');
    expect(start.tabIndex).toBe(0);
    await user.tab();
    expect(document.activeElement).toBe(start);
    await user.keyboard('{ArrowRight}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-17:7'));
    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-17:8'));
    expect(screen.getByTestId('cell-2026-11-17:8').tabIndex).toBe(0);
    expect(start.tabIndex).toBe(-1);
    await user.keyboard('{End}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-22:8'));
    await user.keyboard('{Home}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-16:8'));
    await user.keyboard('{PageUp}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-16:allday'));
    await user.keyboard('{PageDown}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-16:9'));
    await user.keyboard('{ArrowUp}{ArrowUp}{ArrowUp}{ArrowUp}');
    expect(document.activeElement).toBe(screen.getByTestId('cell-2026-11-16:allday'));
    expect(screen.getAllByRole('gridcell').filter((c) => c.tabIndex === 0)).toHaveLength(1);
  });

  it('reports the activated cell on Enter', async () => {
    const user = userEvent.setup();
    const events = await weekEvents();
    const seen: string[] = [];
    render(
      <CalendarGrid
        weekStart={{ year: 2026, month: 11, day: 16 }}
        zone="Europe/Berlin"
        events={events}
        startHour={9}
        endHour={10}
        onActivate={(c) => {
          seen.push(c.events.map((e) => e.title).join('|'));
        }}
      />,
    );
    await user.tab();
    await user.keyboard('{ArrowRight}{ArrowRight}{Enter}');
    expect(seen).toEqual(['D15 recall|D15 theory']);
  });
});

describe('CalendarGrid never hides timed events', () => {
  const zone = 'Europe/Berlin';
  const at = (day: number, hour: number, minute = 0) => Date.UTC(2026, 10, day, hour - 1, minute); // CET = UTC+1 in November
  const event = (id: string, title: string, start: number, end: number): CalendarEvent => ({
    event_id: id,
    title,
    kind: 'APPOINTMENT',
    start,
    end,
    all_day: false,
    immutable: true,
    day_id: null,
    block: null,
    session_id: null,
    adjusted: null,
    provenance: 'OPERATOR',
  });
  const events = [
    event('e-early', 'Early dentist', at(17, 6), at(17, 7)),
    event('e-mid', 'Mid call', at(17, 10), at(17, 11)),
    event('e-late', 'Late call', at(17, 21), at(17, 22)),
    event('e-night', 'Night shift', at(17, 22, 30), at(18, 6)),
    event('e-carry', 'Carried over', at(15, 23), at(16, 2)),
  ];
  const labels = () =>
    screen
      .getAllByRole('gridcell')
      .map((c) => c.getAttribute('aria-label') ?? '')
      .join('\n');

  it('by default extends the hour rows so every timed event of the week is in a cell label', () => {
    render(<CalendarGrid weekStart={{ year: 2026, month: 11, day: 16 }} zone={zone} events={events} />);
    const all = labels();
    for (const title of ['Early dentist', 'Mid call', 'Late call', 'Night shift', 'Carried over']) {
      expect(all).toContain(title);
    }
    expect(screen.getByTestId('cell-2026-11-17:6')).toHaveAttribute('aria-label', 'Tue 2026-11-17 06:00, 1 event: Early dentist 06:00 to 07:00');
    expect(screen.getByTestId('cell-2026-11-17:21')).toHaveAttribute('aria-label', 'Tue 2026-11-17 21:00, 1 event: Late call 21:00 to 22:00');
    expect(screen.getByTestId('cell-2026-11-17:22')).toHaveAttribute('aria-label', expect.stringContaining('Night shift 22:30 to 2026-11-18 06:00'));
    // An event that began before the week is listed at Monday 00:00.
    expect(screen.getByTestId('cell-2026-11-16:0')).toHaveAttribute('aria-label', expect.stringContaining('Carried over 2026-11-15 23:00 to 02:00'));
    expect(screen.queryByTestId('calendar-outside-hours')).toBeNull();
  });

  it('a chip carried over from another day shows that day in its text and tooltip, like the cell label (NEW-9)', () => {
    render(<CalendarGrid weekStart={{ year: 2026, month: 11, day: 16 }} zone={zone} events={events} />);
    const carried = within(screen.getByTestId('cell-2026-11-16:0')).getByText(/Carried over/);
    // A bare "23:00" inside Monday's 00:00 row would read as Monday night; the chip carries Sunday's date.
    expect(carried).toHaveTextContent('2026-11-15 23:00 Carried over');
    expect(carried).toHaveAttribute('title', 'Carried over 2026-11-15 23:00 to 02:00');
    const night = within(screen.getByTestId('cell-2026-11-17:22')).getByText(/Night shift/);
    expect(night).toHaveTextContent('22:30 Night shift');
    expect(night).toHaveAttribute('title', 'Night shift 22:30 to 2026-11-18 06:00');
    const mid = within(screen.getByTestId('cell-2026-11-17:10')).getByText(/Mid call/);
    expect(mid).toHaveTextContent('10:00 Mid call');
    expect(mid).toHaveAttribute('title', 'Mid call 10:00 to 11:00');
  });

  it('with a caller-fixed range lists the events outside it in a note instead of dropping them', () => {
    render(<CalendarGrid weekStart={{ year: 2026, month: 11, day: 16 }} zone={zone} events={events} startHour={7} endHour={21} />);
    expect(screen.queryByTestId('cell-2026-11-17:6')).toBeNull();
    const note = screen.getByTestId('calendar-outside-hours');
    expect(note).toHaveTextContent('4 events outside the shown hours (07:00–21:00)');
    for (const title of ['Early dentist', 'Late call', 'Night shift', 'Carried over']) {
      expect(note).toHaveTextContent(title);
    }
    expect(note).not.toHaveTextContent('Mid call');
  });
});
