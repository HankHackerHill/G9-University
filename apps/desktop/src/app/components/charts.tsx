import type { CSSProperties, ReactNode } from 'react';

/**
 * Charts, drawn in the fill grammar from the design tokens (no chart library; the content
 * security policy allows no remote script, and the application is offline).
 *
 * Rules every chart here follows (docs/ui/design-system.md §8):
 *  - the form is picked by the data's job: several ratios against their own whole are a meter
 *    list (`Bars`); one whole split into ordered parts is a segmented bar (`Segmented`); a
 *    grid of days is a cell map (`ProgramMap`);
 *  - a mark's standing is its fill: solid ink is posted, amber hatch is in doubt, an outline is
 *    only planned, red is owed; ordered parts take the one-hue ramp;
 *  - text is always ink, never the colour of a mark; a legend names every mark when there is
 *    more than one; values are printed, and a table view exists for everything drawn;
 *  - every figure is the backend's: the window sums and scales, it never estimates.
 */

export type Mark = 'posted' | 'doubt' | 'pencil' | 'owed' | 'accent' | 'live' | 'today' | 'rest' | 'quiet' | 'ramp-1' | 'ramp-2' | 'ramp-3' | 'ramp-4';

function share(part: number, whole: number): number {
  if (!Number.isFinite(part) || !Number.isFinite(whole) || whole <= 0) {
    return 0;
  }
  return Math.min(1, Math.max(0, part / whole));
}

/** The key of a chart: each mark with its name in words. */
export function Legend({ items }: { items: readonly { mark: Mark; label: string }[] }) {
  return (
    <ul className="legend" aria-label="Key">
      {items.map((item) => (
        <li key={item.label}>
          <span className="swatch" data-mark={item.mark} aria-hidden="true" />
          {item.label}
        </li>
      ))}
    </ul>
  );
}

/** A chart with its title, the sentence that says what it shows, and its table view. */
export function ChartFigure({ caption, children, table }: { caption: ReactNode; children: ReactNode; table?: ReactNode }) {
  return (
    <figure className="chart-figure">
      {children}
      <figcaption>{caption}</figcaption>
      {table === undefined ? null : (
        <details>
          <summary>Show as a table</summary>
          {table}
        </details>
      )}
    </figure>
  );
}

export interface BarRow {
  key: string;
  name: string;
  /** The whole this row is measured against (what was planned). */
  of: number;
  posted: number;
  doubted?: number;
  /** The figure printed at the end of the row. */
  value: string;
  tone?: 'owed' | 'accent' | 'advice';
}

/**
 * A meter list: one row per named thing, bars on one baseline and one scale. The track is as
 * long as the row's whole (planned), so rows compare; the solid part is posted, the hatched part
 * is in doubt.
 */
export function Bars({ rows, label }: { rows: readonly BarRow[]; label: string }) {
  const longest = rows.reduce((max, r) => Math.max(max, r.of), 0);
  return (
    <ul className="bars" aria-label={label}>
      {rows.map((r) => {
        const posted = share(r.posted, r.of);
        const doubted = Math.min(share(r.doubted ?? 0, r.of), 1 - posted);
        const style = { '--share': longest <= 0 ? 0 : r.of / longest, '--posted': posted, '--doubted': doubted } as CSSProperties;
        return (
          <li key={r.key}>
            <span className="bar-name">{r.name}</span>
            <span className="bar-track" style={style} data-tone={r.tone} aria-hidden="true" />
            <span className="bar-value">{r.value}</span>
          </li>
        );
      })}
    </ul>
  );
}

export interface Part {
  key: string;
  name: string;
  n: number;
  mark: Mark;
}

/**
 * A whole that sits in a single part is a sentence, not a chart. Returns that part, or `null`
 * when two or more parts hold something (draw the chart) or none does.
 */
export function onlyPart(parts: readonly Part[]): Part | null {
  const present = parts.filter((p) => p.n > 0);
  return present.length === 1 ? (present[0] ?? null) : null;
}

/**
 * One whole split into named parts (at most six). Tall by default, with each part's count
 * printed inside its block where it fits; `thin` beside figures that already give the amounts.
 * A part of zero draws nothing. The accessible name lists every part with its count.
 */
export function Segmented({ parts, label, thin = false }: { parts: readonly Part[]; label: string; thin?: boolean }) {
  const total = parts.reduce((sum, p) => sum + p.n, 0);
  const spoken = `${label}: ${parts.map((p) => `${p.name} ${p.n}`).join(', ')}`;
  if (total <= 0) {
    return null;
  }
  return (
    <div className="dist" role="img" aria-label={spoken} data-size={thin ? 'thin' : undefined}>
      {parts
        .filter((p) => p.n > 0)
        .map((p) => (
          <span key={p.key} data-mark={p.mark} style={{ '--n': p.n } as CSSProperties} title={`${p.name}: ${p.n} of ${total}`}>
            {/* Only a block wide enough for a short number carries one; the rest are in the legend and the table. */}
            {thin || p.n / total < 0.08 ? null : p.n}
          </span>
        ))}
    </div>
  );
}

export interface MapDay {
  /** `D01` … `D72`. */
  id: string;
  /** Planned, credited and uncertain time, in milliseconds; `null` when no session of that day is recorded. */
  planned_ms: number | null;
  credited_ms: number;
  uncertain_ms: number;
  today: boolean;
}

/**
 * The program map: every day of the curriculum as one cell, a week per row. A cell fills with
 * ink as its planned time is credited, hatches where time is in doubt, stays an outline while
 * only planned, and is dotted when nothing is recorded for that day.
 */
export function ProgramMap({ days, daysPerWeek, label, describe }: { days: readonly MapDay[]; daysPerWeek: number; label: string; describe: (d: MapDay) => string }) {
  const weeks: MapDay[][] = [];
  for (let i = 0; i < days.length; i += daysPerWeek) {
    weeks.push(days.slice(i, i + daysPerWeek));
  }
  return (
    <div className="pmap" role="img" aria-label={label} style={{ '--days': daysPerWeek } as CSSProperties}>
      {weeks.map((week, w) => (
        <div className="pmap-row" key={week[0]?.id ?? w}>
          <span className="pmap-week" aria-hidden="true">
            W{w + 1}
          </span>
          {week.map((d) => {
            const posted = d.planned_ms === null ? 0 : share(d.credited_ms, d.planned_ms);
            const doubted = d.planned_ms === null ? 0 : Math.min(share(d.uncertain_ms, d.planned_ms), 1 - posted);
            return (
              <span className="tip" key={d.id} data-tip={describe(d)}>
                <span
                  className="pmap-day"
                  data-today={d.today}
                  data-known={d.planned_ms !== null}
                  style={{ '--posted': posted, '--doubted': doubted } as CSSProperties}
                />
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
}

export interface StripInterval {
  key: string;
  /** Length in milliseconds. */
  ms: number;
  continuity: 'VERIFIED' | 'UNCERTAIN' | 'OPEN';
  title: string;
}

/** The intervals of one session, to scale: solid is verified, hatched is uncertain, green is the open one. */
export function IntervalStrip({ intervals, label }: { intervals: readonly StripInterval[]; label: string }) {
  const drawn = intervals.filter((i) => i.ms > 0 || i.continuity === 'OPEN');
  if (drawn.length === 0) {
    return null;
  }
  return (
    <div className="strip" role="img" aria-label={label}>
      {drawn.map((i) => (
        <span key={i.key} data-continuity={i.continuity} style={{ '--n': Math.max(i.ms, 1) } as CSSProperties} title={i.title} />
      ))}
    </div>
  );
}
