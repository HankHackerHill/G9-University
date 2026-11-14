import { currentBlockKey, type SheetLookup, type WeekSheet } from '../../lib/lesson-sheets';
import type { DailyPlan, SessionView } from '../../ipc/operations';
import { formatMinutes } from '../../lib/format';
import '../theme/screens/classroom.css';
import { ProvenanceBadge } from './Badge';
import { MarkdownPreview } from './MarkdownPreview';
import { Loading, NoData, Panel } from './Panel';

/** What the Classroom knows about the lesson sheets of the day on screen. `lookup` is null while they are being read. */
export interface LessonState {
  lookup: SheetLookup | null;
  failed: boolean;
}

function weekText(week: WeekSheet): string {
  return [`**Purpose.** ${week.purpose}`, `**Lab stack.** ${week.lab_stack}`, `**Competencies.** ${week.competencies}`, `**Accelerator.** ${week.accelerator}`].join('\n\n');
}

/**
 * The text of each block of the day, folded under the block's name, the first unfinished block
 * open. It is reading, not a record: it changes nothing in the plan, a timer or a grade. It is
 * shown only for a day whose objectives are the ones the sheet was written for; otherwise the
 * panel says so in words and shows nothing of the sheet.
 */
export function LessonSheetPanel({
  day,
  state,
  sessions,
  label,
}: {
  day: DailyPlan;
  state: LessonState;
  /** Today's sessions; null when they could not be read (then no block is opened for the operator). */
  sessions: readonly SessionView[] | null;
  label: 'VERIFIED' | 'PROPOSED';
}) {
  const { lookup, failed } = state;
  const title = 'Lesson sheet';
  if (failed) {
    return (
      <Panel title={title}>
        <NoData title="No lesson sheet" reason="The lesson sheets that ship with the application could not be loaded, so none is shown. They are part of the application’s files." />
      </Panel>
    );
  }
  if (lookup === null) {
    return (
      <Panel title={title}>
        <Loading what="the lesson sheet" />
      </Panel>
    );
  }
  if (lookup.kind === 'missing') {
    return (
      <Panel title={title}>
        <NoData title="No lesson sheet for this day" reason={`The lesson sheets that ship with the application belong to the G9/CT9 curriculum and have no sheet for ${day.day_id ?? 'this day'}.`} />
      </Panel>
    );
  }
  if (lookup.kind === 'other_version') {
    return (
      <Panel title={title}>
        <NoData
          title="Lesson sheet not shown"
          reason={`The lesson sheets that ship with the application were written for curriculum version ${lookup.builtFor}. Today’s objectives are not the ones they were written for, so they are not shown.`}
        />
      </Panel>
    );
  }
  const { sheet, week } = lookup;
  const open = currentBlockKey(day.blocks, sessions);
  return (
    <Panel
      title={title}
      actions={<ProvenanceBadge label={label} />}
      note="What each block asks of you, written with the curriculum. It is reading, not a record: it changes nothing in the plan, a timer or a grade."
    >
      <div data-testid="lesson-sheet">
        {sheet.sections.map((s) => (
          <details key={s.key} className="day-group lesson" open={s.key === open} data-block={s.key}>
            <summary>
              {s.name}, {formatMinutes(s.minutes)}
            </summary>
            <MarkdownPreview source={s.markdown} label={`${s.name}: lesson text`} />
          </details>
        ))}
        {week === null ? null : (
          <details className="day-group lesson" data-block="week">
            <summary>
              About week {sheet.week}, {week.title}
            </summary>
            <MarkdownPreview source={weekText(week)} label={`About week ${sheet.week}`} />
          </details>
        )}
      </div>
    </Panel>
  );
}

/** What to watch for today and where to read more, for the margin; nothing unless the sheet is shown. */
export function LessonMarginPanels({ state }: { state: LessonState }) {
  const lookup = state.lookup;
  if (lookup?.kind !== 'found') {
    return null;
  }
  const { sheet } = lookup;
  return (
    <>
      {sheet.watch_for === null ? null : (
        <Panel title="Watch for" as="aside">
          <div className="lesson">
            <MarkdownPreview source={sheet.watch_for} label="What to watch for" />
          </div>
        </Panel>
      )}
      <Panel
        title="Sources for the day"
        as="aside"
        note="The window opens no link: copy an address into a browser. A note that says ASSUMED means the terms and the version were not checked."
      >
        <div className="lesson">
          <MarkdownPreview source={sheet.resources} label="Sources for the day" />
        </div>
      </Panel>
    </>
  );
}
