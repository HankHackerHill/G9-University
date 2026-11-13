import { SOURCE_STATES, type SourceState } from '../../ipc/types';
import { stateLabel } from '../../lib/copy';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Stat } from '../components/Stat';
import { useToday } from '../hooks/useToday';

/**
 * The states a source leaves the sequence through (`crates/g9-domain/src/state.rs`): an approved
 * reference can be superseded, and any source can be revoked. The others are the steps from
 * untrusted to approved reference, in the order the machine walks them.
 */
const OFF_SEQUENCE: readonly SourceState[] = ['SUPERSEDED', 'REVOKED'];
const SEQUENCE: readonly SourceState[] = SOURCE_STATES.filter((s) => !OFF_SEQUENCE.includes(s));

/** References the v6.0 handoff (section 14) names as candidates. None is registered and none is verified. */
const CANDIDATES: readonly string[] = [
  'OWASP Web Security Testing Guide, OWASP ASVS/API',
  'MITRE ATT&CK and ATLAS',
  'NIST NICE / AI RMF',
  'Tauri, SQLite, FSRS upstream documentation',
];

/**
 * Research reference system (M15). The only thing this screen reads is `system.status`: whether
 * the window may reach the network (off by default, and visibly so). The source register
 * (`research.sources`) is proposed, not built, so no source is listed and none is counted.
 */
export function ResearchCenter() {
  const today = useToday();
  const s = today.status;

  return (
    <div className="stack">
      <PageHead title="Research Center" lede="Reference sources, how far each can be trusted, and whether this window may reach the network." />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}

      <section className="bento" aria-label="Network and sources">
        <div data-span="6">
          {s === null ? (
            <Stat
              label="Network"
              value="No data"
              standing="missing"
              foot={today.error === null ? 'The network status is still being read.' : `The network status could not be read (${today.error.code}).`}
            />
          ) : s.network.egress_allowed ? (
            <Stat
              label="Network"
              value={<span data-testid="network-indicator">Egress allowed</span>}
              standing="doubt"
              title="network.egress_allowed: true"
              foot={
                <>
                  An operation you approved is in progress. Each fetch still needs <span title="Capability: NETWORK_EGRESS">its own approval</span>, naming the exact host.
                </>
              }
            />
          ) : (
            <Stat
              label="Network"
              value={<span data-testid="network-indicator">Off</span>}
              title="network.egress_allowed: false"
              foot={
                <>
                  The application works offline and denies what is not granted. Each fetch needs <span title="Capability: NETWORK_EGRESS">your approval</span>, naming the exact
                  host, and only you can give it.
                </>
              }
            />
          )}
        </div>
        <div data-span="6">
          <Stat label="Sources" value="No data" standing="missing" foot="There is no source register to count yet." />
        </div>
      </section>

      <Panel title="Approved sources">
        <NoData title="Not built yet" reason="The source register is planned (research.sources), not built. No source is registered, and nothing is fetched or stored for one." />
      </Panel>

      <Panel title="How a source becomes a reference">
        <ol className="steps" aria-label="The steps of a source, in order">
          {SEQUENCE.map((state) => (
            <li key={state} title={state}>
              {stateLabel(state)}
            </li>
          ))}
        </ol>
        <p className="small muted measure">
          An approved reference can be <StateBadge state="SUPERSEDED" /> by a newer one, and a source can be <StateBadge state="REVOKED" /> at any step. An evaluation that
          rejects a source returns it to quarantine.
        </p>
        <p className="small muted measure">An answer drawn from sources must cite passages you can inspect, and keep what a source claims apart from what has been validated.</p>
      </Panel>

      <Panel
        title="Candidate references"
        note={
          <>
            <ProvenanceBadge label="PROPOSED" /> Named in the plan, not registered and not verified. The licence and free-tier terms of each must be checked before it is
            registered.
          </>
        }
      >
        <ul>
          {CANDIDATES.map((c) => (
            <li key={c}>{c}</li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
