import type { ReactNode } from 'react';

/**
 * Advice in the margin. Output of a model or a skill is set in the mentor voice with a dashed
 * rule so it can never be mistaken for a posted record (ADR-0014: such output proposes, it
 * does not decide). Stamps, identifiers and times inside a note keep their own voice.
 */
export function Note({ children, label = 'Advice, not a record' }: { children: ReactNode; label?: string }) {
  return (
    <div className="note" role="note" aria-label={label}>
      {children}
    </div>
  );
}
