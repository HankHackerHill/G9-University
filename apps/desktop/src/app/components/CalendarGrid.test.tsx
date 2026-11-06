import { render, screen } from '@testing-library/react';
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
