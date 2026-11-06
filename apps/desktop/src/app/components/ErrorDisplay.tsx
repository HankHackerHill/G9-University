import type { G9Error } from '../../ipc/types';
import { presentError } from '../../lib/error-text';

function detailsHash(error: G9Error): string | null {
  const d = error.details;
  if (typeof d === 'object' && d !== null && !Array.isArray(d) && typeof d.parameters_sha256 === 'string') {
    return d.parameters_sha256;
  }
  return null;
}

/** Maps every one of the 13 `ErrorCategory` values to operator text. Shows code, never internals. */
export function ErrorDisplay({ error, onRetry }: { error: G9Error; onRetry?: () => void }) {
  const p = presentError(error);
  const hash = detailsHash(error);
  return (
    <div className="error" role="alert" data-severity={p.severity} data-category={error.category}>
      <strong>{p.title}</strong> <span className="badge badge-state">{error.category}</span>
      <div>
        <code>{error.code}</code> — {error.message}
      </div>
      <div className="muted">{p.guidance}</div>
      {error.approval_id !== undefined ? (
        <div>
          Approval request: <code>{error.approval_id}</code>
          {hash !== null ? (
            <>
              {' '}
              bound to <span className="hash">{hash}</span>
            </>
          ) : null}
        </div>
      ) : null}
      {p.retryable && onRetry !== undefined ? (
        <button type="button" onClick={onRetry}>
          Retry
        </button>
      ) : null}
    </div>
  );
}
