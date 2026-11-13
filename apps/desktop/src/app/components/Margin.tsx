import type { ReactNode } from 'react';

/**
 * The margin column: reasons beside the record. Why a rule exists, what a term means, what to
 * do next. It is an `aside`, so it is a complementary landmark, named by `label`.
 */
export function Margin({ label = 'Notes', children }: { label?: string; children: ReactNode }) {
  return (
    <aside className="margin" aria-label={label}>
      {children}
    </aside>
  );
}

/** One note in the margin. */
export function MarginNote({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div className="margin-note">
      {title === undefined ? null : <h3>{title}</h3>}
      {children}
    </div>
  );
}
