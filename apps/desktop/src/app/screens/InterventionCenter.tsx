import { Link } from 'react-router-dom';
import { INTERVENTION_STATES, type InterventionState, type ProvenanceLabel, type TrustClass } from '../../ipc/types';
import { stateLabel } from '../../lib/copy';
import { formatDateTime, formatMinutes } from '../../lib/format';
import { Badge, ProvenanceBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin } from '../components/Margin';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import '../theme/screens/interventions.css';

/**
 * The states an intervention leaves the sequence through (`crates/g9-domain/src/state.rs`):
 * Rejected, Retained and Retired are terminal; Revised goes back to Proposed. Every other state
 * of `INTERVENTION_STATES` is a step of the sequence, in the order the machine walks them.
 */
const OFF_SEQUENCE: readonly InterventionState[] = ['REJECTED', 'RETAINED', 'REVISED', 'RETIRED'];
const SEQUENCE: readonly InterventionState[] = INTERVENTION_STATES.filter((s) => !OFF_SEQUENCE.includes(s));

/**
 * `stateLabel` holds one word per state name, and `APPLIED` is the word of a finished export or
 * erasure there ("Done"). An applied intervention is not done: it is still retested and
 * evaluated. Until copy.ts can tell the two apart, this screen says "Applied" itself.
 */
function interventionStateLabel(state: InterventionState): string {
  return state === 'APPLIED' ? 'Applied' : stateLabel(state);
}

/** The stamp `StateBadge` draws, with the intervention's own word for the state. */
function InterventionStamp({ state }: { state: InterventionState }) {
  const label = interventionStateLabel(state);
  return (
    <Badge tone="state" label={`State: ${label}`} state={state} title={state}>
      {label}
    </Badge>
  );
}

/** Who wrote the record decides its stamp. Only the operator and deterministic code are verified; a model can only advise. */
function provenanceOf(trust: TrustClass): ProvenanceLabel {
  return trust === 'OPERATOR' || trust === 'SYSTEM_DETERMINISTIC' ? 'VERIFIED' : trust === 'MODEL_OUTPUT' ? 'RECOMMENDATION' : 'PROPOSED';
}

export function InterventionCenter() {
  const today = useToday();
  const list = useCommand('interventions.list', {});
  const items = list.data?.interventions ?? [];
  const s = today.status;
  const fromModel = items.filter((i) => i.provenance === 'MODEL_OUTPUT').length;
  // Without the backend's zone a time can only be given in UTC, and then it says so.
  const when = (utc: number) => `${formatDateTime(utc, today.zone)}${s === null ? ' UTC' : ''}`;

  return (
    <div className="stack">
      <PageHead
        title="Intervention Center"
        lede="Hypotheses about weak skills and the interventions that would test them. A model can propose; from approval on, every step is your decision."
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {list.error !== null ? <ErrorDisplay error={list.error} onRetry={list.reload} /> : null}
      <Warnings warnings={list.warnings} />

      <div className="with-margin">
        <Panel title="Interventions">
          {list.error !== null ? (
            <NoData reason={`Interventions could not be read (${list.error.code}).`} />
          ) : list.data === null ? (
            <Loading what="interventions" />
          ) : items.length === 0 ? (
            <NoData
              title="No interventions yet"
              reason="An intervention starts as a hypothesis about a weak skill that the evidence supports. None is supported yet."
              action={<Link to="/competency">Open the competency graph</Link>}
            />
          ) : (
            <ul className="rows intervention-rows">
              {items.map((i) => (
                <li key={i.intervention_id}>
                  <div className="row-main">
                    <span className="row-title">{i.statement}</span>
                    <span className="row-sub">
                      Time budget {formatMinutes(Math.round(i.time_budget_ms / 60_000))}. Created {when(i.created_at)}.
                    </span>
                    <span className="row-sub">Acceptance criterion: {i.acceptance}</span>
                    {i.steps.length === 0 ? null : (
                      <details>
                        <summary>{i.steps.length === 1 ? '1 step' : `${i.steps.length} steps`}</summary>
                        <ol>
                          {i.steps.map((step, n) => (
                            // Steps are plain strings and may repeat: the position is part of the key.
                            <li key={`${n}-${step}`}>{step}</li>
                          ))}
                        </ol>
                      </details>
                    )}
                  </div>
                  <div className="row-end">
                    <ProvenanceBadge label={provenanceOf(i.provenance)} />
                    <InterventionStamp state={i.state} />
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Margin label="How an intervention moves">
          <Panel title="Lifecycle" as="aside">
            <ol className="steps intervention-lifecycle" aria-label="The steps of an intervention, in order">
              {SEQUENCE.map((state) => (
                <li key={state} title={state}>
                  {interventionStateLabel(state)}
                </li>
              ))}
            </ol>
            <p className="small muted">
              An evaluated intervention is <InterventionStamp state="RETAINED" />, <InterventionStamp state="REVISED" /> or <InterventionStamp state="RETIRED" />; a revised one
              returns to proposed.
            </p>
            <p className="small muted">
              Before approval it can be <InterventionStamp state="REJECTED" />. Once approved it can be retired at any point before the retest.
            </p>
          </Panel>

          <Panel title="Diagnosis from the mentor" as="aside">
            {s === null ? (
              today.error === null ? (
                <Loading what="the model status" />
              ) : (
                <NoData reason={`The model status could not be read (${today.error.code}).`} />
              )
            ) : !s.model.available ? (
              <ModelUnavailable reason={s.model.reason} what="Diagnosis" />
            ) : fromModel === 0 ? (
              <p className="muted">Nothing in the list comes from the model.</p>
            ) : (
              <p className="muted">
                {fromModel === 1 ? 'One intervention in the list comes from the model and is' : `${fromModel} interventions in the list come from the model and are`} stamped
                Advice. A model can propose; it cannot decide.
              </p>
            )}
          </Panel>
        </Margin>
      </div>
    </div>
  );
}
