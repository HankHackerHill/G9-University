import type { ReactNode } from 'react';

export type Standing = 'posted' | 'doubt' | 'owed' | 'missing';

/**
 * One measured value with how sure it is. `missing` says in words why there is no value (and
 * is drawn pencilled); it never shows a zero in its place. `foot` holds the sample size or
 * where the figure comes from.
 */
export function Stat({
  label,
  value,
  unit,
  foot,
  standing = 'posted',
  title,
}: {
  label: string;
  value: ReactNode;
  unit?: string | undefined;
  foot?: ReactNode;
  standing?: Standing | undefined;
  title?: string | undefined;
}) {
  return (
    <div className="stat" data-standing={standing} title={title}>
      <div className="stat-label">{label}</div>
      <div className="stat-value">
        {value}
        {unit === undefined ? null : <small>{unit}</small>}
      </div>
      {foot === undefined ? null : <div className="stat-foot">{foot}</div>}
    </div>
  );
}
