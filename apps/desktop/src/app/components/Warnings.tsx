import { mergeWarnings } from '../../lib/warnings';

/**
 * Renders `CommandOutcome.warnings` verbatim. A warning is how the backend discloses that a
 * successful response is partial or PROPOSED (e.g. `export.execution_not_implemented`, no file
 * produced) or that a host fact degrades the install (key-file fallback, read-only storage).
 * Dropping one would present unimplemented behaviour as done, so every screen renders the
 * warnings of the outcomes it shows. Renders nothing when there are none.
 */
export function Warnings({ warnings, label = 'Backend warnings', testId = 'outcome-warnings' }: { warnings: readonly string[]; label?: string; testId?: string }) {
  const list = mergeWarnings(warnings);
  if (list.length === 0) {
    return null;
  }
  return (
    <div className="warnings" role="status" aria-label={label} data-testid={testId}>
      <strong>WARNING</strong> <span className="muted">{label}</span>
      <ul>
        {list.map((w) => (
          <li key={w}>{w}</li>
        ))}
      </ul>
    </div>
  );
}
