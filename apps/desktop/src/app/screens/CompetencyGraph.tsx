import type { LearningDimension } from '../../ipc/operations';
import { formatDateTime } from '../../lib/format';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

const DIMENSIONS: readonly LearningDimension[] = ['RECALL', 'UNDERSTANDING', 'APPLICATION', 'TRANSFER'];

/** List/tree fallback for the competency graph. Charts are optional and would need this table as their accessible alternative. */
export function CompetencyGraph() {
  const today = useToday();
  const projection = useCommand('competency.projection', {});

  return (
    <div>
      <h1>Competency Graph</h1>
      <p className="muted">
        State follows the APPLICATION dimension; <code>VALIDATED</code> requires EXAM-mode unaided evidence (Addendum A.9). Estimates are derived from observations by
        a replayable fold; when there are none, the UI shows NO DATA rather than a number.
      </p>
      {projection.error !== null ? <ErrorDisplay error={projection.error} onRetry={projection.reload} /> : null}
      {projection.loading ? <Loading what="competency projection" /> : null}
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
                      Objectives: {p.objective_ids.join(', ')} · evidence refs: {p.evidence_refs.length === 0 ? 'none' : p.evidence_refs.join(', ')}
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
