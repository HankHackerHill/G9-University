import { mergeWarnings, warningCode } from '../../lib/warnings';

/**
 * Renders `CommandOutcome.warnings` verbatim. A warning is how the backend discloses that a
 * successful response is partial or PROPOSED (e.g. `export.execution_not_implemented`, no file
 * produced) or that a host fact degrades the install (key-file fallback, read-only storage).
 * Dropping one would present unimplemented behaviour as done, so every screen renders the
 * warnings of the outcomes it shows.
 *
 * Accessibility: the disclosures themselves live in a labelled `<section>` (a `region`
 * landmark a screen-reader user can navigate to and re-read). A live region only announces
 * changes to content that was already mounted, so a separate polite live region is always
 * mounted (empty when there is nothing to say) and receives a short summary when warnings
 * first appear or change.
 */
export function Warnings({ warnings, label = 'Backend warnings', testId = 'outcome-warnings' }: { warnings: readonly string[]; label?: string; testId?: string }) {
  const list = mergeWarnings(warnings);
  const announcement = list.length === 0 ? '' : `${list.length} backend warning${list.length === 1 ? '' : 's'}: ${list.map(warningCode).join(', ')}`;
  return (
    <>
      <div className="visually-hidden" role="status" aria-live="polite" aria-atomic="true" data-testid={`${testId}-live`}>
        {announcement}
      </div>
      {list.length > 0 ? (
        <section className="warnings" aria-label={label} data-testid={testId}>
          <strong>WARNING</strong> <span className="muted">{label}</span>
          <ul>
            {list.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </section>
      ) : null}
    </>
  );
}
