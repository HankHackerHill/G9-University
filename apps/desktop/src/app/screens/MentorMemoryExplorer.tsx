import { Link } from 'react-router-dom';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin, MarginNote } from '../components/Margin';
import { Loading, ModelUnavailable, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';

/**
 * Learner hypotheses (M09). A hypothesis is a proposal: model output can only create one, and
 * accepting or rejecting it is an operator command. A register of its own (`memory.hypotheses`)
 * is proposed, not built, so the list is read from the interventions still in state HYPOTHESIS.
 */
export function MentorMemoryExplorer() {
  const today = useToday();
  const interventions = useCommand('interventions.list', { state: 'HYPOTHESIS' });
  const hypotheses = interventions.data?.interventions ?? [];
  const s = today.status;

  return (
    <div className="stack">
      <PageHead
        title="Mentor Memory Explorer"
        lede="What the mentor holds about your learning, where each item came from, and how far it can be trusted. The mentor proposes; you decide."
      />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {interventions.error !== null ? <ErrorDisplay error={interventions.error} onRetry={interventions.reload} /> : null}
      <Warnings warnings={interventions.warnings} />

      <div className="with-margin">
        <Panel
          title="Hypotheses"
          note={
            <>
              Read from the interventions that are still hypotheses; the same records are in the <Link to="/interventions">Intervention Center</Link>. A register of the mentor’s memory (
              <code>memory.hypotheses</code>) is planned, not built.
            </>
          }
        >
          {interventions.error !== null ? (
            <NoData reason={`Hypotheses could not be read (${interventions.error.code}).`} />
          ) : interventions.data === null ? (
            <Loading what="hypotheses" />
          ) : hypotheses.length > 0 ? (
            <ul className="rows">
              {hypotheses.map((h) => (
                <li key={h.intervention_id}>
                  <div className="row-main">
                    <span className="row-title">{h.statement}</span>
                  </div>
                  <div className="row-end">
                    <ProvenanceBadge label="PROPOSED" />
                    <StateBadge state={h.state} />
                  </div>
                </li>
              ))}
            </ul>
          ) : s !== null && !s.model.available ? (
            <ModelUnavailable reason={s.model.reason} what="Hypotheses" />
          ) : (
            <NoData title="No hypotheses yet" reason="A hypothesis appears here when the evidence supports one. None is supported yet." />
          )}
        </Panel>

        <Margin label="How to read a hypothesis">
          <MarginNote title="What a hypothesis is">
            <p>A statement about your learning that the evidence may support: a misconception, a strength, a preference or a blocker. It is a proposal, never a record.</p>
          </MarginNote>
          <MarginNote title="Who decides">
            <p>The mentor proposes; you accept or reject, and you can retire it later. Rejecting a hypothesis also withdraws the plans that were built on it.</p>
          </MarginNote>
          <Panel title="What the stamps mean" as="aside">
            <dl className="pairs">
              <dt>
                <ProvenanceBadge label="VERIFIED" />
              </dt>
              <dd>Written by you, or derived by fixed rules.</dd>
              <dt>
                <ProvenanceBadge label="PROPOSED" />
              </dt>
              <dd>A hypothesis or a plan that waits for your decision. Never used as authority.</dd>
              <dt>
                <ProvenanceBadge label="RECOMMENDATION" />
              </dt>
              <dd>Output of a model or a skill. It can propose; it cannot decide.</dd>
            </dl>
          </Panel>
        </Margin>
      </div>
    </div>
  );
}
