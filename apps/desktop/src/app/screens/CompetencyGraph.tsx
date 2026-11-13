import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import { AUTHORED_CODE_PATTERN, type CompetencyCreated, type LearningDimension } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

const DIMENSIONS: readonly LearningDimension[] = ['RECALL', 'UNDERSTANDING', 'APPLICATION', 'TRANSFER'];

/** List/tree fallback for the competency graph. Charts are optional and would need this table as their accessible alternative. */
export function CompetencyGraph() {
  const today = useToday();
  const client = useIpc();
  const projection = useCommand('competency.projection', {});
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
      setCode('');
      setTitle('');
      setDescription('');
      setValidity('180');
    }
    setBusy(false);
    projection.reload();
  };

  return (
    <div>
      <PageHead title="Competency Graph" lede="What your graded work shows you can do, dimension by dimension, and how sure each estimate is." />
      <p className="muted">
        State follows the APPLICATION dimension; <code>VALIDATED</code> requires EXAM-mode unaided evidence (Addendum A.9). Estimates are derived from observations by
        a replayable fold; when there are none, the UI shows NO DATA rather than a number.
      </p>
      {projection.error !== null ? <ErrorDisplay error={projection.error} onRetry={projection.reload} /> : null}
      {projection.loading ? <Loading what="competency projection" /> : null}
      <Warnings warnings={mergeWarnings(projection.warnings, createWarnings)} />
      <Panel title="New competency">
        <form
          aria-label="New competency"
          onSubmit={(e) => {
            e.preventDefault();
            if (canCreate) {
              void create();
            }
          }}
        >
          <div className="row">
            <label htmlFor="competency-code">Code</label>
            <input
              id="competency-code"
              type="text"
              required
              maxLength={32}
              value={code}
              placeholder="WCP"
              aria-invalid={code !== '' && !codeValid}
              aria-describedby="competency-code-hint"
              onChange={(e) => {
                setCode(e.target.value.toUpperCase());
              }}
            />
            <span id="competency-code-hint" className="field-hint">
              Upper-case letters, digits and hyphens; starts with a letter or digit; unique.
            </span>
          </div>
          <div className="row">
            <label htmlFor="competency-title">Title</label>
            <input
              id="competency-title"
              type="text"
              required
              maxLength={200}
              value={title}
              onChange={(e) => {
                setTitle(e.target.value);
              }}
            />
            <label htmlFor="competency-validity">Validity (days)</label>
            <input
              id="competency-validity"
              type="number"
              inputMode="numeric"
              min={1}
              max={3650}
              step={1}
              value={validity}
              aria-invalid={!validityValid}
              aria-describedby="competency-validity-hint"
              onChange={(e) => {
                setValidity(e.target.value);
              }}
            />
            <span id="competency-validity-hint" className="field-hint">
              1 to 3650; how long validated evidence stays current before a retest is due.
            </span>
          </div>
          <div className="field">
            <label htmlFor="competency-description">Description (optional)</label>
            <textarea
              id="competency-description"
              rows={2}
              maxLength={4000}
              value={description}
              onChange={(e) => {
                setDescription(e.target.value);
              }}
            />
          </div>
          <button type="submit" className="primary" disabled={!canCreate}>
            Create competency
          </button>
        </form>
        {error !== null ? <ErrorDisplay error={error} /> : null}
        {created !== null ? (
          <p role="status" data-testid="competency-created">
            Created <code>{created.code}</code> “{created.title}” (<code>{created.competency_id}</code>), validity {created.validity_days} days. It stays UNASSESSED until
            a graded attempt is finalized for it.
          </p>
        ) : null}
      </Panel>
      {projection.data !== null ? (
        projection.data.projections.length === 0 ? (
          <NoData reason="no competencies loaded" />
        ) : (
          <>
            <p className="muted">
              Derived {formatDateTime(projection.data.derived_at, today.zone)} <ProvenanceBadge label="VERIFIED" />
            </p>
            <ul className="tree" aria-label="Competencies">
              {projection.data.projections.map((p) => (
                <li key={p.competency_id}>
                  <Panel title={p.name}>
                    <p>
                      <code>{p.competency_id}</code> <StateBadge state={p.state} /> {p.provisional ? <StateBadge state="PROVISIONAL" /> : null}
                    </p>
                    <Table
                      caption="Per-dimension estimates"
                      headers={['Dimension', 'Estimate', 'Unaided observations', 'Last observed', 'Uncertainty']}
                      numeric={[2]}
                      rows={DIMENSIONS.map((d) => {
                        const e = p.per_dimension[d];
                        return {
                          key: d,
                          cells: [
                            d,
                            e.score_estimate ?? <span className="muted">NO DATA</span>,
                            e.n_unaided,
                            e.last_observed_at === null ? '—' : formatDateTime(e.last_observed_at, today.zone),
                            e.uncertainty ?? '—',
                          ],
                        };
                      })}
                    />
                    <p className="muted">
                      Objectives: {p.objective_ids.length === 0 ? 'none' : p.objective_ids.join(', ')} · evidence refs:{' '}
                      {p.evidence_refs.length === 0 ? 'none' : p.evidence_refs.join(', ')}
                      {p.contradictions.length > 0 ? ` · contradictions: ${p.contradictions.join('; ')}` : ''}
                    </p>
                  </Panel>
                </li>
              ))}
            </ul>
          </>
        )
      ) : null}
    </div>
  );
}
