import { useId, type ReactNode } from 'react';

/**
 * A labelled control: label above, the control, then a hint or an error. The render prop
 * receives the ids to put on the control, so label, hint and error are wired for assistive
 * technology whatever the control is.
 */
export function Field({
  label,
  hint,
  error,
  wide = false,
  children,
}: {
  label: string;
  hint?: ReactNode;
  error?: string | null;
  /** Span the whole form grid. */
  wide?: boolean;
  children: (ids: { id: string; describedBy: string | undefined; invalid: boolean }) => ReactNode;
}) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const invalid = error !== undefined && error !== null && error !== '';
  const describedBy = [hint === undefined ? null : hintId, invalid ? errorId : null].filter((x): x is string => x !== null).join(' ');
  return (
    <div className={wide ? 'field wide' : 'field'}>
      <label htmlFor={id}>{label}</label>
      {children({ id, describedBy: describedBy === '' ? undefined : describedBy, invalid })}
      {hint === undefined ? null : (
        <span className="field-hint" id={hintId}>
          {hint}
        </span>
      )}
      {invalid ? (
        <span className="field-error" id={errorId} role="alert">
          {error}
        </span>
      ) : null}
    </div>
  );
}
