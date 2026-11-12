import type { ReactNode } from 'react';

/** The head of a screen: its title, one sentence on what the screen is for, and its page-level actions. */
export function PageHead({ title, lede, actions }: { title: string; lede: string; actions?: ReactNode }) {
  return (
    <div className="page-head">
      <div>
        <h1>{title}</h1>
        <p className="lede">{lede}</p>
      </div>
      {actions === undefined ? null : <div className="toolbar">{actions}</div>}
    </div>
  );
}
