import type { ApprovalRequest } from '../../ipc/operations';
import type { IanaZone } from '../../ipc/types';
import { domainLabel, operationLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase } from '../../lib/format';
import { StateBadge } from './Badge';

/**
 * After a cancelled native confirmation (`approval.not_confirmed`): the operator's answer, not an
 * error. Nothing was approved and the request stays pending until it is decided or expires.
 */
export function ConfirmationCancelled() {
  return (
    <p className="muted" role="status" data-testid="flow-confirmation-cancelled">
      Nothing was approved: you cancelled the confirmation. The request keeps waiting until you decide it or it expires.
    </p>
  );
}

export interface ApprovalCardProps {
  approval: ApprovalRequest;
  zone: IanaZone;
  now?: number | undefined;
  onDecide?: (decision: 'APPROVE' | 'REJECT') => void;
  onApply?: () => void;
  disabled?: boolean;
}

/**
 * A voucher: one operation waiting for the operator's decision. It says what will happen in
 * plain words, then exactly what the approval covers: the operation, the record, the area, the
 * fingerprint of the values, the revision and the expiry. "Apply" re-issues the original
 * operation with the approval id.
 *
 * Addendum A.7, ADR-0014: this page only asks. Approve sends `approvals.decide`; the desktop
 * shell then raises a native confirmation dialog whose text Rust writes from the stored request,
 * and only the operator's answer there grants the approval (a cancelled dialog answers
 * `approval.not_confirmed`).
 */
export function ApprovalCard({ approval, zone, now, onDecide, onApply, disabled = false }: ApprovalCardProps) {
  const expired = now !== undefined && now > approval.expires_at;
  const { operation } = approval;
  return (
    <article className="slip" aria-label={`Approval ${approval.approval_id}`} data-testid="approval-card">
      <div className="panel-head">
        <h3>{operationLabel(approval.operation_name)}</h3>
        <StateBadge state={approval.state} />
      </div>
      <p>{approval.summary}</p>
      <dl className="pairs">
        <dt>Operation</dt>
        <dd>
          <code>{approval.operation_name}</code>, action <code>{operation.action}</code>
        </dd>
        <dt>Record</dt>
        <dd>
          {sentenceCase(operation.resource.kind)} <code>{operation.resource.id}</code>
        </dd>
        <dt>Area</dt>
        <dd>
          {domainLabel(operation.scope.domain)}, {operation.scope.mode.toLowerCase()} mode
        </dd>
        <dt title="Keyed fingerprint (HMAC-SHA-256 under this installation’s key) of the exact values this approval covers. A different value needs a new approval.">Fingerprint</dt>
        <dd>
          <span className="hash" data-testid="approval-hash">
            {operation.parameters_sha256}
          </span>
        </dd>
        <dt>Record revision</dt>
        <dd>{approval.expected_revision ?? 'Not tied to a revision'}</dd>
        <dt>Requested</dt>
        <dd>
          <time dateTime={new Date(approval.requested_at).toISOString()}>{formatDateTime(approval.requested_at, zone)}</time>
        </dd>
        <dt>Expires</dt>
        <dd>
          <time dateTime={new Date(approval.expires_at).toISOString()} data-testid="approval-expiry">
            {formatDateTime(approval.expires_at, zone)}
          </time>
          {expired ? ' (expired)' : null}
        </dd>
        <dt>Request</dt>
        <dd>
          <code>{approval.approval_id}</code>
        </dd>
      </dl>
      {approval.state === 'PENDING' && onDecide !== undefined ? (
        <p className="field-hint" role="note" data-testid="approval-native-confirmation">
          Approve opens a confirmation window drawn by the application, outside this page. The approval is granted only when you confirm there.
        </p>
      ) : null}
      {(approval.state === 'PENDING' && onDecide !== undefined) || (approval.state === 'APPROVED' && onApply !== undefined) ? (
        <div className="row">
          {approval.state === 'PENDING' && onDecide !== undefined ? (
            <>
              <button
                type="button"
                className="primary"
                disabled={disabled || expired}
                onClick={() => {
                  onDecide('APPROVE');
                }}
              >
                Approve
              </button>
              <button
                type="button"
                className="danger"
                disabled={disabled || expired}
                onClick={() => {
                  onDecide('REJECT');
                }}
              >
                Reject
              </button>
            </>
          ) : null}
          {approval.state === 'APPROVED' && onApply !== undefined ? (
            <button type="button" className="primary" disabled={disabled || expired} onClick={onApply}>
              Apply with approval
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}
