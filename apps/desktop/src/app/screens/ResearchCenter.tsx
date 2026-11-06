import { SOURCE_STATES } from '../../ipc/types';
import { Badge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { useToday } from '../hooks/useToday';

/** Research reference system (M15). Network egress is OFF by default and visibly so. */
export function ResearchCenter() {
  const today = useToday();
  const s = today.status;

  return (
    <div>
      <h1>Research Center</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      <Panel title="Network">
        {s === null ? (
          <Loading what="network status" />
        ) : (
          <p data-testid="network-indicator">
            Network egress: {s.network.egress_allowed ? <Badge tone="proposed">ALLOWED (approved operation in progress)</Badge> : <Badge tone="off">OFF</Badge>}
            <span className="muted"> — every fetch needs a per-operation NETWORK_EGRESS approval naming the exact host (operator-only capability).</span>
          </p>
        )}
      </Panel>
      <Panel title="Approved sources">
        <NoData reason="no approved reference sources are registered; research.sources is PROPOSED" />
        <p className="muted">
          Source lifecycle: {SOURCE_STATES.map((st) => <StateBadge key={st} state={st} />)} — retrieval answers must cite inspectable chunks and separate the source claim
          from validated truth.
        </p>
      </Panel>
      <Panel title="Candidate reference mappings (not yet verified)">
        <ul>
          <li>OWASP Web Security Testing Guide, OWASP ASVS/API</li>
          <li>MITRE ATT&CK and ATLAS</li>
          <li>NIST NICE / AI RMF</li>
          <li>Tauri, SQLite, FSRS upstream documentation</li>
        </ul>
        <p className="muted">Licenses and free-tier terms must be verified individually before registration (handoff §14).</p>
      </Panel>
    </div>
  );
}
