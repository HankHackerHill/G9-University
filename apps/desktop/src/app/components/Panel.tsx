import type { ReactNode } from 'react';
import { sentenceCase } from '../../lib/format';

/**
 * A ruled region of a page (a record: something you read): a rule, a title, optional actions
 * on the title line, an optional line saying what the region holds. `aside` is a region in
 * the margin column (quieter title).
 */
export function Panel({ title, children, actions, note, as = 'section' }: { title: string; children: ReactNode; actions?: ReactNode; note?: ReactNode; as?: 'section' | 'aside' }) {
  const Tag = as;
  return (
    <Tag className="panel" aria-label={title}>
      <div className="panel-head">
        <h2>{title}</h2>
        {actions}
      </div>
      {note === undefined ? null : <p className="panel-note">{note}</p>}
      {children}
    </Tag>
  );
}

/** A raised region (an instrument: something you operate). */
export function Instrument({ title, children, actions, label }: { title?: string; children: ReactNode; actions?: ReactNode; label?: string }) {
  return (
    <section className="instrument" aria-label={label ?? title}>
      {title === undefined ? null : (
        <div className="panel-head">
          <h2>{title}</h2>
          {actions}
        </div>
      )}
      {children}
    </section>
  );
}

/**
 * Explicit empty state: what is missing and why, and what to do about it when there is
 * something to do. Never substitute a number or a placeholder for missing data.
 */
export function NoData({ reason, title = 'No data', action }: { reason: ReactNode; title?: string; action?: ReactNode }) {
  return (
    <div className="nodata" role="status">
      <strong>{title}</strong>
      <span>{reason}</span>
      {action}
    </div>
  );
}

/** Model-down state for every panel that would otherwise show AI output. It is a setting, not a fault. */
export function ModelUnavailable({ reason, what = 'Recommendation' }: { reason: string | undefined; what?: string }) {
  return (
    <div className="nodata" role="status">
      <strong>{what}: no model</strong>
      <span>
        No model is running (<span title={reason ?? 'UNKNOWN'}>{sentenceCase(reason ?? 'UNKNOWN').toLowerCase()}</span>), so there is no advice here. Planning, notes, timers and reviews do not
        need one.
      </span>
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
