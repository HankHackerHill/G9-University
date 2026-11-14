import { useState, type ReactNode } from 'react';
import { isG9Error } from '../../ipc/client';
import { AUTHORED_CODE_PATTERN, type CompetencyCreated, type CompetencyProjection, type LearningDimension, type PackImportResult } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { stateLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { COMPETENCY_STATE_MARKS, ChartFigure, Legend, Segmented, onlyPart, type Part } from '../components/charts';
import { Composer } from '../components/Composer';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Field } from '../components/Field';
import { Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import '../theme/screens/competency.css';

const DIMENSIONS: readonly LearningDimension[] = ['RECALL', 'UNDERSTANDING', 'APPLICATION', 'TRANSFER'];

/** What stands in a cell whose value does not exist: words, never a dash and never a zero. */
const UNOBSERVED = 'No observation yet';

function unobserved(text: string = UNOBSERVED): ReactNode {
  return <span className="unobserved muted small">{text}</span>;
}

/** A backend figure to two decimals at most; the exact value stays in the `title`. */
function figure(value: number): ReactNode {
  return <span title={String(value)}>{Number.isInteger(value) ? value : Math.round(value * 100) / 100}</span>;
}

/** How many of the four dimensions have an estimate (an estimate exists from the first observation on). */
function observedDimensions(p: CompetencyProjection): number {
  return DIMENSIONS.filter((d) => p.per_dimension[d].score_estimate !== null).length;
}

/** True when the backend holds nothing at all for any dimension: no estimate, no count, no instant, no uncertainty. */
function nothingObserved(p: CompetencyProjection): boolean {
  return DIMENSIONS.every((d) => {
    const e = p.per_dimension[d];
    return e.score_estimate === null && e.n_unaided === 0 && e.last_observed_at === null && e.uncertainty === null;
  });
}

/**
 * The competency graph as its accessible form: a list that picks a competency, and that
 * competency's four dimensions in one table. No node-link diagram is drawn; the distribution over
 * the states is the only chart, and it has a table view.
 */
export function CompetencyGraph() {
  const today = useToday();
  const client = useIpc();
  const projection = useCommand('competency.projection', {});
  // Which of the competency packs that ship with the application are loaded (`competency.import_pack`).
  const program = useCommand('curriculum.program', {});
  const [packBusy, setPackBusy] = useState(false);
  const [packError, setPackError] = useState<G9Error | null>(null);
  const [packResult, setPackResult] = useState<PackImportResult | null>(null);
  const [packWarnings, setPackWarnings] = useState<readonly string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // "Add a competency" (competency.create), opened from the page head.
  const [composing, setComposing] = useState(false);
  const [code, setCode] = useState('');
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [validity, setValidity] = useState('180');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<G9Error | null>(null);
  const [created, setCreated] = useState<CompetencyCreated | null>(null);
  const [createWarnings, setCreateWarnings] = useState<readonly string[]>([]);

  const codeValid = AUTHORED_CODE_PATTERN.test(code);
  const validityDays = /^\d{1,4}$/.test(validity.trim()) ? Number(validity.trim()) : Number.NaN;
  const validityValid = validityDays >= 1 && validityDays <= 3650;
  const canCreate = !busy && codeValid && title.trim() !== '' && validityValid;

  const projections = projection.data?.projections ?? [];
  const selected: CompetencyProjection | undefined = projections.find((p) => p.competency_id === selectedId) ?? projections[0];
  const stateParts: Part[] = COMPETENCY_STATE_MARKS.map(({ state, mark }) => ({
    key: state,
    name: stateLabel(state),
    n: projections.filter((p) => p.state === state).length,
    mark,
  }));
  const oneState = onlyPart(stateParts);
  const readFailed = projection.error === null ? null : `The competency projection could not be read (${projection.error.code})`;

  const closeComposer = () => {
    setComposing(false);
    setError(null);
  };

  const loadPacks = async () => {
    setPackBusy(true);
    const r = await client.invoke('competency.import_pack', {});
    setPackWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setPackError(r);
      setPackResult(null);
    } else {
      setPackError(null);
      setPackResult(r.result);
    }
    setPackBusy(false);
    projection.reload();
    program.reload();
  };
  const packs = program.data?.competencies ?? null;
  const packsMissing = packs !== null && packs.loaded < packs.available;

  const create = async () => {
    setBusy(true);
    const r = await client.invoke('competency.create', {
      code,
      title: title.trim(),
      validity_days: validityDays,
      ...(description.trim() === '' ? {} : { description: description.trim() }),
    });
    setCreateWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
      setCreated(r.result);
      setSelectedId(r.result.competency_id);
      setCode('');
      setTitle('');
      setDescription('');
      setValidity('180');
      setComposing(false);
    }
    setBusy(false);
    projection.reload();
  };

  return (
    <div className="stack">
      <PageHead
        title="Competency Graph"
        lede="What your graded work shows you can do, dimension by dimension, and how sure each estimate is."
        actions={
          <>
            {packsMissing && projections.length > 0 ? (
              <button
                type="button"
                disabled={packBusy}
                onClick={() => {
                  void loadPacks();
                }}
              >
                Load the competency packs
              </button>
            ) : null}
            <button
              type="button"
              className="primary"
              disabled={composing}
              onClick={() => {
                setComposing(true);
              }}
            >
              Add a competency
            </button>
          </>
        }
      />
      {projection.error !== null ? <ErrorDisplay error={projection.error} onRetry={projection.reload} /> : null}
      {program.error !== null ? <ErrorDisplay error={program.error} onRetry={program.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      {packError !== null ? <ErrorDisplay error={packError} /> : null}
      <Warnings warnings={mergeWarnings(projection.warnings, program.warnings, createWarnings, packWarnings)} />
      {packResult !== null ? (
        <p className="receipt" role="status" data-testid="packs-loaded">
          Loaded {packResult.packs.reduce((n, p) => n + p.created, 0)} competencies and {packResult.prerequisites_added} prerequisite links from {packResult.packs.length} packs;{' '}
          {packResult.loaded} of {packResult.available} are stored. The packs are proposed content: every competency stays not assessed until a final grade gives evidence for it.
        </p>
      ) : null}
      {created !== null && !composing ? (
        <p className="receipt" role="status" data-testid="competency-created">
          Competency added: <code>{created.code}</code> “{created.title}”, valid for {created.validity_days} days. It is selected below and stays not assessed until a final
          grade gives evidence for it.
        </p>
      ) : null}

      {composing ? (
        <Composer title="Add a competency" onClose={closeComposer}>
          <form
            aria-label="Add a competency"
            onSubmit={(e) => {
              e.preventDefault();
              if (canCreate) {
                void create();
              }
            }}
          >
            <div className="form-grid">
              <Field label="Code" hint="Upper-case letters, digits and hyphens, starting with a letter or a digit, at most 32 characters. It must be unique, for example WCP.">
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="text"
                    required
                    maxLength={32}
                    value={code}
                    aria-invalid={code !== '' && !codeValid}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setCode(e.target.value.toUpperCase());
                    }}
                  />
                )}
              </Field>
              <Field label="Title">
                {({ id }) => (
                  <input
                    id={id}
                    type="text"
                    required
                    maxLength={200}
                    value={title}
                    onChange={(e) => {
                      setTitle(e.target.value);
                    }}
                  />
                )}
              </Field>
              <Field label="Validity in days" hint="1 to 3650. How long validated evidence stays current before a retest is due.">
                {({ id, describedBy }) => (
                  <input
                    id={id}
                    type="number"
                    inputMode="numeric"
                    min={1}
                    max={3650}
                    step={1}
                    value={validity}
                    aria-invalid={!validityValid}
                    aria-describedby={describedBy}
                    onChange={(e) => {
                      setValidity(e.target.value);
                    }}
                  />
                )}
              </Field>
              <Field wide label="Description (optional)">
                {({ id }) => (
                  <textarea
                    id={id}
                    rows={2}
                    maxLength={4000}
                    value={description}
                    onChange={(e) => {
                      setDescription(e.target.value);
                    }}
                  />
                )}
              </Field>
            </div>
            <div className="form-actions">
              <button type="submit" className="primary" disabled={!canCreate}>
                Add competency
              </button>
              <button type="button" onClick={closeComposer}>
                Cancel
              </button>
              <span className="form-note">A new competency is not assessed until a final grade gives evidence for it.</span>
            </div>
          </form>
        </Composer>
      ) : null}

      <div className="split" data-lead="end">
        {/* The projection is computed by the backend from recorded observations: stamped Verified unless the read came with a warning. */}
        <Panel
          title="Competencies"
          actions={projection.data !== null && projection.warnings.length === 0 ? <ProvenanceBadge label="VERIFIED" /> : undefined}
          note={projection.data === null ? undefined : `Computed ${formatDateTime(projection.data.derived_at, today.zone)} from the recorded observations.`}
        >
          {projection.data === null ? (
            readFailed === null ? (
              <Loading what="competencies" />
            ) : (
              <NoData reason={`${readFailed}.`} />
            )
          ) : projections.length === 0 ? (
            <NoData
              title="No competencies yet"
              reason="Load the competency packs that ship with the application, or write your own with Add a competency. A competency stays not assessed until a final grade gives evidence for it."
              action={
                <div className="toolbar">
                  <button
                    type="button"
                    className="primary"
                    disabled={packBusy}
                    onClick={() => {
                      void loadPacks();
                    }}
                  >
                    Load the competency packs
                  </button>
                </div>
              }
            />
          ) : (
            <div className="stack">
              {oneState !== null ? (
                <p>
                  {oneState.n === 1 ? 'The one competency is' : `All ${oneState.n} competencies are`} <strong>{oneState.name.toLowerCase()}</strong>. A competency changes state
                  when a final grade gives evidence for it.
                </p>
              ) : (
                <ChartFigure
                  caption={`${projections.length} competencies, in the order a competency moves through the states.`}
                  table={
                    <Table caption="Competencies by state" headers={['State', 'Competencies']} numeric={[1]} rows={stateParts.map((p) => ({ key: p.key, cells: [p.name, p.n] }))} />
                  }
                >
                  <Segmented parts={stateParts} label="Competencies by state" />
                  <Legend items={stateParts.map((p) => ({ mark: p.mark, label: `${p.name} ${p.n}` }))} />
                </ChartFigure>
              )}
              <ul className="rows" aria-label="Competencies">
                {projections.map((p) => {
                  const isSelected = selected?.competency_id === p.competency_id;
                  const observed = observedDimensions(p);
                  return (
                    <li key={p.competency_id} data-selected={isSelected}>
                      <button
                        type="button"
                        className="row-pick"
                        aria-pressed={isSelected}
                        onClick={() => {
                          setSelectedId(p.competency_id);
                        }}
                      >
                        <span className="row-main">
                          <span className="row-title">{p.name}</span>
                          <span className="row-sub">{observed === 0 ? UNOBSERVED : `Observed in ${observed} of ${DIMENSIONS.length} dimensions`}</span>
                        </span>
                        <span className="row-end">
                          <StateBadge state={p.state} />
                          {p.provisional ? <StateBadge state="PROVISIONAL" /> : null}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            </div>
          )}
        </Panel>

        <div className="sticky stack">
          {selected === undefined ? (
            projection.data === null && readFailed === null ? null : (
              <Panel title="Competency">
                {readFailed !== null ? (
                  <NoData reason={`${readFailed}, so there is none to show.`} />
                ) : (
                  <NoData title="No competency selected" reason="The competency you choose in the list is shown here, dimension by dimension." />
                )}
              </Panel>
            )
          ) : (
            <Panel
              title={selected.name}
              actions={
                <span className="toolbar">
                  <StateBadge state={selected.state} />
                  {selected.provisional ? <StateBadge state="PROVISIONAL" /> : null}
                </span>
              }
              note={
                selected.provisional
                  ? 'Provisional: this state rests on self-graded evidence. A grade you gave yourself counts as evidence, but it can never validate a competency.'
                  : undefined
              }
            >
              <div className="stack-sm">
                <div className="dimension-table">
                  <Table
                    caption="Estimates by dimension"
                    captionVisible
                    headers={['Dimension', 'Estimate', 'Unaided observations', 'Last observed', 'Uncertainty']}
                    numeric={[1, 2, 4]}
                    widths={['7.5rem', undefined, '6.5rem', undefined, undefined]}
                    // Nothing observed in any dimension is said once, not in twelve identical cells.
                    emptyText={`${UNOBSERVED} in any of the four dimensions: recall, understanding, application and transfer.`}
                    rows={
                      nothingObserved(selected)
                        ? []
                        : DIMENSIONS.map((d) => {
                            const e = selected.per_dimension[d];
                            return {
                              key: d,
                              cells: [
                                <span title={d}>{sentenceCase(d)}</span>,
                                e.score_estimate === null ? unobserved() : figure(e.score_estimate),
                                e.n_unaided === 0 ? unobserved('None') : e.n_unaided,
                                e.last_observed_at === null ? unobserved() : formatDateTime(e.last_observed_at, today.zone),
                                e.uncertainty === null ? unobserved() : figure(e.uncertainty),
                              ],
                            };
                          })
                    }
                  />
                </div>
                <p className="muted small">
                  The state follows the application dimension, and validated needs unaided evidence from exam mode. An estimate is computed from observations; where there is
                  none, the table says so and shows no figure.
                </p>
                <dl className="pairs">
                  <dt>Objectives</dt>
                  <dd>
                    {selected.objective_ids.length === 0
                      ? 'None linked'
                      : selected.objective_ids.map((o, i) => (
                          <span key={o}>
                            {i === 0 ? '' : ', '}
                            <code>{o}</code>
                          </span>
                        ))}
                  </dd>
                  <dt>Evidence</dt>
                  <dd>{selected.evidence_refs.length === 0 ? 'None yet' : <span className="hash">{selected.evidence_refs.join(', ')}</span>}</dd>
                  <dt>Contradictions</dt>
                  <dd>{selected.contradictions.length === 0 ? 'None' : selected.contradictions.map((c) => <div key={c}>{c}</div>)}</dd>
                  <dt>Record</dt>
                  <dd>
                    <code>{selected.competency_id}</code>
                  </dd>
                </dl>
              </div>
            </Panel>
          )}
        </div>
      </div>
    </div>
  );
}
