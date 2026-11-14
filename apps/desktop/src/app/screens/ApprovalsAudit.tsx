import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { ApprovalRequest, AuditEntry } from '../../ipc/operations';
import type { ApprovalState, G9Error, IanaZone, UtcMillis } from '../../ipc/types';
import { domainLabel, operationLabel, stateLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase, shortHash } from '../../lib/format';
import { hasWarning, mergeWarnings, warningCode, warningsOf } from '../../lib/warnings';
import { ApprovalCard } from '../components/ApprovalCard';
import { Badge, ProvenanceBadge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin, MarginNote } from '../components/Margin';
import { Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { Stat } from '../components/Stat';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';

/** How many audit entries the screen asks for (`audit.recent` `limit`). */
const AUDIT_LIMIT = 50;
/** The backend's disclosure that the list holds this run's entries only (g9-app `audit.rs`). */
const AUDIT_SCOPE_WARNING = 'audit.recent.process_scoped';
/** How an entry is chained (g9-storage); the spaces do not break, so the rule stays on one line. */
const HASH_RULE = 'row_hash\u00a0=\u00a0sha256(prev_hash\u00a0‖\u00a0canonical_json(row))';
/** Decided requests are counted in the order a request can end. */
const DECIDED_ORDER: readonly ApprovalState[] = ['APPROVED', 'CONSUMED', 'REJECTED', 'EXPIRED', 'REVOKED'];

function Moment({ at, zone }: { at: UtcMillis; zone: IanaZone }) {
  return <time dateTime={new Date(at).toISOString()}>{formatDateTime(at, zone)}</time>;
}

/** `1 approved, 2 used`: how the decided requests ended, in the operator's words. */
function decidedSummary(decided: readonly ApprovalRequest[]): string {
  return DECIDED_ORDER.map((state) => ({ state, n: decided.filter((a) => a.state === state).length }))
    .filter((x) => x.n > 0)
    .map((x) => `${x.n} ${stateLabel(x.state).toLowerCase()}`)
    .join(', ');
}

/** When a request was decided and what became of it, from the times the backend recorded. */
function decidedLine(a: ApprovalRequest, zone: IanaZone): string {
  const parts: string[] = [];
  if (a.decided_at !== null) {
    parts.push(`decided ${formatDateTime(a.decided_at, zone)}`);
  }
  if (a.consumed_at !== null) {
    parts.push(`used ${formatDateTime(a.consumed_at, zone)}`);
  }
  if (a.state === 'APPROVED') {
    parts.push(`not used yet, expires ${formatDateTime(a.expires_at, zone)}`);
  }
  if (a.state === 'EXPIRED') {
    parts.push(`expired ${formatDateTime(a.expires_at, zone)} ${a.decided_at === null ? 'without a decision' : 'without being used'}`);
  }
  if (parts.length === 0) {
    return 'No decision time is recorded.';
  }
  const text = parts.join(', ');
  return `${text.charAt(0).toUpperCase()}${text.slice(1)}.`;
}

/**
 * A decided request is a record, so it is a ruled line, not a voucher: what it was, how it ended
 * and when. Exactly what the approval covered stays one step away, under the line.
 */
function DecidedRow({ approval: a, zone }: { approval: ApprovalRequest; zone: IanaZone }) {
  const { operation } = a;
  return (
    <li>
      <div className="row-main">
        <span className="row-title">{operationLabel(a.operation_name)}</span>
        <span className="row-sub">{a.summary}</span>
        <span className="row-sub">{decidedLine(a, zone)}</span>
      </div>
      <div className="row-end">
        <StateBadge state={a.state} />
      </div>
      <details>
        <summary>Show what it covered</summary>
        <dl className="pairs">
          <dt>Operation</dt>
          <dd>
            <code>{a.operation_name}</code>, action <code>{operation.action}</code>
          </dd>
          <dt>Record</dt>
          <dd>
            {sentenceCase(operation.resource.kind)} <code>{operation.resource.id}</code>
          </dd>
          <dt>Area</dt>
          <dd>
            {domainLabel(operation.scope.domain)}, {operation.scope.mode.toLowerCase()} mode
          </dd>
          <dt title="Keyed fingerprint (HMAC-SHA-256 under this installation’s key) of the exact values this approval covered. A different value needs a new approval.">Fingerprint</dt>
          <dd>
            <span className="hash">{operation.parameters_sha256}</span>
          </dd>
          <dt>Record revision</dt>
          <dd>{a.expected_revision ?? 'Not tied to a revision'}</dd>
          <dt>Requested</dt>
          <dd>
            <Moment at={a.requested_at} zone={zone} />
          </dd>
          <dt>Expiry</dt>
          <dd>
            <Moment at={a.expires_at} zone={zone} />
          </dd>
          <dt>Request</dt>
          <dd>
            <code>{a.approval_id}</code>
          </dd>
        </dl>
      </details>
    </li>
  );
}

/** Who acted, in words; the backend's exact kind and id stay in the `title`. */
function Actor({ entry }: { entry: AuditEntry }) {
  const exact = `${entry.actor_kind} ${entry.actor_id}`;
  switch (entry.actor_kind) {
    case 'OPERATOR':
      return <span title={exact}>You</span>;
    case 'SYSTEM':
      return <span title={exact}>The application</span>;
    case 'MODEL':
      return <span title={exact}>The model</span>;
    case 'SKILL':
      return (
        <span title={exact}>
          Skill <code>{entry.actor_id}</code>
        </span>
      );
  }
}

/**
 * Approvals/Audit. The requests that wait for the operator are vouchers (the only things to
 * operate here); decided requests and the audit trail are records. The page only asks: Approve
 * sends `approvals.decide` and the desktop shell's native confirmation grants (ADR-0014).
 */
export function ApprovalsAudit() {
  const today = useToday();
  const client = useIpc();
  const approvals = useCommand('approvals.list', {});
  const audit = useCommand('audit.recent', { limit: AUDIT_LIMIT });
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [decideWarnings, setDecideWarnings] = useState<readonly string[]>([]);
  const [cancelled, setCancelled] = useState(false);
  // The request as the backend returned it from the last decision, for the receipt.
  const [decision, setDecision] = useState<ApprovalRequest | null>(null);

  const decide = async (approvalId: string, choice: 'APPROVE' | 'REJECT') => {
    setBusy(true);
    const r = await client.invoke('approvals.decide', { approval_id: approvalId, decision: choice });
    // A cancelled native confirmation is the operator's answer, not a failure.
    const declined = isG9Error(r) && r.code === 'approval.not_confirmed';
    setCancelled(declined);
    setError(isG9Error(r) && !declined ? r : null);
    setDecision(isG9Error(r) ? null : r.result);
    setDecideWarnings(warningsOf(r));
    setBusy(false);
    approvals.reload();
    audit.reload();
  };

  // Derived values exist only when their read succeeded; a pending or failed read is said in words.
  const list = approvals.data?.approvals ?? null;
  const pending = list === null ? [] : list.filter((a) => a.state === 'PENDING');
  const decided = list === null ? [] : list.filter((a) => a.state !== 'PENDING');
  const entries = audit.data?.entries ?? null;
  const chain = audit.data === null ? undefined : audit.data.chain_ok;
  const scoped = hasWarning(audit.warnings, AUDIT_SCOPE_WARNING);
  // "Verified" is claimed for the listed entries only when the backend verified the chain and
  // disclosed nothing about this read except that the list covers this run.
  const chainVerified = chain === true && audit.warnings.every((w) => warningCode(w) === AUDIT_SCOPE_WARNING);
  // A decision stays locked until the list has reloaded: a second press would decide a request the backend has already moved on.
  const locked = busy || approvals.loading;
  const approvalsReason = approvals.error === null ? 'Approval requests are still being read.' : `Approval requests could not be read (${approvals.error.code}).`;
  const auditReason = audit.error === null ? 'The audit trail is still being read.' : `The audit trail could not be read (${audit.error.code}).`;

  return (
    <div className="stack">
      <PageHead title="Approvals / Audit" lede="Decide the protected operations that wait for you, and read the hash-chained record of what was done." />
      {approvals.error !== null ? <ErrorDisplay error={approvals.error} onRetry={approvals.reload} /> : null}
      {audit.error !== null ? <ErrorDisplay error={audit.error} onRetry={audit.reload} /> : null}
      {chain === false ? (
        <div className="banner" role="alert" data-testid="chain-broken">
          <strong>The audit chain is broken.</strong> At least one entry does not match the hash of the one before it, so the record below cannot be trusted as it stands. Stop here
          and verify a backup in Settings/Recovery before you continue.
        </div>
      ) : null}
      <Warnings warnings={mergeWarnings(approvals.warnings, audit.warnings, decideWarnings)} />

      <section className="bento" aria-label="Approvals and audit in numbers">
        <div data-span="4">
          {list === null ? (
            <Stat label="Waiting for you" value="No data" standing="missing" foot={approvalsReason} />
          ) : (
            <Stat
              label="Waiting for you"
              value={pending.length}
              standing={pending.length > 0 ? 'doubt' : 'posted'}
              foot={pending.length === 1 ? 'approval to decide' : 'approvals to decide'}
            />
          )}
        </div>
        <div data-span="4">
          {chain === undefined ? (
            <Stat label="Audit chain" value="No data" standing="missing" foot={auditReason} />
          ) : chain === null ? (
            <Stat label="Audit chain" value="Not verified" standing="missing" foot="The backend returned no result for the check of the chain." />
          ) : chain ? (
            <Stat label="Audit chain" value="Intact" foot="Every entry matches the hash of the one before it." />
          ) : (
            <Stat label="Audit chain" value="Broken" standing="owed" foot="An entry does not match the hash of the one before it." />
          )}
        </div>
        <div data-span="4">
          {entries === null ? (
            <Stat label="Audit entries shown" value="No data" standing="missing" foot={auditReason} />
          ) : (
            <Stat
              label="Audit entries shown"
              value={entries.length}
              foot={
                scoped
                  ? 'Since the application started. Earlier entries were checked with the chain and are not listed.'
                  : `The most recent entries, ${AUDIT_LIMIT} at most.`
              }
            />
          )}
        </div>
      </section>

      <div className="with-margin">
        <div className="stack">
          <Panel title="Waiting for you">
            <div className="stack-sm">
              {error !== null ? <ErrorDisplay error={error} /> : null}
              {cancelled ? (
                <p className="muted" role="status" data-testid="confirmation-cancelled">
                  Nothing was approved: you cancelled the confirmation. The request keeps waiting until you decide it or it expires.
                </p>
              ) : null}
              {decision !== null ? (
                <p className="receipt" role="status" data-testid="decision-receipt">
                  Request {stateLabel(decision.state).toLowerCase()}: {operationLabel(decision.operation_name)}. It is listed under Decided.
                </p>
              ) : null}
              {list === null ? (
                approvals.error === null ? (
                  <Loading what="approval requests" />
                ) : (
                  <NoData reason={`Approval requests could not be read (${approvals.error.code}), so nothing can be decided here.`} />
                )
              ) : pending.length === 0 ? (
                <p className="muted">Nothing waits for your approval.</p>
              ) : (
                pending.map((a) => (
                  <ApprovalCard
                    key={a.approval_id}
                    approval={a}
                    zone={today.zone}
                    now={today.now ?? undefined}
                    disabled={locked}
                    onDecide={(d) => {
                      void decide(a.approval_id, d);
                    }}
                  />
                ))
              )}
            </div>
          </Panel>

          <Panel title="Decided" note="Requests you approved or rejected, and requests that expired. An approved request is used once, by the operation it covers.">
            {list === null ? (
              approvals.error === null ? (
                <Loading what="decided requests" />
              ) : (
                <NoData reason={`Decided requests could not be read (${approvals.error.code}).`} />
              )
            ) : decided.length === 0 ? (
              <NoData title="No decided requests yet" reason="A request appears here once you approve or reject it, or when it expires." />
            ) : (
              <details className="day-group">
                <summary>
                  {decided.length} {decided.length === 1 ? 'request' : 'requests'}: <span className="muted">{decidedSummary(decided)}</span>
                </summary>
                <ul className="rows">
                  {decided.map((a) => (
                    <DecidedRow key={a.approval_id} approval={a} zone={today.zone} />
                  ))}
                </ul>
              </details>
            )}
          </Panel>
        </div>

        {/* The reasons sit beside the requests, the way a ledger carries notes beside its entries. */}
        <Margin label="About approvals">
          <MarginNote title="What an approval covers">
            <p>
              One exact operation: its action, its record, its area, the fingerprint of its values and the record revision. It expires after 10 minutes and is used once.
            </p>
          </MarginNote>
          <MarginNote title="Where it is granted">
            <p role="note" data-testid="native-confirmation-note">
              Approve opens a confirmation window drawn by the application itself, outside this page. It shows the operation, the record and the expiry as stored. Only your
              answer there grants the approval: a script running in this page can ask for an approval but cannot grant one. Reject needs no confirmation.
            </p>
          </MarginNote>
        </Margin>
      </div>

      <Panel
        title="Audit trail"
        actions={chainVerified ? <ProvenanceBadge label="VERIFIED" /> : undefined}
        note={
          <>
            Each entry is hashed together with the one before it, so a changed or removed entry breaks the chain. Entries hold metadata only: no prompts and no private
            content. Newest first, times in {today.zone}. The rule is <code>{HASH_RULE}</code>.
          </>
        }
      >
        {entries === null ? (
          audit.error === null ? (
            <Loading what="audit entries" />
          ) : (
            <NoData reason={`The audit trail could not be read (${audit.error.code}), so no entry can be shown.`} />
          )
        ) : (
          <Table
            caption="Audit entries, newest first"
            headers={['Time', 'Actor', 'Operation and record', 'Outcome', 'Entry hash']}
            widths={['10.5rem', '9.5rem', undefined, '11.5rem', '8rem']}
            emptyText="The backend listed no audit entry."
            rows={entries.map((e) => ({
              key: e.id,
              cells: [
                <Moment at={e.occurred_at} zone={today.zone} />,
                <Actor entry={e} />,
                <span className="row-main">
                  <code>{e.operation}</code>
                  {e.resource_kind === null ? null : (
                    <span className="row-sub">
                      {sentenceCase(e.resource_kind)} {e.resource_id === null ? null : <code>{e.resource_id}</code>}
                    </span>
                  )}
                </span>,
                <span className="row-main">
                  <span>
                    <Badge tone={e.outcome === 'OK' ? 'verified' : 'off'} label={`Outcome: ${e.outcome === 'OK' ? 'done' : 'not done'}`} title={e.outcome}>
                      {e.outcome === 'OK' ? 'Done' : 'Not done'}
                    </Badge>
                  </span>
                  <code className="row-sub" title="The decision recorded for this operation">
                    {e.decision}
                  </code>
                </span>,
                <span className="hash" title={e.row_hash}>
                  {shortHash(e.row_hash)}
                </span>,
              ],
            }))}
          />
        )}
      </Panel>
    </div>
  );
}
