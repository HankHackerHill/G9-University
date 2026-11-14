import { warningMeaning } from '../../lib/copy';
import { isQuietWarning, mergeWarnings, warningCode } from '../../lib/warnings';

/**
 * Renders `CommandOutcome.warnings`. A warning is how the backend discloses that a
 * successful response is partial or PROPOSED (e.g. `export.attachments_missing`: an export was
 * written but some attachment files were missing from the store) or that a host fact degrades
 * the install (key-file fallback, read-only storage).
 * Dropping one would present unimplemented behaviour as done, so every screen renders the
 * warnings of the outcomes it shows. The backend's own sentence is always shown verbatim; when
 * the window knows what a code means it says so in plain words first.
 *
 * Accessibility: the disclosures themselves live in a labelled `<section>` (a `region`
 * landmark a screen-reader user can navigate to and re-read). A live region only announces
 * changes to content that was already mounted, so a separate polite live region is always
 * mounted (empty when there is nothing to say) and receives a short summary when warnings
 * first appear or change.
 */
export function Warnings({ warnings, label = 'Backend warnings', testId = 'outcome-warnings' }: { warnings: readonly string[]; label?: string; testId?: string }) {
  const all = mergeWarnings(warnings);
  // A standing fact (`QUIET_WARNING_CODES`) is a note under the alarms: shown, verbatim, never announced.
  const list = all.filter((w) => !isQuietWarning(w));
  const quiet = all.filter(isQuietWarning);
  const announcement = list.length === 0 ? '' : `${list.length} backend warning${list.length === 1 ? '' : 's'}: ${list.map(warningCode).join(', ')}`;
  return (
    <>
      <div className="visually-hidden" role="status" aria-live="polite" aria-atomic="true" data-testid={`${testId}-live`}>
        {announcement}
      </div>
      {list.length > 0 ? (
        <section className="warnings" aria-label={label} data-testid={testId}>
          <strong>{list.length === 1 ? 'Warning' : `${list.length} warnings`}</strong>
          <ul>
            {list.map((w) => {
              const meaning = warningMeaning(warningCode(w));
              return (
                <li key={w}>
                  {meaning === null ? null : <div>{meaning}</div>}
                  <div className={meaning === null ? undefined : 'faint'}>{w}</div>
                </li>
              );
            })}
          </ul>
        </section>
      ) : null}
      {quiet.length > 0 ? (
        <ul className="warnings-quiet" aria-label={`${label}: standing notes`} data-testid={`${testId}-quiet`}>
          {quiet.map((w) => {
            const meaning = warningMeaning(warningCode(w));
            return (
              <li key={w}>
                {meaning === null ? null : <span>{meaning} </span>}
                <span className="faint">{w}</span>
              </li>
            );
          })}
        </ul>
      ) : null}
    </>
  );
}
