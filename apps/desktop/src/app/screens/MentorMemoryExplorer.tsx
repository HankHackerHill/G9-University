import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

/** Learner hypotheses (M09): model output can only create PROPOSED; acceptance/rejection is an operator command. */
export function MentorMemoryExplorer() {
  const today = useToday();
  const interventions = useCommand('interventions.list', { state: 'HYPOTHESIS' });
  const hypotheses = interventions.data?.interventions ?? [];

  return (
    <div>
      <h1>Mentor Memory Explorer</h1>
      <p className="muted">
        Hypotheses carry kind (MISCONCEPTION, STRENGTH, PREFERENCE, BLOCKER), supporting and contradicting evidence references, a confidence interval and provenance.
        Status: PROPOSED → ACCEPTED | REJECTED → RETIRED. Rejecting a hypothesis invalidates derived plans that referenced it.
      </p>
      {interventions.error !== null ? <ErrorDisplay error={interventions.error} onRetry={interventions.reload} /> : null}
      {interventions.loading ? <Loading what="hypotheses" /> : null}
      <Panel title="Hypotheses">
        {interventions.data !== null && hypotheses.length === 0 ? (
          <>
            <NoData reason="no learner hypotheses exist; a dedicated memory.hypotheses read model is PROPOSED" />
            {today.status !== null && !today.status.model.available ? <ModelUnavailable reason={today.status.model.reason} what="Hypothesis generation" /> : null}
          </>
        ) : null}
        {hypotheses.length > 0 ? (
          <ul>
            {hypotheses.map((h) => (
              <li key={h.intervention_id}>
                <ProvenanceBadge label="PROPOSED" /> {h.statement} <StateBadge state={h.state} />
              </li>
            ))}
          </ul>
        ) : null}
      </Panel>
      <Panel title="Provenance rules">
        <ul>
          <li>
            <ProvenanceBadge label="VERIFIED" /> operator statements and deterministic derivations.
          </li>
          <li>
            <ProvenanceBadge label="RECOMMENDATION" /> model or skill output; advisory only.
          </li>
          <li>
            <ProvenanceBadge label="PROPOSED" /> hypotheses awaiting an operator decision; never used as authority.
          </li>
        </ul>
      </Panel>
    </div>
  );
}
