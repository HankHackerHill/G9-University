import type { ApprovalRequest } from '../../ipc/operations';
import type { IanaZone } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { StateBadge } from './Badge';

export interface ApprovalCardProps {
  approval: ApprovalRequest;
  zone: IanaZone;
  now?: number | undefined;
  onDecide?: (decision: 'APPROVE' | 'REJECT') => void;
  onApply?: () => void;
  disabled?: boolean;
}

/**
 * Shows exactly what an approval binds to: action, resource, parameter hash, expiry.
 * Approve/Reject are requests to the backend (Addendum A.7: the webview requests, the trusted
 * surface grants); "Apply" re-issues the original operation with the approval id.
 */
export function ApprovalCard({ approval, zone, now, onDecide, onApply, disabled = false }: ApprovalCardProps) {
  const expired = now !== undefined && now > approval.expires_at;
  return (
    <article className="panel" aria-label={`Approval ${approval.approval_id}`} data-testid="approval-card">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <strong>{approval.summary}</strong>
        <StateBadge state={approval.state} />
      </div>
      <dl>
        <div className="row">
          <dt className="muted">Approval id</dt>
          <dd>
            <code>{approval.approval_id}</code>
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Operation</dt>
          <dd>
            <code>{approval.operation_name}</code> · action <code>{approval.operation.action}</code>
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Resource</dt>
          <dd>
            <code>{approval.operation.resource.kind}</code> <code>{approval.operation.resource.id}</code>
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Scope</dt>
          <dd>
            {approval.operation.scope.domain} / {approval.operation.scope.mode}
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Parameters SHA-256</dt>
          <dd>
            <span className="hash" data-testid="approval-hash">
              {approval.operation.parameters_sha256}
            </span>
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Expected revision</dt>
          <dd>{approval.expected_revision ?? '—'}</dd>
        </div>
        <div className="row">
          <dt className="muted">Requested</dt>
          <dd>
            <time dateTime={new Date(approval.requested_at).toISOString()}>{formatDateTime(approval.requested_at, zone)}</time>
          </dd>
        </div>
        <div className="row">
          <dt className="muted">Expires</dt>
          <dd>
            <time dateTime={new Date(approval.expires_at).toISOString()} data-testid="approval-expiry">
              {formatDateTime(approval.expires_at, zone)}
            </time>
            {expired ? ' (expired)' : null}
          </dd>
        </div>
      </dl>
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
    </article>
  );
}
