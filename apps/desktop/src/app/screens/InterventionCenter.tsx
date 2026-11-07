import { INTERVENTION_STATES } from '../../ipc/types';
import { formatDateTime, formatMinutes } from '../../lib/format';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

export function InterventionCenter() {
  const today = useToday();
  const list = useCommand('interventions.list', {});
  const items = list.data?.interventions ?? [];

  return (
    <div>
      <h1>Intervention Center</h1>
      <p className="muted">
        Lifecycle: HYPOTHESIS → PROPOSED → REVIEWED → APPROVED → SCHEDULED → APPLIED → RETESTED → EVALUATED → RETAINED | REVISED | RETIRED (REJECTED at any review).
        Model output can only create HYPOTHESIS and PROPOSED; everything from APPROVED onward is an operator decision.
      </p>
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      {list.loading ? <Loading what="interventions" /> : null}
      <Warnings warnings={list.warnings} />
      <Panel title="Interventions">
        {list.data !== null && items.length === 0 ? (
          <>
            <NoData reason="no interventions exist (no hypotheses supported by evidence yet)" />
            {today.status !== null && !today.status.model.available ? <ModelUnavailable reason={today.status.model.reason} what="Diagnosis proposals" /> : null}
          </>
        ) : null}
        {items.length > 0 ? (
          <Table
            caption="Interventions"
            headers={['State', 'Statement', 'Steps', 'Budget', 'Acceptance', 'Provenance', 'Created']}
            rows={items.map((i) => ({
              key: i.intervention_id,
              cells: [
                <StateBadge state={i.state} />,
                i.statement,
                i.steps.join('; '),
                formatMinutes(Math.round(i.time_budget_ms / 60_000)),
                i.acceptance,
                <ProvenanceBadge label={i.provenance === 'OPERATOR' || i.provenance === 'SYSTEM_DETERMINISTIC' ? 'VERIFIED' : i.provenance === 'MODEL_OUTPUT' ? 'RECOMMENDATION' : 'PROPOSED'} />,
                formatDateTime(i.created_at, today.zone),
              ],
            }))}
          />
        ) : null}
      </Panel>
      <Panel title="States">
        <div className="row">
          {INTERVENTION_STATES.map((s) => (
            <StateBadge key={s} state={s} />
          ))}
        </div>
      </Panel>
    </div>
  );
}
