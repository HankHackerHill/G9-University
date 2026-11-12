import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { G9Error } from '../../ipc/types';
import { formatDateTime, shortHash } from '../../lib/format';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard } from '../components/ApprovalCard';
import { Badge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { PageHead } from '../components/PageHead';

export function ApprovalsAudit() {
  const today = useToday();
  const client = useIpc();
  const approvals = useCommand('approvals.list', {});
  const audit = useCommand('audit.recent', { limit: 50 });
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [decideWarnings, setDecideWarnings] = useState<readonly string[]>([]);

  const decide = async (approvalId: string, decision: 'APPROVE' | 'REJECT') => {
    setBusy(true);
    const r = await client.invoke('approvals.decide', { approval_id: approvalId, decision });
    setError(isG9Error(r) ? r : null);
    setDecideWarnings(warningsOf(r));
    setBusy(false);
    approvals.reload();
    audit.reload();
  };

  const list = approvals.data?.approvals ?? [];

  return (
    <div>
      <PageHead title="Approvals / Audit" lede="Operations waiting for your decision, and the hash-chained record of what was done." />
      <p className="muted">
        An approval binds to one exact operation (action, resource, scope, parameter hash, expected revision), expires after 15 minutes and is consumed once.
      </p>
      <p className="banner" role="note" data-testid="confirmation-surface-gap">
        <strong>NOT IMPLEMENTED:</strong> the trusted native confirmation raised from Rust (Addendum A.7, ADR-0014) is <strong>PROPOSED</strong> (SEC-03 OPEN). In this
        build an approval is granted by <code>approvals.decide</code> called from this webview, so script running in the webview could approve its own request. Check the
        operation, resource and parameter hash before you approve.
      </p>
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Warnings warnings={mergeWarnings(approvals.warnings, audit.warnings, decideWarnings)} />
      {approvals.error !== null ? <ErrorDisplay error={approvals.error} onRetry={approvals.reload} /> : null}
      <Panel title="Approval requests">
        {approvals.loading && approvals.data === null ? <Loading what="approvals" /> : null}
        {approvals.data !== null && list.length === 0 ? <NoData reason="no approval requests have been created" /> : null}
        {list.map((a) => (
          <ApprovalCard
            key={a.approval_id}
            approval={a}
            zone={today.zone}
            now={today.now ?? undefined}
            disabled={busy}
            onDecide={(d) => {
              void decide(a.approval_id, d);
            }}
          />
        ))}
      </Panel>
      <Panel title="Audit metadata (hash-chained)">
        {audit.error !== null ? <ErrorDisplay error={audit.error} onRetry={audit.reload} /> : null}
        {audit.loading && audit.data === null ? <Loading what="audit entries" /> : null}
        {audit.data !== null ? (
          <>
            <p>
              Chain:{' '}
              {audit.data.chain_ok === null ? (
                <Badge tone="state">NOT VERIFIED</Badge>
              ) : audit.data.chain_ok ? (
                <Badge tone="verified">INTACT</Badge>
              ) : (
                <Badge tone="off">BROKEN</Badge>
              )}{' '}
              <span className="muted">row_hash = sha256(prev_hash ‖ canonical_json(row)); metadata only, no raw prompts or private content.</span>
            </p>
            <Table
              caption="Most recent audit rows"
              headers={['When', 'Actor', 'Operation', 'Resource', 'Decision', 'Outcome', 'Row hash']}
              rows={audit.data.entries.map((e) => ({
                key: e.id,
                cells: [
                  formatDateTime(e.occurred_at, today.zone),
                  `${e.actor_kind} ${e.actor_id}`,
                  <code>{e.operation}</code>,
                  e.resource_kind === null ? '—' : `${e.resource_kind} ${e.resource_id ?? ''}`,
                  e.decision,
                  e.outcome,
                  <span className="hash" title={e.row_hash}>
                    {shortHash(e.row_hash)}
                  </span>,
                ],
              }))}
              emptyText="No audit rows."
            />
          </>
        ) : null}
      </Panel>
    </div>
  );
}
