import type { ReactNode } from 'react';

/**
 * A ruled region of a page: a rule, a title, optional actions on the title line. `aside` is a
 * region in the margin column (quieter rule and title).
 */
export function Panel({ title, children, actions, as = 'section' }: { title: string; children: ReactNode; actions?: ReactNode; as?: 'section' | 'aside' }) {
  const Tag = as;
  return (
    <Tag className="panel" aria-label={title}>
      <div className="panel-head">
        <h2>{title}</h2>
        {actions}
      </div>
      {children}
    </Tag>
  );
}

/** Explicit empty state. Never substitute a number or a placeholder for missing data. */
export function NoData({ reason }: { reason: string }) {
  return (
    <p className="nodata" role="status">
      <strong>NO DATA</strong> — {reason}
    </p>
  );
}

/** Model-down state for every panel that would otherwise show AI output. */
export function ModelUnavailable({ reason, what = 'Recommendation' }: { reason: string | undefined; what?: string }) {
  return (
    <div className="nodata" role="status">
      <strong>{what} (model unavailable)</strong>
      <div className="muted">
        Reason: <code>{reason ?? 'UNKNOWN'}</code>. Deterministic planning, notes, timers and reviews keep working (R-002, A09).
      </div>
    </div>
  );
}

export function Loading({ what }: { what: string }) {
  return (
    <p className="muted" role="status">
      Loading {what}…
    </p>
  );
}
