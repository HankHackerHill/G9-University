import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { PackImportResult, ProgramStatus } from '../../ipc/operations';
import type { G9Error, IanaZone, JsonValue } from '../../ipc/types';
import { addDays, formatIsoDate, isoWeekday, parseIsoDate } from '../../lib/civil-time';
import { formatCivilDate, formatMinutes } from '../../lib/format';
import { mergeWarnings } from '../../lib/warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import type { UseCommandResult } from '../hooks/useCommand';
import { useIpc } from '../ipc-context';
import '../theme/screens/setup.css';
import { ProvenanceBadge } from './Badge';
import { ErrorDisplay } from './ErrorDisplay';
import { Field } from './Field';
import { FlowOutcome } from './FlowOutcome';
import { Loading, Panel } from './Panel';
import { Warnings } from './Warnings';

/**
 * The curriculum that ships with the application, read only when the operator asks to import it:
 * the draft built in the repository (`docs/curriculum/`, ADR-0022). It is `PROPOSED_SEED` until the
 * operator promotes it. The Appendix A seed stays in the repository as the original it is reconciled against.
 */
async function loadShippedCurriculum(): Promise<JsonValue> {
  const draft = await import('../../../../../data/curriculum/g9-ct9-480.draft.v1.json');
  return draft.default;
}

/** Today when it is a Monday, else the next Monday; empty when today is not known yet. */
function nextMonday(todayIso: string | null): string {
  const d = todayIso === null ? null : parseIsoDate(todayIso);
  if (d === null) {
    return '';
  }
  const weekday = isoWeekday(d);
  return formatIsoDate(addDays(d, weekday === 1 ? 0 : 8 - weekday));
}

/** Why the chosen date cannot be week 1, in words; null when it can. The backend refuses the same dates. */
function startProblem(date: string): string | null {
  if (date === '') {
    return 'Choose a date.';
  }
  const d = parseIsoDate(date);
  if (d === null) {
    return `${date} is not a date.`;
  }
  return isoWeekday(d) === 1 ? null : `${formatCivilDate(date)} is not a Monday. Week 1 runs Monday to Saturday, so it has to start on a Monday.`;
}

const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

/**
 * The program setup, once: import the curriculum that ships with the application (approval and
 * native confirmation: ADR-0014), choose the Monday week 1 starts and the time a day starts, and
 * load the competency packs. Every state shown is the backend's (`curriculum.program`); nothing
 * here starts a clock, plans a session or grades anything.
 */
export function ProgramSetup({
  program,
  zone,
  now,
  todayIso,
  onChanged,
  heading = 'Set up the program',
}: {
  program: UseCommandResult<ProgramStatus>;
  zone: IanaZone;
  now: number | null;
  todayIso: string | null;
  /** Runs after something changed, so the screen can read its own queries again. */
  onChanged: () => void;
  heading?: string;
}) {
  const client = useIpc();
  const importFlow = useApprovalFlow<'curriculum.import'>();
  const [loadProblem, setLoadProblem] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  // Null until the operator touches the field: the suggestion follows today until then.
  const [date, setDate] = useState<string | null>(null);
  const [time, setTime] = useState<string | null>(null);
  const [startBusy, setStartBusy] = useState(false);
  const [startError, setStartError] = useState<G9Error | null>(null);
  const [packBusy, setPackBusy] = useState(false);
  const [packError, setPackError] = useState<G9Error | null>(null);
  const [packResult, setPackResult] = useState<PackImportResult | null>(null);

  const data = program.data;
  const curriculum = data?.curriculum ?? null;
  const start = data?.program_start ?? null;
  const packs = data?.competencies ?? null;
  const importedNow = importFlow.result !== null && !isG9Error(importFlow.result) ? importFlow.result.result : null;

  const changed = () => {
    program.reload();
    onChanged();
  };

  const importCurriculum = async () => {
    setLoadProblem(null);
    let manifest: JsonValue;
    try {
      manifest = await loadShippedCurriculum();
    } catch {
      setLoadProblem('The curriculum file that ships with the application could not be loaded, so nothing was imported.');
      return;
    }
    await importFlow.run((approvalId) => client.invoke('curriculum.import', { manifest, provenance: 'PROPOSED_SEED' }, approvalId === undefined ? {} : { approvalId }));
  };

  const chosenDate = date ?? (start === null ? nextMonday(todayIso) : start.monday);
  const chosenTime = time ?? start?.day_start ?? '09:00';
  const dateProblem = startProblem(chosenDate);
  const timeProblem = TIME.test(chosenTime) ? null : 'Choose a time of day.';

  const setStart = async () => {
    setStartBusy(true);
    setStartError(null);
    const r = await client.invoke('curriculum.set_program_start', { date: chosenDate, day_start: chosenTime }, start === null ? {} : { expectedRevision: start.revision });
    if (isG9Error(r)) {
      setStartError(r);
    } else {
      setEditing(false);
      setDate(null);
      setTime(null);
      changed();
    }
    setStartBusy(false);
  };

  const loadPacks = async () => {
    setPackBusy(true);
    setPackError(null);
    const r = await client.invoke('competency.import_pack', {});
    if (isG9Error(r)) {
      setPackError(r);
      setPackResult(null);
    } else {
      setPackResult(r.result);
      changed();
    }
    setPackBusy(false);
  };

  const showStartForm = curriculum !== null && (start === null || editing);
  const packsLoaded = packs !== null && packs.loaded >= packs.available && packs.available > 0;
  const packsCreated = packResult === null ? 0 : packResult.packs.reduce((n, p) => n + p.created, 0);

  return (
    <Panel title={heading} note="Once, in three steps. Nothing here starts a clock, plans a session or grades anything.">
      {program.error !== null ? <ErrorDisplay error={program.error} onRetry={program.reload} /> : null}
      <Warnings warnings={mergeWarnings(importFlow.warnings)} label="Setup warnings" testId="setup-warnings" />
      {data === null ? (
        program.error === null ? <Loading what="the setup" /> : null
      ) : (
        <ol className="rows setup-steps" aria-label="Setup steps">
          <li data-testid="setup-curriculum" data-state={curriculum === null ? 'todo' : 'done'}>
            <div className="row-main">
              <span className="row-title">{curriculum === null ? '1. Import the curriculum' : '1. Curriculum imported'}</span>
              <span className="row-sub">
                {curriculum === null
                  ? 'The 12-week G9/CT9 program (480 hours) ships with this application as a proposed draft: objectives with success criteria, a hand-in and a lesson sheet for every day. Importing it needs your approval; the confirmation window names what is imported.'
                  : `${curriculum.name ?? 'Curriculum'}, version ${curriculum.version_label}: ${curriculum.weeks} weeks, ${curriculum.days} days, ${curriculum.objectives} objectives, ${formatMinutes(curriculum.planned_minutes)}.`}
              </span>
            </div>
            <div className="row-end">
              {curriculum === null ? (
                <button
                  type="button"
                  className="primary"
                  disabled={importFlow.pending || (importFlow.approval !== null && importFlow.approval.state !== 'CONSUMED')}
                  onClick={() => {
                    void importCurriculum();
                  }}
                >
                  Import the curriculum
                </button>
              ) : (
                <ProvenanceBadge label={curriculum.provenance === 'APPROVED_CANONICAL' ? 'VERIFIED' : 'PROPOSED'} />
              )}
            </div>
            {loadProblem === null ? null : (
              <p className="setup-body field-error" role="alert">
                {loadProblem}
              </p>
            )}
            {curriculum !== null && importedNow === null ? null : (
              <div className="setup-body">
                <FlowOutcome flow={importFlow} zone={zone} now={now} onApplied={changed} />
                {importedNow === null ? null : (
                  <p role="status" data-testid="setup-imported">
                    Imported {importedNow.weeks} weeks, {importedNow.days} days and {importedNow.objectives} objectives as {importedNow.provenance}.
                  </p>
                )}
              </div>
            )}
          </li>

          <li data-testid="setup-start" data-state={start !== null ? 'done' : curriculum === null ? 'waiting' : 'todo'}>
            <div className="row-main">
              <span className="row-title">{start === null ? '2. Choose when week 1 starts' : `2. Week 1 starts ${formatCivilDate(start.monday)}`}</span>
              <span className="row-sub">
                {start === null
                  ? curriculum === null
                    ? 'Import the curriculum first.'
                    : 'Week 1 begins on a Monday and runs Monday to Saturday; Sunday is rest. Choose that Monday and the time your day starts.'
                  : `A planned day starts at ${start.day_start} (${zone}). Sessions already planned keep their own times.`}
              </span>
            </div>
            <div className="row-end">
              {start !== null && !editing ? (
                <button
                  type="button"
                  onClick={() => {
                    setEditing(true);
                    setStartError(null);
                  }}
                >
                  Change
                </button>
              ) : null}
            </div>
            {showStartForm ? (
              <form
                className="setup-body"
                aria-label="Program start"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (!startBusy && dateProblem === null && timeProblem === null) {
                    void setStart();
                  }
                }}
              >
                <div className="form-grid">
                  <Field label="Week 1 starts (a Monday)" error={dateProblem}>
                    {({ id, describedBy, invalid }) => (
                      <input
                        id={id}
                        type="date"
                        required
                        value={chosenDate}
                        aria-describedby={describedBy}
                        aria-invalid={invalid}
                        onChange={(e) => {
                          setDate(e.target.value);
                        }}
                      />
                    )}
                  </Field>
                  <Field label="A day starts at" error={timeProblem} hint={`In ${zone}.`}>
                    {({ id, describedBy, invalid }) => (
                      <input
                        id={id}
                        type="time"
                        required
                        value={chosenTime}
                        aria-describedby={describedBy}
                        aria-invalid={invalid}
                        onChange={(e) => {
                          setTime(e.target.value);
                        }}
                      />
                    )}
                  </Field>
                </div>
                {startError === null ? null : <ErrorDisplay error={startError} />}
                <div className="toolbar">
                  <button type="submit" className="primary" disabled={startBusy || dateProblem !== null || timeProblem !== null}>
                    {start === null ? 'Set the start' : 'Save the new start'}
                  </button>
                  {editing ? (
                    <button
                      type="button"
                      disabled={startBusy}
                      onClick={() => {
                        setEditing(false);
                        setDate(null);
                        setTime(null);
                        setStartError(null);
                      }}
                    >
                      Cancel
                    </button>
                  ) : null}
                </div>
              </form>
            ) : null}
          </li>

          <li data-testid="setup-packs" data-state={packsLoaded ? 'done' : 'todo'}>
            <div className="row-main">
              <span className="row-title">{packsLoaded ? '3. Competency packs loaded' : '3. Load the competency packs'}</span>
              <span className="row-sub">
                {packs === null
                  ? ''
                  : packsLoaded
                    ? `All ${packs.available} competencies of the packs that ship with the application are loaded.`
                    : `The packs that ship with the application define ${packs.available} competencies with their prerequisites (status PROPOSED). They fill the competency graph; loading them grades and credits nothing.${packs.loaded > 0 ? ` ${packs.loaded} of ${packs.available} are loaded.` : ''}`}
              </span>
            </div>
            <div className="row-end">
              {packsLoaded ? (
                <span className="setup-done">Optional step, done</span>
              ) : (
                <button
                  type="button"
                  disabled={packBusy}
                  onClick={() => {
                    void loadPacks();
                  }}
                >
                  {packs !== null && packs.loaded > 0 ? 'Load the rest' : 'Load the competency packs'}
                </button>
              )}
            </div>
            {packError === null && packResult === null ? null : (
              <div className="setup-body">
                {packError === null ? null : <ErrorDisplay error={packError} />}
                {packResult === null ? null : (
                  <p role="status" data-testid="setup-packs-loaded">
                    Loaded {packsCreated} competencies and {packResult.prerequisites_added} prerequisite links from {packResult.packs.length} packs.
                    {packResult.prerequisites_pending > 0 ? ` ${packResult.prerequisites_pending} prerequisites wait for a pack that is not loaded.` : ''}
                  </p>
                )}
              </div>
            )}
          </li>
        </ol>
      )}
    </Panel>
  );
}
